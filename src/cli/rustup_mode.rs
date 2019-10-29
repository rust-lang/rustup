use crate::common;
use crate::errors::*;
use crate::help::*;
use crate::self_update;
use crate::term2;
use crate::term2::Terminal;
use crate::topical_doc;
use clap::{App, AppSettings, Arg, ArgGroup, ArgMatches, Shell, SubCommand};
use rustup::dist::dist::{PartialTargetTriple, PartialToolchainDesc, Profile, TargetTriple};
use rustup::dist::manifest::Component;
use rustup::utils::utils::{self, ExitCode};
use rustup::{command, Cfg, Toolchain};
use std::error::Error;
use std::fmt;
use std::io::Write;
use std::iter;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::str::FromStr;

fn handle_epipe(res: Result<()>) -> Result<()> {
    match res {
        Err(Error(ErrorKind::Io(ref err), _)) if err.kind() == std::io::ErrorKind::BrokenPipe => {
            Ok(())
        }
        res => res,
    }
}

pub fn main() -> Result<()> {
    crate::self_update::cleanup_self_updater()?;

    let matches = cli().get_matches();
    let verbose = matches.is_present("verbose");
    let quiet = matches.is_present("quiet");
    let cfg = &mut common::set_globals(verbose, quiet)?;

    if let Some(t) = matches.value_of("+toolchain") {
        cfg.set_toolchain_override(&t[1..]);
    }

    if maybe_upgrade_data(cfg, &matches)? {
        return Ok(());
    }

    cfg.check_metadata_version()?;

    match matches.subcommand() {
        ("dump-testament", _) => common::dump_testament(),
        ("show", Some(c)) => match c.subcommand() {
            ("active-toolchain", Some(_)) => handle_epipe(show_active_toolchain(cfg))?,
            ("home", Some(_)) => handle_epipe(show_rustup_home(cfg))?,
            ("profile", Some(_)) => handle_epipe(show_profile(cfg))?,
            (_, _) => handle_epipe(show(cfg))?,
        },
        ("install", Some(m)) => update(cfg, m)?,
        ("update", Some(m)) => update(cfg, m)?,
        ("check", Some(_)) => check_updates(cfg)?,
        ("uninstall", Some(m)) => toolchain_remove(cfg, m)?,
        ("default", Some(m)) => default_(cfg, m)?,
        ("toolchain", Some(c)) => match c.subcommand() {
            ("install", Some(m)) => update(cfg, m)?,
            ("list", Some(m)) => handle_epipe(toolchain_list(cfg, m))?,
            ("link", Some(m)) => toolchain_link(cfg, m)?,
            ("uninstall", Some(m)) => toolchain_remove(cfg, m)?,
            (_, _) => unreachable!(),
        },
        ("target", Some(c)) => match c.subcommand() {
            ("list", Some(m)) => handle_epipe(target_list(cfg, m))?,
            ("add", Some(m)) => target_add(cfg, m)?,
            ("remove", Some(m)) => target_remove(cfg, m)?,
            (_, _) => unreachable!(),
        },
        ("component", Some(c)) => match c.subcommand() {
            ("list", Some(m)) => handle_epipe(component_list(cfg, m))?,
            ("add", Some(m)) => component_add(cfg, m)?,
            ("remove", Some(m)) => component_remove(cfg, m)?,
            (_, _) => unreachable!(),
        },
        ("override", Some(c)) => match c.subcommand() {
            ("list", Some(_)) => handle_epipe(common::list_overrides(cfg))?,
            ("set", Some(m)) => override_add(cfg, m)?,
            ("unset", Some(m)) => override_remove(cfg, m)?,
            (_, _) => unreachable!(),
        },
        ("run", Some(m)) => run(cfg, m)?,
        ("which", Some(m)) => which(cfg, m)?,
        ("doc", Some(m)) => doc(cfg, m)?,
        ("man", Some(m)) => man(cfg, m)?,
        ("self", Some(c)) => match c.subcommand() {
            ("update", Some(_)) => self_update::update()?,
            ("uninstall", Some(m)) => self_uninstall(m)?,
            (_, _) => unreachable!(),
        },
        ("set", Some(c)) => match c.subcommand() {
            ("default-host", Some(m)) => set_default_host_triple(cfg, m)?,
            ("profile", Some(m)) => set_profile(cfg, m)?,
            (_, _) => unreachable!(),
        },
        ("completions", Some(c)) => {
            if let Some(shell) = c.value_of("shell") {
                output_completion_script(
                    shell.parse::<Shell>().unwrap(),
                    c.value_of("command")
                        .and_then(|cmd| cmd.parse::<CompletionCommand>().ok())
                        .unwrap_or(CompletionCommand::Rustup),
                )?;
            }
        }
        (_, _) => unreachable!(),
    }

    Ok(())
}

