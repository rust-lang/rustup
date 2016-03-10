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

