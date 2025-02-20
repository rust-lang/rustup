use std::collections::VecDeque;
use std::fmt;
use std::io::Write;
use std::time::{Duration, Instant};

use crate::dist::Notification as In;
use crate::notifications::Notification;
use crate::process::{Process, terminalsource};
use crate::utils::Notification as Un;
use crate::utils::units::{Size, Unit, UnitMode};

/// Keep track of this many past download amounts
const DOWNLOAD_TRACK_COUNT: usize = 5;

/// Tracks download progress and displays information about it to a terminal.
///
/// *not* safe for tracking concurrent downloads yet - it is basically undefined
/// what will happen.
pub(crate) struct DownloadTracker {
    /// Content-Length of the to-be downloaded object.
    content_len: Option<usize>,
    /// Total data downloaded in bytes.
    total_downloaded: usize,
    /// Data downloaded this second.
    downloaded_this_sec: usize,
    /// Keeps track of amount of data downloaded every last few secs.
    /// Used for averaging the download speed. NB: This does not necessarily
    /// represent adjacent seconds; thus it may not show the average at all.
    downloaded_last_few_secs: VecDeque<usize>,
    /// Time stamp of the last second
    last_sec: Option<Instant>,
    /// Time stamp of the start of the download
    start_sec: Option<Instant>,
    term: terminalsource::ColorableTerminal,
    /// Whether we displayed progress for the download or not.
    ///
    /// If the download is quick enough, we don't have time to
    /// display the progress info.
    /// In that case, we do not want to do some cleanup stuff we normally do.
    ///
    /// If we have displayed progress, this is the number of characters we
    /// rendered, so we can erase it cleanly.
    displayed_charcount: Option<usize>,
    /// What units to show progress in
    units: Vec<Unit>,
    /// Whether we display progress
    display_progress: bool,
    stdout_is_a_tty: bool,
}

impl DownloadTracker {
    /// Creates a new DownloadTracker.
    pub(crate) fn new_with_display_progress(display_progress: bool, process: &Process) -> Self {
        Self {
            content_len: None,
            total_downloaded: 0,
            downloaded_this_sec: 0,
            downloaded_last_few_secs: VecDeque::with_capacity(DOWNLOAD_TRACK_COUNT),
            start_sec: None,
            last_sec: None,
            term: process.stdout().terminal(process),
            displayed_charcount: None,
            units: vec![Unit::B],
            display_progress,
            stdout_is_a_tty: process.stdout().is_a_tty(process),
        }
    }

    pub(crate) fn handle_notification(&mut self, n: &Notification<'_>) -> bool {
        match *n {
            Notification::Install(In::Utils(Un::DownloadContentLengthReceived(content_len))) => {
                self.content_length_received(content_len);

                true
            }
            Notification::Install(In::Utils(Un::DownloadDataReceived(data))) => {
                if self.stdout_is_a_tty {
                    self.data_received(data.len());
                }
                true
            }
            Notification::Install(In::Utils(Un::DownloadFinished)) => {
                self.download_finished();
                true
            }
            Notification::Install(In::Utils(Un::DownloadPushUnit(unit))) => {
                self.push_unit(unit);
                true
            }
            Notification::Install(In::Utils(Un::DownloadPopUnit)) => {
                self.pop_unit();
                true
            }

            _ => false,
        }
    }

    /// Notifies self that Content-Length information has been received.
    pub(crate) fn content_length_received(&mut self, content_len: u64) {
        self.content_len = Some(content_len as usize);
    }

