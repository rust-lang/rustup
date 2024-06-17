use std::fmt::{self, Display};
use std::path::Path;

use url::Url;

use crate::utils::notify::NotificationLevel;
use crate::utils::units::{self, Unit};

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
    /// The things we're tracking that are not counted in bytes.
    /// Must be paired with a pop-units; our other calls are not
    /// setup to guarantee this any better.
    DownloadPushUnit(Unit),
    /// finish using an unusual unit.
    DownloadPopUnit,
    NoCanonicalPath(&'a Path),
    ResumingPartialDownload,
    /// This would make more sense as a crate::notifications::Notification
    /// member, but the notification callback is already narrowed to
    /// utils::notifications by the time tar unpacking is called.
    SetDefaultBufferSize(usize),
    Error(String),
    UsingCurl,
    UsingReqwest,
    /// Renaming encountered a file in use error and is retrying.
    /// The InUse aspect is a heuristic - the OS specifies
    /// Permission denied, but as we work in users home dirs and
    /// running programs like virus scanner are known to cause this
    /// the heuristic is quite good.
    RenameInUse(&'a Path, &'a Path),
}

impl<'a> Notification<'a> {
    pub(crate) fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match self {
            SetDefaultBufferSize(_) => NotificationLevel::Trace,
            CreatingDirectory(_, _)
            | RemovingDirectory(_, _)
            | LinkingDirectory(_, _)
            | CopyingDirectory(_, _)
            | DownloadingFile(_, _)
            | DownloadContentLengthReceived(_)
            | DownloadDataReceived(_)
            | DownloadPushUnit(_)
            | DownloadPopUnit
            | DownloadFinished
            | ResumingPartialDownload
            | UsingCurl
            | UsingReqwest => NotificationLevel::Debug,
            RenameInUse(_, _) => NotificationLevel::Info,
            NoCanonicalPath(_) => NotificationLevel::Warn,
            Error(_) => NotificationLevel::Error,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match self {
            CreatingDirectory(name, path) => {
                write!(f, "creating {} directory: '{}'", name, path.display())
            }
            Error(e) => write!(f, "error: '{e}'"),
            LinkingDirectory(_, dest) => write!(f, "linking directory from: '{}'", dest.display()),
            CopyingDirectory(src, _) => write!(f, "copying directory from: '{}'", src.display()),
            RemovingDirectory(name, path) => {
                write!(f, "removing {} directory: '{}'", name, path.display())
            }
            RenameInUse(src, dest) => write!(
                f,
                "retrying renaming '{}' to '{}'",
                src.display(),
                dest.display()
            ),
            SetDefaultBufferSize(size) => write!(
                f,
                "using up to {} of RAM to unpack components",
                units::Size::new(*size, units::Unit::B, units::UnitMode::Norm)
            ),
            DownloadingFile(url, _) => write!(f, "downloading file from: '{url}'"),
            DownloadContentLengthReceived(len) => write!(f, "download size is: '{len}'"),
            DownloadDataReceived(data) => write!(f, "received some data of size {}", data.len()),
            DownloadPushUnit(_) => Ok(()),
            DownloadPopUnit => Ok(()),
            DownloadFinished => write!(f, "download finished"),
            NoCanonicalPath(path) => write!(f, "could not canonicalize path: '{}'", path.display()),
            ResumingPartialDownload => write!(f, "resuming partial download"),
            UsingCurl => write!(f, "downloading with curl"),
            UsingReqwest => write!(f, "downloading with reqwest"),
        }
    }
}
