use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

use anyhow::{anyhow, Error, Result};
use clap::{
    builder::{EnumValueParser, PossibleValuesParser},
    AppSettings, Arg, ArgAction, ArgEnum, ArgGroup, ArgMatches, Command, PossibleValue,
};
use clap_complete::Shell;

use crate::{
    cli::{
        common::{self, PackageUpdate},
        errors::CLIError,
        help::*,
        self_update::{self, check_rustup_update, SelfUpdateMode},
        topical_doc,
    },
    command,
    currentprocess::{
        argsource::ArgSource,
        filesource::{StderrSource, StdoutSource},
        varsource::VarSource,
    },
    dist::{
        dist::{PartialToolchainDesc, Profile, TargetTriple},
        manifest::{Component, ComponentStatus},
    },
    errors::RustupError,
    install::UpdateStatus,
    process,
    terminalsource::{self, ColorableTerminal},
    toolchain::{
        distributable::DistributableToolchain,
        names::{
            custom_toolchain_name_parser, maybe_resolvable_toolchainame_parser,
            partial_toolchain_desc_parser, resolvable_local_toolchainame_parser,
            resolvable_toolchainame_parser, CustomToolchainName, MaybeResolvableToolchainName,
            ResolvableLocalToolchainName, ResolvableToolchainName, ToolchainName,
        },
        toolchain::Toolchain,
    },
    utils::utils,
    Cfg, Notification,
};

const TOOLCHAIN_OVERRIDE_ERROR: &str =
    "To override the toolchain using the 'rustup +toolchain' syntax, \
                        make sure to prefix the toolchain override with a '+'";

fn handle_epipe(res: Result<utils::ExitCode>) -> Result<utils::ExitCode> {
    match res {
        Err(e) => {
            let root = e.root_cause();
            if let Some(io_err) = root.downcast_ref::<std::io::Error>() {
                if io_err.kind() == std::io::ErrorKind::BrokenPipe {
                    return Ok(utils::ExitCode(0));
                }
            }
            Err(e)
        }
        res => res,
    }
}

fn deprecated<F, B, R>(instead: &str, cfg: &mut Cfg, matches: B, callee: F) -> R
where
    F: FnOnce(&mut Cfg, B) -> R,
{
    (cfg.notify_handler)(Notification::PlainVerboseMessage(
        "Use of (currently) unmaintained command line interface.",
    ));
    (cfg.notify_handler)(Notification::PlainVerboseMessage(
        "The exact API of this command may change without warning",
    ));
    (cfg.notify_handler)(Notification::PlainVerboseMessage(
        "Eventually this command will be a true alias.  Until then:",
    ));
    (cfg.notify_handler)(Notification::PlainVerboseMessage(&format!(
        "  Please use `rustup {instead}` instead"
    )));
    callee(cfg, matches)
}

#[cfg_attr(feature = "otel", tracing::instrument(fields(args = format!("{:?}", process().args_os().collect::<Vec<_>>()))))]
pub fn main() -> Result<utils::ExitCode> {
    self_update::cleanup_self_updater()?;

    use clap::ErrorKind::*;
    let matches = match cli().try_get_matches_from(process().args_os()) {
        Ok(matches) => Ok(matches),
        Err(err) if err.kind() == DisplayHelp => {
            write!(process().stdout().lock(), "{err}")?;
            return Ok(utils::ExitCode(0));
        }
        Err(err) if err.kind() == DisplayVersion => {
            write!(process().stdout().lock(), "{err}")?;
            info!("This is the version for the rustup toolchain manager, not the rustc compiler.");

            #[cfg_attr(feature = "otel", tracing::instrument)]
            fn rustc_version() -> std::result::Result<String, Box<dyn std::error::Error>> {
                let cfg = &mut common::set_globals(false, true)?;
                let cwd = std::env::current_dir()?;

                if let Some(t) = process().args().find(|x| x.starts_with('+')) {
                    debug!("Fetching rustc version from toolchain `{}`", t);
                    cfg.set_toolchain_override(&ResolvableToolchainName::try_from(&t[1..])?);
                }

                let toolchain = cfg.find_or_install_override_toolchain_or_default(&cwd)?.0;

                Ok(toolchain.rustc_version())
            }

            match rustc_version() {
                Ok(version) => info!("The currently active `rustc` version is `{}`", version),
                Err(err) => debug!("Wanted to tell you the current rustc version, too, but ran into this error: {}", err),
            }
            return Ok(utils::ExitCode(0));
        }

        Err(err) => {
            if [
                InvalidSubcommand,
                UnknownArgument,
                DisplayHelpOnMissingArgumentOrSubcommand,
            ]
            .contains(&err.kind())
            {
                write!(process().stdout().lock(), "{err}")?;
                return Ok(utils::ExitCode(1));
            }
            if err.kind() == ValueValidation && err.to_string().contains(TOOLCHAIN_OVERRIDE_ERROR) {
                write!(process().stderr().lock(), "{err}")?;
                return Ok(utils::ExitCode(1));
            }
            Err(err)
        }
    }?;
    let verbose = matches.get_flag("verbose");
    let quiet = matches.get_flag("quiet");
    let cfg = &mut common::set_globals(verbose, quiet)?;

    if let Some(t) = matches.get_one::<ResolvableToolchainName>("+toolchain") {
        cfg.set_toolchain_override(t);
    }

    if maybe_upgrade_data(cfg, &matches)? {
        return Ok(utils::ExitCode(0));
    }

    cfg.check_metadata_version()?;

    Ok(match matches.subcommand() {
        Some(s) => match s {
            ("dump-testament", _) => common::dump_testament()?,
            ("show", c) => match c.subcommand() {
                Some(s) => match s {
                    ("active-toolchain", m) => handle_epipe(show_active_toolchain(cfg, m))?,
                    ("home", _) => handle_epipe(show_rustup_home(cfg))?,
                    ("profile", _) => handle_epipe(show_profile(cfg))?,
                    _ => handle_epipe(show(cfg, c))?,
                },
                None => handle_epipe(show(cfg, c))?,
            },
            ("install", m) => deprecated("toolchain install", cfg, m, update)?,
            ("update", m) => update(cfg, m)?,
            ("check", _) => check_updates(cfg)?,
            ("uninstall", m) => deprecated("toolchain uninstall", cfg, m, toolchain_remove)?,
            ("default", m) => default_(cfg, m)?,
            ("toolchain", c) => match c.subcommand() {
                Some(s) => match s {
                    ("install", m) => update(cfg, m)?,
                    ("list", m) => handle_epipe(toolchain_list(cfg, m))?,
                    ("link", m) => toolchain_link(cfg, m)?,
                    ("uninstall", m) => toolchain_remove(cfg, m)?,
                    _ => unreachable!(),
                },
                None => unreachable!(),
            },
            ("target", c) => match c.subcommand() {
                Some(s) => match s {
                    ("list", m) => handle_epipe(target_list(cfg, m))?,
                    ("add", m) => target_add(cfg, m)?,
                    ("remove", m) => target_remove(cfg, m)?,
                    _ => unreachable!(),
                },
                None => unreachable!(),
            },
            ("component", c) => match c.subcommand() {
                Some(s) => match s {
                    ("list", m) => handle_epipe(component_list(cfg, m))?,
                    ("add", m) => component_add(cfg, m)?,
                    ("remove", m) => component_remove(cfg, m)?,
                    _ => unreachable!(),
                },
                None => unreachable!(),
            },
            ("override", c) => match c.subcommand() {
                Some(s) => match s {
                    ("list", _) => handle_epipe(common::list_overrides(cfg))?,
                    ("set", m) => override_add(cfg, m)?,
                    ("unset", m) => override_remove(cfg, m)?,
                    _ => unreachable!(),
                },
                None => unreachable!(),
            },
            ("run", m) => run(cfg, m)?,
            ("which", m) => which(cfg, m)?,
            ("doc", m) => doc(cfg, m)?,
            ("man", m) => man(cfg, m)?,
            ("self", c) => match c.subcommand() {
                Some(s) => match s {
                    ("update", _) => self_update::update(cfg)?,
                    ("uninstall", m) => self_uninstall(m)?,
                    _ => unreachable!(),
                },
                None => unreachable!(),
            },
            ("set", c) => match c.subcommand() {
                Some(s) => match s {
                    ("default-host", m) => set_default_host_triple(cfg, m)?,
                    ("profile", m) => set_profile(cfg, m)?,
                    ("auto-self-update", m) => set_auto_self_update(cfg, m)?,
                    _ => unreachable!(),
                },
                None => unreachable!(),
            },
            ("completions", c) => {
                if let Some(&shell) = c.get_one::<Shell>("shell") {
                    output_completion_script(
                        shell,
                        c.get_one::<CompletionCommand>("command")
                            .copied()
                            .unwrap_or(CompletionCommand::Rustup),
                    )?
                } else {
                    unreachable!()
                }
            }
            _ => unreachable!(),
        },
        None => unreachable!(),
    })
}

