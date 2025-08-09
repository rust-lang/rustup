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
    /// Mapping of files to their corresponding progress bars.
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
                file,
            ))) => {
                self.content_length_received(content_len, file);
                true
            }
            Notification::Install(In::Utils(Un::DownloadDataReceived(data, file))) => {
                self.data_received(data.len(), file);
                true
            }
            Notification::Install(In::Utils(Un::DownloadFinished(file))) => {
                self.download_finished(file);
                true
            }
            Notification::Install(In::DownloadingComponent(component, _, _)) => {
                self.create_progress_bar(component);
                true
            }
            Notification::Install(In::Utils(Un::DownloadPushUnit(_))) => true,
            Notification::Install(In::Utils(Un::DownloadPopUnit)) => true,

            _ => false,
        }
    }

    /// Helper function to find the progress bar for a given file.
    fn find_progress_bar(&mut self, file: &str) -> Option<&mut ProgressBar> {
        // During the installation this function can be called with an empty file/URL.
        if file.is_empty() {
            return None;
        }
        let component = self
            .file_progress_bars
            .keys()
            .find(|comp| file.contains(*comp))
            .cloned()?;

        self.file_progress_bars.get_mut(&component)
    }

    /// Creates a new ProgressBar for the given component.
    pub(crate) fn create_progress_bar(&mut self, component: &str) {
        let pb = ProgressBar::hidden();
        pb.set_style(
            ProgressStyle::with_template(
                "{msg:>12.bold}  [{bar:40}] {bytes}/{total_bytes} ({bytes_per_sec}, ETA: {eta})",
            )
            .unwrap()
            .progress_chars("## "),
        );
        pb.set_message(component.to_string());
        self.multi_progress_bars.add(pb.clone());
        self.file_progress_bars.insert(component.to_string(), pb);
    }

    /// Sets the length for a new ProgressBar and gives it a style.
    pub(crate) fn content_length_received(&mut self, content_len: u64, file: &str) {
        if let Some(pb) = self.find_progress_bar(file) {
            pb.set_length(content_len);
        }
    }

    /// Notifies self that data of size `len` has been received.
    pub(crate) fn data_received(&mut self, len: usize, file: &str) {
        if let Some(pb) = self.find_progress_bar(file) {
            pb.inc(len as u64);
        }
    }

    /// Notifies self that the download has finished.
    pub(crate) fn download_finished(&mut self, file: &str) {
        if let Some(pb) = self.find_progress_bar(file) {
            pb.set_style(
                ProgressStyle::with_template(
                    "{msg:>12.bold}  downloaded {total_bytes} in {elapsed}.",
                )
                .unwrap(),
            );
            let msg = pb.message();
            pb.finish_with_message(msg);
        }
    }
}
