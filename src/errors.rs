#![allow(clippy::large_enum_variant)]

use std::ffi::OsString;
use std::fmt::Debug;
#[cfg(not(windows))]
use std::io;
use std::io::Write;
use std::path::PathBuf;

use thiserror::Error as ThisError;
use url::Url;

use crate::{
    dist::{
        Channel, TargetTriple, ToolchainDesc,
        manifest::{Component, Manifest},
    },
    toolchain::{PathBasedToolchainName, ToolchainName},
};

/// A type erasing thunk for the retry crate to permit use with anyhow. See <https://github.com/dtolnay/anyhow/issues/149>
#[derive(Debug, ThisError)]
#[error(transparent)]
pub struct OperationError(pub anyhow::Error);

#[derive(ThisError, Debug)]
pub enum RustupError {
    #[error("partially downloaded file may have been damaged and was removed, please try again")]
    BrokenPartialFile,
    #[error("component download failed for {0}")]
    ComponentDownloadFailed(String),
    #[error("failure removing component '{name}', directory does not exist: '{}'", .path.display())]
    ComponentMissingDir { name: String, path: PathBuf },
    #[error("failure removing component '{name}', directory does not exist: '{}'", .path.display())]
    ComponentMissingFile { name: String, path: PathBuf },
    #[error("could not create {name} directory: '{}'", .path.display())]
    CreatingDirectory { name: &'static str, path: PathBuf },
    #[error("invalid toolchain name: '{0}'")]
    InvalidToolchainName(String),
    #[error("could not create link from '{}' to '{}'", .src.display(), .dest.display())]
    LinkingFile { src: PathBuf, dest: PathBuf },
    #[error("Unable to proceed. Could not locate working directory.")]
    LocatingWorkingDir,
    #[cfg(not(windows))]
    #[error("failed to set permissions for '{}'", .p.display())]
    SettingPermissions { p: PathBuf, source: io::Error },
    #[error("checksum failed for '{url}', expected: '{expected}', calculated: '{calculated}'")]
    ChecksumFailed {
        url: String,
        expected: String,
        calculated: String,
    },
    #[error("failed to install component: '{name}', detected conflict: '{}'", .path.display())]
    ComponentConflict { name: String, path: PathBuf },
    #[error("toolchain '{0}' does not support components")]
    ComponentsUnsupported(String),
    #[error("toolchain '{0}' does not support components (v1 manifest)")]
    ComponentsUnsupportedV1(String),
    #[error("component manifest for '{0}' is corrupt")]
    CorruptComponent(String),
    #[error("could not download file from '{url}' to '{}'", .path.display())]
    DownloadingFile { url: Url, path: PathBuf },
    #[error("could not download file from '{url}' to '{}'", .path.display())]
    DownloadNotExists { url: Url, path: PathBuf },
    #[error("Missing manifest in toolchain '{}'", .0)]
    MissingManifest(ToolchainDesc),
    #[error("server sent a broken manifest: missing package for component {0}")]
    MissingPackageForComponent(String),
    #[error("could not read {name} directory: '{}'", .path.display())]
    ReadingDirectory { name: &'static str, path: PathBuf },
    #[error("could not read {name} file: '{}'", .path.display())]
    ReadingFile { name: &'static str, path: PathBuf },
    #[error("could not parse {name} file: '{}'", .path.display())]
    ParsingFile { name: &'static str, path: PathBuf },
    #[error("could not remove '{}' directory: '{}'", .name, .path.display())]
    RemovingDirectory { name: &'static str, path: PathBuf },
    #[error("could not remove '{name}' file: '{}'", .path.display())]
    RemovingFile { name: &'static str, path: PathBuf },
    #[error("{}", component_unavailable_msg(.components, .manifest, .toolchain))]
    RequestedComponentsUnavailable {
        components: Vec<Component>,
        manifest: Manifest,
        toolchain: String,
    },
    #[error("command failed: '{}'", PathBuf::from(.name).display())]
    RunningCommand { name: OsString },
    #[error(
        "toolchain '{toolchain}' may not be able to run on this system\n\
        note: to build software for that platform, try `rustup target add {target_triple}` instead\n\
        note: add the `--force-non-host` flag to install the toolchain anyway"
    )]
    ToolchainIncompatible {
        toolchain: String,
        target_triple: TargetTriple,
    },
    #[error("toolchain '{0}' is not installable")]
    ToolchainNotInstallable(String),
    #[error(
        "toolchain '{name}' is not installed{}",
        if let ToolchainName::Official(t) = name {
            let t = if *is_active { "" } else { &format!(" {t}") };
            format!("\nhelp: run `rustup toolchain install{t}` to install it")
        } else {
            String::new()
        },
    )]
    ToolchainNotInstalled {
        name: ToolchainName,
        is_active: bool,
    },
    #[error("path '{0}' not found")]
    PathToolchainNotInstalled(PathBasedToolchainName),
    #[error(
        "rustup could not choose a version of {0} to run, because one wasn't specified explicitly, and no default is configured.\n\
        help: run 'rustup default stable' to download the latest stable release of Rust and set it as your default toolchain."
    )]
    ToolchainNotSelected(String),
    #[error("toolchain '{}' does not contain component {}{}{}", .desc, .component, suggest_message(.suggestion), if .component.contains("rust-std") {
        format!("\nnote: not all platforms have the standard library pre-compiled: https://doc.rust-lang.org/nightly/rustc/platform-support.html{}",
            if desc.channel == Channel::Nightly { "\nhelp: consider using `cargo build -Z build-std` instead" } else { "" }
        )
    } else { "".to_string() })]
    UnknownComponent {
        desc: ToolchainDesc,
        component: String,
        suggestion: Option<String>,
    },
    #[error("toolchain '{}' does not support target '{}'{}\n\
    note: you can see a list of supported targets with `rustc --print=target-list`\n\
    note: if you are adding support for a new target to rustc itself, see https://rustc-dev-guide.rust-lang.org/building/new-target.html", .desc, .target,
    suggest_message(.suggestion))]
    UnknownTarget {
        desc: ToolchainDesc,
        target: TargetTriple,
        suggestion: Option<String>,
    },
    #[error("toolchain '{}' does not have target '{}' installed{}\n", .desc, .target,
    suggest_message(.suggestion))]
    TargetNotInstalled {
        desc: ToolchainDesc,
        target: TargetTriple,
        suggestion: Option<String>,
    },
    #[error(
        "rustup executable proxies don't seem to work\n\
        help: this might be a bug in rustup, please open a new issue here:\n\
        help: https://github.com/rust-lang/rustup/issues/new"
    )]
    BrokenProxy,
    #[error("unknown metadata version: '{0}'")]
    UnknownMetadataVersion(String),
    #[error("manifest version '{0}' is not supported")]
    UnsupportedVersion(String),
    #[error("could not write {name} file: '{}'", .path.display())]
    WritingFile { name: &'static str, path: PathBuf },
    #[error("I/O Error")]
    IOError(#[from] std::io::Error),
}

fn suggest_message(suggestion: &Option<String>) -> String {
    if let Some(suggestion) = suggestion {
        format!("; did you mean '{suggestion}'?")
    } else {
        String::new()
    }
}

/// Returns a error message indicating that certain [`Component`]s are unavailable.
///
/// See also [`components_missing_msg`](../dist/dist/fn.components_missing_msg.html)
/// which generates error messages for component unavailability toolchain-wide operations.
///
/// # Panics
/// This function will panic when the collection of unavailable components `cs` is empty.
fn component_unavailable_msg(cs: &[Component], manifest: &Manifest, toolchain: &str) -> String {
    let mut buf = vec![];
    match cs {
        [] => panic!(
            "`component_unavailable_msg` should not be called with an empty collection of unavailable components"
        ),
        [c] => {
            let _ = writeln!(
                buf,
                "component {} is unavailable for download for channel '{}'",
                c.description(manifest),
                toolchain,
            );

            if toolchain.starts_with("nightly") {
                let _ = write!(
                    buf,
                    "Sometimes not all components are available in any given nightly. "
                );
            }
        }
        cs => {
            // More than one component
            let same_target = cs
                .iter()
                .all(|c| c.target == cs[0].target || c.target.is_none());

            let cs_str = if same_target {
                cs.iter()
                    .map(|c| format!("'{}'", c.short_name(manifest)))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                cs.iter()
                    .map(|c| c.description(manifest))
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            let _ = write!(
                buf,
                "some components are unavailable for download for channel '{toolchain}': {cs_str}"
            );

            if toolchain.starts_with("nightly") {
                let _ = write!(
                    buf,
                    "Sometimes not all components are available in any given nightly. "
                );
            }
        }
    }

    String::from_utf8(buf).unwrap()
}