pub(crate) fn cli() -> Command<'static> {
    let mut app = Command::new("rustup")
        .version(common::version())
        .about("The Rust toolchain installer")
        .after_help(RUSTUP_HELP)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            verbose_arg("Enable verbose output"),
        )
        .arg(
            Arg::new("quiet")
                .conflicts_with("verbose")
                .help("Disable progress output")
                .short('q')
                .long("quiet")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("+toolchain")
                .help("release channel (e.g. +stable) or custom toolchain to set override")
                .value_parser(|s: &str| {
                    if let Some(stripped) = s.strip_prefix('+') {
                        ResolvableToolchainName::try_from(stripped).map_err(|e| clap::Error::raw(clap::ErrorKind::InvalidValue, e))
                    } else {
                        Err(clap::Error::raw(clap::ErrorKind::InvalidSubcommand, format!("\"{s}\" is not a valid subcommand, so it was interpreted as a toolchain name, but it is also invalid. {TOOLCHAIN_OVERRIDE_ERROR}")))
                    }
                }),
        )
        .subcommand(
            Command::new("dump-testament")
                .about("Dump information about the build")
                .hide(true), // Not for users, only CI
        )
        .subcommand(
            Command::new("show")
                .about("Show the active and installed toolchains or profiles")
                .after_help(SHOW_HELP)
                .arg(
                    verbose_arg("Enable verbose output with rustc information for all installed toolchains"),
                )
                .subcommand(
                    Command::new("active-toolchain")
                        .about("Show the active toolchain")
                        .after_help(SHOW_ACTIVE_TOOLCHAIN_HELP)
                        .arg(
                            verbose_arg("Enable verbose output with rustc information"),
                        ),
                )
                .subcommand(
                    Command::new("home")
                        .about("Display the computed value of RUSTUP_HOME"),
                )
                .subcommand(Command::new("profile").about("Show the current profile"))
        )
        .subcommand(
            Command::new("install")
                .about("Update Rust toolchains")
                .after_help(INSTALL_HELP)
                .hide(true) // synonym for 'toolchain install'
                .arg(
                    Arg::new("toolchain")
                        .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                        .required(true)
                        .value_parser(partial_toolchain_desc_parser)
                        .takes_value(true)
                        .multiple_values(true)
                )
                .arg(
                    Arg::new("profile")
                        .long("profile")
                        .value_parser(PossibleValuesParser::new(Profile::names()))
                        .takes_value(true),
                )
                .arg(
                    Arg::new("no-self-update")
                        .help("Don't perform self-update when running the `rustup install` command")
                        .long("no-self-update")
                        .action(ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("force")
                        .help("Force an update, even if some components are missing")
                        .long("force")
                        .action(ArgAction::SetTrue)
                ).arg(
                    Arg::new("force-non-host")
                        .help("Install toolchains that require an emulator. See https://github.com/rust-lang/rustup/wiki/Non-host-toolchains")
                        .long("force-non-host")
                        .action(ArgAction::SetTrue)
                ),
        )
        .subcommand(
            Command::new("uninstall")
                .about("Uninstall Rust toolchains")
                .hide(true) // synonym for 'toolchain uninstall'
                .arg(
                    Arg::new("toolchain")
                        .help(RESOLVABLE_TOOLCHAIN_ARG_HELP)
                        .required(true)
                        .value_parser(resolvable_toolchainame_parser)
                        .takes_value(true)
                        .multiple_values(true),
                ),
        )
        .subcommand(
            Command::new("update")
                .about("Update Rust toolchains and rustup")
                .aliases(&["upgrade", "up"])
                .after_help(UPDATE_HELP)
                .arg(
                    Arg::new("toolchain")
                        .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                        .required(false)
                        .value_parser(partial_toolchain_desc_parser)
                        .takes_value(true)
                        .multiple_values(true),
                )
                .arg(
                    Arg::new("no-self-update")
                        .help("Don't perform self update when running the `rustup update` command")
                        .long("no-self-update")
                        .action(ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("force")
                        .help("Force an update, even if some components are missing")
                        .long("force")
                        .action(ArgAction::SetTrue)
                )
                .arg(
                    Arg::new("force-non-host")
                        .help("Install toolchains that require an emulator. See https://github.com/rust-lang/rustup/wiki/Non-host-toolchains")
                        .long("force-non-host")
                        .action(ArgAction::SetTrue)
                ),
        )
        .subcommand(Command::new("check").about("Check for updates to Rust toolchains and rustup"))
        .subcommand(
            Command::new("default")
                .about("Set the default toolchain")
                .after_help(DEFAULT_HELP)
                .arg(
                    Arg::new("toolchain")
                        .help(MAYBE_RESOLVABLE_TOOLCHAIN_ARG_HELP)
                        .required(false)
                        .value_parser(maybe_resolvable_toolchainame_parser)
                ),
        )
        .subcommand(
            Command::new("toolchain")
                .about("Modify or query the installed toolchains")
                .after_help(TOOLCHAIN_HELP)
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    Command::new("list")
                        .about("List installed toolchains")
                        .arg(
                            verbose_arg("Enable verbose output with toolchain information"),
                        ),
                )
                .subcommand(
                    Command::new("install")
                        .about("Install or update a given toolchain")
                        .aliases(&["update", "add"])
                        .arg(
                            Arg::new("toolchain")
                                .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                                .required(true)
                                .value_parser( partial_toolchain_desc_parser)
                                .takes_value(true)
                                .multiple_values(true),
                        )
                        .arg(
                            Arg::new("profile")
                                .long("profile")
                                .value_parser(PossibleValuesParser::new(Profile::names()))
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("components")
                                .help("Add specific components on installation")
                                .long("component")
                                .short('c')
                                .takes_value(true)
                                .multiple_values(true)
                                .use_value_delimiter(true)
                            .action(ArgAction::Append),
                        )
                        .arg(
                            Arg::new("targets")
                                .help("Add specific targets on installation")
                                .long("target")
                                .short('t')
                                .takes_value(true)
                                .multiple_values(true)
                                .use_value_delimiter(true)
                                .action(ArgAction::Append),
                        )
                        .arg(
                            Arg::new("no-self-update")
                                .help(
                                    "Don't perform self update when running the\
                                     `rustup toolchain install` command",
                                )
                                .long("no-self-update")
                                .takes_value(true)
                                .action(ArgAction::SetTrue)
                        )
                        .arg(
                            Arg::new("force")
                                .help("Force an update, even if some components are missing")
                                .long("force")
                                .action(ArgAction::SetTrue)
                        )
                        .arg(
                            Arg::new("allow-downgrade")
                                .help("Allow rustup to downgrade the toolchain to satisfy your component choice")
                                .long("allow-downgrade")
                                .action(ArgAction::SetTrue)
                        )
                        .arg(
                            Arg::new("force-non-host")
                                .help("Install toolchains that require an emulator. See https://github.com/rust-lang/rustup/wiki/Non-host-toolchains")
                                .long("force-non-host")
                                .action(ArgAction::SetTrue)
                        ),
                )
                .subcommand(
                    Command::new("uninstall")
                        .about("Uninstall a toolchain")
                        .alias("remove")
                        .arg(
                            Arg::new("toolchain")
                                .help(RESOLVABLE_TOOLCHAIN_ARG_HELP)
                                .required(true)
                                .value_parser(resolvable_toolchainame_parser)
                                .takes_value(true)
                                .multiple_values(true),
                        ),
                )
                .subcommand(
                    Command::new("link")
                        .about("Create a custom toolchain by symlinking to a directory")
                        .after_help(TOOLCHAIN_LINK_HELP)
                        .arg(
                            Arg::new("toolchain")
                                .help("Custom toolchain name")
                                .required(true)
                                .value_parser(custom_toolchain_name_parser),
                        )
                        .arg(
                            Arg::new("path")
                                .help("Path to the directory")
                                .required(true),
                        ),
                ),
        )
        .subcommand(
            Command::new("target")
                .about("Modify a toolchain's supported targets")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    Command::new("list")
                        .about("List installed and available targets")
                        .arg(
                            Arg::new("toolchain")
                                .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .value_parser(partial_toolchain_desc_parser)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("installed")
                                .long("installed")
                                .help("List only installed targets")
                                .action(ArgAction::SetTrue)
                        ),
                )
                .subcommand(
                    Command::new("add")
                        .about("Add a target to a Rust toolchain")
                        .alias("install")
                        .arg(
                            Arg::new("target")
                            .required(true)
                            .takes_value(true)
                            .multiple_values(true)
                            .help(
                                "List of targets to install; \
                                \"all\" installs all available targets"
                            )
                        )
                        .arg(
                            Arg::new("toolchain")
                                .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true)
                                .value_parser(partial_toolchain_desc_parser),
                        ),
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove a target from a Rust toolchain")
                        .alias("uninstall")
                        .arg(
                            Arg::new("target")
                            .help("List of targets to uninstall")
                            .required(true)
                            .takes_value(true)
                            .multiple_values(true)
                        )
                        .arg(
                            Arg::new("toolchain")
                                .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true)
                                .value_parser(partial_toolchain_desc_parser),
                        ),
                ),
        )
        .subcommand(
            Command::new("component")
                .about("Modify a toolchain's installed components")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    Command::new("list")
                        .about("List installed and available components")
                        .arg(
                            Arg::new("toolchain")
                                .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true)
                                .value_parser(partial_toolchain_desc_parser),
                        )
                        .arg(
                            Arg::new("installed")
                                .long("installed")
                                .help("List only installed components")
                                .action(ArgAction::SetTrue)
                        ),
                )
                .subcommand(
                    Command::new("add")
                        .about("Add a component to a Rust toolchain")
                        .arg(Arg::new("component").required(true)
                        .takes_value(true).multiple_values(true))
                        .arg(
                            Arg::new("toolchain")
                                .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true)
                                .value_parser( partial_toolchain_desc_parser),
                        )
                        .arg(
                            Arg::new("target")
                            .long("target")
                            .takes_value(true)
                        ),
                )
                .subcommand(
                    Command::new("remove")
                        .about("Remove a component from a Rust toolchain")
                        .arg(Arg::new("component").required(true)
                        .takes_value(true).multiple_values(true))
                        .arg(
                            Arg::new("toolchain")
                                .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true)
                                .value_parser( partial_toolchain_desc_parser),
                        )
                        .arg(
                            Arg::new("target")
                            .long("target")
                            .takes_value(true)
                        ),
                ),
        )
        .subcommand(
            Command::new("override")
                .about("Modify directory toolchain overrides")
                .after_help(OVERRIDE_HELP)
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    Command::new("list").about("List directory toolchain overrides"),
                )
                .subcommand(
                    Command::new("set")
                        .about("Set the override toolchain for a directory")
                        .alias("add")
                        .arg(
                            Arg::new("toolchain")
                                .help(RESOLVABLE_TOOLCHAIN_ARG_HELP)
                                .required(true)
                                .takes_value(true)
                                .value_parser(resolvable_toolchainame_parser),
                        )
                        .arg(
                            Arg::new("path")
                                .long("path")
                                .takes_value(true)
                                .help("Path to the directory"),
                        ),
                )
                .subcommand(
                    Command::new("unset")
                        .about("Remove the override toolchain for a directory")
                        .after_help(OVERRIDE_UNSET_HELP)
                        .alias("remove")
                        .arg(
                            Arg::new("path")
                                .long("path")
                                .takes_value(true)
                                .help("Path to the directory"),
                        )
                        .arg(
                            Arg::new("nonexistent")
                                .long("nonexistent")
                                .help("Remove override toolchain for all nonexistent directories")
                                .action(ArgAction::SetTrue),
                        ),
                ),
        )
        .subcommand(
            Command::new("run")
                .about("Run a command with an environment configured for a given toolchain")
                .after_help(RUN_HELP)
                .trailing_var_arg(true)
                .arg(
                    Arg::new("toolchain")
                        .help(RESOLVABLE_LOCAL_TOOLCHAIN_ARG_HELP)
                        .required(true)
                        .takes_value(true)
                        .value_parser(resolvable_local_toolchainame_parser),
                )
                .arg(
                    Arg::new("command")
                        .required(true)
                        .takes_value(true)
                        .multiple_values(true)
                        .use_value_delimiter(false),
                )
                .arg(
                    Arg::new("install")
                        .help("Install the requested toolchain if needed")
                        .long("install")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("which")
                .about("Display which binary will be run for a given command")
                .arg(Arg::new("command").required(true))
                .arg(
                    Arg::new("toolchain")
                        .help(RESOLVABLE_TOOLCHAIN_ARG_HELP)
                        .long("toolchain")
                        .takes_value(true)
                        .value_parser(resolvable_toolchainame_parser),
                ),
        )
        .subcommand(
            Command::new("doc")
                .alias("docs")
                .about("Open the documentation for the current toolchain")
                .after_help(DOC_HELP)
                .arg(
                    Arg::new("path")
                        .long("path")
                        .help("Only print the path to the documentation")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("toolchain")
                        .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                        .long("toolchain")
                        .takes_value(true)
                        .value_parser(partial_toolchain_desc_parser),
                )
                .arg(Arg::new("topic").help(TOPIC_ARG_HELP))
                .group(
                    ArgGroup::new("page").args(
                        &DOCS_DATA
                            .iter()
                            .map(|(name, _, _)| *name)
                            .collect::<Vec<_>>(),
                    ),
                )
                .args(
                    &DOCS_DATA
                        .iter()
                        .map(|&(name, help_msg, _)| Arg::new(name).long(name).help(help_msg).action(ArgAction::SetTrue))
                        .collect::<Vec<_>>(),
                ),
        );

    if cfg!(not(target_os = "windows")) {
        app = app.subcommand(
            Command::new("man")
                .about("View the man page for a given command")
                .arg(Arg::new("command").required(true))
                .arg(
                    Arg::new("toolchain")
                        .help(OFFICIAL_TOOLCHAIN_ARG_HELP)
                        .long("toolchain")
                        .takes_value(true)
                        .value_parser(partial_toolchain_desc_parser),
                ),
        );
    }

    app = app
        .subcommand(
            Command::new("self")
                .about("Modify the rustup installation")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(Command::new("update").about("Download and install updates to rustup"))
                .subcommand(
                    Command::new("uninstall")
                        .about("Uninstall rustup.")
                        .arg(Arg::new("no-prompt").short('y').action(ArgAction::SetTrue)),
                )
                .subcommand(
                    Command::new("upgrade-data").about("Upgrade the internal data format."),
                ),
        )
        .subcommand(
            Command::new("set")
                .about("Alter rustup settings")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    Command::new("default-host")
                        .about("The triple used to identify toolchains when not specified")
                        .arg(Arg::new("host_triple").required(true)),
                )
                .subcommand(
                    Command::new("profile")
                        .about("The default components installed")
                        .arg(
                            Arg::new("profile-name")
                                .required(true)
                                .value_parser(PossibleValuesParser::new(Profile::names()))
                                .default_value(Profile::default_name()),
                        ),
                )
                .subcommand(
                    Command::new("auto-self-update")
                        .about("The rustup auto self update mode")
                        .arg(
                            Arg::new("auto-self-update-mode")
                                .required(true)
                                .value_parser(PossibleValuesParser::new(SelfUpdateMode::modes()))
                                .default_value(SelfUpdateMode::default_mode()),
                        ),
                ),
        );

    app.subcommand(
        Command::new("completions")
            .about("Generate tab-completion scripts for your shell")
            .after_help(COMPLETIONS_HELP)
            .arg_required_else_help(true)
            .arg(Arg::new("shell").value_parser(EnumValueParser::<Shell>::new()))
            .arg(
                Arg::new("command")
                    .value_parser(EnumValueParser::<CompletionCommand>::new())
                    .default_missing_value("rustup"),
            ),
    )
}

