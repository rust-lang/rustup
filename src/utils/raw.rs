#[cfg(not(windows))]
use std::env;
use std::{
    fs,
    fs::File,
    io,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    str,
};

use rand::RngExt;
use retry::{
    OperationResult,
    delay::{Fibonacci, jitter},
    retry,
};

#[cfg(not(windows))]
use crate::process::Process;

pub(crate) fn ensure_dir_exists<P: AsRef<Path>, F: FnOnce(&Path)>(
    path: P,
    callback: F,
) -> io::Result<bool> {
    if !is_directory(path.as_ref()) {
        callback(path.as_ref());
        fs::create_dir_all(path.as_ref()).map(|()| true)
    } else {
        Ok(false)
    }
}

pub(crate) fn is_directory<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).ok().as_ref().map(fs::Metadata::is_dir) == Some(true)
}

pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).ok().as_ref().map(fs::Metadata::is_file) == Some(true)
}

#[cfg(windows)]
pub fn open_dir_following_links(p: &Path) -> io::Result<File> {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};

    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;

    let mut options = OpenOptions::new();
    options.read(true);
    options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    options.open(p)
}

#[cfg(not(windows))]
pub fn open_dir_following_links(p: &Path) -> io::Result<File> {
    use std::fs::OpenOptions;

    let mut options = OpenOptions::new();
    options.read(true);
    options.open(p)
}

pub fn path_exists<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).is_ok()
}

pub(crate) fn read_locked_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    lock_shared(&file)?;

    let size = file
        .metadata()
        .map(|metadata| usize::try_from(metadata.len()).unwrap_or(usize::MAX))
        .ok();
    let mut contents = String::new();
    contents.try_reserve_exact(size.unwrap_or(0))?;
    file.read_to_string(&mut contents)?;

    Ok(contents)
}

pub(crate) fn random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| char::from(CHARSET[rng.random_range(..CHARSET.len())]))
        .collect()
}

pub fn write_file(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)?;

    Write::write_all(&mut file, contents.as_bytes())?;

    file.sync_data()?;

    Ok(())
}

pub(crate) fn write_locked_file(path: &Path, contents: &str) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        // Truncation must happen after the exclusive lock is held.
        .truncate(false)
        .open(path)?;

    lock(&file)?;
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    Write::write_all(&mut file, contents.as_bytes())?;
    file.sync_data()?;

    Ok(())
}

pub(crate) fn filter_file<F: FnMut(&str) -> bool>(
    src: &Path,
    dest: &Path,
    mut filter: F,
) -> io::Result<usize> {
    let src_file = File::open(src)?;
    let dest_file = File::create(dest)?;

    let mut reader = io::BufReader::new(src_file);
    let mut writer = io::BufWriter::new(dest_file);
    let mut removed = 0;

    for result in io::BufRead::lines(&mut reader) {
        let line = result?;
        if filter(&line) {
            writeln!(writer, "{line}")?;
        } else {
            removed += 1;
        }
    }

    writer.flush()?;

    Ok(removed)
}

pub fn append_file(dest: &Path, line: &str) -> io::Result<()> {
    let mut dest_file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(dest)?;

    writeln!(dest_file, "{line}")?;

    dest_file.sync_data()?;

    Ok(())
}

