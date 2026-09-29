use std::fs;

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
    database::Database,
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

        info!("syncing TUF database from {}", location);

        if config.mode == TufMode::Warn {
            warn!("TUF is set to 'warn' mode, and validation failures will be ignored");
        }

        if config.ignore_failures {
            warn!(
                "RUST_TUF_IGNORE is set to true, and validation failures will be silently ignored"
            );
        }

        if let Some(date) = config.ignore_expiry_after.as_ref() {
            warn!(
                "RUST_TUF_IGNOREDATE is set to true and validation failures will be ignored after {}",
                date
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
        match self.read_verified(&path).await {
            Ok(bytes) => {
                debug!(target, len = bytes.len(), "TUF target verified");
                Ok((bytes, Verification::Verified))
            }
            Err(err) => {
                debug!(target, error = %err, "TUF target verification failed");
                let verification = self.tolerate(err.into())?;
                debug!(
                    target,
                    "reading TUF target unverified after tolerated failure"
                );
                Ok((self.read_unverified(&path).await?, verification))
            }
        }
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
        let mut candidates = Vec::new();
        let database = self.client.database();
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
            match self.client.remote_repo().fetch_target(candidate).await {
                Ok(mut reader) => {
                    let mut bytes = Vec::new();
                    reader.read_to_end(&mut bytes).await?;
                    trace!(target = %path, %candidate, len = bytes.len(), "read unverified TUF target");
                    return Ok(bytes);
                }
                Err(err) => {
                    trace!(target = %path, %candidate, error = %err, "unverified TUF target candidate failed");
                    last_err = err;
                }
            }
        }
        Err(last_err)
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
        match reason {
            Some(reason) => {
                debug!(reason, "tolerating TUF verification failure");
                warn!("TUF verification failed: {err:#}");
                Ok(Verification::Skipped)
            }
            None => {
                debug!(mode = %self.mode, "TUF verification failure is fatal");
                Err(err.context("TUF verification failed"))
            }
        }
    }

    fn trace_database(&self, message: &str) {
        let database: &Database<Pouf1> = self.client.database();
        let root = database.trusted_root();
        trace!(
            root_version = root.version(),
            root_expires = %root.expires(),
            consistent_snapshot = root.consistent_snapshot(),
            timestamp_version = database.trusted_timestamp().map(|m| m.version()),
            timestamp_expires = database.trusted_timestamp().map(|m| m.expires().to_string()),
            snapshot_version = database.trusted_snapshot().map(|m| m.version()),
            snapshot_expires = database.trusted_snapshot().map(|m| m.expires().to_string()),
            targets_version = database.trusted_targets().map(|m| m.version()),
            targets_expires = database.trusted_targets().map(|m| m.expires().to_string()),
            targets_count = database.trusted_targets().map(|m| m.targets().len()),
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
        {
            let mut segments = url.path_segments_mut().map_err(|_| {
                tuf::Error::IllegalArgument(format!("cannot be a base url: {}", self.base))
            })?;
            segments.pop_if_empty();
            segments.push(prefix);
            segments.extend(components);
        }
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
                    debug!(%meta_path, %version, error = format!("{err:#}"), "TUF metadata fetch failed");
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
                    debug!(%target_path, error = format!("{err:#}"), "TUF target fetch failed");
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
