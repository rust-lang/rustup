use std::{path::PathBuf, process::ExitStatus};

use anyhow::Result;

use crate::{
    cli::{common::set_globals, job, self_update},
    command::run_command_for_dir,
    process::Process,
    toolchain::ResolvableLocalToolchainName,
};

#[tracing::instrument(level = "trace")]
pub async fn main(arg0: &str, current_dir: PathBuf, process: &Process) -> Result<ExitStatus> {
    self_update::cleanup_self_updater(process)?;

    let _setup = job::setup();
    let mut args = process.args_os().skip(1);

    // Check for a + toolchain specifier
    let arg1 = args.next();
    let toolchain = arg1
        .as_ref()
        .map(|arg| arg.to_string_lossy())
        .filter(|arg| arg.starts_with('+'))
        .map(|name| ResolvableLocalToolchainName::try_from(&name.as_ref()[1..]))
        .transpose()?;

    // Build command args now while we know whether or not to skip arg 1.
    let cmd_args: Vec<_> = process
        .args_os()
        .skip(1 + toolchain.is_some() as usize)
        .collect();

    let cfg = set_globals(current_dir, false, true, process)?;
    let cmd = cfg.resolve_local_toolchain(toolchain)?.command(arg0)?;
    run_command_for_dir(cmd, arg0, &cmd_args)
}
