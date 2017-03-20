

use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::net::SocketAddr;
use std::fs::File;
use std::sync::{Arc, Mutex, MutexGuard};

use url;
use hyper;
use hyper::header::{Range, ByteRangeSpec};
use hyper::server::Handler;
use hyper::server::request::*;
use hyper::uri::RequestUri::*;
use hyper::server::response::*;
use hyper::net::Fresh;
use hyper::status::StatusCode;

struct ServerImp {
    base_path: PathBuf,
    max_bytes: Option<u64>,
}

pub struct Server {
    server_addr: SocketAddr,
    hserver: hyper::server::Listening,
    server_imp: Arc<Mutex<ServerImp>>,
}

impl ServerImp {
    fn put_file_from_bytes<T: AsRef<Path>>(&self, rel_path: T, bytes: &[u8]) {
        File::create(&self.base_path.join(rel_path))
            .expect("creating file for sample data")
            .write_all(bytes)
            .expect("writing sample data");
    }

    fn stop_after_bytes(&mut self, bytes_to_serve: u64) {
        self.max_bytes = if bytes_to_serve > 0 {
            Some(bytes_to_serve)
        } else {
            None
        };
    }

    fn write_file<T>(&self, path: &Path, mut to: T, requested_range: Option<&Range>) -> Result<(), io::Error>
        where T: Write {

        let mut to_send = try!(File::open(path));

        // Don't need to support every case here, just trying to test the resume-from case
        if let Some(requested_range) = requested_range {
            match requested_range {
                &Range::Bytes(ref byte_ranges) => {
                    match byte_ranges[0] {
                        ByteRangeSpec::AllFrom(n) => {
                            to_send.seek(SeekFrom::Start(n)).expect("Seeking to start of requested byte range");
                        },
                        _ => { unimplemented!() }
                    }
                }
                _ => unimplemented!()
            }

        }

        let mut to_send : Box<io::Read> = if let Some(bytes_to_serve) = self.max_bytes {
            Box::new(to_send.take(bytes_to_serve))
        } else {
            Box::new(to_send)
        };

        io::copy(&mut to_send, &mut to).expect("Copying from file to response");
        Ok(())
    }

    fn write_file_to_response(&self, path: &Path, mut response: Response<Fresh>, mut request: Request) {
        if !path.exists() {
            *response.status_mut() = StatusCode::NotFound;
            return;
        }
        let mut response = response.start().unwrap();
        let byte_ranges = request.headers.get::<Range>();
        self.write_file(path, &mut response, byte_ranges);
    }

    fn serve_file(&self, base_path: &Path, request: Request, mut response: Response<Fresh>) {
        match request.uri.clone() {
            AbsolutePath(s) => {
                let p = base_path.join(&s[1..]);
                self.write_file_to_response(&p, response, request);
            }
            AbsoluteUri(u) => {
                let urlpath = u.path();
                let p = base_path.join(&urlpath[1..]);
                self.write_file_to_response(&p, response, request);
            }
            _ => *response.status_mut() = StatusCode::MethodNotAllowed,
        }
    }
}

impl Server {
    fn imp(&self) -> MutexGuard<ServerImp> {
        self.server_imp.lock().expect("Lock has been poisoned by failure in another thread - aborting test")
    }

    pub fn serve_from(path: &Path) -> Result<Server, hyper::error::Error> {
        let hserver = try!(hyper::server::Server::http("127.0.0.1:0"));
        let base_path = path.to_owned();

        let server_imp = Arc::new(Mutex::new(ServerImp {
            base_path: base_path.clone(),
            max_bytes: None,
        }));

        let server_clone = server_imp.clone();
        let listening = try!(hserver.handle(move |req: Request, resp: Response| {
            server_clone.lock().unwrap().serve_file(&base_path, req, resp);
        }));

        Ok(Server {
               server_addr: listening.socket.clone(),
               hserver: listening,
               server_imp: server_imp,
           })
    }

    pub fn put_file_from_bytes<T: AsRef<Path>>(&self, rel_path: T, bytes: &[u8]) {
        self.imp().put_file_from_bytes(rel_path, bytes)
    }


    pub fn address(&self) -> url::Url {
        let url_string = format!("http://127.0.0.1:{}", self.server_addr.port());
        url::Url::parse(&url_string).expect("Mistake in url for test server")
    }

    pub fn stop_after_bytes(&mut self, bytes_to_serve: u64) {
        self.imp().stop_after_bytes(bytes_to_serve)
    }

}

impl Drop for Server {
    fn drop(&mut self) {
        self.hserver.close().expect("Unable to shut down server");
    }
}
