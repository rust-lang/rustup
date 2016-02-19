use std::error;
use std::path::{Path, PathBuf};
use std::fmt::{self, Display};
use std::io;
use temp;
use utils;
use rust_manifest;
use walkdir;

use notify::{NotificationLevel, Notifyable};
use rust_manifest::Component;

#[derive(Debug)]
pub enum Notification<'a> {
    Utils(utils::Notification<'a>),
    Temp(temp::Notification<'a>),

    Extracting(&'a Path, &'a Path),
    UpdateHashMatches(&'a str),
    CantReadUpdateHash(&'a Path),
    NoUpdateHash(&'a Path),
    ChecksumValid(&'a str),
    SignatureValid(&'a str),
    RollingBack,
    ExtensionNotInstalled(&'a rust_manifest::Component),
    NonFatalError(&'a Error),
    MissingInstalledComponent(&'a str),
}

#[derive(Debug)]
pub enum Error {
    Utils(utils::Error),
    Temp(temp::Error),
    Manifest(rust_manifest::Error),

    InvalidFileExtension,
    InvalidInstaller,
    InvalidToolchainName,
    NotInstalledHere,
    InstallTypeNotPossible,
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
    ExtensionNotFound(rust_manifest::Component),
    InvalidChangeSet,
    NoGPG,
    BadInstallerVersion(String),
    BadInstalledMetadataVersion(String),
    ComponentDirPermissionsFailed(walkdir::Error),
    ComponentFilePermissionsFailed(io::Error),
    ComponentDownloadFailed(Component, utils::Error),
}

pub type Result<T> = ::std::result::Result<T, Error>;
pub type NotifyHandler<'a> = ::notify::NotifyHandler<'a, for<'b> Notifyable<Notification<'b>>>;
pub type SharedNotifyHandler =
    ::notify::SharedNotifyHandler<for<'b> Notifyable<Notification<'b>>>;

extend_error!(Error: temp::Error, e => Error::Temp(e));
extend_error!(Error: utils::Error, e => Error::Utils(e));
extend_error!(Error: rust_manifest::Error, e => Error::Manifest(e));

extend_notification!(Notification: utils::Notification, n => Notification::Utils(n));
extend_notification!(Notification: temp::Notification, n => Notification::Temp(n));

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            Temp(ref n) => n.level(),
            Utils(ref n) => n.level(),
            NoUpdateHash(_) => NotificationLevel::Verbose,
            Extracting(_, _) | ChecksumValid(_) | SignatureValid(_) => NotificationLevel::Normal,
            UpdateHashMatches(_) | RollingBack => NotificationLevel::Info,
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
            UpdateHashMatches(hash) => {
                write!(f, "update hash matches: {}, skipping update...", hash)
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
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        use Error::*;
        match *self {
            Utils(ref e) => error::Error::description(e),
            Temp(ref e) => error::Error::description(e),
            Manifest(ref e) => error::Error::description(e),
            InvalidFileExtension => "invalid file extension",
            InvalidInstaller => "invalid installer",
            InvalidToolchainName => "invalid custom toolchain name",
            NotInstalledHere => "not installed here",
            InstallTypeNotPossible => "install type not possible",
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
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        use Error::*;
        match *self {
            Utils(ref e) => Some(e),
            Temp(ref e) => Some(e),
            Manifest(ref e) => Some(e),
            ComponentFilePermissionsFailed(ref e) => Some(e),
            ComponentDirPermissionsFailed(ref e) => Some(e),
            ExtractingPackage(ref e) => Some(e),
            ComponentDownloadFailed(_, ref e) => Some(e),
            InvalidFileExtension |
            InvalidInstaller |
            InvalidToolchainName |
            NotInstalledHere |
            InstallTypeNotPossible |
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
            BadInstalledMetadataVersion(_) => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use self::Error::*;
        match *self {
            Temp(ref n) => n.fmt(f),
            Utils(ref n) => n.fmt(f),
            Manifest(ref n) => n.fmt(f),

            InvalidFileExtension => write!(f, "invalid file extension"),
            InvalidInstaller => write!(f, "invalid installer"),
            InvalidToolchainName => write!(f, "invalid custom toolchain name"),
            NotInstalledHere => write!(f, "not installed here"),
            InstallTypeNotPossible => write!(f, "install type not possible"),
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
        }
    }
}
