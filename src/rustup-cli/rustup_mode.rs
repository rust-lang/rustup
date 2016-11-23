use clap::{App, Arg, ArgGroup, AppSettings, SubCommand, ArgMatches, Shell};
use common;
use rustup::{Cfg, Toolchain, command};
use rustup::settings::TelemetryMode;
use errors::*;
use rustup_dist::manifest::Component;
use rustup_dist::dist::{TargetTriple, PartialToolchainDesc, PartialTargetTriple};
use rustup_utils::utils;
use self_update;
use std::path::Path;
use std::process::Command;
use std::iter;
use term2;
use std::io::{self, Write};
use help::*;

pub fn main() -> Result<()> {
    try!(::self_update::cleanup_self_updater());

    let ref matches = cli().get_matches();
    let verbose = matches.is_present("verbose");
    let ref cfg = try!(common::set_globals(verbose));

    if try!(maybe_upgrade_data(cfg, matches)) {
        return Ok(())
    }

    try!(cfg.check_metadata_version());

    match matches.subcommand() {
        ("show", Some(_)) => try!(show(cfg)),
        ("install", Some(m)) => try!(update(cfg, m)),
        ("update", Some(m)) => try!(update(cfg, m)),
        ("default", Some(m)) => try!(default_(cfg, m)),
        ("toolchain", Some(c)) => {
            match c.subcommand() {
                ("install", Some(m)) => try!(update(cfg, m)),
                ("list", Some(_)) => try!(common::list_toolchains(cfg)),
                ("link", Some(m)) => try!(toolchain_link(cfg, m)),
                ("uninstall", Some(m)) => try!(toolchain_remove(cfg, m)),
                // Synonyms
                ("update", Some(m)) => try!(update(cfg, m)),
                ("add", Some(m)) => try!(update(cfg, m)),
                ("remove", Some(m)) => try!(toolchain_remove(cfg, m)),
                (_, _) => unreachable!(),
            }
        }
        ("target", Some(c)) => {
            match c.subcommand() {
                ("list", Some(m)) => try!(target_list(cfg, m)),
                ("add", Some(m)) => try!(target_add(cfg, m)),
                ("remove", Some(m)) => try!(target_remove(cfg, m)),
                // Synonyms
                ("install", Some(m)) => try!(target_add(cfg, m)),
                ("uninstall", Some(m)) => try!(target_remove(cfg, m)),
                (_, _) => unreachable!(),
            }
        }
        ("component", Some(c)) => {
            match c.subcommand() {
                ("list", Some(m)) => try!(component_list(cfg, m)),
                ("add", Some(m)) => try!(component_add(cfg, m)),
                ("remove", Some(m)) => try!(component_remove(cfg, m)),
                (_, _) => unreachable!(),
            }
        }
        ("override", Some(c)) => {
            match c.subcommand() {
                ("list", Some(_)) => try!(common::list_overrides(cfg)),
                ("set", Some(m)) => try!(override_add(cfg, m)),
                ("unset", Some(m)) => try!(override_remove(cfg, m)),
                // Synonyms
                ("add", Some(m)) => try!(override_add(cfg, m)),
                ("remove", Some(m)) => try!(override_remove(cfg, m)),
                (_ ,_) => unreachable!(),
            }
        }
        ("run", Some(m)) => try!(run(cfg, m)),
        ("which", Some(m)) => try!(which(cfg, m)),
        ("doc", Some(m)) => try!(doc(cfg, m)),
        ("man", Some(m)) => try!(man(cfg,m)),
        ("self", Some(c)) => {
            match c.subcommand() {
                ("update", Some(_)) => try!(self_update::update()),
                ("uninstall", Some(m)) => try!(self_uninstall(m)),
                (_ ,_) => unreachable!(),
            }
        }
        ("telemetry", Some(c)) => {
            match c.subcommand() {
                ("enable", Some(_)) => try!(set_telemetry(&cfg, TelemetryMode::On)),
                ("disable", Some(_)) => try!(set_telemetry(&cfg, TelemetryMode::Off)),
                ("analyze", Some(_)) => try!(analyze_telemetry(&cfg)),
                (_, _) => unreachable!(),
            }
        }
        ("set", Some(c)) => {
            match c.subcommand() {
                ("default-host", Some(m)) => try!(set_default_host_triple(&cfg, m)),
                (_, _) => unreachable!(),
            }
        }
        ("completions", Some(c)) => {
            if let Some(shell) = c.value_of("shell") {
                cli().gen_completions_to("rustup", shell.parse::<Shell>().unwrap(), &mut io::stdout());
            }
        }
        (_, _) => unreachable!(),
    }

    Ok(())
}

