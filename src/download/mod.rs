//! Easy file downloading

use std::cell::RefCell;
use std::fs::{self, OpenOptions, remove_file};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZero;
use std::path::Path;
use std::str::FromStr;
#[cfg(feature = "reqwest-rustls-tls")]
use std::sync::Arc;
#[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, anyhow};
use reqwest::{Client, ClientBuilder, Proxy, header};
#[cfg(feature = "reqwest-rustls-tls")]
use rustls::crypto::aws_lc_rs;
#[cfg(feature = "reqwest-rustls-tls")]
use rustls_platform_verifier::Verifier;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio_stream::StreamExt;
use tracing::{debug, warn};
use url::Url;

#[cfg(all(feature = "reqwest-rustls-tls", not(target_os = "android")))]
use crate::anchors::RUSTUP_TRUST_ANCHORS;
use crate::{dist::download::DownloadStatus, errors::RustupError, process::Process};

#[cfg(test)]
mod tests;

pub(crate) async fn download_file(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    status: Option<&DownloadStatus>,
    process: &Process,
) -> anyhow::Result<()> {
    download_file_with_resume(url, path, hasher, false, status, process).await
}

pub(crate) async fn download_file_with_resume(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    resume_from_partial: bool,
    status: Option<&DownloadStatus>,
    process: &Process,
) -> anyhow::Result<()> {
    match download_file_(url, path, hasher, resume_from_partial, status, process).await {
        Ok(_) => Ok(()),
        Err(e) => {
            if e.downcast_ref::<io::Error>().is_some() {
                return Err(e);
            }
            let is_client_error = match e.downcast_ref::<DownloadError>() {
                // Specifically treat the bad partial range error as not our
                // fault in case it was something odd which happened.
                Some(DownloadError::HttpStatus(416)) => false,
                Some(DownloadError::HttpStatus(400..=499)) | Some(DownloadError::FileNotFound) => {
                    true
                }
                _ => false,
            };
            Err(e).with_context(|| {
                if is_client_error {
                    RustupError::DownloadNotExists {
                        url: url.clone(),
                        path: path.to_path_buf(),
                    }
                } else {
                    RustupError::DownloadingFile {
                        url: url.clone(),
                        path: path.to_path_buf(),
                    }
                }
            })
        }
    }
}

pub(crate) fn is_network_failure(err: &anyhow::Error) -> bool {
    match err.downcast_ref::<DownloadError>() {
        #[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
        Some(DownloadError::Reqwest(e)) => e.is_timeout() || e.is_connect(),
        _ => false,
    }
}

async fn download_file_(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    resume_from_partial: bool,
    status: Option<&DownloadStatus>,
    process: &Process,
) -> anyhow::Result<()> {
    debug!(url = %url, "downloading file");
    let hasher = RefCell::new(hasher);

    // This callback will write the download to disk and optionally
    // hash the contents, then forward the notification up the stack
    let callback: &dyn Fn(Event<'_>) -> anyhow::Result<()> = &|msg| {
        if let Event::DownloadDataReceived(data) = msg
            && let Some(h) = hasher.borrow_mut().as_mut()
        {
            h.update(data);
        }

        match msg {
            Event::DownloadContentLengthReceived(len) => {
                if let Some(status) = status {
                    status.received_length(len)
                }
            }
            Event::DownloadDataReceived(data) => {
                if let Some(status) = status {
                    status.received_data(data.len())
                }
            }
            Event::ResumingPartialDownload => debug!("resuming partial download"),
        }

        Ok(())
    };

    // Download the file

    let use_rustls = process.var_os("RUSTUP_USE_RUSTLS").map(|it| it != "0");
    if use_rustls == Some(false) {
        warn!(
            "RUSTUP_USE_RUSTLS is set to `0`; the native-tls backend is deprecated,
            please file an issue if the default download backend does not work for your use case"
        );
    }

    let backend = match use_rustls {
        // If the environment explicitly selects a TLS backend that's unavailable, error out.
        #[cfg(not(feature = "reqwest-rustls-tls"))]
        Some(true) => {
            return Err(anyhow!(
                "RUSTUP_USE_RUSTLS is set, but this rustup distribution was not built with the reqwest-rustls-tls feature"
            ));
        }
        #[cfg(not(feature = "reqwest-native-tls"))]
        Some(false) => {
            return Err(anyhow!(
                "RUSTUP_USE_RUSTLS is set to false, but this rustup distribution was not built with the reqwest-native-tls feature"
            ));
        }

        // Prefer explicit selections before falling back to the default TLS stack.
        #[cfg(feature = "reqwest-native-tls")]
        Some(false) => Backend::NativeTls,

        // The default fallback is `rustls`, which should be used whenever available.
        #[cfg(feature = "reqwest-rustls-tls")]
        _ => Backend::Rustls,

        // The `rustls` feature is disabled, fall back to `native-tls` instead.
        #[cfg(all(not(feature = "reqwest-rustls-tls"), feature = "reqwest-native-tls"))]
        _ => Backend::NativeTls,
    };

    let timeout = Duration::from_secs(match process.var("RUSTUP_DOWNLOAD_TIMEOUT") {
        Ok(s) => NonZero::from_str(&s)
            .context(
                "invalid value in RUSTUP_DOWNLOAD_TIMEOUT -- must be a natural number greater than zero",
            )?
            .get(),
        Err(_) => 180,
    });

    debug!("downloading with reqwest");

    let res = backend
        .download_to_path(url, path, resume_from_partial, Some(callback), timeout)
        .await;

    // The notification should only be sent if the download was successful (i.e. didn't timeout)
    if let Some(status) = status {
        match &res {
            Ok(_) => status.finished(),
            Err(_) => status.failed(),
        };
    }

    res
}

