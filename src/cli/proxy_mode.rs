use std::{
    path::PathBuf,
    process::{Command, ExitStatus},
};

use anyhow::Result;

use crate::{
    cli::{common::set_globals, job, self_update},
    command::run_command_for_dir,
    config::ActiveReason,
    process::Process,
    toolchain::ResolvableLocalToolchainName,
};

#[tracing::instrument(level = "trace", skip(process))]
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
    let toolchain_specified = toolchain.is_some();

    // Build command args now while we know whether or not to skip arg 1.
    let cmd_args: Vec<_> = process
        .args_os()
        .skip(1 + toolchain.is_some() as usize)
        .collect();

    let cfg = set_globals(current_dir, true, process)?;
    let toolchain = cfg.resolve_local_toolchain(toolchain).await?;
    let mut cmd = toolchain.command(arg0)?;
    set_env_source(
        &mut cmd,
        if toolchain_specified {
            Some(ActiveReason::CommandLine)
        } else if let Ok(Some((_, reason))) = cfg.active_toolchain() {
            Some(reason)
        } else {
            None
        },
    );
    run_command_for_dir(cmd, arg0, &cmd_args)
}

/// Set the `RUSTUP_TOOLCHAIN_SOURCE` environment variable to indicate how the toolchain was
/// determined.
fn set_env_source(cmd: &mut Command, reason: Option<ActiveReason>) {
    if let Some(reason) = reason {
        cmd.env("RUSTUP_TOOLCHAIN_SOURCE", reason.to_source());
    }
}
