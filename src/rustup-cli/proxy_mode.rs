use common::set_globals;
use rustup::{Cfg};
use errors::*;
use rustup_utils::utils;
use rustup::command::run_command_for_dir;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use job;

pub fn main() -> Result<()> {
    try!(::self_update::cleanup_self_updater());

    job::setup();

    let mut args = env::args();

    let arg0 = args.next().map(|a| PathBuf::from(a));
    let arg0 = arg0.as_ref()
        .and_then(|a| a.file_name())
        .and_then(|a| a.to_str());
    let ref arg0 = try!(arg0.ok_or(ErrorKind::NoExeName));

    // Check for a toolchain specifier.
    let arg1 = args.next();
    let toolchain = arg1.as_ref()
        .and_then(|arg1| {
            if arg1.starts_with("+") {
                Some(&arg1[1..])
            } else {
                None
            }
        });

    // Build command args now while we know whether or not to skip arg 1.
    let cmd_args: Vec<_> = if toolchain.is_none() {
        env::args_os().skip(1).collect()
    } else {
        env::args_os().skip(2).collect()
    };

    let cfg = try!(set_globals(false));
    try!(cfg.check_metadata_version());
    try!(direct_proxy(&cfg, arg0, toolchain, &cmd_args));

    Ok(())
}

fn direct_proxy(cfg: &Cfg, arg0: &str, toolchain: Option<&str>, args: &[OsString]) -> Result<()> {
    let cmd = match toolchain {
        None => try!(cfg.create_command_for_dir(&try!(utils::current_dir()), arg0)),
        Some(tc) => try!(cfg.create_command_for_toolchain(tc, arg0)),
    };
    Ok(try!(run_command_for_dir(cmd, arg0, args, &cfg)))
}

