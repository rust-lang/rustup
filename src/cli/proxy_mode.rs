use std::ffi::OsString;
use std::process;

use anyhow::Result;

use super::common::set_globals;
use super::job;
use super::self_update;
use crate::command::run_command_for_dir;
use crate::utils::utils::{self, ExitCode};
use crate::Cfg;

pub fn main(arg0: &str) -> Result<ExitCode> {
    self_update::cleanup_self_updater()?;

    let ExitCode(c) = {
        let _setup = job::setup();

        let mut args = crate::process().args_os().skip(1);

        // Check for a toolchain specifier.
        let arg1 = args.next();
        let toolchain_arg = arg1
            .as_ref()
            .map(|arg| arg.to_string_lossy())
            .filter(|arg| arg.starts_with('+'));
        let toolchain = toolchain_arg.as_ref().map(|a| &a[1..]);

        // Build command args now while we know whether or not to skip arg 1.
        let cmd_args: Vec<_> = if toolchain.is_none() {
            crate::process().args_os().skip(1).collect()
        } else {
            crate::process().args_os().skip(2).collect()
        };

        let cfg = set_globals(false, true)?;
        cfg.check_metadata_version()?;
        direct_proxy(&cfg, arg0, toolchain, &cmd_args)?
    };

    process::exit(c)
}

fn direct_proxy(
    cfg: &Cfg,
    arg0: &str,
    toolchain: Option<&str>,
    args: &[OsString],
) -> Result<ExitCode> {
    let mut cmd = match toolchain {
        None => cfg.create_command_for_dir(&utils::current_dir()?, arg0)?,
        Some(tc) => cfg.create_command_for_toolchain(tc, false, arg0)?,
    };
    cfg.rustup_safe_directories_env(&mut cmd)?;
    run_command_for_dir(cmd, arg0, args)
}
