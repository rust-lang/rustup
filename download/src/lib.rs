//! Easy file downloading
#![deny(rust_2018_idioms)]

use std::path::Path;

use anyhow::Context;
pub use anyhow::Result;
use url::Url;

mod errors;
pub use crate::errors::*;

#[derive(Debug, Copy, Clone)]
pub enum Backend {
    Curl,
    Reqwest(TlsBackend),
}

#[derive(Debug, Copy, Clone)]
pub enum TlsBackend {
    Rustls,
    Default,
}

#[derive(Debug, Copy, Clone)]
pub enum Event<'a> {
    ResumingPartialDownload,
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(&'a [u8]),
}

fn download_with_backend(
    backend: Backend,
    url: &Url,
    resume_from: u64,
    callback: &dyn Fn(Event<'_>) -> Result<()>,
) -> Result<()> {
    match backend {
        Backend::Curl => curl::download(url, resume_from, callback),
        Backend::Reqwest(tls) => reqwest_be::download(url, resume_from, callback, tls),
    }
}

pub fn download_to_path_with_backend(
    backend: Backend,
    url: &Url,
    path: &Path,
    resume_from_partial: bool,
    callback: Option<&dyn Fn(Event<'_>) -> Result<()>>,
) -> Result<()> {
    use std::cell::RefCell;
    use std::fs::remove_file;
    use std::fs::OpenOptions;
    use std::io::{Read, Seek, SeekFrom, Write};

    || -> Result<()> {
        let (file, resume_from) = if resume_from_partial {
            let possible_partial = OpenOptions::new().read(true).open(&path);

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
                .open(&path)
                .context("error opening file for download")?;

            possible_partial.seek(SeekFrom::End(0))?;

            (possible_partial, downloaded_so_far)
        } else {
            (
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&path)
                    .context("error creating file for download")?,
                0,
            )
        };

        let file = RefCell::new(file);

        download_with_backend(backend, url, resume_from, &|event| {
            if let Event::DownloadDataReceived(data) = event {
                file.borrow_mut()
                    .write_all(data)
                    .context("unable to write download to disk")?;
            }
            match callback {
                Some(cb) => cb(event),
                None => Ok(()),
            }
        })?;

        file.borrow_mut()
            .sync_data()
            .context("unable to sync download to disk")?;

        Ok(())
    }()
    .map_err(|e| {
        // TODO: We currently clear up the cached download on any error, should we restrict it to a subset?
        if let Err(file_err) = remove_file(path).context("cleaning up cached downloads") {
            file_err.context(e)
        } else {
            e
        }
    })
}

/// Download via libcurl; encrypt with the native (or OpenSSl) TLS
/// stack via libcurl
#[cfg(feature = "curl-backend")]
pub mod curl {
    use std::cell::RefCell;
    use std::str;
    use std::time::Duration;

    use anyhow::{Context, Result};
    use curl::easy::Easy;
    use url::Url;

    use super::Event;
    use crate::errors::*;

    pub fn download(
        url: &Url,
        resume_from: u64,
        callback: &dyn Fn(Event<'_>) -> Result<()>,
    ) -> Result<()> {
        // Fetch either a cached libcurl handle (which will preserve open
        // connections) or create a new one if it isn't listed.
        //
        // Once we've acquired it, reset the lifetime from 'static to our local
        // scope.
        thread_local!(static EASY: RefCell<Easy> = RefCell::new(Easy::new()));
        EASY.with(|handle| {
            let mut handle = handle.borrow_mut();

            handle.url(&url.to_string())?;
            handle.follow_location(true)?;

            if resume_from > 0 {
                handle.resume_from(resume_from)?;
            } else {
                // an error here indicates that the range header isn't supported by underlying curl,
                // so there's nothing to "clear" - safe to ignore this error.
                let _ = handle.resume_from(0);
            }

            // Take at most 30s to connect
            handle.connect_timeout(Duration::new(30, 0))?;

            {
                let cberr = RefCell::new(None);
                let mut transfer = handle.transfer();

                // Data callback for libcurl which is called with data that's
                // downloaded. We just feed it into our hasher and also write it out
                // to disk.
                transfer.write_function(|data| {
                    match callback(Event::DownloadDataReceived(data)) {
                        Ok(()) => Ok(data.len()),
                        Err(e) => {
                            *cberr.borrow_mut() = Some(e);
                            Ok(0)
                        }
                    }
                })?;

                // Listen for headers and parse out a `Content-Length` (case-insensitive) if it
                // comes so we know how much we're downloading.
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

                // If an error happens check to see if we had a filesystem error up
                // in `cberr`, but we always want to punt it up.
                transfer.perform().or_else(|e| {
                    // If the original error was generated by one of our
                    // callbacks, return it.
                    match cberr.borrow_mut().take() {
                        Some(cberr) => Err(cberr),
                        None => {
                            // Otherwise, return the error from curl
                            if e.is_file_couldnt_read_file() {
                                Err(e).context(DownloadError::FileNotFound)
                            } else {
                                Err(e).context("error during download")?
                            }
                        }
                    }
                })?;
            }

            // If we didn't get a 20x or 0 ("OK" for files) then return an error
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

#[cfg(feature = "reqwest-backend")]
pub mod reqwest_be {
    use std::io;
    use std::time::Duration;

    use anyhow::{anyhow, Context, Result};
    use lazy_static::lazy_static;
    use reqwest::blocking::{Client, ClientBuilder, Response};
    use reqwest::{header, Proxy};
    use url::Url;

    use super::Event;
    use super::TlsBackend;
    use crate::errors::*;

    pub fn download(
        url: &Url,
        resume_from: u64,
        callback: &dyn Fn(Event<'_>) -> Result<()>,
        tls: TlsBackend,
    ) -> Result<()> {
        // Short-circuit reqwest for the "file:" URL scheme
        if download_from_file_url(url, resume_from, callback)? {
            return Ok(());
        }

        let mut res = request(url, resume_from, tls).context("failed to make network request")?;

        if !res.status().is_success() {
            let code: u16 = res.status().into();
            return Err(anyhow!(DownloadError::HttpStatus(u32::from(code))));
        }

        let buffer_size = 0x10000;
        let mut buffer = vec![0u8; buffer_size];

        if let Some(len) = res.headers().get(header::CONTENT_LENGTH) {
            // TODO possible issues during unwrap?
            let len = len.to_str().unwrap().parse::<u64>().unwrap() + resume_from;
            callback(Event::DownloadContentLengthReceived(len))?;
        }

        loop {
            let bytes_read = io::Read::read(&mut res, &mut buffer)?;

            if bytes_read != 0 {
                callback(Event::DownloadDataReceived(&buffer[0..bytes_read]))?;
            } else {
                return Ok(());
            }
        }
    }

    fn client_generic() -> ClientBuilder {
        Client::builder()
            .gzip(false)
            .proxy(Proxy::custom(env_proxy))
            .timeout(Duration::from_secs(30))
    }
    #[cfg(feature = "reqwest-rustls-tls")]
    lazy_static! {
        static ref CLIENT_RUSTLS_TLS: Client = {
            let catcher = || {
                client_generic().use_rustls_tls()
                    .build()
            };

            // woah, an unwrap?!
            // It's OK. This is the same as what is happening in curl.
            //
            // The curl::Easy::new() internally assert!s that the initialized
            // Easy is not null. Inside reqwest, the errors here would be from
            // the TLS library returning a null pointer as well.
            catcher().unwrap()
        };
    }
    #[cfg(feature = "reqwest-default-tls")]
    lazy_static! {
        static ref CLIENT_DEFAULT_TLS: Client = {
            let catcher = || {
                client_generic()
                    .build()
            };

            // woah, an unwrap?!
            // It's OK. This is the same as what is happening in curl.
            //
            // The curl::Easy::new() internally assert!s that the initialized
            // Easy is not null. Inside reqwest, the errors here would be from
            // the TLS library returning a null pointer as well.
            catcher().unwrap()
        };
    }

    fn env_proxy(url: &Url) -> Option<Url> {
        env_proxy::for_url(url).to_url()
    }

    fn request(
        url: &Url,
        resume_from: u64,
        backend: TlsBackend,
    ) -> Result<Response, DownloadError> {
        let client: &Client = match backend {
            #[cfg(feature = "reqwest-rustls-tls")]
            TlsBackend::Rustls => &CLIENT_RUSTLS_TLS,
            #[cfg(not(feature = "reqwest-rustls-tls"))]
            TlsBackend::Rustls => {
                return Err(DownloadError::BackendUnavailable("reqwest rustls"));
            }
            #[cfg(feature = "reqwest-default-tls")]
            TlsBackend::Default => &CLIENT_DEFAULT_TLS,
            #[cfg(not(feature = "reqwest-default-tls"))]
            TlsBackend::Default => {
                return Err(DownloadError::BackendUnavailable("reqwest default TLS"));
            }
        };
        let mut req = client.get(url.as_str());

        if resume_from != 0 {
            req = req.header(header::RANGE, format!("bytes={}-", resume_from));
        }

        Ok(req.send()?)
    }

    fn download_from_file_url(
        url: &Url,
        resume_from: u64,
        callback: &dyn Fn(Event<'_>) -> Result<()>,
    ) -> Result<bool> {
        use std::fs;

        // The file scheme is mostly for use by tests to mock the dist server
        if url.scheme() == "file" {
            let src = url
                .to_file_path()
                .map_err(|_| DownloadError::Message(format!("bogus file url: '{}'", url)))?;
            if !src.is_file() {
                // Because some of rustup's logic depends on checking
                // the error when a downloaded file doesn't exist, make
                // the file case return the same error value as the
                // network case.
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

#[cfg(not(feature = "curl-backend"))]
pub mod curl {

    use anyhow::{anyhow, Result};

    use super::Event;
    use crate::errors::*;
    use url::Url;

    pub fn download(
        _url: &Url,
        _resume_from: u64,
        _callback: &dyn Fn(Event<'_>) -> Result<()>,
    ) -> Result<()> {
        Err(anyhow!(DownloadError::BackendUnavailable("curl")))
    }
}

#[cfg(not(feature = "reqwest-backend"))]
pub mod reqwest_be {

    use anyhow::{anyhow, Result};

    use super::Event;
    use super::TlsBackend;
    use crate::errors::*;
    use url::Url;

    pub fn download(
        _url: &Url,
        _resume_from: u64,
        _callback: &dyn Fn(Event<'_>) -> Result<()>,
        _tls: TlsBackend,
    ) -> Result<()> {
        Err(anyhow!(DownloadError::BackendUnavailable("reqwest")))
    }
}
