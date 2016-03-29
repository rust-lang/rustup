use errors::Result;
use std::path::{Path, PathBuf};
use std::fs;
use std::io;
use std::process::Command;
use std::ffi::OsString;
use std::env;
use hyper;
use openssl::crypto::hash::Hasher;
use notify::Notifyable;
use errors::{Error, Notification, NotifyHandler};
use raw;
#[cfg(windows)]
use winapi::DWORD;

pub use raw::{is_directory, is_file, path_exists, if_not_empty, random_string, prefix_arg,
                    has_cmd, find_cmd};


pub fn ensure_dir_exists(name: &'static str,
                         path: &Path,
                         notify_handler: NotifyHandler)
                         -> Result<bool> {
    raw::ensure_dir_exists(path,
                           |p| notify_handler.call(Notification::CreatingDirectory(name, p)))
        .map_err(|e| {
            Error::CreatingDirectory {
                name: name,
                path: PathBuf::from(path),
                error: e,
            }
        })
}

pub fn read_file(name: &'static str, path: &Path) -> Result<String> {
    raw::read_file(path).map_err(|e| {
        Error::ReadingFile {
            name: name,
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn write_file(name: &'static str, path: &Path, contents: &str) -> Result<()> {
    raw::write_file(path, contents).map_err(|e| {
        Error::WritingFile {
            name: name,
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn append_file(name: &'static str, path: &Path, line: &str) -> Result<()> {
    raw::append_file(path, line).map_err(|e| {
        Error::WritingFile {
            name: name,
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn rename_file(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
    fs::rename(src, dest).map_err(|e| {
        Error::RenamingFile {
            name: name,
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
            error: e,
        }
    })
}

pub fn rename_dir(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
    fs::rename(src, dest).map_err(|e| {
        Error::RenamingDirectory {
            name: name,
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
            error: e,
        }
    })
}

pub fn filter_file<F: FnMut(&str) -> bool>(name: &'static str,
                                           src: &Path,
                                           dest: &Path,
                                           filter: F)
                                           -> Result<usize> {
    raw::filter_file(src, dest, filter).map_err(|e| {
        Error::FilteringFile {
            name: name,
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
            error: e,
        }
    })
}

pub fn match_file<T, F: FnMut(&str) -> Option<T>>(name: &'static str,
                                                  src: &Path,
                                                  f: F)
                                                  -> Result<Option<T>> {
    raw::match_file(src, f).map_err(|e| {
        Error::ReadingFile {
            name: name,
            path: PathBuf::from(src),
            error: e,
        }
    })
}

pub fn canonicalize_path(path: &Path, notify_handler: NotifyHandler) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        notify_handler.call(Notification::NoCanonicalPath(path));
        PathBuf::from(path)
    })
}

pub fn tee_file<W: io::Write>(name: &'static str, path: &Path, w: &mut W) -> Result<()> {
    raw::tee_file(path, w).map_err(|e| {
        Error::ReadingFile {
            name: name,
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn download_file(url: hyper::Url,
                     path: &Path,
                     hasher: Option<&mut Hasher>,
                     notify_handler: NotifyHandler)
                     -> Result<()> {
    notify_handler.call(Notification::DownloadingFile(&url, path));
    raw::download_file(url.clone(), path, hasher, notify_handler).map_err(|e| {
        Error::DownloadingFile {
            url: url,
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn parse_url(url: &str) -> Result<hyper::Url> {
    hyper::Url::parse(url).map_err(|_| Error::InvalidUrl { url: url.to_owned() })
}

pub fn cmd_status(name: &'static str, cmd: &mut Command) -> Result<()> {
    raw::cmd_status(cmd).map_err(|e| {
        Error::RunningCommand {
            name: OsString::from(name),
            error: e,
        }
    })
}

pub fn assert_is_file(path: &Path) -> Result<()> {
    if !is_file(path) {
        Err(Error::NotAFile { path: PathBuf::from(path) })
    } else {
        Ok(())
    }
}

pub fn assert_is_directory(path: &Path) -> Result<()> {
    if !is_directory(path) {
        Err(Error::NotADirectory { path: PathBuf::from(path) })
    } else {
        Ok(())
    }
}

pub fn symlink_dir(src: &Path, dest: &Path, notify_handler: NotifyHandler) -> Result<()> {
    notify_handler.call(Notification::LinkingDirectory(src, dest));
    raw::symlink_dir(src, dest).map_err(|e| {
        Error::LinkingDirectory {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
            error: e,
        }
    })
}

pub fn symlink_file(src: &Path, dest: &Path) -> Result<()> {
    raw::symlink_file(src, dest).map_err(|e| {
        Error::LinkingFile {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
            error: e,
        }
    })
}

pub fn hardlink_file(src: &Path, dest: &Path) -> Result<()> {
    raw::hardlink(src, dest).map_err(|e| {
        Error::LinkingFile {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
            error: e,
        }
    })
}

pub fn copy_dir(src: &Path, dest: &Path, notify_handler: NotifyHandler) -> Result<()> {
    notify_handler.call(Notification::CopyingDirectory(src, dest));
    raw::copy_dir(src, dest).map_err(|e| {
        Error::CopyingDirectory {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
            error: e,
        }
    })
}

pub fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    fs::copy(src, dest)
        .map_err(|e| {
            Error::CopyingFile {
                src: PathBuf::from(src),
                dest: PathBuf::from(dest),
                error: e,
            }
        })
        .map(|_| ())
}

pub fn remove_dir(name: &'static str, path: &Path, notify_handler: NotifyHandler) -> Result<()> {
    notify_handler.call(Notification::RemovingDirectory(name, path));
    raw::remove_dir(path).map_err(|e| {
        Error::RemovingDirectory {
            name: name,
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn remove_file(name: &'static str, path: &Path) -> Result<()> {
    fs::remove_file(path).map_err(|e| {
        Error::RemovingFile {
            name: name,
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn read_dir(name: &'static str, path: &Path) -> Result<fs::ReadDir> {
    fs::read_dir(path).map_err(|e| {
        Error::ReadingDirectory {
            name: name,
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn open_browser(path: &Path) -> Result<()> {
    match raw::open_browser(path) {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::OpeningBrowser { error: None }),
        Err(e) => Err(Error::OpeningBrowser { error: Some(e) }),
    }
}

pub fn set_permissions(path: &Path, perms: fs::Permissions) -> Result<()> {
    fs::set_permissions(path, perms).map_err(|e| {
        Error::SettingPermissions {
            path: PathBuf::from(path),
            error: e,
        }
    })
}

pub fn make_executable(path: &Path) -> Result<()> {
    #[cfg(windows)]
    fn inner(_: &Path) -> Result<()> {
        Ok(())
    }
    #[cfg(not(windows))]
    fn inner(path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let metadata = try!(fs::metadata(path).map_err(|e| {
            Error::SettingPermissions {
                path: PathBuf::from(path),
                error: e,
            }
        }));
        let mut perms = metadata.permissions();
        let new_mode = (perms.mode() & !0o777) | 0o755;
        perms.set_mode(new_mode);

        set_permissions(path, perms)
    }

    inner(path)
}

pub fn current_dir() -> Result<PathBuf> {
    env::current_dir().map_err(|e| Error::LocatingWorkingDir { error: e })
}

pub fn current_exe() -> Result<PathBuf> {
    env::current_exe().map_err(|e| Error::LocatingWorkingDir { error: e })
}

pub fn to_absolute<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    current_dir().map(|mut v| {
        v.push(path);
        v
    })
}

// On windows, unlike std and cargo, multirust does *not* consider the
// HOME variable. If it did then the install dir would change
// depending on whether you happened to install under msys.
#[cfg(windows)]
pub fn home_dir() -> Option<PathBuf> {
    use std::ptr;
    use kernel32::{GetCurrentProcess, GetLastError, CloseHandle};
    use advapi32::OpenProcessToken;
    use userenv::GetUserProfileDirectoryW;
    use winapi::ERROR_INSUFFICIENT_BUFFER;
    use winapi::winnt::TOKEN_READ;

    ::std::env::var_os("USERPROFILE").map(PathBuf::from).or_else(|| unsafe {
        let me = GetCurrentProcess();
        let mut token = ptr::null_mut();
        if OpenProcessToken(me, TOKEN_READ, &mut token) == 0 {
            return None
        }
        let _g = scopeguard::guard(token, |h| { let _ = CloseHandle(*h); });
        fill_utf16_buf(|buf, mut sz| {
            match GetUserProfileDirectoryW(token, buf, &mut sz) {
                0 if GetLastError() != ERROR_INSUFFICIENT_BUFFER => 0,
                0 => sz,
                _ => sz - 1, // sz includes the null terminator
            }
        }, os2path).ok()
    })
}

#[cfg(windows)]
fn os2path(s: &[u16]) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    PathBuf::from(OsString::from_wide(s))
}

#[cfg(windows)]
fn fill_utf16_buf<F1, F2, T>(mut f1: F1, f2: F2) -> io::Result<T>
    where F1: FnMut(*mut u16, DWORD) -> DWORD,
          F2: FnOnce(&[u16]) -> T
{
    use kernel32::{GetLastError, SetLastError};
    use winapi::{ERROR_INSUFFICIENT_BUFFER};

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
                return Ok(f2(&buf[..k]))
            }
        }
    }
}

#[cfg(unix)]
pub fn home_dir() -> Option<PathBuf> {
    ::std::env::home_dir()
}

pub fn cargo_home() -> Result<PathBuf> {
    let env_var = env::var_os("CARGO_HOME");

    // NB: During the multirust-rs -> rustup transition the install
    // dir changed from ~/.multirust/bin to ~/.cargo/bin. Because
    // multirust used to explicitly set CARGO_HOME it's possible to
    // get here when e.g. installing under `cargo run` and decide to
    // install to the wrong place. This check is to make the
    // multirust-rs to rustup upgrade seamless.
    let env_var = if let Some(v) = env_var {
       let vv = v.to_string_lossy().to_string();
       if vv.contains(".multirust/cargo") ||
            vv.contains(r".multirust\cargo") {
           None
       } else {
           Some(v)
       }
    } else {
        None
    };

    let cwd = try!(env::current_dir().map_err(|_| Error::CargoHome));
    let cargo_home = env_var.clone().map(|home| {
        cwd.join(home)
    });
    let user_home = home_dir().map(|p| p.join(".cargo"));
    cargo_home.or(user_home).ok_or(Error::CargoHome)
}

pub fn multirust_home() -> Result<PathBuf> {
    let cwd = try!(env::current_dir().map_err(|_| Error::MultirustHome));
    let multirust_home = env::var_os("RUSTUP_HOME").map(|home| {
        cwd.join(home)
    });
    let user_home = home_dir().map(|p| p.join(".multirust"));
    multirust_home.or(user_home).ok_or(Error::MultirustHome)
}