pub fn cli() -> App<'static, 'static> {
    let mut app = App::new("rustup")
        .version(common::version())
        .about("The Rust toolchain installer")
        .after_help(RUSTUP_HELP)
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("verbose")
                .help("Enable verbose output")
                .short("v")
                .long("verbose"),
        )
        .arg(
            Arg::with_name("quiet")
                .conflicts_with("verbose")
                .help("Disable progress output")
                .short("q")
                .long("quiet"),
        )
        .arg(
            Arg::with_name("+toolchain")
                .help("release channel (e.g. +stable) or custom toolchain to set override")
                .validator(|s| {
                    if s.starts_with('+') {
                        Ok(())
                    } else {
                        Err("Toolchain overrides must begin with '+'".into())
                    }
                }),
        )
        .subcommand(
            SubCommand::with_name("dump-testament")
                .about("Dump information about the build")
                .setting(AppSettings::Hidden), // Not for users, only CI
        )
        .subcommand(
            SubCommand::with_name("show")
                .about("Show the active and installed toolchains or profiles")
                .after_help(SHOW_HELP)
                .setting(AppSettings::VersionlessSubcommands)
                .setting(AppSettings::DeriveDisplayOrder)
                .subcommand(
                    SubCommand::with_name("active-toolchain")
                        .about("Show the active toolchain")
                        .after_help(SHOW_ACTIVE_TOOLCHAIN_HELP),
                )
                .subcommand(
                    SubCommand::with_name("home")
                        .about("Display the computed value of RUSTUP_HOME"),
                )
                .subcommand(SubCommand::with_name("profile").about("Show the current profile")),
        )
        .subcommand(
            SubCommand::with_name("install")
                .about("Update Rust toolchains")
                .after_help(INSTALL_HELP)
                .setting(AppSettings::Hidden) // synonym for 'toolchain install'
                .arg(
                    Arg::with_name("toolchain")
                        .help(TOOLCHAIN_ARG_HELP)
                        .required(true)
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("profile")
                        .long("profile")
                        .takes_value(true)
                        .possible_values(Profile::names())
                        .required(false),
                )
                .arg(
                    Arg::with_name("no-self-update")
                        .help("Don't perform self-update when running the `rustup install` command")
                        .long("no-self-update")
                        .takes_value(false),
                )
                .arg(
                    Arg::with_name("force")
                        .help("Force an update, even if some components are missing")
                        .long("force")
                        .takes_value(false),
                ),
        )
        .subcommand(
            SubCommand::with_name("uninstall")
                .about("Uninstall Rust toolchains")
                .setting(AppSettings::Hidden) // synonym for 'toolchain uninstall'
                .arg(
                    Arg::with_name("toolchain")
                        .help(TOOLCHAIN_ARG_HELP)
                        .required(true)
                        .multiple(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("Update Rust toolchains and rustup")
                .after_help(UPDATE_HELP)
                .arg(
                    Arg::with_name("toolchain")
                        .help(TOOLCHAIN_ARG_HELP)
                        .required(false)
                        .multiple(true),
                )
                .arg(
                    Arg::with_name("no-self-update")
                        .help("Don't perform self update when running the `rustup update` command")
                        .long("no-self-update")
                        .takes_value(false),
                )
                .arg(
                    Arg::with_name("force")
                        .help("Force an update, even if some components are missing")
                        .long("force")
                        .takes_value(false),
                ),
        )
        .subcommand(SubCommand::with_name("check").about("Check for updates to Rust toolchains"))
        .subcommand(
            SubCommand::with_name("default")
                .about("Set the default toolchain")
                .after_help(DEFAULT_HELP)
                .arg(
                    Arg::with_name("toolchain")
                        .help(TOOLCHAIN_ARG_HELP)
                        .required(false),
                ),
        );

    // We break the app creation here so that rustfmt can cope
    // If rustfmt ceases to format this block, break it up further
    app = app.subcommand(
            SubCommand::with_name("toolchain")
                .about("Modify or query the installed toolchains")
                .after_help(TOOLCHAIN_HELP)
                .setting(AppSettings::VersionlessSubcommands)
                .setting(AppSettings::DeriveDisplayOrder)
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("list")
                        .about("List installed toolchains")
                        .arg(
                            Arg::with_name("verbose")
                                .help("Enable verbose output with toolchain information")
                                .takes_value(false)
                                .short("v")
                                .long("verbose"),
                        )
                )
                .subcommand(
                    SubCommand::with_name("install")
                        .about("Install or update a given toolchain")
                        .aliases(&["update", "add"])
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .required(true)
                                .multiple(true),
                        )
                        .arg(
                            Arg::with_name("profile")
                                .long("profile")
                                .takes_value(true)
                                .possible_values(Profile::names())
                                .required(false)
                        )
                        .arg(
                            Arg::with_name("no-self-update")
                                .help("Don't perform self update when running the `rustup toolchain install` command")
                                .long("no-self-update")
                                .takes_value(false)
                        )
                        .arg(
                            Arg::with_name("components")
                                .help("Add specific components on installation")
                                .long("component")
                                .short("c")
                                .takes_value(true)
                                .multiple(true)
                        )
                        .arg(
                            Arg::with_name("targets")
                                .help("Add specific targets on installation")
                                .long("target")
                                .short("t")
                                .takes_value(true)
                                .multiple(true)
                        )
                        .arg(
                            Arg::with_name("force")
                                .help("Force an update, even if some components are missing")
                                .long("force")
                                .takes_value(false),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("uninstall")
                        .about("Uninstall a toolchain")
                        .alias("remove")
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .required(true)
                                .multiple(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("link")
                        .about("Create a custom toolchain by symlinking to a directory")
                        .after_help(TOOLCHAIN_LINK_HELP)
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .required(true),
                        )
                        .arg(Arg::with_name("path").required(true)),
                ),
        )
        .subcommand(
            SubCommand::with_name("target")
                .about("Modify a toolchain's supported targets")
                .setting(AppSettings::VersionlessSubcommands)
                .setting(AppSettings::DeriveDisplayOrder)
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("list")
                        .about("List installed and available targets")
                        .arg(
                            Arg::with_name("installed")
                                .long("--installed")
                                .help("List only installed targets"),
                        )
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("add")
                        .about("Add a target to a Rust toolchain")
                        .alias("install")
                        .arg(
                            Arg::with_name("target")
                                .required(true)
                                .multiple(true)
                                .help(
                                    "List of targets to install; \
                                     \"all\" installs all available targets"
                                )
                        )
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("remove")
                        .about("Remove a target from a Rust toolchain")
                        .alias("uninstall")
                        .arg(Arg::with_name("target").required(true).multiple(true))
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true),
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("component")
                .about("Modify a toolchain's installed components")
                .setting(AppSettings::VersionlessSubcommands)
                .setting(AppSettings::DeriveDisplayOrder)
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("list")
                        .about("List installed and available components")
                        .arg(
                            Arg::with_name("installed")
                                .long("--installed")
                                .help("List only installed components"),
                        )
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("add")
                        .about("Add a component to a Rust toolchain")
                        .arg(Arg::with_name("component").required(true).multiple(true))
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true),
                        )
                        .arg(Arg::with_name("target").long("target").takes_value(true)),
                )
                .subcommand(
                    SubCommand::with_name("remove")
                        .about("Remove a component from a Rust toolchain")
                        .arg(Arg::with_name("component").required(true).multiple(true))
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .long("toolchain")
                                .takes_value(true),
                        )
                        .arg(Arg::with_name("target").long("target").takes_value(true)),
                ),
        )
        .subcommand(
            SubCommand::with_name("override")
                .about("Modify directory toolchain overrides")
                .after_help(OVERRIDE_HELP)
                .setting(AppSettings::VersionlessSubcommands)
                .setting(AppSettings::DeriveDisplayOrder)
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("list").about("List directory toolchain overrides"),
                )
                .subcommand(
                    SubCommand::with_name("set")
                        .about("Set the override toolchain for a directory")
                        .alias("add")
                        .arg(
                            Arg::with_name("toolchain")
                                .help(TOOLCHAIN_ARG_HELP)
                                .required(true),
                        )
                        .arg(
                            Arg::with_name("path")
                                .long("path")
                                .takes_value(true)
                                .help("Path to the directory"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("unset")
                        .about("Remove the override toolchain for a directory")
                        .after_help(OVERRIDE_UNSET_HELP)
                        .alias("remove")
                        .arg(
                            Arg::with_name("path")
                                .long("path")
                                .takes_value(true)
                                .help("Path to the directory"),
                        )
                        .arg(
                            Arg::with_name("nonexistent")
                                .long("nonexistent")
                                .takes_value(false)
                                .help("Remove override toolchain for all nonexistent directories"),
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("run")
                .about("Run a command with an environment configured for a given toolchain")
                .after_help(RUN_HELP)
                .setting(AppSettings::TrailingVarArg)
                .arg(
                    Arg::with_name("install")
                        .help("Install the requested toolchain if needed")
                        .long("install"),
                )
                .arg(
                    Arg::with_name("toolchain")
                        .help(TOOLCHAIN_ARG_HELP)
                        .required(true),
                )
                .arg(
                    Arg::with_name("command")
                        .required(true)
                        .multiple(true)
                        .use_delimiter(false),
                ),
        )
        .subcommand(
            SubCommand::with_name("which")
                .about("Display which binary will be run for a given command")
                .arg(Arg::with_name("command").required(true))
                .arg(
                    Arg::with_name("toolchain")
                        .help(TOOLCHAIN_ARG_HELP)
                        .long("toolchain")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("doc")
                .alias("docs")
                .about("Open the documentation for the current toolchain")
                .after_help(DOC_HELP)
                .arg(
                    Arg::with_name("path")
                        .long("path")
                        .help("Only print the path to the documentation"),
                )
                .args(
                    &DOCS_DATA
                        .iter()
                        .map(|(name, help_msg, _)| Arg::with_name(name).long(name).help(help_msg))
                        .collect::<Vec<_>>(),
                )
                .arg(
                    Arg::with_name("toolchain")
                        .help(TOOLCHAIN_ARG_HELP)
                        .long("toolchain")
                        .takes_value(true),
                )
                .group(
                    ArgGroup::with_name("page").args(
                        &DOCS_DATA
                            .iter()
                            .map(|(name, _, _)| *name)
                            .collect::<Vec<_>>(),
                    ),
                )
                .arg(
                    Arg::with_name("topic")
                         .help("Topic such as 'core', 'fn', 'usize', 'eprintln!', 'core::arch', 'alloc::format!', 'std::fs', 'std::fs::read_dir', 'std::io::Bytes', 'std::iter::Sum', 'std::io::error::Result' etc..."),
                    ),
        );

    if cfg!(not(target_os = "windows")) {
        app = app.subcommand(
            SubCommand::with_name("man")
                .about("View the man page for a given command")
                .arg(Arg::with_name("command").required(true))
                .arg(
                    Arg::with_name("toolchain")
                        .help(TOOLCHAIN_ARG_HELP)
                        .long("toolchain")
                        .takes_value(true),
                ),
        );
    }

    app = app
        .subcommand(
            SubCommand::with_name("self")
                .about("Modify the rustup installation")
                .setting(AppSettings::VersionlessSubcommands)
                .setting(AppSettings::DeriveDisplayOrder)
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("update").about("Download and install updates to rustup"),
                )
                .subcommand(
                    SubCommand::with_name("uninstall")
                        .about("Uninstall rustup.")
                        .arg(Arg::with_name("no-prompt").short("y")),
                )
                .subcommand(
                    SubCommand::with_name("upgrade-data")
                        .about("Upgrade the internal data format."),
                ),
        )
        .subcommand(
            SubCommand::with_name("set")
                .about("Alter rustup settings")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("default-host")
                        .about("The triple used to identify toolchains when not specified")
                        .arg(Arg::with_name("host_triple").required(true)),
                )
                .subcommand(
                    SubCommand::with_name("profile")
                        .about("The default components installed")
                        .arg(
                            Arg::with_name("profile-name")
                                .required(true)
                                .possible_values(Profile::names())
                                .default_value(Profile::default_name()),
                        ),
                ),
        );

    // Clap provides no good way to say that help should be printed in all
    // cases where an argument without a default is not provided. The following
    // creates lists out all the conditions where the "shell" argument are
    // provided and give the default of "rustup". This way if "shell" is not
    // provided then the help will still be printed.
    let completion_defaults = Shell::variants()
        .iter()
        .map(|&shell| ("shell", Some(shell), "rustup"))
        .collect::<Vec<_>>();

    app.subcommand(
        SubCommand::with_name("completions")
            .about("Generate tab-completion scripts for your shell")
            .after_help(COMPLETIONS_HELP)
            .setting(AppSettings::ArgRequiredElseHelp)
            .arg(Arg::with_name("shell").possible_values(&Shell::variants()))
            .arg(
                Arg::with_name("command")
                    .possible_values(&CompletionCommand::variants())
                    .default_value_ifs(&completion_defaults[..]),
            ),
    )
}

fn maybe_upgrade_data(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<bool> {
    match m.subcommand() {
        ("self", Some(c)) => match c.subcommand() {
            ("upgrade-data", Some(_)) => {
                cfg.upgrade_data()?;
                Ok(true)
            }
            _ => Ok(false),
        },
        _ => Ok(false),
    }
}

fn update_bare_triple_check(cfg: &Cfg, name: &str) -> Result<()> {
    if let Some(triple) = PartialTargetTriple::new(name) {
        warn!("(partial) target triple specified instead of toolchain name");
        let installed_toolchains = cfg.list_toolchains()?;
        let default = cfg.find_default()?;
        let default_name = default.map(|t| t.name().to_string()).unwrap_or_default();
        let mut candidates = vec![];
        for t in installed_toolchains {
            if t == default_name {
                continue;
            }
            if let Ok(desc) = PartialToolchainDesc::from_str(&t) {
                fn triple_comp_eq(given: &str, from_desc: Option<&String>) -> bool {
                    from_desc.map_or(false, |s| *s == *given)
                }

                let triple_matches = triple
                    .arch
                    .as_ref()
                    .map_or(true, |s| triple_comp_eq(s, desc.target.arch.as_ref()))
                    && triple
                        .os
                        .as_ref()
                        .map_or(true, |s| triple_comp_eq(s, desc.target.os.as_ref()))
                    && triple
                        .env
                        .as_ref()
                        .map_or(true, |s| triple_comp_eq(s, desc.target.env.as_ref()));
                if triple_matches {
                    candidates.push(t);
                }
            }
        }
        match candidates.len() {
            0 => err!("no candidate toolchains found"),
            1 => println!("\nyou may use the following toolchain: {}\n", candidates[0]),
            _ => {
                println!("\nyou may use one of the following toolchains:");
                for n in &candidates {
                    println!("{}", n);
                }
                println!();
            }
        }
        return Err(ErrorKind::ToolchainNotInstalled(name.to_string()).into());
    }
    Ok(())
}

fn default_bare_triple_check(cfg: &Cfg, name: &str) -> Result<()> {
    if let Some(triple) = PartialTargetTriple::new(name) {
        warn!("(partial) target triple specified instead of toolchain name");
        let default = cfg.find_default()?;
        let default_name = default.map(|t| t.name().to_string()).unwrap_or_default();
        if let Ok(mut desc) = PartialToolchainDesc::from_str(&default_name) {
            desc.target = triple;
            let maybe_toolchain = format!("{}", desc);
            let toolchain = cfg.get_toolchain(maybe_toolchain.as_ref(), false)?;
            if toolchain.name() == default_name {
                warn!(
                    "(partial) triple '{}' resolves to a toolchain that is already default",
                    name
                );
            } else {
                println!(
                    "\nyou may use the following toolchain: {}\n",
                    toolchain.name()
                );
            }
            return Err(ErrorKind::ToolchainNotInstalled(name.to_string()).into());
        }
    }
    Ok(())
}

fn default_(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    if m.is_present("toolchain") {
        let toolchain = m.value_of("toolchain").expect("");
        default_bare_triple_check(cfg, toolchain)?;
        let toolchain = cfg.get_toolchain(toolchain, false)?;

        let status = if !toolchain.is_custom() {
            Some(toolchain.install_from_dist_if_not_installed()?)
        } else if !toolchain.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(toolchain.name().to_string()).into());
        } else {
            None
        };

        toolchain.make_default()?;

        if let Some(status) = status {
            println!();
            common::show_channel_update(cfg, toolchain.name(), Ok(status))?;
        }

        let cwd = utils::current_dir()?;
        if let Some((toolchain, reason)) = cfg.find_override(&cwd)? {
            info!(
                "note that the toolchain '{}' is currently in use ({})",
                toolchain.name(),
                reason
            );
        }
    } else {
        let default_toolchain: Result<String> = cfg
            .get_default()?
            .ok_or_else(|| "no default toolchain configured".into());
        println!("{} (default)", default_toolchain?);
    }

    Ok(())
}

