use std::{
    borrow::Cow,
    fs,
    io::Read,
    ops,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, anyhow};
use indicatif::{MultiProgress, ProgressBar, ProgressBarIter, ProgressDrawTarget, ProgressStyle};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};
use url::Url;

use crate::{
    config::Cfg,
    dist::{
        Channel, ChannelToolchainName, DEFAULT_DIST_SERVER,
        manifest::{Manifest, ManifestWithHash},
        temp,
    },
    download::{DownloadOptions, is_network_failure},
    errors::RustupError,
    process::Process,
    tuf::TufConfig,
    utils,
};

const UPDATE_HASH_LEN: usize = 20;

pub struct DownloadCfg<'a> {
    pub tmp_cx: Arc<temp::Context>,
    pub download_dir: &'a PathBuf,
    pub(super) tracker: DownloadTracker,
    pub(super) permit_copy_rename: bool,
    pub process: &'a Process,
    pub(crate) tuf: &'a TufConfig,
}

impl<'a> DownloadCfg<'a> {
    /// construct a download configuration
    pub(crate) fn new(cfg: &'a Cfg<'a>) -> Self {
        DownloadCfg {
            tmp_cx: Arc::new(temp::Context::new(
                cfg.rustup_dir.join("tmp"),
                cfg.dist_root_server.as_str(),
            )),
            download_dir: &cfg.download_dir,
            tracker: DownloadTracker::new(!cfg.quiet, cfg.process),
            permit_copy_rename: cfg.process.permit_copy_rename(),
            process: cfg.process,
            tuf: &cfg.tuf,
        }
    }

    /// Downloads a file and validates its hash. Resumes interrupted downloads.
    /// Partial downloads are stored in `self.download_dir`, keyed by hash. If the
    /// target file already exists, then the hash is checked and it is returned
    /// immediately without re-downloading.
    ///
    /// TUF: This is how artifacts arrive. They are not TUF targets: they
    /// come straight from the dist server and are trusted through `hash`, which
    /// the caller took from a manifest that TUF vouched for.
    pub(crate) async fn download(
        &self,
        url: &Url,
        hash: &str,
        status: &DownloadStatus,
    ) -> anyhow::Result<File> {
        utils::ensure_dir_exists("Download Directory", self.download_dir)?;
        let target_file = self.download_dir.join(Path::new(hash));
        debug!(
            url = url.as_ref(),
            via = "direct",
            expected_sha256 = hash,
            "downloading component; verification is the sha256 from the manifest"
        );

        if target_file.exists() {
            let cached_result = file_hash(&target_file)?;
            if hash == cached_result {
                debug!(
                    url = url.as_ref(),
                    via = "cache",
                    sha256 = cached_result,
                    "reusing previously downloaded file, cached sha256 matches manifest"
                );
                return Ok(File { path: target_file });
            } else {
                warn!("bad checksum for cached download");
                fs::remove_file(&target_file).context("cleaning up previous download")?;
            }
        }

        let partial_file_path = target_file.with_file_name(
            target_file
                .file_name()
                .map(|s| s.to_str().unwrap_or("_"))
                .unwrap_or("_")
                .to_owned()
                + ".partial",
        );

        let partial_file_existed = partial_file_path.exists();

        let mut hasher = Sha256::new();
        let mut download = DownloadOptions::try_from(self.process)?
            .start(url, &partial_file_path, None)
            .with_hasher(&mut hasher)
            .with_status(status)
            .with_resume();

        if let Err(e) = download.download().await {
            let is_network_failure = is_network_failure(&e);
            let err = Err(e);
            return match (partial_file_existed, is_network_failure) {
                (true, true) => err.context(RustupError::IncompletePartialFile),
                (true, false) => err.context(RustupError::BrokenPartialFile),
                (false, _) => err,
            };
        };

        let actual_hash = faster_hex::hex_string(&hasher.finalize());

        if hash != actual_hash {
            // Incorrect hash
            if partial_file_existed {
                self.clean(&[hash.to_string() + ".partial"])?;
                Err(anyhow!(RustupError::BrokenPartialFile))
            } else {
                Err(RustupError::ChecksumFailed {
                    url: url.to_string(),
                    expected: hash.to_string(),
                    calculated: actual_hash,
                }
                .into())
            }
        } else {
            debug!(
                url = url.as_ref(),
                via = "direct",
                sha256 = actual_hash,
                "checksum passed against manifest sha256"
            );
            utils::rename(
                "downloaded",
                &partial_file_path,
                &target_file,
                self.permit_copy_rename,
            )?;
            Ok(File { path: target_file })
        }
    }

