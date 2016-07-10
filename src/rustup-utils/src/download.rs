//! Easy file downloading

use errors::*;
use notifications::Notification;
use sha2::Sha256;
use std::env;
use std::path::Path;
use url::Url;

pub fn download_file(url: &Url,
                     path: &Path,
                     hasher: Option<&mut Sha256>,
                     notify_handler: &Fn(Notification))
                     -> Result<()> {
    if env::var_os("RUSTUP_USE_HYPER").is_some() {
        self::hyper::download_file(url, path, hasher, notify_handler)
    } else {
        self::curl::download_file(url, path, hasher, notify_handler)
    }
}


/// Download via libcurl; encrypt with the native (or OpenSSl) TLS
/// stack via libcurl
mod curl {
    use curl::easy::Easy;
    use errors::*;
    use notifications::Notification;
    use sha2::{Sha256, Digest};
    use std::cell::RefCell;
    use std::fs;
    use std::path::Path;
    use std::str;
    use std::time::Duration;
    use url::Url;

    pub fn download_file(url: &Url,
                         path: &Path,
                         mut hasher: Option<&mut Sha256>,
                         notify_handler: &Fn(Notification))
                         -> Result<()> {
        use notifications::Notification;
        use std::io::Write;

        let mut file = try!(fs::File::create(&path).chain_err(
            || "error creating file for download"));

        // Fetch either a cached libcurl handle (which will preserve open
        // connections) or create a new one if it isn't listed.
        //
        // Once we've acquired it, reset the lifetime from 'static to our local
        // scope.
        thread_local!(static EASY: RefCell<Easy> = RefCell::new(Easy::new()));
        EASY.with(|handle| {
            let mut handle = handle.borrow_mut();

            try!(handle.url(&url.to_string()).chain_err(|| "failed to set url"));
            try!(handle.follow_location(true).chain_err(|| "failed to set follow redirects"));

            // Take at most 30s to connect
            try!(handle.connect_timeout(Duration::new(30, 0)).chain_err(|| "failed to set connect timeout"));

            // Fail if less than 10 bytes are transferred every 30 seconds
            try!(handle.low_speed_limit(10).chain_err(|| "failed to set low speed limit"));
            try!(handle.low_speed_time(Duration::new(30, 0)).chain_err(|| "failed to set low speed time"));

            {
                let fserr = RefCell::new(None);
                let mut transfer = handle.transfer();

                // Data callback for libcurl which is called with data that's
                // downloaded. We just feed it into our hasher and also write it out
                // to disk.
                try!(transfer.write_function(|data| {
                    if let Some(ref mut h) = hasher {
                        h.input(data);
                    }
                    notify_handler(Notification::DownloadDataReceived(data.len()));
                    match file.write_all(data) {
                        Ok(()) => Ok(data.len()),
                        Err(e) => {
                            *fserr.borrow_mut() = Some(e);
                            Ok(0)
                        }
                    }
                }).chain_err(|| "failed to set write"));

                // Listen for headers and parse out a `Content-Length` if it comes
                // so we know how much we're downloading.
                try!(transfer.header_function(|header| {
                    if let Ok(data) = str::from_utf8(header) {
                        let prefix = "Content-Length: ";
                        if data.starts_with(prefix) {
                            if let Ok(s) = data[prefix.len()..].trim().parse() {
                                let msg = Notification::DownloadContentLengthReceived(s);
                                notify_handler(msg);
                            }
                        }
                    }
                    true
                }).chain_err(|| "failed to set header"));

                // If an error happens check to see if we had a filesystem error up
                // in `fserr`, but we always want to punt it up.
                try!(transfer.perform().or_else(|e| {
                    match fserr.borrow_mut().take() {
                        Some(fs) => Err(fs).chain_err(|| ErrorKind::HttpError(e)),
                        None => Err(ErrorKind::HttpError(e).into())
                    }
                }));
            }

            // If we didn't get a 200 or 0 ("OK" for files) then return an error
            let code = try!(handle.response_code().chain_err(|| "failed to get response code"));
            if code != 200 && code != 0 {
                return Err(ErrorKind::HttpStatus(code).into());
            }

            notify_handler(Notification::DownloadFinished);
            Ok(())
        })
    }
}

/// Download via hyper; encrypt with the native (or OpenSSl) TLS
/// stack via native-tls
mod hyper {
    use hyper;
    use notifications::Notification;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io;
    use std::path::Path;
    use std::time::Duration;
    use url::Url;
    use errors::*;