fn verbose_arg(help: &str) -> Arg<'_> {
    Arg::new("verbose")
        .help(help)
        .short('v')
        .long("verbose")
        .action(ArgAction::SetTrue)
}

fn maybe_upgrade_data(cfg: &Cfg, m: &ArgMatches) -> Result<bool> {
    match m.subcommand() {
        Some(("self", c)) => match c.subcommand() {
            Some(("upgrade-data", _)) => {
                cfg.upgrade_data()?;
                Ok(true)
            }
            _ => Ok(false),
        },
        _ => Ok(false),
    }
}

fn default_(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    if let Some(toolchain) = m.get_one::<MaybeResolvableToolchainName>("toolchain") {
        match toolchain.to_owned() {
            MaybeResolvableToolchainName::None => {
                cfg.set_default(None)?;
            }
            MaybeResolvableToolchainName::Some(ResolvableToolchainName::Custom(toolchain_name)) => {
                Toolchain::new(cfg, (&toolchain_name).into())?;
                cfg.set_default(Some(&toolchain_name.into()))?;
            }
            MaybeResolvableToolchainName::Some(ResolvableToolchainName::Official(toolchain)) => {
                let desc = toolchain.resolve(&cfg.get_default_host_triple()?)?;
                let status = DistributableToolchain::install_if_not_installed(cfg, &desc)?;

                cfg.set_default(Some(&(&desc).into()))?;

                writeln!(process().stdout().lock())?;

                common::show_channel_update(cfg, PackageUpdate::Toolchain(desc), Ok(status))?;
            }
        };

        let cwd = utils::current_dir()?;
        if let Some((toolchain, reason)) = cfg.find_override(&cwd)? {
            info!("note that the toolchain '{toolchain}' is currently in use ({reason})");
        }
    } else {
        let default_toolchain = cfg
            .get_default()?
            .ok_or_else(|| anyhow!("no default toolchain configured"))?;
        writeln!(process().stdout().lock(), "{default_toolchain} (default)")?;
    }

    Ok(utils::ExitCode(0))
}

