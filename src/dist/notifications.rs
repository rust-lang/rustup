use crate::dist::manifest::Component;
use crate::dist::temp;
use crate::dist::{TargetTriple, ToolchainDesc};
use crate::utils::notify::NotificationLevel;
use std::fmt::{self, Display};
use std::path::Path;

use super::manifest::Manifest;

#[derive(Debug)]
pub enum Notification<'a> {
    Utils(crate::utils::Notification<'a>),
    Temp(temp::Notification<'a>),

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
    DownloadingComponent(&'a str, &'a TargetTriple, Option<&'a TargetTriple>),
    InstallingComponent(&'a str, &'a TargetTriple, Option<&'a TargetTriple>),
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
            Temp(n) => n.level(),
            Utils(n) => n.level(),
            ChecksumValid(_)
            | NoUpdateHash(_)
            | FileAlreadyDownloaded
            | DownloadingLegacyManifest => NotificationLevel::Debug,
            Extracting(_, _)
            | DownloadingComponent(_, _, _)
            | InstallingComponent(_, _, _)
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
        }
    }
}

impl<'a> Display for Notification<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        use self::Notification::*;
        match self {
            Temp(n) => n.fmt(f),
            Utils(n) => n.fmt(f),
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
            DownloadingComponent(c, h, t) => {
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
        }
    }
}