    fn proxy_from_env(url: &Url) -> Option<(String, u16)> {
        use std::env::var_os;

        let mut maybe_https_proxy = var_os("https_proxy").map(|ref v| v.to_str().unwrap_or("").to_string());
        if maybe_https_proxy.is_none() {
            maybe_https_proxy = var_os("HTTPS_PROXY").map(|ref v| v.to_str().unwrap_or("").to_string());
        }
        let maybe_http_proxy = var_os("http_proxy").map(|ref v| v.to_str().unwrap_or("").to_string());
        let mut maybe_all_proxy = var_os("all_proxy").map(|ref v| v.to_str().unwrap_or("").to_string());
        if maybe_all_proxy.is_none() {
            maybe_all_proxy = var_os("ALL_PROXY").map(|ref v| v.to_str().unwrap_or("").to_string());
        }
        if let Some(url_value) = match url.scheme() {
                                     "https" => maybe_https_proxy.or(maybe_http_proxy.or(maybe_all_proxy)),
                                     "http" => maybe_http_proxy.or(maybe_all_proxy),
                                     _ => maybe_all_proxy,
                                 } {
            if let Ok(proxy_url) = Url::parse(&url_value) {
                if let Some(host) = proxy_url.host_str() {
                    let port = proxy_url.port().unwrap_or(8080);
                    return Some((host.to_string(), port));
                }
            }
        }
        None
    }

    pub fn download_file(url: &Url,
                         path: &Path,
                         mut hasher: Option<&mut Sha256>,
                         notify_handler: &Fn(Notification))
                         -> Result<()> {

        // Short-circuit hyper for the "file:" URL scheme
        if try!(download_from_file_url(url, path, &mut hasher)) {
            return Ok(());
        }

        use hyper::client::{Client, ProxyConfig};
        use hyper::error::Result as HyperResult;
        use hyper::header::ContentLength;
        use hyper::net::{SslClient, NetworkStream, HttpsConnector};
        use native_tls;
        use std::io::Result as IoResult;
        use std::io::{Read, Write};
        use std::net::{SocketAddr, Shutdown};
        use std::sync::{Arc, Mutex};

        // The Hyper HTTP client
        let client;

        let maybe_proxy = proxy_from_env(url);
        if url.scheme() == "https" {

            // All the following is adapter code to use native_tls with hyper.

            struct NativeSslClient;

            impl<T: NetworkStream + Send + Clone> SslClient<T> for NativeSslClient {
                type Stream = NativeSslStream<T>;

                fn wrap_client(&self, stream: T, host: &str) -> HyperResult<Self::Stream> {
                    use native_tls::ClientBuilder as TlsClientBuilder;
                    use hyper::error::Error as HyperError;

                    let mut ssl_builder = try!(TlsClientBuilder::new()
                                               .map_err(|e| HyperError::Ssl(Box::new(e))));
                    let ssl_stream = try!(ssl_builder.handshake(host, stream)
                                          .map_err(|e| HyperError::Ssl(Box::new(e))));

                    Ok(NativeSslStream(Arc::new(Mutex::new(ssl_stream))))
                }
            }

            #[derive(Clone)]
            struct NativeSslStream<T>(Arc<Mutex<native_tls::TlsStream<T>>>);

            #[derive(Debug)]
            struct NativeSslPoisonError;

            impl ::std::error::Error for NativeSslPoisonError {
                fn description(&self) -> &str { "mutex poisoned during TLS operation" }
            }

            impl ::std::fmt::Display for NativeSslPoisonError {
                fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::result::Result<(), ::std::fmt::Error> {
                    f.write_str(::std::error::Error::description(self))
                }
            }

            impl<T> NetworkStream for NativeSslStream<T>
                where T: NetworkStream
            {
                fn peer_addr(&mut self) -> IoResult<SocketAddr> {
                    self.0.lock()
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
                        .and_then(|mut t| t.get_mut().peer_addr())
                }
                fn set_read_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
                    self.0.lock()
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
                        .and_then(|t| t.get_ref().set_read_timeout(dur))
                }
                fn set_write_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
                    self.0.lock()
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
                        .and_then(|t| t.get_ref().set_write_timeout(dur))
                }
                fn close(&mut self, how: Shutdown) -> IoResult<()> {
                    self.0.lock()
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
                        .and_then(|mut t| t.get_mut().close(how))
                }
            }