fn check_updates(cfg: &Cfg) -> Result<()> {
    let mut t = term2::stdout();
    let channels = cfg.list_channels()?;

    for channel in channels {
        match channel {
            (ref name, Ok(ref toolchain)) => {
                let current_version = toolchain.show_version()?;
                let dist_version = toolchain.show_dist_version()?;
                let _ = t.attr(term2::Attr::Bold);
                write!(t, "{} - ", name)?;
                match (current_version, dist_version) {
                    (None, None) => {
                        let _ = t.fg(term2::color::RED);
                        writeln!(t, "Cannot identify installed or update versions")?;
                    }
                    (Some(cv), None) => {
                        let _ = t.fg(term2::color::GREEN);
                        write!(t, "Up to date")?;
                        let _ = t.reset();
                        writeln!(t, " : {}", cv)?;
                    }
                    (Some(cv), Some(dv)) => {
                        let _ = t.fg(term2::color::YELLOW);
                        write!(t, "Update available")?;
                        let _ = t.reset();
                        writeln!(t, " : {} -> {}", cv, dv)?;
                    }
                    (None, Some(dv)) => {
                        let _ = t.fg(term2::color::YELLOW);
                        write!(t, "Update available")?;
                        let _ = t.reset();
                        writeln!(t, " : (Unknown version) -> {}", dv)?;
                    }
                }
            }
            (_, Err(err)) => return Err(err.into()),
        }
    }
    Ok(())
}

