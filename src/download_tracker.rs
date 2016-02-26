use term;

use std::fmt;
use time::precise_time_s;
use std::collections::VecDeque;

/// Keep track of this many past download amounts
const DOWNLOAD_TRACK_COUNT: usize = 5;

/// Tracks download progress and displays information about it to a terminal.
pub struct DownloadTracker {
    /// Content-Length of the to-be downloaded object.
    content_len: Option<u64>,
    /// Total data downloaded in bytes.
    total_downloaded: usize,
    /// Data downloaded this second.
    downloaded_this_sec: usize,
    /// Keeps track of amount of data downloaded every last few secs.
    /// Used for averaging the download speed.
    downloaded_last_few_secs: VecDeque<usize>,
    /// Time stamp of the last second
    last_sec: Option<f64>,
    /// How many seconds have elapsed since the download started
    seconds_elapsed: u32,
    /// The terminal we write the information to.
    term: Box<term::StdoutTerminal>,
}

impl DownloadTracker {
    /// Creates a new DownloadTracker.
    pub fn new() -> Self {
        DownloadTracker {
            content_len: None,
            total_downloaded: 0,
            downloaded_this_sec: 0,
            downloaded_last_few_secs: VecDeque::with_capacity(DOWNLOAD_TRACK_COUNT),
            seconds_elapsed: 0,
            last_sec: None,
            term: term::stdout().expect("Failed to open terminal"),
        }
    }
    /// Notifies self that Content-Length information has been received.
    pub fn content_length_received(&mut self, content_len: u64) {
        self.content_len = Some(content_len);
    }
    /// Notifies self that data of size `len` has been received.
    pub fn data_received(&mut self, len: usize) {
        self.total_downloaded += len;
        self.downloaded_this_sec += len;

        match self.last_sec {
            None => self.last_sec = Some(precise_time_s()),
            Some(start) => {
                let elapsed = precise_time_s() - start;
                if elapsed >= 1.0 {
                    self.seconds_elapsed += 1;

                    self.display();
                    self.last_sec = Some(precise_time_s());
                    if self.downloaded_last_few_secs.len() == DOWNLOAD_TRACK_COUNT {
                        self.downloaded_last_few_secs.pop_back();
                    }
                    self.downloaded_last_few_secs.push_front(self.downloaded_this_sec);
                    self.downloaded_this_sec = 0;
                }
            }
        }
    }
    /// Notifies self that the download has finished.
    pub fn download_finished(&mut self) {
        let _ = writeln!(&mut self.term, "");
        self.last_sec = None;
    }
    /// Display the tracked download information to the terminal.
    fn display(&mut self) {
        let total_h = HumanReadable(self.total_downloaded as f64);

        match self.content_len {
            Some(content_len) => {
                use std::borrow::Cow;

                let percent = (self.total_downloaded as f64 / content_len as f64) * 100.;
                let content_len_h = HumanReadable(content_len as f64);
                let remaining = content_len - self.total_downloaded as u64;
                let sum = self.downloaded_last_few_secs
                              .iter()
                              .fold(0usize, |a, &v| a + v);
                let len = self.downloaded_last_few_secs.len();
                let speed = if len > 0 {
                    (sum / len) as u64
                } else {
                    0
                };
                let eta = if speed > 0 {
                    Cow::Owned(format!("{}s", remaining / speed))
                } else {
                    Cow::Borrowed("Unknown")
                };
                let _ = write!(&mut self.term,
                               "{} / {} ({:.2}%) ~{}/s ETA: {}",
                               total_h,
                               content_len_h,
                               percent,
                               HumanReadable(speed as f64),
                               eta);
            }
            None => {
                let _ = write!(&mut self.term, "{}", total_h);
            }
        }
        // delete_line() doesn't seem to clear the line properly.
        // Instead, let's just print some whitespace to clear it.
        let _ = write!(&mut self.term, "                ");
        let _ = self.term.flush();
        let _ = self.term.carriage_return();
    }
}

/// Human readable representation of data size in bytes
struct HumanReadable<T>(T);

impl<T: Into<f64> + Clone> fmt::Display for HumanReadable<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const KIB: f64 = 1024.0;
        const MIB: f64 = 1048576.0;
        let size: f64 = self.0.clone().into();

        if size >= MIB {
            write!(f, "{:.2} MiB", size / MIB)
        } else if size >= KIB {
            write!(f, "{:.2} KiB", size / KIB)
        } else {
            write!(f, "{} B", size)
        }
    }
}
