
#[macro_use]
extern crate clap;
extern crate rand;
extern crate regex;
extern crate hyper;
extern crate multirust;
extern crate term;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;
#[cfg(windows)]
extern crate user32;

use clap::{App, ArgMatches};
use std::env;
use std::path::{Path, PathBuf};
use std::io::{Write, BufRead};
use std::process::{Command, Stdio};
use std::process;
use std::ffi::{OsStr, OsString};
use std::fmt;
use multirust::*;


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
	let verbose = m.map(|m| m.is_present("verbose")).unwrap_or(false);
	Cfg::from_env(NotifyHandler::from(move |n: Notification| {
		match n.level() {
			NotificationLevel::Verbose => if verbose {
				println!("{}", n);
			},
			NotificationLevel::Normal => {
				println!("{}", n);
			},
			NotificationLevel::Info => {
				info!("{}", n);
			},
			NotificationLevel::Warn => {
				warn!("{}", n);
			}
		}
	}))
		
}

fn main() {
	if let Err(e) = try_main() {
		err!("{}", e);
		std::process::exit(1);
	}
}

fn try_main() -> Result<()> {
	let mut arg_iter = env::args_os();
	let arg0 = PathBuf::from(arg_iter.next().unwrap());
	let arg0_stem = arg0.file_stem().expect("invalid multirust invocation")
		.to_str().expect("don't know how to proxy that binary");
	
	match arg0_stem {
		"rustc" | "rustdoc" | "cargo" | "rust-lldb" | "rust-gdb" => {
			run_proxy(arg0_stem, arg_iter)
		},
		other => {
			if other.starts_with("multirust") {
				let arg1 = arg_iter.next();
				if let Some("run") = arg1.as_ref().and_then(|s| s.to_str()) {
					let arg2 = PathBuf::from(arg_iter.next().expect("expected binary name"))
						.file_stem().expect("invalid binary name").to_owned();
					let stem = arg2.to_str().expect("don't know how to proxy that binary");
					if !stem.starts_with("-") {
						run_proxy(stem, arg_iter)
					} else {
						run_multirust()
					}
				} else {
					run_multirust()
				}
			} else {
				Err(Error::Custom { id: "no-proxy".to_owned(), desc: format!("don't know how to proxy that binary: {}", other) })
			}
		},
	}
}

fn current_dir() -> Result<PathBuf> {
	env::current_dir().map_err(|_| Error::LocatingWorkingDir)
}

fn run_proxy<I: Iterator<Item=OsString>>(binary: &str, arg_iter: I) -> Result<()> {
	let cfg = try!(set_globals(None));
	
	let result = cfg.create_command_for_dir(&try!(current_dir()), binary);
	
	if let Ok(mut command) = result {
		for arg in arg_iter {
			if let Some("--multirust") = arg.to_str() {
				println!("Proxied via multirust");
				std::process::exit(0);
			} else {
				command.arg(arg);
			}
		}
		let result = command.status()
			.ok().expect(&format!("failed to run `{}`", binary));
			
		// Ensure correct exit code is returned
		std::process::exit(result.code().unwrap_or(1));
	} else {
		for arg in arg_iter {
			if let Some("--multirust") = arg.to_str() {
				println!("Proxied via multirust");
				std::process::exit(0);
			}
		}
		result.map(|_|())
	}
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
	let mut cmd = shell_cmd("rustc --multirust".as_ref());
	cmd
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null());
	let result = utils::cmd_status("rustc", cmd);
	result.is_ok()
}

fn test_installed(cfg: &Cfg) -> bool {
	utils::is_file(cfg.multirust_dir.join(bin_path("multirust")))
}

