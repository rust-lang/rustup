
use std::path::Path;
use std::fmt::{self, Display};
use temp;
use utils;

use notify::{NotificationLevel, Notifyable};

pub enum Notification<'a> {
    Utils(utils::Notification<'a>),
    Temp(temp::Notification<'a>),

    Extracting(&'a Path, &'a Path),
    UpdateHashMatches(&'a str),
    CantReadUpdateHash(&'a Path),
    NoUpdateHash(&'a Path),
    ChecksumValid(&'a str),
}

pub enum Error {
    Utils(utils::Error),
    Temp(temp::Error),

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
}

pub type Result<T> = ::std::result::Result<T, Error>;
pub type NotifyHandler<'a> = ::notify::NotifyHandler<'a, for<'b> Notifyable<Notification<'b>>>;

extend_error!(Error: temp::Error, e => Error::Temp(e));
extend_error!(Error: utils::Error, e => Error::Utils(e));

extend_notification!(Notification: utils::Notification, n => Notification::Utils(n));
extend_notification!(Notification: temp::Notification, n => Notification::Temp(n));

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            Temp(ref n) => n.level(),
            Utils(ref n) => n.level(),
            NoUpdateHash(_) => NotificationLevel::Verbose,
            Extracting(_, _) | ChecksumValid(_) => NotificationLevel::Normal,
            UpdateHashMatches(_) => NotificationLevel::Info,
            CantReadUpdateHash(_) => NotificationLevel::Warn,
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
        }
    }
}
