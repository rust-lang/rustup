use std::fmt::{self, Display};

use crate::dist::TargetTriple;
use crate::utils::notify::NotificationLevel;

#[derive(Debug)]
pub(crate) enum Notification<'a> {
    /// The URL of the download is passed as the last argument, to allow us to track concurrent downloads.
    DownloadingComponent(&'a str, &'a TargetTriple, Option<&'a TargetTriple>, &'a str),
    RetryingDownload(&'a str),
    /// Received the Content-Length of the to-be downloaded data with
    /// the respective URL of the download (for tracking concurrent downloads).
    DownloadContentLengthReceived(u64, Option<&'a str>),
    /// Received some data.
    DownloadDataReceived(&'a [u8], Option<&'a str>),
    /// Download has finished.
    DownloadFinished(Option<&'a str>),
    /// Download has failed.
    DownloadFailed(&'a str),
}

impl Notification<'_> {
    pub(crate) fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match self {
            DownloadingComponent(_, _, _, _) | RetryingDownload(_) => NotificationLevel::Info,
            DownloadContentLengthReceived(_, _)
            | DownloadDataReceived(_, _)
            | DownloadFinished(_)
            | DownloadFailed(_) => NotificationLevel::Debug,
        }
    }
}

impl Display for Notification<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match self {
            DownloadingComponent(c, h, t, _) => {
                if Some(h) == t.as_ref() || t.is_none() {
                    write!(f, "downloading component '{c}'")
                } else {
                    write!(f, "downloading component '{}' for '{}'", c, t.unwrap())
                }
            }
            RetryingDownload(url) => write!(f, "retrying download for '{url}'"),
            DownloadContentLengthReceived(len, _) => write!(f, "download size is: '{len}'"),
            DownloadDataReceived(data, _) => write!(f, "received some data of size {}", data.len()),
            DownloadFinished(_) => write!(f, "download finished"),
            DownloadFailed(_) => write!(f, "download failed"),
        }
    }
}
