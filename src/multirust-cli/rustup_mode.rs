use clap::{App, Arg, AppSettings, SubCommand, ArgMatches};
use common;
use multirust::{Result, Cfg, Error};
use multirust_dist::manifest::Component;
use multirust_utils::utils;
use self_update;
use std::path::Path;

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
        ("default", Some(m)) => try!(default_(cfg, m)),
        ("update", Some(m)) => try!(update(cfg, m)),
        ("run", Some(m)) => try!(run(cfg, m)),
        ("target", Some(c)) => {
            match c.subcommand() {
                ("list", Some(_)) => try!(target_list(cfg)),
                ("add", Some(m)) => try!(target_add(cfg, m)),
                ("remove", Some(m)) => try!(target_remove(cfg, m)),
                (_, _) => unreachable!(),
            }
        }
        ("toolchain", Some(c)) => {
            match c.subcommand() {
                ("list", Some(_)) => try!(common::list_toolchains(cfg)),
                ("link", Some(m)) => try!(toolchain_link(cfg, m)),
                ("remove", Some(m)) => try!(toolchain_remove(cfg, m)),
                (_, _) => unreachable!(),
            }
        }
        ("override", Some(c)) => {
            match c.subcommand() {
                ("list", Some(_)) => try!(common::list_overrides(cfg)),
                ("add", Some(m)) => try!(override_add(cfg, m)),
                ("remove", Some(_)) => try!(override_remove(cfg)),
                (_ ,_) => unreachable!(),
            }
        }
        ("doc", Some(m)) => try!(doc(cfg, m)),
        ("self", Some(c)) => {
            match c.subcommand() {
                ("update", Some(_)) => try!(self_update::update()),
                ("uninstall", Some(m)) => try!(self_uninstall(m)),
                (_ ,_) => unreachable!(),
            }
        }
        (_, _) => {
            try!(update_all_channels(cfg, &matches));
        }
    }

    Ok(())
}

pub fn cli() -> App<'static, 'static> {
    App::new("rustup")
        .version("0.0.5")
        .author("Diggory Blake")
        .about("The Rust toolchain installer")
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::DeriveDisplayOrder)
        .arg(Arg::with_name("verbose")
            .help("Enable verbose output")
            .short("v")
            .long("verbose"))
        .arg(Arg::with_name("no-self-update")
            .help("Don't perform self update when running the `rustup` command")
            .long("no-self-update")
            .takes_value(false)
            .hidden(true))
        .subcommand(SubCommand::with_name("default")
            .about("Set the default toolchain")
            .arg(Arg::with_name("toolchain")
                .required(true)))
        .subcommand(SubCommand::with_name("update")
            .about("Install or update a toolchain from a Rust distribution channel")
            .arg(Arg::with_name("toolchain")
                .required(false)))
        .subcommand(SubCommand::with_name("run")
            .about("Run a command with an environment configured for a given toolchain")
            .setting(AppSettings::TrailingVarArg)
            .arg(Arg::with_name("toolchain")
                .required(true))
            .arg(Arg::with_name("command")
                .required(true).multiple(true)))
        .subcommand(SubCommand::with_name("target")
            .about("Modify a toolchain's supported targets")
            .subcommand(SubCommand::with_name("list")
                .about("List installed and available targets"))
            .subcommand(SubCommand::with_name("add")
                .about("Add a target to a Rust toolchain")
                .arg(Arg::with_name("target")
                    .required(true)))
            .subcommand(SubCommand::with_name("remove")
                .about("Remove a target  from a Rust toolchain")
                .arg(Arg::with_name("target")
                    .required(true))
                .arg(Arg::with_name("toolchain"))))
        .subcommand(SubCommand::with_name("toolchain")
            .about("Modify the installed toolchains")
            .subcommand(SubCommand::with_name("list")
                .about("List installed toolchains"))
            .subcommand(SubCommand::with_name("link")
                .about("Create a custom toolchain by symlinking to a directory")
                .arg(Arg::with_name("toolchain")
                    .required(true))
                .arg(Arg::with_name("path")
                    .required(true)))
            .subcommand(SubCommand::with_name("remove")
                .about("Uninstall a toolchain")
                .arg(Arg::with_name("toolchain")
                     .required(true))))
        .subcommand(SubCommand::with_name("override")
            .about("Modify directory toolchain overrides")
            .subcommand(SubCommand::with_name("list")
                .about("List directory toolchain overrides"))
            .subcommand(SubCommand::with_name("add")
                .about("Set the override toolchain for a directory")
                .arg(Arg::with_name("toolchain")
                     .required(true)))
            .subcommand(SubCommand::with_name("remove")
                .about("Remove the override toolchain for a directory")))
        .subcommand(SubCommand::with_name("self")
            .about("Modify the rustup installation")
            .subcommand(SubCommand::with_name("update")
                .about("Downloadand and install updates to rustup"))
            .subcommand(SubCommand::with_name("uninstall")
                .about("Uninstall rustup.")
                .arg(Arg::with_name("no-prompt")
                     .short("y")))
            .subcommand(SubCommand::with_name("upgrade-data")
                .about("Upgrade the internal data format.")))
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

