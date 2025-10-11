use std::collections::HashMap;
use std::fs;
use std::ops;
use std::path::{Path, PathBuf};
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
use crate::errors::*;
use crate::notifications::Notification;
use crate::process::Process;
use crate::utils;

const UPDATE_HASH_LEN: usize = 20;

#[derive(Clone)]
pub struct DownloadCfg<'a> {
    pub dist_root: &'a str,
    pub tmp_cx: &'a temp::Context,
    pub download_dir: &'a PathBuf,
    pub(crate) notify_handler: &'a dyn Fn(Notification<'_>),
    pub process: &'a Process,
}

impl<'a> DownloadCfg<'a> {
    /// construct a download configuration
    pub(crate) fn new(cfg: &'a Cfg<'a>) -> Self {
        DownloadCfg {
            dist_root: &cfg.dist_root_url,
            tmp_cx: &cfg.tmp_cx,
            download_dir: &cfg.download_dir,
            notify_handler: &*cfg.notify_handler,
            process: cfg.process,
        }
    }

    /// Downloads a file and validates its hash. Resumes interrupted downloads.
    /// Partial downloads are stored in `self.download_dir`, keyed by hash. If the
    /// target file already exists, then the hash is checked and it is returned
    /// immediately without re-downloading.
    pub(crate) async fn download(&self, url: &Url, hash: &str) -> Result<File> {
        utils::ensure_dir_exists("Download Directory", self.download_dir)?;
        let target_file = self.download_dir.join(Path::new(hash));

        if target_file.exists() {
            let cached_result = file_hash(&target_file, self.notify_handler)?;
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
            &|n| (self.notify_handler)(n),
            self.process,
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
            utils::rename("downloaded", &partial_file_path, &target_file, self.process)?;
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
        let hash_file = self.tmp_cx.new_file()?;

        download_file(
            &hash_url,
            &hash_file,
            None,
            &|n| (self.notify_handler)(n),
            self.process,
        )
        .await?;

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
        download_file(
            &url,
            &file,
            Some(&mut hasher),
            &|n| (self.notify_handler)(n),
            self.process,
        )
        .await?;
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
}

pub(crate) struct Notifier {
    tracker: Mutex<DownloadTracker>,
}

impl Notifier {
    pub(crate) fn new(quiet: bool, process: &Process) -> Self {
        Self {
            tracker: Mutex::new(DownloadTracker::new(!quiet, process)),
        }
    }

    pub(crate) fn handle(&self, n: Notification<'_>) {
        self.tracker.lock().unwrap().handle_notification(&n);
    }
}

/// Tracks download progress and displays information about it to a terminal.
///
/// *not* safe for tracking concurrent downloads yet - it is basically undefined
/// what will happen.
pub(crate) struct DownloadTracker {
    /// MultiProgress bar for the downloads.
    multi_progress_bars: MultiProgress,
    /// Mapping of URLs being downloaded to their corresponding progress bars.
    /// The `Option<Instant>` represents the instant where the download is being retried,
    /// allowing us delay the reappearance of the progress bar so that the user can see
    /// the message "retrying download" for at least a second.
    /// Without it, the progress bar would reappear immediately, not allowing the user to
    /// correctly see the message, before the progress bar starts again.
    file_progress_bars: HashMap<String, (ProgressBar, Option<Instant>)>,
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
            file_progress_bars: HashMap::new(),
        }
    }

    pub(crate) fn handle_notification(&mut self, n: &Notification<'_>) {
        match *n {
            Notification::DownloadContentLengthReceived(content_len, url) => {
                if let Some(url) = url {
                    self.content_length_received(content_len, url);
                }
            }
            Notification::DownloadDataReceived(data, url) => {
                if let Some(url) = url {
                    self.data_received(data.len(), url);
                }
            }
            Notification::DownloadFinished(url) => {
                if let Some(url) = url {
                    self.download_finished(url);
                }
            }
            Notification::DownloadFailed(url) => {
                self.download_failed(url);
                debug!("download failed");
            }
            Notification::DownloadingComponent(component, _, _, url) => {
                self.create_progress_bar(component.to_owned(), url.to_owned());
            }
            Notification::RetryingDownload(url) => {
                self.retrying_download(url);
            }
        }
    }

    /// Creates a new ProgressBar for the given component.
    pub(crate) fn create_progress_bar(&mut self, component: String, url: String) {
        let pb = ProgressBar::hidden();
        pb.set_style(
            ProgressStyle::with_template(
                "{msg:>12.bold}  [{bar:40}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})",
            )
            .unwrap()
            .progress_chars("## "),
        );
        pb.set_message(component);
        self.multi_progress_bars.add(pb.clone());
        self.file_progress_bars.insert(url, (pb, None));
    }

    /// Sets the length for a new ProgressBar and gives it a style.
    pub(crate) fn content_length_received(&mut self, content_len: u64, url: &str) {
        if let Some((pb, _)) = self.file_progress_bars.get(url) {
            pb.reset();
            pb.set_length(content_len);
        }
    }

    /// Notifies self that data of size `len` has been received.
    pub(crate) fn data_received(&mut self, len: usize, url: &str) {
        let Some((pb, retry_time)) = self.file_progress_bars.get_mut(url) else {
            return;
        };
        pb.inc(len as u64);
        if !retry_time.is_some_and(|instant| instant.elapsed() > Duration::from_secs(1)) {
            return;
        }
        *retry_time = None;
        pb.set_style(
            ProgressStyle::with_template(
                "{msg:>12.bold}  [{bar:40}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})",
            )
            .unwrap()
            .progress_chars("## "),
        );
    }

    /// Notifies self that the download has finished.
    pub(crate) fn download_finished(&mut self, url: &str) {
        let Some((pb, _)) = self.file_progress_bars.get(url) else {
            return;
        };
        pb.set_style(
            ProgressStyle::with_template("{msg:>12.bold}  downloaded {total_bytes} in {elapsed}")
                .unwrap(),
        );
        pb.finish();
    }

    /// Notifies self that the download has failed.
    pub(crate) fn download_failed(&mut self, url: &str) {
        let Some((pb, _)) = self.file_progress_bars.get(url) else {
            return;
        };
        pb.set_style(
            ProgressStyle::with_template("{msg:>12.bold}  download failed after {elapsed}")
                .unwrap(),
        );
        pb.finish();
    }

    /// Notifies self that the download is being retried.
    pub(crate) fn retrying_download(&mut self, url: &str) {
        let Some((pb, retry_time)) = self.file_progress_bars.get_mut(url) else {
            return;
        };
        *retry_time = Some(Instant::now());
        pb.set_style(ProgressStyle::with_template("{msg:>12.bold}  retrying download").unwrap());
    }
}

fn file_hash(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut downloaded = utils::FileReaderWithProgress::new_file(path, notify_handler)?;
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
