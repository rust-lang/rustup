#[cfg(not(windows))]
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::str;

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
pub fn open_dir_following_links(p: &Path) -> std::io::Result<File> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;

    let mut options = OpenOptions::new();
    options.read(true);
    options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    options.open(p)
}

#[cfg(not(windows))]
pub fn open_dir_following_links(p: &Path) -> std::io::Result<File> {
    use std::fs::OpenOptions;

    let mut options = OpenOptions::new();
    options.read(true);
    options.open(p)
}

pub fn path_exists<P: AsRef<Path>>(path: P) -> bool {
    fs::metadata(path).is_ok()
}

pub(crate) fn random_string(length: usize) -> String {
    use rand::Rng;
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

    io::Write::write_all(&mut file, contents.as_bytes())?;

    file.sync_data()?;

    Ok(())
}

pub(crate) fn filter_file<F: FnMut(&str) -> bool>(
    src: &Path,
    dest: &Path,
    mut filter: F,
) -> io::Result<usize> {
    let src_file = fs::File::open(src)?;
    let dest_file = fs::File::create(dest)?;

    let mut reader = io::BufReader::new(src_file);
    let mut writer = io::BufWriter::new(dest_file);
    let mut removed = 0;

    for result in io::BufRead::lines(&mut reader) {
        let line = result?;
        if filter(&line) {
            writeln!(writer, "{}", &line)?;
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
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Storage::FileSystem::*;
    use windows_sys::Win32::System::IO::*;
    use windows_sys::Win32::System::Ioctl::FSCTL_SET_REPARSE_POINT;
    use windows_sys::Win32::System::SystemServices::*;

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
        // Again because remove_dir all doesn't delete write-only files on windows,
        // this is a custom implementation, more-or-less copied from cargo.
        // cc rust-lang/rust#31944
        // cc https://github.com/rust-lang/cargo/blob/master/tests/support/paths.rs#L52
        remove_dir_all::remove_dir_all(path)
    }
}

pub(crate) fn copy_dir(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir(dest)?;
    for entry in src.read_dir()? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let src = entry.path();
        let dest = dest.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&src, &dest)?;
        } else {
            fs::copy(&src, &dest)?;
        }
    }
    Ok(())
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
    use std::ffi::OsStr;
    use std::io;
    use std::os::windows::ffi::OsStrExt;

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
