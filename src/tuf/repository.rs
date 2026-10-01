use std::{collections::HashSet, fs, path::Path};

use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, Utc};
use futures_util::{
    FutureExt,
    future::BoxFuture,
    io::{AsyncRead, AsyncReadExt, Cursor},
};
use tracing::{debug, info, trace, warn};
use tuf::{
    client::{Client, Config},
    metadata::{Metadata, MetadataPath, MetadataVersion, RawSignedMetadata, TargetPath},
    pouf::Pouf1,
    repository::{FileSystemRepository, RepositoryProvider},
};
use url::Url;

use crate::{
    download::DownloadOptions,
    errors::RustupError,
    tuf::{
        TufConfig, TufMode,
        consts::{METADATA_PREFIX, ROOT, TARGETS_PREFIX},
    },
    utils,
};

/// A TUF client over the dist repository: a local metadata cache under
/// [`TufConfig::home`] kept in sync with the remote at [`TufConfig::server`].
pub(crate) struct TufRepository {
    mode: TufMode,
    ignore_failures: bool,
    ignore_expiry_after: Option<DateTime<Utc>>,
    client: Client<Pouf1, FileSystemRepository<Pouf1>, Remote>,
}

impl TufRepository {
    /// Opens the repository at [`TufConfig::server`]. The remote's own
    /// metadata and target downloads use `options` and are never themselves
    /// TUF-verified, which is what keeps this from recursing.
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn open(config: &TufConfig, options: DownloadOptions) -> anyhow::Result<Self> {
        let location = config.server.as_str();
        info!("syncing TUF database from {location}");
        if config.mode == TufMode::Warn {
            warn!("TUF is set to 'warn' mode, and validation failures will be ignored");
        }
        if config.ignore_failures {
            warn!("RUSTUP_TUF_IGNORE is set, and all TUF validation failures will be ignored");
        }
        if let Some(date) = config.ignore_expiry_after {
            warn!(
                "RUSTUP_TUF_IGNOREDATE is set, and TUF metadata expiry after {date} will be ignored"
            );
        }
        debug!(
            location,
            home = %config.home.display(),
            mode = %config.mode,
            ignore_failures = config.ignore_failures,
            ignore_expiry_after = ?config.ignore_expiry_after,
            "opening TUF repository"
        );

        utils::ensure_dir_exists("tuf home", &config.home)?;
        let local = FileSystemRepository::new(&config.home);
        let remote = Remote::from_location(location, options)?;
        let bytes = match &config.root {
            Some(path) => {
                debug!(path = %path.display(), "using trusted TUF root from RUSTUP_TUF_ROOT");
                utils::read_file("tuf root", path)?.into_bytes()
            }
            None => {
                debug!("using trusted TUF root shipped with rustup");
                ROOT.to_vec()
            }
        };
        trace!(len = bytes.len(), "read trusted TUF root");
        let root = RawSignedMetadata::new(bytes);
        let client = Client::with_trusted_root(Config::default(), &root, local, remote)
            .await
            .with_context(|| format!("error loading TUF repository from '{location}'"))?;

        let repository = Self {
            mode: config.mode,
            ignore_failures: config.ignore_failures,
            ignore_expiry_after: config.ignore_expiry_after,
            client,
        };
        repository.trace_database("loaded TUF trust database");
        Ok(repository)
    }

    /// Updates the metadata from the remote, verifying the chain of trust.
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn verify(&mut self) -> anyhow::Result<Verification> {
        if self.mode == TufMode::Off {
            debug!("TUF mode is off, skipping metadata update");
            return Ok(Verification::Skipped);
        }

        let start_time = self.start_time();
        debug!(%start_time, "updating TUF metadata from remote");
        match self.client.update_with_start_time(&start_time).await {
            Ok(updated) => {
                debug!(updated, "TUF metadata update succeeded");
                self.trace_database("TUF trust database after update");
                self.warn_expired();
                Ok(Verification::Verified)
            }
            Err(err) => {
                debug!(error = %err, "TUF metadata update failed");
                self.tolerate(err.into())
            }
        }
    }

