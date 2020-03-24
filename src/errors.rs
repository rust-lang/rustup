#![allow(clippy::large_enum_variant)]
#![allow(deprecated)] // because of `Error::description` deprecation in `error_chain`

use crate::component_for_bin;
use crate::dist::dist::Profile;
use crate::dist::manifest::{Component, Manifest};
use crate::dist::temp;
use error_chain::error_chain;
use std::ffi::OsString;
use std::fmt::{self, Debug, Display};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use thiserror::Error as ThisError;
use url::Url;

pub const TOOLSTATE_MSG: &str =
    "If you require these components, please install and use the latest successful build version,\n\
     which you can find at <https://rust-lang.github.io/rustup-components-history>.\n\nAfter determining \
     the correct date, install it with a command such as:\n\n    \
     rustup toolchain install nightly-2018-12-27\n\n\
     Then you can use the toolchain with commands such as:\n\n    \
     cargo +nightly-2018-12-27 build";

error_chain! {
    links {
        Download(download::Error, download::ErrorKind);
        Limits(effective_limits::Error, effective_limits::ErrorKind);
    }

    foreign_links {
        Temp(temp::Error);
        Io(io::Error);
        Open(opener::OpenError);
        Thread(std::sync::mpsc::RecvError);
    }

    errors {
        LocatingWorkingDir {
            description("Unable to proceed. Could not locate working directory.")
        }
        ReadingFile {
            name: &'static str,
            path: PathBuf,
        } {
            description("could not read file")
            display("could not read {} file: '{}'", name, path.display())
        }
        ReadingDirectory {
            name: &'static str,
            path: PathBuf,
        } {
            description("could not read directory")
            display("could not read {} directory: '{}'", name, path.display())
        }
        WritingFile {
            name: &'static str,
            path: PathBuf,
        } {
            description("could not write file")
            display("could not write {} file: '{}'", name, path.display())
        }
        CreatingDirectory {
            name: &'static str,
            path: PathBuf,
        } {
            description("could not create directory")
            display("could not create {} directory: '{}'", name, path.display())
        }
        ExpectedType(t: &'static str, n: String) {
            description("expected type")
            display("expected type: '{}' for '{}'", t, n)
        }
        FilteringFile {
            name: &'static str,
            src: PathBuf,
            dest: PathBuf,
        } {
            description("could not copy file")
            display("could not copy {} file from '{}' to '{}'", name, src.display(), dest.display())
        }
        RenamingFile {
            name: &'static str,
            src: PathBuf,
            dest: PathBuf,
        } {
            description("could not rename file")
            display("could not rename {} file from '{}' to '{}'",
                name, src.display(), dest.display())
        }
        RenamingDirectory {
            name: &'static str,
            src: PathBuf,
            dest: PathBuf,
        } {
            description("could not rename directory")
            display("could not rename {} directory from '{}' to '{}'", name, src.display(), dest.display())
        }
        DownloadingFile {
            url: Url,
            path: PathBuf,
        } {
            description("could not download file")
            display("could not download file from '{}' to '{}'", url, path.display())
        }
        DownloadNotExists {
            url: Url,
            path: PathBuf,
        } {
            description("could not download file")
            display("could not download file from '{}' to '{}'", url, path.display())
        }
        InvalidUrl {
            url: String,
        } {
            description("invalid url")
            display("invalid url: {}", url)
        }
        RunningCommand {
            name: OsString,
        } {
            description("command failed")
            display("command failed: '{}'", PathBuf::from(name).display())
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
        } {
            description("could not link file")
            display("could not create link from '{}' to '{}'", src.display(), dest.display())
        }
        LinkingDirectory {
            src: PathBuf,
            dest: PathBuf,
        } {
            description("could not symlink directory")
            display("could not create link from '{}' to '{}'", src.display(), dest.display())
        }
        CopyingDirectory {
            src: PathBuf,
            dest: PathBuf,
        } {
            description("could not copy directory")
            display("could not copy directory from '{}' to '{}'", src.display(), dest.display())
        }
        CopyingFile {
            src: PathBuf,
            dest: PathBuf,
        } {
            description("could not copy file")
            display("could not copy file from '{}' to '{}'", src.display(), dest.display())
        }
        RemovingFile {
            name: &'static str,
            path: PathBuf,
        } {
            description("could not remove file")
            display("could not remove '{}' file: '{}'", name, path.display())
        }
        CargoHome {
            description("couldn't find value of CARGO_HOME")
        }
        InvalidToolchainName(t: String) {
            description("invalid toolchain name")
            display("invalid toolchain name: '{}'", t)
        }
        InvalidProfile(t: String) {
            description("invalid profile name")
            display("invalid profile name: '{}'; valid names are: {}", t, valid_profile_names())
        }
        ChecksumFailed {
            url: String,
            expected: String,
            calculated: String,
        } {
            description("checksum failed")
            display("checksum failed, expected: '{}', calculated: '{}'",
                    expected,
                    calculated)
        }
        SignatureVerificationInternalError(msg: String) {
            description("internal error verifying signature")
            display("internal error verifying signature: {}", msg)
        }
        SignatureVerificationFailed {
            url: String,
        } {
            description("signature verification failed")
            display("signature verification failed for {}", url)
        }
        ComponentConflict {
            name: String,
            path: PathBuf,
        } {
            description("conflicting component")
            display("failed to install component: '{}', detected conflict: '{:?}'",
                    name,
                    path)
        }
        ComponentMissingFile {
            name: String,
            path: PathBuf,
        } {
            description("missing file in component")
            display("failure removing component '{}', directory does not exist: '{:?}'",
                    name,
                    path)
        }
        ComponentMissingDir {
            name: String,
            path: PathBuf,
        } {
            description("missing directory in component")
            display("failure removing component '{}', directory does not exist: '{:?}'",
                    name,
                    path)
        }
        CorruptComponent(name: String) {
            description("corrupt component manifest")
            display("component manifest for '{}' is corrupt", name)
        }
        ExtractingPackage {
            description("failed to extract package (perhaps you ran out of disk space?)")
        }
        BadInstallerVersion(v: String) {
            description("unsupported installer version")
            display("unsupported installer version: {}", v)
        }
        BadInstalledMetadataVersion(v: String) {
            description("unsupported metadata version in existing installation")
            display("unsupported metadata version in existing installation: {}", v)
        }
        ComponentDirPermissionsFailed {
            description("I/O error walking directory during install")
        }
        ComponentFilePermissionsFailed {
            description("error setting file permissions during install")
        }
        Parsing(e: toml::de::Error) {
            description("error parsing manifest")
        }
        UnsupportedVersion(v: String) {
            description("unsupported manifest version")
            display("manifest version '{}' is not supported", v)
        }
        MissingPackageForComponent(name: String) {
            description("missing package for component")
            display("server sent a broken manifest: missing package for component {}", name)
        }
        MissingPackageForRename(name: String) {
            description("missing package for the target of a rename")
            display("server sent a broken manifest: missing package for the target of a rename {}", name)
        }
        UnknownMetadataVersion(v: String) {
            description("unknown metadata version")
            display("unknown metadata version: '{}'", v)
        }
        ToolchainNotInstalled(t: String) {
            description("toolchain is not installed")
            display("toolchain '{}' is not installed", t)
        }
        OverrideToolchainNotInstalled(t: String) {
            description("override toolchain is not installed")
            display("override toolchain '{}' is not installed", t)
        }
        BinaryNotFound(bin: String, t: String, is_default: bool) {
            description("toolchain does not contain binary")
            display("'{}' is not installed for the toolchain '{}'{}", bin, t, install_msg(bin, t, *is_default))
        }
        NeedMetadataUpgrade {
            description("rustup's metadata is out of date. run `rustup self upgrade-data`")
        }
        UpgradeIoError {
            description("I/O error during upgrade")
        }
        ComponentsUnsupported(t: String) {
            description("toolchain does not support components")
            display("toolchain '{}' does not support components", t)
        }
        AddingRequiredComponent(t: String, c: String) {
            description("required component cannot be added")
            display("component {} was automatically added because it is required for toolchain '{}'",
                    c, t)
        }
        ParsingFallbackSettings(e: toml::de::Error) {
            description("error parsing settings")
        }
        ParsingSettings(e: toml::de::Error) {
            description("error parsing settings")
        }
        UnsupportedKind(v: String) {
            description("unsupported tar entry")
            display("tar entry kind '{}' is not supported", v)
        }
        BadPath(v: PathBuf) {
            description("bad path in tar")
            display("tar path '{}' is not supported", v.display())
        }
        InvalidPgpKey(v: PathBuf, error: pgp::errors::Error) {
            description("invalid PGP key"),
            display("unable to read the PGP key '{}'", v.display())
        }
        BrokenPartialFile {
            description("partially downloaded file may have been damaged and was removed, please try again")
        }
    }
}

