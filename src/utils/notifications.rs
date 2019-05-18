use std::fmt::{self, Display};
use std::path::Path;

use url::Url;

use crate::utils::notify::NotificationLevel;

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
    /// This thins we're tracking is not counted in bytes.
    /// Must be paired with a pop-units; our other calls are not
    /// setup to guarantee this any better.
    DownloadPushUnits(&'a str),
    /// finish using an unusual unit.
    DownloadPopUnits,
    NoCanonicalPath(&'a Path),
    ResumingPartialDownload,
    UsingCurl,
    UsingReqwest,
}

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match self {
            CreatingDirectory(_, _) | RemovingDirectory(_, _) => NotificationLevel::Verbose,
            LinkingDirectory(_, _)
            | CopyingDirectory(_, _)
            | DownloadingFile(_, _)
            | DownloadContentLengthReceived(_)
            | DownloadDataReceived(_)
            | DownloadPushUnits(_)
            | DownloadPopUnits
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
        match self {
            CreatingDirectory(name, path) => {
                write!(f, "creating {} directory: '{}'", name, path.display())
            }
            LinkingDirectory(_, dest) => write!(f, "linking directory from: '{}'", dest.display()),
            CopyingDirectory(src, _) => write!(f, "copying directory from: '{}'", src.display()),
            RemovingDirectory(name, path) => {
                write!(f, "removing {} directory: '{}'", name, path.display())
            }
            DownloadingFile(url, _) => write!(f, "downloading file from: '{}'", url),
            DownloadContentLengthReceived(len) => write!(f, "download size is: '{}'", len),
            DownloadDataReceived(data) => write!(f, "received some data of size {}", data.len()),
            DownloadPushUnits(_) => Ok(()),
            DownloadPopUnits => Ok(()),
            DownloadFinished => write!(f, "download finished"),
            NoCanonicalPath(path) => write!(f, "could not canonicalize path: '{}'", path.display()),
            ResumingPartialDownload => write!(f, "resuming partial download"),
            UsingCurl => write!(f, "downloading with curl"),
            UsingReqwest => write!(f, "downloading with reqwest"),
        }
    }
}
