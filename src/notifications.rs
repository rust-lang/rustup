use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

use crate::errors::*;

use crate::dist::temp;
use crate::utils::notify::NotificationLevel;

#[derive(Debug)]
pub enum Notification<'a> {
    Install(crate::dist::Notification<'a>),
    Utils(crate::utils::Notification<'a>),
    Temp(temp::Notification<'a>),

    SetDefaultToolchain(&'a str),
    SetOverrideToolchain(&'a Path, &'a str),
    LookingForToolchain(&'a str),
    ToolchainDirectory(&'a Path, &'a str),
    UpdatingToolchain(&'a str),
    InstallingToolchain(&'a str),
    InstalledToolchain(&'a str),
    UsingExistingToolchain(&'a str),
    UninstallingToolchain(&'a str),
    UninstalledToolchain(&'a str),
    ToolchainNotInstalled(&'a str),
    UpdateHashMatches,
    UpgradingMetadata(&'a str, &'a str),
    MetadataUpgradeNotNeeded(&'a str),
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
impl<'a> From<temp::Notification<'a>> for Notification<'a> {
    fn from(n: temp::Notification<'a>) -> Notification<'a> {
        Notification::Temp(n)
    }
}

impl<'a> Notification<'a> {
    pub fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match self {
            Install(n) => n.level(),
            Utils(n) => n.level(),
            Temp(n) => n.level(),
            ToolchainDirectory(_, _)
            | LookingForToolchain(_)
            | WritingMetadataVersion(_)
            | InstallingToolchain(_)
            | UpdatingToolchain(_)
            | ReadMetadataVersion(_)
            | InstalledToolchain(_)
            | UpdateHashMatches => NotificationLevel::Verbose,
            SetDefaultToolchain(_)
            | SetOverrideToolchain(_, _)
            | UsingExistingToolchain(_)
            | UninstallingToolchain(_)
            | UninstalledToolchain(_)
            | ToolchainNotInstalled(_)
            | UpgradingMetadata(_, _)
            | MetadataUpgradeNotNeeded(_) => NotificationLevel::Info,
            NonFatalError(_) => NotificationLevel::Error,
            UpgradeRemovesToolchains | MissingFileDuringSelfUninstall(_) => NotificationLevel::Warn,
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match self {
            Install(n) => n.fmt(f),
            Utils(n) => n.fmt(f),
            Temp(n) => n.fmt(f),
            SetDefaultToolchain(name) => write!(f, "default toolchain set to '{}'", name),
            SetOverrideToolchain(path, name) => write!(
                f,
                "override toolchain for '{}' set to '{}'",
                path.display(),
                name
            ),
            LookingForToolchain(name) => write!(f, "looking for installed toolchain '{}'", name),
            ToolchainDirectory(path, _) => write!(f, "toolchain directory: '{}'", path.display()),
            UpdatingToolchain(name) => write!(f, "updating existing install for '{}'", name),
            InstallingToolchain(name) => write!(f, "installing toolchain '{}'", name),
            InstalledToolchain(name) => write!(f, "toolchain '{}' installed", name),
            UsingExistingToolchain(name) => write!(f, "using existing install for '{}'", name),
            UninstallingToolchain(name) => write!(f, "uninstalling toolchain '{}'", name),
            UninstalledToolchain(name) => write!(f, "toolchain '{}' uninstalled", name),
            ToolchainNotInstalled(name) => write!(f, "no toolchain installed for '{}'", name),
            UpdateHashMatches => write!(f, "toolchain is already up to date"),
            UpgradingMetadata(from_ver, to_ver) => write!(
                f,
                "upgrading metadata version from '{}' to '{}'",
                from_ver, to_ver
            ),
            MetadataUpgradeNotNeeded(ver) => write!(
                f,
                "nothing to upgrade: metadata version is already '{}'",
                ver
            ),
            WritingMetadataVersion(ver) => write!(f, "writing metadata version: '{}'", ver),
            ReadMetadataVersion(ver) => write!(f, "read metadata version: '{}'", ver),
            NonFatalError(e) => write!(f, "{}", e),
            UpgradeRemovesToolchains => write!(
                f,
                "this upgrade will remove all existing toolchains. you will need to reinstall them"
            ),
            MissingFileDuringSelfUninstall(p) => write!(
                f,
                "expected file does not exist to uninstall: {}",
                p.display()
            ),
        }
    }
}