fn run_multirust() -> Result<()> {
	let yaml = load_yaml!("cli.yml");
	let app_matches = App::from_yaml(yaml).get_matches();
	
	let cfg = try!(set_globals(Some(&app_matches)));
	
	match app_matches.subcommand_name() {
		Some("upgrade-data")|Some("delete-data")|Some("install")|Some("uninstall") => {}, // Don't need consistent metadata
		Some(_) => { try!(cfg.check_metadata_version()); },
		_ => {},
	}
	
	// Make sure everything is set-up correctly
	match app_matches.subcommand_name() {
		Some("install") => {},
		_ => {
			if !test_proxies() {
				if !test_installed(&cfg) {
					warn!("multirust is not installed for the current user: \
						`rustc` invocations will not be proxied.\n\n\
						For more information, run  `multirust install --help`\n");
				} else {
					warn!("multirust is installed but is not set up correctly: \
						`rustc` invocations will not be proxied.\n\n\
						Ensure '{}' is on your PATH, and has priority.\n", cfg.multirust_dir.join("bin").display());
				}
			}
		},
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
		("upgrade-data", Some(_)) => cfg.upgrade_data().map(|_|()),
		("delete-data", Some(m)) => delete_data(&cfg, m),
		("install", Some(m)) => install(&cfg, m),
		("uninstall", Some(m)) => uninstall(&cfg, m),
		("which", Some(m)) => which(&cfg, m),
		("ctl", Some(m)) => ctl(&cfg, m),
		("doc", Some(m)) => doc(&cfg, m),
		_ => maybe_install(&cfg),
	}
}

fn maybe_install(cfg: &Cfg) -> Result<()> {
	let exe_path = try!(env::current_exe().map_err(|_| Error::LocatingWorkingDir));
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
		read_line();
	}
	Ok(())
}

fn install(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
	handle_install(cfg, m.is_present("move"), m.is_present("add-to-path"))
}

fn handle_install(cfg: &Cfg, should_move: bool, add_to_path: bool) -> Result<()> {
	#[cfg(windows)]
	fn create_proxy_script(mut path: PathBuf, name: &'static str) -> Result<()> {
		path.push(name.to_owned() + ".bat");
		utils::write_file(name, &path, &format!("@\"%~dp0\\multirust\" run {} %*", name))
	}
	#[cfg(not(windows))]
	fn create_proxy_script(mut path: PathBuf, name: &'static str) -> Result<()> {
		path.push(name.to_owned() + ".sh");
		utils::write_file(name, &path, &format!("#!/bin/sh\n\"`dirname $0`/multirust\" run {} \"$@\"", name))
	}
	
	let bin_path = cfg.multirust_dir.join("bin");
	
	try!(utils::ensure_dir_exists("bin", &bin_path, &cfg.notify_handler));
	
	let dest_path = bin_path.join("multirust".to_owned() + env::consts::EXE_SUFFIX);
	let src_path = try!(env::current_exe().map_err(|_| Error::LocatingWorkingDir));
	
	if should_move {
		try!(utils::rename_file("multirust", &src_path, &dest_path));
	} else {
		try!(utils::copy_file(&src_path, &dest_path));
	}
	
	let tools = ["rustc", "rustdoc", "cargo", "rust-lldb", "rust-gdb"];
	for tool in &tools {
		try!(create_proxy_script(bin_path.clone(), tool));
	}
	
	#[cfg(windows)]
	fn do_add_to_path(_cfg: &Cfg, path: PathBuf) -> Result<()> {
		
		use winreg::RegKey;
		use winapi::*;
		use user32::*;
		use std::ptr;

		let root = RegKey::predef(HKEY_CURRENT_USER);
		let environment = try!(root.open_subkey_with_flags("Environment", KEY_READ|KEY_WRITE)
			.map_err(|_| Error::PermissionDenied));
		
		let mut new_path: String = path.into_os_string().into_string().ok().expect("cannot install to invalid unicode path");
		let old_path: String = environment.get_value("PATH").unwrap_or(String::new());
		new_path.push_str(";");
		new_path.push_str(&old_path);
		try!(environment.set_value("PATH", &new_path)
			.map_err(|_| Error::PermissionDenied));
		
		const HWND_BROADCAST: HWND = 0xffff as HWND;
		const SMTO_ABORTIFHUNG: UINT = 0x0002;
		
		// Tell other processes to update their environment
		unsafe {
			SendMessageTimeoutA(HWND_BROADCAST, WM_SETTINGCHANGE, 0 as WPARAM,
				"Environment\0".as_ptr() as LPARAM, SMTO_ABORTIFHUNG,
				5000, ptr::null_mut());
		}
		
		println!("PATH has been updated. You may need to restart your shell for changes to take effect.");
		
		Ok(())
	}
	#[cfg(not(windows))]
	fn do_add_to_path(cfg: &Cfg, path: PathBuf) -> Result<()> {
		let tmp = path.into_os_string().into_string().ok().expect("cannot install to invalid unicode path");
		try!(utils::append_file(".profile", &cfg.home_dir.join(".profile"), &format!("\n# Multirust override:\nexport PATH={}:$PATH", &tmp)));
		
		println!("'~/.profile' has been updated. You will need to start a new login shell for changes to take effect.");
		
		Ok(())
	}
	
	if add_to_path {
		try!(do_add_to_path(cfg, bin_path));
	}
	
	info!("Installed");
	read_line();
	
	Ok(())
}