#[derive(ThisError, Debug)]
pub enum RustupError {
    #[error("component download failed for {}", .component)]
    ComponentDownloadFailed {
        component: String,
        source: SyncError<retry::Error<Error>>,
    },
    #[error("component manifest for '{0}' is corrupt")]
    CorruptComponent(String),
    #[error("invalid toolchain name: '{0}'")]
    InvalidToolchainName(String),
    #[error("Unable to proceed. Could not locate working directory.")]
    LocatingWorkingDir {
        #[from]
        source: io::Error,
    },
    #[error("failed to set permissions for '{}'", .p.display())]
    SettingPermissions { p: PathBuf, source: io::Error },
    #[error("toolchain '{name}' does not support components")]
    ComponentsUnsupported {
        name: String,
        source: SyncError<Error>,
    },
    #[error("unable to read the PGP key '{}'", .path.display())]
    InvalidPgpKey { path: PathBuf, source: PGPError },
    #[error("Missing manifest in toolchain '{}'", .name)]
    MissingManifest { name: String },
    #[error("no release found for '{}'", .name)]
    MissingReleaseForToolchain {
        name: String,
        source: SyncError<Error>,
    },
    #[error("could not remove '{}' directory: '{}'", .name, .path.display())]
    RemovingDirectory {
        name: String,
        path: PathBuf,
        source: io::Error,
    },
    #[error("{}", component_unavailable_msg(&.components, &.manifest, &.toolchain))]
    RequestedComponentsUnavailable {
        components: Vec<Component>,
        manifest: Manifest,
        toolchain: String,
    },
    #[error("toolchain '{0}' is not installed")]
    ToolchainNotInstalled(String),
    #[error("no override and no default toolchain set")]
    ToolchainNotSelected,
    #[error("toolchain '{}' does not contain component {}{}", .name, .component, if let Some(suggestion) = .suggestion {
        format!("; did you mean '{}'?", suggestion)
    } else {
        "".to_string()
    })]
    UnknownComponent {
        name: String,
        component: String,
        suggestion: Option<String>,
    },
    #[error("unknown metadata version: '{0}'")]
    UnknownMetadataVersion(String),
}

