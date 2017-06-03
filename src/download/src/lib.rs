//! Easy file downloading

#[macro_use]
extern crate error_chain;
extern crate url;

#[cfg(feature = "rustls-backend")]
#[macro_use]
extern crate lazy_static;
#[cfg(feature = "rustls-backend")]
extern crate ca_loader;

use url::Url;
use std::path::Path;

mod errors;
pub use errors::*;

#[derive(Debug, Copy, Clone)]
pub enum Backend { Curl, Hyper, Rustls }

#[derive(Debug, Copy, Clone)]
pub enum Event<'a> {
    ResumingPartialDownload,
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(&'a [u8]),
}

fn download_with_backend(backend: Backend,
                             url: &Url,
                             resume_from: u64,
                             callback: &Fn(Event) -> Result<()>)
                             -> Result<()> {
    match backend {
        Backend::Curl => curl::download(url, resume_from, callback),
        Backend::Hyper => hyper::download(url, callback),
        Backend::Rustls => rustls::download(url, callback),
    }
}

fn supports_partial_download(backend: &Backend) -> bool {
    match backend {
        &Backend::Curl => true,
        _ => false
    }
}

pub fn download_to_path_with_backend(
    backend: Backend,
    url: &Url,
    path: &Path,
    resume_from_partial: bool,
    callback: Option<&Fn(Event) -> Result<()>>)
    -> Result<()>
{
    use std::cell::RefCell;
    use std::fs::{OpenOptions};
    use std::io::{Read, Write, Seek, SeekFrom};

    || -> Result<()> {
        let (file, resume_from) = if resume_from_partial && supports_partial_download(&backend) {
            let possible_partial = OpenOptions::new()
                    .read(true)
                    .open(&path);

            let downloaded_so_far = if let Ok(mut partial) = possible_partial {
                if let Some(cb) = callback {
                    try!(cb(Event::ResumingPartialDownload));

                    let mut buf = vec![0; 32768];
                    let mut downloaded_so_far = 0;
                    loop {
                        let n = try!(partial.read(&mut buf));
                        downloaded_so_far += n as u64;
                        if n == 0 {
                            break;
                        }
                        try!(cb(Event::DownloadDataReceived(&buf[..n])));
                    }

                    downloaded_so_far
                } else {
                    let file_info = try!(partial.metadata());
                    file_info.len()
                }
            } else {
                0
            };

            let mut possible_partial =
                try!(OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&path)
                        .chain_err(|| "error opening file for download"));

            try!(possible_partial.seek(SeekFrom::End(0)));

            (possible_partial, downloaded_so_far)
        } else {
            (try!(OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&path)
                    .chain_err(|| "error creating file for download")), 0)
        };

        let file = RefCell::new(file);

        try!(download_with_backend(backend, url, resume_from, &|event| {
            if let Event::DownloadDataReceived(data) = event {
                try!(file.borrow_mut().write_all(data)
                     .chain_err(|| "unable to write download to disk"));
            }
            match callback {
                Some(cb) => cb(event),
                None => Ok(())
            }
        }));

        try!(file.borrow_mut().sync_data()
             .chain_err(|| "unable to sync download to disk"));

        Ok(())
    }().map_err(|e| {

        // TODO is there any point clearing up here? What kind of errors will leave us with an unusable partial?
        e
    })
}

/// Download via libcurl; encrypt with the native (or OpenSSl) TLS
/// stack via libcurl
#[cfg(feature = "curl-backend")]
pub mod curl {

    extern crate curl;

    use self::curl::easy::Easy;
    use errors::*;
    use std::cell::RefCell;
    use std::str;
    use std::time::Duration;
    use url::Url;
    use super::Event;

