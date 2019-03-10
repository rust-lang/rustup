#![allow(dead_code)]

use std::io;
use std::path::PathBuf;

use error_chain::error_chain;
use error_chain::error_chain_processing;
use error_chain::{impl_error_chain_kind, impl_error_chain_processed, impl_extract_backtrace};
use rustup::dist::temp;

error_chain! {
    links {
        Rustup(rustup::Error, rustup::ErrorKind);
        Dist(rustup::dist::Error, rustup::dist::ErrorKind);
        Utils(rustup::utils::Error, rustup::utils::ErrorKind);
    }

    foreign_links {
        Temp(temp::Error);
        Io(io::Error);
        Term(term::Error);
    }

    errors {
        PermissionDenied {
            description("permission denied")
        }
        ToolchainNotInstalled(t: String) {
            description("toolchain is not installed")
            display("toolchain '{}' is not installed", t)
        }
        InvalidToolchainName(t: String) {
            description("invalid toolchain name")
            display("invalid toolchain name: '{}'", t)
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
