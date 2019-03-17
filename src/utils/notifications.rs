use log::debug;

use crate::Verbosity;

#[derive(Debug)]
pub enum Notification<'a> {
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(&'a [u8]),
    /// Download has finished.
    DownloadFinished,
    ResumingPartialDownload,
}

impl<'a> Notification<'a> {
    pub fn log_with_verbosity(&self, verbosity: Verbosity) {
        match verbosity {
            Verbosity::Verbose => {
                use self::Notification::*;
                match self {
                    DownloadContentLengthReceived(len) => debug!("download size is: '{}'", len),
                    DownloadDataReceived(data) => {
                        debug!("received some data of size {}", data.len())
                    }
                    DownloadFinished => debug!("download finished"),
                    ResumingPartialDownload => debug!("resuming partial download"),
                }
            }
            Verbosity::NotVerbose => (),
        }
    }
}