    pub fn download(url: &Url,
                    resume_from: u64,
                    callback: &Fn(Event) -> Result<()> )
                    -> Result<()> {
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

            if resume_from > 0 {
                try!(handle.resume_from(resume_from)
                    .chain_err(|| "setting the range header for download resumption"));
            } else {
                // an error here indicates that the range header isn't supported by underlying curl,
                // so there's nothing to "clear" - safe to ignore this error.
                let _ = handle.resume_from(0);
            }

            // Take at most 30s to connect
            try!(handle.connect_timeout(Duration::new(30, 0)).chain_err(|| "failed to set connect timeout"));

            {
                let cberr = RefCell::new(None);
                let mut transfer = handle.transfer();

                // Data callback for libcurl which is called with data that's
                // downloaded. We just feed it into our hasher and also write it out
                // to disk.
                try!(transfer.write_function(|data| {
                    match callback(Event::DownloadDataReceived(data)) {
                        Ok(()) => Ok(data.len()),
                        Err(e) => {
                            *cberr.borrow_mut() = Some(e);
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
                }).chain_err(|| "failed to set header"));

                // If an error happens check to see if we had a filesystem error up
                // in `cberr`, but we always want to punt it up.
                try!(transfer.perform().or_else(|e| {
                    // If the original error was generated by one of our
                    // callbacks, return it.
                    match cberr.borrow_mut().take() {
                        Some(cberr) => Err(cberr),
                        None => {
                            // Otherwise, return the error from curl
                            if e.is_file_couldnt_read_file() {
                                Err(e).chain_err(|| ErrorKind::FileNotFound)
                            } else {
                                Err(e).chain_err(|| "error during download")
                            }
                        }
                    }
                }));
            }

            // If we didn't get a 20x or 0 ("OK" for files) then return an error
            let code = try!(handle.response_code().chain_err(|| "failed to get response code"));
            match code {
                0 | 200 ... 299 => {},
                _ => { return Err(ErrorKind::HttpStatus(code).into()); }
            };

            Ok(())
        })
    }
}

/// Download via hyper; encrypt with the native (or OpenSSl) TLS
/// stack via native-tls
#[cfg(feature = "hyper-backend")]
pub mod hyper {

    extern crate hyper;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    extern crate openssl_probe;
    extern crate native_tls;

    use super::Event;
    use std::io;
    use std::time::Duration;
    use url::Url;
    use errors::*;
    use hyper_base;
    use self::hyper::error::Result as HyperResult;
    use self::hyper::net::{SslClient, NetworkStream};
    use std::io::Result as IoResult;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, Shutdown};
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::fmt::Debug;

    pub fn download(url: &Url,
                    callback: &Fn(Event) -> Result<()>)
                    -> Result<()> {
        hyper_base::download::<NativeSslClient>(url, callback)
    }

    struct NativeSslClient;

    impl hyper_base::NewSslClient for NativeSslClient {
        fn new() -> Self { NativeSslClient }
        fn maybe_init_certs() { maybe_init_certs() }
    }

    impl<T: NetworkStream + Send + Clone + Debug + Sync> SslClient<T> for NativeSslClient {
        type Stream = NativeSslStream<T>;

        fn wrap_client(&self, stream: T, host: &str) -> HyperResult<Self::Stream> {
            use self::native_tls::TlsConnector;
            use self::hyper::error::Error as HyperError;

            let builder = try!(TlsConnector::builder()
                                .map_err(|e| HyperError::Ssl(Box::new(e))));
            let cx = try!(builder.build()
                                .map_err(|e| HyperError::Ssl(Box::new(e))));
            let ssl_stream = try!(cx.connect(host, stream)
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

    impl<T> NativeSslStream<T> {
        fn lock<'a>(&'a self) -> IoResult<MutexGuard<'a, native_tls::TlsStream<T>>> {
            self.0.lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
        }
    }

    impl<T> NetworkStream for NativeSslStream<T>
        where T: NetworkStream
    {
        fn peer_addr(&mut self) -> IoResult<SocketAddr> {
            self.lock()
                .and_then(|mut t| t.get_mut().peer_addr())
        }
        fn set_read_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
            self.lock()
                .and_then(|t| t.get_ref().set_read_timeout(dur))
        }
        fn set_write_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
            self.lock()
                .and_then(|t| t.get_ref().set_write_timeout(dur))
        }
        fn close(&mut self, how: Shutdown) -> IoResult<()> {
            self.lock()
                .and_then(|mut t| t.get_mut().close(how))
        }
    }

    impl<T> Read for NativeSslStream<T>
        where T: Read + Write
    {
        fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
            self.lock()
                .and_then(|mut t| t.read(buf))
        }
    }

    impl<T> Write for NativeSslStream<T>
        where T: Read + Write
    {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            self.lock()
                .and_then(|mut t| t.write(buf))
        }
        fn flush(&mut self) -> IoResult<()> {
            self.lock()
                .and_then(|mut t| t.flush())
        }
    }

    // Tell our statically-linked OpenSSL where to find root certs
    // cc https://github.com/alexcrichton/git2-rs/blob/master/libgit2-sys/lib.rs#L1267
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    fn maybe_init_certs() {
        use std::sync::{Once, ONCE_INIT};
        static INIT: Once = ONCE_INIT;
        INIT.call_once(|| {
            openssl_probe::init_ssl_cert_env_vars();
        });
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn maybe_init_certs() { }
}

/// Download via hyper; encrypt with rustls
#[cfg(feature = "rustls-backend")]
pub mod rustls {

