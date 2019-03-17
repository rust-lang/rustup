use log::{debug, warn};

use crate::dist::dist::TargetTriple;
use crate::Verbosity;

#[derive(Debug)]
pub enum Notification<'a> {
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(&'a [u8]),
    /// Download has finished.
    DownloadFinished,

    FileAlreadyDownloaded,
    CachedFileChecksumFailed,
    ComponentUnavailable(&'a str, Option<&'a TargetTriple>),
}

impl<'a> Notification<'a> {
    pub fn log_with_verbosity(&self, verbosity: Verbosity) {
        use self::Notification::*;
        let verbose = match verbosity {
            Verbosity::Verbose => true,
            Verbosity::NotVerbose => false,
        };
        match self {
            DownloadContentLengthReceived(_) | DownloadDataReceived(_) | DownloadFinished
                if !verbose => {}
            FileAlreadyDownloaded if !verbose => (),
            DownloadContentLengthReceived(len) => debug!("download size is: '{}'", len),
            DownloadDataReceived(data) => debug!("received some data of size {}", data.len()),
            DownloadFinished => debug!("download finished"),
            FileAlreadyDownloaded => debug!("reusing previously downloaded file"),
            CachedFileChecksumFailed => warn!("bad checksum for cached download"),
            ComponentUnavailable(pkg, toolchain) => warn!(
                "component '{}' is not available anymore{}",
                pkg,
                match toolchain {
                    Some(tc) => format!(" on target '{}'", tc),
                    None => "".to_string(),
                }
            ),
        }
    }
}
