use notifications::NotifyHandler;

use std::error;
use std::fs;
use std::path::Path;
use std::io;
use std::char::from_u32;
use std::io::Write;
use std::process::{Command, Stdio, ExitStatus};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::thread;
use std::time::Duration;
use hyper::{self, Client};
use sha2::{Sha256, Digest};
use errors::*;

use rand::random;

pub fn ensure_dir_exists<P: AsRef<Path>, F: FnOnce(&Path)>(path: P,
                                                           callback: F)
                                                           -> io::Result<bool> {
    if !is_directory(path.as_ref()) {
        callback(path.as_ref());
        fs::create_dir_all(path.as_ref()).map(|()| true)
    } else {
        Ok(false)
    }
}

pub fn is_directory<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).ok().as_ref().map(fs::Metadata::is_dir) == Some(true)
}

pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).ok().as_ref().map(fs::Metadata::is_file) == Some(true)
}

pub fn path_exists<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).is_ok()
}

pub fn random_string(length: usize) -> String {
    let chars = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    (0..length).map(|_| from_u32(chars[random::<usize>() % chars.len()] as u32).unwrap()).collect()
}

pub fn if_not_empty<S: PartialEq<str>>(s: S) -> Option<S> {
    if s == *"" {
        None
    } else {
        Some(s)
    }
}

pub fn write_file(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = try!(fs::OpenOptions::new()
                            .write(true)
                            .truncate(true)
                            .create(true)
                            .open(path));

    try!(io::Write::write_all(&mut file, contents.as_bytes()));

    try!(file.sync_data());

    Ok(())
}

pub fn read_file(path: &Path) -> io::Result<String> {
    let mut file = try!(fs::OpenOptions::new()
                            .read(true)
                            .open(path));

    let mut contents = String::new();

    try!(io::Read::read_to_string(&mut file, &mut contents));

    Ok(contents)
}

pub fn filter_file<F: FnMut(&str) -> bool>(src: &Path,
                                           dest: &Path,
                                           mut filter: F)
                                           -> io::Result<usize> {
    let src_file = try!(fs::File::open(src));
    let dest_file = try!(fs::File::create(dest));

    let mut reader = io::BufReader::new(src_file);
    let mut writer = io::BufWriter::new(dest_file);
    let mut removed = 0;

    for result in io::BufRead::lines(&mut reader) {
        let line = try!(result);
        if filter(&line) {
            try!(writeln!(&mut writer, "{}", &line));
        } else {
            removed += 1;
        }
    }

    try!(writer.flush());

    Ok(removed)
}

pub fn match_file<T, F: FnMut(&str) -> Option<T>>(src: &Path, mut f: F) -> io::Result<Option<T>> {
    let src_file = try!(fs::File::open(src));

    let mut reader = io::BufReader::new(src_file);

    for result in io::BufRead::lines(&mut reader) {
        let line = try!(result);
        if let Some(r) = f(&line) {
            return Ok(Some(r));
        }
    }

    Ok(None)
}

pub fn append_file(dest: &Path, line: &str) -> io::Result<()> {
    let mut dest_file = try!(fs::OpenOptions::new()
                                 .write(true)
                                 .append(true)
                                 .create(true)
                                 .open(dest));

    try!(writeln!(&mut dest_file, "{}", line));

    try!(dest_file.sync_data());

    Ok(())
}

pub fn tee_file<W: io::Write>(path: &Path, mut w: &mut W) -> io::Result<()> {
    let mut file = try!(fs::OpenOptions::new()
                            .read(true)
                            .open(path));

    let buffer_size = 0x10000;
    let mut buffer = vec![0u8; buffer_size];

    loop {
        let bytes_read = try!(io::Read::read(&mut file, &mut buffer));

        if bytes_read != 0 {
            try!(io::Write::write_all(w, &mut buffer[0..bytes_read]));
        } else {
            return Ok(());
        }
    }
}