pub fn cli() -> App<'static, 'static> {
    App::new("rustup")
        .version(common::version())
        .about("The Rust toolchain installer")
        .after_help(RUSTUP_HELP)
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(Arg::with_name("verbose")
            .help("Enable verbose output")
            .short("v")
            .long("verbose"))
        .subcommand(SubCommand::with_name("show")
            .about("Show the active and installed toolchains")
            .after_help(SHOW_HELP))
        .subcommand(SubCommand::with_name("install")
            .about("Update Rust toolchains")
            .after_help(TOOLCHAIN_INSTALL_HELP)
            .setting(AppSettings::Hidden) // synonym for 'toolchain install'
            .arg(Arg::with_name("toolchain")
                .required(true)))
        .subcommand(SubCommand::with_name("update")
            .about("Update Rust toolchains")
            .after_help(UPDATE_HELP)
            .arg(Arg::with_name("toolchain")
                .required(false))
            .arg(Arg::with_name("no-self-update")
                .help("Don't perform self update when running the `rustup` command")
                .long("no-self-update")
                .takes_value(false)
                .hidden(true)))
        .subcommand(SubCommand::with_name("default")
            .about("Set the default toolchain")
            .after_help(DEFAULT_HELP)
            .arg(Arg::with_name("toolchain")
                .required(true)))
        .subcommand(SubCommand::with_name("toolchain")
            .about("Modify or query the installed toolchains")
            .after_help(TOOLCHAIN_HELP)
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("list")
                .about("List installed toolchains"))
            .subcommand(SubCommand::with_name("install")
                .about("Install or update a given toolchain")
                .arg(Arg::with_name("toolchain")
                .required(true)))
            .subcommand(SubCommand::with_name("uninstall")
                .about("Uninstall a toolchain")
                .arg(Arg::with_name("toolchain")
                     .required(true)))
            .subcommand(SubCommand::with_name("link")
                .about("Create a custom toolchain by symlinking to a directory")
                .arg(Arg::with_name("toolchain")
                    .required(true))
                .arg(Arg::with_name("path")
                    .required(true)))
            .subcommand(SubCommand::with_name("update")
                .setting(AppSettings::Hidden) // synonym for 'install'
                .arg(Arg::with_name("toolchain")
                .required(true)))
            .subcommand(SubCommand::with_name("add")
                .setting(AppSettings::Hidden) // synonym for 'install'
                .arg(Arg::with_name("toolchain")
                     .required(true)))
            .subcommand(SubCommand::with_name("remove")
                .setting(AppSettings::Hidden) // synonym for 'uninstall'
                .arg(Arg::with_name("toolchain")
                     .required(true))))
        .subcommand(SubCommand::with_name("target")
            .about("Modify a toolchain's supported targets")
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("list")
                .about("List installed and available targets")
                .arg(Arg::with_name("toolchain")
                    .long("toolchain")
                    .takes_value(true)))
            .subcommand(SubCommand::with_name("add")
                .about("Add a target to a Rust toolchain")
                .arg(Arg::with_name("target")
                    .required(true))
                .arg(Arg::with_name("toolchain")
                    .long("toolchain")
                    .takes_value(true)))
            .subcommand(SubCommand::with_name("remove")
                .about("Remove a target  from a Rust toolchain")
                .arg(Arg::with_name("target")
                    .required(true))
                .arg(Arg::with_name("toolchain")
                    .long("toolchain")
                    .takes_value(true)))
            .subcommand(SubCommand::with_name("install")
                .setting(AppSettings::Hidden) // synonym for 'add'
                .arg(Arg::with_name("target")
                    .required(true))
                .arg(Arg::with_name("toolchain")
                    .long("toolchain")
                    .takes_value(true)))
            .subcommand(SubCommand::with_name("uninstall")
                .setting(AppSettings::Hidden) // synonym for 'remove'
                .arg(Arg::with_name("target")
                    .required(true))
                .arg(Arg::with_name("toolchain")
                    .long("toolchain")
                    .takes_value(true))))
        .subcommand(SubCommand::with_name("component")
            .about("Modify a toolchain's installed components")
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("list")
                .about("List installed and available components")
                .arg(Arg::with_name("toolchain")
                    .long("toolchain")
                    .takes_value(true)))
            .subcommand(SubCommand::with_name("add")
                .about("Add a component to a Rust toolchain")
                .arg(Arg::with_name("component")
                    .required(true))
                .arg(Arg::with_name("toolchain")
                    .long("toolchain")
                    .takes_value(true))
                .arg(Arg::with_name("target")
                    .long("target")
                    .takes_value(true)))
            .subcommand(SubCommand::with_name("remove")
                .about("Remove a component from a Rust toolchain")
                .arg(Arg::with_name("component")
                    .required(true))
                .arg(Arg::with_name("toolchain")
                    .long("toolchain")
                    .takes_value(true))
                .arg(Arg::with_name("target")
                    .long("target")
                    .takes_value(true))))
        .subcommand(SubCommand::with_name("override")
            .about("Modify directory toolchain overrides")
            .after_help(OVERRIDE_HELP)
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("list")
                .about("List directory toolchain overrides"))
            .subcommand(SubCommand::with_name("set")
                .about("Set the override toolchain for a directory")
                .arg(Arg::with_name("toolchain")
                     .required(true)))
            .subcommand(SubCommand::with_name("unset")
                .about("Remove the override toolchain for a directory")
                .after_help(OVERRIDE_UNSET_HELP)
                .arg(Arg::with_name("path")
                    .long("path")
                    .takes_value(true)
                    .help("Path to the directory"))
                .arg(Arg::with_name("nonexistent")
                    .long("nonexistent")
                    .takes_value(false)
                    .help("Remove override toolchain for all nonexistent directories")))
            .subcommand(SubCommand::with_name("add")
                .setting(AppSettings::Hidden) // synonym for 'set'
                .arg(Arg::with_name("toolchain")
                     .required(true)))
            .subcommand(SubCommand::with_name("remove")
                .setting(AppSettings::Hidden) // synonym for 'unset'
                .about("Remove the override toolchain for a directory")
                .arg(Arg::with_name("path")
                    .long("path")
                    .takes_value(true))
                .arg(Arg::with_name("nonexistent")
                    .long("nonexistent")
                    .takes_value(false)
                    .help("Remove override toolchain for all nonexistent directories"))))
        .subcommand(SubCommand::with_name("run")
            .about("Run a command with an environment configured for a given toolchain")
            .after_help(RUN_HELP)
            .setting(AppSettings::TrailingVarArg)
            .arg(Arg::with_name("toolchain")
                .required(true))
            .arg(Arg::with_name("command")
                .required(true).multiple(true).use_delimiter(false)))
        .subcommand(SubCommand::with_name("which")
            .about("Display which binary will be run for a given command")
            .arg(Arg::with_name("command")
                .required(true)))
        .subcommand(SubCommand::with_name("doc")
            .about("Open the documentation for the current toolchain")
            .after_help(DOC_HELP)
            .arg(Arg::with_name("book")
                 .long("book")
                 .help("The Rust Programming Language book"))
            .arg(Arg::with_name("std")
                 .long("std")
                 .help("Standard library API documentation"))
            .group(ArgGroup::with_name("page")
                 .args(&["book", "std"])))
        .subcommand(SubCommand::with_name("man")
                    .about("View the man page for a given command")
                    .arg(Arg::with_name("command")
                         .required(true))
                    .arg(Arg::with_name("toolchain")
                         .long("toolchain")
                         .takes_value(true)))
        .subcommand(SubCommand::with_name("self")
            .about("Modify the rustup installation")
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("update")
                .about("Download and install updates to rustup"))
            .subcommand(SubCommand::with_name("uninstall")
                .about("Uninstall rustup.")
                .arg(Arg::with_name("no-prompt")
                     .short("y")))
            .subcommand(SubCommand::with_name("upgrade-data")
                .about("Upgrade the internal data format.")))
        .subcommand(SubCommand::with_name("telemetry")
            .about("rustup telemetry commands")
            .setting(AppSettings::Hidden)
            .setting(AppSettings::VersionlessSubcommands)
            .setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("enable")
                            .about("Enable rustup telemetry"))
            .subcommand(SubCommand::with_name("disable")
                            .about("Disable rustup telemetry"))
            .subcommand(SubCommand::with_name("analyze")
                            .about("Analyze stored telemetry")))
        .subcommand(SubCommand::with_name("set")
            .about("Alter rustup settings")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(SubCommand::with_name("default-host")
                .about("The triple used to identify toolchains when not specified")
                .arg(Arg::with_name("host_triple")
                    .required(true))))
        .subcommand(SubCommand::with_name("completions")
            .about("Generate completion scripts for your shell")
            .after_help(COMPLETIONS_HELP)
            .setting(AppSettings::ArgRequiredElseHelp)
            .arg(Arg::with_name("shell")
                .possible_values(&Shell::variants())))
}

