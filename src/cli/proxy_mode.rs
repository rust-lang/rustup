use crate::common::set_globals;
use crate::job;
use anyhow::Result;
use rustup::command::run_command_for_dir;
use rustup::errors::SyncError;
use rustup::utils::utils::{self, ExitCode};
use rustup::Cfg;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process;

pub fn main() -> Result<()> {
    crate::self_update::cleanup_self_updater()?;

    let ExitCode(c) = {
        let _setup = job::setup();

        let mut args = env::args_os();

        let arg0 = args.next().map(PathBuf::from);
        let arg0 = arg0
            .as_ref()
            .and_then(|a| a.file_name())
            .and_then(std::ffi::OsStr::to_str);
        let arg0 = arg0.ok_or(crate::errors::CLIError::NoExeName)?;

        // Check for a toolchain specifier.
        let arg1 = args.next();
        let toolchain_arg = arg1
            .as_ref()
            .map(|arg| arg.to_string_lossy())
            .filter(|arg| arg.starts_with('+'));
        let toolchain = toolchain_arg.as_ref().map(|a| &a[1..]);

        // Build command args now while we know whether or not to skip arg 1.
        let cmd_args: Vec<_> = if toolchain.is_none() {
            env::args_os().skip(1).collect()
        } else {
            env::args_os().skip(2).collect()
        };

        let cfg = SyncError::maybe(set_globals(false, true))?;
        cfg.check_metadata_version()?;
        direct_proxy(&cfg, &arg0, toolchain, &cmd_args)?
    };

    process::exit(c)
}

fn direct_proxy(
    cfg: &Cfg,
    arg0: &str,
    toolchain: Option<&str>,
    args: &[OsString],
) -> Result<ExitCode> {
    let cmd = match toolchain {
        None => SyncError::maybe(
            cfg.create_command_for_dir(&SyncError::maybe(utils::current_dir())?, arg0),
        )?,
        Some(tc) => cfg.create_command_for_toolchain(tc, false, arg0)?,
    };
    Ok(SyncError::maybe(run_command_for_dir(cmd, arg0, args))?)
}
