
use std::path::{Path, PathBuf};
use std::error;
use std::fs;
use std::io;
use std::process::Command;
use std::ffi::OsString;
use std::fmt::{self, Display};
use std::env;
use hyper;
use openssl::crypto::hash::Hasher;

use notify::{self, NotificationLevel, Notifyable};

pub mod raw;

pub use self::raw::{is_directory, is_file, path_exists, if_not_empty, random_string, prefix_arg,
                    home_dir, has_cmd, find_cmd};

#[derive(Debug)]
pub enum Notification<'a> {
    CreatingDirectory(&'a str, &'a Path),
    LinkingDirectory(&'a Path, &'a Path),
    CopyingDirectory(&'a Path, &'a Path),
    RemovingDirectory(&'a str, &'a Path),
    DownloadingFile(&'a hyper::Url, &'a Path),
    /// Received the Content-Length of the to-be downloaded data.
    DownloadContentLengthReceived(u64),
    /// Received some data.
    DownloadDataReceived(usize),
    /// Download has finished.
    DownloadFinished,
    NoCanonicalPath(&'a Path),
}

#[derive(Debug)]
pub enum Error {
    LocatingHome,
    LocatingWorkingDir {
        error: io::Error,
    },
    ReadingFile {
        name: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    ReadingDirectory {
        name: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    WritingFile {
        name: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    CreatingDirectory {
        name: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    FilteringFile {
        name: &'static str,
        src: PathBuf,
        dest: PathBuf,
        error: io::Error,
    },
    RenamingFile {
        name: &'static str,
        src: PathBuf,
        dest: PathBuf,
        error: io::Error,
    },
    RenamingDirectory {
        name: &'static str,
        src: PathBuf,
        dest: PathBuf,
        error: io::Error,
    },
    DownloadingFile {
        url: hyper::Url,
        path: PathBuf,
        error: raw::DownloadError,
    },
    InvalidUrl {
        url: String,
    },
    RunningCommand {
        name: OsString,
        error: raw::CommandError,
    },
    NotAFile {
        path: PathBuf,
    },
    NotADirectory {
        path: PathBuf,
    },
    LinkingFile {
        src: PathBuf,
        dest: PathBuf,
        error: io::Error,
    },
    LinkingDirectory {
        src: PathBuf,
        dest: PathBuf,
        error: io::Error,
    },
    CopyingDirectory {
        src: PathBuf,
        dest: PathBuf,
        error: raw::CommandError,
    },
    CopyingFile {
        src: PathBuf,
        dest: PathBuf,
        error: io::Error,
    },
    RemovingFile {
        name: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    RemovingDirectory {
        name: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    OpeningBrowser {
        error: Option<io::Error>,
    },
    SettingPermissions {
        path: PathBuf,
        error: io::Error,
    },
}

pub type Result<T> = ::std::result::Result<T, Error>;
pub type NotifyHandler<'a> = notify::NotifyHandler<'a, for<'b> Notifyable<Notification<'b>>>;

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            CreatingDirectory(_, _) | RemovingDirectory(_, _) => NotificationLevel::Verbose,
            LinkingDirectory(_, _) |
            CopyingDirectory(_, _) |
            DownloadingFile(_, _) |
            DownloadContentLengthReceived(_) |
            DownloadDataReceived(_) |
            DownloadFinished => NotificationLevel::Verbose,
            NoCanonicalPath(_) => NotificationLevel::Warn,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            CreatingDirectory(name, path) => {
                write!(f, "creating {} directory: '{}'", name, path.display())
            }
            LinkingDirectory(_, dest) => write!(f, "linking directory from: '{}'", dest.display()),
            CopyingDirectory(src, _) => write!(f, "coping directory from: '{}'", src.display()),
            RemovingDirectory(name, path) => {
                write!(f, "removing {} directory: '{}'", name, path.display())
            }
            DownloadingFile(url, _) => write!(f, "downloading file from: '{}'", url),
            DownloadContentLengthReceived(len) => write!(f, "download size is: '{}'", len),
            DownloadDataReceived(len) => write!(f, "received some data of size {}", len),
            DownloadFinished => write!(f, "download finished"),
            NoCanonicalPath(path) => write!(f, "could not canonicalize path: '{}'", path.display()),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            LocatingHome => "could not locate home directory",
            LocatingWorkingDir {..} => "could not locate working directory",
            ReadingFile {..} =>  "could not read file",
            ReadingDirectory {..} => "could not read directory",
            WritingFile {..} =>  "could not write file",
            CreatingDirectory {..} => "could not create directory",
            FilteringFile {..} => "could not copy  file",
            RenamingFile {..} => "could not rename file",
            RenamingDirectory {..} => "could not rename directory",
            DownloadingFile {..} => "could not download file",
            InvalidUrl {..} => "invalid url",
            RunningCommand {..} => "command failed",
            NotAFile {..} => "not a file",
            NotADirectory {..} => "not a directory",
            LinkingFile {..} => "could not link file",
            LinkingDirectory {..} => "could not symlink directory",
            CopyingDirectory {..} => "could not copy directory",
            CopyingFile {..} => "could not copy file",
            RemovingFile {..} => "could not remove file",
            RemovingDirectory {..} => "could not remove directory",
            OpeningBrowser { error: Some(_) } => "could not open browser",
            OpeningBrowser { error: None } => "could not open browser: no browser installed",
            SettingPermissions {..} => "failed to set permissions",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        use self::Error::*;
        match *self {
            // Variants that do not carry an error.
            LocatingHome |
            InvalidUrl {..} |
            NotAFile {..} |
            NotADirectory {..} => None,
            // Variants that carry `error: io::Error`.
            LocatingWorkingDir { ref error, .. } |
            ReadingFile { ref error, .. } |
            ReadingDirectory { ref error, .. } |
            WritingFile { ref error, .. } |
            CreatingDirectory { ref error, .. } |
            FilteringFile { ref error, .. } |
            RenamingFile { ref error, .. } |
            RenamingDirectory { ref error, .. } |
            LinkingFile { ref error, .. } |
            LinkingDirectory { ref error, .. } |
            CopyingFile { ref error, .. } |
            RemovingFile { ref error, .. } |
            RemovingDirectory { ref error, .. } |
            SettingPermissions { ref error, .. } => Some(error),
            // Variants that carry `error: raw::CommandError`.
            RunningCommand { ref error, .. } |
            CopyingDirectory { ref error, .. } => Some(error),
            // Variant carrying its own error type.
            DownloadingFile { ref error, .. } => Some(error),
            // Variant carrying `error: Option<io::Error>`.
            OpeningBrowser { error: Some(ref e) } => Some(e),
            OpeningBrowser { error: None } => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use self::Error::*;
        match *self {
            LocatingHome => write!(f, "could not locate home directory"),
            LocatingWorkingDir { ref error } => {
                write!(f, "could not locate working directory ({})", error)
            }
            ReadingFile { ref name, ref path, ref error } => {
                write!(f,
                       "could not read {} file: '{}' ({})",
                       name,
                       path.display(),
                       error)
            }
            ReadingDirectory { ref name, ref path, ref error } => {
                write!(f,
                       "could not read {} directory: '{}' ({})",
                       name,
                       path.display(),
                       error)
            }
            WritingFile { ref name, ref path, ref error } => {
                write!(f,
                       "could not write {} file: '{}' ({})",
                       name,
                       path.display(),
                       error)
            }
            CreatingDirectory { ref name, ref path, ref error } => {
                write!(f,
                       "could not create {} directory: '{}' ({})",
                       name,
                       path.display(),
                       error)
            }
            FilteringFile { ref name, ref src, ref dest, ref error } => {
                write!(f,
                       "could not copy {} file from '{}' to '{}' ({})",
                       name,
                       src.display(),
                       dest.display(),
                       error)
            }
            RenamingFile { ref name, ref src, ref dest, ref error } => {
                write!(f,
                       "could not rename {} file from '{}' to '{}' ({})",
                       name,
                       src.display(),
                       dest.display(),
                       error)
            }
            RenamingDirectory { ref name, ref src, ref dest, ref error } => {
                write!(f,
                       "could not rename {} directory from '{}' to '{}' ({})",
                       name,
                       src.display(),
                       dest.display(),
                       error)
            }
            DownloadingFile { ref url, ref path, ref error } => {
                write!(f,
                       "could not download file from '{}' to '{}' ({})",
                       url,
                       path.display(),
                       error)
            }
            InvalidUrl { ref url } => write!(f, "invalid url: '{}'", url),
            RunningCommand { ref name, ref error } => {
                write!(f,
                       "command failed: '{}' ({})",
                       PathBuf::from(name).display(),
                       error)
            }
            NotAFile { ref path } => write!(f, "not a file: '{}'", path.display()),
            NotADirectory { ref path } => write!(f, "not a directory: '{}'", path.display()),
            LinkingFile { ref src, ref dest, ref error } => {
                write!(f,
                       "could not create link from '{}' to '{}' ({})",
                       src.display(),
                       dest.display(),
                       error)
            }
            LinkingDirectory { ref src, ref dest, ref error } => {
                write!(f,
                       "could not create symlink from '{}' to '{}' ({})",
                       src.display(),
                       dest.display(),
                       error)
            }
            CopyingDirectory { ref src, ref dest, ref error } => {
                write!(f,
                       "could not copy directory from '{}' to '{}' ({})",
                       src.display(),
                       dest.display(),
                       error)
            }
            CopyingFile { ref src, ref dest, ref error } => {
                write!(f,
                       "could not copy file from '{}' to '{}' ({})",
                       src.display(),
                       dest.display(),
                       error)
            }
            RemovingFile { ref name, ref path, ref error } => {
                write!(f,
                       "could not remove {} file: '{}' ({})",
                       name,
                       path.display(),
                       error)
            }
            RemovingDirectory { ref name, ref path, ref error } => {
                write!(f,
                       "could not remove {} directory: '{} ({})'",
                       name,
                       path.display(),
                       error)
            }
            OpeningBrowser { error: Some(ref e) } => write!(f, "could not open browser: {}", e),
            OpeningBrowser { error: None } => {
                write!(f, "could not open browser: no browser installed")
            }
            SettingPermissions { ref path, ref error } => {
                write!(f,
                       "failed to set permissions for: '{} ({})'",
                       path.display(),
                       error)
            }
        }
    }
}

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
        let new_mode = perms.mode() | 0o111;
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

pub fn get_local_data_path() -> Result<PathBuf> {
    #[cfg(windows)]
    fn inner() -> Result<PathBuf> {
        raw::windows::get_special_folder(&raw::windows::FOLDERID_LocalAppData)
            .map_err(|_| Error::LocatingHome)
    }
    #[cfg(not(windows))]
    fn inner() -> Result<PathBuf> {
        // TODO: consider using ~/.local/ instead
        home_dir()
            .ok_or(Error::LocatingHome)
            .map(PathBuf::from)
            .and_then(to_absolute)
    }

    inner()
}
