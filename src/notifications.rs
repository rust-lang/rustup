use std::fmt::{self, Display};
use std::path::PathBuf;

use crate::errors::*;

use crate::utils::notify::NotificationLevel;

#[derive(Debug)]
pub enum Notification<'a> {
    Install(crate::dist::Notification<'a>),
    Utils(crate::utils::Notification<'a>),

    WritingMetadataVersion(&'a str),
    ReadMetadataVersion(&'a str),
    NonFatalError(&'a Error),
    UpgradeRemovesToolchains,
    MissingFileDuringSelfUninstall(PathBuf),
}

impl<'a> From<crate::dist::Notification<'a>> for Notification<'a> {
    fn from(n: crate::dist::Notification<'a>) -> Notification<'a> {
        Notification::Install(n)
    }
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
            Install(ref n) => n.level(),
            Utils(ref n) => n.level(),
            WritingMetadataVersion(_) | ReadMetadataVersion(_) => NotificationLevel::Verbose,
            NonFatalError(_) => NotificationLevel::Error,
            UpgradeRemovesToolchains | MissingFileDuringSelfUninstall(_) => NotificationLevel::Warn,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> ::std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match *self {
            Install(ref n) => n.fmt(f),
            Utils(ref n) => n.fmt(f),
            WritingMetadataVersion(ver) => write!(f, "writing metadata version: '{}'", ver),
            ReadMetadataVersion(ver) => write!(f, "read metadata version: '{}'", ver),
            NonFatalError(e) => write!(f, "{}", e),
            UpgradeRemovesToolchains => write!(
                f,
                "this upgrade will remove all existing toolchains. you will need to reinstall them"
            ),
            MissingFileDuringSelfUninstall(ref p) => write!(
                f,
                "expected file does not exist to uninstall: {}",
                p.display()
            ),
        }
    }
}