pub struct PGPError(pub pgp::errors::Error);

impl std::error::Error for PGPError {}

impl Display for PGPError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Debug for PGPError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

/// Inspired by failure::SyncFailure, but not identical.
///
/// SyncError does not grant full safety: it will panic when errors are used
/// across threads (e.g. by threaded error logging libraries). This could be
/// fixed, but as we don't do that within rustup, it is not needed. If using
/// this code elsewhere, just hunt down and remove the unchecked unwraps.
pub struct SyncError<T: 'static> {
    inner: Arc<Mutex<T>>,
    proxy: Option<CauseProxy<T>>,
}

impl<T: std::error::Error + 'static> SyncError<T> {
    pub fn new(err: T) -> Self {
        let arc = Arc::new(Mutex::new(err));
        let proxy = match arc.lock().unwrap().source() {
            None => None,
            Some(source) => Some(CauseProxy::new(source, Arc::downgrade(&arc), 0)),
        };

        SyncError { inner: arc, proxy }
    }

    pub fn maybe<R>(r: std::result::Result<R, T>) -> std::result::Result<R, Self> {
        match r {
            Ok(v) => Ok(v),
            Err(e) => Err(SyncError::new(e)),
        }
    }

    pub fn unwrap(self) -> T {
        Arc::try_unwrap(self.inner).unwrap().into_inner().unwrap()
    }
}