fn check_updates(cfg: &Cfg) -> Result<utils::ExitCode> {
    let mut t = process().stdout().terminal();
    let channels = cfg.list_channels()?;

    for channel in channels {
        let (name, distributable) = channel;
        let current_version = distributable.show_version()?;
        let dist_version = distributable.show_dist_version()?;
        let _ = t.attr(terminalsource::Attr::Bold);
        write!(t.lock(), "{name} - ")?;
        match (current_version, dist_version) {
            (None, None) => {
                let _ = t.fg(terminalsource::Color::Red);
                writeln!(t.lock(), "Cannot identify installed or update versions")?;
            }
            (Some(cv), None) => {
                let _ = t.fg(terminalsource::Color::Green);
                write!(t.lock(), "Up to date")?;
                let _ = t.reset();
                writeln!(t.lock(), " : {cv}")?;
            }
            (Some(cv), Some(dv)) => {
                let _ = t.fg(terminalsource::Color::Yellow);
                write!(t.lock(), "Update available")?;
                let _ = t.reset();
                writeln!(t.lock(), " : {cv} -> {dv}")?;
            }
            (None, Some(dv)) => {
                let _ = t.fg(terminalsource::Color::Yellow);
                write!(t.lock(), "Update available")?;
                let _ = t.reset();
                writeln!(t.lock(), " : (Unknown version) -> {dv}")?;
            }
        }
    }

    check_rustup_update()?;

    Ok(utils::ExitCode(0))
}

