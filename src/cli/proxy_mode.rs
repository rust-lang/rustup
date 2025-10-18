use std::{path::PathBuf, process::ExitStatus};

use anyhow::Result;

use crate::{
    cli::{job, self_update},
    command::run_command_for_dir,
    config::{ActiveSource, Cfg},
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

    // Build command args now while we know whether or not to skip arg 1.
    let cmd_args: Vec<_> = process
        .args_os()
        .skip(1 + toolchain.is_some() as usize)
        .collect();

    let cfg = Cfg::from_env(current_dir, true, process)?;
    let (toolchain, source) = cfg
        .local_toolchain(match toolchain {
            Some(name) => Some((
                name.resolve(&cfg.get_default_host_triple()?)?,
                ActiveSource::CommandLine,
            )),
            None => None,
        })
        .await?;

    let mut cmd = toolchain.command(arg0)?;
    cmd.env("RUSTUP_TOOLCHAIN_SOURCE", source.to_string());
    run_command_for_dir(cmd, arg0, &cmd_args)
}
