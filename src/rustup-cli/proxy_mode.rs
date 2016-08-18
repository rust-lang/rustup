use common::set_globals;
use rustup::{Cfg};
use errors::*;
use rustup_utils::utils;
use rustup::command::run_command_for_dir;
use rustup::Toolchain;
use std::env;
use std::ffi::{OsString, OsStr};
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
        env::args_os().collect()
    } else {
        env::args_os().take(1).chain(env::args_os().skip(2)).collect()
    };

    let cfg = try!(set_globals(false));
    try!(cfg.check_metadata_version());
    try!(direct_proxy(&cfg, arg0, toolchain, &cmd_args));

    Ok(())
}

fn direct_proxy(cfg: &Cfg, arg0: &str, toolchain: Option<&str>, args: &[OsString]) -> Result<()> {
    let toolchain = match toolchain {
        None => try!(cfg.toolchain_for_dir(&try!(utils::current_dir()))).0,
        Some(tc) => try!(cfg.get_toolchain(tc, false)),
    };

    // Detect use of a target plugin
    for index in 1..(args.len()-1) {
        // Look for a `--target` argument
        if args[index] == OsStr::new("--target") {
            // Check it's followed by a value of the form `<target_plugin>:<target_desc>`
            if let Some(target) = args[index+1].to_str() {
                let mut split_iter = target.splitn(2, ":");
                if let (target_plugin, Some(target_desc)) = (
                    split_iter.next().expect("splitn should always return at least one result"),
                    split_iter.next()
                ) {
                    // It was, so proxy via the plugin, skipping these two arguments
                    let new_args: Vec<&OsStr> = args[0..index].iter()
                        .chain(args[(index+2)..].iter())
                        .map(OsString::as_os_str)
                        .collect();
                    return plugin_proxy(&toolchain, target_plugin, target_desc, &new_args);
                }
            }
        }
    }

    let cmd = try!(cfg.create_command(&toolchain, arg0));
    Ok(try!(run_command_for_dir(cmd, &args, &cfg)))
}

fn plugin_proxy(toolchain: &Toolchain, target_plugin: &str, target_desc: &str, args: &[&OsStr]) -> Result<()> {
    let plugin_name = format!("target-{}", target_plugin);
    // Install the plugin if required
    let plugin = try!(toolchain.install_plugin(&plugin_name, &[]));
    // Tell the plugin to add the target
    try!(plugin.target_add(target_desc));
    // Tell the plugin to run the original command
    Ok(try!(plugin.target_run(target_desc, args)))
}
