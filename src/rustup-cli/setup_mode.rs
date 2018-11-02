use std::env;
use self_update::{self, InstallOpts};
use errors::*;
use clap::{App, AppSettings, Arg};
use rustup_dist::dist::TargetTriple;
use common;

pub fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    let arg1 = args.get(1).map(|a| &**a);

    // Secret command used during self-update. Not for users.
    if arg1 == Some("--self-replace") {
        return self_update::self_replace();
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
                .takes_value(true)
                .help("Choose a profile to install"),
        )
        .arg(
            Arg::with_name("no-modify-path")
                .long("no-modify-path")
                .help("Don't configure the PATH environment variable"),
        );

    let matches = cli.get_matches();
    let no_prompt = matches.is_present("no-prompt");
    let verbose = matches.is_present("verbose");
    let default_host = matches
        .value_of("default-host")
        .map(|s| s.to_owned())
        .unwrap_or_else(|| TargetTriple::from_host_or_build().to_string());
    let default_toolchain = matches.value_of("default-toolchain").unwrap_or("stable");
    let profile = matches.value_of("profile").unwrap_or("default");
    let no_modify_path = matches.is_present("no-modify-path");

    let opts = InstallOpts {
        default_host_triple: default_host,
        default_toolchain: default_toolchain.to_owned(),
        profile: profile.to_owned(),
        no_modify_path: no_modify_path,
    };

    self_update::install(no_prompt, verbose, opts)?;

    Ok(())
}