fn maybe_upgrade_data(cfg: &Cfg, m: &ArgMatches) -> Result<bool> {
    match m.subcommand() {
        ("self", Some(c)) => {
            match c.subcommand() {
                ("upgrade-data", Some(_)) => {
                    try!(cfg.upgrade_data());
                    Ok(true)
                }
                _ => Ok(false),
            }
        }
        _ => Ok(false)
    }
}

fn update_bare_triple_check(cfg: &Cfg, name: &str) -> Result<()> {
    if let Some(triple) = PartialTargetTriple::from_str(name) {
        warn!("(partial) target triple specified instead of toolchain name");
        let installed_toolchains = try!(cfg.list_toolchains());
        let default = try!(cfg.find_default());
        let default_name = default.map(|t| t.name().to_string())
                           .unwrap_or("".into());
        let mut candidates = vec![];
        for t in installed_toolchains {
            if t == default_name {
                continue;
            }
            if let Ok(desc) = PartialToolchainDesc::from_str(&t) {
                fn triple_comp_eq(given: &String, from_desc: Option<&String>) -> bool {
                    from_desc.map_or(false, |s| *s == *given)
                }

                let triple_matches =
                    triple.arch.as_ref().map_or(true, |s| triple_comp_eq(s, desc.target.arch.as_ref()))
                    && triple.os.as_ref().map_or(true, |s| triple_comp_eq(s, desc.target.os.as_ref()))
                    && triple.env.as_ref().map_or(true, |s| triple_comp_eq(s, desc.target.env.as_ref()));
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
                for n in candidates.iter() {
                    println!("{}", n);
                }
                println!("");
            }
        }
        return Err(ErrorKind::ToolchainNotInstalled(name.to_string()).into());
    }
    Ok(())
}

