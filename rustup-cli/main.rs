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

extern crate rustup_dist;
extern crate rustup_utils;
#[macro_use]
extern crate error_chain;

extern crate clap;
extern crate regex;
extern crate rustup;
extern crate term;
extern crate itertools;
extern crate time;
extern crate rand;
extern crate same_file;
extern crate scopeguard;
extern crate tempdir;
extern crate sha2;
extern crate markdown;
extern crate toml;
extern crate wait_timeout;

#[cfg(windows)]
extern crate gcc;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;
extern crate libc;

#[macro_use]
mod log;
mod common;
mod download_tracker;
mod proxy_mode;
mod setup_mode;
mod rustup_mode;
mod self_update;
mod job;
mod term2;
mod errors;
mod help;

use std::env;
use std::path::PathBuf;
use errors::*;
use rustup_dist::dist::TargetTriple;
use rustup::env_var::RUST_RECURSION_COUNT_MAX;

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

    // Do various hacks to clean up past messes
    do_compatibility_hacks();

    // The name of arg0 determines how the program is going to behave
    let arg0 = env::args().next().map(PathBuf::from);
    let name = arg0.as_ref()
        .and_then(|a| a.file_stem())
        .and_then(|a| a.to_str());

    match name {
        Some("rustup") => {
            rustup_mode::main()
        }
        Some(n) if n.starts_with("multirust-setup")||
                   n.starts_with("rustup-setup") ||
                   n.starts_with("rustup-init") => {
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
        Some(n) if n.starts_with("multirust-") => {
            // This is for compatibility with previous revisions of
            // multirust-rs self-update, which expect multirust-rs to
            // be available on the server, downloads it to
            // ~/.multirust/tmp/multirust-$random, and execute it with
            // `self install` as the arguments.  FIXME: Verify this
            // works.
            let opts = self_update::InstallOpts {
                default_host_triple: TargetTriple::from_host_or_build().to_string(),
                default_toolchain: "stable".to_string(),
                no_modify_path: false,
            };
            if cfg!(windows) {
                self_update::install(false, false, opts)
            } else {
                self_update::install(true, false, opts)
            }
        }
        Some(_) => {
            proxy_mode::main()
        }
        None => {
            // Weird case. No arg0, or it's unparsable.
            Err(ErrorKind::NoExeName.into())
        }
    }
}

fn do_recursion_guard() -> Result<()> {
    let recursion_count = env::var("RUST_RECURSION_COUNT").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    if recursion_count > RUST_RECURSION_COUNT_MAX {
        return Err(ErrorKind::InfiniteRecursion.into());
    }

    Ok(())
}

fn do_compatibility_hacks() {
    make_environment_compatible();
    fix_windows_reg_key();
    delete_multirust_bin();
}

// Convert any MULTIRUST_ env vars to RUSTUP_ and warn about them
fn make_environment_compatible() {
    let ref vars = ["HOME", "TOOLCHAIN", "DIST_ROOT", "UPDATE_ROOT", "GPG_KEY"];
    for var in vars {
        let ref mvar = format!("MULTIRUST_{}", var);
        let ref rvar = format!("RUSTUP_{}", var);
        let mval = env::var_os(mvar);
        let rval = env::var_os(rvar);

        match (mval, rval) {
            (Some(mval), None) => {
                env::set_var(rvar, mval);
                warn!("environment variable {} is deprecated. Use {}.", mvar, rvar);
            }
            _ => ()
        }
    }
}

// #261 We previously incorrectly set HKCU/Environment/PATH to a
// REG_SZ type, when it should be REG_EXPAND_SZ. Silently fix it.
#[cfg(windows)]
fn fix_windows_reg_key() {
    use winreg::RegKey;
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let env = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE);

    let env = if let Ok(e) = env { e } else { return };

    let path = env.get_raw_value("PATH");

    let mut path = if let Ok(p) = path { p } else { return };

    if path.vtype == RegType::REG_EXPAND_SZ { return }

    path.vtype = RegType::REG_EXPAND_SZ;

    let _ = env.set_raw_value("PATH", &path);
}

#[cfg(not(windows))]
fn fix_windows_reg_key() { }

// rustup used to be called 'multirust'. This deletes the old bin.
fn delete_multirust_bin() {
    use rustup_utils::utils;
    use std::env::consts::EXE_SUFFIX;
    use std::fs;

    if let Ok(home) = utils::cargo_home() {
        let legacy_bin = home.join(format!("bin/multirust{}", EXE_SUFFIX));
        if legacy_bin.exists() {
            let _ = fs::remove_file(legacy_bin);
        }
    }
}
