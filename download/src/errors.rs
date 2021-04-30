use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("http request returned an unsuccessful status code: {0}")]
    HttpStatus(u32),
    #[error("file not found")]
    FileNotFound,
    #[error("download backend '{0}' unavailable")]
    BackendUnavailable(&'static str),
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[cfg(feature = "reqwest-backend")]
    #[error(transparent)]
    Reqwest(#[from] ::reqwest::Error),
    #[error(transparent)]
    CurlError(#[from] curl::Error),
}
