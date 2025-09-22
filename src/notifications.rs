use std::fmt::{self, Display};
use std::io;
use std::path::{Path, PathBuf};

use crate::settings::MetadataVersion;
use crate::{dist::ToolchainDesc, toolchain::ToolchainName, utils::notify::NotificationLevel};

#[derive(Debug)]
pub enum Notification<'a> {
    Install(crate::dist::Notification<'a>),
    Utils(crate::utils::Notification<'a>),
    CreatingRoot(&'a Path),
    CreatingFile(&'a Path),
    CreatingDirectory(&'a Path),
    FileDeletion(&'a Path, io::Result<()>),
    DirectoryDeletion(&'a Path, io::Result<()>),
    SetAutoInstall(&'a str),
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
    UpgradingMetadata(MetadataVersion, MetadataVersion),
    MetadataUpgradeNotNeeded(MetadataVersion),
    ReadMetadataVersion(MetadataVersion),
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

impl Notification<'_> {
    pub(crate) fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match self {
            Install(n) => n.level(),
            Utils(n) => n.level(),
            CreatingRoot(_) | CreatingFile(_) | CreatingDirectory(_) => NotificationLevel::Debug,
            FileDeletion(_, result) | DirectoryDeletion(_, result) => {
                if result.is_ok() {
                    NotificationLevel::Debug
                } else {
                    NotificationLevel::Warn
                }
            }
            ToolchainDirectory(_)
            | LookingForToolchain(_)
            | InstallingToolchain(_)
            | UpdatingToolchain(_)
            | ReadMetadataVersion(_)
            | InstalledToolchain(_)
            | UpdateHashMatches => NotificationLevel::Debug,
            SetAutoInstall(_)
            | SetDefaultToolchain(_)
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

impl Display for Notification<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match self {
            Install(n) => n.fmt(f),
            Utils(n) => n.fmt(f),
            CreatingRoot(path) => write!(f, "creating temp root: {}", path.display()),
            CreatingFile(path) => write!(f, "creating temp file: {}", path.display()),
            CreatingDirectory(path) => write!(f, "creating temp directory: {}", path.display()),
            FileDeletion(path, result) => {
                if result.is_ok() {
                    write!(f, "deleted temp file: {}", path.display())
                } else {
                    write!(f, "could not delete temp file: {}", path.display())
                }
            }
            DirectoryDeletion(path, result) => {
                if result.is_ok() {
                    write!(f, "deleted temp directory: {}", path.display())
                } else {
                    write!(f, "could not delete temp directory: {}", path.display())
                }
            }
            SetAutoInstall(auto) => write!(f, "auto install set to '{auto}'"),
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
