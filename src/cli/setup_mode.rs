use anyhow::Result;
use clap::{builder::PossibleValuesParser, Parser};

use crate::{
    cli::{
        common,
        self_update::{self, InstallOpts},
    },
    currentprocess::filesource::StdoutSource,
    dist::dist::Profile,
    process,
    toolchain::names::MaybeOfficialToolchainName,
    utils::utils,
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
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Disable progress output
    #[arg(short, long)]
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

    #[arg(
        long,
        value_parser = PossibleValuesParser::new(Profile::names()),
        default_value = Profile::default_name(),
    )]
    profile: String,

    /// Component name to also install
    #[arg(short, long, value_delimiter = ',', num_args = 1..)]
    components: Vec<String>,

    /// Target name to also install
    #[arg(short, long, value_delimiter = ',', num_args = 1..)]
    targets: Vec<String>,

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

#[cfg_attr(feature = "otel", tracing::instrument)]
pub fn main() -> Result<utils::ExitCode> {
    use clap::error::ErrorKind;

    let RustupInit {
        verbose,
        quiet,
        no_prompt,
        default_host,
        default_toolchain,
        profile,
        components,
        targets,
        no_update_default_toolchain,
        no_modify_path,
        self_replace,
        dump_testament,
    } = match RustupInit::try_parse() {
        Ok(args) => args,
        Err(e) if [ErrorKind::DisplayHelp, ErrorKind::DisplayVersion].contains(&e.kind()) => {
            write!(process().stdout().lock(), "{e}")?;
            return Ok(utils::ExitCode(0));
        }
        Err(e) => return Err(e.into()),
    };

    if self_replace {
        return self_update::self_replace();
    }

    if dump_testament {
        common::dump_testament()?;
        return Ok(utils::ExitCode(0));
    }

    if &profile == "complete" {
        warn!("{}", common::WARN_COMPLETE_PROFILE);
    }

    let opts = InstallOpts {
        default_host_triple: default_host,
        default_toolchain,
        profile,
        no_modify_path,
        no_update_toolchain: no_update_default_toolchain,
        components: &components.iter().map(|s| &**s).collect::<Vec<_>>(),
        targets: &targets.iter().map(|s| &**s).collect::<Vec<_>>(),
    };

    self_update::install(no_prompt, verbose, quiet, opts)
}