/// User agent header value for HTTP request.
/// See: https://github.com/rust-lang/rustup/issues/2860.
#[cfg(feature = "reqwest-native-tls")]
const REQWEST_DEFAULT_TLS_USER_AGENT: &str = concat!(
    "rustup/",
    env!("CARGO_PKG_VERSION"),
    " (reqwest; default-tls)"
);

#[cfg(feature = "reqwest-rustls-tls")]
const REQWEST_RUSTLS_TLS_USER_AGENT: &str =
    concat!("rustup/", env!("CARGO_PKG_VERSION"), " (reqwest; rustls)");

#[derive(Debug, Copy, Clone)]
enum Backend {
    #[cfg(feature = "reqwest-rustls-tls")]
    Rustls,
    #[cfg(feature = "reqwest-native-tls")]
    NativeTls,
}

impl Backend {
    async fn download_to_path(
        self,
        url: &Url,
        path: &Path,
        resume_from_partial: bool,
        callback: Option<DownloadCallback<'_>>,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let Err(err) = self
            .download_impl(url, path, resume_from_partial, callback, timeout)
            .await
        else {
            return Ok(());
        };

        // TODO: Currently, we only refrain from removing the cached download
        // if there was a network failure from the client side.
        // It may be worth looking for other cases where removal is also not desired.
        Err(
            if !(resume_from_partial && is_network_failure(&err))
                && let Err(file_err) = remove_file(path).context("cleaning up cached downloads")
            {
                file_err.context(err)
            } else {
                err
            },
        )
    }

    async fn download_impl(
        self,
        url: &Url,
        path: &Path,
        resume_from_partial: bool,
        callback: Option<DownloadCallback<'_>>,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let (file, resume_from) = if resume_from_partial {
            // TODO: blocking call
            let possible_partial = OpenOptions::new().read(true).open(path);

            let downloaded_so_far = if let Ok(mut partial) = possible_partial {
                if let Some(cb) = callback {
                    cb(Event::ResumingPartialDownload)?;

                    let mut buf = vec![0; 32768];
                    let mut downloaded_so_far = 0;
                    loop {
                        let n = partial.read(&mut buf)?;
                        downloaded_so_far += n as u64;
                        if n == 0 {
                            break;
                        }
                        cb(Event::DownloadDataReceived(&buf[..n]))?;
                    }

                    downloaded_so_far
                } else {
                    let file_info = partial.metadata()?;
                    file_info.len()
                }
            } else {
                0
            };

            // TODO: blocking call
            let mut possible_partial = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
                .context("error opening file for download")?;

            possible_partial.seek(SeekFrom::End(0))?;

            (possible_partial, downloaded_so_far)
        } else {
            (
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(path)
                    .context("error creating file for download")?,
                0,
            )
        };

        let file = RefCell::new(file);

        // TODO: the sync callback will stall the async runtime if IO calls block, which is OS dependent. Rearrange.
        self.download(url, resume_from, timeout, &|event| {
            if let Event::DownloadDataReceived(data) = event {
                file.borrow_mut()
                    .write_all(data)
                    .context("unable to write download to disk")?;
            }
            match callback {
                Some(cb) => cb(event),
                None => Ok(()),
            }
        })
        .await?;

        file.borrow_mut()
            .sync_data()
            .context("unable to sync download to disk")?;

        Ok::<(), anyhow::Error>(())
    }

    async fn download(
        self,
        url: &Url,
        resume_from: u64,
        timeout: Duration,
        callback: DownloadCallback<'_>,
    ) -> anyhow::Result<()> {
        let client = match self {
            #[cfg(feature = "reqwest-rustls-tls")]
            Self::Rustls => rustls_client(timeout)?,
            #[cfg(feature = "reqwest-native-tls")]
            Self::NativeTls => native_tls_client(timeout)?,
        };

        download(url, resume_from, callback, client).await
    }
}

#[derive(Debug, Copy, Clone)]
enum Event<'a> {
    ResumingPartialDownload,
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(&'a [u8]),
}

