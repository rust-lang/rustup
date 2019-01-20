use crate::component_for_bin;
use rustup_dist::{self, temp};
use rustup_utils;
use toml;

error_chain! {
    links {
        Dist(rustup_dist::Error, rustup_dist::ErrorKind);
        Utils(rustup_utils::Error, rustup_utils::ErrorKind);
    }

    foreign_links {
        Temp(temp::Error);
    }

    errors {
        UnknownMetadataVersion(v: String) {
            description("unknown metadata version")
            display("unknown metadata version: '{}'", v)
        }
        ToolchainNotInstalled(t: String) {
            description("toolchain is not installed")
            display("toolchain '{}' is not installed", t)
        }
        OverrideToolchainNotInstalled(t: String) {
            description("override toolchain is not installed")
            display("override toolchain '{}' is not installed", t)
        }
        BinaryNotFound(bin: String, t: String, is_default: bool) {
            description("toolchain does not contain binary")
            display("'{}' is not installed for the toolchain '{}'{}", bin, t, install_msg(bin, t, *is_default))
        }
        NeedMetadataUpgrade {
            description("rustup's metadata is out of date. run `rustup self upgrade-data`")
        }
        UpgradeIoError {
            description("I/O error during upgrade")
        }
        BadInstallerType(s: String) {
            description("invalid extension for installer")
            display("invalid extension for installer: '{}'", s)
        }
        ComponentsUnsupported(t: String) {
            description("toolchain does not support components")
            display("toolchain '{}' does not support components", t)
        }
        UnknownComponent(t: String, c: String) {
            description("toolchain does not contain component")
            display("toolchain '{}' does not contain component {}", t, c)
        }
        AddingRequiredComponent(t: String, c: String) {
            description("required component cannot be added")
            display("component {} was automatically added because it is required for toolchain '{}'",
                    c, t)
        }
        ParsingSettings(e: toml::de::Error) {
            description("error parsing settings")
        }
        RemovingRequiredComponent(t: String, c: String) {
            description("required component cannot be removed")
            display("component {} is required for toolchain '{}' and cannot be removed",
                    c, t)
        }
        NoExeName {
            description("couldn't determine self executable name")
        }
        TelemetryCleanupError {
            description("unable to remove old telemetry files")
        }
        TelemetryAnalysisError {
            description("error analyzing telemetry files")
        }
    }
}

fn install_msg(bin: &str, toolchain: &str, is_default: bool) -> String {
    match component_for_bin(bin) {
        Some(c) => format!("\nTo install, run `rustup component add {}{}`", c, {
            if is_default {
                String::new()
            } else {
                format!(" --toolchain {}", toolchain)
            }
        }),
        None => String::new(),
    }
}
