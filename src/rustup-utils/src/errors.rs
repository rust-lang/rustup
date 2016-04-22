use std::path::PathBuf;
use std::ffi::OsString;
use hyper;

pub type Result<T> = ::std::result::Result<T, Error>;

easy_error! {
    #[derive(Debug)]
    pub enum Error {
        LocatingHome {
            description("could not locate home directory")
        }
        LocatingWorkingDir {
            error: io::Error,
        } {
            description("could not locate working directory")
            cause(error)
        }
        ReadingFile {
            name: &'static str,
            path: PathBuf,
            error: io::Error,
        } {
            description("could not read file")
            display("could not read {} file: '{}'", name, path.display())
            cause(error)
        }
        ReadingDirectory {
            name: &'static str,
            path: PathBuf,
            error: io::Error,
        } {
            description("could not read directory")
            display("could not read {} directory: '{}'", name, path.display())
            cause(error)
        }
        WritingFile {
            name: &'static str,
            path: PathBuf,
            error: io::Error,
        } {
            description("could not write file")
            display("could not write {} file: '{}'", name, path.display())
            cause(error)
        }
        CreatingDirectory {
            name: &'static str,
            path: PathBuf,
            error: io::Error,
        } {
            description("could not create directory")
            display("could not crate {} directory: '{}'", name, path.display())
            cause(error)
        }
        FilteringFile {
            name: &'static str,
            src: PathBuf,
            dest: PathBuf,
            error: io::Error,
        } {
            description("could not copy  file")
            display("could not copy {} file from '{}' to '{}'", name, src.display(), dest.display())
            cause(error)
        }
        RenamingFile {
            name: &'static str,
            src: PathBuf,
            dest: PathBuf,
            error: io::Error,
        } {
            description("could not rename file")
            display("could not rename {} file from '{}' to '{}'", name, src.display(), dest.display())
            cause(error)
        }
        RenamingDirectory {
            name: &'static str,
            src: PathBuf,
            dest: PathBuf,
            error: io::Error,
        } {
            description("could not rename directory")
            display("could not rename {} directory from '{}' to '{}'", name, src.display(), dest.display())
            cause(error)
        }
        DownloadingFile {
            url: hyper::Url,
            path: PathBuf,
            error: raw::DownloadError,
        } {
            description("could not download file")
            display("could not download file from '{}' to '{}", url, path.display())
            cause(error)
        }
        InvalidUrl {
            url: String,
        } {
            description("invalid url")
            display("invalid url: {}", url)
        }
        RunningCommand {
            name: OsString,
            error: raw::CommandError,
        } {
            description("command failed")
            display("command failed: '{}'", PathBuf::from(name).display())
            cause(error)
        }
        NotAFile {
            path: PathBuf,
        } {
            description("not a file")
            display("not a file: '{}'", path.display())
        }
        NotADirectory {
            path: PathBuf,
        } {
            description("not a directory")
            display("not a directory: '{}'", path.display())
        }
        LinkingFile {
            src: PathBuf,
            dest: PathBuf,
            error: io::Error,
        } {
            description("could not link file")
            display("could not create link from '{}' to '{}'", src.display(), dest.display())
            cause(error)
        }
        LinkingDirectory {
            src: PathBuf,
            dest: PathBuf,
            error: io::Error,
        } {
            description("could not symlink directory")
            display("could not create link from '{}' to '{}'", src.display(), dest.display())
            cause(error)
        }
        CopyingDirectory {
            src: PathBuf,
            dest: PathBuf,
            error: raw::CommandError,
        } {
            description("could not copy directory")
            display("could not copy directory from '{}' to '{}'", src.display(), dest.display())
        }
        CopyingFile {
            src: PathBuf,
            dest: PathBuf,
            error: io::Error,
        } {
            description("could not copy file")
            display("could not copy file from '{}' to '{}'", src.display(), dest.display())
            cause(error)
        }
        RemovingFile {
            name: &'static str,
            path: PathBuf,
            error: io::Error,
        } {
            description("could not remove file")
            display("could not remove '{}' file: '{}'", name, path.display())
            cause(error)
        }
        RemovingDirectory {
            name: &'static str,
            path: PathBuf,
            error: io::Error,
        } {
            description("could not remove directory")
            display("could not remove '{}' directory: '{}'", name, path.display())
            cause(error)
        }
        OpeningBrowser {
            error: io::Error,
        } {
            description("could not open browser")
            cause(error)
        }
        NoBrowser {
            description("could not open browser: no browser installed")
        }
        SettingPermissions {
            path: PathBuf,
            error: io::Error,
        } {
            description("failed to set permissions")
            display("failed to set permissions for '{}'", path.display())
            cause(error)
        }
        CargoHome {
            description("couldn't find value of CARGO_HOME")
        }
        MultirustHome {
            description("couldn't find value of RUSTUP_HOME")
        }
    }
}

#[macro_export]
macro_rules! extend_error {
    ($cur:ty: $base:ty, $p:ident => $e:expr) => (
        impl From<$base> for $cur {
            fn from($p: $base) -> $cur {
                $e
            }
        }
    )
}