    /// Reads the target named `target`, verified against the trusted metadata
    /// whenever the mode allows it.
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn fetch_target(
        &mut self,
        target: &str,
    ) -> anyhow::Result<(Vec<u8>, Verification)> {
        let path = TargetPath::new(target)?;
        debug!(target, mode = %self.mode, "fetching TUF target");
        if self.mode == TufMode::Off {
            debug!(target, "TUF mode is off, reading target unverified");
            return Ok((self.read_unverified(&path).await?, Verification::Skipped));
        }

        let err = match self.read_verified(&path).await {
            Ok(bytes) => {
                debug!(target, len = bytes.len(), "TUF target verified");
                return Ok((bytes, Verification::Verified));
            }
            Err(err) => err,
        };
        debug!(target, error = %err, "TUF target verification failed");
        let verification = self.tolerate(err.into())?;
        debug!(
            target,
            "reading TUF target unverified after tolerated failure"
        );
        Ok((self.read_unverified(&path).await?, verification))
    }

    /// Finds the newest target named `filename` under `folder` (searched
    /// recursively), using only the targets metadata; `None` means no role
    /// under `folder` has a target with that name.
    ///
    /// Delegations are walked depth first and loaded lazily: a delegated
    /// role is fetched and verified (through the client's own delegation
    /// walk) only when the search reaches it, so nothing past the match is
    /// downloaded. Within a role the newest match wins, and delegations are
    /// descended into newest first. The dated folders (`2026/`,
    /// `2026/09-16/`, `2026-09-16.toml`) are zero-padded, so "newest" is
    /// simply the greatest path.
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn find_targets(
        &mut self,
        folder: &Path,
        filename: &str,
    ) -> anyhow::Result<Option<String>> {
        let start_time = self.start_time();
        debug!(folder = %folder.display(), filename, "searching TUF targets metadata");
        let matches = |target: &TargetPath| {
            let target = Path::new(target.as_str());
            target.starts_with(folder) && target.file_name() == Some(filename.as_ref())
        };
        let overlaps = |prefix: &TargetPath| {
            let prefix = Path::new(prefix.as_str());
            prefix.starts_with(folder) || folder.starts_with(prefix)
        };

        // The roles still to search, each with the delegated path prefix
        // that led to it (`None` for the top-level role). Children go on the
        // stack oldest first, so the newest is popped, and loaded, first.
        let mut seen = HashSet::new();
        let mut pending = vec![(MetadataPath::targets(), None)];
        while let Some((role, prefix)) = pending.pop() {
            if !seen.insert(role.clone()) {
                trace!(%role, "role already searched, skipping");
                continue;
            }
            if let Some(prefix) = prefix {
                self.load_delegation(&role, &prefix, filename, &start_time)
                    .await?;
            }

            let database = self.client.database();
            let targets = if role == MetadataPath::targets() {
                database.trusted_targets()
            } else {
                database.trusted_delegations().get(&role)
            };
            let Some(targets) = targets else {
                debug!(%role, "role is not in the trusted database, skipping");
                continue;
            };
            trace!(
                %role,
                version = targets.version(),
                targets = targets.targets().len(),
                delegations = targets.delegations().roles().len(),
                "searching role"
            );
            if let Some(target) = targets.targets().keys().filter(|t| matches(t)).max() {
                debug!(%role, %target, roles_searched = seen.len(), "found TUF target");
                return Ok(Some(target.as_str().to_owned()));
            }

            // Each delegation is keyed by its newest path overlapping `folder`.
            let mut delegations = targets
                .delegations()
                .roles()
                .iter()
                .filter_map(|delegation| {
                    let prefix = delegation.paths().iter().filter(|p| overlaps(p)).max()?;
                    Some((delegation.name().clone(), Some(prefix.clone())))
                })
                .collect::<Vec<_>>();
            delegations.sort_unstable_by(|(_, a), (_, b)| a.cmp(b));
            trace!(
                %role,
                delegations = ?delegations.iter().rev().map(|(role, _)| role).collect::<Vec<_>>(),
                "delegations to search, newest first"
            );
            pending.extend(delegations);
        }

        debug!(
            folder = %folder.display(),
            filename,
            roles_searched = seen.len(),
            "no matching TUF target"
        );
        Ok(None)
    }

    /// Makes the client fetch and verify the delegated role `role`, if it
    /// hasn't already, by looking up `filename` under `prefix`, one of the
    /// paths delegated to it. The lookup itself is expected to fail when no
    /// such target exists; only the side effect of loading the role matters.
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    async fn load_delegation(
        &mut self,
        role: &MetadataPath,
        prefix: &TargetPath,
        filename: &str,
        start_time: &DateTime<Utc>,
    ) -> anyhow::Result<()> {
        if self.delegation_loaded(role) {
            trace!(%role, "delegation already loaded");
            return Ok(());
        }

        let probe = Path::new(prefix.as_str()).join(filename);
        let probe = TargetPath::new(probe.to_string_lossy())?;
        debug!(%role, %prefix, %probe, "probing to load delegation");
        match self
            .client
            .fetch_target_description_with_start_time(&probe, start_time)
            .await
        {
            Ok(_) => trace!(%role, %probe, "probe target exists"),
            Err(err) => trace!(
                %role,
                %probe,
                error = %err,
                "probe lookup failed (expected when the probe target does not exist)"
            ),
        }
        debug!(%role, loaded = self.delegation_loaded(role), "probe finished");
        Ok(())
    }

