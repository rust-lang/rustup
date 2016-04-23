use std::path::PathBuf;

use rustup_dist::{self, temp};
use rustup_utils;
use rustup_dist::manifest::Component;
use rustup_error::ForeignError;

pub type Result<T> = ::std::result::Result<T, ErrorChain>;

easy_error! {
    pub chain_error ChainError;

    pub error_chain ErrorChain;

    pub error Error {
        Install(e: rustup_dist::Error) {
            description(e.description())
            display("{}", e)
        }
        Utils(e: rustup_utils::Error) {
            description(e.description())
            display("{}", e)
        }
        Temp(e: ForeignError) {
            description(&e.description)
            display("{}", e.display)
        }

        UnknownMetadataVersion(v: String) {
            description("unknown metadata version")
            display("unknown metadata version: '{}'", v)
        }
        InvalidEnvironment {
            description("invalid environment")
        }
        NoDefaultToolchain {
            description("no default toolchain configured")
        }
        PermissionDenied {
            description("permission denied")
        }
        ToolchainNotInstalled(t: String) {
            description("toolchain is not installed")
            display("toolchain '{}' is not installed", t)
        }
        UnknownHostTriple {
            description("unknown host triple")
        }
        InfiniteRecursion {
            description("infinite recursion detected")
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
        NotSelfInstalled(p: PathBuf) {
            description("rustup is not installed")
            display("rustup is not installed at '{}'", p.display())
        }
        CantSpawnWindowsGcExe {
            description("failed to spawn cleanup process")
        }
        WindowsUninstallMadness {
            description("failure during windows uninstall")
        }
        SelfUpdateFailed {
            description("self-updater failed to replace multirust executable")
        }
        ReadStdin {
            description("unable to read from stdin for confirmation")
        }
        Custom {
            id: String,
            desc: String,
        } {
            description(&desc)
        }
        TelemetryCleanupError {
            description("unable to remove old telemetry files")
        }
    }
}

impl From<rustup_dist::ErrorChain> for ErrorChain {
    fn from(e: rustup_dist::ErrorChain) -> Self {
        ErrorChain(Error::Install(e.0), e.1)
    }
}

impl From<rustup_utils::ErrorChain> for ErrorChain {
    fn from(e: rustup_utils::ErrorChain) -> Self {
        ErrorChain(Error::Utils(e.0), e.1)
    }
}

impl From<temp::Error> for ErrorChain {
    fn from(e: temp::Error) -> Self {
        ErrorChain(Error::Temp(ForeignError::new(&e)), Some(Box::new(e)))
    }
}