fn update(cfg: &mut Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let self_update = !m.is_present("no-self-update") && !self_update::NEVER_SELF_UPDATE;
    if let Some(p) = m.value_of("profile") {
        let p = Profile::from_str(p)?;
        cfg.set_profile_override(p);
    }
    let cfg = &cfg;
    if let Some(names) = m.values_of("toolchain") {
        for name in names {
            update_bare_triple_check(cfg, name)?;
            let toolchain = cfg.get_toolchain(name, false)?;

            let status = if !toolchain.is_custom() {
                let components: Vec<_> = m
                    .values_of("components")
                    .map(|v| v.collect())
                    .unwrap_or_else(Vec::new);
                let targets: Vec<_> = m
                    .values_of("targets")
                    .map(|v| v.collect())
                    .unwrap_or_else(Vec::new);
                Some(toolchain.install_from_dist(m.is_present("force"), &components, &targets)?)
            } else if !toolchain.exists() {
                return Err(ErrorKind::InvalidToolchainName(toolchain.name().to_string()).into());
            } else {
                None
            };

            if let Some(status) = status {
                println!();
                common::show_channel_update(cfg, toolchain.name(), Ok(status))?;
            }

            if cfg.get_default()?.is_none() {
                use rustup::UpdateStatus;
                if let Some(UpdateStatus::Installed) = status {
                    toolchain.make_default()?;
                }
            }
        }
        if self_update {
            common::self_update(|| Ok(()))?;
        }
    } else {
        common::update_all_channels(cfg, self_update, m.is_present("force"))?;
        info!("cleaning up downloads & tmp directories");
        utils::delete_dir_contents(&cfg.download_dir);
        cfg.temp_cfg.clean();
    }

    Ok(())
}

