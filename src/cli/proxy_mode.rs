use std::{path::PathBuf, process::ExitStatus};

use anyhow::Result;

use crate::toolchain::toolchain::Toolchain;
use crate::{
    cli::{common::set_globals, job, self_update},
    command::run_command_for_dir,
    currentprocess::process,
    toolchain::names::ResolvableLocalToolchainName,
};

#[cfg_attr(feature = "otel", tracing::instrument)]
pub async fn main(arg0: &str, current_dir: PathBuf) -> Result<ExitStatus> {
    self_update::cleanup_self_updater()?;

    let _setup = job::setup();

    let process = process();
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
    let cmd_args: Vec<_> = crate::currentprocess::process()
        .args_os()
        .skip(1 + toolchain.is_some() as usize)
        .collect();

    let cfg = set_globals(current_dir, false, true)?;
    cfg.check_metadata_version()?;
    let toolchain = toolchain
        .map(|t| t.resolve(&cfg.get_default_host_triple()?))
        .transpose()?;

    let toolchain = match toolchain {
        None => cfg.find_or_install_active_toolchain().await?.0,
        Some(tc) => Toolchain::from_local(&tc, false, &cfg).await?,
    };

    let cmd = toolchain.command(arg0)?;
    run_command_for_dir(cmd, arg0, &cmd_args)
}
