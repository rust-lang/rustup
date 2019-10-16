use crate::common;
use crate::errors::*;
use crate::self_update::{self, InstallOpts};
use clap::{App, AppSettings, Arg};
use rustup::dist::dist::{Profile, TargetTriple};
use std::env;

pub fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    let arg1 = args.get(1).map(|a| &**a);

    // Secret command used during self-update. Not for users.
    if arg1 == Some("--self-replace") {
        return self_update::self_replace();
    }

    // Internal testament dump used during CI.  Not for users.
    if arg1 == Some("--dump-testament") {
        common::dump_testament();
        return Ok(());
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
                .multiple(true),
        )
        .arg(
            Arg::with_name("targets")
                .help("Target name to also install")
                .long("target")
                .short("target")
                .takes_value(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("no-modify-path")
                .long("no-modify-path")
                .help("Don't configure the PATH environment variable"),
        );

    let matches = cli.get_matches();
    let no_prompt = matches.is_present("no-prompt");
    let verbose = matches.is_present("verbose");
    let quiet = matches.is_present("quiet");
    let default_host = matches.value_of("default-host").map_or_else(
        || TargetTriple::from_host_or_build().to_string(),
        std::borrow::ToOwned::to_owned,
    );
    let default_toolchain = matches.value_of("default-toolchain").unwrap_or("stable");
    let profile = matches
        .value_of("profile")
        .expect("Unreachable: Clap should supply a default");
    let no_modify_path = matches.is_present("no-modify-path");

    let components: Vec<_> = matches
        .values_of("components")
        .map_or_else(Vec::new, Iterator::collect);

    let targets: Vec<_> = matches
        .values_of("targets")
        .map_or_else(Vec::new, Iterator::collect);

    let opts = InstallOpts {
        default_host_triple: default_host,
        default_toolchain: default_toolchain.to_owned(),
        profile: profile.to_owned(),
        no_modify_path,
        components: &components,
        targets: &targets,
    };

    self_update::install(no_prompt, verbose, quiet, opts)?;

    Ok(())
}