    async fn read_verified(&mut self, path: &TargetPath) -> tuf::Result<Vec<u8>> {
        let start_time = self.start_time();
        trace!(target = %path, %start_time, "reading TUF target through client");
        let mut reader = self
            .client
            .fetch_target_with_start_time(path, &start_time)
            .await?;
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        trace!(target = %path, len = bytes.len(), "read verified TUF target");
        Ok(bytes)
    }

    async fn read_unverified(&self, path: &TargetPath) -> tuf::Result<Vec<u8>> {
        let database = self.client.database();
        let mut candidates = Vec::new();
        if database.trusted_root().consistent_snapshot()
            && let Some(description) = database
                .trusted_targets()
                .and_then(|targets| targets.targets().get(path))
        {
            for digest in description.hashes().values() {
                candidates.push(path.with_hash_prefix(digest)?);
            }
        }
        candidates.push(path.clone());
        trace!(target = %path, ?candidates, "reading TUF target without verification");

        let mut last_err = tuf::Error::TargetNotFound(path.clone());
        for candidate in &candidates {
            let mut reader = match self.client.remote_repo().fetch_target(candidate).await {
                Ok(reader) => reader,
                Err(err) => {
                    trace!(
                        target = %path,
                        %candidate,
                        error = %err,
                        "unverified TUF target candidate failed"
                    );
                    last_err = err;
                    continue;
                }
            };
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            trace!(
                target = %path,
                %candidate,
                len = bytes.len(),
                "read unverified TUF target"
            );
            return Ok(bytes);
        }
        Err(last_err)
    }

    fn delegation_loaded(&self, role: &MetadataPath) -> bool {
        self.client
            .database()
            .trusted_delegations()
            .contains_key(role)
    }

    fn start_time(&self) -> DateTime<Utc> {
        if self.ignore_failures {
            return DateTime::<Utc>::MIN_UTC;
        }

        let now = Utc::now();
        match self.ignore_expiry_after {
            Some(ignore_after) => now.min(ignore_after),
            None => now,
        }
    }

    fn warn_expired(&self) {
        let now = Utc::now();
        let database = self.client.database();
        let roles = [
            ("root", Some(*database.trusted_root().expires())),
            (
                "timestamp",
                database.trusted_timestamp().map(|m| *m.expires()),
            ),
            (
                "snapshot",
                database.trusted_snapshot().map(|m| *m.expires()),
            ),
            ("targets", database.trusted_targets().map(|m| *m.expires())),
        ];
        for (role, expires) in roles {
            if let Some(expires) = expires
                && expires <= now
            {
                warn!("TUF {role} metadata expired at {expires}, ignoring expiry");
            }
        }
    }

    fn tolerate(&self, err: anyhow::Error) -> anyhow::Result<Verification> {
        let reason = match self.mode {
            TufMode::Off => Some("mode is off"),
            TufMode::Warn => Some("mode is warn"),
            TufMode::On if self.ignore_failures => Some("RUSTUP_TUF_IGNORE is set"),
            TufMode::On => None,
        };
        let Some(reason) = reason else {
            debug!(mode = %self.mode, "TUF verification failure is fatal");
            return Err(err.context("TUF verification failed"));
        };

        debug!(reason, "tolerating TUF verification failure");
        warn!("TUF verification failed: {err:#}");
        Ok(Verification::Skipped)
    }

    fn trace_database(&self, message: &str) {
        let database = self.client.database();
        let root = database.trusted_root();
        let timestamp = database.trusted_timestamp();
        let snapshot = database.trusted_snapshot();
        let targets = database.trusted_targets();
        trace!(
            root_version = root.version(),
            root_expires = %root.expires(),
            consistent_snapshot = root.consistent_snapshot(),
            timestamp_version = timestamp.map(|m| m.version()),
            timestamp_expires = timestamp.map(|m| m.expires().to_string()),
            snapshot_version = snapshot.map(|m| m.version()),
            snapshot_expires = snapshot.map(|m| m.expires().to_string()),
            targets_version = targets.map(|m| m.version()),
            targets_expires = targets.map(|m| m.expires().to_string()),
            targets_count = targets.map(|m| m.targets().len()),
            delegations = database.trusted_delegations().len(),
            "{message}"
        );
    }
}