fn update_all_channels(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    common::update_all_channels(cfg, !m.is_present("no-self-update"))
}

fn default_(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let ref toolchain = try!(cfg.get_toolchain(toolchain, false));

    let status = if !toolchain.is_custom() {
        Some(try!(toolchain.install_from_dist_if_not_installed()))
    } else if !toolchain.exists() {
        return Err(Error::ToolchainNotInstalled(toolchain.name().to_string()));
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
    let toolchain = if let Some(name) = m.value_of("toolchain") {
        try!(cfg.get_toolchain(name, false))
    } else {
        let ref cwd = try!(utils::current_dir());
        let (toolchain, _) = try!(cfg.toolchain_for_dir(cwd));
        toolchain
    };

    let status = if !toolchain.is_custom() {
        Some(try!(toolchain.install_from_dist()))
    } else if !toolchain.exists() {
        return Err(Error::ToolchainNotInstalled(toolchain.name().to_string()));
    } else {
        None
    };

    if let Some(status) = status {
        println!("");
        try!(common::show_channel_update(cfg, toolchain.name(), Ok(status)));
    }

    Ok(())
}

fn run(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let ref toolchain = try!(cfg.get_toolchain(toolchain, false));
    let args = m.values_of("command").unwrap();
    let args: Vec<_> = args.collect();
    let cmd = try!(toolchain.create_command(args[0]));

    common::run_inner(cmd, &args)
}

fn target_list(cfg: &Cfg) -> Result<()> {
    let ref cwd = try!(utils::current_dir());
    let (toolchain, _) = try!(cfg.toolchain_for_dir(cwd));

    common::list_targets(&toolchain)
}

fn target_add(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let target = m.value_of("target").expect("");
    let ref cwd = try!(utils::current_dir());
    let (toolchain, _) = try!(cfg.toolchain_for_dir(cwd));
    let new_component = Component {
        pkg: "rust-std".to_string(),
        target: target.to_string(),
    };

    toolchain.add_component(new_component)
}

fn target_remove(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let target = m.value_of("target").expect("");
    let ref cwd = try!(utils::current_dir());
    let (toolchain, _) = try!(cfg.toolchain_for_dir(cwd));
    let new_component = Component {
        pkg: "rust-std".to_string(),
        target: target.to_string(),
    };

    toolchain.remove_component(new_component)
}

fn toolchain_link(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let ref path = m.value_of("path").expect("");
    let toolchain = try!(cfg.get_toolchain(toolchain, true));

    toolchain.install_from_dir(Path::new(path), true)
}

fn toolchain_remove(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let toolchain = try!(cfg.get_toolchain(toolchain, false));

    toolchain.remove()
}

fn override_add(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let ref toolchain = m.value_of("toolchain").expect("");
    let toolchain = try!(cfg.get_toolchain(toolchain, false));

    let status = if !toolchain.is_custom() {
        Some(try!(toolchain.install_from_dist_if_not_installed()))
    } else if !toolchain.exists() {
        return Err(Error::ToolchainNotInstalled(toolchain.name().to_string()));
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

fn override_remove(cfg: &Cfg) -> Result<()> {
    let ref path = try!(utils::current_dir());

    let ref override_db = cfg.override_db;
    let notify_handler = cfg.notify_handler.as_ref();

    if try!(override_db.find(path, notify_handler)).is_none() {
        info!("no override toolchain for '{}'", path.display());
        return Ok(());
    }

    try!(override_db.remove(path, &cfg.temp_cfg, notify_handler));
    info!("override toolchain for '{}' removed", path.display());
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

    cfg.open_docs_for_dir(&try!(utils::current_dir()), doc_url)
}

fn self_uninstall(m: &ArgMatches) -> Result<()> {
    let no_prompt = m.is_present("no-prompt");

    self_update::uninstall(no_prompt)
}
