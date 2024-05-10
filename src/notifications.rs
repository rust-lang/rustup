use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

use crate::{
    dist::{dist::ToolchainDesc, temp},
    toolchain::names::ToolchainName,
    utils::notify::NotificationLevel,
};

#[derive(Debug)]
pub(crate) enum Notification<'a> {
    Install(crate::dist::Notification<'a>),
    Utils(crate::utils::Notification<'a>),
    Temp(temp::Notification<'a>),

    SetDefaultToolchain(Option<&'a ToolchainName>),
    SetOverrideToolchain(&'a Path, &'a str),
    SetProfile(&'a str),
    SetSelfUpdate(&'a str),
    LookingForToolchain(&'a ToolchainDesc),
    ToolchainDirectory(&'a Path),
    UpdatingToolchain(&'a str),
    InstallingToolchain(&'a str),
    InstalledToolchain(&'a str),
    UsingExistingToolchain(&'a ToolchainDesc),
    UninstallingToolchain(&'a ToolchainName),
    UninstalledToolchain(&'a ToolchainName),
    UpdateHashMatches,
    UpgradingMetadata(&'a str, &'a str),
    MetadataUpgradeNotNeeded(&'a str),
    ReadMetadataVersion(&'a str),
    NonFatalError(&'a anyhow::Error),
    UpgradeRemovesToolchains,
    /// Both `rust-toolchain` and `rust-toolchain.toml` exist within a directory
    DuplicateToolchainFile {
        rust_toolchain: &'a Path,
        rust_toolchain_toml: &'a Path,
    },
}

impl<'a> From<crate::dist::Notification<'a>> for Notification<'a> {
    fn from(n: crate::dist::Notification<'a>) -> Self {
        Notification::Install(n)
    }
}
impl<'a> From<crate::utils::Notification<'a>> for Notification<'a> {
    fn from(n: crate::utils::Notification<'a>) -> Self {
        Notification::Utils(n)
    }
}
impl<'a> From<temp::Notification<'a>> for Notification<'a> {
    fn from(n: temp::Notification<'a>) -> Self {
        Notification::Temp(n)
    }
}

impl<'a> Notification<'a> {
    pub(crate) fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match self {
            Install(n) => n.level(),
            Utils(n) => n.level(),
            Temp(n) => n.level(),
            ToolchainDirectory(_)
            | LookingForToolchain(_)
            | InstallingToolchain(_)
            | UpdatingToolchain(_)
            | ReadMetadataVersion(_)
            | InstalledToolchain(_)
            | UpdateHashMatches => NotificationLevel::Verbose,
            SetDefaultToolchain(_)
            | SetOverrideToolchain(_, _)
            | SetProfile(_)
            | SetSelfUpdate(_)
            | UsingExistingToolchain(_)
            | UninstallingToolchain(_)
            | UninstalledToolchain(_)
            | UpgradingMetadata(_, _)
            | MetadataUpgradeNotNeeded(_) => NotificationLevel::Info,
            NonFatalError(_) => NotificationLevel::Error,
            UpgradeRemovesToolchains | DuplicateToolchainFile { .. } => NotificationLevel::Warn,
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
            SetDefaultToolchain(None) => write!(f, "default toolchain unset"),
            SetDefaultToolchain(Some(name)) => write!(f, "default toolchain set to '{name}'"),
            SetOverrideToolchain(path, name) => write!(
                f,
                "override toolchain for '{}' set to '{}'",
                path.display(),
                name
            ),
            SetProfile(name) => write!(f, "profile set to '{name}'"),
            SetSelfUpdate(mode) => write!(f, "auto-self-update mode set to '{mode}'"),
            LookingForToolchain(name) => write!(f, "looking for installed toolchain '{name}'"),
            ToolchainDirectory(path) => write!(f, "toolchain directory: '{}'", path.display()),
            UpdatingToolchain(name) => write!(f, "updating existing install for '{name}'"),
            InstallingToolchain(name) => write!(f, "installing toolchain '{name}'"),
            InstalledToolchain(name) => write!(f, "toolchain '{name}' installed"),
            UsingExistingToolchain(name) => write!(f, "using existing install for '{name}'"),
            UninstallingToolchain(name) => write!(f, "uninstalling toolchain '{name}'"),
            UninstalledToolchain(name) => write!(f, "toolchain '{name}' uninstalled"),
            UpdateHashMatches => write!(f, "toolchain is already up to date"),
            UpgradingMetadata(from_ver, to_ver) => write!(
                f,
                "upgrading metadata version from '{from_ver}' to '{to_ver}'"
            ),
            MetadataUpgradeNotNeeded(ver) => {
                write!(f, "nothing to upgrade: metadata version is already '{ver}'")
            }
            ReadMetadataVersion(ver) => write!(f, "read metadata version: '{ver}'"),
            NonFatalError(e) => write!(f, "{e}"),
            UpgradeRemovesToolchains => write!(
                f,
                "this upgrade will remove all existing toolchains. you will need to reinstall them"
            ),
            DuplicateToolchainFile {
                rust_toolchain,
                rust_toolchain_toml,
            } => write!(
                f,
                "both `{0}` and `{1}` exist. Using `{0}`",
                rust_toolchain
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(rust_toolchain))
                    .display(),
                rust_toolchain_toml
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(rust_toolchain_toml))
                    .display(),
            ),
        }
    }
}