/// Whether bytes handed out by [`TufRepository`] were checked against its metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Verification {
    /// The mode, or a tolerated failure, meant no check was made.
    Skipped,
    /// Signatures, hashes and lengths all checked out.
    Verified,
}

/// The remote side of the repository, served either from a local directory
/// (for tests and mirrors on disk) or over HTTP.
enum Remote {
    FileSystem(FileSystemRepository<Pouf1>),
    Http(HttpRepository),
}

impl Remote {
    fn from_location(location: &str, options: DownloadOptions) -> anyhow::Result<Self> {
        if utils::is_directory(location) {
            debug!(
                path = location,
                "using filesystem TUF remote from directory path"
            );
            return Ok(Self::FileSystem(FileSystemRepository::new(location)));
        }

        let url = utils::parse_url(location)?;
        Ok(match url.scheme() {
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| anyhow!("invalid TUF file url '{url}'"))?;
                debug!(path = %path.display(), "using filesystem TUF remote from file url");
                Self::FileSystem(FileSystemRepository::new(path))
            }
            "http" | "https" => {
                debug!(%url, "using http TUF remote");
                Self::Http(HttpRepository::new(url, options))
            }
            scheme => bail!("unsupported TUF repository scheme '{scheme}' in '{url}'"),
        })
    }
}

impl RepositoryProvider<Pouf1> for Remote {
    fn fetch_metadata<'a>(
        &'a self,
        meta_path: &MetadataPath,
        version: MetadataVersion,
    ) -> BoxFuture<'a, tuf::Result<Box<dyn AsyncRead + Send + Unpin + 'a>>> {
        match self {
            Self::FileSystem(repo) => repo.fetch_metadata(meta_path, version),
            Self::Http(repo) => repo.fetch_metadata(meta_path, version),
        }
    }

    fn fetch_target<'a>(
        &'a self,
        target_path: &TargetPath,
    ) -> BoxFuture<'a, tuf::Result<Box<dyn AsyncRead + Send + Unpin + 'a>>> {
        match self {
            Self::FileSystem(repo) => repo.fetch_target(target_path),
            Self::Http(repo) => repo.fetch_target(target_path),
        }
    }
}

/// A remote reached through rustup's own downloader, so proxies, TLS backend
/// and timeouts behave exactly as for every other download.
struct HttpRepository {
    base: Url,
    options: DownloadOptions,
}

impl HttpRepository {
    fn new(base: Url, options: DownloadOptions) -> Self {
        trace!(%base, ?options, "created TUF http repository");
        Self { base, options }
    }

    fn url(&self, prefix: &str, components: &[String]) -> tuf::Result<Url> {
        let mut url = self.base.clone();
        let mut segments = url.path_segments_mut().map_err(|_| {
            tuf::Error::IllegalArgument(format!("cannot be a base url: {}", self.base))
        })?;
        segments.pop_if_empty();
        segments.push(prefix);
        segments.extend(components);
        drop(segments);
        trace!(prefix, ?components, %url, "resolved TUF url");
        Ok(url)
    }

    async fn fetch(&self, url: Url) -> anyhow::Result<Vec<u8>> {
        // Removed when dropped, whether or not the download succeeds.
        let file = tempfile::Builder::new()
            .prefix("rustup-tuf")
            .tempfile()
            .context("error creating temp file for TUF download")?;
        let path = file.path();
        debug!(%url, path = %path.display(), "fetching TUF file");
        self.options.start(&url, path, None).download().await?;
        let bytes = fs::read(path)
            .with_context(|| format!("error reading TUF file '{}'", path.display()))?;
        trace!(%url, len = bytes.len(), "fetched TUF file");
        Ok(bytes)
    }
}