fn update(cfg: &mut Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let self_update_mode = cfg.get_self_update_mode()?;
    // Priority: no-self-update feature > self_update_mode > no-self-update args.
    // Update only if rustup does **not** have the no-self-update feature,
    // and auto-self-update is configured to **enable**
    // and has **no** no-self-update parameter.
    let self_update = !self_update::NEVER_SELF_UPDATE
        && self_update_mode == SelfUpdateMode::Enable
        && !m.get_flag("no-self-update");
    let forced = m.get_flag("force-non-host");
    if let Ok(Some(p)) = m.try_get_one::<String>("profile") {
        let p = Profile::from_str(p)?;
        cfg.set_profile_override(p);
    }
    let cfg = &cfg;
    if cfg.get_profile()? == Profile::Complete {
        warn!("{}", common::WARN_COMPLETE_PROFILE);
    }
    if let Ok(Some(names)) = m.try_get_many::<PartialToolchainDesc>("toolchain") {
        for name in names.map(|n| n.to_owned()) {
            // This needs another pass to fix it all up
            if name.has_triple() {
                let host_arch = TargetTriple::from_host_or_build();

                let target_triple = name.clone().resolve(&host_arch)?.target;
                if !forced && !host_arch.can_run(&target_triple)? {
                    err!("DEPRECATED: future versions of rustup will require --force-non-host to install a non-host toolchain.");
                    warn!("toolchain '{name}' may not be able to run on this system.");
                    warn!(
                            "If you meant to build software to target that platform, perhaps try `rustup target add {}` instead?",
                            target_triple.to_string()
                        );
                }
            }
            let desc = name.resolve(&cfg.get_default_host_triple()?)?;

            let components: Vec<_> = m
                .try_get_many::<String>("components")
                .ok()
                .flatten()
                .map_or_else(Vec::new, |v| v.map(|s| &**s).collect());
            let targets: Vec<_> = m
                .try_get_many::<String>("targets")
                .ok()
                .flatten()
                .map_or_else(Vec::new, |v| v.map(|s| &**s).collect());

            let force = m.get_flag("force");
            let allow_downgrade =
                matches!(m.try_get_one::<bool>("allow-downgrade"), Ok(Some(true)));
            let profile = cfg.get_profile()?;
            let status = match crate::toolchain::distributable::DistributableToolchain::new(
                cfg,
                desc.clone(),
            ) {
                Ok(mut d) => {
                    d.update_extra(&components, &targets, profile, force, allow_downgrade)?
                }
                Err(RustupError::ToolchainNotInstalled(_)) => {
                    crate::toolchain::distributable::DistributableToolchain::install(
                        cfg,
                        &desc,
                        &components,
                        &targets,
                        profile,
                        force,
                    )?
                    .0
                }
                Err(e) => Err(e)?,
            };

            writeln!(process().stdout().lock())?;
            common::show_channel_update(
                cfg,
                PackageUpdate::Toolchain(desc.clone()),
                Ok(status.clone()),
            )?;
            if cfg.get_default()?.is_none() && matches!(status, UpdateStatus::Installed) {
                cfg.set_default(Some(&desc.into()))?;
            }
        }
        if self_update {
            common::self_update(|| Ok(utils::ExitCode(0)))?;
        }
    } else {
        common::update_all_channels(cfg, self_update, m.get_flag("force"))?;
        info!("cleaning up downloads & tmp directories");
        utils::delete_dir_contents(&cfg.download_dir);
        cfg.temp_cfg.clean();
    }

    if !self_update::NEVER_SELF_UPDATE && self_update_mode == SelfUpdateMode::CheckOnly {
        check_rustup_update()?;
    }

    if self_update::NEVER_SELF_UPDATE {
        info!("self-update is disabled for this build of rustup");
        info!("any updates to rustup will need to be fetched with your system package manager")
    }

    Ok(utils::ExitCode(0))
}

fn run(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain = m
        .get_one::<ResolvableLocalToolchainName>("toolchain")
        .unwrap();
    let args = m.get_many::<String>("command").unwrap();
    let args: Vec<_> = args.collect();
    let toolchain = toolchain.resolve(&cfg.get_default_host_triple()?)?;
    let cmd = cfg.create_command_for_toolchain(&toolchain, m.get_flag("install"), args[0])?;

    let code = command::run_command_for_dir(cmd, args[0], &args[1..])?;
    Ok(code)
}

fn which(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let binary = m.get_one::<String>("command").unwrap();
    let binary_path = if let Some(toolchain) = m.get_one::<ResolvableToolchainName>("toolchain") {
        let desc = toolchain.resolve(&cfg.get_default_host_triple()?)?;
        Toolchain::new(cfg, desc.into())?.binary_file(binary)
    } else {
        cfg.which_binary(&utils::current_dir()?, binary)?
    };

    utils::assert_is_file(&binary_path)?;

    writeln!(process().stdout().lock(), "{}", binary_path.display())?;
    Ok(utils::ExitCode(0))
}

