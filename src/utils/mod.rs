//!  Utility functions for Rustup

use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::ops::{BitAnd, BitAndAssign};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use anyhow::{Context, Result, anyhow, bail};
use retry::delay::{Fibonacci, jitter};
use retry::{OperationResult, retry};
use url::Url;

use crate::errors::*;
use crate::process::Process;

#[cfg(not(windows))]
pub(crate) use crate::utils::raw::find_cmd;
pub use crate::utils::raw::{is_file, path_exists};
pub(crate) use crate::utils::{notifications::Notification, raw::is_directory};

pub(crate) mod notifications;
pub(crate) mod notify;
pub mod raw;
pub(crate) mod units;

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct ExitCode(pub i32);

impl BitAnd for ExitCode {
    type Output = Self;

    // If `self` is `0` (success), yield `rhs`.
    fn bitand(self, rhs: Self) -> Self::Output {
        match self.0 {
            0 => rhs,
            _ => self,
        }
    }
}

impl BitAndAssign for ExitCode {
    // If `self` is `0` (success), set `self` to `rhs`.
    fn bitand_assign(&mut self, rhs: Self) {
        if self.0 == 0 {
            *self = rhs
        }
    }
}

impl From<ExitStatus> for ExitCode {
    fn from(status: ExitStatus) -> Self {
        Self(match status.success() {
            true => 0,
            false => status.code().unwrap_or(1),
        })
    }
}

pub fn ensure_dir_exists<'a, N>(
    name: &'static str,
    path: &'a Path,
    notify_handler: &'a dyn Fn(N),
) -> Result<bool>
where
    N: From<Notification<'a>>,
{
    raw::ensure_dir_exists(path, |_| {
        notify_handler(Notification::CreatingDirectory(name, path).into())
    })
    .with_context(|| RustupError::CreatingDirectory {
        name,
        path: PathBuf::from(path),
    })
}

