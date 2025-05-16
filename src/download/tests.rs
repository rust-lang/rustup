use std::convert::Infallible;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc::{Sender, channel};
use std::thread;

use http_body_util::Full;
use hyper::Request;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use tempfile::TempDir;

#[cfg(feature = "curl-backend")]
mod curl {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use url::Url;

    use super::{serve_file, tmp_dir, write_file};
    use crate::download::{Backend, Event};

    #[tokio::test]
    async fn partially_downloaded_file_gets_resumed_from_byte_offset() {
        let tmpdir = tmp_dir();
        let from_path = tmpdir.path().join("download-source");
        write_file(&from_path, "xxx45");

        let target_path = tmpdir.path().join("downloaded");
        write_file(&target_path, "123");

        let from_url = Url::from_file_path(&from_path).unwrap();
        Backend::Curl
            .download_to_path(&from_url, &target_path, true, None)
            .await
            .expect("Test download failed");

        assert_eq!(std::fs::read_to_string(&target_path).unwrap(), "12345");
    }

    #[tokio::test]
    async fn callback_gets_all_data_as_if_the_download_happened_all_at_once() {
        let tmpdir = tmp_dir();
        let target_path = tmpdir.path().join("downloaded");
        write_file(&target_path, "123");

        let addr = serve_file(b"xxx45".to_vec());

        let from_url = format!("http://{addr}").parse().unwrap();

        let callback_partial = AtomicBool::new(false);
        let callback_len = Mutex::new(None);
        let received_in_callback = Mutex::new(Vec::new());

        Backend::Curl
            .download_to_path(
                &from_url,
                &target_path,
                true,
                Some(&|msg| {
                    match msg {
                        Event::ResumingPartialDownload => {
                            assert!(!callback_partial.load(Ordering::SeqCst));
                            callback_partial.store(true, Ordering::SeqCst);
                        }
                        Event::DownloadContentLengthReceived(len) => {
                            let mut flag = callback_len.lock().unwrap();
                            assert!(flag.is_none());
                            *flag = Some(len);
                        }
                        Event::DownloadDataReceived(data) => {
                            for b in data.iter() {
                                received_in_callback.lock().unwrap().push(*b);
                            }
                        }
                    }

                    Ok(())
                }),
            )
            .await
            .expect("Test download failed");

        assert!(callback_partial.into_inner());
        assert_eq!(*callback_len.lock().unwrap(), Some(5));
        let observed_bytes = received_in_callback.into_inner().unwrap();
        assert_eq!(observed_bytes, vec![b'1', b'2', b'3', b'4', b'5']);
        assert_eq!(std::fs::read_to_string(&target_path).unwrap(), "12345");
    }
}

#[cfg(any(feature = "reqwest-rustls-tls", feature = "reqwest-native-tls"))]
mod reqwest {
    use std::env::{remove_var, set_var};
    use std::error::Error;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{LazyLock, Mutex};
    use std::thread;
    use std::time::Duration;

    use env_proxy::for_url;
    use reqwest::{Client, Proxy};
    use url::Url;

    use super::{serve_file, tmp_dir, write_file};
    use crate::download::{Backend, Event, TlsBackend};

    static SERIALISE_TESTS: LazyLock<tokio::sync::Mutex<()>> =
        LazyLock::new(|| tokio::sync::Mutex::new(()));

    unsafe fn scrub_env() {
        unsafe {
            remove_var("http_proxy");
            remove_var("https_proxy");
            remove_var("HTTPS_PROXY");
            remove_var("ftp_proxy");
            remove_var("FTP_PROXY");
            remove_var("all_proxy");
            remove_var("ALL_PROXY");
            remove_var("no_proxy");
            remove_var("NO_PROXY");
        }
    }

    // Tests for correctly retrieving the proxy (host, port) tuple from $https_proxy
    #[tokio::test]
    async fn read_basic_proxy_params() {
        let _guard = SERIALISE_TESTS.lock().await;
        // SAFETY: We are setting environment variables when `SERIALISE_TESTS` is locked,
        // and those environment variables in question are not relevant elsewhere in the test suite.
        unsafe {
            scrub_env();
            set_var("https_proxy", "http://proxy.example.com:8080");
        }
        let u = Url::parse("https://www.example.org").ok().unwrap();
        assert_eq!(
            for_url(&u).host_port(),
            Some(("proxy.example.com".to_string(), 8080))
        );
    }

    // Tests to verify if socks feature is available and being used
    #[tokio::test]
    async fn socks_proxy_request() {
        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
        let _guard = SERIALISE_TESTS.lock().await;

        // SAFETY: We are setting environment variables when `SERIALISE_TESTS` is locked,
        // and those environment variables in question are not relevant elsewhere in the test suite.
        unsafe {
            scrub_env();
            set_var("all_proxy", "socks5://127.0.0.1:1080");
        }

        thread::spawn(move || {
            let listener = TcpListener::bind("127.0.0.1:1080").unwrap();
            let incoming = listener.incoming();
            for _ in incoming {
                CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            }
        });

        let env_proxy = |url: &Url| for_url(url).to_url();
        let url = Url::parse("http://192.168.0.1/").unwrap();

        let client = Client::builder()
            // HACK: set `pool_max_idle_per_host` to `0` to avoid an issue in the underlying
            // `hyper` library that causes the `reqwest` client to hang in some cases.
            // See <https://github.com/hyperium/hyper/issues/2312> for more details.
            .pool_max_idle_per_host(0)
            .proxy(Proxy::custom(env_proxy))
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap();
        let res = client.get(url.as_str()).send().await;

        if let Err(e) = res {
            let s = e.source().unwrap();
            assert!(
                s.to_string().contains("client error (Connect)"),
                "Expected socks connect error, got: {s}",
            );
            assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
        } else {
            panic!("Socks proxy was ignored")
        }
    }