#[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
fn show(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let verbose = m.get_flag("verbose");

    // Print host triple
    {
        let mut t = process().stdout().terminal();
        t.attr(terminalsource::Attr::Bold)?;
        write!(t.lock(), "Default host: ")?;
        t.reset()?;
        writeln!(t.lock(), "{}", cfg.get_default_host_triple()?)?;
    }

    // Print rustup home directory
    {
        let mut t = process().stdout().terminal();
        t.attr(terminalsource::Attr::Bold)?;
        write!(t.lock(), "rustup home:  ")?;
        t.reset()?;
        writeln!(t.lock(), "{}", cfg.rustup_dir.display())?;
        writeln!(t.lock())?;
    }

    let cwd = utils::current_dir()?;
    let installed_toolchains = cfg.list_toolchains()?;
    // XXX: we may want a find_without_install capability for show.
    let active_toolchain = cfg.find_or_install_override_toolchain_or_default(&cwd);

    // active_toolchain will carry the reason we don't have one in its detail.
    let active_targets = if let Ok(ref at) = active_toolchain {
        if let Ok(distributable) = DistributableToolchain::try_from(&at.0) {
            let components = (|| {
                let manifestation = distributable.get_manifestation()?;
                let config = manifestation.read_config()?.unwrap_or_default();
                let manifest = distributable.get_manifest()?;
                manifest.query_components(distributable.desc(), &config)
            })();

            match components {
                Ok(cs_vec) => cs_vec
                    .into_iter()
                    .filter(|c| c.component.short_name_in_manifest() == "rust-std")
                    .filter(|c| c.installed)
                    .collect(),
                Err(_) => vec![],
            }
        } else {
            // These three vec![] could perhaps be reduced with and_then on active_toolchain.
            vec![]
        }
    } else {
        vec![]
    };

    let show_installed_toolchains = installed_toolchains.len() > 1;
    let show_active_targets = active_targets.len() > 1;
    let show_active_toolchain = true;

    // Only need to display headers if we have multiple sections
    let show_headers = [
        show_installed_toolchains,
        show_active_targets,
        show_active_toolchain,
    ]
    .iter()
    .filter(|x| **x)
    .count()
        > 1;

    if show_installed_toolchains {
        let mut t = process().stdout().terminal();

        if show_headers {
            print_header::<Error>(&mut t, "installed toolchains")?;
        }
        let default_name = cfg
            .get_default()?
            .ok_or_else(|| anyhow!("no default toolchain configured"))?;
        for it in installed_toolchains {
            if default_name == it {
                writeln!(t.lock(), "{it} (default)")?;
            } else {
                writeln!(t.lock(), "{it}")?;
            }
            if verbose {
                let toolchain = Toolchain::new(cfg, it.into())?;
                writeln!(process().stdout().lock(), "{}", toolchain.rustc_version())?;
                // To make it easy to see what rustc that belongs to what
                // toolchain we separate each pair with an extra newline
                writeln!(process().stdout().lock())?;
            }
        }
        if show_headers {
            writeln!(t.lock())?
        };
    }

    if show_active_targets {
        let mut t = process().stdout().terminal();

        if show_headers {
            print_header::<Error>(&mut t, "installed targets for active toolchain")?;
        }
        for at in active_targets {
            writeln!(
                t.lock(),
                "{}",
                at.component
                    .target
                    .as_ref()
                    .expect("rust-std should have a target")
            )?;
        }
        if show_headers {
            writeln!(t.lock())?;
        };
    }

    if show_active_toolchain {
        let mut t = process().stdout().terminal();

        if show_headers {
            print_header::<Error>(&mut t, "active toolchain")?;
        }

        match active_toolchain {
            Ok(atc) => match atc {
                (ref toolchain, Some(ref reason)) => {
                    writeln!(t.lock(), "{} ({})", toolchain.name(), reason)?;
                    writeln!(t.lock(), "{}", toolchain.rustc_version())?;
                }
                (ref toolchain, None) => {
                    writeln!(t.lock(), "{} (default)", toolchain.name())?;
                    writeln!(t.lock(), "{}", toolchain.rustc_version())?;
                }
            },
            Err(err) => {
                let root_cause = err.root_cause();
                if let Some(RustupError::ToolchainNotSelected) =
                    root_cause.downcast_ref::<RustupError>()
                {
                    writeln!(t.lock(), "no active toolchain")?;
                } else if let Some(cause) = err.source() {
                    writeln!(t.lock(), "(error: {err}, {cause})")?;
                } else {
                    writeln!(t.lock(), "(error: {err})")?;
                }
            }
        }

        if show_headers {
            writeln!(t.lock())?
        }
    }

    fn print_header<E>(t: &mut ColorableTerminal, s: &str) -> std::result::Result<(), E>
    where
        E: From<std::io::Error>,
    {
        t.attr(terminalsource::Attr::Bold)?;
        writeln!(t.lock(), "{s}")?;
        writeln!(t.lock(), "{}", "-".repeat(s.len()))?;
        writeln!(t.lock())?;
        t.reset()?;
        Ok(())
    }

    Ok(utils::ExitCode(0))
}

#[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
fn show_active_toolchain(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let verbose = m.get_flag("verbose");
    let cwd = utils::current_dir()?;
    match cfg.find_or_install_override_toolchain_or_default(&cwd) {
        Err(e) => {
            let root_cause = e.root_cause();
            if let Some(RustupError::ToolchainNotSelected) =
                root_cause.downcast_ref::<RustupError>()
            {
            } else {
                return Err(e);
            }
        }
        Ok((toolchain, reason)) => {
            if let Some(reason) = reason {
                writeln!(
                    process().stdout().lock(),
                    "{} ({})",
                    toolchain.name(),
                    reason
                )?;
            } else {
                writeln!(process().stdout().lock(), "{} (default)", toolchain.name())?;
            }
            if verbose {
                writeln!(process().stdout().lock(), "{}", toolchain.rustc_version())?;
            }
        }
    }
    Ok(utils::ExitCode(0))
}

#[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
fn show_rustup_home(cfg: &Cfg) -> Result<utils::ExitCode> {
    writeln!(process().stdout().lock(), "{}", cfg.rustup_dir.display())?;
    Ok(utils::ExitCode(0))
}

fn target_list(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain = m
        .get_one::<PartialToolchainDesc>("toolchain")
        .map(Into::into);
    let toolchain = explicit_or_dir_toolchain2(cfg, toolchain)?;
    // downcasting required because the toolchain files can name any toolchain
    let distributable = (&toolchain).try_into()?;

    if m.get_flag("installed") {
        common::list_installed_targets(distributable)
    } else {
        common::list_targets(distributable)
    }
}

fn target_add(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain_name = m
        .get_one::<PartialToolchainDesc>("toolchain")
        .map(Into::into);
    let toolchain = explicit_or_dir_toolchain2(cfg, toolchain_name)?;
    // XXX: long term move this error to cli ? the normal .into doesn't work
    // because Result here is the wrong sort and expression type ascription
    // isn't a feature yet.
    // list_components *and* add_component would both be inappropriate for
    // custom toolchains.
    let distributable = DistributableToolchain::try_from(&toolchain)?;
    let manifestation = distributable.get_manifestation()?;
    let config = manifestation.read_config()?.unwrap_or_default();
    let manifest = distributable.get_manifest()?;
    let components = manifest.query_components(distributable.desc(), &config)?;

    let mut targets: Vec<_> = m
        .get_many::<String>("target")
        .unwrap()
        .map(ToOwned::to_owned)
        .collect();

    if targets.contains(&"all".to_string()) {
        if targets.len() != 1 {
            return Err(anyhow!(
                "`rustup target add {}` includes `all`",
                targets.join(" ")
            ));
        }

        targets.clear();
        for component in components {
            if component.component.short_name_in_manifest() == "rust-std"
                && component.available
                && !component.installed
            {
                let target = component
                    .component
                    .target
                    .as_ref()
                    .expect("rust-std should have a target");
                targets.push(target.to_string());
            }
        }
    }

    for target in targets {
        let new_component = Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new(&target)),
            false,
        );
        distributable.add_component(new_component)?;
    }

    Ok(utils::ExitCode(0))
}

