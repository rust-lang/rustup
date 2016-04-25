#![allow(dead_code)]

use std::path::PathBuf;

use rustup;
use rustup_dist::{self, temp};
use rustup_utils;

error_chain! {
    types {
        Error, ErrorKind, ChainErr, Result;
    }

    links {
        rustup::Error, rustup::ErrorKind, Rustup;
        rustup_dist::Error, rustup_dist::ErrorKind, Dist;
        rustup_utils::Error, rustup_utils::ErrorKind, Utils;
    }

    foreign_links {
        temp::Error, Temp,
        "temporary file error";
    }

    errors {
        PermissionDenied {
            description("permission denied")
        }
        ToolchainNotInstalled(t: String) {
            description("toolchain is not installed")
            display("toolchain '{}' is not installed", t)
        }
        InfiniteRecursion {
            description("infinite recursion detected")
        }
        NoExeName {
            description("couldn't determine self executable name")
        }
        NotSelfInstalled(p: PathBuf) {
            description("rustup is not installed")
            display("rustup is not installed at '{}'", p.display())
        }
        WindowsUninstallMadness {
            description("failure during windows uninstall")
        }
    }
}
