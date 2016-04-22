use std::path::PathBuf;
use std::error;
use std::fmt::{self, Display};
use std::io;

use rustup_dist::{self, temp};
use rustup_utils;
use rustup_dist::manifest::Component;

#[derive(Debug)]
pub enum Error {
    Install(rustup_dist::Error),
    Utils(rustup_utils::ErrorChain<rustup_utils::Error>),
    Temp(temp::Error),

    UnknownMetadataVersion(String),
    InvalidEnvironment,
    NoDefaultToolchain,
    PermissionDenied,
    ToolchainNotInstalled(String),
    UnknownHostTriple,
    InfiniteRecursion,
    NeedMetadataUpgrade,
    UpgradeIoError(io::Error),
    BadInstallerType(String),
    ComponentsUnsupported(String),
    UnknownComponent(String, Component),
    AddingRequiredComponent(String, Component),
    RemovingRequiredComponent(String, Component),
    NoExeName,
    NotSelfInstalled(PathBuf),
    CantSpawnWindowsGcExe,
    WindowsUninstallMadness(io::Error),
    SelfUpdateFailed,
    ReadStdin,
    Custom {
        id: String,
        desc: String,
    },
    TelemetryCleanupError(io::Error),
}

pub type Result<T> = ::std::result::Result<T, Error>;

extend_error!(Error: rustup_dist::Error, e => Error::Install(e));
extend_error!(Error: rustup_utils::ErrorChain<rustup_utils::Error>, e => Error::Utils(e));
extend_error!(Error: temp::Error, e => Error::Temp(e));

impl error::Error for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            Install(ref e) => error::Error::description(e),
            Utils(ref e) => error::Error::description(e),
            Temp(ref e) => error::Error::description(e),
            UnknownMetadataVersion(_) => "unknown metadata version",
            InvalidEnvironment => "invalid environment",
            NoDefaultToolchain => "no default toolchain configured",
            PermissionDenied => "permission denied",
            ToolchainNotInstalled(_) => "toolchain is not installed",
            UnknownHostTriple => "unknown host triple",
            InfiniteRecursion =>  "infinite recursion detected",
            NeedMetadataUpgrade => "rustup's metadata is out of date. run `rustup self upgrade-data`",
            UpgradeIoError(_) => "I/O error during upgrade",
            BadInstallerType(_) => "invalid extension for installer",
            ComponentsUnsupported(_) => "toolchain does not support componentsn",
            UnknownComponent(_ ,_) => "toolchain does not contain component",
            AddingRequiredComponent(_, _) => "required component cannot be added",
            RemovingRequiredComponent(_, _) => "required component cannot be removed",
            NoExeName => "couldn't determine self executable name",
            NotSelfInstalled(_) => "rustup is not installed",
            CantSpawnWindowsGcExe => "failed to spawn cleanup process",
            WindowsUninstallMadness(_) => "failure during windows uninstall",
            SelfUpdateFailed => "self-updater failed to replace multirust executable",
            ReadStdin => "unable to read from stdin for confirmation",
            Custom { ref desc, .. } => desc,
            TelemetryCleanupError(_) => "unable to remove old telemetry files"
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        use Error::*;
        match *self {
            Install(ref e) => Some(e),
            Utils(ref e) => Some(e),
            Temp(ref e) => Some(e),
            UpgradeIoError(ref e) => Some(e),
            WindowsUninstallMadness(ref e) => Some(e),
            TelemetryCleanupError(ref e) => Some(e),
            UnknownMetadataVersion(_) |
            InvalidEnvironment |
            NoDefaultToolchain |
            PermissionDenied |
            ToolchainNotInstalled(_) |
            UnknownHostTriple |
            InfiniteRecursion |
            NeedMetadataUpgrade |
            BadInstallerType(_) |
            ComponentsUnsupported(_) |
            UnknownComponent(_, _) |
            AddingRequiredComponent(_, _) |
            RemovingRequiredComponent(_, _) |
            NoExeName |
            NotSelfInstalled(_) |
            CantSpawnWindowsGcExe |
            SelfUpdateFailed |
            ReadStdin |
            Custom {..} => None,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use std::error::Error;
        use self::Error::*;
        match *self {
            Install(ref n) => n.fmt(f),
            Utils(ref n) => n.fmt(f),
            Temp(ref n) => n.fmt(f),
            UnknownMetadataVersion(ref ver) => write!(f, "unknown metadata version: '{}'", ver),
            InvalidEnvironment => write!(f, "invalid environment"),
            NoDefaultToolchain => write!(f, "no default toolchain configured"),
            PermissionDenied => write!(f, "permission denied"),
            ToolchainNotInstalled(ref name) => write!(f, "toolchain '{}' is not installed", name),
            UnknownHostTriple => write!(f, "unknown host triple"),
            InfiniteRecursion => {
                write!(f,
                       "infinite recursion detected: the command may not exist for this toolchain")
            }
            NeedMetadataUpgrade => write!(f, "{}", self.description()),
            UpgradeIoError(ref e) => {
                write!(f, "I/O error during upgrade: {}", e.description())
            }
            BadInstallerType(ref s) => {
                write!(f, "invalid extension for installer: '{}'", s)
            }
            ComponentsUnsupported(ref t) => {
                write!(f, "toolchain '{}' does not support components", t)
            }
            UnknownComponent(ref t, ref c) => {
                write!(f, "toolchain '{}' does not contain component '{}' for target '{}'", t, c.pkg, c.target)
            }
            AddingRequiredComponent(ref t, ref c) => {
                write!(f, "component '{}' for target '{}' is required for toolchain '{}' and cannot be re-added",
                       c.pkg, c.target, t)
            }
            RemovingRequiredComponent(ref t, ref c) => {
                write!(f, "component '{}' for target '{}' is required for toolchain '{}' and cannot be removed",
                       c.pkg, c.target, t)
            }
            NoExeName => write!(f, "couldn't determine self executable name"),
            NotSelfInstalled(ref p) => {
                write!(f, "rustup is not installed at '{}'", p.display())
            }
            CantSpawnWindowsGcExe => write!(f, "{}", self.description()),
            WindowsUninstallMadness(ref e) => write!(f, "failure during windows uninstall: {}", e),
            SelfUpdateFailed => write!(f, "{}", self.description()),
            ReadStdin => write!(f, "{}", self.description()),
            Custom { ref desc, .. } => write!(f, "{}", desc),
            TelemetryCleanupError(ref e) => write!(f, "Unable to delete telemetry files {}", e.description()),
        }
    }
}
