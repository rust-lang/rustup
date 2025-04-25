use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::warn;
use tracing_subscriber::{EnvFilter, Registry, reload::Handle};

use crate::{
    cli::{
        common::{self, update_console_filter},
        self_update::{self, InstallOpts},
    },
    dist::Profile,
    process::Process,
    toolchain::MaybeOfficialToolchainName,
    utils,
};

/// The installer for rustup
#[derive(Debug, Parser)]
#[command(
    name = "rustup-init",
    bin_name = "rustup-init[EXE]",
    version = common::version(),
    before_help = format!("rustup-init {}", common::version()),
)]
struct RustupInit {
    /// Set log level to 'DEBUG' if 'RUSTUP_LOG' is unset
    #[arg(short, long, conflicts_with = "quiet")]
    verbose: bool,

    /// Disable progress output, set log level to 'WARN' if 'RUSTUP_LOG' is unset
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// Disable confirmation prompt
    #[arg(short = 'y')]
    no_prompt: bool,

    /// Choose a default host triple
    #[arg(long)]
    default_host: Option<String>,

    /// Choose a default toolchain to install. Use 'none' to not install any toolchains at all
    #[arg(long)]
    default_toolchain: Option<MaybeOfficialToolchainName>,

    #[arg(long, value_enum, default_value_t)]
    profile: Profile,

    /// Comma-separated list of component names to also install
    #[arg(short, long, value_delimiter = ',')]
    component: Vec<String>,

    /// Comma-separated list of target names to also install
    #[arg(short, long, value_delimiter = ',')]
    target: Vec<String>,

    /// Don't update any existing default toolchain after install
    #[arg(long)]
    no_update_default_toolchain: bool,

    /// Don't configure the PATH environment variable
    #[arg(long)]
    no_modify_path: bool,

    /// Secret command used during self-update. Not for users
    #[arg(long, hide = true)]
    self_replace: bool,

    /// Internal testament dump used during CI. Not for users
    #[arg(long, hide = true)]
    dump_testament: bool,
}

#[tracing::instrument(level = "trace", skip(process, console_filter))]
pub async fn main(
    current_dir: PathBuf,
    process: &Process,
    console_filter: Handle<EnvFilter, Registry>,
) -> Result<utils::ExitCode> {
    use clap::error::ErrorKind;

    let RustupInit {
        verbose,
        quiet,
        no_prompt,
        default_host,
        default_toolchain,
        profile,
        component,
        target,
        no_update_default_toolchain,
        no_modify_path,
        self_replace,
        dump_testament,
    } = match RustupInit::try_parse() {
        Ok(args) => args,
        Err(e) if [ErrorKind::DisplayHelp, ErrorKind::DisplayVersion].contains(&e.kind()) => {
            write!(process.stdout().lock(), "{e}")?;
            return Ok(utils::ExitCode(0));
        }
        Err(e) => return Err(e.into()),
    };

    if self_replace {
        return self_update::self_replace(process);
    }

    if dump_testament {
        return common::dump_testament(process);
    }

    if profile == Profile::Complete {
        warn!("{}", common::WARN_COMPLETE_PROFILE);
    }

    update_console_filter(process, &console_filter, quiet, verbose);

    let opts = InstallOpts {
        default_host_triple: default_host,
        default_toolchain,
        profile,
        no_modify_path,
        no_update_toolchain: no_update_default_toolchain,
        components: &component.iter().map(|s| &**s).collect::<Vec<_>>(),
        targets: &target.iter().map(|s| &**s).collect::<Vec<_>>(),
    };

    self_update::install(current_dir, no_prompt, quiet, opts, process).await
}
