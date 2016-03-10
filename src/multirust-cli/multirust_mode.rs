use clap::ArgMatches;
use cli;
use common::ask;
use common::{set_globals, run_inner};
use multirust::*;
use multirust_dist::manifest::Component;
use self_update;
use std::env;
use std::ffi::OsStr;
use std::io::Write;
use std::iter;
use std::path::Path;
use std::process::Command;
use term;
use tty;

pub fn main() -> Result<()> {
    let need_metadata = try!(command_requires_metadata());
    if need_metadata {
        let cfg = try!(Cfg::from_env(shared_ntfy!(move |_: Notification| { })));
        try!(cfg.check_metadata_version());
    }

    let app_matches = cli::get().get_matches();
    let verbose = app_matches.is_present("verbose");
    let cfg = try!(set_globals(verbose));

    match app_matches.subcommand() {
        ("update", Some(m)) => update(&cfg, m),
        ("default", Some(m)) => default_(&cfg, m),
        ("override", Some(m)) => override_(&cfg, m),
        ("show-default", Some(_)) => show_default(&cfg),
        ("show-override", Some(_)) => show_override(&cfg),
        ("list-overrides", Some(_)) => list_overrides(&cfg),
        ("list-toolchains", Some(_)) => list_toolchains(&cfg),
        ("remove-override", Some(m)) => remove_override(&cfg, m),
        ("remove-toolchain", Some(m)) => remove_toolchain_args(&cfg, m),
        ("list-targets", Some(m)) => list_targets(&cfg, m),
        ("add-target", Some(m)) => add_target(&cfg, m),
        ("remove-target", Some(m)) => remove_target(&cfg, m),
        ("run", Some(m)) => run(&cfg, m),
        ("proxy", Some(m)) => proxy(&cfg, m),
        ("upgrade-data", Some(_)) => cfg.upgrade_data().map(|_| ()),
        ("delete-data", Some(m)) => delete_data(&cfg, m),
        ("self", Some(c)) => {
            match c.subcommand() {
                ("install", Some(m)) => self_install(&cfg, m),
                ("uninstall", Some(m)) => self_uninstall(&cfg, m),
                ("update", Some(m)) => self_update(&cfg, m),
                _ => Ok(()),
            }
        }
        ("which", Some(m)) => which(&cfg, m),
        ("doc", Some(m)) => doc(&cfg, m),
        _ => {
            let result = maybe_self_install(&cfg);
            println!("");

            // Suspend in case we were run from the UI
            try!(utils::cmd_status("shell",
                                   &mut shell_cmd((if cfg!(windows) {
                                                      "pause"
                                                  } else {
                                                      "echo -n \"Press any key to continue...\" && \
                                                       CFG=`stty -g` && stty -echo -icanon && dd \
                                                       count=1 1>/dev/null 2>&1 && stty $CFG && \
                                                       echo"
                                                  })
                                                  .as_ref())));

            result
        }
    }
}

fn run(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(get_toolchain(cfg, m, false));
    let args = m.values_of("command").unwrap();

    run_inner(cfg, toolchain.create_command(args[0]), &args)
}

fn proxy(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let args = m.values_of("command").unwrap();

    run_inner(cfg,
              cfg.create_command_for_dir(&try!(utils::current_dir()), args[0]),
              &args)
}

fn shell_cmd(cmdline: &OsStr) -> Command {
    #[cfg(windows)]
    fn inner(cmdline: &OsStr) -> Command {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(cmdline);
        cmd
    }
    #[cfg(not(windows))]
    fn inner(cmdline: &OsStr) -> Command {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(cmdline);
        cmd
    }

    inner(cmdline)
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

fn maybe_self_install(cfg: &Cfg) -> Result<()> {
    self_update::maybe_install(cfg)
}

fn self_install(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    self_update::install(cfg, m.is_present("move"), m.is_present("add-to-path"))
}

fn self_uninstall(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let no_prompt = m.is_present("no-prompt");
    self_update::uninstall(cfg, no_prompt)
}

fn self_update(cfg: &Cfg, _m: &ArgMatches) -> Result<()> {
    self_update::update(cfg)
}

fn get_toolchain<'a>(cfg: &'a Cfg, m: &ArgMatches, create_parent: bool) -> Result<Toolchain<'a>> {
    cfg.get_toolchain(m.value_of("toolchain").unwrap(), create_parent)
}

