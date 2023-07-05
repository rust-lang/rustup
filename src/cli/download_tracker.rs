use std::fmt;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::currentprocess::{filesource::StdoutSource, process};
use crate::dist::Notification as In;
use crate::utils::Notification as Un;
use crate::Notification;

fn new_hidden_progress_bar() -> ProgressBar {
    let progress_bar = ProgressBar::hidden();
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("Total: {bytes} Speed: {bytes_per_sec} Elapsed: {elapsed}")
            .expect("invalid template string for progress bar"),
    );
    progress_bar
}

/// Tracks download progress and displays information about it to a terminal.
///
/// *not* safe for tracking concurrent downloads yet - it is basically undefined
/// what will happen.
pub(crate) struct DownloadTracker {
    progress_bar: ProgressBar,
    /// Whether we display progress
    display_progress: bool,
}

impl DownloadTracker {
    /// Creates a new DownloadTracker.
    pub(crate) fn new() -> Self {
        Self {
            progress_bar: new_hidden_progress_bar(),
            display_progress: true,
        }
    }

    pub(crate) fn with_display_progress(mut self, display_progress: bool) -> Self {
        self.display_progress = display_progress;
        self
    }

    pub(crate) fn handle_notification(&mut self, n: &Notification<'_>) -> bool {
        match *n {
            Notification::Install(In::Utils(Un::DownloadContentLengthReceived(content_len))) => {
                self.content_length_received(content_len);
                true
            }
            Notification::Install(In::Utils(Un::DownloadDataReceived(data))) => {
                if process().stdout().is_a_tty() {
                    self.data_received(data.len());
                }
                true
            }
            Notification::Install(In::Utils(Un::DownloadFinished)) => {
                self.download_finished();
                true
            }
            _ => false,
        }
    }

    /// Notifies self that Content-Length information has been received.
    pub fn content_length_received(&mut self, content_len: u64) {
        if self.display_progress {
            let progress_bar = ProgressBar::hidden();
            progress_bar.set_length(content_len);
            progress_bar
                .set_style(ProgressStyle::default_bar().template(
                " {bytes} / {total_bytes} ({percent:3.0}%) {bytes_per_sec} in {elapsed} ETA: {eta}",
            ).expect("invalid template string for progress bar"),);
            self.progress_bar = progress_bar;
        }
    }

    /// Notifies self that data of size `len` has been received.
    pub fn data_received(&mut self, len: usize) {
        self.progress_bar.inc(len as u64);
        if self.display_progress
            && self.progress_bar.is_hidden()
            && self.progress_bar.elapsed() >= Duration::from_secs(1)
        {
            self.progress_bar
                .set_draw_target(ProgressDrawTarget::stdout());
        }
    }

    /// Notifies self that the download has finished.
    pub fn download_finished(&mut self) {
        if self.display_progress && self.progress_bar.elapsed() >= Duration::from_secs(1) {
            self.progress_bar.finish();
        }
        self.progress_bar = new_hidden_progress_bar();
    }
}

trait DurationDisplay {
    fn display(self) -> Display;
}

impl DurationDisplay for Duration {
    fn display(self) -> Display {
        Display(self)
    }
}

/// Human readable representation of a `Duration`.
struct Display(Duration);

impl fmt::Display for Display {
    #[allow(clippy::many_single_char_names)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const SECS_PER_YEAR: u64 = 60 * 60 * 24 * 365;
        let secs = self.0.as_secs();
        if secs > SECS_PER_YEAR {
            return f.write_str("Unknown");
        }
        match format_dhms(secs) {
            (0, 0, 0, s) => write!(f, "{s:2.0}s"),
            (0, 0, m, s) => write!(f, "{m:2.0}m {s:2.0}s"),
            (0, h, m, s) => write!(f, "{h:2.0}h {m:2.0}m {s:2.0}s"),
            (d, h, m, s) => write!(f, "{d:3.0}d {h:2.0}h {m:2.0}m {s:2.0}s"),
        }
    }
}

// we're doing modular arithmetic, treat as integer
fn format_dhms(sec: u64) -> (u64, u8, u8, u8) {
    let (mins, sec) = (sec / 60, (sec % 60) as u8);
    let (hours, mins) = (mins / 60, (mins % 60) as u8);
    let (days, hours) = (hours / 24, (hours % 24) as u8);
    (days, hours, mins, sec)
}

#[cfg(test)]
mod tests {
    use rustup_macros::unit_test as test;

    use super::format_dhms;

    #[test]
    fn download_tracker_format_dhms_test() {
        assert_eq!(format_dhms(2), (0, 0, 0, 2));

        assert_eq!(format_dhms(60), (0, 0, 1, 0));

        assert_eq!(format_dhms(3_600), (0, 1, 0, 0));

        assert_eq!(format_dhms(3_600 * 24), (1, 0, 0, 0));

        assert_eq!(format_dhms(52_292), (0, 14, 31, 32));

        assert_eq!(format_dhms(222_292), (2, 13, 44, 52));
    }
}