fn uninstall(cfg: &Cfg, _: &ArgMatches) -> Result<()> {
	#[cfg(windows)]
	fn inner(cfg: &Cfg) -> Result<()> {
		let mut cmd = Command::new("cmd");
		let _ = cmd
			.arg("/C").arg("start").arg("cmd").arg("/C")
			.arg(&format!("echo Uninstalling... & ping -n 4 127.0.0.1>nul & rd /S /Q {} & echo Uninstalled", cfg.multirust_dir.display()))
			.spawn();
		Ok(())
	}
	#[cfg(not(windows))]
	fn inner(cfg: &Cfg) -> Result<()> {
		println!("Uninstalling...");
		utils::remove_dir("multirust", &cfg.multirust_dir, &cfg.notify_handler)
	}
	
	warn!("This will not attempt to remove the '.multirust/bin' directory from your PATH");
	try!(inner(cfg));
	
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
	
	toolchain.make_override(&try!(current_dir()))
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
	cfg.open_docs_for_dir(&try!(current_dir()), doc_url(m))
}

fn ctl_home(cfg: &Cfg) -> Result<()> {
	println!("{}", cfg.multirust_dir.display());
	Ok(())
}

fn ctl_overide_toolchain(cfg: &Cfg) -> Result<()> {
	let (toolchain, _) = try!(cfg.toolchain_for_dir(&try!(current_dir())));
	
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
	
	let binary_path = try!(cfg.which_binary(&try!(current_dir()), binary))
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
		"y"|"Y" => Some(true),
		"n"|"N" => Some(false),
		_ => None,
	}
}

fn delete_data(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
	if !m.is_present("no-prompt") {
		if !ask("This will delete all toolchains, overrides, aliases, and other multirust data associated with this user. Continue?").unwrap_or(false) {
			println!("aborting");
			return Ok(());
		}
	}
	
	cfg.delete_data()
}

fn remove_override(cfg: &Cfg, m: &ArgMatches) -> Result<()> {
	if let Some(path) = m.value_of("override") {
		cfg.override_db.remove(path.as_ref(), &cfg.temp_cfg, &cfg.notify_handler)
	} else {
		cfg.override_db.remove(&try!(current_dir()), &cfg.temp_cfg, &cfg.notify_handler)
	}.map(|_|())
}

fn show_tool_versions(toolchain: &Toolchain) -> Result<()> {
	println!("");

	if toolchain.exists() {
		let rustc_path = toolchain.prefix().binary_file("rustc");
		let cargo_path = toolchain.prefix().binary_file("cargo");

		try!(toolchain.prefix().with_ldpath(|| {
			if utils::is_file(&rustc_path) {
				Command::new(&rustc_path)
					.arg("--version")
					.status()
					.ok().expect("failed to run rustc");
			} else {
				println!("(no rustc command in toolchain?)");
			}
			if utils::is_file(&cargo_path) {
				Command::new(&cargo_path)
					.arg("--version")
					.status()
					.ok().expect("failed to run cargo");
			} else {
				println!("(no cargo command in toolchain?)");
			}
			Ok(())
		}));
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
	if let Some((toolchain, reason)) = try!(cfg.find_override(&try!(current_dir()))) {
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
	let result = cfg.update_all_channels();
	
	if result[0].is_ok() {
		println!("'stable' update succeeded");
	} else {
		println!("'stable' update FAILED");
	}
	if result[1].is_ok() {
		println!("'beta' update succeeded");
	} else {
		println!("'beta' update FAILED");
	}
	if result[2].is_ok() {
		println!("'nightly' update succeeded");
	} else {
		println!("'nightly' update FAILED");
	}
	
	println!("stable revision:");
	try!(show_tool_versions(&try!(cfg.get_toolchain("stable", false))));
	println!("beta revision:");
	try!(show_tool_versions(&try!(cfg.get_toolchain("beta", false))));
	println!("nightly revision:");
	try!(show_tool_versions(&try!(cfg.get_toolchain("nightly", false))));
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