    #[tokio::test]
    async fn resume_partial_from_file_url() {
        let tmpdir = tmp_dir();
        let from_path = tmpdir.path().join("download-source");
        write_file(&from_path, "xxx45");

        let target_path = tmpdir.path().join("downloaded");
        write_file(&target_path, "123");

        let from_url = Url::from_file_path(&from_path).unwrap();
        Backend::Reqwest(TlsBackend::NativeTls)
            .download_to_path(&from_url, &target_path, true, None)
            .await
            .expect("Test download failed");

        assert_eq!(std::fs::read_to_string(&target_path).unwrap(), "12345");
    }

    #[tokio::test]
    async fn callback_gets_all_data_as_if_the_download_happened_all_at_once() {
        let tmpdir = tmp_dir();
        let target_path = tmpdir.path().join("downloaded");
        write_file(&target_path, "123");

        let addr = serve_file(b"xxx45".to_vec());

        let from_url = format!("http://{addr}").parse().unwrap();

        let callback_partial = AtomicBool::new(false);
        let callback_len = Mutex::new(None);
        let received_in_callback = Mutex::new(Vec::new());

        Backend::Reqwest(TlsBackend::NativeTls)
            .download_to_path(
                &from_url,
                &target_path,
                true,
                Some(&|msg| {
                    match msg {
                        Event::ResumingPartialDownload => {
                            assert!(!callback_partial.load(Ordering::SeqCst));
                            callback_partial.store(true, Ordering::SeqCst);
                        }
                        Event::DownloadContentLengthReceived(len) => {
                            let mut flag = callback_len.lock().unwrap();
                            assert!(flag.is_none());
                            *flag = Some(len);
                        }
                        Event::DownloadDataReceived(data) => {
                            for b in data.iter() {
                                received_in_callback.lock().unwrap().push(*b);
                            }
                        }
                    }

                    Ok(())
                }),
            )
            .await
            .expect("Test download failed");

        assert!(callback_partial.into_inner());
        assert_eq!(*callback_len.lock().unwrap(), Some(5));
        let observed_bytes = received_in_callback.into_inner().unwrap();
        assert_eq!(observed_bytes, vec![b'1', b'2', b'3', b'4', b'5']);
        assert_eq!(std::fs::read_to_string(&target_path).unwrap(), "12345");
    }
}

pub fn tmp_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("rustup-download-test-")
        .tempdir()
        .expect("creating tempdir for test")
}

pub fn write_file(path: &Path, contents: &str) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .expect("writing test data");

    io::Write::write_all(&mut file, contents.as_bytes()).expect("writing test data");

    file.sync_data().expect("writing test data");
}

// A dead simple hyper server implementation.
// For more info, see:
// https://hyper.rs/guides/1/server/hello-world/
async fn run_server(addr_tx: Sender<SocketAddr>, addr: SocketAddr, contents: Vec<u8>) {
    let svc = service_fn(move |req: Request<hyper::body::Incoming>| {
        let contents = contents.clone();
        async move {
            let res = serve_contents(req, contents);
            Ok::<_, Infallible>(res)
        }
    });

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("can not bind");

    let addr = listener.local_addr().unwrap();
    addr_tx.send(addr).unwrap();

    loop {
        let (stream, _) = listener
            .accept()
            .await
            .expect("could not accept connection");
        let io = hyper_util::rt::TokioIo::new(stream);

        let svc = svc.clone();
        tokio::spawn(async move {
            if let Err(err) = http1::Builder::new().serve_connection(io, svc).await {
                eprintln!("failed to serve connection: {err:?}");
            }
        });
    }
}

pub fn serve_file(contents: Vec<u8>) -> SocketAddr {
    let addr = ([127, 0, 0, 1], 0).into();
    let (addr_tx, addr_rx) = channel();

    thread::spawn(move || {
        let server = run_server(addr_tx, addr, contents);
        let rt = tokio::runtime::Runtime::new().expect("could not creating Runtime");
        rt.block_on(server);
    });

    let addr = addr_rx.recv();
    addr.unwrap()
}

fn serve_contents(
    req: hyper::Request<hyper::body::Incoming>,
    contents: Vec<u8>,
) -> hyper::Response<Full<Bytes>> {
    let mut range_header = None;
    let (status, body) = if let Some(range) = req.headers().get(hyper::header::RANGE) {
        // extract range "bytes={start}-"
        let range = range.to_str().expect("unexpected Range header");
        assert!(range.starts_with("bytes="));
        let range = range.trim_start_matches("bytes=");
        assert!(range.ends_with('-'));
        let range = range.trim_end_matches('-');
        assert_eq!(range.split('-').count(), 1);
        let start: u64 = range.parse().expect("unexpected Range header");

        range_header = Some(format!("bytes {}-{len}/{len}", start, len = contents.len()));
        (
            hyper::StatusCode::PARTIAL_CONTENT,
            contents[start as usize..].to_vec(),
        )
    } else {
        (hyper::StatusCode::OK, contents)
    };

    let mut res = hyper::Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_LENGTH, body.len())
        .body(Full::new(Bytes::from(body)))
        .unwrap();
    if let Some(range) = range_header {
        res.headers_mut()
            .insert(hyper::header::CONTENT_RANGE, range.parse().unwrap());
    }
    res
}
