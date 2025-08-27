use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::collections::HashMap;

use crate::dist::Notification as In;
use crate::notifications::Notification;
use crate::process::Process;
use crate::utils::Notification as Un;

/// Tracks download progress and displays information about it to a terminal.
///
/// *not* safe for tracking concurrent downloads yet - it is basically undefined
/// what will happen.
pub(crate) struct DownloadTracker {
    /// MultiProgress bar for the downloads.
    multi_progress_bars: MultiProgress,
    /// Mapping of URLs being downloaded to their corresponding progress bars.
    file_progress_bars: HashMap<String, ProgressBar>,
}

impl DownloadTracker {
    /// Creates a new DownloadTracker.
    pub(crate) fn new_with_display_progress(display_progress: bool, process: &Process) -> Self {
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

    pub(crate) fn handle_notification(&mut self, n: &Notification<'_>) -> bool {
        match *n {
            Notification::Install(In::Utils(Un::DownloadContentLengthReceived(
                content_len,
                url,
            ))) => {
                if let Some(url) = url {
                    self.content_length_received(content_len, url);
                }
                true
            }
            Notification::Install(In::Utils(Un::DownloadDataReceived(data, url))) => {
                if let Some(url) = url {
                    self.data_received(data.len(), url);
                }
                true
            }
            Notification::Install(In::Utils(Un::DownloadFinished(url))) => {
                if let Some(url) = url {
                    self.download_finished(url);
                }
                true
            }
            Notification::Install(In::DownloadingComponent(component, _, _, url)) => {
                self.create_progress_bar(component.to_owned(), url.to_owned());
                true
            }
            _ => false,
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
        self.file_progress_bars.insert(url, pb);
    }

    /// Sets the length for a new ProgressBar and gives it a style.
    pub(crate) fn content_length_received(&mut self, content_len: u64, url: &str) {
        if let Some(pb) = self.file_progress_bars.get(url) {
            pb.reset();
            pb.set_length(content_len);
        }
    }

    /// Notifies self that data of size `len` has been received.
    pub(crate) fn data_received(&mut self, len: usize, url: &str) {
        if let Some(pb) = self.file_progress_bars.get(url) {
            pb.inc(len as u64);
        }
    }

    /// Notifies self that the download has finished.
    pub(crate) fn download_finished(&mut self, url: &str) {
        let Some(pb) = self.file_progress_bars.get(url) else {
            return;
        };
        pb.set_style(
            ProgressStyle::with_template("{msg:>12.bold}  downloaded {total_bytes} in {elapsed}")
                .unwrap(),
        );
        pb.finish();
    }
}