fn target_remove(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain = m
        .get_one::<PartialToolchainDesc>("toolchain")
        .map(Into::into);
    let toolchain = explicit_or_dir_toolchain2(cfg, toolchain)?;
    let distributable = DistributableToolchain::try_from(&toolchain)?;

    for target in m.get_many::<String>("target").unwrap() {
        let new_component = Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new(target)),
            false,
        );
        distributable.remove_component(new_component)?;
    }

    Ok(utils::ExitCode(0))
}

fn component_list(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;
    // downcasting required because the toolchain files can name any toolchain
    let distributable = (&toolchain).try_into()?;

    if m.get_flag("installed") {
        common::list_installed_components(distributable)?;
    } else {
        common::list_components(distributable)?;
    }
    Ok(utils::ExitCode(0))
}

fn component_add(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain = m
        .get_one::<PartialToolchainDesc>("toolchain")
        .map(Into::into);
    let toolchain = explicit_or_dir_toolchain2(cfg, toolchain)?;
    let distributable = DistributableToolchain::try_from(&toolchain)?;
    let target = get_target(m, &distributable);

    for component in m.get_many::<String>("component").unwrap() {
        let new_component = Component::new_with_target(component, false)
            .unwrap_or_else(|| Component::new(component.to_string(), target.clone(), true));
        distributable.add_component(new_component)?;
    }

    Ok(utils::ExitCode(0))
}

fn get_target(m: &ArgMatches, distributable: &DistributableToolchain<'_>) -> Option<TargetTriple> {
    m.get_one::<String>("target")
        .map(|s| &**s)
        .map(TargetTriple::new)
        .or_else(|| Some(distributable.desc().target.clone()))
}

fn component_remove(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;
    let distributable = DistributableToolchain::try_from(&toolchain)?;
    let target = get_target(m, &distributable);

    for component in m.get_many::<String>("component").unwrap() {
        let new_component = Component::new_with_target(component, false)
            .unwrap_or_else(|| Component::new(component.to_string(), target.clone(), true));
        distributable.remove_component(new_component)?;
    }

    Ok(utils::ExitCode(0))
}

fn explicit_or_dir_toolchain<'a>(cfg: &'a Cfg, m: &ArgMatches) -> Result<Toolchain<'a>> {
    let toolchain = m.get_one::<ResolvableToolchainName>("toolchain");
    explicit_or_dir_toolchain2(cfg, toolchain.cloned())
}

fn explicit_or_dir_toolchain2(
    cfg: &Cfg,
    toolchain: Option<ResolvableToolchainName>,
) -> Result<Toolchain<'_>> {
    match toolchain {
        Some(toolchain) => {
            let desc = toolchain.resolve(&cfg.get_default_host_triple()?)?;
            Ok(Toolchain::new(cfg, desc.into())?)
        }
        None => {
            let cwd = utils::current_dir()?;
            let (toolchain, _) = cfg.find_or_install_override_toolchain_or_default(&cwd)?;

            Ok(toolchain)
        }
    }
}

fn toolchain_list(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    common::list_toolchains(cfg, m.get_flag("verbose"))
}

fn toolchain_link(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain = m.get_one::<CustomToolchainName>("toolchain").unwrap();
    let path = m.get_one::<String>("path").unwrap();
    cfg.ensure_toolchains_dir()?;
    crate::toolchain::custom::CustomToolchain::install_from_dir(
        cfg,
        Path::new(path),
        toolchain,
        true,
    )?;
    Ok(utils::ExitCode(0))
}

fn toolchain_remove(cfg: &mut Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    for toolchain_name in m.get_many::<ResolvableToolchainName>("toolchain").unwrap() {
        let toolchain_name = toolchain_name.resolve(&cfg.get_default_host_triple()?)?;
        Toolchain::ensure_removed(cfg, (&toolchain_name).into())?;
    }
    Ok(utils::ExitCode(0))
}

fn override_add(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain_name = m.get_one::<ResolvableToolchainName>("toolchain").unwrap();
    let toolchain_name = toolchain_name.resolve(&cfg.get_default_host_triple()?)?;

    let path = if let Some(path) = m.get_one::<String>("path") {
        PathBuf::from(path)
    } else {
        utils::current_dir()?
    };

    match Toolchain::new(cfg, (&toolchain_name).into()) {
        Ok(_) => {}
        Err(e @ RustupError::ToolchainNotInstalled(_)) => match &toolchain_name {
            ToolchainName::Custom(_) => Err(e)?,
            ToolchainName::Official(desc) => {
                let status = DistributableToolchain::install(
                    cfg,
                    desc,
                    &[],
                    &[],
                    cfg.get_profile()?,
                    false,
                )?
                .0;
                writeln!(process().stdout().lock())?;
                common::show_channel_update(
                    cfg,
                    PackageUpdate::Toolchain(desc.clone()),
                    Ok(status),
                )?;
            }
        },
        Err(e) => Err(e)?,
    }

    cfg.make_override(&path, &toolchain_name)?;
    Ok(utils::ExitCode(0))
}

fn override_remove(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let paths = if m.get_flag("nonexistent") {
        let list: Vec<_> = cfg.settings_file.with(|s| {
            Ok(s.overrides
                .iter()
                .filter_map(|(k, _)| {
                    if Path::new(k).is_dir() {
                        None
                    } else {
                        Some(k.clone())
                    }
                })
                .collect())
        })?;
        if list.is_empty() {
            info!("no nonexistent paths detected");
        }
        list
    } else if let Some(path) = m.get_one::<String>("path") {
        vec![path.to_owned()]
    } else {
        vec![utils::current_dir()?.to_str().unwrap().to_string()]
    };

    for path in paths {
        if cfg
            .settings_file
            .with_mut(|s| Ok(s.remove_override(Path::new(&path), cfg.notify_handler.as_ref())))?
        {
            info!("override toolchain for '{}' removed", path);
        } else {
            info!("no override toolchain for '{}'", path);
            if m.get_one::<String>("path").is_none() && !m.get_flag("nonexistent") {
                info!(
                    "you may use `--path <path>` option to remove override toolchain \
                     for a specific path"
                );
            }
        }
    }
    Ok(utils::ExitCode(0))
}

