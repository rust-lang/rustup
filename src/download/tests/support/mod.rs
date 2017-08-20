extern crate futures;
extern crate hyper;
extern crate tempdir;

use std::fs::{self, File};
use std::io::{self, Read};
use std::net::SocketAddr;
use std::path::Path;

use self::futures::sync::oneshot;
use self::tempdir::TempDir;

pub fn tmp_dir() -> TempDir {
    TempDir::new("rustup-download-test-").expect("creating tempdir for test")
}

pub fn file_contents(path: &Path) -> String {
    let mut result = String::new();
    File::open(&path).unwrap().read_to_string(&mut result).expect("reading test result file");
    result
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


type Shutdown = oneshot::Sender<()>;

pub fn serve_file(contents: Vec<u8>) -> (SocketAddr, Shutdown) {
    use std::thread;
    use self::futures::Future;
    use self::hyper::server::Http;

    let http = Http::new();
    let addr = ([127, 0, 0, 1], 0).into();
    let (addr_tx, addr_rx) = oneshot::channel();
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {

        let server = http.bind(&addr, move || Ok(ServeFile(contents.clone())))
            .expect("server setup failed");
        let addr = server.local_addr().expect("local addr failed");
        addr_tx.send(addr).unwrap();
        server.run_until(rx.map_err(|_| ()))
            .expect("server failed");
    });
    let addr = addr_rx.wait().unwrap();
    (addr, tx)
}

struct ServeFile(Vec<u8>);

impl hyper::server::Service for ServeFile {
    type Request = hyper::server::Request;
    type Response = hyper::server::Response;
    type Error = hyper::Error;
    type Future = futures::future::FutureResult<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let mut return_range = None;
        let (status, body) = if let Some(range) = req.headers().get::<hyper::header::Range>() {
            match *range {
                hyper::header::Range::Bytes(ref specs) => {
                    assert_eq!(specs.len(), 1);
                    match specs[0] {
                        hyper::header::ByteRangeSpec::AllFrom(start) => {
                            return_range = Some(hyper::header::ContentRange(
                                hyper::header::ContentRangeSpec::Bytes {
                                    range: Some((start, self.0.len() as u64)),
                                    instance_length: Some(self.0.len() as u64),
                                }
                            ));
                            (hyper::StatusCode::PartialContent, self.0[start as usize..].to_vec())
                        },
                        _ => panic!("unexpected Range header"),
                    }
                },
                _ => panic!("unexpected Range header"),
            }
        } else {
            (hyper::StatusCode::Ok, self.0.clone())
        };

        let mut res = hyper::server::Response::new()
            .with_status(status)
            .with_header(hyper::header::ContentLength(body.len() as u64))
            .with_body(body);
        if let Some(range) = return_range {
            res.headers_mut().set(range);
        }
        futures::future::ok(res)
    }
}
