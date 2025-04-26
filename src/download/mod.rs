//! Easy file downloading

use std::fs;
use std::fs::OpenOptions;
use std::fs::remove_file;
<<<<<<< HEAD
use std::ops;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
=======
use std::path::Path;

use anyhow::Context;
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
#[cfg(any(
    not(feature = "curl-backend"),
    not(feature = "reqwest-rustls-tls"),
    not(feature = "reqwest-native-tls")
))]
use anyhow::anyhow;
use sha2::Sha256;
use thiserror::Error;
#[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
use tracing::info;
use tracing::{debug, warn};
use url::Url;

use crate::{
    diskio::IOPriority,
    errors::RustupError,
    process::Process,
    utils::Notification,
};

#[cfg(test)]
mod tests;

// New structure to track concurrent downloads
#[derive(Debug, Default)]
struct DownloadStats {
    active_downloads: AtomicBool,
}

// Global download stats to track concurrent downloads
static DOWNLOAD_STATS: std::sync::LazyLock<Arc<DownloadStats>> =
    std::sync::LazyLock::new(|| Arc::new(DownloadStats::default()));

/// Categorize downloads by their importance
fn determine_download_priority(url: &Url, path: &Path) -> IOPriority {
    let path_str = path.to_string_lossy();
    let url_str = url.as_str();

    // Prioritize metadata files
    if url_str.contains("channel-rust-") && url_str.ends_with(".toml") {
        debug!("Prioritizing channel metadata: {}", url_str);
        return IOPriority::Critical;
    }

    // Prioritize small index files
    if url_str.contains("/index.html") || url_str.contains("/dist/") && path_str.ends_with(".toml") {
        debug!("Prioritizing index file: {}", url_str);
        return IOPriority::Critical;
    }

    // Large documentation files are lower priority
    if path_str.contains("rust-docs") || url_str.contains("rust-docs") {
        debug!("Setting docs to background priority: {}", url_str);
        return IOPriority::Background;
    }

    // Standard components are normal priority
    IOPriority::Normal
}