    extern crate hyper;
    extern crate rustls;

    use super::Event;
    use std::io;
    use std::time::Duration;
    use url::Url;
    use errors::*;
    use hyper_base;
    use self::hyper::error::Result as HyperResult;
    use self::hyper::net::{SslClient, NetworkStream};
    use self::rustls::Session;
    use std::io::Result as IoResult;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, Shutdown};
    use std::sync::{Arc, Mutex, MutexGuard};

    pub fn download(url: &Url,
                    callback: &Fn(Event) -> Result<()>)
                    -> Result<()> {
        hyper_base::download::<NativeSslClient>(url, callback)
    }

    struct NativeSslClient;

    impl hyper_base::NewSslClient for NativeSslClient {
        fn new() -> Self { NativeSslClient }
        fn maybe_init_certs() { }
    }

    impl<T: NetworkStream + Send + Clone> SslClient<T> for NativeSslClient {
        type Stream = NativeSslStream<T>;

        fn wrap_client(&self, stream: T, host: &str) -> HyperResult<Self::Stream> {
            let config = global_config();
            let tls_client = rustls::ClientSession::new(&config, host);

            Ok(NativeSslStream(Arc::new(Mutex::new((stream, tls_client)))))
        }
    }

    fn global_config() -> Arc<rustls::ClientConfig> {
        use ca_loader::{CertBundle, CertItem};
        use std::fs::File;
        use std::io::BufReader;

        lazy_static! {
            static ref CONFIG: Arc<rustls::ClientConfig> = init();
        }

        fn init() -> Arc<rustls::ClientConfig> {
            let mut config = rustls::ClientConfig::new();
            let bundle = CertBundle::new().expect("cannot initialize CA cert bundle");
            let mut added = 0;
            let mut invalid = 0;
            for cert in bundle {
                let (c_added, c_invalid) = match cert {
                    CertItem::Blob(blob) => match config.root_store.add(&blob) {
                        Ok(_) => (1, 0),
                        Err(_) => (0, 1)
                    },
                    CertItem::File(name) => {
                        if let Ok(cf) = File::open(name) {
                            let mut reader = BufReader::new(cf);
                            match config.root_store.add_pem_file(&mut reader) {
                                Ok(pair) => pair,
                                Err(_) => (0, 1)
                            }
                        } else {
                            (0, 1)
                        }
                    }
                };
                added += c_added;
                invalid += c_invalid;
            }
            if added == 0 {
                panic!("no CA certs added, {} were invalid", invalid);
            }
            Arc::new(config)
        }

        CONFIG.clone()
    }

    #[derive(Clone)]
    struct NativeSslStream<T>(Arc<Mutex<(T, rustls::ClientSession)>>);

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

    impl<T> NativeSslStream<T> {
        fn lock<'a>(&'a self) -> IoResult<MutexGuard<'a, (T, rustls::ClientSession)>> {
            self.0.lock()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, NativeSslPoisonError))
        }
    }

    impl<T> NetworkStream for NativeSslStream<T>
        where T: NetworkStream
    {
        fn peer_addr(&mut self) -> IoResult<SocketAddr> {
            self.lock()
                .and_then(|mut t| t.0.peer_addr())
        }
        fn set_read_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
            self.lock()
                .and_then(|t| t.0.set_read_timeout(dur))
        }
        fn set_write_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
            self.lock()
                .and_then(|t| t.0.set_write_timeout(dur))
        }
        fn close(&mut self, how: Shutdown) -> IoResult<()> {
            self.lock()
                .and_then(|mut t| t.0.close(how))
        }
    }

    impl<T> Read for NativeSslStream<T>
        where T: Read + Write
    {
        fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
            self.lock()
                .and_then(|mut t| {
                    let (ref mut stream, ref mut tls) = *t;
                    while tls.wants_read() {
                        match tls.read_tls(stream) {
                            Ok(_) => {
                                match tls.process_new_packets() {
                                    Ok(_) => (),
                                    Err(e) => return Err(io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))
                                }
                                while tls.wants_write() {
                                    try!(tls.write_tls(stream));
                                }
                            },
                            Err(e) => return Err(e),
                        }
                    }

                    tls.read(buf)
                })
        }
    }

    impl<T> Write for NativeSslStream<T>
        where T: Read + Write
    {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            self.lock()
                .and_then(|mut t| {
                    let (ref mut stream, ref mut tls) = *t;
                    let res = tls.write(buf);
                    while tls.wants_write() {
                        try!(tls.write_tls(stream));
                    }

                    res
                })
        }
        fn flush(&mut self) -> IoResult<()> {
            self.lock()
                .and_then(|mut t| {
                    t.0.flush()
                })
        }
    }

}