fn run(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = m.value_of("toolchain").expect("");
    let args = m.values_of("command").unwrap();
    let args: Vec<_> = args.collect();
    let cmd = cfg.create_command_for_toolchain(toolchain, m.is_present("install"), args[0])?;

    let ExitCode(c) = command::run_command_for_dir(cmd, args[0], &args[1..])?;

    process::exit(c)
}

fn which(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let binary = m.value_of("command").expect("");
    let binary_path = if m.is_present("toolchain") {
        let toolchain = m.value_of("toolchain").expect("");
        cfg.which_binary_by_toolchain(toolchain, binary)?
            .expect("binary not found")
    } else {
        cfg.which_binary(&utils::current_dir()?, binary)?
            .expect("binary not found")
    };

    utils::assert_is_file(&binary_path)?;

    println!("{}", binary_path.display());

    Ok(())
}

fn show(cfg: &Cfg) -> Result<()> {
    // Print host triple
    {
        let mut t = term2::stdout();
        t.attr(term2::Attr::Bold)?;
        write!(t, "Default host: ")?;
        t.reset()?;
        writeln!(t, "{}", cfg.get_default_host_triple()?)?;
    }

    // Print rustup home directory
    {
        let mut t = term2::stdout();
        t.attr(term2::Attr::Bold)?;
        write!(t, "rustup home:  ")?;
        t.reset()?;
        writeln!(t, "{}", cfg.rustup_dir.display())?;
        writeln!(t)?;
    }

    let cwd = utils::current_dir()?;
    let installed_toolchains = cfg.list_toolchains()?;
    let active_toolchain = cfg.find_override_toolchain_or_default(&cwd);

    // active_toolchain will carry the reason we don't have one in its detail.
    let active_targets = if let Ok(ref at) = active_toolchain {
        if let Some((ref t, _)) = *at {
            match t.list_components() {
                Ok(cs_vec) => cs_vec
                    .into_iter()
                    .filter(|c| c.component.short_name_in_manifest() == "rust-std")
                    .filter(|c| c.installed)
                    .collect(),
                Err(_) => vec![],
            }
        } else {
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
        let mut t = term2::stdout();
        if show_headers {
            print_header(&mut t, "installed toolchains")?;
        }
        let default_name: Result<String> = cfg
            .get_default()?
            .ok_or_else(|| "no default toolchain configured".into());
        let default_name = default_name?;
        for it in installed_toolchains {
            if default_name == it {
                writeln!(t, "{} (default)", it)?;
            } else {
                writeln!(t, "{}", it)?;
            }
        }
        if show_headers {
            writeln!(t)?
        };
    }

    if show_active_targets {
        let mut t = term2::stdout();
        if show_headers {
            print_header(&mut t, "installed targets for active toolchain")?;
        }
        for at in active_targets {
            writeln!(
                t,
                "{}",
                at.component
                    .target
                    .as_ref()
                    .expect("rust-std should have a target")
            )?;
        }
        if show_headers {
            writeln!(t)?;
        };
    }

    if show_active_toolchain {
        let mut t = term2::stdout();
        if show_headers {
            print_header(&mut t, "active toolchain")?;
        }

        match active_toolchain {
            Ok(atc) => match atc {
                Some((ref toolchain, Some(ref reason))) => {
                    writeln!(t, "{} ({})", toolchain.name(), reason)?;
                    writeln!(t, "{}", common::rustc_version(toolchain))?;
                }
                Some((ref toolchain, None)) => {
                    writeln!(t, "{} (default)", toolchain.name())?;
                    writeln!(t, "{}", common::rustc_version(toolchain))?;
                }
                None => {
                    writeln!(t, "no active toolchain")?;
                }
            },
            Err(err) => {
                if let Some(cause) = err.source() {
                    writeln!(t, "(error: {}, {})", err, cause)?;
                } else {
                    writeln!(t, "(error: {})", err)?;
                }
            }
        }

        if show_headers {
            writeln!(t)?
        }
    }

    fn print_header(t: &mut term::StdoutTerminal, s: &str) -> Result<()> {
        t.attr(term2::Attr::Bold)?;
        writeln!(t, "{}", s)?;
        writeln!(t, "{}", iter::repeat("-").take(s.len()).collect::<String>())?;
        writeln!(t)?;
        t.reset()?;
        Ok(())
    }

    Ok(())
}

fn show_active_toolchain(cfg: &Cfg) -> Result<()> {
    let cwd = utils::current_dir()?;
    if let Some((toolchain, reason)) = cfg.find_override_toolchain_or_default(&cwd)? {
        if let Some(reason) = reason {
            println!("{} ({})", toolchain.name(), reason);
        } else {
            println!("{} (default)", toolchain.name());
        }
    }
    Ok(())
}

fn show_rustup_home(cfg: &Cfg) -> Result<()> {
    println!("{}", cfg.rustup_dir.display());
    Ok(())
}

fn target_list(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;

    if m.is_present("installed") {
        common::list_installed_targets(&toolchain)
    } else {
        common::list_targets(&toolchain)
    }
}

fn target_add(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;

    let mut targets: Vec<String> = m
        .values_of("target")
        .expect("")
        .map(ToString::to_string)
        .collect();

    if targets.contains(&"all".to_string()) {
        if targets.len() != 1 {
            return Err(ErrorKind::TargetAllSpecifiedWithTargets(targets).into());
        }

        targets.clear();
        for component in toolchain.list_components()? {
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

    for target in &targets {
        let new_component = Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new(target)),
            false,
        );
        toolchain.add_component(new_component)?;
    }

    Ok(())
}

fn target_remove(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;

    for target in m.values_of("target").expect("") {
        let new_component = Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new(target)),
            false,
        );

        toolchain.remove_component(new_component)?;
    }

    Ok(())
}

