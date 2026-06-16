use std::borrow::Cow;
use std::fs;
use std::io::{self, Read};
use std::ops;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use indicatif::{MultiProgress, ProgressBar, ProgressBarIter, ProgressDrawTarget, ProgressStyle};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};
use url::Url;

use crate::config::Cfg;
use crate::dist::manifest::{Manifest, ManifestWithHash};
use crate::dist::{Channel, DEFAULT_DIST_SERVER, ToolchainDesc, temp};
use crate::download::{DownloadOptions, is_network_failure};
use crate::errors::RustupError;
use crate::process::Process;
use crate::utils;

const UPDATE_HASH_LEN: usize = 20;

pub struct DownloadCfg<'a> {
    pub tmp_cx: Arc<temp::Context>,
    pub download_dir: &'a PathBuf,
    pub(super) tracker: DownloadTracker,
    pub(super) permit_copy_rename: bool,
    pub process: &'a Process,
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
        }
    }

    /// Downloads a file and validates its hash. Resumes interrupted downloads.
    /// Partial downloads are stored in `self.download_dir` under unique names.
    /// If the target file already exists, then the hash is checked and it is
    /// returned immediately without re-downloading.
    pub(crate) async fn download(
        &self,
        url: &Url,
        hash: &str,
        status: &DownloadStatus,
    ) -> Result<File> {
        utils::ensure_dir_exists("Download Directory", self.download_dir)?;
        let target_file = self.download_dir.join(Path::new(hash));

        if let Some(file) = self.cached_file(&target_file, hash)? {
            debug!(url = url.as_ref(), "checksum passed");
            return Ok(file);
        }

        let partial = Self::partial_download(&target_file)?;

        let mut hasher = Sha256::new();
        let mut download = DownloadOptions::try_from(self.process)?
            .start(url, &partial.path)
            .with_hasher(&mut hasher)
            .with_status(status)
            .with_resume();

        if let Err(e) = download.download().await {
            let is_network_failure = is_network_failure(&e);
            if is_network_failure {
                Self::keep_partial_for_resume(&partial);
            } else {
                utils::ensure_file_removed("partial download", &partial.path)?;
            }
            let err = Err(e);
            return match (partial.existed, is_network_failure) {
                (true, true) => err.context(RustupError::IncompletePartialFile),
                (true, false) => err.context(RustupError::BrokenPartialFile),
                (false, _) => err,
            };
        };

        let actual_hash = faster_hex::hex_string(&hasher.finalize());

        if hash != actual_hash {
            // Incorrect hash
            utils::ensure_file_removed("partial download", &partial.path)?;
            if partial.existed {
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
            debug!(url = url.as_ref(), "checksum passed");
            self.finish_download(&partial.path, &target_file, hash)
        }
    }

    fn cached_file(&self, target_file: &Path, hash: &str) -> Result<Option<File>> {
        if target_file.exists() {
            let cached_result = file_hash(target_file)?;
            if hash == cached_result {
                debug!("reusing previously downloaded file");
                return Ok(Some(File {
                    path: target_file.to_path_buf(),
                }));
            } else {
                warn!("bad checksum for cached download");
                fs::remove_file(target_file).context("cleaning up previous download")?;
            }
        }

        Ok(None)
    }

    fn partial_download(target_file: &Path) -> Result<PartialDownload> {
        let legacy_path = Self::legacy_partial_path(target_file);
        let path = Self::unique_partial_path(target_file);

        let existed = match fs::rename(&legacy_path, &path) {
            Ok(()) => true,
            Err(e) if e.kind() == io::ErrorKind::NotFound => false,
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "claiming partial download '{}' for '{}'",
                        legacy_path.display(),
                        path.display()
                    )
                });
            }
        };

        Ok(PartialDownload {
            path,
            legacy_path,
            existed,
        })
    }

    fn legacy_partial_path(target_file: &Path) -> PathBuf {
        target_file.with_file_name(
            target_file
                .file_name()
                .map(|s| s.to_str().unwrap_or("_"))
                .unwrap_or("_")
                .to_owned()
                + ".partial",
        )
    }

    fn unique_partial_path(target_file: &Path) -> PathBuf {
        let file_name = target_file
            .file_name()
            .map(|s| s.to_str().unwrap_or("_"))
            .unwrap_or("_");
        target_file.with_file_name(format!(
            "{file_name}.{}.partial",
            utils::raw::random_string(16)
        ))
    }

    fn keep_partial_for_resume(partial: &PartialDownload) {
        if !utils::path_exists(&partial.path) {
            return;
        }

        if utils::path_exists(&partial.legacy_path) {
            if let Err(e) = utils::ensure_file_removed("partial download", &partial.path) {
                warn!(
                    "could not remove duplicate partial download {} ({e})",
                    partial.path.display()
                );
            }
            return;
        }

        if let Err(e) = fs::rename(&partial.path, &partial.legacy_path) {
            warn!(
                "could not keep partial download {} for resumption at {} ({e})",
                partial.path.display(),
                partial.legacy_path.display()
            );
        }
    }

    fn finish_download(
        &self,
        partial_file_path: &Path,
        target_file: &Path,
        hash: &str,
    ) -> Result<File> {
        if let Some(file) = self.cached_file(target_file, hash)? {
            utils::ensure_file_removed("partial download", partial_file_path)?;
            return Ok(file);
        }

        match utils::rename(
            "downloaded",
            partial_file_path,
            target_file,
            self.permit_copy_rename,
        ) {
            Ok(()) => Ok(File {
                path: target_file.to_path_buf(),
            }),
            Err(e) => match self.cached_file(target_file, hash)? {
                Some(file) => {
                    utils::ensure_file_removed("partial download", partial_file_path)?;
                    Ok(file)
                }
                None => Err(e),
            },
        }
    }

    pub(crate) fn clean(&self, hashes: &[impl AsRef<Path>]) -> Result<()> {
        for hash in hashes.iter() {
            let used_file = self.download_dir.join(hash);
            if self.download_dir.join(&used_file).exists() {
                fs::remove_file(used_file).context("cleaning up cached downloads")?;
            }
        }
        Ok(())
    }

    async fn download_hash(&self, url: &str) -> Result<String> {
        let hash_url = utils::parse_url(&(url.to_owned() + ".sha256"))?;
        let hash_file = self.tmp_cx.new_file()?;
        DownloadOptions::try_from(self.process)?
            .start(&hash_url, &hash_file)
            .download()
            .await?;
        utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())
    }

    pub(crate) async fn dl_v2_manifest(
        &self,
        update_hash: Option<&Path>,
        toolchain: &ToolchainDesc,
        cfg: &Cfg<'_>,
    ) -> Result<Option<ManifestWithHash>> {
        let manifest_url = toolchain.manifest_v2_url(&cfg.dist_root_url, self.process);
        match self
            .download_and_check(&manifest_url, update_hash, None, ".toml")
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
            Err(any) => {
                if let Some(err @ RustupError::ChecksumFailed { .. }) =
                    any.downcast_ref::<RustupError>()
                {
                    // Manifest checksum mismatched.
                    warn!("{err}");

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
                Err(any)
            }
        }
    }

    pub(super) async fn dl_v1_manifest(
        &self,
        dist_root: &str,
        toolchain: &ToolchainDesc,
    ) -> Result<Vec<String>> {
        let root_url = toolchain.package_dir(dist_root);

        if let Channel::Version(ver) = &toolchain.channel {
            // This is an explicit version. In v1 there was no manifest,
            // you just know the file to download, so synthesize one.
            let installer_name = format!("{}/rust-{}-{}.tar.gz", root_url, ver, toolchain.target);
            return Ok(vec![installer_name]);
        }

        let manifest_url = toolchain.manifest_v1_url(dist_root, self.process);
        let manifest_dl = self
            .download_and_check(&manifest_url, None, None, "")
            .await?;
        let (manifest_file, _) = manifest_dl.unwrap();
        let manifest_str = utils::read_file("manifest", &manifest_file)?;
        let urls = manifest_str
            .lines()
            .map(|s| format!("{root_url}/{s}"))
            .collect();

        Ok(urls)
    }

    /// Downloads a file, sourcing its hash from the same url with a `.sha256` suffix.
    /// If `update_hash` is present, then that will be compared to the downloaded hash,
    /// and if they match, the download is skipped.
    /// Verifies the signature found at the same url with a `.asc` suffix, and prints a
    /// warning when the signature does not verify, or is not found.
    pub(crate) async fn download_and_check(
        &self,
        url_str: &str,
        update_hash: Option<&Path>,
        status: Option<&DownloadStatus>,
        ext: &str,
    ) -> Result<Option<(temp::File, String)>> {
        let hash = self.download_hash(url_str).await?;
        let partial_hash: String = hash.chars().take(UPDATE_HASH_LEN).collect();

        if let Some(hash_file) = update_hash {
            if utils::is_file(hash_file) {
                if let Ok(contents) = utils::read_file("update hash", hash_file) {
                    if contents == partial_hash {
                        // Skip download, update hash matches
                        return Ok(None);
                    }
                } else {
                    warn!(
                        "can't read update hash {}, can't skip update",
                        hash_file.display()
                    );
                }
            } else {
                debug!(file = %hash_file.display(), "no update hash file found");
            }
        }

        let url = utils::parse_url(url_str)?;
        let file = self.tmp_cx.new_file_with_ext("", ext)?;

        let mut hasher = Sha256::new();
        let download = DownloadOptions::try_from(self.process)?
            .start(&url, &file)
            .with_hasher(&mut hasher);

        let mut download = match status {
            Some(status) => download.with_status(status),
            None => download,
        };

        download.download().await?;
        let actual_hash = faster_hex::hex_string(&hasher.finalize());

        if hash != actual_hash {
            // Incorrect hash
            return Err(RustupError::ChecksumFailed {
                url: url_str.to_owned(),
                expected: hash,
                calculated: actual_hash,
            }
            .into());
        } else {
            debug!(url = url_str, "checksum passed");
        }

        Ok(Some((file, partial_hash)))
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
        progress.set_message(component_name);
        self.tracker.multi_progress_bars.add(progress.clone());

        DownloadStatus {
            progress,
            retry_time: Mutex::new(None),
            name_width,
        }
    }

    pub(crate) fn url(&self, url: &str) -> Result<Url> {
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
            DownloadStatus::progress_style(
                self.name_width,
                "downloading [{bar:15}] {total_bytes:>11} ({bytes_per_sec}, ETA: {eta})",
            )
            .progress_chars("## "),
        );
    }

    pub(crate) fn finished(&self) {
        self.progress.set_style(DownloadStatus::progress_style(
            self.name_width,
            "pending installation {total_bytes:>20}",
        ));
        self.progress.tick(); // A tick is needed for the new style to appear, as it is static.
    }

    pub(crate) fn failed(&self) {
        self.progress.set_style(DownloadStatus::progress_style(
            self.name_width,
            "download failed after {elapsed}",
        ));
        self.progress.finish();
    }

    pub(crate) fn retrying(&self) {
        *self.retry_time.lock().unwrap() = Some(Instant::now());
        self.progress.set_style(DownloadStatus::progress_style(
            self.name_width,
            "retrying download...",
        ));
    }

    pub(crate) fn unpack<T: Read>(&self, inner: T) -> ProgressBarIter<T> {
        self.progress.reset();
        self.progress.set_style(
            DownloadStatus::progress_style(
                self.name_width,
                "unpacking   [{bar:15}] {total_bytes:>11} ({bytes_per_sec}, ETA: {eta})",
            )
            .progress_chars("## "),
        );
        self.progress.wrap_read(inner)
    }

    pub(crate) fn installing(&self) {
        self.progress.set_style(
            DownloadStatus::progress_style(
                self.name_width,
                "installing {spinner:.green} {total_bytes:>28}",
            )
            .tick_chars(r"|/-\ "),
        );
        self.progress.enable_steady_tick(Duration::from_millis(100));
    }

    pub(crate) fn installed(&self) {
        self.progress.set_style(DownloadStatus::progress_style(
            self.name_width,
            "installed {total_bytes:>31}",
        ));
        self.progress.finish();
    }

    fn progress_style(name_width: usize, suffix: &str) -> ProgressStyle {
        let template = format!("{{msg:>{name_width}.bold}} {suffix}");
        ProgressStyle::with_template(&template).unwrap()
    }
}

