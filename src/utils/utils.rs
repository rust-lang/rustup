use crate::errors::*;
use crate::utils::notifications::Notification;
use crate::utils::raw;
use sha2::Sha256;
use std::cmp::Ord;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

#[cfg(windows)]
use winapi::shared::minwindef::DWORD;

pub use crate::utils::utils::raw::{
    find_cmd, has_cmd, if_not_empty, is_directory, is_file, path_exists, prefix_arg, random_string,
};

pub struct ExitCode(pub i32);

pub fn ensure_dir_exists(
    name: &'static str,
    path: &Path,
    notify_handler: &dyn Fn(Notification<'_>),
) -> Result<bool> {
    raw::ensure_dir_exists(path, |p| {
        notify_handler(Notification::CreatingDirectory(name, p))
    })
    .chain_err(|| ErrorKind::CreatingDirectory {
        name,
        path: PathBuf::from(path),
    })
}

pub fn read_file(name: &'static str, path: &Path) -> Result<String> {
    raw::read_file(path).chain_err(|| ErrorKind::ReadingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub fn write_file(name: &'static str, path: &Path, contents: &str) -> Result<()> {
    raw::write_file(path, contents).chain_err(|| ErrorKind::WritingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub fn append_file(name: &'static str, path: &Path, line: &str) -> Result<()> {
    raw::append_file(path, line).chain_err(|| ErrorKind::WritingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub fn write_line(name: &'static str, file: &mut File, path: &Path, line: &str) -> Result<()> {
    writeln!(file, "{}", line).chain_err(|| ErrorKind::WritingFile {
        name,
        path: path.to_path_buf(),
    })
}

pub fn write_str(name: &'static str, file: &mut File, path: &Path, s: &str) -> Result<()> {
    write!(file, "{}", s).chain_err(|| ErrorKind::WritingFile {
        name,
        path: path.to_path_buf(),
    })
}

pub fn rename_file(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
    rename(name, src, dest)
}

pub fn rename_dir(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
    rename(name, src, dest)
}

pub fn filter_file<F: FnMut(&str) -> bool>(
    name: &'static str,
    src: &Path,
    dest: &Path,
    filter: F,
) -> Result<usize> {
    raw::filter_file(src, dest, filter).chain_err(|| ErrorKind::FilteringFile {
        name,
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

pub fn match_file<T, F: FnMut(&str) -> Option<T>>(
    name: &'static str,
    src: &Path,
    f: F,
) -> Result<Option<T>> {
    raw::match_file(src, f).chain_err(|| ErrorKind::ReadingFile {
        name,
        path: PathBuf::from(src),
    })
}

pub fn canonicalize_path(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        notify_handler(Notification::NoCanonicalPath(path));
        PathBuf::from(path)
    })
}

pub fn tee_file<W: io::Write>(name: &'static str, path: &Path, w: &mut W) -> Result<()> {
    raw::tee_file(path, w).chain_err(|| ErrorKind::ReadingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub fn download_file(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    notify_handler: &dyn Fn(Notification<'_>),
) -> Result<()> {
    download_file_with_resume(&url, &path, hasher, false, &notify_handler)
}

pub fn download_file_with_resume(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    resume_from_partial: bool,
    notify_handler: &dyn Fn(Notification<'_>),
) -> Result<()> {
    use download::ErrorKind as DEK;
    match download_file_(url, path, hasher, resume_from_partial, notify_handler) {
        Ok(_) => Ok(()),
        Err(e) => {
            let is_client_error = match e.kind() {
                ErrorKind::Download(DEK::HttpStatus(400..=499)) => true,
                ErrorKind::Download(DEK::FileNotFound) => true,
                _ => false,
            };
            Err(e).chain_err(|| {
                if is_client_error {
                    ErrorKind::DownloadNotExists {
                        url: url.clone(),
                        path: path.to_path_buf(),
                    }
                } else {
                    ErrorKind::DownloadingFile {
                        url: url.clone(),
                        path: path.to_path_buf(),
                    }
                }
            })
        }
    }
}

fn download_file_(
    url: &Url,
    path: &Path,
    hasher: Option<&mut Sha256>,
    resume_from_partial: bool,
    notify_handler: &dyn Fn(Notification<'_>),
) -> Result<()> {
    use download::download_to_path_with_backend;
    use download::{Backend, Event};
    use sha2::Digest;
    use std::cell::RefCell;

    notify_handler(Notification::DownloadingFile(url, path));

    let hasher = RefCell::new(hasher);

    // This callback will write the download to disk and optionally
    // hash the contents, then forward the notification up the stack
    let callback: &dyn Fn(Event<'_>) -> download::Result<()> = &|msg| {
        if let Event::DownloadDataReceived(data) = msg {
            if let Some(h) = hasher.borrow_mut().as_mut() {
                h.input(data);
            }
        }

        match msg {
            Event::DownloadContentLengthReceived(len) => {
                notify_handler(Notification::DownloadContentLengthReceived(len));
            }
            Event::DownloadDataReceived(data) => {
                notify_handler(Notification::DownloadDataReceived(data));
            }
            Event::ResumingPartialDownload => {
                notify_handler(Notification::ResumingPartialDownload);
            }
        }

        Ok(())
    };

    // Download the file

    // Keep the hyper env var around for a bit
    let use_curl_backend = env::var_os("RUSTUP_USE_CURL").is_some();
    let (backend, notification) = if use_curl_backend {
        (Backend::Curl, Notification::UsingCurl)
    } else {
        (Backend::Reqwest, Notification::UsingReqwest)
    };
    notify_handler(notification);
    download_to_path_with_backend(backend, url, path, resume_from_partial, Some(callback))?;

    notify_handler(Notification::DownloadFinished);

    Ok(())
}

pub fn parse_url(url: &str) -> Result<Url> {
    Url::parse(url).chain_err(|| format!("failed to parse url: {}", url))
}

pub fn cmd_status(name: &'static str, cmd: &mut Command) -> Result<()> {
    raw::cmd_status(cmd).chain_err(|| ErrorKind::RunningCommand {
        name: OsString::from(name),
    })
}

pub fn assert_is_file(path: &Path) -> Result<()> {
    if !is_file(path) {
        Err(ErrorKind::NotAFile {
            path: PathBuf::from(path),
        }
        .into())
    } else {
        Ok(())
    }
}

pub fn assert_is_directory(path: &Path) -> Result<()> {
    if !is_directory(path) {
        Err(ErrorKind::NotADirectory {
            path: PathBuf::from(path),
        }
        .into())
    } else {
        Ok(())
    }
}

pub fn symlink_dir(
    src: &Path,
    dest: &Path,
    notify_handler: &dyn Fn(Notification<'_>),
) -> Result<()> {
    notify_handler(Notification::LinkingDirectory(src, dest));
    raw::symlink_dir(src, dest).chain_err(|| ErrorKind::LinkingDirectory {
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

pub fn hard_or_symlink_file(src: &Path, dest: &Path) -> Result<()> {
    if hardlink_file(src, dest).is_err() {
        symlink_file(src, dest)?;
    }
    Ok(())
}

pub fn hardlink_file(src: &Path, dest: &Path) -> Result<()> {
    raw::hardlink(src, dest).chain_err(|| ErrorKind::LinkingFile {
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

#[cfg(unix)]
pub fn symlink_file(src: &Path, dest: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dest).chain_err(|| ErrorKind::LinkingFile {
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

#[cfg(windows)]
pub fn symlink_file(src: &Path, dest: &Path) -> Result<()> {
    // we are supposed to not use symlink on windows
    Err(ErrorKind::LinkingFile {
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    }
    .into())
}

pub fn copy_dir(src: &Path, dest: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<()> {
    notify_handler(Notification::CopyingDirectory(src, dest));
    raw::copy_dir(src, dest).chain_err(|| ErrorKind::CopyingDirectory {
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

pub fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(src).chain_err(|| ErrorKind::ReadingFile {
        name: "metadata for",
        path: PathBuf::from(src),
    })?;
    if metadata.file_type().is_symlink() {
        symlink_file(&src, dest).map(|_| ())
    } else {
        fs::copy(src, dest)
            .chain_err(|| ErrorKind::CopyingFile {
                src: PathBuf::from(src),
                dest: PathBuf::from(dest),
            })
            .map(|_| ())
    }
}

pub fn remove_dir(
    name: &'static str,
    path: &Path,
    notify_handler: &dyn Fn(Notification<'_>),
) -> Result<()> {
    notify_handler(Notification::RemovingDirectory(name, path));
    raw::remove_dir(path).chain_err(|| ErrorKind::RemovingDirectory {
        name,
        path: PathBuf::from(path),
    })
}

pub fn remove_file(name: &'static str, path: &Path) -> Result<()> {
    fs::remove_file(path).chain_err(|| ErrorKind::RemovingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub fn ensure_file_removed(name: &'static str, path: &Path) -> Result<()> {
    let result = fs::remove_file(path);
    if let Err(err) = &result {
        if err.kind() == io::ErrorKind::NotFound {
            return Ok(());
        }
    }
    result.chain_err(|| ErrorKind::RemovingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub fn read_dir(name: &'static str, path: &Path) -> Result<fs::ReadDir> {
    fs::read_dir(path).chain_err(|| ErrorKind::ReadingDirectory {
        name,
        path: PathBuf::from(path),
    })
}

pub fn open_browser(path: &Path) -> Result<()> {
    opener::open(path).chain_err(|| "couldn't open browser")
}

pub fn set_permissions(path: &Path, perms: fs::Permissions) -> Result<()> {
    fs::set_permissions(path, perms).chain_err(|| ErrorKind::SettingPermissions {
        path: PathBuf::from(path),
    })
}

pub fn file_size(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path).chain_err(|| ErrorKind::ReadingFile {
        name: "metadata for",
        path: PathBuf::from(path),
    })?;
    Ok(metadata.len())
}

pub fn make_executable(path: &Path) -> Result<()> {
    #[cfg(windows)]
    fn inner(_: &Path) -> Result<()> {
        Ok(())
    }
    #[cfg(not(windows))]
    fn inner(path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(path).chain_err(|| ErrorKind::SettingPermissions {
            path: PathBuf::from(path),
        })?;
        let mut perms = metadata.permissions();
        let mode = perms.mode();
        let new_mode = (mode & !0o777) | 0o755;

        // Check if permissions are ok already - #1638
        if mode == new_mode {
            return Ok(());
        }

        perms.set_mode(new_mode);
        set_permissions(path, perms)
    }

    inner(path)
}

pub fn current_dir() -> Result<PathBuf> {
    env::current_dir().chain_err(|| ErrorKind::LocatingWorkingDir)
}

pub fn current_exe() -> Result<PathBuf> {
    env::current_exe().chain_err(|| ErrorKind::LocatingWorkingDir)
}

pub fn to_absolute<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    current_dir().map(|mut v| {
        v.push(path);
        v
    })
}

// On windows, unlike std and cargo, rustup does *not* consider the
// HOME variable. If it did then the install dir would change
// depending on whether you happened to install under msys.
#[cfg(windows)]
pub fn home_dir() -> Option<PathBuf> {
    use std::ptr;
    use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::userenv::GetUserProfileDirectoryW;
    use winapi::um::winnt::TOKEN_READ;

    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| unsafe {
            let me = GetCurrentProcess();
            let mut token = ptr::null_mut();
            if OpenProcessToken(me, TOKEN_READ, &mut token) == 0 {
                return None;
            }
            let _g = scopeguard::guard(token, |h| {
                let _ = CloseHandle(h);
            });
            fill_utf16_buf(
                |buf, mut sz| {
                    match GetUserProfileDirectoryW(token, buf, &mut sz) {
                        0 if GetLastError() != ERROR_INSUFFICIENT_BUFFER => 0,
                        0 => sz,
                        _ => sz - 1, // sz includes the null terminator
                    }
                },
                os2path,
            )
            .ok()
        })
}

#[cfg(windows)]
fn os2path(s: &[u16]) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    PathBuf::from(OsString::from_wide(s))
}

#[cfg(windows)]
fn fill_utf16_buf<F1, F2, T>(mut f1: F1, f2: F2) -> io::Result<T>
where
    F1: FnMut(*mut u16, DWORD) -> DWORD,
    F2: FnOnce(&[u16]) -> T,
{
    use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
    use winapi::um::errhandlingapi::{GetLastError, SetLastError};

    // Start off with a stack buf but then spill over to the heap if we end up
    // needing more space.
    let mut stack_buf = [0u16; 512];
    let mut heap_buf = Vec::new();
    unsafe {
        let mut n = stack_buf.len();
        loop {
            let buf = if n <= stack_buf.len() {
                &mut stack_buf[..]
            } else {
                let extra = n - heap_buf.len();
                heap_buf.reserve(extra);
                heap_buf.set_len(n);
                &mut heap_buf[..]
            };

            // This function is typically called on windows API functions which
            // will return the correct length of the string, but these functions
            // also return the `0` on error. In some cases, however, the
            // returned "correct length" may actually be 0!
            //
            // To handle this case we call `SetLastError` to reset it to 0 and
            // then check it again if we get the "0 error value". If the "last
            // error" is still 0 then we interpret it as a 0 length buffer and
            // not an actual error.
            SetLastError(0);
            let k = match f1(buf.as_mut_ptr(), n as DWORD) {
                0 if GetLastError() == 0 => 0,
                0 => return Err(io::Error::last_os_error()),
                n => n,
            } as usize;
            if k == n && GetLastError() == ERROR_INSUFFICIENT_BUFFER {
                n *= 2;
            } else if k >= n {
                n = k;
            } else {
                return Ok(f2(&buf[..k]));
            }
        }
    }
}

#[cfg(unix)]
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

pub fn cargo_home() -> Result<PathBuf> {
    let cargo_home = env::var_os("CARGO_HOME");
    let cargo_home = cargo_home.filter(|v| !v.to_string_lossy().trim().is_empty());
    let cargo_home = if let Some(home) = cargo_home {
        let cwd = env::current_dir().chain_err(|| ErrorKind::GettingCwd)?;
        Some(cwd.join(home))
    } else {
        None
    };

    let user_home = home_dir().map(|p| p.join(".cargo"));
    cargo_home
        .or(user_home)
        .ok_or_else(|| ErrorKind::CargoHome.into())
}

// Creates a ~/.rustup folder
pub fn create_rustup_home() -> Result<()> {
    // If RUSTUP_HOME is set then don't make any assumptions about where it's
    // ok to put ~/.rustup
    if env::var_os("RUSTUP_HOME").is_some() {
        return Ok(());
    }

    let home = rustup_home_in_user_dir()?;
    fs::create_dir_all(&home).chain_err(|| "unable to create ~/.rustup")?;

    Ok(())
}

fn dot_dir(name: &str) -> Option<PathBuf> {
    home_dir().map(|p| p.join(name))
}

pub fn rustup_home_in_user_dir() -> Result<PathBuf> {
    dot_dir(".rustup").ok_or_else(|| ErrorKind::RustupHome.into())
}

pub fn rustup_home() -> Result<PathBuf> {
    let rustup_home_env = env::var_os("RUSTUP_HOME");

    let rustup_home = if rustup_home_env.is_some() {
        let cwd = env::current_dir().chain_err(|| ErrorKind::GettingCwd)?;
        rustup_home_env.clone().map(|home| cwd.join(home))
    } else {
        None
    };

    let user_home = dot_dir(".rustup");
    rustup_home
        .or(user_home)
        .ok_or_else(|| ErrorKind::RustupHome.into())
}

pub fn format_path_for_display(path: &str) -> String {
    let unc_present = path.find(r"\\?\");

    match unc_present {
        None => path.to_owned(),
        Some(_) => path[4..].to_owned(),
    }
}

/// Encodes a utf-8 string as a null-terminated UCS-2 string in bytes
#[cfg(windows)]
pub fn string_to_winreg_bytes(s: &str) -> Vec<u8> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStrExt;
    let v: Vec<_> = OsString::from(format!("{}\x00", s)).encode_wide().collect();
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2).to_vec() }
}

// This is used to decode the value of HKCU\Environment\PATH. If that
// key is not unicode (or not REG_SZ | REG_EXPAND_SZ) then this
// returns null.  The winreg library itself does a lossy unicode
// conversion.
#[cfg(windows)]
pub fn string_from_winreg_value(val: &winreg::RegValue) -> Option<String> {
    use std::slice;
    use winreg::enums::RegType;

    match val.vtype {
        RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
            // Copied from winreg
            let words = unsafe {
                slice::from_raw_parts(val.bytes.as_ptr() as *const u16, val.bytes.len() / 2)
            };
            let mut s = if let Ok(s) = String::from_utf16(words) {
                s
            } else {
                return None;
            };
            while s.ends_with('\u{0}') {
                s.pop();
            }
            Some(s)
        }
        _ => None,
    }
}

pub fn toolchain_sort<T: AsRef<str>>(v: &mut Vec<T>) {
    use semver::{Identifier, Version};

    fn special_version(ord: u64, s: &str) -> Version {
        Version {
            major: 0,
            minor: 0,
            patch: 0,
            pre: vec![Identifier::Numeric(ord), Identifier::AlphaNumeric(s.into())],
            build: vec![],
        }
    }

    fn toolchain_sort_key(s: &str) -> Version {
        if s.starts_with("stable") {
            special_version(0, s)
        } else if s.starts_with("beta") {
            special_version(1, s)
        } else if s.starts_with("nightly") {
            special_version(2, s)
        } else {
            Version::parse(&s.replace("_", "-")).unwrap_or_else(|_| special_version(3, s))
        }
    }

    v.sort_by(|a, b| {
        let a_str: &str = a.as_ref();
        let b_str: &str = b.as_ref();
        let a_key = toolchain_sort_key(a_str);
        let b_key = toolchain_sort_key(b_str);
        a_key.cmp(&b_key)
    });
}

fn rename(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
    fs::rename(src, dest).chain_err(|| ErrorKind::RenamingFile {
        name,
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

pub struct FileReaderWithProgress<'a> {
    fh: std::io::BufReader<std::fs::File>,
    notify_handler: &'a dyn Fn(Notification<'_>),
    nbytes: u64,
    flen: u64,
}

impl<'a> FileReaderWithProgress<'a> {
    pub fn new_file(path: &Path, notify_handler: &'a dyn Fn(Notification<'_>)) -> Result<Self> {
        let fh = match std::fs::File::open(path) {
            Ok(fh) => fh,
            Err(_) => Err(ErrorKind::ReadingFile {
                name: "downloaded",
                path: path.to_path_buf(),
            })?,
        };

        // Inform the tracker of the file size
        let flen = fh.metadata()?.len();
        (notify_handler)(Notification::DownloadContentLengthReceived(flen));

        let fh = BufReader::with_capacity(8 * 1024 * 1024, fh);

        Ok(FileReaderWithProgress {
            fh,
            notify_handler,
            nbytes: 0,
            flen,
        })
    }
}

impl<'a> std::io::Read for FileReaderWithProgress<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.fh.read(buf) {
            Ok(nbytes) => {
                self.nbytes += nbytes as u64;
                if nbytes != 0 {
                    (self.notify_handler)(Notification::DownloadDataReceived(&buf[0..nbytes]));
                }
                if (nbytes == 0) || (self.flen == self.nbytes) {
                    (self.notify_handler)(Notification::DownloadFinished);
                }
                Ok(nbytes)
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_home() {
        // CARGO_HOME unset, we'll get the default ending in /.cargo
        let cargo_home1 = cargo_home();
        let ch = format!("{}", cargo_home1.unwrap().display());
        assert!(ch.contains("/.cargo"));

        env::set_var("CARGO_HOME", "/test");
        let cargo_home2 = cargo_home();
        assert_eq!("/test", format!("{}", cargo_home2.unwrap().display()));
    }

    #[test]
    fn test_toochain_sort() {
        let expected = vec![
            "stable-x86_64-unknown-linux-gnu",
            "beta-x86_64-unknown-linux-gnu",
            "nightly-x86_64-unknown-linux-gnu",
            "1.0.0-x86_64-unknown-linux-gnu",
            "1.2.0-x86_64-unknown-linux-gnu",
            "1.8.0-x86_64-unknown-linux-gnu",
            "1.10.0-x86_64-unknown-linux-gnu",
        ];

        let mut v = vec![
            "1.8.0-x86_64-unknown-linux-gnu",
            "1.0.0-x86_64-unknown-linux-gnu",
            "nightly-x86_64-unknown-linux-gnu",
            "stable-x86_64-unknown-linux-gnu",
            "1.10.0-x86_64-unknown-linux-gnu",
            "beta-x86_64-unknown-linux-gnu",
            "1.2.0-x86_64-unknown-linux-gnu",
        ];

        toolchain_sort(&mut v);

        assert_eq!(expected, v);
    }
}