fn component_list(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;

    if m.is_present("installed") {
        common::list_installed_components(&toolchain)
    } else {
        common::list_components(&toolchain)
    }
}

fn component_add(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;
    let target = m.value_of("target").map(TargetTriple::new).or_else(|| {
        toolchain
            .desc()
            .as_ref()
            .ok()
            .map(|desc| desc.target.clone())
    });

    for component in m.values_of("component").expect("") {
        let new_component = Component::new(component.to_string(), target.clone(), true);

        toolchain.add_component(new_component)?;
    }

    Ok(())
}

fn component_remove(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;
    let target = m.value_of("target").map(TargetTriple::new).or_else(|| {
        toolchain
            .desc()
            .as_ref()
            .ok()
            .map(|desc| desc.target.clone())
    });

    for component in m.values_of("component").expect("") {
        let new_component = Component::new(component.to_string(), target.clone(), true);

        toolchain.remove_component(new_component)?;
    }

    Ok(())
}

fn explicit_or_dir_toolchain<'a>(cfg: &'a Cfg, m: &ArgMatches<'_>) -> Result<Toolchain<'a>> {
    let toolchain = m.value_of("toolchain");
    if let Some(toolchain) = toolchain {
        let toolchain = cfg.get_toolchain(toolchain, false)?;
        return Ok(toolchain);
    }

    let cwd = utils::current_dir()?;
    let (toolchain, _) = cfg.toolchain_for_dir(&cwd)?;

    Ok(toolchain)
}