impl RepositoryProvider<Pouf1> for HttpRepository {
    fn fetch_metadata<'a>(
        &'a self,
        meta_path: &MetadataPath,
        version: MetadataVersion,
    ) -> BoxFuture<'a, tuf::Result<Box<dyn AsyncRead + Send + Unpin + 'a>>> {
        let meta_path = meta_path.clone();
        async move {
            trace!(%meta_path, %version, "fetching TUF metadata over http");
            let url = self.url(METADATA_PREFIX, &meta_path.components::<Pouf1>(version))?;
            match self.fetch(url).await {
                Ok(bytes) => Ok(Box::new(Cursor::new(bytes)) as Box<dyn AsyncRead + Send + Unpin>),
                Err(err) if is_not_found(&err) => {
                    trace!(%meta_path, %version, "TUF metadata not found");
                    Err(tuf::Error::MetadataNotFound {
                        path: meta_path,
                        version,
                    })
                }
                Err(err) => {
                    debug!(
                        %meta_path,
                        %version,
                        error = format!("{err:#}"),
                        "TUF metadata fetch failed"
                    );
                    Err(tuf::Error::Opaque(format!("{err:#}")))
                }
            }
        }
        .boxed()
    }

    fn fetch_target<'a>(
        &'a self,
        target_path: &TargetPath,
    ) -> BoxFuture<'a, tuf::Result<Box<dyn AsyncRead + Send + Unpin + 'a>>> {
        let target_path = target_path.clone();
        async move {
            let url = self.url(TARGETS_PREFIX, &target_path.components())?;
            trace!(%target_path, %url, "fetching TUF target over http");
            match self.fetch(url).await {
                Ok(bytes) => Ok(Box::new(Cursor::new(bytes)) as Box<dyn AsyncRead + Send + Unpin>),
                Err(err) if is_not_found(&err) => {
                    trace!(%target_path, "TUF target not found");
                    Err(tuf::Error::TargetNotFound(target_path))
                }
                Err(err) => {
                    debug!(
                        %target_path,
                        error = format!("{err:#}"),
                        "TUF target fetch failed"
                    );
                    Err(tuf::Error::Opaque(format!("{err:#}")))
                }
            }
        }
        .boxed()
    }
}

fn is_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<RustupError>(),
        Some(RustupError::DownloadNotExists { .. })
    )
}

#[cfg(test)]
mod tests {
    use std::{env, fs, io, path::Path};

    use tracing::subscriber;
    use tracing_subscriber::{EnvFilter, filter::LevelFilter};
    use tuf::{
        client::{Client, Config},
        metadata::RawSignedMetadata,
        repository::FileSystemRepository,
    };

    use super::{Remote, TufRepository};
    use crate::{download::DownloadOptions, process::Process, tuf::TufMode};

    /// Searches the repository at `RUSTUP_TUF_SERVER`, trusted from the root
    /// at `RUSTUP_TUF_ROOT`, for `1.93.0.toml`, loading delegated roles on
    /// the way. The metadata cache lives in a fresh temp dir. Tracing goes to
    /// stdout, filtered by `RUSTUP_LOG` (default: everything under `rustup::tuf`).
    ///
    /// Run with `cargo test --features test --lib -- tuf::repository::tests::find_targets --ignored --nocapture`.
    #[ignore = "uses the TUF repository named by RUSTUP_TUF_SERVER and RUSTUP_TUF_ROOT"]
    #[tokio::test]
    async fn find_targets() {
        let filter = EnvFilter::builder()
            .with_env_var("RUSTUP_LOG")
            .with_default_directive(LevelFilter::OFF.into())
            .from_env_lossy()
            .add_directive("rustup::tuf=trace".parse().unwrap());
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(io::stdout)
            .finish();
        let _guard = subscriber::set_default(subscriber);

        let server = env::var("RUSTUP_TUF_SERVER").expect("RUSTUP_TUF_SERVER is set");
        let root = env::var("RUSTUP_TUF_ROOT").expect("RUSTUP_TUF_ROOT is set");
        let home = tempfile::Builder::new()
            .prefix("rustup-tuf-find-targets")
            .tempdir()
            .unwrap();

        let options = DownloadOptions::try_from(&Process::os()).unwrap();
        let root = RawSignedMetadata::new(fs::read(root).unwrap());
        let client = Client::with_trusted_root(
            Config::default(),
            &root,
            FileSystemRepository::new(home.path()),
            Remote::from_location(&server, options).unwrap(),
        )
        .await
        .unwrap();
        let mut repository = TufRepository {
            mode: TufMode::On,
            ignore_failures: false,
            ignore_expiry_after: None,
            client,
        };
        repository.verify().await.unwrap();

        let found = repository
            .find_targets(Path::new(""), "1.93.0.toml")
            .await
            .unwrap();
        eprintln!("found: {found:#?}");
        let found = found.expect("no target named 1.93.0.toml found");
        assert_eq!(Path::new(&found).file_name(), Some("1.93.0.toml".as_ref()));
    }
}
