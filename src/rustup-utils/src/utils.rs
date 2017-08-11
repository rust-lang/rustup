use errors::*;
use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Write};
use std::process::Command;
use std::ffi::OsString;
use std::env;
use sha2::Sha256;
use notifications::{Notification};
use raw;
#[cfg(windows)]
use winreg;
use std::cmp::Ord;
use url::Url;

pub use raw::{is_directory, is_file, path_exists, if_not_empty, random_string, prefix_arg,
                    has_cmd, find_cmd};

pub fn ensure_dir_exists(name: &'static str,
                         path: &Path,
                         notify_handler: &Fn(Notification))
                         -> Result<bool> {
    raw::ensure_dir_exists(path,
                           |p| notify_handler(Notification::CreatingDirectory(name, p)))
        .chain_err(|| {
            ErrorKind::CreatingDirectory {
                name: name,
                path: PathBuf::from(path),
            }
        })
}

pub fn read_file(name: &'static str, path: &Path) -> Result<String> {
    raw::read_file(path).chain_err(|| {
        ErrorKind::ReadingFile {
            name: name,
            path: PathBuf::from(path),
        }
    })
}

pub fn write_file(name: &'static str, path: &Path, contents: &str) -> Result<()> {
    raw::write_file(path, contents).chain_err(|| {
        ErrorKind::WritingFile {
            name: name,
            path: PathBuf::from(path),
        }
    })
}

pub fn append_file(name: &'static str, path: &Path, line: &str) -> Result<()> {
    raw::append_file(path, line).chain_err(|| {
        ErrorKind::WritingFile {
            name: name,
            path: PathBuf::from(path),
        }
    })
}

pub fn write_line(name: &'static str, file: &mut File, path: &Path, line: &str) -> Result<()> {
    writeln!(file, "{}", line).chain_err(|| {
        ErrorKind::WritingFile {
            name: name,
            path: path.to_path_buf(),
        }
    })
}

pub fn write_str(name: &'static str, file: &mut File, path: &Path, s: &str) -> Result<()> {
    write!(file, "{}", s).chain_err(|| {
        ErrorKind::WritingFile {
            name: name,
            path: path.to_path_buf(),
        }
    })
}

pub fn rename_file(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
    fs::rename(src, dest).chain_err(|| {
        ErrorKind::RenamingFile {
            name: name,
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
        }
    })
}

pub fn rename_dir(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
    fs::rename(src, dest).chain_err(|| {
        ErrorKind::RenamingDirectory {
            name: name,
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
        }
    })
}

pub fn filter_file<F: FnMut(&str) -> bool>(name: &'static str,
                                           src: &Path,
                                           dest: &Path,
                                           filter: F)
                                           -> Result<usize> {
    raw::filter_file(src, dest, filter).chain_err(|| {
        ErrorKind::FilteringFile {
            name: name,
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
        }
    })
}

pub fn match_file<T, F: FnMut(&str) -> Option<T>>(name: &'static str,
                                                  src: &Path,
                                                  f: F)
                                                  -> Result<Option<T>> {
    raw::match_file(src, f).chain_err(|| {
        ErrorKind::ReadingFile {
            name: name,
            path: PathBuf::from(src),
        }
    })
}

pub fn canonicalize_path(path: &Path, notify_handler: &Fn(Notification)) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        notify_handler(Notification::NoCanonicalPath(path));
        PathBuf::from(path)
    })
}

pub fn tee_file<W: io::Write>(name: &'static str, path: &Path, w: &mut W) -> Result<()> {
    raw::tee_file(path, w).chain_err(|| {
        ErrorKind::ReadingFile {
            name: name,
            path: PathBuf::from(path),
        }
    })
}

pub fn download_file(url: &Url,
                     path: &Path,
                     hasher: Option<&mut Sha256>,
                     notify_handler: &Fn(Notification))
                     -> Result<()> {
     download_file_with_resume(&url, &path, hasher, false, &notify_handler)
}