fn default_bare_triple_check(cfg: &Cfg, name: &str) -> Result<()> {
    if let Some(triple) = PartialTargetTriple::from_str(name) {
        warn!("(partial) target triple specified instead of toolchain name");
        let default = try!(cfg.find_default());
        let default_name = default.map(|t| t.name().to_string())
                           .unwrap_or("".into());
        if let Ok(mut desc) = PartialToolchainDesc::from_str(&default_name) {
            desc.target = triple;
            let maybe_toolchain = format!("{}", desc);
            let ref toolchain = try!(cfg.get_toolchain(maybe_toolchain.as_ref(), false));
            if toolchain.name() == default_name {
                warn!("(partial) triple '{}' resolves to a toolchain that is already default", name);
            } else {
                println!("\nyou may use the following toolchain: {}\n", toolchain.name());
            }
            return Err(ErrorKind::ToolchainNotInstalled(name.to_string()).into());
        }
    }
    Ok(())
}

fn default_(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    try!(default_bare_triple_check(cfg, toolchain));
    let ref toolchain = try!(cfg.get_toolchain(toolchain, false));

    let status = if !toolchain.is_custom() {
        Some(try!(toolchain.install_from_dist_if_not_installed()))
    } else if !toolchain.exists() {
        return Err(ErrorKind::ToolchainNotInstalled(toolchain.name().to_string()).into());
    } else {
        None
    };

    try!(toolchain.make_default());

    if let Some(status) = status {
        println!("");
        try!(common::show_channel_update(cfg, toolchain.name(), Ok(status)));
    }

    Ok(())
}

fn update(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    if let Some(name) = m.value_of("toolchain") {
        try!(update_bare_triple_check(cfg, name));
        let toolchain = try!(cfg.get_toolchain(name, false));

        let status = if !toolchain.is_custom() {
            Some(try!(toolchain.install_from_dist()))
        } else if !toolchain.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(toolchain.name().to_string()).into());
        } else {
            None
        };

        if let Some(status) = status {
            println!("");
            try!(common::show_channel_update(cfg, toolchain.name(), Ok(status)));
        }
    } else {
        try!(common::update_all_channels(cfg, !m.is_present("no-self-update") && !self_update::NEVER_SELF_UPDATE));
    }

    Ok(())
}

