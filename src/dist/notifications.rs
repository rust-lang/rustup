use crate::dist::dist::TargetTriple;
use crate::utils::notify::NotificationLevel;
use std::fmt::{self, Display};

#[derive(Debug)]
pub enum Notification<'a> {
    Utils(crate::utils::Notification<'a>),

    FileAlreadyDownloaded,
    CachedFileChecksumFailed,
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
            FileAlreadyDownloaded => NotificationLevel::Verbose,
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