type DownloadCallback<'a> = &'a dyn Fn(Event<'_>) -> anyhow::Result<()>;

async fn download(
    url: &Url,
    resume_from: u64,
    callback: &dyn Fn(Event<'_>) -> anyhow::Result<()>,
    client: &Client,
) -> anyhow::Result<()> {
    // Short-circuit reqwest for the "file:" URL scheme
    // The file scheme is mostly for use by tests to mock the dist server
    if url.scheme() == "file" {
        let src = url
            .to_file_path()
            .map_err(|_| DownloadError::Message(format!("bogus file url: '{url}'")))?;
        if !src.is_file() {
            // Because some of rustup's logic depends on checking
            // the error when a downloaded file doesn't exist, make
            // the file case return the same error value as the
            // network case.
            return Err(anyhow!(DownloadError::FileNotFound));
        }

        let mut f = fs::File::open(src).context("unable to open downloaded file")?;
        Seek::seek(&mut f, SeekFrom::Start(resume_from))?;

        let mut buffer = vec![0u8; 0x10000];
        loop {
            let bytes_read = Read::read(&mut f, &mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            callback(Event::DownloadDataReceived(&buffer[0..bytes_read]))?;
        }

        return Ok(());
    }

    let mut req = client.get(url.as_str());
    if resume_from != 0 {
        req = req.header(header::RANGE, format!("bytes={resume_from}-"));
    }
    let res = req
        .send()
        .await
        .map_err(DownloadError::Reqwest)
        .context("error downloading file")?;

    // If a download is being resumed, we expect a 206 response;
    // otherwise, if the server ignored the range header,
    // an error is thrown preemptively to avoid corruption.
    let status = res.status().into();
    match (resume_from > 0, status) {
        (true, 206) | (false, 200..=299) => {}
        _ => return Err(DownloadError::HttpStatus(u32::from(status)).into()),
    }

    if let Some(len) = res.content_length() {
        let len = len + resume_from;
        callback(Event::DownloadContentLengthReceived(len))?;
    }

    let mut stream = res.bytes_stream();
    while let Some(item) = stream.next().await {
        let bytes = item.map_err(DownloadError::Reqwest)?;
        callback(Event::DownloadDataReceived(&bytes))?;
    }
    Ok(())
}

fn client_generic() -> ClientBuilder {
    Client::builder()
        // HACK: set `pool_max_idle_per_host` to `0` to avoid an issue in the underlying
        // `hyper` library that causes the `reqwest` client to hang in some cases.
        // See <https://github.com/hyperium/hyper/issues/2312> for more details.
        .pool_max_idle_per_host(0)
        .gzip(false)
        .proxy(Proxy::custom(env_proxy))
}

#[cfg(feature = "reqwest-rustls-tls")]
fn rustls_client(timeout: Duration) -> Result<&'static Client, DownloadError> {
    // If the client is already initialized, the passed timeout is ignored.
    if let Some(client) = CLIENT_RUSTLS_TLS.get() {
        return Ok(client);
    }

    let provider = Arc::new(aws_lc_rs::default_provider());
    #[cfg(not(target_os = "android"))]
    let result =
        Verifier::new_with_extra_roots(RUSTUP_TRUST_ANCHORS.iter().cloned(), provider.clone());
    #[cfg(target_os = "android")]
    let result = Verifier::new(provider.clone());
    let verifier = result.map_err(|err| {
        DownloadError::Message(format!("failed to initialize platform verifier: {err}"))
    })?;

    let mut tls_config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .dangerous() // We're using a rustls verifier, so it's okay
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let client = client_generic()
        .read_timeout(timeout)
        .use_preconfigured_tls(tls_config)
        .user_agent(REQWEST_RUSTLS_TLS_USER_AGENT)
        .build()
        .map_err(DownloadError::Reqwest)?;

    let _ = CLIENT_RUSTLS_TLS.set(client);
    // "The cell is guaranteed to contain a value when `set` returns, though not necessarily
    // the one provided."
    Ok(CLIENT_RUSTLS_TLS.get().unwrap())
}

#[cfg(feature = "reqwest-rustls-tls")]
static CLIENT_RUSTLS_TLS: OnceLock<Client> = OnceLock::new();

#[cfg(feature = "reqwest-native-tls")]
fn native_tls_client(timeout: Duration) -> Result<&'static Client, DownloadError> {
    // If the client is already initialized, the passed timeout is ignored.
    if let Some(client) = CLIENT_NATIVE_TLS.get() {
        return Ok(client);
    }

    let client = client_generic()
        .read_timeout(timeout)
        .user_agent(REQWEST_DEFAULT_TLS_USER_AGENT)
        .build()
        .map_err(DownloadError::Reqwest)?;

    let _ = CLIENT_NATIVE_TLS.set(client);
    // "The cell is guaranteed to contain a value when `set` returns, though not necessarily
    // the one provided."
    Ok(CLIENT_NATIVE_TLS.get().unwrap())
}

#[cfg(feature = "reqwest-native-tls")]
static CLIENT_NATIVE_TLS: OnceLock<Client> = OnceLock::new();

fn env_proxy(url: &Url) -> Option<Url> {
    env_proxy::for_url(url).to_url()
}

#[derive(Debug, Error)]
enum DownloadError {
    #[error("http request returned an unsuccessful status code: {0}")]
    HttpStatus(u32),
    #[error("file not found")]
    FileNotFound,
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
    #[error(transparent)]
    Reqwest(#[from] ::reqwest::Error),
}