fn run(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let args = m.values_of("command").unwrap();
    let args: Vec<_> = args.collect();
    let cmd = try!(cfg.create_command_for_toolchain(toolchain, args[0]));

    Ok(try!(command::run_command_for_dir(cmd, args[0], &args[1..], &cfg)))
}

fn which(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let binary = m.value_of("command").expect("");

    let binary_path = try!(cfg.which_binary(&try!(utils::current_dir()), binary))
                          .expect("binary not found");

    try!(utils::assert_is_file(&binary_path));

    println!("{}", binary_path.display());

    Ok(())
}

fn show(cfg: &Cfg) -> Result<()> {
    // Print host triple
    {
        let mut t = term2::stdout();
        let _ = t.attr(term2::Attr::Bold);
        let _ = write!(t, "Default host: ");
        let _ = t.reset();
        println!("{}", try!(cfg.get_default_host_triple()));
        println!("");
    }

    let ref cwd = try!(utils::current_dir());
    let installed_toolchains = try!(cfg.list_toolchains());
    let active_toolchain = try!(cfg.find_override_toolchain_or_default(cwd));
    let active_targets = if let Some((ref t, _)) = active_toolchain {
        match t.list_components() {
            Ok(cs_vec) => cs_vec
                .into_iter()
                .filter(|c| c.component.pkg == "rust-std")
                .filter(|c| c.installed)
                .collect(),
            Err(_) => vec![]
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
        show_active_toolchain
    ].iter().filter(|x| **x).count() > 1;

    if show_installed_toolchains {
        if show_headers { print_header("installed toolchains") }
        let default = try!(cfg.find_default());
        let default_name = default.map(|t| t.name().to_string())
                           .unwrap_or("".into());
        for t in installed_toolchains {
            if default_name == t {
                println!("{} (default)", t);
            } else {
                println!("{}", t);
            }
        }
        if show_headers { println!("") };
    }

    if show_active_targets {
        if show_headers {
            print_header("installed targets for active toolchain");
        }
        for t in active_targets {
            println!("{}", t.component.target.as_ref().expect("rust-std should have a target"));
        }
        if show_headers { println!("") };
    }

    if show_active_toolchain {
        if show_headers { print_header("active toolchain") }

        match active_toolchain {
            Some((ref toolchain, Some(ref reason))) => {
                println!("{} ({})", toolchain.name(), reason);
                println!("{}", common::rustc_version(toolchain));
            }
            Some((ref toolchain, None)) => {
                println!("{} (default)", toolchain.name());
                println!("{}", common::rustc_version(toolchain));
            }
            None => {
                println!("no active toolchain");
            }
        }

        if show_headers { println!("") };
    }

    fn print_header(s: &str) {
        let mut t = term2::stdout();
        let _ = t.attr(term2::Attr::Bold);
        let _ = writeln!(t, "{}", s);
        let _ = writeln!(t, "{}", iter::repeat("-").take(s.len()).collect::<String>());
        let _ = writeln!(t, "");
        let _ = t.reset();
    }

    Ok(())
}

fn target_list(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(explicit_or_dir_toolchain(cfg, m));

    common::list_targets(&toolchain)
}

fn target_add(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(explicit_or_dir_toolchain(cfg, m));
    let target = m.value_of("target").expect("");
    let new_component = Component {
        pkg: "rust-std".to_string(),
        target: Some(TargetTriple::from_str(target)),
    };

    Ok(try!(toolchain.add_component(new_component)))
}

fn target_remove(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(explicit_or_dir_toolchain(cfg, m));
    let target = m.value_of("target").expect("");
    let new_component = Component {
        pkg: "rust-std".to_string(),
        target: Some(TargetTriple::from_str(target)),
    };

    Ok(try!(toolchain.remove_component(new_component)))
}

fn component_list(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(explicit_or_dir_toolchain(cfg, m));

    common::list_components(&toolchain)
}

fn component_add(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(explicit_or_dir_toolchain(cfg, m));
    let component = m.value_of("component").expect("");
    let target = m.value_of("target").map(TargetTriple::from_str).or_else(|| {
        toolchain.desc().as_ref().ok().map(|desc| desc.target.clone())
    });

    let new_component = Component {
        pkg: component.to_string(),
        target: target,
    };

    Ok(try!(toolchain.add_component(new_component)))
}

fn component_remove(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(explicit_or_dir_toolchain(cfg, m));
    let component = m.value_of("component").expect("");
    let target = m.value_of("target").map(TargetTriple::from_str).or_else(|| {
        toolchain.desc().as_ref().ok().map(|desc| desc.target.clone())
    });

    let new_component = Component {
        pkg: component.to_string(),
        target: target,
    };

    Ok(try!(toolchain.remove_component(new_component)))
}

fn explicit_or_dir_toolchain<'a>(cfg: &'a Cfg, m: &ArgMatches) -> Result<Toolchain<'a>> {
    let toolchain = m.value_of("toolchain");
    if let Some(toolchain) = toolchain {
        let toolchain = try!(cfg.get_toolchain(toolchain, false));
        return Ok(toolchain);
    }

    let ref cwd = try!(utils::current_dir());
    let (toolchain, _) = try!(cfg.toolchain_for_dir(cwd));

    Ok(toolchain)
}

