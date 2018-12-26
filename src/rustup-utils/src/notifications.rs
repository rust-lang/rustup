use std::fmt::{self, Display};
use std::path::Path;

use url::Url;

use crate::notify::NotificationLevel;

#[derive(Debug)]
pub enum Notification<'a> {
    CreatingDirectory(&'a str, &'a Path),
    LinkingDirectory(&'a Path, &'a Path),
    CopyingDirectory(&'a Path, &'a Path),
    RemovingDirectory(&'a str, &'a Path),
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
    UsingHyperDeprecated,
}

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            CreatingDirectory(_, _) | RemovingDirectory(_, _) => NotificationLevel::Verbose,
            LinkingDirectory(_, _)
            | CopyingDirectory(_, _)
            | DownloadingFile(_, _)
            | DownloadContentLengthReceived(_)
            | DownloadDataReceived(_)
            | DownloadFinished
            | ResumingPartialDownload
            | UsingCurl
            | UsingReqwest => NotificationLevel::Verbose,
            UsingHyperDeprecated | NoCanonicalPath(_) => NotificationLevel::Warn,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            CreatingDirectory(name, path) => {
                write!(f, "creating {} directory: '{}'", name, path.display())
            }
            LinkingDirectory(_, dest) => write!(f, "linking directory from: '{}'", dest.display()),
            CopyingDirectory(src, _) => write!(f, "coping directory from: '{}'", src.display()),
            RemovingDirectory(name, path) => {
                write!(f, "removing {} directory: '{}'", name, path.display())
            }
            DownloadingFile(url, _) => write!(f, "downloading file from: '{}'", url),
            DownloadContentLengthReceived(len) => write!(f, "download size is: '{}'", len),
            DownloadDataReceived(data) => write!(f, "received some data of size {}", data.len()),
            DownloadFinished => write!(f, "download finished"),
            NoCanonicalPath(path) => write!(f, "could not canonicalize path: '{}'", path.display()),
            ResumingPartialDownload => write!(f, "resuming partial download"),
            UsingCurl => write!(f, "downloading with curl"),
            UsingReqwest => write!(f, "downloading with reqwest"),
            UsingHyperDeprecated => f.write_str(
                "RUSTUP_USE_HYPER environment variable is deprecated,\
                 use RUSTUP_USE_REQWEST instead",
            ),
        }
    }
}
