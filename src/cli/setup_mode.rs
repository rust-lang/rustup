use anyhow::Result;
use clap::{App, AppSettings, Arg};

use super::common;
use super::self_update::{self, InstallOpts};
use crate::dist::dist::Profile;
use crate::process;
use crate::utils::utils;

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

    // XXX: If you change anything here, please make the same changes in rustup-init.sh
    let cli = App::new("rustup-init")
        .version(common::version())
        .about("The installer for rustup")
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Enable verbose output"),
        )
        .arg(
            Arg::with_name("quiet")
                .conflicts_with("verbose")
                .short("q")
                .long("quiet")
                .help("Disable progress output"),
        )
        .arg(
            Arg::with_name("no-prompt")
                .short("y")
                .help("Disable confirmation prompt."),
        )
        .arg(
            Arg::with_name("default-host")
                .long("default-host")
                .takes_value(true)
                .help("Choose a default host triple"),
        )
        .arg(
            Arg::with_name("default-toolchain")
                .long("default-toolchain")
                .takes_value(true)
                .help("Choose a default toolchain to install"),
        )
        .arg(
            Arg::with_name("profile")
                .long("profile")
                .possible_values(Profile::names())
                .default_value(Profile::default_name()),
        )
        .arg(
            Arg::with_name("components")
                .help("Component name to also install")
                .long("component")
                .short("c")
                .takes_value(true)
                .multiple(true)
                .use_delimiter(true),
        )
        .arg(
            Arg::with_name("targets")
                .help("Target name to also install")
                .long("target")
                .short("target")
                .takes_value(true)
                .multiple(true)
                .use_delimiter(true),
        )
        .arg(
            Arg::with_name("no-update-default-toolchain")
                .long("no-update-default-toolchain")
                .help("Don't update any existing default toolchain after install"),
        )
        .arg(
            Arg::with_name("no-modify-path")
                .long("no-modify-path")
                .help("Don't configure the PATH environment variable"),
        );

    let matches = match cli.get_matches_from_safe(process().args_os()) {
        Ok(matches) => matches,
        Err(e)
            if e.kind == clap::ErrorKind::HelpDisplayed
                || e.kind == clap::ErrorKind::VersionDisplayed =>
        {
            writeln!(process().stdout(), "{}", e.message)?;
            return Ok(utils::ExitCode(0));
        }
        Err(e) => return Err(e.into()),
    };
    let no_prompt = matches.is_present("no-prompt");
    let verbose = matches.is_present("verbose");
    let quiet = matches.is_present("quiet");
    let default_host = matches.value_of("default-host").map(ToOwned::to_owned);
    let default_toolchain = matches.value_of("default-toolchain").map(ToOwned::to_owned);
    let profile = matches
        .value_of("profile")
        .expect("Unreachable: Clap should supply a default");
    let no_modify_path = matches.is_present("no-modify-path");
    let no_update_toolchain = matches.is_present("no-update-default-toolchain");

    let components: Vec<_> = matches
        .values_of("components")
        .map(|v| v.collect())
        .unwrap_or_else(Vec::new);

    let targets: Vec<_> = matches
        .values_of("targets")
        .map(|v| v.collect())
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
