#![allow(clippy::large_enum_variant)]

use std::ffi::OsString;
use std::fmt::Debug;
use std::io::{self, Write};
use std::path::PathBuf;

use thiserror::Error as ThisError;
use url::Url;

use crate::currentprocess::process;
use crate::dist::manifest::{Component, Manifest};

const TOOLSTATE_MSG: &str =
    "If you require these components, please install and use the latest successful build version,\n\
     which you can find at <https://rust-lang.github.io/rustup-components-history>.\n\nAfter determining \
     the correct date, install it with a command such as:\n\n    \
     rustup toolchain install nightly-2018-12-27\n\n\
     Then you can use the toolchain with commands such as:\n\n    \
     cargo +nightly-2018-12-27 build";

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
    #[error("unable to read the PGP key '{}'", .path.display())]
    InvalidPgpKey {
        path: PathBuf,
        source: anyhow::Error,
    },
    #[error("invalid toolchain name: '{0}'")]
    InvalidToolchainName(String),
    #[error("could not create link from '{}' to '{}'", .src.display(), .dest.display())]
    LinkingFile { src: PathBuf, dest: PathBuf },
    #[error("Unable to proceed. Could not locate working directory.")]
    LocatingWorkingDir,
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
    #[error("component manifest for '{0}' is corrupt")]
    CorruptComponent(String),
    #[error("could not download file from '{url}' to '{}'", .path.display())]
    DownloadingFile { url: Url, path: PathBuf },
    #[error("could not download file from '{url}' to '{}'", .path.display())]
    DownloadNotExists { url: Url, path: PathBuf },
    #[error("Missing manifest in toolchain '{}'", .name)]
    MissingManifest { name: String },
    #[error("server sent a broken manifest: missing package for component {0}")]
    MissingPackageForComponent(String),
    #[error("could not read {name} directory: '{}'", .path.display())]
    ReadingDirectory { name: &'static str, path: PathBuf },
    #[error("could not read {name} file: '{}'", .path.display())]
    ReadingFile { name: &'static str, path: PathBuf },
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
    #[error("toolchain '{0}' is not installable")]
    ToolchainNotInstallable(String),
    #[error("toolchain '{0}' is not installed")]
    ToolchainNotInstalled(String),
    #[error(
        "rustup could not choose a version of {} to run, because one wasn't specified explicitly, and no default is configured.\n{}",
        process().name().unwrap_or_else(|| "Rust".into()),
        "help: run 'rustup default stable' to download the latest stable release of Rust and set it as your default toolchain."
    )]
    ToolchainNotSelected,
    #[error("toolchain '{}' does not contain component {}{}{}", .name, .component, if let Some(suggestion) = .suggestion {
        format!("; did you mean '{suggestion}'?")
    } else {
        "".to_string()
    }, if .component.contains("rust-std") {
        format!("\nnote: not all platforms have the standard library pre-compiled: https://doc.rust-lang.org/nightly/rustc/platform-support.html{}",
            if name.contains("nightly") { "\nhelp: consider using `cargo build -Z build-std` instead" } else { "" }
        )
    } else { "".to_string() })]
    UnknownComponent {
        name: String,
        component: String,
        suggestion: Option<String>,
    },
    #[error("unknown metadata version: '{0}'")]
    UnknownMetadataVersion(String),
    #[error("manifest version '{0}' is not supported")]
    UnsupportedVersion(String),
    #[error("could not write {name} file: '{}'", .path.display())]
    WritingFile { name: &'static str, path: PathBuf },
}

fn remove_component_msg(cs: &Component, manifest: &Manifest, toolchain: &str) -> String {
    if cs.short_name_in_manifest() == "rust-std" {
        // We special-case rust-std as it's the stdlib so really you want to do
        // rustup target remove
        format!(
            "    rustup target remove --toolchain {} {}",
            toolchain,
            cs.target.as_deref().unwrap_or(toolchain)
        )
    } else {
        format!(
            "    rustup component remove --toolchain {}{} {}",
            toolchain,
            if let Some(target) = cs.target.as_ref() {
                format!(" --target {target}")
            } else {
                String::default()
            },
            cs.short_name(manifest)
        )
    }
}

fn component_unavailable_msg(cs: &[Component], manifest: &Manifest, toolchain: &str) -> String {
    assert!(!cs.is_empty());

    let mut buf = vec![];

    if cs.len() == 1 {
        let _ = writeln!(
            buf,
            "component {} is unavailable for download for channel '{}'",
            &cs[0].description(manifest),
            toolchain,
        );
        if toolchain.starts_with("nightly") {
            let _ = write!(
                buf,
                "Sometimes not all components are available in any given nightly. "
            );
        }
        let _ = write!(
            buf,
            "If you don't need the component, you can remove it with:\n\n{}",
            remove_component_msg(&cs[0], manifest, toolchain)
        );
    } else {
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

        let remove_msg = cs
            .iter()
            .map(|c| remove_component_msg(c, manifest, toolchain))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = write!(
            buf,
            "some components unavailable for download for channel '{toolchain}': {cs_str}\n\
            If you don't need the components, you can remove them with:\n\n{remove_msg}\n\n{TOOLSTATE_MSG}",
        );
    }

    String::from_utf8(buf).unwrap()
}
