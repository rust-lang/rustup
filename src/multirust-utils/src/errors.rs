use std::path::{Path, PathBuf};
use std::fmt::{self, Display};
use std::error;
use std::io;
use std::ffi::OsString;
use hyper;
use raw;

use notify::{self, NotificationLevel, Notifyable};

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
