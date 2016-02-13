#[macro_use]
extern crate rust_install;

#[macro_use]
extern crate clap;
extern crate rand;
extern crate regex;
extern crate hyper;
#[macro_use]
extern crate multirust;
extern crate term;
extern crate openssl;
extern crate itertools;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;
#[cfg(windows)]
extern crate user32;

use clap::ArgMatches;
use std::env;
use std::path::{Path, PathBuf};
use std::io::{Write, BufRead};
use std::process::{Command, Stdio};
use std::process;
use std::ffi::OsStr;
use std::fmt;
use std::iter;
use std::thread;
use std::time::Duration;
use multirust::*;
use rust_install::dist;
use openssl::crypto::hash::{Type, Hasher};
use itertools::Itertools;

mod cli;

macro_rules! warn {
	( $ ( $ arg : tt ) * ) => ( $crate::warn_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}
macro_rules! err {
	( $ ( $ arg : tt ) * ) => ( $crate::err_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}
macro_rules! info {
	( $ ( $ arg : tt ) * ) => ( $crate::info_fmt ( format_args ! ( $ ( $ arg ) * ) ) )
}

fn warn_fmt(args: fmt::Arguments) {
    let mut t = term::stderr().unwrap();
    let _ = t.fg(term::color::BRIGHT_YELLOW);
    let _ = write!(t, "warning: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

fn err_fmt(args: fmt::Arguments) {
    let mut t = term::stderr().unwrap();
    let _ = t.fg(term::color::BRIGHT_RED);
    let _ = write!(t, "error: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

fn info_fmt(args: fmt::Arguments) {
    let mut t = term::stderr().unwrap();
    let _ = t.fg(term::color::BRIGHT_GREEN);
    let _ = write!(t, "info: ");
    let _ = t.reset();
    let _ = t.write_fmt(args);
    let _ = write!(t, "\n");
}

fn set_globals(m: Option<&ArgMatches>) -> Result<Cfg> {
    // Base config
    let verbose = m.map_or(false, |m| m.is_present("verbose"));
    Cfg::from_env(shared_ntfy!(move |n: Notification| {
        use multirust::notify::NotificationLevel::*;
        match n.level() {
            Verbose => {
                if verbose {
                    println!("{}", n);
                }
            }
            Normal => {
                println!("{}", n);
            }
            Info => {
                info!("{}", n);
            }
            Warn => {
                warn!("{}", n);
            }
            Error => {
                err!("{}", n);
            }
        }
    }))

}

fn main() {
    if let Err(e) = run_multirust() {
        err!("{}", e);
        std::process::exit(1);
    }
}

fn run_inner<S: AsRef<OsStr>>(_: &Cfg, command: Result<Command>, args: &[S]) -> Result<()> {
    if let Ok(mut command) = command {
        for arg in &args[1..] {
            if arg.as_ref() == <str as AsRef<OsStr>>::as_ref("--multirust") {
                println!("Proxied via multirust");
                std::process::exit(0);
            } else {
                command.arg(arg);
            }
        }
        match command.status() {
            Ok(result) => {
                // Ensure correct exit code is returned
                std::process::exit(result.code().unwrap_or(1));
            }
            Err(e) => {
                Err(utils::Error::RunningCommand {
                        name: args[0].as_ref().to_owned(),
                        error: utils::raw::CommandError::Io(e),
                    }
                    .into())
            }
        }

    } else {
        for arg in &args[1..] {
            if arg.as_ref() == <str as AsRef<OsStr>>::as_ref("--multirust") {
                println!("Proxied via multirust");
                std::process::exit(0);
            }
        }
        command.map(|_| ())
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

fn direct_proxy(cfg: &Cfg, arg0: &str) -> Result<()> {
    let args: Vec<_> = env::args_os().collect();

    run_inner(cfg,
              cfg.create_command_for_dir(&try!(utils::current_dir()), arg0),
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

fn test_proxies() -> bool {
    let result = utils::cmd_status("rustc",
                                   shell_cmd("rustc --multirust".as_ref())
                                       .stdin(Stdio::null())
                                       .stdout(Stdio::null())
                                       .stderr(Stdio::null()));
    result.is_ok()
}

fn test_installed(cfg: &Cfg) -> bool {
    utils::is_file(cfg.multirust_dir.join(bin_path("multirust")))
}

fn maybe_direct_proxy() -> Result<bool> {
    let arg0: PathBuf = env::args_os().next().unwrap().into();

    if let Some(name) = arg0.file_stem().and_then(OsStr::to_str) {
        if !name.starts_with("multirust") {
            let cfg = try!(set_globals(None));
            try!(direct_proxy(&cfg, name));
            return Ok(true);
        }
    }
    Ok(false)
}

fn run_multirust() -> Result<()> {
    // Check for infinite recursion
    if env::var("RUST_RECURSION_COUNT").ok().and_then(|s| s.parse().ok()).unwrap_or(0) > 5 {
        return Err(Error::InfiniteRecursion);
    }

    // If the executable name is not multirust*, then go straight
    // to proxying
    if try!(maybe_direct_proxy()) {
        return Ok(());
    }

    let app_matches = cli::get().get_matches();

    let cfg = try!(set_globals(Some(&app_matches)));

    match app_matches.subcommand_name() {
        Some("upgrade-data") | Some("delete-data") | Some("install") | 
        Some("uninstall") | None => {} // Don't need consistent metadata
        Some(_) => {
            try!(cfg.check_metadata_version());
        }
    }

    // Make sure everything is set-up correctly
    match app_matches.subcommand_name() {
        Some("self") | Some("proxy") => {}
        _ => {
            if !test_proxies() {
                if !test_installed(&cfg) {
                    warn!("multirust is not installed for the current user: `rustc` invocations \
                           will not be proxied.\n\nFor more information, run  `multirust install \
                           --help`\n");
                } else {
                    warn!("multirust is installed but is not set up correctly: `rustc` \
                           invocations will not be proxied.\n\nEnsure '{}' is on your PATH, and \
                           has priority.\n",
                          cfg.multirust_dir.join("bin").display());
                }
            }
        }
    }

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
        ("ctl", Some(m)) => ctl(&cfg, m),
        ("doc", Some(m)) => doc(&cfg, m),
        _ => {
            let result = maybe_install(&cfg);
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

fn maybe_install(cfg: &Cfg) -> Result<()> {
    let exe_path = try!(utils::current_exe());
    if !test_installed(&cfg) {
        if !ask("Install multirust now?").unwrap_or(false) {
            return Ok(());
        }
        let add_to_path = ask("Add multirust to PATH?").unwrap_or(false);
        return handle_install(cfg, false, add_to_path);
    } else if exe_path.parent() != Some(&cfg.multirust_dir.join("bin")) {
        println!("Existing multirust installation detected.");
        if !ask("Replace or update it now?").unwrap_or(false) {
            return Ok(());
        }
        return handle_install(cfg, false, false);
    } else {
        println!("This is the currently installed multirust binary.");
    }
    Ok(())
}

fn self_install(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    handle_install(cfg, m.is_present("move"), m.is_present("add-to-path"))
}

fn handle_install(cfg: &Cfg, should_move: bool, add_to_path: bool) -> Result<()> {
    #[allow(dead_code)]
    fn create_bat_proxy(mut path: PathBuf, name: &'static str) -> utils::Result<()> {
        path.push(name.to_owned() + ".bat");
        utils::write_file(name,
                          &path,
                          &format!("@\"%~dp0\\multirust\" proxy {} %*", name))
    }
    #[allow(dead_code)]
    fn create_sh_proxy(mut path: PathBuf, name: &'static str) -> utils::Result<()> {
        path.push(name.to_owned());
        try!(utils::write_file(name,
                               &path,
                               &format!("#!/bin/sh\n\"`dirname $0`/multirust\" proxy {} \"$@\"",
                                        name)));
        utils::make_executable(&path)
    }
    fn create_symlink_proxy(mut path: PathBuf, name: &'static str) -> utils::Result<()> {
        let mut dest_path = path.clone();
        dest_path.push("multirust".to_owned() + env::consts::EXE_SUFFIX);
        path.push(name.to_owned() + env::consts::EXE_SUFFIX);
        utils::symlink_file(&dest_path, &path)
    }
    fn create_hardlink_proxy(mut path: PathBuf, name: &'static str) -> utils::Result<()> {
        let mut dest_path = path.clone();
        dest_path.push("multirust".to_owned() + env::consts::EXE_SUFFIX);
        path.push(name.to_owned() + env::consts::EXE_SUFFIX);
        utils::hardlink_file(&dest_path, &path)
    }

    let bin_path = cfg.multirust_dir.join("bin");

    try!(utils::ensure_dir_exists("bin", &bin_path, ntfy!(&cfg.notify_handler)));

    let dest_path = bin_path.join("multirust".to_owned() + env::consts::EXE_SUFFIX);
    let src_path = try!(utils::current_exe());

    if should_move {
        if cfg!(windows) {
            // Wait for old version to exit
            thread::sleep(Duration::from_millis(1000));
        }
        try!(utils::rename_file("multirust", &src_path, &dest_path));
    } else {
        try!(utils::copy_file(&src_path, &dest_path));
    }

    let tools = ["rustc", "rustdoc", "cargo", "rust-lldb", "rust-gdb"];
    for tool in &tools {
        // There are five ways to create the proxies:
        // 1) Shell/batch scripts
        //    On windows, `CreateProcess` (on which Command is based) will not look for batch scripts
        // 2) Symlinks
        //    On windows, symlinks require admin privileges to create
        // 3) Copies of the multirust binary
        //    The multirust binary is not exactly small
        // 4) Stub executables
        //    Complicates build process and even trivial rust executables are quite large
        // 5) Hard links
        //    Downsides are yet to be determined
        // As a result, use hardlinks on windows, and symlinks elsewhere.

        // try!(create_bat_proxy(bin_path.clone(), tool));
        // try!(create_sh_proxy(bin_path.clone(), tool));

        if cfg!(windows) {
            try!(create_hardlink_proxy(bin_path.clone(), tool));
        } else {
            try!(create_symlink_proxy(bin_path.clone(), tool));
        }
    }

    #[cfg(windows)]
    fn do_add_to_path(path: PathBuf) -> Result<()> {

        use winreg::RegKey;
        use winapi::*;
        use user32::*;
        use std::ptr;

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = try!(root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                                   .map_err(|_| Error::PermissionDenied));

        let mut new_path: String = path.into_os_string()
                                       .into_string()
                                       .ok()
                                       .expect("cannot install to invalid unicode path");
        let old_path: String = environment.get_value("PATH").unwrap_or(String::new());
        new_path.push_str(";");
        new_path.push_str(&old_path);
        try!(environment.set_value("PATH", &new_path)
                        .map_err(|_| Error::PermissionDenied));

        // const HWND_BROADCAST: HWND = 0xffff as HWND;
        // const SMTO_ABORTIFHUNG: UINT = 0x0002;

        // Tell other processes to update their environment
        unsafe {
            SendMessageTimeoutA(HWND_BROADCAST,
                                WM_SETTINGCHANGE,
                                0 as WPARAM,
                                "Environment\0".as_ptr() as LPARAM,
                                SMTO_ABORTIFHUNG,
                                5000,
                                ptr::null_mut());
        }

        println!("PATH has been updated. You may need to restart your shell for changes to take \
                  effect.");

        Ok(())
    }
    #[cfg(not(windows))]
    fn do_add_to_path(path: PathBuf) -> Result<()> {
        let home_dir = try!(utils::home_dir().ok_or(utils::Error::LocatingHome));
        let tmp = path.into_os_string()
                      .into_string()
                      .expect("cannot install to invalid unicode path");
        try!(utils::append_file(".profile",
                                &home_dir.join(".profile"),
                                &format!("\n# Multirust override:\nexport PATH=\"{}:$PATH\"",
                                         &tmp)));

        println!("'~/.profile' has been updated. You will need to start a new login shell for \
                  changes to take effect.");

        Ok(())
    }

    if add_to_path {
        try!(do_add_to_path(bin_path));
    }

    info!("Installed");

    Ok(())
}

fn self_uninstall(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    if !m.is_present("no-prompt") &&
       !ask("This will delete all toolchains, overrides, aliases, and other multirust data \
            associated with this user. Continue?")
            .unwrap_or(false) {
        println!("aborting");
        return Ok(());
    }

    #[cfg(windows)]
    fn inner(cfg: &Cfg) -> Result<()> {
        let mut cmd = Command::new("cmd");
        let _ = cmd.arg("/C")
                   .arg("start")
                   .arg("cmd")
                   .arg("/C")
                   .arg(&format!("echo Uninstalling... & ping -n 4 127.0.0.1>nul & rd /S /Q {} \
                                  & echo Uninstalled",
                                 cfg.multirust_dir.display()))
                   .spawn();
        Ok(())
    }
    #[cfg(not(windows))]
    fn inner(cfg: &Cfg) -> Result<()> {
        println!("Uninstalling...");
        Ok(try!(utils::remove_dir("multirust", &cfg.multirust_dir, ntfy!(&cfg.notify_handler))))
    }

    warn!("This will not attempt to remove the '.multirust/bin' directory from your PATH");
    try!(inner(cfg));

    process::exit(0);
}

fn self_update(cfg: &Cfg, _m: &ArgMatches) -> Result<()> {
    // Get host triple
    let triple = if let (arch, Some(os), maybe_env) = dist::get_host_triple() {
        if let Some(env) = maybe_env {
            format!("{}-{}-{}", arch, os, env)
        } else {
            format!("{}-{}", arch, os)
        }
    } else {
        return Err(Error::UnknownHostTriple);
    };

    // Get download URL
    let url = format!("https://github.\
                       com/Diggsey/multirust-rs-binaries/raw/master/{}/multirust-rs{}",
                      triple,
                      env::consts::EXE_SUFFIX);

    // Calculate own hash
    let mut hasher = Hasher::new(Type::SHA256);
    try!(utils::tee_file("self", &try!(utils::current_exe()), &mut hasher));
    let current_hash = hasher.finish()
                             .iter()
                             .map(|b| format!("{:02x}", b))
                             .join("");

    // Download latest hash
    let mut latest_hash = {
        let hash_url = try!(utils::parse_url(&(url.clone() + ".sha256")));
        let hash_file = try!(cfg.temp_cfg.new_file());
        try!(utils::download_file(hash_url, &hash_file, None, ntfy!(&cfg.notify_handler)));
        try!(utils::read_file("hash", &hash_file))
    };
    latest_hash.truncate(64);

    // If up-to-date
    if latest_hash == current_hash {
        info!("Already up to date!");
        return Ok(());
    }

    // Get download path
    let download_file = try!(cfg.temp_cfg.new_file_with_ext("multirust-", env::consts::EXE_SUFFIX));
    let download_url = try!(utils::parse_url(&url));

    // Download new version
    let mut hasher = Hasher::new(Type::SHA256);
    try!(utils::download_file(download_url,
                              &download_file,
                              Some(&mut hasher),
                              ntfy!(&cfg.notify_handler)));
    let download_hash = hasher.finish()
                              .iter()
                              .map(|b| format!("{:02x}", b))
                              .join("");

    // Check that hash is correct
    if latest_hash != download_hash {
        return Err(Error::Install(rust_install::Error::ChecksumFailed {
            url: url,
            expected: latest_hash,
            calculated: download_hash,
        }));
    }

    // Mark as executable
    try!(utils::make_executable(&download_file));

    #[cfg(windows)]
    fn inner(path: &Path) -> Result<()> {
        let mut cmd = Command::new("cmd");
        let _ = cmd.arg("/C")
                   .arg("start")
                   .arg(path)
                   .arg("self")
                   .arg("install")
                   .arg("-m")
                   .spawn();
        Ok(())
    }
    #[cfg(not(windows))]
    fn inner(path: &Path) -> Result<()> {
        Ok(try!(utils::cmd_status("update",
                                  Command::new(path).arg("self").arg("install").arg("-m"))))
    }

    println!("Installing...");
    try!(inner(&download_file));
    process::exit(0);
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
        try!(toolchain.install_from_dist_if_not_installed());
    }

    toolchain.make_default()
}

fn override_(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(get_toolchain(cfg, m, true));
    if !try!(common_install_args(&toolchain, m)) {
        try!(toolchain.install_from_dist_if_not_installed());
    }

    toolchain.make_override(&try!(utils::current_dir()))
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

fn doc_url(m: &ArgMatches) -> &'static str {
    if m.is_present("all") {
        "index.html"
    } else {
        "std/index.html"
    }
}

fn doc(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    cfg.open_docs_for_dir(&try!(utils::current_dir()), doc_url(m))
}

fn ctl_home(cfg: &Cfg) -> Result<()> {
    println!("{}", cfg.multirust_dir.display());
    Ok(())
}

fn ctl_overide_toolchain(cfg: &Cfg) -> Result<()> {
    let (toolchain, _) = try!(cfg.toolchain_for_dir(&try!(utils::current_dir())));

    println!("{}", toolchain.name());
    Ok(())
}

fn ctl_default_toolchain(cfg: &Cfg) -> Result<()> {
    let toolchain = try!(try!(cfg.find_default()).ok_or(Error::NoDefaultToolchain));

    println!("{}", toolchain.name());
    Ok(())
}

fn ctl_toolchain_sysroot(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let toolchain = try!(get_toolchain(cfg, m, false));

    let toolchain_dir = toolchain.prefix().path();
    println!("{}", toolchain_dir.display());
    Ok(())
}

fn ctl(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    match m.subcommand() {
        ("home", Some(_)) => ctl_home(cfg),
        ("override-toolchain", Some(_)) => ctl_overide_toolchain(cfg),
        ("default-toolchain", Some(_)) => ctl_default_toolchain(cfg),
        ("toolchain-sysroot", Some(m)) => ctl_toolchain_sysroot(cfg, m),
        _ => Ok(()),
    }
}

fn which(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    let binary = m.value_of("binary").unwrap();

    let binary_path = try!(cfg.which_binary(&try!(utils::current_dir()), binary))
                          .expect("binary not found");

    try!(utils::assert_is_file(&binary_path));

    println!("{}", binary_path.display());
    Ok(())
}

fn read_line() -> String {
    let stdin = std::io::stdin();
    let stdin = stdin.lock();
    let mut lines = stdin.lines();
    lines.next().unwrap().unwrap()
}

fn ask(question: &str) -> Option<bool> {
    print!("{} (y/n) ", question);
    let _ = std::io::stdout().flush();
    let input = read_line();

    match &*input {
        "y" | "Y" => Some(true),
        "n" | "N" => Some(false),
        _ => None,
    }
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
        let rustc_path = toolchain.prefix().binary_file("rustc");
        let cargo_path = toolchain.prefix().binary_file("cargo");

        if utils::is_file(&rustc_path) {
            let mut cmd = Command::new(&rustc_path);
            cmd.arg("--version");
            toolchain.prefix().set_ldpath(&mut cmd);

            if utils::cmd_status("rustc", &mut cmd).is_err() {
                println!("(failed to run rustc)");
            }
        } else {
            println!("(no rustc command in toolchain?)");
        }
        if utils::is_file(&cargo_path) {
            let mut cmd = Command::new(&cargo_path);
            cmd.arg("--version");
            toolchain.prefix().set_ldpath(&mut cmd);

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
        println!("default location: {}", toolchain.prefix().path().display());

        show_tool_versions(&toolchain)
    } else {
        println!("no default toolchain configured. run `multirust helpdefault`");
        Ok(())
    }
}

fn show_override(cfg: &Cfg) -> Result<()> {
    if let Some((toolchain, reason)) = try!(cfg.find_override(&try!(utils::current_dir()))) {
        println!("override toolchain: {}", toolchain.name());
        println!("override location: {}", toolchain.prefix().path().display());
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

fn update(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
    if let Some(name) = m.value_of("toolchain") {
        let toolchain = try!(cfg.get_toolchain(name, true));
        if !try!(common_install_args(&toolchain, m)) {
            try!(toolchain.install_from_dist())
        }
    } else {
        try!(update_all_channels(cfg))
    }
    Ok(())
}
