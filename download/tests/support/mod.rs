use std::convert::Infallible;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc::{channel, Sender};
use std::thread;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::Request;
use tempfile::TempDir;

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
                eprintln!("failed to serve connection: {:?}", err);
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