pub fn read_file(name: &'static str, path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| RustupError::ReadingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub fn write_file(name: &'static str, path: &Path, contents: &str) -> Result<()> {
    raw::write_file(path, contents).with_context(|| RustupError::WritingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub(crate) fn append_file(name: &'static str, path: &Path, line: &str) -> Result<()> {
    raw::append_file(path, line).with_context(|| RustupError::WritingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub(crate) fn write_line(
    name: &'static str,
    mut file: impl Write,
    path: &Path,
    line: &str,
) -> Result<()> {
    writeln!(file, "{line}").with_context(|| RustupError::WritingFile {
        name,
        path: path.to_path_buf(),
    })
}

pub(crate) fn write_str(name: &'static str, file: &mut File, path: &Path, s: &str) -> Result<()> {
    write!(file, "{s}").with_context(|| RustupError::WritingFile {
        name,
        path: path.to_path_buf(),
    })
}

pub(crate) fn filter_file<F: FnMut(&str) -> bool>(
    name: &'static str,
    src: &Path,
    dest: &Path,
    filter: F,
) -> Result<usize> {
    raw::filter_file(src, dest, filter).with_context(|| {
        format!(
            "could not copy {} file from '{}' to '{}'",
            name,
            src.display(),
            dest.display()
        )
    })
}

pub(crate) fn canonicalize_path<'a, N>(path: &'a Path, notify_handler: &dyn Fn(N)) -> PathBuf
where
    N: From<Notification<'a>>,
{
    fs::canonicalize(path).unwrap_or_else(|_| {
        notify_handler(Notification::NoCanonicalPath(path).into());
        PathBuf::from(path)
    })
}

pub(crate) fn parse_url(url: &str) -> Result<Url> {
    Url::parse(url).with_context(|| format!("failed to parse url: {url}"))
}

pub(crate) fn assert_is_file(path: &Path) -> Result<()> {
    if !is_file(path) {
        Err(anyhow!(format!("not a file: '{}'", path.display())))
    } else {
        Ok(())
    }
}

pub(crate) fn assert_is_directory(path: &Path) -> Result<()> {
    if !is_directory(path) {
        Err(anyhow!(format!("not a directory: '{}'", path.display())))
    } else {
        Ok(())
    }
}

pub(crate) fn symlink_dir<'a, N>(
    src: &'a Path,
    dest: &'a Path,
    notify_handler: &dyn Fn(N),
) -> Result<()>
where
    N: From<Notification<'a>>,
{
    notify_handler(Notification::LinkingDirectory(src, dest).into());
    raw::symlink_dir(src, dest).with_context(|| {
        format!(
            "could not create link from '{}' to '{}'",
            src.display(),
            dest.display()
        )
    })
}

/// Attempts to symlink a file, falling back to hard linking if that fails.
///
/// If `dest` already exists then it will be replaced.
pub(crate) fn symlink_or_hardlink_file(src: &Path, dest: &Path) -> Result<()> {
    let _ = fs::remove_file(dest);
    // Use a relative symlink path if the src and dest are in the same directory.
    let symlink_target = if src.parent() == dest.parent() {
        src.file_name().map(Path::new).unwrap_or(src)
    } else {
        src
    };
    // The error is only used by macos
    let Err(_err) = symlink_file(symlink_target, dest) else {
        return Ok(());
    };

    // Some mac filesystems can do hardlinks to symlinks, some can't.
    // See rust-lang/rustup#3136 for why it's better never to use them.
    #[cfg(target_os = "macos")]
    if fs::symlink_metadata(src)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(_err);
    }

    hardlink_file(src, dest)
}

pub fn hardlink_file(src: &Path, dest: &Path) -> Result<()> {
    fs::hard_link(src, dest).with_context(|| RustupError::LinkingFile {
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

#[cfg(unix)]
fn symlink_file(src: &Path, dest: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dest).with_context(|| RustupError::LinkingFile {
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

#[cfg(windows)]
fn symlink_file(src: &Path, dest: &Path) -> Result<()> {
    std::os::windows::fs::symlink_file(src, dest).with_context(|| RustupError::LinkingFile {
        src: PathBuf::from(src),
        dest: PathBuf::from(dest),
    })
}

pub(crate) fn copy_dir<'a, N>(
    src: &'a Path,
    dest: &'a Path,
    notify_handler: &dyn Fn(N),
) -> Result<()>
where
    N: From<Notification<'a>>,
{
    notify_handler(Notification::CopyingDirectory(src, dest).into());
    raw::copy_dir(src, dest).with_context(|| {
        format!(
            "could not copy directory from '{}' to '{}'",
            src.display(),
            dest.display()
        )
    })
}

pub(crate) fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(src).with_context(|| RustupError::ReadingFile {
        name: "metadata for",
        path: PathBuf::from(src),
    })?;
    if metadata.file_type().is_symlink() {
        symlink_file(src, dest).map(|_| ())
    } else {
        fs::copy(src, dest)
            .with_context(|| {
                format!(
                    "could not copy file from '{}' to '{}'",
                    src.display(),
                    dest.display()
                )
            })
            .map(|_| ())
    }
}

pub(crate) fn remove_dir<'a, N>(
    name: &'static str,
    path: &'a Path,
    notify_handler: &dyn Fn(N),
) -> Result<()>
where
    N: From<Notification<'a>>,
{
    notify_handler(Notification::RemovingDirectory(name, path).into());
    raw::remove_dir(path).with_context(|| RustupError::RemovingDirectory {
        name,
        path: PathBuf::from(path),
    })
}

pub fn remove_file(name: &'static str, path: &Path) -> Result<()> {
    // Most files we go to remove won't ever be in use. Some, like proxies, may
    // be for indefinite periods, and this will mean we are slower to error and
    // have the user fix the issue. Others, like the setup binary, are
    // transiently in use, and this wait loop will fix the issue transparently
    // for a rare performance hit.
    retry(
        Fibonacci::from_millis(1).map(jitter).take(10),
        || match fs::remove_file(path) {
            Ok(()) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::PermissionDenied => OperationResult::Retry(e),
                _ => OperationResult::Err(e),
            },
        },
    )
    .with_context(|| RustupError::RemovingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub(crate) fn ensure_file_removed(name: &'static str, path: &Path) -> Result<()> {
    let result = remove_file(name, path);
    if let Err(err) = &result {
        if let Some(retry::Error { error: e, .. }) = err.downcast_ref::<retry::Error<io::Error>>() {
            if e.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
        }
    }
    result.with_context(|| RustupError::RemovingFile {
        name,
        path: PathBuf::from(path),
    })
}

pub(crate) fn read_dir(name: &'static str, path: &Path) -> Result<fs::ReadDir> {
    fs::read_dir(path).with_context(|| RustupError::ReadingDirectory {
        name,
        path: PathBuf::from(path),
    })
}

pub(crate) fn open_browser(path: impl AsRef<OsStr>) -> Result<()> {
    opener::open_browser(path).context("couldn't open browser")
}

#[cfg(not(windows))]
fn set_permissions(path: &Path, perms: fs::Permissions) -> Result<()> {
    fs::set_permissions(path, perms).map_err(|e| {
        RustupError::SettingPermissions {
            p: PathBuf::from(path),
            source: e,
        }
        .into()
    })
}

pub fn file_size(path: &Path) -> Result<u64> {
    Ok(fs::metadata(path)
        .with_context(|| RustupError::ReadingFile {
            name: "metadata for",
            path: PathBuf::from(path),
        })?
        .len())
}

pub(crate) fn make_executable(path: &Path) -> Result<()> {
    #[allow(clippy::unnecessary_wraps)]
    #[cfg(windows)]
    fn inner(_: &Path) -> Result<()> {
        Ok(())
    }
    #[cfg(not(windows))]
    fn inner(path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(path).map_err(|e| RustupError::SettingPermissions {
            p: PathBuf::from(path),
            source: e,
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

pub fn current_exe() -> Result<PathBuf> {
    env::current_exe().context(RustupError::LocatingWorkingDir)
}

pub(crate) fn format_path_for_display(path: &str) -> String {
    let unc_present = path.find(r"\\?\");

    match unc_present {
        None => path.to_owned(),
        Some(_) => path[4..].to_owned(),
    }
}

#[cfg(target_os = "linux")]
fn copy_and_delete<'a, N>(
    name: &'static str,
    src: &'a Path,
    dest: &'a Path,
    notify_handler: &'a dyn Fn(N),
) -> Result<()>
where
    N: From<Notification<'a>>,
{
    // https://github.com/rust-lang/rustup/issues/1239
    // This uses std::fs::copy() instead of the faster std::fs::rename() to
    // avoid cross-device link errors.
    if src.is_dir() {
        copy_dir(src, dest, notify_handler).and(remove_dir_all::remove_dir_all(src).with_context(
            || RustupError::RemovingDirectory {
                name,
                path: PathBuf::from(src),
            },
        ))
    } else {
        copy_file(src, dest).and(remove_file(name, src))
    }
}

pub fn rename<'a, N>(
    name: &'static str,
    src: &'a Path,
    dest: &'a Path,
    notify_handler: &'a dyn Fn(N),
    #[allow(unused_variables)] // Only used on Linux
    process: &Process,
) -> Result<()>
where
    N: From<Notification<'a>>,
{
    // https://github.com/rust-lang/rustup/issues/1870
    // 21 fib steps from 1 sums to ~28 seconds, hopefully more than enough
    // for our previous poor performance that avoided the race condition with
    // McAfee and Norton.
    #[cfg(target_os = "linux")]
    use libc::EXDEV;
    retry(
        Fibonacci::from_millis(1).map(jitter).take(26),
        || match fs::rename(src, dest) {
            Ok(()) => OperationResult::Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::PermissionDenied => {
                    notify_handler(Notification::RenameInUse(src, dest).into());
                    OperationResult::Retry(e)
                }
                #[cfg(target_os = "linux")]
                _ if process.var_os("RUSTUP_PERMIT_COPY_RENAME").is_some()
                    && Some(EXDEV) == e.raw_os_error() =>
                {
                    match copy_and_delete(name, src, dest, notify_handler) {
                        Ok(()) => OperationResult::Ok(()),
                        Err(_) => OperationResult::Err(e),
                    }
                }
                _ => OperationResult::Err(e),
            },
        },
    )
    .with_context(|| {
        format!(
            "could not rename {} file from '{}' to '{}'",
            name,
            src.display(),
            dest.display()
        )
    })
}

pub(crate) fn delete_dir_contents_following_links(dir_path: &Path) {
    use remove_dir_all::RemoveDir;

    match raw::open_dir_following_links(dir_path).and_then(|mut p| p.remove_dir_contents(None)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            panic!("Unable to clean up {}: {:?}", dir_path.display(), e);
        }
        _ => {}
    }
}

pub(crate) struct FileReaderWithProgress<'a> {
    fh: io::BufReader<File>,
    notify_handler: &'a dyn Fn(Notification<'_>),
    nbytes: u64,
    flen: u64,
}

impl<'a> FileReaderWithProgress<'a> {
    pub(crate) fn new_file(
        path: &Path,
        notify_handler: &'a dyn Fn(Notification<'_>),
    ) -> Result<Self> {
        let fh = match File::open(path) {
            Ok(fh) => fh,
            Err(_) => {
                bail!(RustupError::ReadingFile {
                    name: "downloaded",
                    path: path.to_path_buf(),
                })
            }
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

impl io::Read for FileReaderWithProgress<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
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

// search user database to get home dir of euid user
#[cfg(unix)]
pub(crate) fn home_dir_from_passwd() -> Option<PathBuf> {
    use std::ffi::{CStr, OsString};
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStringExt;
    use std::ptr;
    unsafe {
        let init_size = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
            -1 => 1024,
            n => n as usize,
        };
        let mut buf = Vec::with_capacity(init_size);
        let mut pwd: MaybeUninit<libc::passwd> = MaybeUninit::uninit();
        let mut pwdp = ptr::null_mut();
        match libc::getpwuid_r(
            libc::geteuid(),
            pwd.as_mut_ptr(),
            buf.as_mut_ptr(),
            buf.capacity(),
            &mut pwdp,
        ) {
            0 if !pwdp.is_null() => {
                let pwd = pwd.assume_init();
                let bytes = CStr::from_ptr(pwd.pw_dir).to_bytes().to_vec();
                let pw_dir = OsString::from_vec(bytes);
                Some(PathBuf::from(pw_dir))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_file() {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let f_path = tempdir.path().join("f");
        File::create(&f_path).unwrap();

        assert!(f_path.exists());
        assert!(remove_file("f", &f_path).is_ok());

        assert!(!f_path.exists());
        let result = remove_file("f", &f_path);
        let err = result.unwrap_err();

        match err.downcast_ref::<RustupError>() {
            Some(RustupError::RemovingFile { name, path }) => {
                assert_eq!(*name, "f");
                assert_eq!(path.clone(), f_path);
            }
            _ => panic!("Expected an error removing file"),
        }
    }

    #[test]
    fn test_ensure_file_removed() {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let f_path = tempdir.path().join("f");
        File::create(&f_path).unwrap();

        assert!(f_path.exists());
        assert!(ensure_file_removed("f", &f_path).is_ok());

        assert!(!f_path.exists());
        assert!(ensure_file_removed("f", &f_path).is_ok());
    }
}
