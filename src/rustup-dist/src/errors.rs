use crate::manifest::{Component, Manifest};
use crate::temp;
use rustup_utils;
use std::io::{self, Write};
use std::path::PathBuf;
use toml;

pub const TOOLSTATE_MSG: &str = "if you require these components, please install and use the latest successful build version, \
                                which you can find at https://rust-lang-nursery.github.io/rust-toolstate, for example.\n\
                                rustup install nightly-2018-12-27";

error_chain! {
    links {
        Utils(rustup_utils::Error, rustup_utils::ErrorKind);
    }

    foreign_links {
        Temp(temp::Error);
        Io(io::Error);
    }

    errors {
        InvalidToolchainName(t: String) {
            description("invalid toolchain name")
            display("invalid toolchain name: '{}'", t)
        }
        InvalidCustomToolchainName(t: String) {
            description("invalid custom toolchain name")
            display("invalid custom toolchain name: '{}'", t)
        }
        ChecksumFailed {
            url: String,
            expected: String,
            calculated: String,
        } {
            description("checksum failed")
            display("checksum failed, expected: '{}', calculated: '{}'",
                    expected,
                    calculated)
        }
        ComponentConflict {
            name: String,
            path: PathBuf,
        } {
            description("conflicting component")
            display("failed to install component: '{}', detected conflict: '{:?}'",
                    name,
                    path)
        }
        ComponentMissingFile {
            name: String,
            path: PathBuf,
        } {
            description("missing file in component")
            display("failure removing component '{}', directory does not exist: '{:?}'",
                    name,
                    path)
        }
        ComponentMissingDir {
            name: String,
            path: PathBuf,
        } {
            description("missing directory in component")
            display("failure removing component '{}', directory does not exist: '{:?}'",
                    name,
                    path)
        }
        CorruptComponent(name: String) {
            description("corrupt component manifest")
            display("component manifest for '{}' is corrupt", name)
        }
        ExtractingPackage {
            description("failed to extract package")
        }
        BadInstallerVersion(v: String) {
            description("unsupported installer version")
            display("unsupported installer version: {}", v)
        }
        BadInstalledMetadataVersion(v: String) {
            description("unsupported metadata version in existing installation")
            display("unsupported metadata version in existing installation: {}", v)
        }
        ComponentDirPermissionsFailed {
            description("I/O error walking directory during install")
        }
        ComponentFilePermissionsFailed {
            description("error setting file permissions during install")
        }
        ComponentDownloadFailed(c: String) {
            description("component download failed")
            display("component download failed for {}", c)
        }
        Parsing(e: toml::de::Error) {
            description("error parsing manifest")
        }
        UnsupportedVersion(v: String) {
            description("unsupported manifest version")
            display("manifest version '{}' is not supported", v)
        }
        MissingPackageForComponent(name: String) {
            description("missing package for component")
            display("server sent a broken manifest: missing package for component {}", name)
        }
        MissingPackageForRename(name: String) {
            description("missing package for the target of a rename")
            display("server sent a broken manifest: missing package for the target of a rename {}", name)
        }
        RequestedComponentsUnavailable(c: Vec<Component>, manifest: Manifest) {
            description("some requested components are unavailable to download")
            display("{}", component_unavailable_msg(&c, &manifest))
        }
    }
}

fn component_unavailable_msg(cs: &[Component], manifest: &Manifest) -> String {
    assert!(!cs.is_empty());

    let mut buf = vec![];

    if cs.len() == 1 {
        let _ = write!(
            buf,
            "component {} is unavailable for download",
            &cs[0].description(manifest)
        );
    } else {
        use itertools::Itertools;
        let same_target = cs
            .iter()
            .all(|c| c.target == cs[0].target || c.target.is_none());
        if same_target {
            let mut cs_strs = cs.iter().map(|c| format!("'{}'", c.short_name(manifest)));
            let cs_str = cs_strs.join(", ");
            let _ = write!(
                    buf,
                    "some components unavailable for download: {}\n{}",
                    cs_str,
                    TOOLSTATE_MSG,
                );
        } else {
            let mut cs_strs = cs.iter().map(|c| c.description(manifest));
            let cs_str = cs_strs.join(", ");
            let _ = write!(
                    buf,
                    "some components unavailable for download: {}\n{}",
                    cs_str,
                    TOOLSTATE_MSG,
                );
        }
    }

    String::from_utf8(buf).expect("")
}
