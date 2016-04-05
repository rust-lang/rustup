extern crate multirust_dist;
#[macro_use]
extern crate multirust_utils;

#[macro_use]
extern crate clap;
extern crate regex;
extern crate hyper;
#[macro_use]
extern crate multirust;
extern crate term;
extern crate openssl;
extern crate itertools;
extern crate time;
extern crate rand;
extern crate scopeguard;
extern crate tempdir;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;
#[cfg(windows)]
extern crate user32;
#[cfg(windows)]
extern crate kernel32;
extern crate libc;

#[macro_use]
mod log;
mod cli;
mod common;
mod download_tracker;
mod multirust_mode;
mod proxy_mode;
mod setup_mode;
mod rustup_mode;
mod self_update;
mod tty;
mod job;
mod term2;

use std::env;
use std::path::PathBuf;
use multirust::{Error, Result};

fn main() {
    if let Err(e) = run_multirust() {
        err!("{}", e);
        std::process::exit(1);
    }
}

fn run_multirust() -> Result<()> {
    // Guard against infinite recursion
    let recursion_count = env::var("RUST_RECURSION_COUNT").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(0);
    if recursion_count > 5 {
        return Err(Error::InfiniteRecursion);
    }

    // Map MULTIRUST_ env vars to RUSTUP_
    // FIXME: Remove this soon to get it out of the proxy path
    make_environment_compatible();

    // The name of arg0 determines how the program is going to behave
    let arg0 = env::args().next().map(|a| PathBuf::from(a));
    let name = arg0.as_ref()
        .and_then(|a| a.file_stem())
        .and_then(|a| a.to_str());
    match name {
        Some("rustup") => {
            rustup_mode::main()
        }
        Some("multirust") => {
            multirust_mode::main()
        }
        Some(n) if n.starts_with("multirust-setup")||
                   n.starts_with("rustup-setup") => {
            // NB: The above check is only for the prefix of the file
            // name. Browsers rename duplicates to
            // e.g. multirust-setup(2), and this allows all variations
            // to work.
            setup_mode::main()
        }
        Some(n) if n.starts_with("multirust-gc-") => {
            // This is the final uninstallation stage on windows where
            // multirust deletes its own exe
            self_update::complete_windows_uninstall()
        }
        Some(n) if n.starts_with("multirust-") => {
            // This is for compatibility with previous revisions of
            // multirust-rs self-update, which expect multirust-rs to
            // be available on the server, downloads it to
            // ~/.multirust/tmp/multirust-$random, and execute it with
            // `self install` as the arguments.  FIXME: Verify this
            // works.
            if cfg!(windows) {
                self_update::install(false, false, "stable")
            } else {
                self_update::install(true, false, "stable")
            }
        }
        Some(_) => {
            proxy_mode::main()
        }
        None => {
            // Weird case. No arg0, or it's unparsable.
            Err(Error::NoExeName)
        }
    }
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
