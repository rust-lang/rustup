use std::borrow::Cow;
use std::fs;
use std::ops;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};
use url::Url;

use crate::config::Cfg;
use crate::dist::temp;
use crate::download::{download_file, download_file_with_resume};
use crate::errors::RustupError;
use crate::process::Process;
use crate::utils;

const UPDATE_HASH_LEN: usize = 20;

#[derive(Clone)]
pub struct DownloadCfg {
    pub tmp_cx: Arc<temp::Context>,
    pub download_dir: Arc<PathBuf>,
    pub(super) tracker: Arc<DownloadTracker>,
    pub process: Arc<Process>,
}

impl DownloadCfg {
    /// construct a download configuration
    pub(crate) fn new(cfg: &'_ Cfg<'_>) -> Self {
        DownloadCfg {
            tmp_cx: cfg.tmp_cx.clone(),
            download_dir: Arc::new(cfg.download_dir.clone()),
            tracker: Arc::new(DownloadTracker::new(!cfg.quiet, cfg.process)),
            process: Arc::new(cfg.process.clone()),
        }
    }

    /// Downloads a file and validates its hash. Resumes interrupted downloads.
    /// Partial downloads are stored in `self.download_dir`, keyed by hash. If the
    /// target file already exists, then the hash is checked and it is returned
    /// immediately without re-downloading.
    pub(crate) async fn download(
        &self,
        url: &Url,
        hash: &str,
        status: &DownloadStatus,
    ) -> Result<File> {
        utils::ensure_dir_exists("Download Directory", &self.download_dir)?;
        let target_file = self.download_dir.join(Path::new(hash));

        if target_file.exists() {
            let cached_result = file_hash(&target_file)?;
            if hash == cached_result {
                debug!("reusing previously downloaded file");
                debug!(url = url.as_ref(), "checksum passed");
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

        if let Err(e) = download_file_with_resume(
            url,
            &partial_file_path,
            Some(&mut hasher),
            true,
            Some(status),
            &self.process,
        )
        .await
        {
            let err = Err(e);
            if partial_file_existed {
                return err.context(RustupError::BrokenPartialFile);
            } else {
                return err;
            }
        };

        let actual_hash = format!("{:x}", hasher.finalize());

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
            debug!(url = url.as_ref(), "checksum passed");
            utils::rename(
                "downloaded",
                &partial_file_path,
                &target_file,
                &self.process,
            )?;
            Ok(File { path: target_file })
        }
    }

    pub(crate) fn clean(&self, hashes: &[String]) -> Result<()> {
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
        let hash_file = self.tmp_cx.clone().new_file()?;

        download_file(&hash_url, &hash_file, None, None, &self.process).await?;

        utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())
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
        let file = self.tmp_cx.clone().new_file_with_ext("", ext)?;

        let mut hasher = Sha256::new();
        download_file(&url, &file, Some(&mut hasher), status, &self.process).await?;
        let actual_hash = format!("{:x}", hasher.finalize());

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

    pub(crate) fn status_for(&self, component: impl Into<Cow<'static, str>>) -> DownloadStatus {
        let progress = ProgressBar::hidden();
        progress.set_style(
            ProgressStyle::with_template(
                "{msg:>12.bold}  [{bar:30}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})",
            )
            .unwrap()
            .progress_chars("## "),
        );
        progress.set_message(component);
        self.tracker.multi_progress_bars.add(progress.clone());

        DownloadStatus {
            progress,
            retry_time: Mutex::new(None),
        }
    }
}

/// Tracks download progress and displays information about it to a terminal.
///
/// *not* safe for tracking concurrent downloads yet - it is basically undefined
/// what will happen.
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
            ProgressStyle::with_template(
                "{msg:>12.bold}  [{bar:30}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})",
            )
            .unwrap()
            .progress_chars("## "),
        );
    }

    pub(crate) fn finished(&self) {
        self.progress.set_style(
            ProgressStyle::with_template("{msg:>12.bold}  pending installation {total_bytes:>10}")
                .unwrap(),
        );
        self.progress.tick(); // A tick is needed for the new style to appear, as it is static.
    }

    pub(crate) fn failed(&self) {
        self.progress.set_style(
            ProgressStyle::with_template("{msg:>12.bold}  download failed after {elapsed}")
                .unwrap(),
        );
        self.progress.finish();
    }

    pub(crate) fn retrying(&self) {
        *self.retry_time.lock().unwrap() = Some(Instant::now());
        self.progress.set_style(
            ProgressStyle::with_template("{msg:>12.bold}  retrying download...").unwrap(),
        );
    }

    pub(crate) fn installing(&self) {
        self.progress.set_style(
            ProgressStyle::with_template(
                "{msg:>12.bold}  installing {spinner:.green} {total_bytes:>18}",
            )
            .unwrap()
            .tick_chars(r"|/-\ "),
        );
        self.progress.enable_steady_tick(Duration::from_millis(100));
    }

    pub(crate) fn installed(&self) {
        self.progress.set_style(
            ProgressStyle::with_template("{msg:>12.bold}  installed {total_bytes:>21}").unwrap(),
        );
        self.progress.finish();
    }
}

fn file_hash(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut downloaded = utils::FileReaderWithProgress::new_file(path)?;
    use std::io::Read;
    let mut buf = vec![0; 32768];
    while let Ok(n) = downloaded.read(&mut buf) {
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
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