    pub(crate) fn clean(&self, hashes: &[impl AsRef<Path>]) -> anyhow::Result<()> {
        for hash in hashes.iter() {
            let used_file = self.download_dir.join(hash);
            if self.download_dir.join(&used_file).exists() {
                fs::remove_file(used_file).context("cleaning up cached downloads")?;
            }
        }
        Ok(())
    }

    pub(crate) async fn dl_v2_manifest(
        &self,
        update_hash: Option<&Path>,
        toolchain: &ChannelToolchainName,
        cfg: &Cfg<'_>,
    ) -> anyhow::Result<Option<ManifestWithHash>> {
        // TUF
        let manifest_url = if self.tuf.enabled() {
            toolchain.manifest_v3_url(cfg).await?
        } else {
            toolchain.manifest_v2_url(&cfg.dist_root_url, self.process)
        };

        match self
            .download_and_check(&manifest_url, update_hash, None, ".toml", Some(self.tuf))
            .await
        {
            Ok(manifest_dl) => {
                // Downloaded ok!
                let Some((manifest_file, hash)) = manifest_dl else {
                    return Ok(None);
                };
                let manifest_str = utils::read_file("manifest", &manifest_file)?;
                let manifest =
                    Manifest::parse(&manifest_str).with_context(|| RustupError::ParsingFile {
                        name: "manifest",
                        path: manifest_file.to_path_buf(),
                    })?;

                Ok(Some(ManifestWithHash { manifest, hash }))
            }
            Err(err) => {
                if let Some(RustupError::ChecksumFailed { .. }) = err.downcast_ref::<RustupError>()
                {
                    // Manifest checksum mismatched.
                    warn!("{err:#}");

                    if cfg.dist_root_url.starts_with(DEFAULT_DIST_SERVER) {
                        info!(
                            "this is likely due to an ongoing update of the official release server, please try again later"
                        );
                        info!(
                            "see <https://github.com/rust-lang/rustup/issues/3390> for more details"
                        );
                    } else {
                        info!(
                            "this might indicate an issue with the third-party release server '{}'",
                            cfg.dist_root_url
                        );
                        info!(
                            "see <https://github.com/rust-lang/rustup/issues/3885> for more details"
                        );
                    }
                }
                Err(err)
            }
        }
    }

    pub(super) async fn dl_v1_manifest(
        &self,
        dist_root: &str,
        toolchain: &ChannelToolchainName,
    ) -> anyhow::Result<Vec<String>> {
        let root_url = toolchain.package_dir(dist_root);

        if let Channel::Version(ver) = &toolchain.channel {
            // This is an explicit version. In v1 there was no manifest,
            // you just know the file to download, so synthesize one.
            let installer_name = format!("{}/rust-{}-{}.tar.gz", root_url, ver, toolchain.target);
            return Ok(vec![installer_name]);
        }

        let manifest_url = toolchain.manifest_v1_url(dist_root, self.process);
        let manifest_dl = self
            .download_and_check(&manifest_url, None, None, "", None)
            .await?;
        let (manifest_file, _) = manifest_dl.unwrap();
        let manifest_str = utils::read_file("manifest", &manifest_file)?;
        let urls = manifest_str
            .lines()
            .map(|s| format!("{root_url}/{s}"))
            .collect();

        Ok(urls)
    }

