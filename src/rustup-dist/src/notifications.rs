use std::path::Path;
use std::fmt::{self, Display};
use temp;
use rustup_utils;
use rustup_utils::notify::{NotificationLevel};
use manifest::Component;
use dist::TargetTriple;
use errors::*;

#[derive(Debug)]
pub enum Notification<'a> {
    Utils(rustup_utils::Notification<'a>),
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
    DownloadingComponent(&'a str, &'a TargetTriple, &'a TargetTriple),
    InstallingComponent(&'a str, &'a TargetTriple, &'a TargetTriple),
    DownloadingManifest(&'a str),
    DownloadingLegacyManifest,
    ManifestChecksumFailedHack,
}

impl<'a> From<rustup_utils::Notification<'a>> for Notification<'a> {
    fn from(n: rustup_utils::Notification<'a>) -> Notification<'a> {
        Notification::Utils(n)
    }
}

impl<'a> From<temp::Notification<'a>> for Notification<'a> {
    fn from(n: temp::Notification<'a>) -> Notification<'a> {
        Notification::Temp(n)
    }
}

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            Temp(ref n) => n.level(),
            Utils(ref n) => n.level(),
            ChecksumValid(_) | NoUpdateHash(_) |
            DownloadingLegacyManifest  => NotificationLevel::Verbose,
            Extracting(_, _) | SignatureValid(_)  |
            DownloadingComponent(_, _, _) |
            InstallingComponent(_, _, _) |
            ComponentAlreadyInstalled(_)  |
            RollingBack | DownloadingManifest(_) => NotificationLevel::Info,
            CantReadUpdateHash(_) | ExtensionNotInstalled(_) |
            MissingInstalledComponent(_) |
            ManifestChecksumFailedHack => NotificationLevel::Warn,
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
            DownloadingComponent(c, h, t) => {
                if h == t {
                    write!(f, "downloading component '{}'", c)
                } else {
                    write!(f, "downloading component '{}' for '{}'", c, t)
                }
            }
            InstallingComponent(c, h, t) => {
                if h == t {
                    write!(f, "installing component '{}'", c)
                } else {
                    write!(f, "installing component '{}' for '{}'", c, t)
                }
            }
            DownloadingManifest(t) => write!(f, "syncing channel updates for '{}'", t),
            DownloadingLegacyManifest => write!(f, "manifest not found. trying legacy manifest"),
            ManifestChecksumFailedHack => write!(f, "update not yet available, sorry! try again later"),
        }
    }
}