fn toolchain_link(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let ref path = m.value_of("path").expect("");
    let toolchain = try!(cfg.get_toolchain(toolchain, true));

    Ok(try!(toolchain.install_from_dir(Path::new(path), true)))
}

fn toolchain_remove(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let toolchain = try!(cfg.get_toolchain(toolchain, false));

    Ok(try!(toolchain.remove()))
}

fn override_add(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let toolchain = try!(cfg.get_toolchain(toolchain, false));

    let status = if !toolchain.is_custom() {
        Some(try!(toolchain.install_from_dist_if_not_installed()))
    } else if !toolchain.exists() {
        return Err(ErrorKind::ToolchainNotInstalled(toolchain.name().to_string()).into());
    } else {
        None
    };

    try!(toolchain.make_override(&try!(utils::current_dir())));

    if let Some(status) = status {
        println!("");
        try!(common::show_channel_update(cfg, toolchain.name(), Ok(status)));
    }

    Ok(())
}

fn override_remove(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let paths = if m.is_present("nonexistent") {
        let list: Vec<_> = try!(cfg.settings_file.with(|s| Ok(s.overrides.iter().filter_map(|(k, _)|
            if Path::new(k).is_dir() {
                None
            } else {
                Some(k.clone())
            }
        ).collect())));
        if list.is_empty() {
            info!("no nonexistent paths detected");
        }
        list
    } else {
        if m.is_present("path") {
            vec![m.value_of("path").unwrap().to_string()]
        } else {
            vec![try!(utils::current_dir()).to_str().unwrap().to_string()]
        }
    };

    for path in paths {
        if try!(cfg.settings_file.with_mut(|s| {
            Ok(s.remove_override(&Path::new(&path), cfg.notify_handler.as_ref()))
        })) {
            info!("override toolchain for '{}' removed", path);
        } else {
            info!("no override toolchain for '{}'", path);
            if !m.is_present("path") && !m.is_present("nonexistent") {
                info!("you may use `--path <path>` option to remove override toolchain \
                       for a specific path");
            }
        }
    }
    Ok(())
}

fn doc(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let doc_url = if m.is_present("book") {
        "book/index.html"
    } else if m.is_present("std") {
        "std/index.html"
    } else {
        "index.html"
    };

    Ok(try!(cfg.open_docs_for_dir(&try!(utils::current_dir()), doc_url)))
}

fn man(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let manpage = m.value_of("command").expect("");
    let toolchain = try!(explicit_or_dir_toolchain(cfg, m));
    let mut man_path = toolchain.path().to_path_buf();
    man_path.push("share");
    man_path.push("man");
    man_path.push("man1");
    man_path.push(manpage.to_owned() + ".1");
    try!(utils::assert_is_file(&man_path));
    Command::new("man")
        .arg(man_path)
        .status()
        .expect("failed to open man page");
    Ok(())
}

fn self_uninstall(m: &ArgMatches) -> Result<()> {
    let no_prompt = m.is_present("no-prompt");

    self_update::uninstall(no_prompt)
}

fn set_telemetry(cfg: &Cfg, t: TelemetryMode) -> Result<()> {
    match t {
        TelemetryMode::On => Ok(try!(cfg.set_telemetry(true))),
        TelemetryMode::Off => Ok(try!(cfg.set_telemetry(false))),
    }
}

fn analyze_telemetry(cfg: &Cfg) -> Result<()> {
    let analysis = try!(cfg.analyze_telemetry());
    common::show_telemetry(analysis)
}

fn set_default_host_triple(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    try!(cfg.set_default_host_triple(m.value_of("host_triple").expect("")));
    Ok(())
}