fn file_hash(path: &Path) -> Result<String> {
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

struct PartialDownload {
    path: PathBuf,
    legacy_path: PathBuf,
    existed: bool,
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::process::TestProcess;

    #[test]
    fn partial_download_claims_legacy_partial_for_resume() {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let target_file = tempdir.path().join("abc123");
        let legacy_partial = DownloadCfg::legacy_partial_path(&target_file);
        fs::write(&legacy_partial, b"partial contents").unwrap();

        let partial = DownloadCfg::partial_download(&target_file).unwrap();

        assert!(partial.existed);
        assert_ne!(partial.path, legacy_partial);
        assert!(!legacy_partial.exists());
        assert_eq!(fs::read(&partial.path).unwrap(), b"partial contents");
        assert!(
            partial
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("abc123.")
        );
        assert!(
            partial
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".partial")
        );
    }

    #[test]
    fn finish_download_reuses_valid_cache_from_race() {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let download_dir = tempdir.path().join("downloads");
        utils::ensure_dir_exists("download dir", &download_dir).unwrap();

        let content = b"cached component contents";
        let hash = faster_hex::hex_string(&Sha256::digest(content));
        let target_file = download_dir.join(&hash);
        let partial_file = download_dir.join(format!("{hash}.other-process.partial"));
        fs::write(&target_file, content).unwrap();
        fs::write(&partial_file, content).unwrap();

        let tp = TestProcess::default();
        let tmp_cx = Arc::new(temp::Context::new(
            tempdir.path().join("tmp"),
            DEFAULT_DIST_SERVER,
        ));
        let cfg = DownloadCfg {
            tmp_cx,
            download_dir: &download_dir,
            tracker: DownloadTracker::new(false, &tp.process),
            permit_copy_rename: tp.process.permit_copy_rename(),
            process: &tp.process,
        };

        let file = cfg
            .finish_download(&partial_file, &target_file, &hash)
            .unwrap();

        assert_eq!(&*file, target_file.as_path());
        assert!(!partial_file.exists());
        assert_eq!(fs::read(&target_file).unwrap(), content);
    }
}
