use std::fmt::{self, Display};
use std::io;
use std::path::{Path, PathBuf};

use url::Url;

use crate::dist::TargetTriple;
use crate::dist::manifest::{Component, Manifest};
use crate::settings::MetadataVersion;
use crate::utils::units;
use crate::{dist::ToolchainDesc, toolchain::ToolchainName, utils::notify::NotificationLevel};

pub(crate) type NotifyHandler = dyn for<'a> Fn(Notification<'a>) + Sync + Send;

#[derive(Debug)]
pub enum Notification<'a> {
    Extracting(&'a Path, &'a Path),
    ComponentAlreadyInstalled(&'a str),
    CantReadUpdateHash(&'a Path),
    NoUpdateHash(&'a Path),
    ChecksumValid(&'a str),
    FileAlreadyDownloaded,
    CachedFileChecksumFailed,
    RollingBack,
    ExtensionNotInstalled(&'a str),
    NonFatalError(&'a anyhow::Error),
    MissingInstalledComponent(&'a str),
    /// The URL of the download is passed as the last argument, to allow us to track concurrent downloads.
    DownloadingComponent(&'a str, &'a TargetTriple, Option<&'a TargetTriple>, &'a str),
    InstallingComponent(&'a str, &'a TargetTriple, Option<&'a TargetTriple>),
    ComponentInstalled(&'a str, &'a TargetTriple, Option<&'a TargetTriple>),
    RemovingComponent(&'a str, &'a TargetTriple, Option<&'a TargetTriple>),
    RemovingOldComponent(&'a str, &'a TargetTriple, Option<&'a TargetTriple>),
    DownloadingManifest(&'a str),
    DownloadedManifest(&'a str, Option<&'a str>),
    DownloadingLegacyManifest,
    SkippingNightlyMissingComponent(&'a ToolchainDesc, &'a Manifest, &'a [Component]),
    ForcingUnavailableComponent(&'a str),
    ComponentUnavailable(&'a str, Option<&'a TargetTriple>),
    StrayHash(&'a Path),
    SignatureInvalid(&'a str),
    RetryingDownload(&'a str),
    CreatingDirectory(&'a str, &'a Path),
    LinkingDirectory(&'a Path, &'a Path),
    CopyingDirectory(&'a Path, &'a Path),
    RemovingDirectory(&'a str, &'a Path),
    DownloadingFile(&'a Url, &'a Path),
    /// Received the Content-Length of the to-be downloaded data with
    /// the respective URL of the download (for tracking concurrent downloads).
    DownloadContentLengthReceived(u64, Option<&'a str>),
    /// Received some data.
    DownloadDataReceived(&'a [u8], Option<&'a str>),
    /// Download has finished.
    DownloadFinished(Option<&'a str>),
    /// Download has failed.
    DownloadFailed(&'a str),
    NoCanonicalPath(&'a Path),
    ResumingPartialDownload,
    /// This would make more sense as a crate::notifications::Notification
    /// member, but the notification callback is already narrowed to
    /// utils::notifications by the time tar unpacking is called.
    SetDefaultBufferSize(usize),
    Error(String),
    UsingCurl,
    UsingReqwest,
    /// Renaming encountered a file in use error and is retrying.
    /// The InUse aspect is a heuristic - the OS specifies
    /// Permission denied, but as we work in users home dirs and
    /// running programs like virus scanner are known to cause this
    /// the heuristic is quite good.
    RenameInUse(&'a Path, &'a Path),
    CreatingRoot(&'a Path),
    CreatingFile(&'a Path),
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
    UpgradeRemovesToolchains,
    /// Both `rust-toolchain` and `rust-toolchain.toml` exist within a directory
    DuplicateToolchainFile {
        rust_toolchain: &'a Path,
        rust_toolchain_toml: &'a Path,
    },
}

impl Notification<'_> {
    pub(crate) fn level(&self) -> NotificationLevel {
        use self::Notification::*;
        match self {
            ChecksumValid(_)
            | NoUpdateHash(_)
            | FileAlreadyDownloaded
            | DownloadingLegacyManifest => NotificationLevel::Debug,
            Extracting(_, _)
            | DownloadingComponent(_, _, _, _)
            | InstallingComponent(_, _, _)
            | ComponentInstalled(_, _, _)
            | RemovingComponent(_, _, _)
            | RemovingOldComponent(_, _, _)
            | ComponentAlreadyInstalled(_)
            | RollingBack
            | DownloadingManifest(_)
            | SkippingNightlyMissingComponent(_, _, _)
            | RetryingDownload(_)
            | DownloadedManifest(_, _) => NotificationLevel::Info,
            CantReadUpdateHash(_)
            | ExtensionNotInstalled(_)
            | MissingInstalledComponent(_)
            | CachedFileChecksumFailed
            | ComponentUnavailable(_, _)
            | ForcingUnavailableComponent(_)
            | StrayHash(_) => NotificationLevel::Warn,
            NonFatalError(_) => NotificationLevel::Error,
            SignatureInvalid(_) => NotificationLevel::Warn,
            SetDefaultBufferSize(_) => NotificationLevel::Trace,
            CreatingDirectory(_, _)
            | RemovingDirectory(_, _)
            | LinkingDirectory(_, _)
            | CopyingDirectory(_, _)
            | DownloadingFile(_, _)
            | DownloadContentLengthReceived(_, _)
            | DownloadDataReceived(_, _)
            | DownloadFinished(_)
            | DownloadFailed(_)
            | ResumingPartialDownload
            | UsingCurl
            | UsingReqwest => NotificationLevel::Debug,
            RenameInUse(_, _) => NotificationLevel::Info,
            NoCanonicalPath(_) => NotificationLevel::Warn,
            Error(_) => NotificationLevel::Error,
            CreatingRoot(_) | CreatingFile(_) => NotificationLevel::Debug,
            FileDeletion(_, result) | DirectoryDeletion(_, result) => match result {
                Ok(_) => NotificationLevel::Debug,
                Err(_) => NotificationLevel::Warn,
            },
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
            UpgradeRemovesToolchains | DuplicateToolchainFile { .. } => NotificationLevel::Warn,
        }
    }
}

impl Display for Notification<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match self {
            Extracting(_, _) => write!(f, "extracting..."),
            ComponentAlreadyInstalled(c) => write!(f, "component {c} is up to date"),
            CantReadUpdateHash(path) => write!(
                f,
                "can't read update hash file: '{}', can't skip update...",
                path.display()
            ),
            NoUpdateHash(path) => write!(f, "no update hash at: '{}'", path.display()),
            ChecksumValid(_) => write!(f, "checksum passed"),
            FileAlreadyDownloaded => write!(f, "reusing previously downloaded file"),
            CachedFileChecksumFailed => write!(f, "bad checksum for cached download"),
            RollingBack => write!(f, "rolling back changes"),
            ExtensionNotInstalled(c) => write!(f, "extension '{c}' was not installed"),
            NonFatalError(e) => write!(f, "{e}"),
            MissingInstalledComponent(c) => {
                write!(f, "during uninstall component {c} was not found")
            }
            DownloadingComponent(c, h, t, _) => {
                if Some(h) == t.as_ref() || t.is_none() {
                    write!(f, "downloading component '{c}'")
                } else {
                    write!(f, "downloading component '{}' for '{}'", c, t.unwrap())
                }
            }
            InstallingComponent(c, h, t) => {
                if Some(h) == t.as_ref() || t.is_none() {
                    write!(f, "installing component '{c}'")
                } else {
                    write!(f, "installing component '{}' for '{}'", c, t.unwrap())
                }
            }
            ComponentInstalled(c, h, t) => match t {
                Some(t) if t != h => write!(f, "component '{c}' for '{t}' installed"),
                _ => write!(f, "component '{c}' installed"),
            },
            RemovingComponent(c, h, t) => {
                if Some(h) == t.as_ref() || t.is_none() {
                    write!(f, "removing component '{c}'")
                } else {
                    write!(f, "removing component '{}' for '{}'", c, t.unwrap())
                }
            }
            RemovingOldComponent(c, h, t) => {
                if Some(h) == t.as_ref() || t.is_none() {
                    write!(f, "removing previous version of component '{c}'")
                } else {
                    write!(
                        f,
                        "removing previous version of component '{}' for '{}'",
                        c,
                        t.unwrap()
                    )
                }
            }
            DownloadingManifest(t) => write!(f, "syncing channel updates for '{t}'"),
            DownloadedManifest(date, Some(version)) => {
                write!(f, "latest update on {date}, rust version {version}")
            }
            DownloadedManifest(date, None) => {
                write!(f, "latest update on {date}, no rust version")
            }
            DownloadingLegacyManifest => write!(f, "manifest not found. trying legacy manifest"),
            ComponentUnavailable(pkg, toolchain) => {
                if let Some(tc) = toolchain {
                    write!(f, "component '{pkg}' is not available on target '{tc}'")
                } else {
                    write!(f, "component '{pkg}' is not available")
                }
            }
            StrayHash(path) => write!(
                f,
                "removing stray hash found at '{}' in order to continue",
                path.display()
            ),
            SkippingNightlyMissingComponent(toolchain, manifest, components) => write!(
                f,
                "skipping nightly which is missing installed component{} '{}'",
                if components.len() > 1 { "s" } else { "" },
                components
                    .iter()
                    .map(|component| {
                        if component.target.as_ref() != Some(&toolchain.target) {
                            component.name(manifest)
                        } else {
                            component.short_name(manifest)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("', '")
            ),
            ForcingUnavailableComponent(component) => {
                write!(f, "Force-skipping unavailable component '{component}'")
            }
            SignatureInvalid(url) => write!(f, "Signature verification failed for '{url}'"),
            RetryingDownload(url) => write!(f, "retrying download for '{url}'"),
            CreatingDirectory(name, path) => {
                write!(f, "creating {} directory: '{}'", name, path.display())
            }
            Error(e) => write!(f, "error: '{e}'"),
            LinkingDirectory(_, dest) => write!(f, "linking directory from: '{}'", dest.display()),
            CopyingDirectory(src, _) => write!(f, "copying directory from: '{}'", src.display()),
            RemovingDirectory(name, path) => {
                write!(f, "removing {} directory: '{}'", name, path.display())
            }
            RenameInUse(src, dest) => write!(
                f,
                "retrying renaming '{}' to '{}'",
                src.display(),
                dest.display()
            ),
            SetDefaultBufferSize(size) => write!(
                f,
                "using up to {} of RAM to unpack components",
                units::Size::new(*size)
            ),
            DownloadingFile(url, _) => write!(f, "downloading file from: '{url}'"),
            DownloadContentLengthReceived(len, _) => write!(f, "download size is: '{len}'"),
            DownloadDataReceived(data, _) => write!(f, "received some data of size {}", data.len()),
            DownloadFinished(_) => write!(f, "download finished"),
            DownloadFailed(_) => write!(f, "download failed"),
            NoCanonicalPath(path) => write!(f, "could not canonicalize path: '{}'", path.display()),
            ResumingPartialDownload => write!(f, "resuming partial download"),
            UsingCurl => write!(f, "downloading with curl"),
            UsingReqwest => write!(f, "downloading with reqwest"),
            CreatingRoot(path) => write!(f, "creating temp root: {}", path.display()),
            CreatingFile(path) => write!(f, "creating temp file: {}", path.display()),
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
