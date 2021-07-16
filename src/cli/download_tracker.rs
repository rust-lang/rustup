use std::fmt;
use std::time::Duration;

use crate::dist::Notification as In;
use crate::utils::tty;
use crate::utils::Notification as Un;
use crate::Notification;

/// Tracks download progress and displays information about it to a terminal.
pub struct DownloadTracker {
    progress_bar: indicatif::ProgressBar,
}

impl DownloadTracker {
    /// Creates a new DownloadTracker.
    pub fn new() -> Self {
        let progress_bar = indicatif::ProgressBar::hidden();
        progress_bar.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        Self { progress_bar }
    }

    pub fn with_display_progress(self, display_progress: bool) -> Self {
        if display_progress {
            self.progress_bar.set_style(
                indicatif::ProgressStyle::default_bar().template(
                    "{bytes} / {total_bytes} ({percent:3.0}%) {bytes_per_sec} ETA: {eta}",
                ),
            );
            self.progress_bar
                .set_draw_target(indicatif::ProgressDrawTarget::stdout());
        } else {
            self.progress_bar.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        }
        self
    }

    pub(crate) fn handle_notification(&mut self, n: &Notification<'_>) -> bool {
        match *n {
            Notification::Install(In::Utils(Un::DownloadContentLengthReceived(content_len))) => {
                self.content_length_received(content_len);

                true
            }
            Notification::Install(In::Utils(Un::DownloadDataReceived(data))) => {
                if tty::stdout_isatty() {
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
        self.progress_bar.set_length(content_len);
    }

    /// Notifies self that data of size `len` has been received.
    pub fn data_received(&mut self, len: usize) {
        self.progress_bar.inc(len as u64);
    }

    /// Notifies self that the download has finished.
    pub fn download_finished(&mut self) {
        self.progress_bar.finish();
        self.progress_bar.reset();
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
            (0, 0, 0, s) => write!(f, "{:2.0}s", s),
            (0, 0, m, s) => write!(f, "{:2.0}m {:2.0}s", m, s),
            (0, h, m, s) => write!(f, "{:2.0}h {:2.0}m {:2.0}s", h, m, s),
            (d, h, m, s) => write!(f, "{:3.0}d {:2.0}h {:2.0}m {:2.0}s", d, h, m, s),
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