    /// Downloads `url_str` into a fresh temp file with the extension `ext` and
    /// returns it together with the truncated sha256 of its contents.
    ///
    /// If `update_hash` names a file holding that same truncated hash, nothing
    /// has changed since the last update: `None` is returned instead.
    ///
    /// optional `tuf` argument defines whether TUF is used for a given download -
    /// on `None` we attempt to use the legacy sha256 sidecars. With `tuf`, we skip
    /// the hash verification and download and utilize TUF entirely for verification
    pub(crate) async fn download_and_check(
        &self,
        url_str: &str,
        update_hash: Option<&Path>,
        status: Option<&DownloadStatus>,
        ext: &str,
        tuf: Option<&TufConfig>,
    ) -> anyhow::Result<Option<(temp::File, String)>> {
        if let Some(tuf) = tuf
            && tuf.enabled()
        {
            debug!(
                url = url_str,
                via = "tuf",
                mode = %tuf.mode,
                "downloading through TUF; no .sha256 sidecar check, verification is the TUF metadata"
            );
            let (file, hash) = self
                .download_hashed(url_str, status, ext, Some(tuf))
                .await?;
            let partial_hash: String = hash.chars().take(UPDATE_HASH_LEN).collect();
            debug!(
                url = url_str,
                via = "tuf",
                sha256 = hash,
                "hash taken from TUF-verified bytes"
            );
            if self.update_hash_matches(update_hash, &partial_hash) {
                return Ok(None);
            }
            return Ok(Some((file, partial_hash)));
        }

        debug!(
            url = url_str,
            via = "direct",
            tuf = tuf.map(|tuf| tuf.mode.as_str()).unwrap_or("not a target"),
            "downloading directly; verification is the .sha256 sidecar"
        );

        let hash = self.download_hash(url_str).await?;
        let partial_hash: String = hash.chars().take(UPDATE_HASH_LEN).collect();
        debug!(
            url = url_str,
            expected_sha256 = hash,
            "fetched .sha256 sidecar"
        );
        if self.update_hash_matches(update_hash, &partial_hash) {
            return Ok(None);
        }

        let (file, actual_hash) = self.download_hashed(url_str, status, ext, None).await?;
        if hash != actual_hash {
            // Incorrect hash
            debug!(
                url = url_str,
                expected_sha256 = hash,
                actual_sha256 = actual_hash,
                "checksum failed against .sha256 sidecar"
            );
            return Err(RustupError::ChecksumFailed {
                url: url_str.to_owned(),
                expected: hash,
                calculated: actual_hash,
            }
            .into());
        }
        debug!(
            url = url_str,
            via = "direct",
            sha256 = actual_hash,
            "checksum passed against .sha256 sidecar"
        );

        Ok(Some((file, partial_hash)))
    }

    /// Downloads `url_str` into a fresh temp file, returning it with the full
    /// sha256 of its contents. Goes through TUF when `tuf` is given and enabled.
    async fn download_hashed(
        &self,
        url_str: &str,
        status: Option<&DownloadStatus>,
        ext: &str,
        tuf: Option<&TufConfig>,
    ) -> anyhow::Result<(temp::File, String)> {
        let url = utils::parse_url(url_str)?;
        let file = self.tmp_cx.new_file_with_ext("", ext)?;
        let via = match tuf {
            Some(tuf) if tuf.enabled() => "tuf",
            _ => "direct",
        };
        debug!(url = url_str, via, path = %file.display(), "downloading to temp file");

        let mut hasher = Sha256::new();
        let download = DownloadOptions::try_from(self.process)?
            .start(&url, &file, tuf)
            .with_hasher(&mut hasher);

        let mut download = match status {
            Some(status) => download.with_status(status),
            None => download,
        };

        download.download().await?;
        let sha256 = faster_hex::hex_string(&hasher.finalize());
        debug!(url = url_str, via, sha256, "download finished");
        Ok((file, sha256))
    }

    /// Fetches the `.sha256` sidecar for `url` and returns the hash it holds.
    /// Only used with TUF disabled; the sidecars are not TUF targets.
    async fn download_hash(&self, url: &str) -> anyhow::Result<String> {
        let hash_url = utils::parse_url(&(url.to_owned() + ".sha256"))?;
        let hash_file = self.tmp_cx.new_file()?;
        debug!(url = %hash_url, via = "direct", "fetching .sha256 sidecar");
        DownloadOptions::try_from(self.process)?
            .start(&hash_url, &hash_file, None)
            .download()
            .await?;
        utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())
    }

    /// Whether `update_hash` names a file already holding `partial_hash`,
    /// meaning nothing has changed since the last update.
    fn update_hash_matches(&self, update_hash: Option<&Path>, partial_hash: &str) -> bool {
        let Some(hash_file) = update_hash else {
            return false;
        };
        if !utils::is_file(hash_file) {
            debug!(file = %hash_file.display(), "no update hash file found");
            return false;
        }
        match utils::read_file("update hash", hash_file) {
            Ok(contents) if contents == partial_hash => {
                debug!(file = %hash_file.display(), "update hash matches, nothing to update");
                true
            }
            Ok(_) => false,
            Err(_) => {
                warn!(
                    "can't read update hash {}, can't skip update",
                    hash_file.display()
                );
                false
            }
        }
    }

    pub(crate) fn status_for(
        &self,
        component_name: impl Into<Cow<'static, str>>,
        name_width: usize,
    ) -> DownloadStatus {
        let progress = ProgressBar::hidden();
        progress.set_style(
            DownloadStatus::progress_style(
                name_width,
                "downloading [{bar:15}] {total_bytes:>11} ({bytes_per_sec}, ETA: {eta})",
            )
            .progress_chars("## "),
        );
        progress.set_prefix(component_name);
        self.tracker.multi_progress_bars.add(progress.clone());

        DownloadStatus {
            progress,
            retry_time: Mutex::new(None),
            name_width,
        }
    }

    pub(crate) fn url(&self, url: &str) -> anyhow::Result<Url> {
        match &*self.tmp_cx.dist_server {
            server if server != DEFAULT_DIST_SERVER => utils::parse_url(
                &url.replace(DEFAULT_DIST_SERVER, self.tmp_cx.dist_server.as_str()),
            ),
            _ => utils::parse_url(url),
        }
    }
}

