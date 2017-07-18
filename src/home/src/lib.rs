extern crate rand;
extern crate scopeguard;
#[macro_use]
extern crate error_chain;
extern crate rustc_serialize;
extern crate sha2;
extern crate url;
extern crate toml;
extern crate download;
extern crate semver;
#[macro_use]
extern crate lazy_static;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;
#[cfg(windows)]
extern crate shell32;
#[cfg(windows)]
extern crate ole32;
#[cfg(windows)]
extern crate kernel32;
#[cfg(windows)]
extern crate advapi32;
#[cfg(windows)]
extern crate userenv;

#[cfg(unix)]
extern crate libc;

#[cfg(windows)]
use winapi::DWORD;
use std::path::PathBuf;
use std::io;
use std::error::Error as StdError;
use std::fmt;
use std::env;

// On windows, unlike std and cargo, rustup does *not* consider the
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
    use scopeguard;

    ::std::env::var_os("USERPROFILE").map(PathBuf::from).or_else(|| unsafe {
        let me = GetCurrentProcess();
        let mut token = ptr::null_mut();
        if OpenProcessToken(me, TOKEN_READ, &mut token) == 0 {
            return None;
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
    use std::ffi::OsString;
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

pub fn cargo_home() -> io::Result<PathBuf> {
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
            vv.contains(r".multirust\cargo") ||
            vv.trim().is_empty() {
           None
       } else {
           Some(v)
       }
    } else {
        None
    };

    let cwd = env::current_dir()?;
    let env_cargo_home = env_var.map(|home| cwd.join(home));
    let home_dir = home_dir()
        .ok_or(io::Error::new(io::ErrorKind::Other, "couldn't find home dir"));
    let user_home = home_dir.map(|p| p.join(".cargo"));

    if let Some(p) = env_cargo_home {
        Ok(p)
    } else {
        user_home
    }
}

pub fn rustup_home() -> io::Result<PathBuf> {
    let env_var = env::var_os("RUSTUP_HOME");
    let cwd = env::current_dir()?;
    let env_rustup_home = env_var.map(|home| cwd.join(home));
    let home_dir = home_dir()
        .ok_or(io::Error::new(io::ErrorKind::Other, "couldn't find home dir"));

    let user_home = if use_rustup_dir() {
        home_dir.map(|d| d.join(".rustup"))
    } else {
        home_dir.map(|d| d.join(".multirust"))
    };

    if let Some(p) = env_rustup_home {
        Ok(p)
    } else {
        user_home
    }
}

fn use_rustup_dir() -> bool {
    fn rustup_dir() -> Option<PathBuf> {
        home_dir().map(|p| p.join(".rustup"))
    }

    fn multirust_dir() -> Option<PathBuf> {
        home_dir().map(|p| p.join(".multirust"))
    }

    fn rustup_dir_exists() -> bool {
        rustup_dir().map(|p| p.exists()).unwrap_or(false)
    }

    fn multirust_dir_exists() -> bool {
        multirust_dir().map(|p| p.exists()).unwrap_or(false)
    }

    fn rustup_old_version_exists() -> bool {
        rustup_dir()
            .map(|p| p.join("rustup-version").exists())
            .unwrap_or(false)
    }

    !rustup_old_version_exists()
        && (rustup_dir_exists() || !multirust_dir_exists())
}
