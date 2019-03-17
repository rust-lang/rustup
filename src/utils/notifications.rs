use std::fmt::{self, Display};
use std::path::Path;

use url::Url;

use crate::utils::notify::NotificationLevel;

#[derive(Debug)]
pub enum Notification<'a> {
    DownloadingFile(&'a Url, &'a Path),
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(&'a [u8]),
    /// Download has finished.
    DownloadFinished,
    NoCanonicalPath(&'a Path),
    ResumingPartialDownload,
    UsingCurl,
    UsingReqwest,
}

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            DownloadingFile(_, _)
            | DownloadContentLengthReceived(_)
            | DownloadDataReceived(_)
            | DownloadFinished
            | ResumingPartialDownload
            | UsingCurl
            | UsingReqwest => NotificationLevel::Verbose,
            NoCanonicalPath(_) => NotificationLevel::Warn,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            DownloadingFile(url, _) => write!(f, "downloading file from: '{}'", url),
            DownloadContentLengthReceived(len) => write!(f, "download size is: '{}'", len),
            DownloadDataReceived(data) => write!(f, "received some data of size {}", data.len()),
            DownloadFinished => write!(f, "download finished"),
            NoCanonicalPath(path) => write!(f, "could not canonicalize path: '{}'", path.display()),
            ResumingPartialDownload => write!(f, "resuming partial download"),
            UsingCurl => write!(f, "downloading with curl"),
            UsingReqwest => write!(f, "downloading with reqwest"),
        }
    }
}
