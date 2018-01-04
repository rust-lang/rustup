use std::path::{Path, PathBuf};
use std::fmt::{self, Display};

use errors::*;

use rustup_dist::{self, temp};
use rustup_utils;
use rustup_utils::notify::NotificationLevel;

#[derive(Debug)]
pub enum Notification<'a> {
    Install(rustup_dist::Notification<'a>),
    Utils(rustup_utils::Notification<'a>),
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
    SetTelemetry(&'a str),

    TelemetryCleanupError(&'a Error),
}

impl<'a> From<rustup_dist::Notification<'a>> for Notification<'a> {
    fn from(n: rustup_dist::Notification<'a>) -> Notification<'a> {
        Notification::Install(n)
    }
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
            Install(ref n) => n.level(),
            Utils(ref n) => n.level(),
            Temp(ref n) => n.level(),
            ToolchainDirectory(_, _) |
            LookingForToolchain(_) |
            WritingMetadataVersion(_) |
            InstallingToolchain(_) |
            UpdatingToolchain(_) |
            ReadMetadataVersion(_) |
            InstalledToolchain(_) |
            UpdateHashMatches |
            TelemetryCleanupError(_) => NotificationLevel::Verbose,
            SetDefaultToolchain(_) |
            SetOverrideToolchain(_, _) |
            UsingExistingToolchain(_) |
            UninstallingToolchain(_) |
            UninstalledToolchain(_) |
            ToolchainNotInstalled(_) |
            UpgradingMetadata(_, _) |
            MetadataUpgradeNotNeeded(_) |
            SetTelemetry(_) => NotificationLevel::Info,
            NonFatalError(_) => NotificationLevel::Error,
            UpgradeRemovesToolchains |
            MissingFileDuringSelfUninstall(_) => NotificationLevel::Warn,
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
            InstalledToolchain(name) => write!(f, "toolchain '{}' installed", name),
            UsingExistingToolchain(name) => write!(f, "using existing install for '{}'", name),
            UninstallingToolchain(name) => write!(f, "uninstalling toolchain '{}'", name),
            UninstalledToolchain(name) => write!(f, "toolchain '{}' uninstalled", name),
            ToolchainNotInstalled(name) => write!(f, "no toolchain installed for '{}'", name),
            UpdateHashMatches => {
                write!(f, "toolchain is already up to date")
            }
            UpgradingMetadata(from_ver, to_ver) => {
                write!(f,
                       "upgrading metadata version from '{}' to '{}'",
                       from_ver,
                       to_ver)
            }
            MetadataUpgradeNotNeeded(ver) => {
                write!(f,
                       "nothing to upgrade: metadata version is already '{}'",
                       ver)
            }
            WritingMetadataVersion(ver) => write!(f, "writing metadata version: '{}'", ver),
            ReadMetadataVersion(ver) => write!(f, "read metadata version: '{}'", ver),
            NonFatalError(e) => write!(f, "{}", e),
            UpgradeRemovesToolchains => write!(f, "this upgrade will remove all existing toolchains. you will need to reinstall them"),
            MissingFileDuringSelfUninstall(ref p) => {
                write!(f, "expected file does not exist to uninstall: {}", p.display())
            }
            SetTelemetry(telemetry_status) => write!(f, "telemetry set to '{}'", telemetry_status),
            TelemetryCleanupError(e) => write!(f, "unable to remove old telemetry files: '{}'", e),
        }
    }
}
