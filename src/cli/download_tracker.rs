use crate::term2;
use rustup::dist::Notification as In;
use rustup::utils::tty;
use rustup::utils::Notification as Un;
use rustup::Notification;
use std::collections::VecDeque;
use std::fmt;
use std::io::Write;
use term::Terminal;
use time::precise_time_s;

/// Keep track of this many past download amounts
const DOWNLOAD_TRACK_COUNT: usize = 5;

/// Tracks download progress and displays information about it to a terminal.
pub struct DownloadTracker {
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
    last_sec: Option<f64>,
    /// Time stamp of the start of the download
    start_sec: f64,
    /// The terminal we write the information to.
    /// XXX: Could be a term trait, but with #1818 on the horizon that
    ///      is a pointless change to make - better to let that transition
    ///      happen and take stock after that.
    term: term2::StdoutTerminal,
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
    units: Vec<String>,
}

impl DownloadTracker {
    /// Creates a new DownloadTracker.
    pub fn new() -> Self {
        DownloadTracker {
            content_len: None,
            total_downloaded: 0,
            downloaded_this_sec: 0,
            downloaded_last_few_secs: VecDeque::with_capacity(DOWNLOAD_TRACK_COUNT),
            start_sec: precise_time_s(),
            last_sec: None,
            term: term2::stdout(),
            displayed_charcount: None,
            units: vec!["B".into(); 1],
        }
    }

    pub fn handle_notification(&mut self, n: &Notification<'_>) -> bool {
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
            Notification::Install(In::Utils(Un::DownloadPushUnits(units))) => {
                self.push_units(units.into());
                true
            }
            Notification::Install(In::Utils(Un::DownloadPopUnits)) => {
                self.pop_units();
                true
            }

            _ => false,
        }
    }

    /// Notifies self that Content-Length information has been received.
    pub fn content_length_received(&mut self, content_len: u64) {
        self.content_len = Some(content_len as usize);
    }

    /// Notifies self that data of size `len` has been received.
    pub fn data_received(&mut self, len: usize) {
        self.total_downloaded += len;
        self.downloaded_this_sec += len;

        let current_time = precise_time_s();

        match self.last_sec {
            None => self.last_sec = Some(current_time),
            Some(prev) => {
                let elapsed = current_time - prev;
                if elapsed >= 1.0 {
                    self.display();
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
    pub fn download_finished(&mut self) {
        if self.displayed_charcount.is_some() {
            // Display the finished state
            self.display();
            let _ = writeln!(self.term);
        }
        self.prepare_for_new_download();
    }
    // we're doing modular arithmetic, treat as integer
    pub fn from_seconds(sec: u32) -> (u32, u32, u32, u32) {
        let d = sec / (24 * 3600);
        let h = sec % (24 * 3600) / 3600;
        let min = sec % 3600 / 60;
        let sec = sec % 60;

        (d, h, min, sec)
    }
    /// Resets the state to be ready for a new download.
    fn prepare_for_new_download(&mut self) {
        self.content_len = None;
        self.total_downloaded = 0;
        self.downloaded_this_sec = 0;
        self.downloaded_last_few_secs.clear();
        self.start_sec = precise_time_s();
        self.last_sec = None;
        self.displayed_charcount = None;
    }
    /// Display the tracked download information to the terminal.
    fn display(&mut self) {
        // Panic if someone pops the default bytes unit...
        let units = &self.units.last().unwrap();
        let total_h = Size(self.total_downloaded, units);
        let sum: usize = self.downloaded_last_few_secs.iter().sum();
        let len = self.downloaded_last_few_secs.len();
        let speed = if len > 0 { sum / len } else { 0 };
        let speed_h = Size(speed, units);
        let elapsed_h = Duration(precise_time_s() - self.start_sec);

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
            let _ = write!(self.term, "{}", " ".repeat(n));
            let _ = self.term.flush();
            let _ = self.term.carriage_return();
        }

        let output = match self.content_len {
            Some(content_len) => {
                let content_len_h = Size(content_len, units);
                let content_len = content_len as f64;
                let percent = (self.total_downloaded as f64 / content_len) * 100.;
                let remaining = content_len - self.total_downloaded as f64;
                let eta_h = Duration(remaining / speed as f64);
                format!(
                    "{} / {} ({:3.0} %) {}/s in {} ETA: {}",
                    total_h, content_len_h, percent, speed_h, elapsed_h, eta_h
                )
            }
            None => format!(
                "Total: {} Speed: {}/s Elapsed: {}",
                total_h, speed_h, elapsed_h
            ),
        };

        let _ = write!(self.term, "{}", output);
        // Since stdout is typically line-buffered and we don't print a newline, we manually flush.
        let _ = self.term.flush();
        self.displayed_charcount = Some(output.chars().count());
    }

    pub fn push_units(&mut self, new_units: String) {
        self.units.push(new_units);
    }

    pub fn pop_units(&mut self) {
        self.units.pop();
    }
}

/// Human readable representation of duration(seconds).
struct Duration(f64);

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // repurposing the alternate mode for ETA
        let sec = self.0;

        if sec.is_infinite() {
            write!(f, "Unknown")
        } else {
            match DownloadTracker::from_seconds(sec as u32) {
                (d, h, m, s) if d > 0 => write!(f, "{:3.0}d {:2.0}h {:2.0}m {:2.0}s", d, h, m, s),
                (0, h, m, s) if h > 0 => write!(f, "{:2.0}h {:2.0}m {:2.0}s", h, m, s),
                (0, 0, m, s) if m > 0 => write!(f, "{:2.0}m {:2.0}s", m, s),
                (_, _, _, s) => write!(f, "{:2.0}s", s),
            }
        }
    }
}

/// Human readable size (some units)
struct Size<'a>(usize, &'a str);

impl<'a> fmt::Display for Size<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const KI: f64 = 1024.0;
        const MI: f64 = KI * KI;
        let size = self.0 as f64;

        if size >= MI {
            write!(f, "{:5.1} Mi{}", size / MI, self.1) // XYZ.P Mi
        } else if size >= KI {
            write!(f, "{:5.1} Ki{}", size / KI, self.1)
        } else {
            write!(f, "{:3.0} {}", size, self.1)
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn download_tracker_from_seconds_test() {
        use crate::download_tracker::DownloadTracker;
        assert_eq!(DownloadTracker::from_seconds(2), (0, 0, 0, 2));

        assert_eq!(DownloadTracker::from_seconds(60), (0, 0, 1, 0));

        assert_eq!(DownloadTracker::from_seconds(3_600), (0, 1, 0, 0));

        assert_eq!(DownloadTracker::from_seconds(3_600 * 24), (1, 0, 0, 0));

        assert_eq!(DownloadTracker::from_seconds(52_292), (0, 14, 31, 32));

        assert_eq!(DownloadTracker::from_seconds(222_292), (2, 13, 44, 52));
    }

}
