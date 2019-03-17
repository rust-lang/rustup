use crate::dist::dist::TargetTriple;
use crate::utils::notify::NotificationLevel;
use std::fmt::{self, Display};

#[derive(Debug)]
pub enum Notification<'a> {
    Utils(crate::utils::Notification<'a>),

    FileAlreadyDownloaded,
    CachedFileChecksumFailed,
    DownloadedManifest(&'a str, Option<&'a str>),
    DownloadingLegacyManifest,
    ManifestChecksumFailedHack,
    ComponentUnavailable(&'a str, Option<&'a TargetTriple>),
}

impl<'a> From<crate::utils::Notification<'a>> for Notification<'a> {
    fn from(n: crate::utils::Notification<'a>) -> Notification<'a> {
        Notification::Utils(n)
    }
}

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match *self {
            Utils(ref n) => n.level(),
            FileAlreadyDownloaded | DownloadingLegacyManifest => NotificationLevel::Verbose,
            ManifestChecksumFailedHack | DownloadedManifest(_, _) => NotificationLevel::Info,
            CachedFileChecksumFailed | ComponentUnavailable(_, _) => NotificationLevel::Warn,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            Utils(ref n) => n.fmt(f),
            FileAlreadyDownloaded => write!(f, "reusing previously downloaded file"),
            CachedFileChecksumFailed => write!(f, "bad checksum for cached download"),
            DownloadedManifest(date, Some(version)) => {
                write!(f, "latest update on {}, rust version {}", date, version)
            }
            DownloadedManifest(date, None) => {
                write!(f, "latest update on {}, no rust version", date)
            }
            DownloadingLegacyManifest => write!(f, "manifest not found. trying legacy manifest"),
            ManifestChecksumFailedHack => {
                write!(f, "update not yet available, sorry! try again later")
            }
            ComponentUnavailable(pkg, toolchain) => {
                if let Some(tc) = toolchain {
                    write!(
                        f,
                        "component '{}' is not available anymore on target '{}'",
                        pkg, tc
                    )
                } else {
                    write!(f, "component '{}' is not available anymore", pkg)
                }
            }
        }
    }
}
