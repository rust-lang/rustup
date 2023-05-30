use anyhow::Result;
use clap::{builder::PossibleValuesParser, AppSettings, Arg, ArgAction, Command};

use crate::{
    cli::{
        common,
        self_update::{self, InstallOpts},
    },
    currentprocess::{argsource::ArgSource, filesource::StdoutSource},
    dist::dist::Profile,
    process,
    toolchain::names::{maybe_official_toolchainame_parser, MaybeOfficialToolchainName},
    utils::utils,
};

#[cfg_attr(feature = "otel", tracing::instrument)]
pub fn main() -> Result<utils::ExitCode> {
    let args: Vec<_> = process().args().collect();
    let arg1 = args.get(1).map(|a| &**a);

    // Secret command used during self-update. Not for users.
    if arg1 == Some("--self-replace") {
        return self_update::self_replace();
    }

    // Internal testament dump used during CI.  Not for users.
    if arg1 == Some("--dump-testament") {
        common::dump_testament()?;
        return Ok(utils::ExitCode(0));
    }

    // NOTICE: If you change anything here, please make the same changes in rustup-init.sh
    let cli = Command::new("rustup-init")
        .version(common::version())
        .about("The installer for rustup")
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .conflicts_with("verbose")
                .short('q')
                .long("quiet")
                .help("Disable progress output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no-prompt")
                .short('y')
                .help("Disable confirmation prompt.")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("default-host")
                .long("default-host")
                .takes_value(true)
                .help("Choose a default host triple"),
        )
        .arg(
            Arg::new("default-toolchain")
                .long("default-toolchain")
                .takes_value(true)
                .help("Choose a default toolchain to install. Use 'none' to not install any toolchains at all")
                .value_parser(maybe_official_toolchainame_parser)
        )
        .arg(
            Arg::new("profile")
                .long("profile")
                .value_parser(PossibleValuesParser::new(Profile::names()))
                .default_value(Profile::default_name()),
        )
        .arg(
            Arg::new("components")
                .help("Component name to also install")
                .long("component")
                .short('c')
                .takes_value(true)
                .multiple_values(true)
                .use_value_delimiter(true)
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("targets")
                .help("Target name to also install")
                .long("target")
                .short('t')
                .takes_value(true)
                .multiple_values(true)
                .use_value_delimiter(true)
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("no-update-default-toolchain")
                .long("no-update-default-toolchain")
                .help("Don't update any existing default toolchain after install")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no-modify-path")
                .long("no-modify-path")
                .help("Don't configure the PATH environment variable")
                .action(ArgAction::SetTrue),
        );

    let matches = match cli.try_get_matches_from(process().args_os()) {
        Ok(matches) => matches,
        Err(e)
            if e.kind() == clap::ErrorKind::DisplayHelp
                || e.kind() == clap::ErrorKind::DisplayVersion =>
        {
            write!(process().stdout().lock(), "{e}")?;
            return Ok(utils::ExitCode(0));
        }
        Err(e) => return Err(e.into()),
    };
    let no_prompt = matches.get_flag("no-prompt");
    let verbose = matches.get_flag("verbose");
    let quiet = matches.get_flag("quiet");
    let default_host = matches
        .get_one::<String>("default-host")
        .map(ToOwned::to_owned);
    let default_toolchain = matches
        .get_one::<MaybeOfficialToolchainName>("default-toolchain")
        .map(ToOwned::to_owned);
    let profile = matches
        .get_one::<String>("profile")
        .expect("Unreachable: Clap should supply a default");
    let no_modify_path = matches.get_flag("no-modify-path");
    let no_update_toolchain = matches.get_flag("no-update-default-toolchain");

    let components: Vec<_> = matches
        .get_many::<String>("components")
        .map(|v| v.map(|s| &**s).collect())
        .unwrap_or_else(Vec::new);

    let targets: Vec<_> = matches
        .get_many::<String>("targets")
        .map(|v| v.map(|s| &**s).collect())
        .unwrap_or_else(Vec::new);

    let opts = InstallOpts {
        default_host_triple: default_host,
        default_toolchain,
        profile: profile.to_owned(),
        no_modify_path,
        no_update_toolchain,
        components: &components,
        targets: &targets,
    };

    if profile == "complete" {
        warn!("{}", common::WARN_COMPLETE_PROFILE);
    }

    self_update::install(no_prompt, verbose, quiet, opts)
}
