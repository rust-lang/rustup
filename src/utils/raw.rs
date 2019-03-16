use std::char::from_u32;
use std::env;
use std::error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::str;

use rand::random;

pub fn ensure_dir_exists<P: AsRef<Path>, F: FnOnce(&Path)>(
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
    (0..length)
        .map(|_| from_u32(chars[random::<usize>() % chars.len()] as u32).unwrap())
        .collect()
}

pub fn if_not_empty<S: PartialEq<str>>(s: S) -> Option<S> {
    if s == *"" {
        None
    } else {
        Some(s)
    }
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

pub fn read_file(path: &Path) -> io::Result<String> {
    let mut file = fs::OpenOptions::new().read(true).open(path)?;

    let mut contents = String::new();

    io::Read::read_to_string(&mut file, &mut contents)?;

    Ok(contents)
}

pub fn filter_file<F: FnMut(&str) -> bool>(
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
            writeln!(&mut writer, "{}", &line)?;
        } else {
            removed += 1;
        }
    }

    writer.flush()?;

    Ok(removed)
}

pub fn match_file<T, F: FnMut(&str) -> Option<T>>(src: &Path, mut f: F) -> io::Result<Option<T>> {
    let src_file = fs::File::open(src)?;

    let mut reader = io::BufReader::new(src_file);

    for result in io::BufRead::lines(&mut reader) {
        let line = result?;
        if let Some(r) = f(&line) {
            return Ok(Some(r));
        }
    }

    Ok(None)
}

pub fn append_file(dest: &Path, line: &str) -> io::Result<()> {
    let mut dest_file = fs::OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open(dest)?;

    writeln!(&mut dest_file, "{}", line)?;

    dest_file.sync_data()?;

    Ok(())
}

pub fn tee_file<W: io::Write>(path: &Path, w: &mut W) -> io::Result<()> {
    let mut file = fs::OpenOptions::new().read(true).open(path)?;

    let buffer_size = 0x10000;
    let mut buffer = vec![0u8; buffer_size];

    loop {
        let bytes_read = io::Read::read(&mut file, &mut buffer)?;

        if bytes_read != 0 {
            io::Write::write_all(w, &mut buffer[0..bytes_read])?;
        } else {
            return Ok(());
        }
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
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use winapi::shared::minwindef::*;
    use winapi::um::fileapi::*;
    use winapi::um::ioapiset::*;
    use winapi::um::winbase::*;
    use winapi::um::winioctl::FSCTL_SET_REPARSE_POINT;
    use winapi::um::winnt::*;

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
            0 as *mut _,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            ptr::null_mut(),
        );

        let mut data = [0u8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE];
        let db = data.as_mut_ptr() as *mut REPARSE_MOUNTPOINT_DATA_BUFFER;
        let buf = &mut (*db).ReparseTarget as *mut WCHAR;
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
        (*db).ReparseTargetMaximumLength = (i * 2) as WORD;
        (*db).ReparseTargetLength = ((i - 1) * 2) as WORD;
        (*db).ReparseDataLength = (*db).ReparseTargetLength as DWORD + 12;

        let mut ret = 0;
        let res = DeviceIoControl(
            h as *mut _,
            FSCTL_SET_REPARSE_POINT,
            data.as_ptr() as *mut _,
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

    fn cause(&self) -> Option<&dyn error::Error> {
        use self::CommandError::*;
        match *self {
            Io(ref e) => Some(e),
            Status(_) => None,
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        if cfg!(windows) {
            fs::remove_dir(path)
        } else {
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

pub fn copy_dir(src: &Path, dest: &Path) -> io::Result<()> {
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

pub fn prefix_arg<S: AsRef<OsStr>>(name: &str, s: S) -> OsString {
    let mut arg = OsString::from(name);
    arg.push(s);
    arg
}

pub fn has_cmd(cmd: &str) -> bool {
    let cmd = format!("{}{}", cmd, env::consts::EXE_SUFFIX);
    let path = env::var_os("PATH").unwrap_or(OsString::new());
    env::split_paths(&path)
        .map(|p| p.join(&cmd))
        .any(|p| p.exists())
}

pub fn find_cmd<'a>(cmds: &[&'a str]) -> Option<&'a str> {
    cmds.into_iter().map(|&s| s).filter(|&s| has_cmd(s)).next()
}

pub fn open_browser(path: &Path) -> io::Result<bool> {
    #[cfg(not(windows))]
    fn inner(path: &Path) -> io::Result<bool> {
        use std::env;
        use std::process::Stdio;

        let env_browser = env::var_os("BROWSER").map(|b| env::split_paths(&b).collect::<Vec<_>>());
        let env_commands = env_browser
            .as_ref()
            .map(|cmds| cmds.iter().by_ref().filter_map(|b| b.to_str()).collect())
            .unwrap_or(vec![]);

        let commands = [
            "xdg-open",
            "open",
            "firefox",
            "chromium",
            "sensible-browser",
        ];
        if let Some(cmd) = find_cmd(&env_commands).or(find_cmd(&commands)) {
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
        use std::ptr;
        use winapi::ctypes;
        use winapi::shared::minwindef::HINSTANCE;
        use winapi::shared::ntdef::LPCWSTR;
        use winapi::shared::windef::HWND;

        // FIXME: When winapi has this function, use their version
        extern "system" {
            pub fn ShellExecuteW(
                hwnd: HWND,
                lpOperation: LPCWSTR,
                lpFile: LPCWSTR,
                lpParameters: LPCWSTR,
                lpDirectory: LPCWSTR,
                nShowCmd: ctypes::c_int,
            ) -> HINSTANCE;
        }
        const SW_SHOW: ctypes::c_int = 5;

        let path = windows::to_u16s(path)?;
        let operation = windows::to_u16s("open")?;
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                operation.as_ptr(),
                path.as_ptr(),
                ptr::null(),
                ptr::null(),
                SW_SHOW,
            )
        };
        Ok(result as usize > 32)
    }
    inner(path)
}

#[cfg(windows)]
pub mod windows {
    use std::ffi::OsStr;
    use std::io;
    use std::os::windows::ffi::OsStrExt;

    pub fn to_u16s<S: AsRef<OsStr>>(s: S) -> io::Result<Vec<u16>> {
        fn inner(s: &OsStr) -> io::Result<Vec<u16>> {
            let mut maybe_result: Vec<u16> = s.encode_wide().collect();
            if maybe_result.iter().any(|&u| u == 0) {
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
