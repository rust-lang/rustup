use clap::ArgMatches;
use cli;
use common::{self, confirm, set_globals,
             show_channel_update, show_tool_versions,
             update_all_channels};
use errors::*;
use rustup::{Cfg, Notification, command, utils, Toolchain};
use rustup_dist::manifest::Component;
use rustup_dist::dist::TargetTriple;
use self_update;
use std::env;
use std::path::{Path, PathBuf};
use job;

pub fn main() -> Result<()> {
    try!(::self_update::cleanup_self_updater());

    let need_metadata = try!(command_requires_metadata());
    if need_metadata {
        let cfg = try!(Cfg::from_env(shared_ntfy!(move |_: Notification| { })));
        try!(cfg.check_metadata_version());
    }

    let app_matches = cli::get().get_matches();
    let verbose = app_matches.is_present("verbose");
    let cfg = try!(set_globals(verbose));

    // On windows, put the process and its children in a job object
    // so they are all killed together.
    //
    // We don't do this for self-uninstall and upgrade because
    // it interferes with the fragile self-deletion code.
    maybe_setup_winjob(&app_matches);

    warn!("'multirust' is being renamed to 'rustup'. this command is deprecated. use 'rustup' instead");

    match app_matches.subcommand() {
        ("update", Some(m)) => update(&cfg, m),
        ("default", Some(m)) => default_(&cfg, m),
        ("override", Some(m)) => override_(&cfg, m),
        ("show-default", Some(_)) => show_default(&cfg),
        ("show-override", Some(_)) => show_override(&cfg),
        ("list-overrides", Some(_)) => common::list_overrides(&cfg),
        ("list-toolchains", Some(_)) => common::list_toolchains(&cfg),
        ("remove-override", Some(m)) => remove_override(&cfg, m),
        ("remove-toolchain", Some(m)) => remove_toolchain_args(&cfg, m),
        ("list-targets", Some(m)) => list_targets(&cfg, m),
        ("add-target", Some(m)) => add_target(&cfg, m),
        ("remove-target", Some(m)) => remove_target(&cfg, m),
        ("run", Some(m)) => run(&cfg, m),
        ("proxy", Some(m)) => proxy(&cfg, m),
        ("upgrade-data", Some(_)) => Ok(try!(cfg.upgrade_data().map(|_| ()))),
        ("delete-data", Some(m)) => delete_data(&cfg, m),
        ("self", Some(c)) => {
            match c.subcommand() {
                ("uninstall", Some(m)) => self_uninstall(m),
                ("update", Some(_)) => self_update(),
                _ => Ok(()),
            }
        }
        ("which", Some(m)) => which(&cfg, m),
        ("doc", Some(m)) => doc(&cfg, m),
        _ => {
            unreachable!()
        }
    }
}

fn maybe_setup_winjob(m: &ArgMatches) {
    match m.subcommand() {
        ("self", _) => { }
        (_, _) => {
            job::setup();
        }
    }
}

fn run(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(get_toolchain(cfg, m, false));
    let args = m.values_of("command").unwrap();
    let args: Vec<_> = args.collect();
    let cmd = try!(toolchain.create_command(args[0]));
    Ok(try!(command::run_command_for_dir(cmd, &args, &cfg)))
}

fn proxy(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let args = m.values_of("command").unwrap();
    let args: Vec<_> = args.collect();
    let cmd = try!(cfg.create_command_for_dir(&try!(utils::current_dir()), args[0]));
    Ok(try!(command::run_command_for_dir(cmd, &args, &cfg)))
}

fn command_requires_metadata() -> Result<bool> {
    let args = env::args().collect::<Vec<_>>();
    let arg1 = args.get(1).map(|s| &**s);
    let arg2 = args.get(2).map(|s| &**s);

    match (arg1, arg2) {
        (Some("upgrade-data"), _) |
        (Some("delete-data"), _) |
        (Some("self"), Some("install")) => {
            Ok(false)
        }
        (None, None) => {
            // Running multirust in its self-install mode
            Ok(false)
        }
        (_, _) => {
            Ok(true)
        }
    }
}

fn self_uninstall(m: &ArgMatches) -> Result<()> {
    let no_prompt = m.is_present("no-prompt");
    self_update::uninstall(no_prompt)
}

fn self_update() -> Result<()> {
    self_update::update()
}

fn get_toolchain<'a>(cfg: &'a Cfg, m: &ArgMatches, create_parent: bool) -> Result<Toolchain<'a>> {
    Ok(try!(cfg.get_toolchain(m.value_of("toolchain").unwrap(), create_parent)))
}

fn remove_toolchain_args(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    Ok(try!(try!(get_toolchain(cfg, m, false)).remove()))
}

fn default_(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(get_toolchain(cfg, m, true));
    let status = if !try!(common_install_args(&toolchain, m)) {
        if !toolchain.is_custom() {
            Some(try!(toolchain.install_from_dist_if_not_installed()))
        } else if !toolchain.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(toolchain.name().to_string()).unchained());
        } else {
            None
        }
    } else {
        None
    };

    try!(toolchain.make_default());

    if let Some(status) = status {
        println!("");
        try!(show_channel_update(cfg, toolchain.name(), Ok(status)));
    }

    Ok(())
}

