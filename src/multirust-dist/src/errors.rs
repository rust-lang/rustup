use std::error;
use std::path::{Path, PathBuf};
use std::fmt::{self, Display};
use std::io;
use temp;
use walkdir;
use toml;
use multirust_utils;
use multirust_utils::notify::{self, NotificationLevel, Notifyable};
use manifest::Component;

#[derive(Debug)]
pub enum Notification<'a> {
    Utils(multirust_utils::Notification<'a>),
    Temp(temp::Notification<'a>),

    Extracting(&'a Path, &'a Path),
    ComponentAlreadyInstalled(&'a Component),
    CantReadUpdateHash(&'a Path),
    NoUpdateHash(&'a Path),
    ChecksumValid(&'a str),
    SignatureValid(&'a str),
    RollingBack,
    ExtensionNotInstalled(&'a Component),
    NonFatalError(&'a Error),
    MissingInstalledComponent(&'a str),
    DownloadingComponent(&'a str),
    InstallingComponent(&'a str),
    DownloadingManifest,
    DownloadingLegacyManifest,
}

#[derive(Debug)]
pub enum Error {
    Utils(multirust_utils::Error),
    Temp(temp::Error),

    InvalidFileExtension,
    InvalidInstaller,
    InvalidToolchainName(String),
    NotInstalledHere,
    UnsupportedHost(String),
    ChecksumFailed {
        url: String,
        expected: String,
        calculated: String,
    },
    ComponentConflict {
        name: String,
        path: PathBuf,
    },
    ComponentMissingFile {
        name: String,
        path: PathBuf,
    },
    ComponentMissingDir {
        name: String,
        path: PathBuf,
    },
    CorruptComponent(String),
    ExtractingPackage(io::Error),
    ExtensionNotFound(Component),
    InvalidChangeSet,
    NoGPG,
    BadInstallerVersion(String),
    BadInstalledMetadataVersion(String),
    ComponentDirPermissionsFailed(walkdir::Error),
    ComponentFilePermissionsFailed(io::Error),
    ComponentDownloadFailed(Component, multirust_utils::Error),
    ObsoleteDistManifest,
    Parsing(Vec<toml::ParserError>),
    MissingKey(String),
    ExpectedType(&'static str, String),
    PackageNotFound(String),
    TargetNotFound(String),
    MissingRoot,
    UnsupportedVersion(String),
    MissingPackageForComponent(Component),
    RequestedComponentsUnavailable(Vec<Component>),
    NoManifestFound(String, Box<Error>),
}

pub type Result<T> = ::std::result::Result<T, Error>;
pub type NotifyHandler<'a> = notify::NotifyHandler<'a, for<'b> Notifyable<Notification<'b>>>;
pub type SharedNotifyHandler =
    notify::SharedNotifyHandler<for<'b> Notifyable<Notification<'b>>>;

extend_error!(Error: temp::Error, e => Error::Temp(e));
extend_error!(Error: multirust_utils::Error, e => Error::Utils(e));

extend_notification!(Notification: multirust_utils::Notification, n => Notification::Utils(n));
extend_notification!(Notification: temp::Notification, n => Notification::Temp(n));

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            Temp(ref n) => n.level(),
            Utils(ref n) => n.level(),
            ChecksumValid(_) | NoUpdateHash(_) |
            DownloadingLegacyManifest  => NotificationLevel::Verbose,
            Extracting(_, _) | SignatureValid(_)  |
            DownloadingComponent(_) |
            InstallingComponent(_) |
            ComponentAlreadyInstalled(_)  |
            RollingBack | DownloadingManifest => NotificationLevel::Info,
            CantReadUpdateHash(_) | ExtensionNotInstalled(_) |
            MissingInstalledComponent(_) => NotificationLevel::Warn,
            NonFatalError(_) => NotificationLevel::Error,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            Temp(ref n) => n.fmt(f),
            Utils(ref n) => n.fmt(f),
            Extracting(_, _) => write!(f, "extracting..."),
            ComponentAlreadyInstalled(ref c) => {
                write!(f, "component '{}' for target '{}' is up to date",
                       c.pkg, c.target)
            }
            CantReadUpdateHash(path) => {
                write!(f,
                       "can't read update hash file: '{}', can't skip update...",
                       path.display())
            }
            NoUpdateHash(path) => write!(f, "no update hash at: '{}'", path.display()),
            ChecksumValid(_) => write!(f, "checksum passed"),
            SignatureValid(_) => write!(f, "signature valid"),
            RollingBack => write!(f, "rolling back changes"),
            ExtensionNotInstalled(c) => {
                write!(f, "extension '{}-{}' was not installed", c.pkg, c.target)
            }
            NonFatalError(e) => write!(f, "{}", e),
            MissingInstalledComponent(c) => write!(f, "during uninstall component {} was not found", c),
            DownloadingComponent(c) => write!(f, "downloading component '{}'", c),
            InstallingComponent(c) => write!(f, "installing component '{}'", c),
            DownloadingManifest => write!(f, "downloading toolchain manifest"),
            DownloadingLegacyManifest => write!(f, "manifest not found. trying legacy manifest"),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            Utils(ref e) => error::Error::description(e),
            Temp(ref e) => error::Error::description(e),
            InvalidFileExtension => "invalid file extension",
            InvalidInstaller => "invalid installer",
            InvalidToolchainName(_) => "invalid custom toolchain name",
            NotInstalledHere => "not installed here",
            UnsupportedHost(_) => "binary package not provided for host",
            ChecksumFailed {..} => "checksum failed",
            ComponentConflict {..} => "conflicting component",
            ComponentMissingFile {..} => "missing file in component",
            ComponentMissingDir {..} => "missing directory in component",
            CorruptComponent(_) => "corrupt component manifest",
            ExtractingPackage(_) => "failed to extract package",
            ExtensionNotFound(_) => "could not find extension",
            InvalidChangeSet => "invalid change-set",
            NoGPG => "could not find 'gpg' on PATH",
            BadInstallerVersion(_) => "unsupported installer version",
            BadInstalledMetadataVersion(_) => "unsupported metadata version in existing installation",
            ComponentDirPermissionsFailed(_) => "I/O error walking directory during install",
            ComponentFilePermissionsFailed(_) => "error setting file permissions during install",
            ComponentDownloadFailed(_, _) => "component download failed",
            ObsoleteDistManifest => "the server unexpectedly provided an obsolete version of the distribution manifest",
            Parsing(_) => "error parsing manifest",
            MissingKey(_) => "missing key",
            ExpectedType(_, _) => "expected type",
            PackageNotFound(_) => "package not found",
            TargetNotFound(_) => "target not found",
            MissingRoot => "manifest has no root package",
            UnsupportedVersion(_) => "unsupported manifest version",
            MissingPackageForComponent(_) => "missing package for component",
            RequestedComponentsUnavailable(_) => "some requested components are unavailable to download",
            NoManifestFound(_, _) => "no release found",
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        use self::Error::*;
        match *self {
            Utils(ref e) => Some(e),
            Temp(ref e) => Some(e),
            ComponentFilePermissionsFailed(ref e) => Some(e),
            ComponentDirPermissionsFailed(ref e) => Some(e),
            ExtractingPackage(ref e) => Some(e),
            ComponentDownloadFailed(_, ref e) => Some(e),
            NoManifestFound(_, ref e) => Some(e),
            InvalidFileExtension |
            InvalidInstaller |
            InvalidToolchainName(_) |
            NotInstalledHere |
            UnsupportedHost(_) |
            ChecksumFailed {..} |
            ComponentConflict {..} |
            ComponentMissingFile {..} |
            ComponentMissingDir {..} |
            CorruptComponent(_) |
            ExtensionNotFound(_) |
            InvalidChangeSet |
            NoGPG |
            BadInstallerVersion(_) |
            BadInstalledMetadataVersion(_) |
            ObsoleteDistManifest |
            Parsing(_) |
            MissingKey(_) |
            ExpectedType(_, _) |
            PackageNotFound(_) |
            TargetNotFound(_) |
            MissingRoot |
            UnsupportedVersion(_) |
            MissingPackageForComponent(_) |
            RequestedComponentsUnavailable(_) => None
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use self::Error::*;
        match *self {
            Temp(ref n) => n.fmt(f),
            Utils(ref n) => n.fmt(f),

            InvalidFileExtension => write!(f, "invalid file extension"),
            InvalidInstaller => write!(f, "invalid installer"),
            InvalidToolchainName(ref s) => write!(f, "invalid custom toolchain name: '{}'", s),
            NotInstalledHere => write!(f, "not installed here"),
            UnsupportedHost(ref spec) => {
                write!(f, "a binary package was not provided for: '{}'", spec)
            }
            ChecksumFailed { url: _, ref expected, ref calculated } => {
                write!(f,
                       "checksum failed, expected: '{}', calculated: '{}'",
                       expected,
                       calculated)
            }
            ComponentConflict { ref name, ref path } => {
                write!(f,
                       "failed to install component: '{}', detected conflict: '{:?}'",
                       name,
                       path)
            }
            ComponentMissingFile { ref name, ref path } => {
                write!(f,
                       "failure removing component '{}', file does not exist: '{:?}'",
                       name,
                       path)
            }
            ComponentMissingDir { ref name, ref path } => {
                write!(f,
                       "failure removing component '{}', directory does not exist: '{:?}'",
                       name,
                       path)
            }
            CorruptComponent(ref name) => write!(f, "component manifest for '{}' is corrupt", name),
            ExtractingPackage(ref error) => write!(f, "failed to extract package: {}", error),
            ExtensionNotFound(ref c) => {
                write!(f, "could not find extension: '{}-{}'", c.pkg, c.target)
            }
            InvalidChangeSet => write!(f, "invalid change-set"),
            NoGPG => {
                write!(f,
                       "could not find 'gpg': ensure it is on PATH or disable GPG verification")
            }
            BadInstallerVersion(ref v) => write!(f, "unsupported installer version: {}", v),
            BadInstalledMetadataVersion(ref v) => {
                write!(f,
                       "unsupported metadata version in existing installation: {}",
                       v)
            }
            ComponentDirPermissionsFailed(ref e) => {
                write!(f, "I/O error walking directory during install: {}", e)
            }
            ComponentFilePermissionsFailed(ref e) => {
                write!(f, "error setting file permissions during install: {}", e)
            }
            ComponentDownloadFailed(ref component, ref e) => {
                write!(f, "component download failed for {}-{}: {}", component.pkg, component.target, e)
            }
            ObsoleteDistManifest => {
                write!(f, "the server unexpectedly provided an obsolete version of the distribution manifest")
            },
            Parsing(ref n) => {
                for e in n {
                    try!(e.fmt(f));
                    try!(writeln!(f, ""));
                }
                Ok(())
            }
            MissingKey(ref n) => write!(f, "missing key: '{}'", n),
            ExpectedType(ref t, ref n) => write!(f, "expected type: '{}' for '{}'", t, n),
            PackageNotFound(ref n) => write!(f, "package not found: '{}'", n),
            TargetNotFound(ref n) => write!(f, "target not found: '{}'", n),
            MissingRoot => write!(f, "manifest has no root package"),
            UnsupportedVersion(ref v) => write!(f, "manifest version '{}' is not supported", v),
            MissingPackageForComponent(ref c) => write!(f,"manifest missing package for component {}", c.name()),
            RequestedComponentsUnavailable(ref cs) => {
                assert!(!cs.is_empty());
                if cs.len() == 1 {
                    write!(f, "component '{}' for '{}' is unavailable for download",
                           cs[0].pkg, cs[0].target)
                } else {
                    use itertools::Itertools;
                    let same_target = cs.iter().all(|c| c.target == cs[0].target);
                    if same_target {
                        let mut cs_strs = cs.iter().map(|c| format!("'{}'", c.pkg));
                        let cs_str = cs_strs.join(", ");
                        write!(f, "some components unavailable for download: {}",
                               cs_str)
                    } else {
                        let mut cs_strs = cs.iter().map(|c| format!("'{}' for '{}'", c.pkg, c.target));
                        let cs_str = cs_strs.join(", ");
                        write!(f, "some components unavailable for download: {}",
                               cs_str)
                    }
                }
            }
            NoManifestFound(ref ch, ref e) => {
                use multirust_utils::raw::DownloadError;
                use hyper::status::StatusCode::NotFound;
                match **e {
                    Error::Utils(multirust_utils::Error::DownloadingFile {
                        error: DownloadError::Status(NotFound),
                        ..
                    }) => {
                        write!(f, "no release found for '{}'", ch)
                    }
                    _ => {
                        // FIXME: Need handle other common cases nicely,
                        // like dns lookup, network unavailable.
                        write!(f, "failed to download manifest for '{}': {}", ch, e)
                    }
                }
            }
        }
    }
}
