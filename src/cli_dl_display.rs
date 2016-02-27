use term;
use std::fmt;
use multirust::Notification;
use rust_install::Notification as In;
use rust_install::utils::Notification as Un;
use std::cell::{Cell, RefCell};

pub struct DownloadDisplayer {
    content_len: Cell<Option<u64>>,
    total_downloaded: Cell<usize>,
    term: RefCell<Box<term::StdoutTerminal>>,
}

impl DownloadDisplayer {
    pub fn new() -> DownloadDisplayer {
        DownloadDisplayer {
            content_len: Cell::new(None),
            total_downloaded: Cell::new(0),
            term: RefCell::new(term::stdout().expect("Failed to open terminal.")),
        }
    }

    pub fn handle_notification(&self, n: &Notification) -> bool {
        let is_atty = super::stderr_isatty();
        let is_unix = cfg!(unix);

        // FIXME: term.carriage_return does not work on windows
        if is_atty && is_unix {
            self.handle_notification_tty(n)
        } else {
            self.handle_notification_null(n)
        }
    }

    pub fn handle_notification_tty(&self, n: &Notification) -> bool {
        match n {
            &Notification::Install(In::Utils(Un::DownloadContentLengthReceived(len))) => {
                self.content_len.set(Some(len));
                self.total_downloaded.set(0);
            }
            &Notification::Install(In::Utils(Un::DownloadDataReceived(len))) => {
                let mut t = self.term.borrow_mut();
                self.total_downloaded.set(self.total_downloaded.get() + len);
                let total_downloaded = self.total_downloaded.get();
                let total_h = HumanReadable(total_downloaded as f64);

                match self.content_len.get() {
                    Some(content_len) => {
                        let percent = (total_downloaded as f64 / content_len as f64) * 100.;
                        let content_len_h = HumanReadable(content_len as f64);
                        let _ = write!(t, "{} / {} ({:.2}%)", total_h, content_len_h, percent);
                    }
                    None => {
                        let _ = write!(t, "{}", total_h);
                    }
                }
                // delete_line() doesn't seem to clear the line properly.
                // Instead, let's just print some whitespace to clear it.
                let _ = write!(t, "                ");
                let _ = t.flush();
                let _ = t.carriage_return();
            }
            &Notification::Install(In::Utils(Un::DownloadFinished)) => {
                let _ = writeln!(self.term.borrow_mut(), "");
            }
            _ => return false
        }

        true
    }

    pub fn handle_notification_null(&self, n: &Notification) -> bool {
        match n {
            &Notification::Install(In::Utils(Un::DownloadContentLengthReceived)) |
            &Notification::Install(In::Utils(Un::DownloadDataReceived(_))) |
            &Notification::Install(In::Utils(Un::DownloadFinished)) => {
                true
            }
            _ => false
        }
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