pub fn download_file_with_resume(url: &Url,
                     path: &Path,
                     hasher: Option<&mut Sha256>,
                     resume_from_partial: bool,
                     notify_handler: &Fn(Notification))
                     -> Result<()> {
    use download::ErrorKind as DEK;
    match download_file_(url, path, hasher, resume_from_partial, notify_handler) {
        Ok(_) => Ok(()),
        Err(e) => {
            let is_client_error = match e.kind() {
                &ErrorKind::Download(DEK::HttpStatus(400 ... 499)) => true,
                &ErrorKind::Download(DEK::FileNotFound) => true,
                _ => false
            };
            Err(e).chain_err(|| if is_client_error {
                ErrorKind::DownloadNotExists {
                    url: url.clone(),
                    path: path.to_path_buf(),
                }
            } else {
                ErrorKind::DownloadingFile {
                    url: url.clone(),
                    path: path.to_path_buf(),
                }
            })
        }
    }
}

fn download_file_(url: &Url,
                  path: &Path,
                  hasher: Option<&mut Sha256>,
                  resume_from_partial: bool,
                  notify_handler: &Fn(Notification))
                  -> Result<()> {

    use sha2::Digest;
    use std::cell::RefCell;
    use download::download_to_path_with_backend;
    use download::{self, Event, Backend};

    notify_handler(Notification::DownloadingFile(url, path));

    let hasher = RefCell::new(hasher);

    // This callback will write the download to disk and optionally
    // hash the contents, then forward the notification up the stack
    let callback: &Fn(Event) -> download::Result<()> = &|msg| {
        match msg {
            Event::DownloadDataReceived(data) => {
                if let Some(ref mut h) = *hasher.borrow_mut() {
                    h.input(data);
                }
            }
            _ => ()
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
    let use_hyper_backend = env::var_os("RUSTUP_USE_HYPER").is_some();
    let use_rustls_backend = env::var_os("RUSTUP_USE_RUSTLS").is_some();
    let (backend, notification) = if use_hyper_backend {
        (Backend::Hyper, Notification::UsingHyper)
    } else if use_rustls_backend {
        (Backend::Rustls, Notification::UsingRustls)
    } else {
        (Backend::Curl, Notification::UsingCurl)
    };
    notify_handler(notification);
    try!(download_to_path_with_backend(backend, url, path, resume_from_partial, Some(callback)));

    notify_handler(Notification::DownloadFinished);

    Ok(())
}

pub fn parse_url(url: &str) -> Result<Url> {
    Url::parse(url).chain_err(|| format!("failed to parse url: {}", url))
}

pub fn cmd_status(name: &'static str, cmd: &mut Command) -> Result<()> {
    raw::cmd_status(cmd).chain_err(|| {
        ErrorKind::RunningCommand {
            name: OsString::from(name),
        }
    })
}

pub fn assert_is_file(path: &Path) -> Result<()> {
    if !is_file(path) {
        Err(ErrorKind::NotAFile { path: PathBuf::from(path) }.into())
    } else {
        Ok(())
    }
}

pub fn assert_is_directory(path: &Path) -> Result<()> {
    if !is_directory(path) {
        Err(ErrorKind::NotADirectory { path: PathBuf::from(path) }.into())
    } else {
        Ok(())
    }
}

pub fn symlink_dir(src: &Path, dest: &Path, notify_handler: &Fn(Notification)) -> Result<()> {
    notify_handler(Notification::LinkingDirectory(src, dest));
    raw::symlink_dir(src, dest).chain_err(|| {
        ErrorKind::LinkingDirectory {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
        }
    })
}

pub fn hard_or_symlink_file(src: &Path, dest: &Path) -> Result<()> {
    if hardlink_file(src, dest).is_err() {
        symlink_file(src, dest)?;
    }
    Ok(())
}

pub fn hardlink_file(src: &Path, dest: &Path) -> Result<()> {
    raw::hardlink(src, dest).chain_err(|| {
        ErrorKind::LinkingFile {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
        }
    })
}

#[cfg(unix)]
pub fn symlink_file(src: &Path, dest: &Path) -> Result<()> {
    ::std::os::unix::fs::symlink(src, dest).chain_err(|| {
        ErrorKind::LinkingFile {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
        }
    })
}

#[cfg(windows)]
pub fn symlink_file(src: &Path, dest: &Path) -> Result<()> {
    // we are supposed to not use symlink on windows
    Err(ErrorKind::LinkingFile {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
        }.into()
    )
}

pub fn copy_dir(src: &Path, dest: &Path, notify_handler: &Fn(Notification)) -> Result<()> {
    notify_handler(Notification::CopyingDirectory(src, dest));
    raw::copy_dir(src, dest).chain_err(|| {
        ErrorKind::CopyingDirectory {
            src: PathBuf::from(src),
            dest: PathBuf::from(dest),
        }
    })
}

pub fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    fs::copy(src, dest)
        .chain_err(|| {
            ErrorKind::CopyingFile {
                src: PathBuf::from(src),
                dest: PathBuf::from(dest),
            }
        })
        .map(|_| ())
}

pub fn remove_dir(name: &'static str, path: &Path, notify_handler: &Fn(Notification)) -> Result<()> {
    notify_handler(Notification::RemovingDirectory(name, path));
    raw::remove_dir(path).chain_err(|| {
        ErrorKind::RemovingDirectory {
            name: name,
            path: PathBuf::from(path),
        }
    })
}

pub fn remove_file(name: &'static str, path: &Path) -> Result<()> {
    fs::remove_file(path).chain_err(|| {
        ErrorKind::RemovingFile {
            name: name,
            path: PathBuf::from(path),
        }
    })
}

pub fn read_dir(name: &'static str, path: &Path) -> Result<fs::ReadDir> {
    fs::read_dir(path).chain_err(|| {
        ErrorKind::ReadingDirectory {
            name: name,
            path: PathBuf::from(path),
        }
    })
}

pub fn open_browser(path: &Path) -> Result<()> {
    match raw::open_browser(path) {
        Ok(true) => Ok(()),
        Ok(false) => Err("no browser installed".into()),
        Err(e) => Err(e).chain_err(|| "could not open browser")
    }
}

pub fn set_permissions(path: &Path, perms: fs::Permissions) -> Result<()> {
    fs::set_permissions(path, perms).chain_err(|| {
        ErrorKind::SettingPermissions {
            path: PathBuf::from(path),
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

        let metadata = try!(fs::metadata(path).chain_err(|| {
            ErrorKind::SettingPermissions {
                path: PathBuf::from(path),
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

pub use home::home_dir;

pub fn cargo_home() -> Result<PathBuf> {
    Ok(::home::cargo_home().
       chain_err(|| "couldn't find value of CARGO_HOME")?)
}

// Convert the ~/.multirust folder to ~/.rustup while dealing with rustup.sh
// metadata, which used to also live in ~/.rustup, but now lives in ~/rustup.sh.
pub fn do_rustup_home_upgrade() {

    fn rustup_home_is_set() -> bool {
        env::var_os("RUSTUP_HOME").is_some()
    }

    fn rustup_dir() -> Option<PathBuf> {
        dot_dir(".rustup")
    }

    fn rustup_sh_dir() -> Option<PathBuf> {
        dot_dir(".rustup.sh")
    }

    fn multirust_dir() -> Option<PathBuf> {
        dot_dir(".multirust")
    }

    fn rustup_dir_exists() -> bool {
        rustup_dir().map(|p| p.exists()).unwrap_or(false)
    }

    fn rustup_sh_dir_exists() -> bool {
        rustup_sh_dir().map(|p| p.exists()).unwrap_or(false)
    }

    fn multirust_dir_exists() -> bool {
        multirust_dir().map(|p| p.exists()).unwrap_or(false)
    }

    fn rustup_old_version_exists() -> bool {
        rustup_dir()
            .map(|p| p.join("rustup-version").exists())
            .unwrap_or(false)
    }

    fn delete_rustup_dir() -> Result<()> {
        if let Some(dir) = rustup_dir() {
            raw::remove_dir(&dir)
                .chain_err(|| "unable to delete rustup dir")?;
        }

        Ok(())
    }

    fn rename_rustup_dir_to_rustup_sh() -> Result<()> {
        let dirs = (rustup_dir(), rustup_sh_dir());
        if let (Some(rustup), Some(rustup_sh)) = dirs {
            fs::rename(&rustup, &rustup_sh)
                .chain_err(|| "unable to rename rustup dir")?;
        }

        Ok(())
    }

    fn rename_multirust_dir_to_rustup() -> Result<()> {
        let dirs = (multirust_dir(), rustup_dir());
        if let (Some(rustup), Some(rustup_sh)) = dirs {
            fs::rename(&rustup, &rustup_sh)
                .chain_err(|| "unable to rename multirust dir")?;
        }

        Ok(())
    }

    // If RUSTUP_HOME is set then its default path doesn't matter, so we're
    // not going to risk doing any I/O work and making a mess.
    if rustup_home_is_set() { return }

    // Now we are just trying to get a bogus, rustup.sh-created ~/.rustup out
    // of the way in the manner that is least likely to take time and generate
    // errors. First try to rename it to ~/.rustup.sh, then try to delete it.
    // If that doesn't work we can't use the ~/.rustup name.
    let old_rustup_dir_removed = if rustup_old_version_exists() {
        if !rustup_sh_dir_exists() {
            if rename_rustup_dir_to_rustup_sh().is_ok() {
                true
            } else {
                if delete_rustup_dir().is_ok() {
                    true
                } else {
                    false
                }
            }
        } else {
            if delete_rustup_dir().is_ok() {
                true
            } else {
                false
            }
        }
    } else {
        true
    };

    // Now we're trying to move ~/.multirust to ~/.rustup
    if old_rustup_dir_removed {
        if multirust_dir_exists() {
            if !rustup_dir_exists() {
                if rename_multirust_dir_to_rustup().is_ok() {
                    // Finally, making the hardlink from ~/.multirust back to
                    // ~/.rustup, for temporary compatibility.
                    let _ = create_legacy_multirust_symlink();
                }
            }
        }
    }
}

// Creates a ~/.rustup folder and a ~/.multirust symlink
pub fn create_rustup_home() -> Result<()> {
    // If there's an existing install, then try to upgrade
    do_rustup_home_upgrade();

    // If RUSTUP_HOME is set then don't make any assumptions about where it's
    // ok to put ~/.multirust
    if env::var_os("RUSTUP_HOME").is_some() { return Ok(()) }

    let home = rustup_home_in_user_dir()?;
    fs::create_dir_all(&home)
        .chain_err(|| "unable to create ~/.rustup")?;

    // This is a temporary compatibility symlink
    create_legacy_multirust_symlink()?;

    Ok(())
}

// Create a symlink from ~/.multirust to ~/.rustup to temporarily
// accomodate old tools that are expecting that directory
fn create_legacy_multirust_symlink() -> Result<()> {
    let newhome = rustup_home_in_user_dir()?;
    let oldhome = legacy_multirust_home()?;

    if oldhome.exists() {
        return Ok(());
    }

    raw::symlink_dir(&newhome, &oldhome)
        .chain_err(|| format!("unable to symlink {} from {}",
                              newhome.display(), oldhome.display()))?;

    Ok(())
}

pub fn delete_legacy_multirust_symlink() -> Result<()> {
    let oldhome = legacy_multirust_home()?;

    if oldhome.exists() {
        let meta = fs::symlink_metadata(&oldhome)
            .chain_err(|| "unable to get metadata for ~/.multirust")?;
        if meta.file_type().is_symlink() {
            // remove_dir handles unlinking symlinks
            raw::remove_dir(&oldhome)
                .chain_err(|| format!("unable to delete legacy symlink {}", oldhome.display()))?;
        }
    }

    Ok(())
}

fn dot_dir(name: &str) -> Option<PathBuf> {
    home_dir().map(|p| p.join(name))
}

pub fn legacy_multirust_home() -> Result<PathBuf> {
    dot_dir(".multirust").ok_or(ErrorKind::RustupHome.into())
}

pub fn rustup_home_in_user_dir() -> Result<PathBuf> {
    dot_dir(".rustup").ok_or(ErrorKind::RustupHome.into())
}

pub fn rustup_home() -> Result<PathBuf> {
    do_rustup_home_upgrade();
    ::home::rustup_home().chain_err(|| ErrorKind::RustupHome)
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
    unsafe { ::std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2).to_vec() }
}

// This is used to decode the value of HKCU\Environment\PATH. If that
// key is not unicode (or not REG_SZ | REG_EXPAND_SZ) then this
// returns null.  The winreg library itself does a lossy unicode
// conversion.
#[cfg(windows)]
pub fn string_from_winreg_value(val: &winreg::RegValue) -> Option<String> {
    use winreg::enums::RegType;
    use std::slice;

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
            while s.ends_with('\u{0}') {s.pop();}
            Some(s)
        }
        _ => None
    }
}

pub fn toolchain_sort<T: AsRef<str>>(v: &mut Vec<T>) {
    use semver::{Version, Identifier};

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


#[cfg(test)]
mod tests {
    use super::*;

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