const DOCS_DATA: &[(&str, &str, &str)] = &[
    // flags can be used to open specific documents, e.g. `rustup doc --nomicon`
    // tuple elements: document name used as flag, help message, document index path
    ("alloc", "The Rust core allocation and collections library", "alloc/index.html"),
    ("book", "The Rust Programming Language book", "book/index.html"),
    ("cargo", "The Cargo Book", "cargo/index.html"),
    ("core", "The Rust Core Library", "core/index.html"),
    ("edition-guide", "The Rust Edition Guide", "edition-guide/index.html"),
    ("nomicon", "The Dark Arts of Advanced and Unsafe Rust Programming", "nomicon/index.html"),
    ("proc_macro", "A support library for macro authors when defining new macros", "proc_macro/index.html"),
    ("reference", "The Rust Reference", "reference/index.html"),
    ("rust-by-example", "A collection of runnable examples that illustrate various Rust concepts and standard libraries", "rust-by-example/index.html"),
    ("rustc", "The compiler for the Rust programming language", "rustc/index.html"),
    ("rustdoc", "Documentation generator for Rust projects", "rustdoc/index.html"),
    ("std", "Standard library API documentation", "std/index.html"),
    ("test", "Support code for rustc's built in unit-test and micro-benchmarking framework", "test/index.html"),
    ("unstable-book", "The Unstable Book", "unstable-book/index.html"),
    ("embedded-book", "The Embedded Rust Book", "embedded-book/index.html"),
];

fn doc(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let toolchain = m
        .get_one::<PartialToolchainDesc>("toolchain")
        .map(Into::into);
    let toolchain = explicit_or_dir_toolchain2(cfg, toolchain)?;

    if let Ok(distributable) = DistributableToolchain::try_from(&toolchain) {
        let manifestation = distributable.get_manifestation()?;
        let config = manifestation.read_config()?.unwrap_or_default();
        let manifest = distributable.get_manifest()?;
        let components = manifest.query_components(distributable.desc(), &config)?;
        if let [_] = components
            .into_iter()
            .filter(|cstatus| {
                cstatus.component.short_name_in_manifest() == "rust-docs" && !cstatus.installed
            })
            .take(1)
            .collect::<Vec<ComponentStatus>>()
            .as_slice()
        {
            info!(
                "`rust-docs` not installed in toolchain `{}`",
                distributable.desc()
            );
            info!(
                "To install, try `rustup component add --toolchain {} rust-docs`",
                distributable.desc()
            );
            return Err(anyhow!(
                "unable to view documentation which is not installed"
            ));
        }
    };

    let topical_path: PathBuf;
    let cached_path: String;
    let mut has_docs_data_link = false;

    let doc_url =
        if let Some((short, _, path)) = DOCS_DATA.iter().find(|(name, _, _)| m.get_flag(name)) {
            if let Some(topic) = m.get_one::<String>("topic") {
                has_docs_data_link = true;
                cached_path = format!("{}/{}", short, topic);
                cached_path.as_str()
            } else {
                path
            }
        } else if let Some(topic) = m.get_one::<String>("topic") {
            topical_path = topical_doc::local_path(&toolchain.doc_path("").unwrap(), topic)?;
            topical_path.to_str().unwrap()
        } else {
            "index.html"
        };

    if m.get_flag("path") {
        let doc_path = toolchain.doc_path(doc_url)?;
        writeln!(process().stdout().lock(), "{}", doc_path.display())?;
        Ok(utils::ExitCode(0))
    } else {
        if has_docs_data_link {
            let url_path_buf = toolchain.doc_path(doc_url)?;
            let url_path = format!("file:///{}", url_path_buf.to_str().unwrap());
            let doc_path = Path::new(&url_path);
            utils::open_browser(doc_path)?
        } else {
            toolchain.open_docs(doc_url)?;
        }
        Ok(utils::ExitCode(0))
    }
}

fn man(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    let command = m.get_one::<String>("command").unwrap();

    let toolchain = explicit_or_dir_toolchain(cfg, m)?;
    let mut path = toolchain.path().to_path_buf();
    path.push("share");
    path.push("man");
    utils::assert_is_directory(&path)?;

    let mut manpaths = std::ffi::OsString::from(path);
    manpaths.push(":"); // prepend to the default MANPATH list
    if let Some(path) = process().var_os("MANPATH") {
        manpaths.push(path);
    }
    process::Command::new("man")
        .env("MANPATH", manpaths)
        .arg(command)
        .status()
        .expect("failed to open man page");
    Ok(utils::ExitCode(0))
}

fn self_uninstall(m: &ArgMatches) -> Result<utils::ExitCode> {
    let no_prompt = m.get_flag("no-prompt");

    self_update::uninstall(no_prompt)
}

fn set_default_host_triple(cfg: &Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    cfg.set_default_host_triple(m.get_one::<String>("host_triple").unwrap())?;
    Ok(utils::ExitCode(0))
}

fn set_profile(cfg: &mut Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    cfg.set_profile(m.get_one::<String>("profile-name").unwrap())?;
    Ok(utils::ExitCode(0))
}

fn set_auto_self_update(cfg: &mut Cfg, m: &ArgMatches) -> Result<utils::ExitCode> {
    if self_update::NEVER_SELF_UPDATE {
        let mut args = crate::process().args_os();
        let arg0 = args.next().map(PathBuf::from);
        let arg0 = arg0
            .as_ref()
            .and_then(|a| a.to_str())
            .ok_or(CLIError::NoExeName)?;
        warn!("{} is built with the no-self-update feature: setting auto-self-update will not have any effect.",arg0);
    }
    cfg.set_auto_self_update(m.get_one::<String>("auto-self-update-mode").unwrap())?;
    Ok(utils::ExitCode(0))
}

#[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
fn show_profile(cfg: &Cfg) -> Result<utils::ExitCode> {
    writeln!(process().stdout().lock(), "{}", cfg.get_profile()?)?;
    Ok(utils::ExitCode(0))
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum CompletionCommand {
    Rustup,
    Cargo,
}

impl clap::ValueEnum for CompletionCommand {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Rustup, Self::Cargo]
    }

    fn to_possible_value<'a>(&self) -> Option<clap::PossibleValue<'a>> {
        Some(match self {
            CompletionCommand::Rustup => PossibleValue::new("rustup"),
            CompletionCommand::Cargo => PossibleValue::new("cargo"),
        })
    }
}

impl fmt::Display for CompletionCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_possible_value() {
            Some(v) => write!(f, "{}", v.get_name()),
            None => unreachable!(),
        }
    }
}

fn output_completion_script(shell: Shell, command: CompletionCommand) -> Result<utils::ExitCode> {
    match command {
        CompletionCommand::Rustup => {
            clap_complete::generate(shell, &mut cli(), "rustup", &mut process().stdout().lock());
        }
        CompletionCommand::Cargo => {
            if let Shell::Zsh = shell {
                writeln!(process().stdout().lock(), "#compdef cargo")?;
            }

            let script = match shell {
                Shell::Bash => "/etc/bash_completion.d/cargo",
                Shell::Zsh => "/share/zsh/site-functions/_cargo",
                _ => {
                    return Err(anyhow!(
                        "{} does not currently support completions for {}",
                        command,
                        shell
                    ))
                }
            };

            writeln!(
                process().stdout().lock(),
                "if command -v rustc >/dev/null 2>&1; then\n\
                    \tsource \"$(rustc --print sysroot)\"{script}\n\
                 fi",
            )?;
        }
    }

    Ok(utils::ExitCode(0))
}
