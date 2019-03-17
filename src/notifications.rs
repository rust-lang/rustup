use log::warn;

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
}

impl<'a> Notification<'a> {
    pub fn log(&self) {
        use self::Notification::*;
        match self {
            DownloadContentLengthReceived(_) | DownloadDataReceived(_) | DownloadFinished => {}
            FileAlreadyDownloaded => (),
            CachedFileChecksumFailed => warn!("bad checksum for cached download"),
        }
    }
}
