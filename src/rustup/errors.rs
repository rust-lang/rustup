use rustup_dist::{self, temp};
use rustup_utils;
use rustup_dist::manifest::Component;
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
            display("toolchain '{}' does not contain component {}", t, c.description())
        }
        AddingRequiredComponent(t: String, c: Component) {
            description("required component cannot be added")
            display("component {} is required for toolchain '{}' and cannot be re-added",
                    c.description(), t)
        }
        ParsingSettings(e: Vec<toml::ParserError>) {
            description("error parsing settings")
        }
        RemovingRequiredComponent(t: String, c: Component) {
            description("required component cannot be removed")
            display("component {} is required for toolchain '{}' and cannot be removed",
                    c.description(), t)
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