/// Tracks download progress and displays information about it to a terminal.
pub(crate) struct DownloadTracker {
    /// MultiProgress bar for the downloads.
    multi_progress_bars: MultiProgress,
}

impl DownloadTracker {
    /// Creates a new DownloadTracker.
    pub(crate) fn new(display_progress: bool, process: &Process) -> Self {
        let multi_progress_bars = MultiProgress::with_draw_target(if display_progress {
            process.progress_draw_target()
        } else {
            ProgressDrawTarget::hidden()
        });
        // Help avoid flickering by moving the cursor instead of clearing the line.
        multi_progress_bars.set_move_cursor(true);
        Self {
            multi_progress_bars,
        }
    }
}

pub(crate) struct DownloadStatus {
    progress: ProgressBar,
    /// The instant where the download is being retried.
    ///
    /// Allows us to delay the reappearance of the progress bar so that the user can see
    /// the message "retrying download" for at least a second. Without it, the progress
    /// bar would reappear immediately, not allowing the user to correctly see the message,
    /// before the progress bar starts again.
    retry_time: Mutex<Option<Instant>>,
    /// The dynamic maximum width of the component names for alignment
    name_width: usize,
}

impl DownloadStatus {
    pub(crate) fn received_length(&self, len: u64) {
        self.progress.reset();
        self.progress.set_length(len);
    }

    pub(crate) fn received_data(&self, len: usize) {
        self.progress.inc(len as u64);
        let mut retry_time = self.retry_time.lock().unwrap();
        if !retry_time.is_some_and(|instant| instant.elapsed() > Duration::from_secs(1)) {
            return;
        }

        *retry_time = None;
        self.progress.set_style(
            Self::progress_style(
                self.name_width,
                "downloading [{bar:15}] {total_bytes:>11} ({bytes_per_sec}, ETA: {eta})",
            )
            .progress_chars("## "),
        );
    }

    pub(crate) fn finished(&self) {
        self.progress.set_style(Self::progress_style(
            self.name_width,
            "pending installation {total_bytes:>20}",
        ));
        self.progress.tick(); // A tick is needed for the new style to appear, as it is static.
    }

    pub(crate) fn failed(&self) {
        self.progress.set_style(Self::progress_style(
            self.name_width,
            "download failed after {elapsed}",
        ));
        self.progress.finish();
    }

    pub(crate) fn retrying(&self) {
        *self.retry_time.lock().unwrap() = Some(Instant::now());
        self.progress.set_style(Self::progress_style(
            self.name_width,
            "retrying download...",
        ));
    }

    pub(crate) fn unpack<T: Read>(&self, inner: T) -> ProgressBarIter<T> {
        self.progress.reset();
        self.progress.set_style(
            Self::progress_style(
                self.name_width,
                "unpacking   [{bar:15}] {total_bytes:>11} ({bytes_per_sec}, ETA: {eta})",
            )
            .progress_chars("## "),
        );
        self.progress.wrap_read(inner)
    }

    pub(crate) fn installing(&self) {
        self.progress.set_style(
            Self::progress_style(
                self.name_width,
                "installing {spinner:.green} {total_bytes:>28}",
            )
            .tick_chars(r"|/-\ "),
        );
        self.progress.enable_steady_tick(Duration::from_millis(100));
    }

    pub(crate) fn installed(&self) {
        self.progress.set_message("installed");
        self.progress.set_style(Self::progress_style(
            self.name_width,
            "{msg:.green.bold} {total_bytes:>31}",
        ));
        self.progress.finish();
    }

    fn progress_style(name_width: usize, suffix: &str) -> ProgressStyle {
        let template = format!("{{prefix:>{name_width}.bold}} {suffix}");
        ProgressStyle::with_template(&template).unwrap()
    }
}

fn file_hash(path: &Path) -> anyhow::Result<String> {
    let mut hasher = Sha256::new();
    let mut downloaded = utils::buffered(path)?;
    let mut buf = vec![0; 32768];
    while let Ok(n) = downloaded.read(&mut buf) {
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(faster_hex::hex_string(&hasher.finalize()))
}

pub(crate) struct File {
    path: PathBuf,
}

impl ops::Deref for File {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.path.as_path()
    }
}