#[cfg(feature = "hyper")]
pub mod hyper_base {

    extern crate hyper;
    extern crate env_proxy;

    use super::Event;
    use std::io;
    use url::Url;
    use errors::*;
    use self::hyper::net::{SslClient, HttpStream};

    pub trait NewSslClient {
        fn new() -> Self;
        fn maybe_init_certs();
    }

    pub fn download<S>(url: &Url,
                       callback: &Fn(Event) -> Result<()>)
                       -> Result<()>
        where S: SslClient<HttpStream> + NewSslClient + Send + Sync + 'static,
    {

        // Short-circuit hyper for the "file:" URL scheme
        if try!(download_from_file_url(url, callback)) {
            return Ok(());
        }

        use self::hyper::client::{Client, ProxyConfig};
        use self::hyper::header::ContentLength;
        use self::hyper::net::{HttpsConnector};

        S::maybe_init_certs();

        // The Hyper HTTP client
        let maybe_proxy = env_proxy::for_url(url);
        let client = match url.scheme() {
            "https" => match maybe_proxy {
                None => Client::with_connector(HttpsConnector::new(S::new())),
                Some(host_port) => Client::with_proxy_config(ProxyConfig(host_port.0, host_port.1, S::new()))
            },
            "http" => match maybe_proxy {
                None => Client::new(),
                Some(host_port) => Client::with_http_proxy(host_port.0, host_port.1)
            },
            _ => return Err(format!("unsupported URL scheme: '{}'", url.scheme()).into())
        };

        let mut res = try!(client.get(url.clone()).send()
                           .chain_err(|| "failed to make network request"));
        if res.status != self::hyper::Ok {
            return Err(ErrorKind::HttpStatus(res.status.to_u16() as u32).into());
        }

        let buffer_size = 0x10000;
        let mut buffer = vec![0u8; buffer_size];

        if let Some(len) = res.headers.get::<ContentLength>().cloned() {
            try!(callback(Event::DownloadContentLengthReceived(len.0)));
        }

        loop {
            let bytes_read = try!(io::Read::read(&mut res, &mut buffer)
                                  .chain_err(|| "error reading from socket"));

            if bytes_read != 0 {
                try!(callback(Event::DownloadDataReceived(&buffer[0..bytes_read])));
            } else {
                return Ok(());
            }
        }
    }

    fn download_from_file_url(url: &Url,
                              callback: &Fn(Event) -> Result<()>)
                              -> Result<bool> {

        use std::fs;
        use std::io;

        // The file scheme is mostly for use by tests to mock the dist server
        if url.scheme() == "file" {
            let src = try!(url.to_file_path()
                           .map_err(|_| Error::from(format!("bogus file url: '{}'", url))));
            if !src.is_file() {
                // Because some of multirust's logic depends on checking
                // the error when a downloaded file doesn't exist, make
                // the file case return the same error value as the
                // network case.
                return Err(ErrorKind::FileNotFound.into());
            }

            let ref mut f = try!(fs::File::open(src)
                                 .chain_err(|| "unable to open downloaded file"));

            let ref mut buffer = vec![0u8; 0x10000];
            loop {
                let bytes_read = try!(io::Read::read(f, buffer)
                                      .chain_err(|| "unable to read downloaded file"));
                if bytes_read == 0 { break }
                try!(callback(Event::DownloadDataReceived(&buffer[0..bytes_read])));
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

}

#[cfg(not(feature = "curl-backend"))]
pub mod curl {

    use errors::*;
    use url::Url;
    use super::Event;

    pub fn download(_url: &Url,
                    _resume_from: u64,
                    _callback: &Fn(Event) -> Result<()> )
                    -> Result<()> {
        Err(ErrorKind::BackendUnavailable("curl").into())
    }
}

#[cfg(not(feature = "hyper-backend"))]
pub mod hyper {

    use errors::*;
    use url::Url;
    use super::Event;

    pub fn download(_url: &Url,
                    _callback: &Fn(Event) -> Result<()> )
                    -> Result<()> {
        Err(ErrorKind::BackendUnavailable("hyper").into())
    }
}

#[cfg(not(feature = "rustls-backend"))]
pub mod rustls {

    use errors::*;
    use url::Url;
    use super::Event;

    pub fn download(_url: &Url,
                    _callback: &Fn(Event) -> Result<()> )
                    -> Result<()> {
        Err(ErrorKind::BackendUnavailable("rustls").into())
    }
}