pub(crate) async fn download_file(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    notify_handler: &dyn Fn(Notification<'_>),
    process: &Process,
) -> anyhow::Result<()> {
    download_file_with_resume(url, path, hasher, false, &notify_handler, process).await
}

pub(crate) async fn download_file_with_resume(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    resume_from_partial: bool,
    notify_handler: &dyn Fn(Notification<'_>),
    process: &Process,
) -> anyhow::Result<()> {
    use crate::download::DownloadError as DEK;

    // Track this download in our concurrent download stats
    let download_stats = DOWNLOAD_STATS.clone();
    download_stats.active_downloads.store(true, Ordering::SeqCst);

    // Set priority for this download
    let priority = determine_download_priority(url, path);

    // Log the priority we've assigned to this download
    debug!(
        "Downloading with {:?} priority: {} -> {}",
        priority,
        url,
        path.display()
    );

    let result = match download_file_(
        url,
        path,
        hasher,
        resume_from_partial,
        notify_handler,
        process,
        priority,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(e) => {
            if e.downcast_ref::<std::io::Error>().is_some() {
                return Err(e);
            }
            let is_client_error = match e.downcast_ref::<DEK>() {
                // Specifically treat the bad partial range error as not our
                // fault in case it was something odd which happened.
                Some(DEK::HttpStatus(416)) => false,
                Some(DEK::HttpStatus(400..=499)) | Some(DEK::FileNotFound) => true,
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
    };

    // Mark this download as complete
    download_stats.active_downloads.store(false, Ordering::SeqCst);

    result
}

async fn download_file_(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    resume_from_partial: bool,
    notify_handler: &dyn Fn(Notification<'_>),
    process: &Process,
<<<<<<< HEAD
    priority: IOPriority,
) -> Result<()> {
=======
) -> anyhow::Result<()> {
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
    #[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
    use crate::download::{Backend, Event, TlsBackend};
    use sha2::Digest;
    use std::rc::Rc;
    use std::cell::RefCell;

    notify_handler(Notification::DownloadingFile(url, path));

    // Make hasher thread-safe by using a Rc<RefCell> in this scope
    let hasher = Rc::new(RefCell::new(hasher));

    // This callback will write the download to disk and optionally
    // hash the contents, then forward the notification up the stack
    let hasher_clone = hasher.clone();
    let callback: &dyn Fn(Event<'_>) -> anyhow::Result<()> = &|msg| {
        if let Event::DownloadDataReceived(data) = msg {
            if let Some(h) = hasher_clone.borrow_mut().as_mut() {
                h.update(data);
            }
        }
        match msg {
            Event::DownloadContentLengthReceived(len) => {
                notify_handler(Notification::DownloadContentLengthReceived(len));
            }
            Event::DownloadDataReceived(data) => {
                notify_handler(Notification::DownloadDataReceived(data));
            }
            Event::ResumingPartialDownload => {
                notify_handler(Notification::ResumingPartialDownload);
            }
        }
        Ok(())
    };

    // Download the file
    // Keep the curl env var around for a bit
    let use_curl_backend = process.var_os("RUSTUP_USE_CURL").map(|it| it != "0");
    if use_curl_backend == Some(true) {
        warn!(
            "RUSTUP_USE_CURL is set; the curl backend is deprecated, please file an issue if the \
            default download backend does not work for your use case"
        );
    }
    let use_rustls = process.var_os("RUSTUP_USE_RUSTLS").map(|it| it != "0");
    let backend = match (use_curl_backend, use_rustls) {
        #[cfg(not(feature = "reqwest-rustls-tls"))]
        (_, Some(true)) => {
            return Err(anyhow!(
                "RUSTUP_USE_RUSTLS is set, but this rustup distribution was not built with the reqwest-rustls-tls feature"
            ));
        }
        #[cfg(not(feature = "reqwest-native-tls"))]
        (_, Some(false)) => {
            return Err(anyhow!(
                "RUSTUP_USE_RUSTLS is set to false, but this rustup distribution was not built with the reqwest-native-tls feature"
            ));
        }
        #[cfg(not(feature = "curl-backend"))]
        (Some(true), _) => {
            return Err(anyhow!(
                "RUSTUP_USE_CURL is set, but this rustup distribution was not built with the curl-backend feature"
            ));
        }
        #[cfg(feature = "curl-backend")]
        (Some(true), None) => Backend::Curl,
        #[cfg(feature = "reqwest-native-tls")]
        (_, Some(false)) => {
            if use_curl_backend == Some(true) {
                info!(
                    "RUSTUP_USE_CURL is set and RUSTUP_USE_RUSTLS is set to off, using reqwest with native-tls"
                );
            }
            Backend::Reqwest(TlsBackend::NativeTls)
        }
        #[cfg(feature = "reqwest-rustls-tls")]
        _ => {
            if use_curl_backend == Some(true) {
                info!(
                    "both RUSTUP_USE_CURL and RUSTUP_USE_RUSTLS are set, using reqwest with rustls"
                );
            }
            Backend::Reqwest(TlsBackend::Rustls)
        }
        #[cfg(all(not(feature = "reqwest-rustls-tls"), feature = "reqwest-native-tls"))]
        _ => Backend::Reqwest(TlsBackend::NativeTls),
        #[cfg(all(
            not(feature = "reqwest-rustls-tls"),
            not(feature = "reqwest-native-tls"),
            feature = "curl-backend"
        ))]
        _ => Backend::Curl,
    };

    notify_handler(match backend {
        #[cfg(feature = "curl-backend")]
        Backend::Curl => Notification::UsingCurl,
        #[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
        Backend::Reqwest(_) => Notification::UsingReqwest,
    });

    let res = backend
        .download_to_path(url, path, resume_from_partial, Some(callback), priority)
        .await;

    notify_handler(Notification::DownloadFinished);
    res
}

/// User agent header value for HTTP request.
/// See: https://github.com/rust-lang/rustup/issues/2860.
#[cfg(feature = "curl-backend")]
const CURL_USER_AGENT: &str = concat!("rustup/", env!("CARGO_PKG_VERSION"), " (curl)");

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
    #[cfg(feature = "curl-backend")]
    Curl,
    #[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
    Reqwest(TlsBackend),
}

impl Backend {
    async fn download_to_path(
        self,
        url: &Url,
        path: &Path,
        resume_from_partial: bool,
        callback: Option<DownloadCallback<'_>>,
<<<<<<< HEAD
        priority: IOPriority,
    ) -> Result<()> {
=======
    ) -> anyhow::Result<()> {
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
        let Err(err) = self
            .download_impl(url, path, resume_from_partial, callback, priority)
            .await
        else {
            return Ok(());
        };

        Err(
            if let Err(file_err) = remove_file(path).context("cleaning up cached downloads") {
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
<<<<<<< HEAD
        priority: IOPriority,
    ) -> Result<()> {
        use std::rc::Rc;
=======
    ) -> anyhow::Result<()> {
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
        use std::cell::RefCell;
        use std::io::{Read, Seek, SeekFrom, Write};

        let (file, resume_from) = if resume_from_partial {
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

        let file = Rc::new(RefCell::<std::fs::File>::new(file));

        let file_writer = {
            let file_clone = file.clone();
            move |data: &[u8]| -> Result<()> {
                if priority == IOPriority::Background && data.len() > 1_000_000 {
                    debug!("Processing large background priority write: {} bytes", data.len());
                }

                file_clone
                    .borrow_mut()
                    .write_all(data)
                    .context("unable to write download to disk")
            }
        };

        self.download(url, resume_from, &|event| {
            if let Event::DownloadDataReceived(data) = event {
                file_writer(data)?;
            }
            match callback {
                Some(cb) => cb(event),
                None => Ok(()),
            }
        }, priority)
        .await?;

        file.borrow_mut()
            .sync_data()
            .context("unable to sync download to disk")?;

        Ok::<(), anyhow::Error>(())
    }

    #[cfg_attr(
        all(
            not(feature = "curl-backend"),
            not(feature = "reqwest-rustls-tls"),
            not(feature = "reqwest-native-tls")
        ),
        allow(unused_variables)
    )]
    async fn download(
        self,
        url: &Url,
        resume_from: u64,
        callback: DownloadCallback<'_>,
<<<<<<< HEAD
        priority: IOPriority,
    ) -> Result<()> {
=======
    ) -> anyhow::Result<()> {
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
        match self {
            #[cfg(feature = "curl-backend")]
            Self::Curl => curl::download(url, resume_from, callback, priority),
            #[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
            Self::Reqwest(tls) => tls.download(url, resume_from, callback, priority).await,
        }
    }
}

#[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
#[derive(Debug, Copy, Clone)]
enum TlsBackend {
    #[cfg(feature = "reqwest-rustls-tls")]
    Rustls,
    #[cfg(feature = "reqwest-native-tls")]
    NativeTls,
}

#[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
impl TlsBackend {
    async fn download(
        self,
        url: &Url,
        resume_from: u64,
        callback: DownloadCallback<'_>,
<<<<<<< HEAD
        priority: IOPriority,
    ) -> Result<()> {
=======
    ) -> anyhow::Result<()> {
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
        let client = match self {
            #[cfg(feature = "reqwest-rustls-tls")]
            Self::Rustls => &reqwest_be::CLIENT_RUSTLS_TLS,
            #[cfg(feature = "reqwest-native-tls")]
            Self::NativeTls => &reqwest_be::CLIENT_NATIVE_TLS,
        };
        reqwest_be::download(url, resume_from, callback, client, priority).await
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

/// Download via libcurl; encrypt with the native (or OpenSSl) TLS
/// stack via libcurl
#[cfg(feature = "curl-backend")]
mod curl {
    use std::cell::RefCell;
    use std::str;
    use std::time::Duration;

    use anyhow::{Context, Result};
    use curl::easy::Easy;
    use url::Url;

    use super::{DownloadError, Event, IOPriority};

    pub(super) fn download(
        url: &Url,
        resume_from: u64,
        callback: &dyn Fn(Event<'_>) -> Result<()>,
        priority: IOPriority,
    ) -> Result<()> {
        thread_local!(static EASY: RefCell<Easy> = RefCell::new(Easy::new()));
        EASY.with(|handle| {
            let mut handle = handle.borrow_mut();

            handle.url(url.as_ref())?;
            handle.follow_location(true)?;
            handle.useragent(super::CURL_USER_AGENT)?;

            if resume_from > 0 {
                handle.resume_from(resume_from)?;
            } else {
                let _ = handle.resume_from(0);
            }

            handle.connect_timeout(Duration::new(30, 0))?;

            {
                let cberr = RefCell::new(None);
                let mut transfer = handle.transfer();

                transfer.write_function(|data| {
                    match callback(Event::DownloadDataReceived(data)) {
                        Ok(()) => Ok(data.len()),
                        Err(e) => {
                            *cberr.borrow_mut() = Some(e);
                            Ok(0)
                        }
                    }
                })?;

                transfer.header_function(|header| {
                    if let Ok(data) = str::from_utf8(header) {
                        let prefix = "content-length: ";
                        if data.to_ascii_lowercase().starts_with(prefix) {
                            if let Ok(s) = data[prefix.len()..].trim().parse::<u64>() {
                                let msg = Event::DownloadContentLengthReceived(s + resume_from);
                                match callback(msg) {
                                    Ok(()) => (),
                                    Err(e) => {
                                        *cberr.borrow_mut() = Some(e);
                                        return false;
                                    }
                                }
                            }
                        }
                    }
                    true
                })?;

                transfer.perform().or_else(|e| {
                    match cberr.borrow_mut().take() {
                        Some(cberr) => Err(cberr),
                        None => {
                            if e.is_file_couldnt_read_file() {
                                Err(e).context(DownloadError::FileNotFound)
                            } else {
                                Err(e).context("error during download")?
                            }
                        }
                    }
                })?;
            }

            let code = handle.response_code()?;
            match code {
                0 | 200..=299 => {}
                _ => {
                    return Err(DownloadError::HttpStatus(code).into());
                }
            };

            Ok(())
        })
    }
}

#[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
mod reqwest_be {
    use std::io;
    #[cfg(feature = "reqwest-rustls-tls")]
    use std::sync::Arc;
    #[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
    use std::sync::LazyLock;
    use std::time::Duration;
<<<<<<< HEAD
    use anyhow::{Context, Result, anyhow};
=======

    use anyhow::{Context, anyhow};
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
    use reqwest::{Client, ClientBuilder, Proxy, Response, header};
    #[cfg(feature = "reqwest-rustls-tls")]
    use rustls::crypto::aws_lc_rs;
    #[cfg(feature = "reqwest-rustls-tls")]
    use rustls_platform_verifier::BuilderVerifierExt;
    use tokio::time::sleep;
    use tokio_stream::StreamExt;
<<<<<<< HEAD
    use tracing::{debug, info};
=======
    use tracing::error;
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
    use url::Url;
    use super::{DownloadError, Event, IOPriority};

    pub(super) async fn download(
        url: &Url,
        resume_from: u64,
        callback: &dyn Fn(Event<'_>) -> anyhow::Result<()>,
        client: &Client,
<<<<<<< HEAD
        priority: IOPriority,
    ) -> Result<()> {
=======
    ) -> anyhow::Result<()> {
>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
        // Short-circuit reqwest for the "file:" URL scheme
        if download_from_file_url(url, resume_from, callback)? {
            return Ok(());
        }

        // Adjust request timeouts based on priority
        let timeout = match priority {
            IOPriority::Critical => Duration::from_secs(20),  // Shorter timeout for critical files
            IOPriority::Normal => Duration::from_secs(30),    // Default timeout
            IOPriority::Background => Duration::from_secs(60), // Longer timeout for background files
        };

        // Add some yield points for low-priority downloads to prevent them from blocking high-priority ones
        if priority == IOPriority::Background {
            // Small delay for background transfers to let critical ones go first
            sleep(Duration::from_millis(50)).await;
        }

        let res = request(url, resume_from, client, timeout)
            .await
<<<<<<< HEAD
            .context("failed to make network request")?;
            
=======
            .inspect_err(|error| error!(?error, "failed to download file"))
            .context("error downloading file")?;

>>>>>>> b79969b8d08825c543ea2cfe9732be87be85763c
        if !res.status().is_success() {
            let code: u16 = res.status().into();
            return Err(anyhow!(DownloadError::HttpStatus(u32::from(code))));
        }

        if let Some(len) = res.content_length() {
            let len = len + resume_from;
            callback(Event::DownloadContentLengthReceived(len))?;
            
            // Log download size based on priority
            match priority {
                IOPriority::Critical => debug!("Critical download size: {}KB", len/1024),
                IOPriority::Normal => debug!("Normal download size: {}KB", len/1024),
                IOPriority::Background => debug!("Background download size: {}KB", len/1024),
            }
        }

        let mut stream = res.bytes_stream();
        let mut total_bytes_received = 0;
        let mut chunk_counter = 0;
        let yield_frequency = match priority {
            IOPriority::Critical => 500,     // Almost never yield for critical downloads
            IOPriority::Normal => 100,       // Occasionally yield
            IOPriority::Background => 10,    // Frequently yield
        };
        
        while let Some(item) = stream.next().await {
            let bytes = item?;
            total_bytes_received += bytes.len();
            callback(Event::DownloadDataReceived(&bytes))?;
            
            // For background downloads, occasionally yield to let other tasks run
            chunk_counter += 1;
            if priority != IOPriority::Critical && chunk_counter % yield_frequency == 0 {
                // Give other tasks a chance to run - especially higher priority ones
                sleep(Duration::from_millis(1)).await;
            }
        }
        
        debug!("Downloaded {} bytes with {:?} priority", total_bytes_received, priority);
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
            .read_timeout(Duration::from_secs(30))
            // Allow for more concurrent connections
            .pool_max_idle_per_host(50)
    }

    #[cfg(feature = "reqwest-rustls-tls")]
    pub(super) static CLIENT_RUSTLS_TLS: LazyLock<Client> = LazyLock::new(|| {
        let mut tls_config =
            rustls::ClientConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_platform_verifier()
                .with_no_client_auth();
        tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let catcher = || {
            client_generic()
                .use_preconfigured_tls(tls_config)
                .user_agent(super::REQWEST_RUSTLS_TLS_USER_AGENT)
                .build()
        };

        catcher().unwrap()
    });

    #[cfg(feature = "reqwest-native-tls")]
    pub(super) static CLIENT_NATIVE_TLS: LazyLock<Client> = LazyLock::new(|| {
        let catcher = || {
            client_generic()
                .user_agent(super::REQWEST_DEFAULT_TLS_USER_AGENT)
                .build()
        };

        catcher().unwrap()
    });

    fn env_proxy(url: &Url) -> Option<Url> {
        env_proxy::for_url(url).to_url()
    }

    async fn request(
        url: &Url,
        resume_from: u64,
        client: &Client,
        timeout: Duration,
    ) -> Result<Response, DownloadError> {
        let mut req = client.get(url.as_str())
            .timeout(timeout);
            
        if resume_from != 0 {
            req = req.header(header::RANGE, format!("bytes={resume_from}-"));
        }
        
        Ok(req.send().await?)
    }

    fn download_from_file_url(
        url: &Url,
        resume_from: u64,
        callback: &dyn Fn(Event<'_>) -> anyhow::Result<()>,
    ) -> anyhow::Result<bool> {
        use std::fs;

        if url.scheme() == "file" {
            let src = url
                .to_file_path()
                .map_err(|_| DownloadError::Message(format!("bogus file url: '{url}'")))?;
            if !src.is_file() {
                return Err(anyhow!(DownloadError::FileNotFound));
            }

            let mut f = fs::File::open(src).context("unable to open downloaded file")?;
            io::Seek::seek(&mut f, io::SeekFrom::Start(resume_from))?;

            let mut buffer = vec![0u8; 0x10000];
            loop {
                let bytes_read = io::Read::read(&mut f, &mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                callback(Event::DownloadDataReceived(&buffer[0..bytes_read]))?;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }
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
    IoError(#[from] std::io::Error),
    #[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
    #[error(transparent)]
    Reqwest(#[from] ::reqwest::Error),
    #[cfg(feature = "curl-backend")]
    #[error(transparent)]
    CurlError(#[from] ::curl::Error),
}