            impl<T> Read for NativeSslStream<T>
                where T: Read + Write
            {
                fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
                    self.0.lock()
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
                        .and_then(|mut t| t.read(buf))
                }
            }

            impl<T> Write for NativeSslStream<T>
                where T: Read + Write
            {
                fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
                    self.0.lock()
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
                        .and_then(|mut t| t.write(buf))
                }
                fn flush(&mut self) -> IoResult<()> {
                    self.0.lock()
                        .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
                        .and_then(|mut t| t.flush())
                }
            }

            maybe_init_certs();

            if maybe_proxy.is_none() {
                // Connect with hyper + native_tls
                client = Client::with_connector(HttpsConnector::new(NativeSslClient));
            } else {
                let proxy_host_port = maybe_proxy.unwrap();
                client = Client::with_proxy_config(ProxyConfig(proxy_host_port.0, proxy_host_port.1, NativeSslClient));
            }
        } else if url.scheme() == "http" {
            if maybe_proxy.is_none() {
                client = Client::new();
            } else {
                let proxy_host_port = maybe_proxy.unwrap();
                client = Client::with_http_proxy(proxy_host_port.0, proxy_host_port.1);
            }
        } else {
            return Err(format!("unsupported URL scheme: '{}'", url.scheme()).into());
        }

        let mut res = try!(client.get(url.clone()).send()
                           .chain_err(|| "failed to make network request"));
        if res.status != hyper::Ok {
            return Err(ErrorKind::HttpStatus(res.status.to_u16() as u32).into());
        }

        let buffer_size = 0x10000;
        let mut buffer = vec![0u8; buffer_size];

        let mut file = try!(fs::File::create(path).chain_err(
            || "error creating file for download"));

        if let Some(len) = res.headers.get::<ContentLength>().cloned() {
            notify_handler(Notification::DownloadContentLengthReceived(len.0));
        }

        loop {
            let bytes_read = try!(io::Read::read(&mut res, &mut buffer)
                                  .chain_err(|| "error reading from socket"));

            if bytes_read != 0 {
                if let Some(ref mut h) = hasher {
                    h.input(&buffer[0..bytes_read]);
                }
                try!(io::Write::write_all(&mut file, &mut buffer[0..bytes_read])
                     .chain_err(|| "unable to write download to disk"));
                notify_handler(Notification::DownloadDataReceived(bytes_read));
            } else {
                try!(file.sync_data().chain_err(|| "unable to sync download to disk"));
                notify_handler(Notification::DownloadFinished);
                return Ok(());
            }
        }
    }

    // Tell our statically-linked OpenSSL where to find root certs
    // cc https://github.com/alexcrichton/git2-rs/blob/master/libgit2-sys/lib.rs#L1267
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn maybe_init_certs() {
        use std::sync::{Once, ONCE_INIT};
        static INIT: Once = ONCE_INIT;
        INIT.call_once(|| {
            ::openssl_sys::probe::init_ssl_cert_env_vars();
        });
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn maybe_init_certs() { }

    fn download_from_file_url(url: &Url,
                              path: &Path,
                              hasher: &mut Option<&mut Sha256>)
                              -> Result<bool> {
        use raw::is_file;

        // The file scheme is mostly for use by tests to mock the dist server
        if url.scheme() == "file" {
            let src = try!(url.to_file_path()
                           .map_err(|_| Error::from(format!("bogus file url: '{}'", url))));
            if !is_file(&src) {
                // Because some of multirust's logic depends on checking
                // the error when a downloaded file doesn't exist, make
                // the file case return the same error value as the
                // network case.
                return Err(ErrorKind::HttpStatus(hyper::status::StatusCode::NotFound.to_u16() as u32).into());
            }
            try!(fs::copy(&src, path).chain_err(|| "failure copying file"));

            if let Some(ref mut h) = *hasher {
                let ref mut f = try!(fs::File::open(path)
                                     .chain_err(|| "unable to open downloaded file"));

                let ref mut buffer = vec![0u8; 0x10000];
                loop {
                    let bytes_read = try!(io::Read::read(f, buffer)
                                          .chain_err(|| "unable to read downloaded file"));
                    if bytes_read == 0 { break }
                    h.input(&buffer[0..bytes_read]);
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }
}