fn update(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    if let Some(name) = m.value_of("toolchain") {
        let toolchain = try!(cfg.get_toolchain(name, true));
        let status = if !try!(common_install_args(&toolchain, m)) {
            if !toolchain.is_custom() {
                Some(try!(toolchain.install_from_dist()))
            } else if !toolchain.exists() {
                return Err(ErrorKind::ToolchainNotInstalled(toolchain.name().to_string()).unchained());
            } else {
                None
            }
        } else {
            None
        };

        if let Some(status) = status {
            println!("");
            try!(show_channel_update(cfg, name, Ok(status)));
        }
    } else {
        try!(update_all_channels(cfg, false))
    }
    Ok(())
}

fn override_(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(get_toolchain(cfg, m, true));
    let status = if !try!(common_install_args(&toolchain, m)) {
        if !toolchain.is_custom() {
            Some(try!(toolchain.install_from_dist_if_not_installed()))
        } else {
            None
        }
    } else {
        None
    };

    try!(toolchain.make_override(&try!(utils::current_dir())));

    if let Some(status) = status {
        println!("");
        try!(show_channel_update(cfg, toolchain.name(), Ok(status)));
    }

    Ok(())
}

fn common_install_args(toolchain: &Toolchain, m: &ArgMatches) -> Result<bool> {

    if let Some(installers) = m.values_of("installer") {
        let is: Vec<_> = installers.map(|i| i.as_ref()).collect();
        try!(toolchain.install_from_installers(&*is));
    } else if let Some(path) = m.value_of("copy-local") {
        try!(toolchain.install_from_dir(Path::new(path), false));
    } else if let Some(path) = m.value_of("link-local") {
        try!(toolchain.install_from_dir(Path::new(path), true));
    } else {
        return Ok(false);
    }
    Ok(true)
}

fn doc_url(m: &ArgMatches) -> &'static str {
    if m.is_present("book") {
        "book/index.html"
    } else if m.is_present("std") {
        "std/index.html"
    } else {
        "index.html"
    }
}

fn doc(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    Ok(try!(cfg.open_docs_for_dir(&try!(utils::current_dir()), doc_url(m))))
}

fn which(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let binary = m.value_of("binary").unwrap();

    let binary_path = try!(cfg.which_binary(&try!(utils::current_dir()), binary))
                          .expect("binary not found");

    try!(utils::assert_is_file(&binary_path));

    println!("{}", binary_path.display());
    Ok(())
}

fn delete_data(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let msg =
r"
This will delete all toolchains, overrides, aliases, and other
multirust data associated with this user.

Continue? (y/N)";

    if !m.is_present("no-prompt") && !try!(confirm(msg, false)) {
        info!("aborting delete-data");
        return Ok(());
    }

    try!(cfg.delete_data());

    info!("deleted directory '{}'", cfg.multirust_dir.display());

    Ok(())
}

fn remove_override(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let cwd = try!(utils::current_dir());
    let ref path = m.value_of("override")
        .map(|p| PathBuf::from(p)).unwrap_or(cwd);

    if try!(cfg.override_db.find(path, cfg.notify_handler.as_ref())).is_none() {
        info!("no override toolchain for '{}'", path.display());
        return Ok(());
    }

    try!(cfg.override_db.remove(path,
                                &cfg.temp_cfg,
                                cfg.notify_handler.as_ref()));
    info!("override toolchain for '{}' removed", path.display());
    Ok(())
}

fn show_default(cfg: &Cfg) -> Result<()> {
    if let Some(toolchain) = try!(cfg.find_default()) {
        println!("default toolchain: {}", toolchain.name());
        println!("default location: {}", toolchain.path().display());

        println!("");
        try!(show_tool_versions(&toolchain));
        println!("");
    } else {
        println!("no default toolchain configured. run `multirust help default`");
        println!("");
    }

    Ok(())
}

fn show_override(cfg: &Cfg) -> Result<()> {
    if let Some((toolchain, reason)) = try!(cfg.find_override(&try!(utils::current_dir()))) {
        println!("override toolchain: {}", toolchain.name());
        println!("override location: {}", toolchain.path().display());
        // FIXME: On windows this displays the UNC portion of the
        // windows path, which is pretty ugly
        println!("override reason: {}", reason);

        println!("");
        try!(show_tool_versions(&toolchain));
        println!("");
    } else {
        println!("no override");
        try!(show_default(cfg));
    }
    Ok(())
}

fn list_targets(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = m.value_of("toolchain").unwrap();
    let toolchain = try!(cfg.get_toolchain(toolchain, false));
    try!(common::list_targets(&toolchain));

    Ok(())
}

fn add_target(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = m.value_of("toolchain").unwrap();
    let target = m.value_of("target").unwrap();
    let toolchain = try!(cfg.get_toolchain(toolchain, false));
    let new_component = Component {
        pkg: "rust-std".to_string(),
        target: TargetTriple::from_str(target),
    };
    try!(toolchain.add_component(new_component));

    Ok(())
}

fn remove_target(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = m.value_of("toolchain").unwrap();
    let target = m.value_of("target").unwrap();
    let toolchain = try!(cfg.get_toolchain(toolchain, false));
    let new_component = Component {
        pkg: "rust-std".to_string(),
        target: TargetTriple::from_str(target),
    };
    try!(toolchain.remove_component(new_component));

    Ok(())
}