pub fn symlink_dir(src: &Path, dest: &Path) -> io::Result<()> {
    #[cfg(windows)]
    fn symlink_dir_inner(src: &Path, dest: &Path) -> io::Result<()> {
        // On Windows creating symlinks isn't allowed by default so if it fails
        // we fallback to creating a directory junction.
        // We prefer to use symlinks here because junction point paths, unlike symlinks,
        // must always be absolute. This makes moving the rustup directory difficult.
        std::os::windows::fs::symlink_dir(src, dest).or_else(|_| symlink_junction_inner(src, dest))
    }
    #[cfg(not(windows))]
    fn symlink_dir_inner(src: &Path, dest: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(src, dest)
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
    use std::{os::windows::ffi::OsStrExt, ptr};

    use windows_sys::Win32::{
        Foundation::GENERIC_WRITE,
        Storage::FileSystem::{
            CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            IO::DeviceIoControl, Ioctl::FSCTL_SET_REPARSE_POINT,
            SystemServices::IO_REPARSE_TAG_MOUNT_POINT,
        },
    };

    const MAXIMUM_REPARSE_DATA_BUFFER_SIZE: usize = 16 * 1024;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct REPARSE_MOUNTPOINT_DATA_BUFFER {
        ReparseTag: u32,
        ReparseDataLength: u32,
        Reserved: u16,
        ReparseTargetLength: u16,
        ReparseTargetMaximumLength: u16,
        Reserved1: u16,
        ReparseTarget: u16,
    }

    // We're using low-level APIs to create the junction, and these are more picky about paths.
    // For example, forward slashes cannot be used as a path separator, so we should try to
    // canonicalize the path first.
    let target = fs::canonicalize(target)?;

    fs::create_dir(junction)?;

    let path = windows::to_u16s(junction)?;

    unsafe {
        let h = CreateFileW(
            path.as_ptr(),
            GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            ptr::null_mut(),
        );

        let mut data = [0u8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE];
        let db = data.as_mut_ptr().cast::<REPARSE_MOUNTPOINT_DATA_BUFFER>();
        let buf = &mut (*db).ReparseTarget as *mut u16;
        let mut i = 0;
        // FIXME: this conversion is very hacky
        let v = br"\??\";
        let v = v.iter().map(|x| *x as u16);
        for c in v.chain(target.as_os_str().encode_wide().skip(4)) {
            *buf.offset(i) = c;
            i += 1;
        }
        *buf.offset(i) = 0;
        i += 1;
        (*db).ReparseTag = IO_REPARSE_TAG_MOUNT_POINT;
        (*db).ReparseTargetMaximumLength = (i * 2) as u16;
        (*db).ReparseTargetLength = ((i - 1) * 2) as u16;
        (*db).ReparseDataLength = (*db).ReparseTargetLength as u32 + 12;

        let mut ret = 0;
        let res = DeviceIoControl(
            h,
            FSCTL_SET_REPARSE_POINT,
            data.as_mut_ptr().cast(),
            (*db).ReparseDataLength + 8,
            ptr::null_mut(),
            0,
            &mut ret,
            ptr::null_mut(),
        );

        if res == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

fn is_retryable_dir_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::DirectoryNotEmpty
            | io::ErrorKind::PermissionDenied
            | io::ErrorKind::ResourceBusy
    )
}

pub fn remove_dir(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        #[cfg(windows)]
        {
            fs::remove_dir(path)
        }
        #[cfg(not(windows))]
        {
            fs::remove_file(path)
        }
    } else {
        let result = retry(Fibonacci::from_millis(10).map(jitter).take(10), || {
            // Again because remove_dir all doesn't delete write-only files on windows,
            // this is a custom implementation, more-or-less copied from cargo.
            // cc rust-lang/rust#31944
            // cc https://github.com/rust-lang/cargo/blob/master/tests/support/paths.rs#L52
            match remove_dir_all::remove_dir_all(path) {
                Ok(()) => OperationResult::Ok(()),
                Err(e) if is_retryable_dir_error(&e) => OperationResult::Retry(e),
                Err(e) => OperationResult::Err(e),
            }
        });

        // Best-effort sync of parent directory to ensure filesystem metadata consistency
        // after parallel deletion. Helps prevent "Directory not empty" errors on subsequent
        // operations. See https://github.com/rust-lang/rustup/issues/4657
        if result.is_ok()
            && let Some(parent) = path.parent()
        {
            let _ = open_dir_following_links(parent).and_then(|f| f.sync_all());
        }

        result.map_err(|e| e.error)
    }
}

pub(crate) fn copy_dir(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir(dest)?;
    for entry in src.read_dir()? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let src = entry.path();
        let dest = dest.join(entry.file_name());
        // Check for symlinks first - is_dir() follows symlinks
        if kind.is_symlink() {
            copy_symlink(&src, &dest)?;
        } else if kind.is_dir() {
            copy_dir(&src, &dest)?;
        } else {
            fs::copy(&src, &dest)?;
        }
    }
    Ok(())
}

/// Copy a symlink, preserving its target
fn copy_symlink(src: &Path, dest: &Path) -> io::Result<()> {
    let target = fs::read_link(src)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target, dest)
    }
    #[cfg(windows)]
    {
        // Determine symlink type by checking what the source symlink points to
        let meta = fs::metadata(src);
        if meta.map(|m| m.is_dir()).unwrap_or(false) {
            std::os::windows::fs::symlink_dir(&target, dest)
        } else {
            std::os::windows::fs::symlink_file(&target, dest)
        }
    }
}

#[cfg(not(windows))]
fn has_cmd(cmd: &str, process: &Process) -> bool {
    let cmd = format!("{}{}", cmd, env::consts::EXE_SUFFIX);
    let path = process.var_os("PATH").unwrap_or_default();
    env::split_paths(&path)
        .map(|p| p.join(&cmd))
        .any(|p| p.exists())
}

#[cfg(not(windows))]
pub(crate) fn find_cmd<'a>(cmds: &[&'a str], process: &Process) -> Option<&'a str> {
    cmds.iter().cloned().find(|&s| has_cmd(s, process))
}