    /// Notifies self that data of size `len` has been received.
    pub(crate) fn data_received(&mut self, len: usize) {
        self.total_downloaded += len;
        self.downloaded_this_sec += len;

        let current_time = Instant::now();

        match self.last_sec {
            None => self.last_sec = Some(current_time),
            Some(prev) => {
                let elapsed = current_time.saturating_duration_since(prev);
                if elapsed >= Duration::from_secs(1) {
                    if self.display_progress {
                        self.display();
                    }
                    self.last_sec = Some(current_time);
                    if self.downloaded_last_few_secs.len() == DOWNLOAD_TRACK_COUNT {
                        self.downloaded_last_few_secs.pop_back();
                    }
                    self.downloaded_last_few_secs
                        .push_front(self.downloaded_this_sec);
                    self.downloaded_this_sec = 0;
                }
            }
        }
    }
    /// Notifies self that the download has finished.
    pub(crate) fn download_finished(&mut self) {
        if self.displayed_charcount.is_some() {
            // Display the finished state
            self.display();
            let _ = writeln!(self.term.lock());
        }
        self.prepare_for_new_download();
    }
    /// Resets the state to be ready for a new download.
    fn prepare_for_new_download(&mut self) {
        self.content_len = None;
        self.total_downloaded = 0;
        self.downloaded_this_sec = 0;
        self.downloaded_last_few_secs.clear();
        self.start_sec = Some(Instant::now());
        self.last_sec = None;
        self.displayed_charcount = None;
    }
    /// Display the tracked download information to the terminal.
    fn display(&mut self) {
        match self.start_sec {
            // Maybe forgot to call `prepare_for_new_download` first
            None => {}
            Some(start_sec) => {
                // Panic if someone pops the default bytes unit...
                let unit = *self.units.last().unwrap();
                let total_h = Size::new(self.total_downloaded, unit, UnitMode::Norm);
                let sum: usize = self.downloaded_last_few_secs.iter().sum();
                let len = self.downloaded_last_few_secs.len();
                let speed = if len > 0 { sum / len } else { 0 };
                let speed_h = Size::new(speed, unit, UnitMode::Rate);
                let elapsed_h = Instant::now().saturating_duration_since(start_sec);

                // First, move to the start of the current line and clear it.
                let _ = self.term.carriage_return();
                // We'd prefer to use delete_line() but on Windows it seems to
                // sometimes do unusual things
                // let _ = self.term.as_mut().unwrap().delete_line();
                // So instead we do:
                if let Some(n) = self.displayed_charcount {
                    // This is not ideal as very narrow terminals might mess up,
                    // but it is more likely to succeed until term's windows console
                    // fixes whatever's up with delete_line().
                    let _ = write!(self.term.lock(), "{}", " ".repeat(n));
                    let _ = self.term.lock().flush();
                    let _ = self.term.carriage_return();
                }

                let output = match self.content_len {
                    Some(content_len) => {
                        let content_len_h = Size::new(content_len, unit, UnitMode::Norm);
                        let percent = (self.total_downloaded as f64 / content_len as f64) * 100.;
                        let remaining = content_len - self.total_downloaded;
                        let eta_h = Duration::from_secs(if speed == 0 {
                            u64::MAX
                        } else {
                            (remaining / speed) as u64
                        });
                        format!(
                            "{} / {} ({:3.0} %) {} in {}{}",
                            total_h,
                            content_len_h,
                            percent,
                            speed_h,
                            elapsed_h.display(),
                            Eta(eta_h),
                        )
                    }
                    None => format!(
                        "Total: {} Speed: {} Elapsed: {}",
                        total_h,
                        speed_h,
                        elapsed_h.display()
                    ),
                };

                let _ = write!(self.term.lock(), "{output}");
                // Since stdout is typically line-buffered and we don't print a newline, we manually flush.
                let _ = self.term.lock().flush();
                self.displayed_charcount = Some(output.chars().count());
            }
        }
    }

    pub(crate) fn push_unit(&mut self, new_unit: Unit) {
        self.units.push(new_unit);
    }

    pub(crate) fn pop_unit(&mut self) {
        self.units.pop();
    }
}

struct Eta(Duration);

impl fmt::Display for Eta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Duration::ZERO => Ok(()),
            _ => write!(f, " ETA: {}", self.0.display()),
        }
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