fn toolchain_list(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    common::list_toolchains(cfg, m.is_present("verbose"))
}

fn toolchain_link(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = m.value_of("toolchain").expect("");
    let path = m.value_of("path").expect("");
    let toolchain = cfg.get_toolchain(toolchain, true)?;

    toolchain
        .install_from_dir(Path::new(path), true)
        .map_err(std::convert::Into::into)
}

fn toolchain_remove(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    for toolchain in m.values_of("toolchain").expect("") {
        let toolchain = cfg.get_toolchain(toolchain, false)?;
        toolchain.remove()?;
    }
    Ok(())
}

fn override_add(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = m.value_of("toolchain").expect("");
    let toolchain = cfg.get_toolchain(toolchain, false)?;

    let status = if !toolchain.is_custom() {
        Some(toolchain.install_from_dist_if_not_installed()?)
    } else if !toolchain.exists() {
        return Err(ErrorKind::ToolchainNotInstalled(toolchain.name().to_string()).into());
    } else {
        None
    };

    let path = if let Some(path) = m.value_of("path") {
        PathBuf::from(path)
    } else {
        utils::current_dir()?
    };
    toolchain.make_override(&path)?;

    if let Some(status) = status {
        println!();
        common::show_channel_update(cfg, toolchain.name(), Ok(status))?;
    }

    Ok(())
}

fn override_remove(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let paths = if m.is_present("nonexistent") {
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
    } else if m.is_present("path") {
        vec![m.value_of("path").unwrap().to_string()]
    } else {
        vec![utils::current_dir()?.to_str().unwrap().to_string()]
    };

    for path in paths {
        if cfg
            .settings_file
            .with_mut(|s| Ok(s.remove_override(&Path::new(&path), cfg.notify_handler.as_ref())))?
        {
            info!("override toolchain for '{}' removed", path);
        } else {
            info!("no override toolchain for '{}'", path);
            if !m.is_present("path") && !m.is_present("nonexistent") {
                info!(
                    "you may use `--path <path>` option to remove override toolchain \
                     for a specific path"
                );
            }
        }
    }
    Ok(())
}