#[cfg(windows)]
pub(crate) mod windows {
    use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt};

    pub(crate) fn to_u16s<S: AsRef<OsStr>>(s: S) -> io::Result<Vec<u16>> {
        fn inner(s: &OsStr) -> io::Result<Vec<u16>> {
            let mut maybe_result: Vec<u16> = s.encode_wide().collect();
            if maybe_result.contains(&0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "strings passed to WinAPI cannot contain NULs",
                ));
            }
            maybe_result.push(0);
            Ok(maybe_result)
        }
        inner(s.as_ref())
    }
}

/// File-locking support.
///
/// On most platforms this is a thin wrapper around `File::lock` and `File::lock_shared`.
/// However, this isn't supported on Solaris[^1] so we instead implement it ourselves
/// using `fcntl`.
///
/// [^1]: See [rust-lang/rust#157390](https://github.com/rust-lang/rust/issues/157390) for more
pub use flock::{lock, lock_shared, try_lock};

#[cfg(not(target_os = "solaris"))]
mod flock {
    use std::{
        fs::{File, TryLockError},
        io,
    };

    pub fn lock(file: &File) -> io::Result<()> {
        #[expect(clippy::disallowed_methods)]
        file.lock()
    }

    pub fn lock_shared(file: &File) -> io::Result<()> {
        #[expect(clippy::disallowed_methods)]
        file.lock_shared()
    }

    pub fn try_lock(file: &File) -> Result<(), TryLockError> {
        #[expect(clippy::disallowed_methods)]
        file.try_lock()
    }
}

#[cfg(target_os = "solaris")]
mod flock {
    // Code adapted from Cargo:
    // https://github.com/rust-lang/cargo/blob/b57863321f8d0a0f76fcf9ad636b6e39b024d0c0/src/util/flock.rs
    use std::{
        fs::{File, TryLockError},
        io, mem,
        os::unix::io::AsRawFd,
    };

    pub fn lock(file: &File) -> io::Result<()> {
        fcntl_lock(file, libc::F_WRLCK, FcntlCmd::F_SETLKW)
    }

    pub fn lock_shared(file: &File) -> io::Result<()> {
        fcntl_lock(file, libc::F_RDLCK, FcntlCmd::F_SETLKW)
    }

    pub fn try_lock(file: &File) -> Result<(), TryLockError> {
        match fcntl_lock(file, libc::F_WRLCK, FcntlCmd::F_SETLK) {
            Ok(()) => Ok(()),
            Err(e) if is_would_block(&e) => Err(TryLockError::WouldBlock),
            Err(e) => Err(TryLockError::Error(e)),
        }
    }

    fn fcntl_lock(file: &File, lock_type: libc::c_short, cmd: FcntlCmd) -> io::Result<()> {
        let mut lock = flock_for_whole_file(lock_type);
        loop {
            // SAFETY:
            // fcntl takes a variable number of additonal arguments depending on `cmd`.
            // In our case `cmd` is either F_SETLK or F_SETLKW which takes a single
            // argument, which is a pointer to a mutable `flock`.
            // We have exclusive ownership of `lock` so it's safe to pass.
            let result = unsafe { libc::fcntl(file.as_raw_fd(), cmd as libc::c_int, &mut lock) };
            if result != -1 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if cmd == FcntlCmd::F_SETLKW && error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
    }

    #[repr(i32)]
    #[derive(Copy, Clone, Eq, PartialEq)]
    #[expect(nonstandard_style)]
    enum FcntlCmd {
        F_SETLK = libc::F_SETLK,
        F_SETLKW = libc::F_SETLKW,
    }

    fn flock_for_whole_file(lock_type: libc::c_short) -> libc::flock {
        // SAFETY: libc::flock is a plain C data structure, so can be zeroed.
        let mut lock = unsafe { mem::zeroed::<libc::flock>() };
        lock.l_type = lock_type;
        lock.l_whence = libc::SEEK_SET as libc::c_short;
        lock.l_start = 0;
        lock.l_len = 0;
        lock
    }

    fn is_would_block(error: &io::Error) -> bool {
        matches!(error.raw_os_error(), Some(libc::EACCES | libc::EAGAIN))
            || error.kind() == io::ErrorKind::WouldBlock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_file_round_trip_truncates_previous_contents() -> io::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join("state.toml");

        write_locked_file(&path, "last_release_notified_secs = 123456\n")?;
        write_locked_file(&path, "version = \"12\"\n")?;

        assert_eq!(read_locked_file(&path)?, "version = \"12\"\n");
        Ok(())
    }
}
