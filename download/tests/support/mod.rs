use std::convert::Infallible;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc::{channel, Sender};
use std::thread;

use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request};
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

async fn run_server(addr_tx: Sender<SocketAddr>, addr: SocketAddr, contents: Vec<u8>) {
    let make_svc = make_service_fn(|_: &AddrStream| {
        let contents = contents.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let contents = contents.clone();
                async move {
                    let res = serve_contents(req, contents);
                    Ok::<_, Infallible>(res)
                }
            }))
        }
    });

    let server = hyper::server::Server::bind(&addr).serve(make_svc);
    let addr = server.local_addr();
    addr_tx.send(addr).unwrap();

    if let Err(e) = server.await {
        eprintln!("server error: {e}");
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
    req: hyper::Request<hyper::Body>,
    contents: Vec<u8>,
) -> hyper::Response<hyper::Body> {
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
        .body(hyper::Body::from(body))
        .unwrap();
    if let Some(range) = range_header {
        res.headers_mut()
            .insert(hyper::header::CONTENT_RANGE, range.parse().unwrap());
    }
    res
}