fn remove_toolchain_args(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    try!(get_toolchain(cfg, m, false)).remove()
}

fn default_(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(get_toolchain(cfg, m, true));
    if !try!(common_install_args(&toolchain, m)) {
        if !toolchain.is_custom() {
            try!(toolchain.install_from_dist_if_not_installed());
        }
    }

    try!(toolchain.make_default());

    println!("");
    try!(show_channel_version(cfg, toolchain.name()));
    Ok(())
}

fn update(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    if let Some(name) = m.value_of("toolchain") {
        let toolchain = try!(cfg.get_toolchain(name, true));
        if !try!(common_install_args(&toolchain, m)) {
            try!(toolchain.install_from_dist())
        }
        println!("");
        try!(show_channel_version(cfg, name));
    } else {
        try!(update_all_channels(cfg))
    }
    Ok(())
}

fn override_(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(get_toolchain(cfg, m, true));
    if !try!(common_install_args(&toolchain, m)) {
        if !toolchain.is_custom() {
            try!(toolchain.install_from_dist_if_not_installed());
        }
    }

    try!(toolchain.make_override(&try!(utils::current_dir())));

    println!("");
    try!(show_channel_version(cfg, toolchain.name()));
    Ok(())
}

fn common_install_args(toolchain: &Toolchain, m: &ArgMatches) -> Result<bool> {

    if let Some(installers) = m.values_of("installer") {
        let is: Vec<_> = installers.iter().map(|i| i.as_ref()).collect();
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

fn show_channel_version(cfg: &Cfg, name: &str) -> Result<()> {
    let mut t = term::stdout().unwrap();
    if tty::stdout_isatty() { let _ = t.fg(term::color::BRIGHT_WHITE); }
    if tty::stdout_isatty() { let _ = t.bg(term::color::BLACK); }
    let _ = write!(t, "{}", name);
    if tty::stdout_isatty() { let _ = t.reset(); }
    let _ = writeln!(t, " revision:");
    try!(show_tool_versions(&try!(cfg.get_toolchain(&name, false))));
    Ok(())
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
    cfg.open_docs_for_dir(&try!(utils::current_dir()), doc_url(m))
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
    if !m.is_present("no-prompt") &&
       !ask("This will delete all toolchains, overrides, aliases, and other multirust data \
             associated with this user. Continue?")
            .unwrap_or(false) {
        println!("aborting");
        return Ok(());
    }

    cfg.delete_data()
}

fn remove_override(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    if let Some(path) = m.value_of("override") {
        cfg.override_db.remove(path.as_ref(), &cfg.temp_cfg, cfg.notify_handler.as_ref())
    } else {
        cfg.override_db.remove(&try!(utils::current_dir()),
                               &cfg.temp_cfg,
                               cfg.notify_handler.as_ref())
    }
    .map(|_| ())
}

fn show_tool_versions(toolchain: &Toolchain) -> Result<()> {
    println!("");

    if toolchain.exists() {
        let rustc_path = toolchain.binary_file("rustc");
        let cargo_path = toolchain.binary_file("cargo");

        if utils::is_file(&rustc_path) {
            let mut cmd = Command::new(&rustc_path);
            cmd.arg("--version");
            toolchain.set_ldpath(&mut cmd);

            if utils::cmd_status("rustc", &mut cmd).is_err() {
                println!("(failed to run rustc)");
            }
        } else {
            println!("(no rustc command in toolchain?)");
        }
        if utils::is_file(&cargo_path) {
            let mut cmd = Command::new(&cargo_path);
            cmd.arg("--version");
            toolchain.set_ldpath(&mut cmd);

            if utils::cmd_status("cargo", &mut cmd).is_err() {
                println!("(failed to run cargo)");
            }
        } else {
            println!("(no cargo command in toolchain?)");
        }
    } else {
        println!("(toolchain not installed)");
    }
    println!("");
    Ok(())
}

fn show_default(cfg: &Cfg) -> Result<()> {
    if let Some(toolchain) = try!(cfg.find_default()) {
        println!("default toolchain: {}", toolchain.name());
        println!("default location: {}", toolchain.path().display());

        show_tool_versions(&toolchain)
    } else {
        println!("no default toolchain configured. run `multirust help default`");
        Ok(())
    }
}

fn show_override(cfg: &Cfg) -> Result<()> {
    if let Some((toolchain, reason)) = try!(cfg.find_override(&try!(utils::current_dir()))) {
        println!("override toolchain: {}", toolchain.name());
        println!("override location: {}", toolchain.path().display());
        // FIXME: On windows this displays the UNC portion of the
        // windows path, which is pretty ugly
        println!("override reason: {}", reason);

        show_tool_versions(&toolchain)
    } else {
        println!("no override");
        show_default(cfg)
    }
}

fn list_overrides(cfg: &Cfg) -> Result<()> {
    let mut overrides = try!(cfg.override_db.list());

    overrides.sort();

    if overrides.is_empty() {
        println!("no overrides");
    } else {
        for o in overrides {
            println!("{}", o);
        }
    }
    Ok(())
}

fn list_toolchains(cfg: &Cfg) -> Result<()> {
    let mut toolchains = try!(cfg.list_toolchains());

    toolchains.sort();

    if toolchains.is_empty() {
        println!("no installed toolchains");
    } else {
        for toolchain in toolchains {
            println!("{}", &toolchain);
        }
    }
    Ok(())
}

fn update_all_channels(cfg: &Cfg) -> Result<()> {
    let toolchains = try!(cfg.update_all_channels());

    let max_name_length = toolchains.iter().map(|&(ref n, _)| n.len()).max().unwrap_or(0);
    let padding_str: String = iter::repeat(' ').take(max_name_length).collect();

    println!("");
    let mut t = term::stdout().unwrap();
    for &(ref name, ref result) in &toolchains {
        let _ = t.fg(term::color::BRIGHT_WHITE);
        let _ = t.bg(term::color::BLACK);
        let _ = write!(t,
                       "{}{}",
                       &padding_str[0..(max_name_length - name.len())],
                       name);
        let _ = t.reset();
        let _ = write!(t, " update ");
        if result.is_ok() {
            let _ = t.fg(term::color::BRIGHT_GREEN);
            let _ = writeln!(t, "succeeded");
            let _ = t.reset();
        } else {
            let _ = t.fg(term::color::BRIGHT_RED);
            let _ = writeln!(t, "FAILED");
            let _ = t.reset();
        }
    }
    println!("");

    for (name, _) in toolchains {
        let _ = t.fg(term::color::BRIGHT_WHITE);
        let _ = t.bg(term::color::BLACK);
        let _ = write!(t, "{}", name);
        let _ = t.reset();
        let _ = writeln!(t, " revision:");
        try!(show_tool_versions(&try!(cfg.get_toolchain(&name, false))));
    }
    Ok(())
}

fn list_targets(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = m.value_of("toolchain").unwrap();
    let toolchain = try!(cfg.get_toolchain(toolchain, false));
    for component in try!(toolchain.list_components()) {
        if component.pkg == "rust-std" {
            println!("{}", component.target);
        }
    }

    Ok(())
}

fn add_target(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = m.value_of("toolchain").unwrap();
    let target = m.value_of("target").unwrap();
    let toolchain = try!(cfg.get_toolchain(toolchain, false));
    let new_component = Component {
        pkg: "rust-std".to_string(),
        target: target.to_string(),
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
        target: target.to_string(),
    };
    try!(toolchain.remove_component(new_component));

    Ok(())
}
