use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::debug;

use crate::notifications::Notification;
use crate::process::Process;

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