impl<T: std::error::Error + 'static> std::error::Error for SyncError<T> {
    #[cfg(backtrace)]
    fn backtrace(&self) -> Option<&Backtrace> {}

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.proxy.as_ref().map(|x| x as _)
    }
}

impl<T> Display for SyncError<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.lock().unwrap().fmt(f)
    }
}

impl<T> Debug for SyncError<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.lock().unwrap().fmt(f)
    }
}

struct CauseProxy<T: 'static> {
    inner: Weak<Mutex<T>>,
    next: Option<Box<CauseProxy<T>>>,
    depth: u32,
}

impl<T: std::error::Error> CauseProxy<T> {
    fn new(err: &dyn std::error::Error, weak: Weak<Mutex<T>>, depth: u32) -> Self {
        // Can't allocate an object, or mutate the proxy safely during source(),
        // so we just take the hit at construction, recursively. We can't hold
        // references outside the mutex at all, so instead we remember how many
        // steps to get to this proxy. And if some error chain plays tricks, the
        // user gets both pieces.
        CauseProxy {
            inner: weak.clone(),
            depth,
            next: match err.source() {
                None => None,
                Some(source) => Some(Box::new(CauseProxy::new(source, weak, depth + 1))),
            },
        }
    }

    fn with_instance<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&(dyn std::error::Error + 'static)) -> R,
    {
        let arc = self.inner.upgrade().unwrap();
        {
            let e = arc.lock().unwrap();
            let mut source = e.source().unwrap();
            for _ in 0..self.depth {
                source = source.source().unwrap();
            }
            f(source)
        }
    }
}

impl<T: std::error::Error + 'static> std::error::Error for CauseProxy<T> {
    #[cfg(backtrace)]
    fn backtrace(&self) -> Option<&Backtrace> {}

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.next.as_ref().map(|x| x as _)
    }
}

impl<T> Display for CauseProxy<T>
where
    T: Display + std::error::Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with_instance(|i| std::fmt::Display::fmt(&i, f))
    }
}

impl<T> Debug for CauseProxy<T>
where
    T: Debug + std::error::Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.with_instance(|i| std::fmt::Debug::fmt(&i, f))
    }
}

pub fn valid_profile_names() -> String {
    Profile::names()
        .iter()
        .map(|s| format!("'{}'", s))
        .collect::<Vec<_>>()
        .join(", ")
}

fn component_unavailable_msg(cs: &[Component], manifest: &Manifest, toolchain: &str) -> String {
    assert!(!cs.is_empty());

    let mut buf = vec![];

    if cs.len() == 1 {
        let _ = write!(
            buf,
            "component {} is unavailable for download for channel {}{}",
            &cs[0].description(manifest),
            toolchain,
            if toolchain.starts_with("nightly") {
                "\nSometimes not all components are available in any given nightly."
            } else {
                ""
            }
        );
    } else {
        let same_target = cs
            .iter()
            .all(|c| c.target == cs[0].target || c.target.is_none());
        if same_target {
            let cs_str = cs
                .iter()
                .map(|c| format!("'{}'", c.short_name(manifest)))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = write!(
                buf,
                "some components unavailable for download for channel {}: {}\n{}",
                toolchain, cs_str, TOOLSTATE_MSG,
            );
        } else {
            let cs_str = cs
                .iter()
                .map(|c| c.description(manifest))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = write!(
                buf,
                "some components unavailable for download for channel {}: {}\n{}",
                toolchain, cs_str, TOOLSTATE_MSG,
            );
        }
    }

    String::from_utf8(buf).unwrap()
}

fn install_msg(bin: &str, toolchain: &str, is_default: bool) -> String {
    match component_for_bin(bin) {
        Some(c) => format!("\nTo install, run `rustup component add {}{}`", c, {
            if is_default {
                String::new()
            } else {
                format!(" --toolchain {}", toolchain)
            }
        }),
        None => String::new(),
    }
}
