use std::error;
use std::path::PathBuf;
use std::fmt::{self, Display};
use std::io;
use temp;
use walkdir;
use toml;
use rustup_utils;
use rustup_error;
use manifest::Component;
use dist::TargetTriple;
use rustup_error::ErrorChain;

#[derive(Debug)]
pub enum Error {
    Utils(ErrorChain<rustup_utils::Error>),
    Temp(temp::Error),

    InvalidFileExtension,
    InvalidInstaller,
    InvalidTargetTriple(String),
    InvalidToolchainName(String),
    InvalidCustomToolchainName(String),
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
    ComponentDownloadFailed(Component, Box<ErrorChain<rustup_utils::Error>>),
    ObsoleteDistManifest,
    Parsing(Vec<toml::ParserError>),
    MissingKey(String),
    ExpectedType(&'static str, String),
    PackageNotFound(String),
    TargetNotFound(TargetTriple),
    MissingRoot,
    UnsupportedVersion(String),
    MissingPackageForComponent(Component),
    RequestedComponentsUnavailable(Vec<Component>),
    NoManifestFound(String, Box<Error>),
    Chained(Box<rustup_error::ErrorChain<Error>>),
    CreatingFile(PathBuf),
}

pub type Result<T> = ::std::result::Result<T, Error>;

extend_error!(Error: temp::Error, e => Error::Temp(e));
extend_error!(Error: rustup_error::ErrorChain<rustup_utils::Error>, e => Error::Utils(e));
extend_error!(Error: rustup_error::ErrorChain<Error>, e => Error::Chained(Box::new(e)));

impl error::Error for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            Utils(ref e) => error::Error::description(e),
            Temp(ref e) => error::Error::description(e),
            InvalidFileExtension => "invalid file extension",
            InvalidInstaller => "invalid installer",
            InvalidTargetTriple(_) => "invalid target triple",
            InvalidToolchainName(_) => "invalid toolchain name",
            InvalidCustomToolchainName(_) => "invalid custom toolchain name",
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
            CreatingFile(_) => "error creating file",
            Chained(ref e) => e.description(),
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
            Chained(ref e) => Some(e),
            InvalidFileExtension |
            InvalidInstaller |
            InvalidTargetTriple(_) |
            InvalidToolchainName(_) |
            InvalidCustomToolchainName(_) |
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
            CreatingFile(_) |
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
            InvalidTargetTriple(ref s) => write!(f, "invalid target triple: '{}'", s),
            InvalidToolchainName(ref s) => write!(f, "invalid toolchain name: '{}'", s),
            InvalidCustomToolchainName(ref s) => write!(f, "invalid custom toolchain name: '{}'", s),
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
            MissingPackageForComponent(ref c) => {
                write!(f,"server sent a broken manifest: missing package for component {}", c.name())
            }
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
                match **e {
                    Error::Utils(rustup_error::ErrorChain(rustup_utils::Error::Download404 { .. }, _)) => {
                        write!(f, "no release found for '{}'", ch)
                    }
                    _ => {
                        // FIXME: Need handle other common cases nicely,
                        // like dns lookup, network unavailable.
                        write!(f, "failed to download manifest for '{}': {}", ch, e)
                    }
                }
            }
            CreatingFile(ref p) => write!(f, "error creating file '{}'", p.display()),
            Chained(ref e) => write!(f, "{}", e),
        }
    }
}