pub fn download_file<P: AsRef<Path>>(url: hyper::Url,
                                     path: P,
                                     mut hasher: Option<&mut Sha256>,
                                     notify_handler: NotifyHandler)
                                     -> Result<()> {

    // Short-circuit hyper for the "file:" URL scheme
    if try!(download_from_file_url(&url, &path, &mut hasher)) {
        return Ok(());
    }

    use hyper::error::Result as HyperResult;
    use hyper::header::ContentLength;
    use hyper::net::{SslClient, NetworkStream, HttpsConnector};
    use native_tls;
    use notifications::Notification;
    use std::io::Result as IoResult;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, Shutdown};
    use std::sync::{Arc, Mutex};

    // This is just a defensive measure to make sure I'm not sending
    // anything through hyper I haven't tested.
    if url.scheme() != "https" {
        return Err(format!("unsupported URL scheme: '{}'", url.scheme()).into());
    }

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

    impl<T> NetworkStream for NativeSslStream<T>
        where T: NetworkStream
    {
        fn peer_addr(&mut self) -> IoResult<SocketAddr> {
            self.0.lock().expect("").get_mut().peer_addr()
        }
        fn set_read_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
            self.0.lock().expect("").get_ref().set_read_timeout(dur)
        }
        fn set_write_timeout(&self, dur: Option<Duration>) -> IoResult<()> {
            self.0.lock().expect("").get_ref().set_read_timeout(dur)
        }
        fn close(&mut self, how: Shutdown) -> IoResult<()> {
            self.0.lock().expect("").get_mut().close(how)
        }
    }

    impl<T> Read for NativeSslStream<T>
        where T: Read + Write
    {
        fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
            self.0.lock().expect("").read(buf)
        }
    }

    impl<T> Write for NativeSslStream<T>
        where T: Read + Write
    {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            self.0.lock().expect("").write(buf)
        }
        fn flush(&mut self) -> IoResult<()> {
            self.0.lock().expect("").flush()
        }
    }

    maybe_init_certs();

    // Connect with hyper + native_tls
    let client = Client::with_connector(HttpsConnector::new(NativeSslClient));

    let mut res = try!(client.get(url).send()
                       .chain_err(|| "failed to make network request"));
    if res.status != hyper::Ok {
        return Err(ErrorKind::HttpStatus(res.status).into());
    }

    let buffer_size = 0x10000;
    let mut buffer = vec![0u8; buffer_size];

    let mut file = try!(fs::File::create(&path).chain_err(
        || "error creating file for download"));

    if let Some(len) = res.headers.get::<ContentLength>().cloned() {
        notify_handler.call(Notification::DownloadContentLengthReceived(len.0));
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
            notify_handler.call(Notification::DownloadDataReceived(bytes_read));
        } else {
            try!(file.sync_data().chain_err(|| "unable to sync download to disk"));
            notify_handler.call(Notification::DownloadFinished);
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

fn download_from_file_url<P: AsRef<Path>>(url: &hyper::Url,
                                          path: P,
                                          hasher: &mut Option<&mut Sha256>)
                                          -> Result<bool> {
    // The file scheme is mostly for use by tests to mock the dist server
    if url.scheme() == "file" {
        let src = try!(url.to_file_path()
                       .map_err(|_| Error::from(format!("bogus file url: '{}'", url))));
        if !is_file(&src) {
            // Because some of multirust's logic depends on checking
            // the error when a downloaded file doesn't exist, make
            // the file case return the same error value as the
            // network case.
            return Err(ErrorKind::HttpStatus(hyper::status::StatusCode::NotFound).into());
        }
        try!(fs::copy(&src, path.as_ref()).chain_err(|| "failure copying file"));

        if let Some(ref mut h) = *hasher {
            let ref mut f = try!(fs::File::open(path.as_ref())
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

pub fn symlink_dir(src: &Path, dest: &Path) -> io::Result<()> {
    #[cfg(windows)]
    fn symlink_dir_inner(src: &Path, dest: &Path) -> io::Result<()> {
        // std's symlink uses Windows's symlink function, which requires
        // admin. We can create directory junctions the hard way without
        // though.
        symlink_junction_inner(src, dest)
    }
    #[cfg(not(windows))]
    fn symlink_dir_inner(src: &Path, dest: &Path) -> io::Result<()> {
        ::std::os::unix::fs::symlink(src, dest)
    }

    let _ = remove_dir(dest);
    symlink_dir_inner(src, dest)
}

// Creating a directory junction on windows involves dealing with reparse
// points and the DeviceIoControl function, and this code is a skeleton of
// what can be found here:
//
// http://www.flexhex.com/docs/articles/hard-links.phtml
//
// Copied from std
#[cfg(windows)]
#[allow(non_snake_case)]
fn symlink_junction_inner(target: &Path, junction: &Path) -> io::Result<()> {
    use winapi::*;
    use kernel32::*;
    use std::ptr;
    use std::os::windows::ffi::OsStrExt;

    const MAXIMUM_REPARSE_DATA_BUFFER_SIZE: usize = 16 * 1024;

    #[repr(C)]
    pub struct REPARSE_MOUNTPOINT_DATA_BUFFER {
        pub ReparseTag: DWORD,
        pub ReparseDataLength: DWORD,
        pub Reserved: WORD,
        pub ReparseTargetLength: WORD,
        pub ReparseTargetMaximumLength: WORD,
        pub Reserved1: WORD,
        pub ReparseTarget: WCHAR,
    }

    fn to_u16s<S: AsRef<OsStr>>(s: S) -> io::Result<Vec<u16>> {
        fn inner(s: &OsStr) -> io::Result<Vec<u16>> {
            let mut maybe_result: Vec<u16> = s.encode_wide().collect();
            if maybe_result.iter().any(|&u| u == 0) {
                return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                          "strings passed to WinAPI cannot contain NULs"));
            }
            maybe_result.push(0);
            Ok(maybe_result)
        }
        inner(s.as_ref())
    }

    try!(fs::create_dir(junction));

    let path = try!(to_u16s(junction));

    unsafe {
        let h = CreateFileW(path.as_ptr(),
                            GENERIC_WRITE,
                            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                            0 as *mut _,
                            OPEN_EXISTING,
                            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                            ptr::null_mut());
    
        let mut data = [0u8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE];
        let mut db = data.as_mut_ptr()
                        as *mut REPARSE_MOUNTPOINT_DATA_BUFFER;
        let buf = &mut (*db).ReparseTarget as *mut _;
        let mut i = 0;
        // FIXME: this conversion is very hacky
        let v = br"\??\";
        let v = v.iter().map(|x| *x as u16);
        for c in v.chain(target.as_os_str().encode_wide()) {
            *buf.offset(i) = c;
            i += 1;
        }
        *buf.offset(i) = 0;
        i += 1;
        (*db).ReparseTag = IO_REPARSE_TAG_MOUNT_POINT;
        (*db).ReparseTargetMaximumLength = (i * 2) as WORD;
        (*db).ReparseTargetLength = ((i - 1) * 2) as WORD;
        (*db).ReparseDataLength =
                (*db).ReparseTargetLength as DWORD + 12;

        let mut ret = 0;
        let res = DeviceIoControl(h as *mut _,
                                  FSCTL_SET_REPARSE_POINT,
                                  data.as_ptr() as *mut _,
                                  (*db).ReparseDataLength + 8,
                                  ptr::null_mut(), 0,
                                  &mut ret,
                                  ptr::null_mut());
    
        if res == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

pub fn hardlink(src: &Path, dest: &Path) -> io::Result<()> {
    let _ = fs::remove_file(dest);
    fs::hard_link(src, dest)
}

#[derive(Debug)]
pub enum CommandError {
    Io(io::Error),
    Status(ExitStatus),
}

pub type CommandResult<T> = ::std::result::Result<T, CommandError>;

impl error::Error for CommandError {
    fn description(&self) -> &str {
        use self::CommandError::*;
        match *self {
            Io(_) => "could not execute command",
            Status(_) => "command exited with unsuccessful status",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        use self::CommandError::*;
        match *self {
            Io(ref e) => Some(e),
            Status(_) => None,
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CommandError::Io(ref e) => write!(f, "Io: {}", e),
            CommandError::Status(ref s) => write!(f, "Status: {}", s),
        }
    }
}

pub fn cmd_status(cmd: &mut Command) -> CommandResult<()> {
    cmd.status().map_err(CommandError::Io).and_then(|s| {
        if s.success() {
            Ok(())
        } else {
            Err(CommandError::Status(s))
        }
    })
}

pub fn remove_dir(path: &Path) -> io::Result<()> {
    if try!(fs::symlink_metadata(path)).file_type().is_symlink() {
        if cfg!(windows) {
            fs::remove_dir(path)
        } else {
            fs::remove_file(path)
        }
    } else {
        let mut result = Ok(());

        // The implementation of `remove_dir_all` is broken on windows,
        // so may need to try multiple times!
        for _ in 0..5 {
            result = rm_rf(path);
            if !is_directory(path) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(16));
        }
        result
    }
}

// Again because remove_dir all doesn't delete write-only files on windows,
// this is a custom implementation, more-or-less copied from cargo.
// cc rust-lang/rust#31944
// cc https://github.com/rust-lang/cargo/blob/master/tests/support/paths.rs#L52-L80
fn rm_rf(path: &Path) -> io::Result<()> {
    if path.exists() {
        for file in fs::read_dir(path).unwrap() {
            let file = try!(file);
            let is_dir = try!(file.file_type()).is_dir();
            let ref file = file.path();

            if is_dir {
                try!(rm_rf(file));
            } else {
                // On windows we can't remove a readonly file, and git will
                // often clone files as readonly. As a result, we have some
                // special logic to remove readonly files on windows.
                match fs::remove_file(file) {
                    Ok(()) => {}
                    Err(ref e) if cfg!(windows) &&
                        e.kind() == io::ErrorKind::PermissionDenied => {
                            let mut p = file.metadata().unwrap().permissions();
                            p.set_readonly(false);
                            fs::set_permissions(file, p).unwrap();
                            try!(fs::remove_file(file));
                        }
                    Err(e) => return Err(e)
                }
            }
        }
        fs::remove_dir(path)
    } else {
        Ok(())
    }
}

pub fn copy_dir(src: &Path, dest: &Path) -> CommandResult<()> {
    #[cfg(windows)]
    fn copy_dir_inner(src: &Path, dest: &Path) -> CommandResult<()> {
        Command::new("robocopy")
            .arg(src)
            .arg(dest)
            .arg("/E")
            .arg("/NFL")
            .arg("/NDL")
            .arg("/NJH")
            .arg("/NJS")
            .arg("/nc")
            .arg("/ns")
            .arg("/np")
            .status()
            .map_err(CommandError::Io)
            .and_then(|s| {
                match s.code() {
                    // Robocopy has non-zero exit codes for successful copies...
                    Some(value) if value < 8 => Ok(()),
                    _ => Err(CommandError::Status(s)),
                }
            })
    }
    #[cfg(not(windows))]
    fn copy_dir_inner(src: &Path, dest: &Path) -> CommandResult<()> {
        cmd_status(Command::new("cp").arg("-R").arg(src).arg(dest))
    }

    let _ = remove_dir(dest);
    copy_dir_inner(src, dest)
}

pub fn prefix_arg<S: AsRef<OsStr>>(name: &str, s: S) -> OsString {
    let mut arg = OsString::from(name);
    arg.push(s);
    arg
}

pub fn has_cmd(cmd: &str) -> bool {
    #[cfg(not(windows))]
    fn inner(cmd: &str) -> bool {
        cmd_status(Command::new("which")
                       .arg(cmd)
                       .stdin(Stdio::null())
                       .stdout(Stdio::null())
                       .stderr(Stdio::null()))
            .is_ok()
    }
    #[cfg(windows)]
    fn inner(cmd: &str) -> bool {
        cmd_status(Command::new("where")
                       .arg("/Q")
                       .arg(cmd))
            .is_ok()
    }

    inner(cmd)
}

pub fn find_cmd<'a>(cmds: &[&'a str]) -> Option<&'a str> {
    cmds.into_iter().map(|&s| s).filter(|&s| has_cmd(s)).next()
}

pub fn open_browser(path: &Path) -> io::Result<bool> {
    #[cfg(not(windows))]
    fn inner(path: &Path) -> io::Result<bool> {
        let commands = ["xdg-open", "open", "firefox", "chromium"];
        if let Some(cmd) = find_cmd(&commands) {
            Command::new(cmd)
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map(|_| true)
        } else {
            Ok(false)
        }
    }
    #[cfg(windows)]
    fn inner(path: &Path) -> io::Result<bool> {
        Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| true)
    }
    inner(path)
}

#[cfg(windows)]
pub mod windows {
    use winapi::*;
    use std::io;
    use std::path::PathBuf;
    use std::ptr;
    use std::slice;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use shell32;
    use ole32;

    #[allow(non_upper_case_globals)]
    pub const FOLDERID_LocalAppData: GUID = GUID {
        Data1: 0xF1B32785,
        Data2: 0x6FBA,
        Data3: 0x4FCF,
        Data4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91],
    };
    #[allow(non_upper_case_globals)]
    pub const FOLDERID_Profile: GUID = GUID {
        Data1: 0x5E6C858F,
        Data2: 0x0E22,
        Data3: 0x4760,
        Data4: [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x71, 0x73],
    };

    pub fn get_special_folder(id: &shtypes::KNOWNFOLDERID) -> io::Result<PathBuf> {


        let mut path = ptr::null_mut();
        let result;

        unsafe {
            let code = shell32::SHGetKnownFolderPath(id, 0, ptr::null_mut(), &mut path);
            if code == 0 {
                let mut length = 0usize;
                while *path.offset(length as isize) != 0 {
                    length += 1;
                }
                let slice = slice::from_raw_parts(path, length);
                result = Ok(OsString::from_wide(slice).into());
            } else {
                result = Err(io::Error::from_raw_os_error(code));
            }
            ole32::CoTaskMemFree(path as *mut _);
        }
        result
    }
}
