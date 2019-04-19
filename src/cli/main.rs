//! The main rustup commandline application
//!
//! The rustup binary is a chimera, changing its behavior based on the
//! name of the binary. This is used most prominently to enable
//! rustup's tool 'proxies' - that is, rustup itself and the rustup
//! proxies are the same binary; when the binary is called 'rustup' or
//! 'rustup.exe' rustup behaves like the rustup commandline
//! application; when it is called 'rustc' it behaves as a proxy to
//! 'rustc'.
//!
//! This scheme is further used to distingush the rustup installer,
//! called 'rustup-init' which is again just the rustup binary under a
//! different name.

#![recursion_limit = "1024"]

#[macro_use]
mod log;
mod common;
mod download_tracker;
#[allow(deprecated)] // WORKAROUND https://github.com/rust-lang-nursery/error-chain/issues/254
mod errors;
mod help;
mod job;
mod proxy_mode;
mod rustup_mode;
mod self_update;
mod setup_mode;
mod term2;

use crate::errors::*;
use rustup::env_var::RUST_RECURSION_COUNT_MAX;
use std::env;
use std::path::PathBuf;

fn main() {
    if let Err(ref e) = run_rustup() {
        common::report_error(e);
        std::process::exit(1);
    }
}

fn run_rustup() -> Result<()> {
    // Guard against infinite proxy recursion. This mostly happens due to
    // bugs in rustup.
    do_recursion_guard()?;

    // The name of arg0 determines how the program is going to behave
    let arg0 = env::args().next().map(PathBuf::from);
    let name = arg0
        .as_ref()
        .and_then(|a| a.file_stem())
        .and_then(|a| a.to_str());

    match name {
        Some("rustup") => rustup_mode::main(),
        Some(n) if n.starts_with("rustup-setup") || n.starts_with("rustup-init") => {
            // NB: The above check is only for the prefix of the file
            // name. Browsers rename duplicates to
            // e.g. rustup-setup(2), and this allows all variations
            // to work.
            setup_mode::main()
        }
        Some(n) if n.starts_with("rustup-gc-") => {
            // This is the final uninstallation stage on windows where
            // rustup deletes its own exe
            self_update::complete_windows_uninstall()
        }
        Some(_) => proxy_mode::main(),
        None => {
            // Weird case. No arg0, or it's unparsable.
            Err(ErrorKind::NoExeName.into())
        }
    }
}

fn do_recursion_guard() -> Result<()> {
    let recursion_count = env::var("RUST_RECURSION_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if recursion_count > RUST_RECURSION_COUNT_MAX {
        return Err(ErrorKind::InfiniteRecursion.into());
    }

    Ok(())
}
