use std::error;
use std::fmt::{self, Display};
use std::path::Path;

use multirust_errors::Wrapped;
use rust_install::{self, utils, temp};
use rust_install::notify::{self, NotificationLevel, Notifyable};

#[derive(Debug)]
pub enum Notification<'a> {
    Install(rust_install::Notification<'a>),
    Utils(utils::Notification<'a>),
    Temp(temp::Notification<'a>),

    SetDefaultToolchain(&'a str),
    SetOverrideToolchain(&'a Path, &'a str),
    LookingForToolchain(&'a str),
    ToolchainDirectory(&'a Path, &'a str),
    UpdatingToolchain(&'a str),
    InstallingToolchain(&'a str),
    UsingExistingToolchain(&'a str),
    UninstallingToolchain(&'a str),
    UninstalledToolchain(&'a str),
    ToolchainNotInstalled(&'a str),

    UpgradingMetadata(&'a str, &'a str),
    WritingMetadataVersion(&'a str),
    ReadMetadataVersion(&'a str),
    NonFatalError(&'a Error),
}

#[derive(Debug)]
pub enum Error {
    Install(rust_install::Error),
    Utils(utils::Error),
    Temp(temp::Error),

    UnknownMetadataVersion(String),
    InvalidEnvironment,
    NoDefaultToolchain,
    PermissionDenied,
    ToolchainNotInstalled(String),
    UnknownHostTriple,
    InfiniteRecursion,
    Custom {
        id: String,
        desc: String,
    },
}

pub type Result<T> = ::std::result::Result<T, Error>;
pub type NotifyHandler<'a> = notify::NotifyHandler<'a, for<'b> Notifyable<Notification<'b>>>;
pub type SharedNotifyHandler = notify::SharedNotifyHandler<for<'b> Notifyable<Notification<'b>>>;

extend_error!(Error: rust_install::Error, e => Error::Install(e));
extend_error!(Error: utils::Error, e => Error::Utils(e));
extend_error!(Error: temp::Error, e => Error::Temp(e));

extend_notification!(Notification: rust_install::Notification, n => Notification::Install(n));
extend_notification!(Notification: utils::Notification, n => Notification::Utils(n));
extend_notification!(Notification: temp::Notification, n => Notification::Temp(n));

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            Install(ref n) => n.level(),
            Utils(ref n) => n.level(),
            Temp(ref n) => n.level(),
            ToolchainDirectory(_, _) |
            LookingForToolchain(_) |
            WritingMetadataVersion(_) |
            ReadMetadataVersion(_) => NotificationLevel::Verbose,
            SetDefaultToolchain(_) |
            SetOverrideToolchain(_, _) |
            UpdatingToolchain(_) |
            InstallingToolchain(_) |
            UsingExistingToolchain(_) |
            UninstallingToolchain(_) |
            UninstalledToolchain(_) |
            ToolchainNotInstalled(_) |
            UpgradingMetadata(_, _) => NotificationLevel::Info,
            NonFatalError(_) => NotificationLevel::Error,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            Install(ref n) => n.fmt(f),
            Utils(ref n) => n.fmt(f),
            Temp(ref n) => n.fmt(f),
            SetDefaultToolchain(name) => write!(f, "default toolchain set to '{}'", name),
            SetOverrideToolchain(path, name) => {
                write!(f,
                       "override toolchain for '{}' set to '{}'",
                       path.display(),
                       name)
            }
            LookingForToolchain(name) => write!(f, "looking for installed toolchain '{}'", name),
            ToolchainDirectory(path, _) => write!(f, "toolchain directory: '{}'", path.display()),
            UpdatingToolchain(name) => write!(f, "updating existing install for '{}'", name),
            InstallingToolchain(name) => write!(f, "installing toolchain '{}'", name),
            UsingExistingToolchain(name) => write!(f, "using existing install for '{}'", name),
            UninstallingToolchain(name) => write!(f, "uninstalling toolchain '{}'", name),
            UninstalledToolchain(name) => write!(f, "toolchain '{}' uninstalled", name),
            ToolchainNotInstalled(name) => write!(f, "no toolchain installed for '{}'", name),
            UpgradingMetadata(from_ver, to_ver) => {
                write!(f,
                       "upgrading metadata version from '{}' to '{}'",
                       from_ver,
                       to_ver)
            }
            WritingMetadataVersion(ver) => write!(f, "writing metadata version: '{}'", ver),
            ReadMetadataVersion(ver) => write!(f, "read metadata version: '{}'", ver),
            NonFatalError(e) => write!(f, "{}", e),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
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
            Custom { ref desc, .. } => write!(f, "{}", desc),
        }
    }
}

/// This impl gets around `Error` not implementing `error::Error` itself.
impl From<Error> for Box<error::Error> {
    fn from(e: Error) -> Box<error::Error> {
        Box::new(Wrapped::from(e))
    }
}
