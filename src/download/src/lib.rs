//! Easy file downloading

use std::path::Path;
use url::Url;

mod errors;
pub use crate::errors::*;

#[derive(Debug, Copy, Clone)]
pub enum Event<'a> {
    ResumingPartialDownload,
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(&'a [u8]),
}

pub fn download_to_path(
    url: &Url,
    path: &Path,
    resume_from_partial: bool,
    callback: Option<&dyn Fn(Event<'_>) -> Result<()>>,
) -> Result<()> {
    use std::cell::RefCell;
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
                .chain_err(|| "error opening file for download")?;

            possible_partial.seek(SeekFrom::End(0))?;

            (possible_partial, downloaded_so_far)
        } else {
            (
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&path)
                    .chain_err(|| "error creating file for download")?,
                0,
            )
        };

        let file = RefCell::new(file);

        reqwest_be::download(url, resume_from, &|event| {
            if let Event::DownloadDataReceived(data) = event {
                file.borrow_mut()
                    .write_all(data)
                    .chain_err(|| "unable to write download to disk")?;
            }
            match callback {
                Some(cb) => cb(event),
                None => Ok(()),
            }
        })?;

        file.borrow_mut()
            .sync_data()
            .chain_err(|| "unable to sync download to disk")?;

        Ok(())
    }()
    .map_err(|e| {
        // TODO is there any point clearing up here? What kind of errors will leave us with an unusable partial?
        e
    })
}

pub mod reqwest_be {
    use super::Event;
    use crate::errors::*;
    use lazy_static::lazy_static;
    use reqwest::{header, Client, Proxy, Response};
    use std::io;
    use std::time::Duration;
    use url::Url;

    pub fn download(
        url: &Url,
        resume_from: u64,
        callback: &dyn Fn(Event<'_>) -> Result<()>,
    ) -> Result<()> {
        // Short-circuit reqwest for the "file:" URL scheme
        if download_from_file_url(url, resume_from, callback)? {
            return Ok(());
        }

        let mut res = request(url, resume_from).chain_err(|| "failed to make network request")?;

        if !res.status().is_success() {
            let code: u16 = res.status().into();
            return Err(ErrorKind::HttpStatus(code as u32).into());
        }

        let buffer_size = 0x10000;
        let mut buffer = vec![0u8; buffer_size];

        if let Some(len) = res.headers().get(header::CONTENT_LENGTH) {
            // TODO possible issues during unwrap?
            let len = len.to_str().unwrap().parse::<u64>().unwrap() + resume_from;
            callback(Event::DownloadContentLengthReceived(len))?;
        }

        loop {
            let bytes_read =
                io::Read::read(&mut res, &mut buffer).chain_err(|| "error reading from socket")?;

            if bytes_read != 0 {
                callback(Event::DownloadDataReceived(&buffer[0..bytes_read]))?;
            } else {
                return Ok(());
            }
        }
    }

    lazy_static! {
        static ref CLIENT: Client = {
            let catcher = || {
                Client::builder()
                    .gzip(false)
                    .proxy(Proxy::custom(env_proxy))
                    .timeout(Duration::from_secs(30))
                    .build()
            };

            // Inside reqwest, the errors here would be from the TLS library
            // returning a null pointer.
            catcher().unwrap()
        };
    }

    fn env_proxy(url: &Url) -> Option<Url> {
        ::env_proxy::for_url(url).to_url()
    }

    fn request(url: &Url, resume_from: u64) -> ::reqwest::Result<Response> {
        let mut req = CLIENT.get(url.as_str());

        if resume_from != 0 {
            req = req.header(header::RANGE, format!("bytes={}-", resume_from));
        }

        req.send()
    }

    fn download_from_file_url(
        url: &Url,
        resume_from: u64,
        callback: &dyn Fn(Event<'_>) -> Result<()>,
    ) -> Result<bool> {
        use std::fs;
        use std::io;

        // The file scheme is mostly for use by tests to mock the dist server
        if url.scheme() == "file" {
            let src = url
                .to_file_path()
                .map_err(|_| Error::from(format!("bogus file url: '{}'", url)))?;
            if !src.is_file() {
                // Because some of rustup's logic depends on checking
                // the error when a downloaded file doesn't exist, make
                // the file case return the same error value as the
                // network case.
                return Err(ErrorKind::FileNotFound.into());
            }

            let ref mut f = fs::File::open(src).chain_err(|| "unable to open downloaded file")?;
            io::Seek::seek(f, io::SeekFrom::Start(resume_from))?;

            let ref mut buffer = vec![0u8; 0x10000];
            loop {
                let bytes_read =
                    io::Read::read(f, buffer).chain_err(|| "unable to read downloaded file")?;
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
