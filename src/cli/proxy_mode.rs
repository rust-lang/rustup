use std::ffi::OsString;

use anyhow::Result;

use crate::{
    cli::{common::set_globals, job, self_update},
    command::run_command_for_dir,
    toolchain::names::{LocalToolchainName, ResolvableLocalToolchainName},
    utils::utils::{self, ExitCode},
    Cfg,
};

#[cfg_attr(feature = "otel", tracing::instrument)]
pub fn main(arg0: &str) -> Result<ExitCode> {
    self_update::cleanup_self_updater()?;

    let ExitCode(c) = {
        let _setup = job::setup();

        let mut args = crate::process().args_os().skip(1);

        // Check for a + toolchain specifier
        let arg1 = args.next();
        let toolchain = arg1
            .as_ref()
            .map(|arg| arg.to_string_lossy())
            .filter(|arg| arg.starts_with('+'))
            .map(|name| ResolvableLocalToolchainName::try_from(&name.as_ref()[1..]))
            .transpose()?;

        // Build command args now while we know whether or not to skip arg 1.
        let cmd_args: Vec<_> = crate::process()
            .args_os()
            .skip(1 + toolchain.is_some() as usize)
            .collect();

        let cfg = set_globals(false, true)?;
        cfg.check_metadata_version()?;
        let toolchain = toolchain
            .map(|t| t.resolve(&cfg.get_default_host_triple()?))
            .transpose()?;
        direct_proxy(&cfg, arg0, toolchain, &cmd_args)?
    };

    Ok(ExitCode(c))
}

#[cfg_attr(feature = "otel", tracing::instrument(skip(cfg)))]
fn direct_proxy(
    cfg: &Cfg,
    arg0: &str,
    toolchain: Option<LocalToolchainName>,
    args: &[OsString],
) -> Result<ExitCode> {
    let cmd = match toolchain {
        None => cfg.create_command_for_dir(&utils::current_dir()?, arg0)?,
        Some(tc) => cfg.create_command_for_toolchain(&tc, false, arg0)?,
    };
    run_command_for_dir(cmd, arg0, args)
}
