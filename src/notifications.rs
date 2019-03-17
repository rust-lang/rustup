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
        match self {
            DownloadContentLengthReceived(len) => match verbosity {
                Verbosity::Verbose => debug!("download size is: '{}'", len),
                Verbosity::NotVerbose => (),
            },
            DownloadDataReceived(data) => match verbosity {
                Verbosity::Verbose => debug!("received some data of size {}", data.len()),
                Verbosity::NotVerbose => (),
            },
            DownloadFinished => match verbosity {
                Verbosity::Verbose => debug!("download finished"),
                Verbosity::NotVerbose => (),
            },
            FileAlreadyDownloaded => match verbosity {
                Verbosity::Verbose => debug!("reusing previously downloaded file"),
                Verbosity::NotVerbose => (),
            },
            CachedFileChecksumFailed => warn!("bad checksum for cached download"),
            ComponentUnavailable(pkg, toolchain) => {
                if let Some(tc) = toolchain {
                    warn!(
                        "component '{}' is not available anymore on target '{}'",
                        pkg, tc
                    )
                } else {
                    warn!("component '{}' is not available anymore", pkg)
                }
            }
        }
    }
}
