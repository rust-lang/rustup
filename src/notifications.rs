use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

use crate::dist::TargetTriple;
use crate::utils::notify::NotificationLevel;
use crate::utils::units;

#[derive(Debug)]
pub(crate) enum Notification<'a> {
    FileAlreadyDownloaded,
    CachedFileChecksumFailed,
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
    /// This would make more sense as a crate::notifications::Notification
    /// member, but the notification callback is already narrowed to
    /// utils::notifications by the time tar unpacking is called.
    SetDefaultBufferSize(usize),
    /// Both `rust-toolchain` and `rust-toolchain.toml` exist within a directory
    DuplicateToolchainFile {
        rust_toolchain: &'a Path,
        rust_toolchain_toml: &'a Path,
    },
}

impl Notification<'_> {
    pub(crate) fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match self {
            FileAlreadyDownloaded => NotificationLevel::Debug,
            DownloadingComponent(_, _, _, _) | RetryingDownload(_) => NotificationLevel::Info,
            CachedFileChecksumFailed => NotificationLevel::Warn,
            SetDefaultBufferSize(_) => NotificationLevel::Trace,
            DownloadContentLengthReceived(_, _)
            | DownloadDataReceived(_, _)
            | DownloadFinished(_)
            | DownloadFailed(_) => NotificationLevel::Debug,
            DuplicateToolchainFile { .. } => NotificationLevel::Warn,
        }
    }
}

impl Display for Notification<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match self {
            FileAlreadyDownloaded => write!(f, "reusing previously downloaded file"),
            CachedFileChecksumFailed => write!(f, "bad checksum for cached download"),
            DownloadingComponent(c, h, t, _) => {
                if Some(h) == t.as_ref() || t.is_none() {
                    write!(f, "downloading component '{c}'")
                } else {
                    write!(f, "downloading component '{}' for '{}'", c, t.unwrap())
                }
            }
            RetryingDownload(url) => write!(f, "retrying download for '{url}'"),
            SetDefaultBufferSize(size) => write!(
                f,
                "using up to {} of RAM to unpack components",
                units::Size::new(*size)
            ),
            DownloadContentLengthReceived(len, _) => write!(f, "download size is: '{len}'"),
            DownloadDataReceived(data, _) => write!(f, "received some data of size {}", data.len()),
            DownloadFinished(_) => write!(f, "download finished"),
            DownloadFailed(_) => write!(f, "download failed"),
            DuplicateToolchainFile {
                rust_toolchain,
                rust_toolchain_toml,
            } => write!(
                f,
                "both `{0}` and `{1}` exist. Using `{0}`",
                rust_toolchain
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(rust_toolchain))
                    .display(),
                rust_toolchain_toml
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(rust_toolchain_toml))
                    .display(),
            ),
        }
    }
}
