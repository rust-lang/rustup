use rustup_dist::{self, temp};
use rustup_utils;
use rustup_dist::manifest::Component;

error_chain! {
    types {
        Error, ErrorKind, ChainErr, Result;
    }

    links {
        rustup_dist::Error, rustup_dist::ErrorKind, Dist;
        rustup_utils::Error, rustup_utils::ErrorKind, Utils;
    }

    foreign_links {
        temp::Error, Temp,
        "temporary file error";
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
        UnknownComponent(t: String, c: Component) {
            description("toolchain does not contain component")
            display("toolchain '{}' does not contain component '{}' for target '{}'", t, c.pkg, c.target)
        }
        AddingRequiredComponent(t: String, c: Component) {
            description("required component cannot be added")
            display("component '{}' for target '{}' is required for toolchain '{}' and cannot be re-added",
                    c.pkg, c.target, t)
        }
        RemovingRequiredComponent(t: String, c: Component) {
            description("required component cannot be removed")
            display("component '{}' for target '{}' is required for toolchain '{}' and cannot be removed",
                    c.pkg, c.target, t)
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