const DOCS_DATA: &[(&str, &str, &str,)] = &[
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
    ("rustdoc", "Generate documentation for Rust projects", "rustdoc/index.html"),
    ("std", "Standard library API documentation", "std/index.html"),
    ("test", "Support code for rustc's built in unit-test and micro-benchmarking framework", "test/index.html"),
    ("unstable-book", "The Unstable Book", "unstable-book/index.html"),
    ("embedded-book", "The Embedded Rust Book", "embedded-book/index.html"),
];

fn doc(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let toolchain = explicit_or_dir_toolchain(cfg, m)?;
    let topical_path: PathBuf;

    let doc_url = if let Some(topic) = m.value_of("topic") {
        topical_path = topical_doc::local_path(&toolchain.doc_path("").unwrap(), topic)?;
        topical_path.to_str().unwrap()
    } else if let Some((_, _, path)) = DOCS_DATA.iter().find(|(name, _, _)| m.is_present(name)) {
        path
    } else {
        "index.html"
    };

    if m.is_present("path") {
        let doc_path = toolchain.doc_path(doc_url)?;
        println!("{}", doc_path.display());
        Ok(())
    } else {
        toolchain.open_docs(doc_url).map_err(Into::into)
    }
}

fn man(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    let command = m.value_of("command").unwrap();

    let toolchain = explicit_or_dir_toolchain(cfg, m)?;
    let mut toolchain = toolchain.path().to_path_buf();
    toolchain.push("share");
    toolchain.push("man");
    utils::assert_is_directory(&toolchain)?;

    let mut manpaths = std::ffi::OsString::from(toolchain);
    manpaths.push(":"); // prepend to the default MANPATH list
    if let Some(path) = std::env::var_os("MANPATH") {
        manpaths.push(path);
    }
    Command::new("man")
        .env("MANPATH", manpaths)
        .arg(command)
        .status()
        .expect("failed to open man page");
    Ok(())
}

fn self_uninstall(m: &ArgMatches<'_>) -> Result<()> {
    let no_prompt = m.is_present("no-prompt");

    self_update::uninstall(no_prompt)
}

fn set_default_host_triple(cfg: &Cfg, m: &ArgMatches<'_>) -> Result<()> {
    cfg.set_default_host_triple(m.value_of("host_triple").expect(""))?;
    Ok(())
}

fn set_profile(cfg: &mut Cfg, m: &ArgMatches) -> Result<()> {
    cfg.set_profile(&m.value_of("profile-name").unwrap())?;
    Ok(())
}

fn show_profile(cfg: &Cfg) -> Result<()> {
    println!("{}", cfg.get_profile()?);
    Ok(())
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CompletionCommand {
    Rustup,
    Cargo,
}

static COMPLETIONS: &[(&str, CompletionCommand)] = &[
    ("rustup", CompletionCommand::Rustup),
    ("cargo", CompletionCommand::Cargo),
];

impl CompletionCommand {
    fn variants() -> Vec<&'static str> {
        COMPLETIONS.iter().map(|&(s, _)| s).collect::<Vec<_>>()
    }
}

impl FromStr for CompletionCommand {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match COMPLETIONS
            .iter()
            .find(|&(val, _)| val.eq_ignore_ascii_case(s))
        {
            Some(&(_, cmd)) => Ok(cmd),
            None => {
                let completion_options = COMPLETIONS
                    .iter()
                    .map(|&(v, _)| v)
                    .fold("".to_owned(), |s, v| format!("{}{}, ", s, v));
                Err(format!(
                    "[valid values: {}]",
                    completion_options.trim_end_matches(", ")
                ))
            }
        }
    }
}

impl fmt::Display for CompletionCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match COMPLETIONS.iter().find(|&(_, cmd)| cmd == self) {
            Some(&(val, _)) => write!(f, "{}", val),
            None => unreachable!(),
        }
    }
}

fn output_completion_script(shell: Shell, command: CompletionCommand) -> Result<()> {
    match command {
        CompletionCommand::Rustup => {
            cli().gen_completions_to("rustup", shell, &mut term2::stdout());
        }
        CompletionCommand::Cargo => {
            if let Shell::Zsh = shell {
                writeln!(&mut term2::stdout(), "#compdef cargo")?;
            }

            let script = match shell {
                Shell::Bash => "/etc/bash_completion.d/cargo",
                Shell::Zsh => "/share/zsh/site-functions/_cargo",
                _ => return Err(ErrorKind::UnsupportedCompletionShell(shell, command).into()),
            };

            writeln!(
                &mut term2::stdout(),
                "source $(rustc --print sysroot){}",
                script,
            )?;
        }
    }

    Ok(())
}
