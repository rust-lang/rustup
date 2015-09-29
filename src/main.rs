
#[macro_use]
extern crate clap;
extern crate rand;
extern crate regex;
extern crate hyper;
extern crate multirust;

use clap::{App, ArgMatches};
use std::env;
use std::path::{Path, PathBuf};
use std::io::BufRead;
use std::process::Command;
use std::ffi::OsString;
use multirust::*;

fn set_globals(matches: Option<&ArgMatches>) -> Result<Cfg> {
	// Base config
	let verbose = matches.map(|m| m.is_present("verbose")).unwrap_or(false);
	Cfg::from_env(NotifyHandler::from(move |n: Notification| {
		if verbose || !n.is_verbose() {
			println!("{}", n);
		}
	}))
		
}

fn main() {
	if let Err(e) = try_main() {
		println!("error: {}", e);
		std::process::exit(1);
	}
}

fn try_main() -> Result<()> {
	let mut arg_iter = env::args_os();
	let arg0 = PathBuf::from(arg_iter.next().unwrap());
	let arg0_stem = arg0.file_stem().expect("invalid multirust invocation")
		.to_str().expect("don't know how to proxy that binary");
	
	match arg0_stem {
		"multirust" | "multirust-rs" => {
			let arg1 = arg_iter.next();
			if let Some("run") = arg1.as_ref().and_then(|s| s.to_str()) {
				let arg2 = arg_iter.next().expect("expected binary name");
				let stem = arg2.to_str().expect("don't know how to proxy that binary");
				if !stem.starts_with("-") {
					run_proxy(stem, arg_iter)
				} else {
					run_multirust()
				}
			} else {
				run_multirust()
			}
		},
		"rustc" | "rustdoc" | "cargo" | "rust-lldb" | "rust-gdb" => {
			run_proxy(arg0_stem, arg_iter)
		},
		other => {
			Err(Error::Custom { id: "no-proxy".to_owned(), desc: format!("don't know how to proxy that binary: {}", other) })
		},
	}
}

fn current_dir() -> Result<PathBuf> {
	env::current_dir().map_err(|_| Error::LocatingWorkingDir)
}

fn run_proxy<I: Iterator<Item=OsString>>(binary: &str, arg_iter: I) -> Result<()> {
	let cfg = try!(set_globals(None));
	
	let mut command = try!(cfg.create_command_for_dir(&try!(current_dir()), binary));
	
	for arg in arg_iter {
		command.arg(arg);
	}
	let result = command.status()
		.ok().expect(&format!("failed to run `{}`", binary));
			
	// Ensure correct exit code is returned
	std::process::exit(result.code().unwrap_or(1));
}

fn run_multirust() -> Result<()> {
	let yaml = load_yaml!("cli.yml");
	let app_matches = App::from_yaml(yaml).get_matches();
	
	let cfg = try!(set_globals(Some(&app_matches)));
	
	match app_matches.subcommand_name() {
		Some("upgrade-data")|Some("delete-data") => {}, // Don't need consistent metadata
		Some(_) => { try!(cfg.check_metadata_version()); },
		_ => {},
	}
	
	match app_matches.subcommand() {
		("update", Some(matches)) => update(&cfg, matches),
		("default", Some(matches)) => default_(&cfg, matches),
		("override", Some(matches)) => override_(&cfg, matches),
		("show-default", Some(_)) => show_default(&cfg),
		("show-override", Some(_)) => show_override(&cfg),
		("list-overrides", Some(_)) => list_overrides(&cfg),
		("list-toolchains", Some(_)) => list_toolchains(&cfg),
		("remove-override", Some(matches)) => remove_override(&cfg, matches),
		("remove-toolchain", Some(matches)) => remove_toolchain_args(&cfg, matches),
		("upgrade-data", Some(_)) => cfg.upgrade_data().map(|_|()),
		("delete-data", Some(matches)) => delete_data(&cfg, matches),
		("which", Some(matches)) => which(&cfg, matches),
		("ctl", Some(matches)) => ctl(&cfg, matches),
		("doc", Some(matches)) => doc(&cfg, matches),
		_ => Ok(()),
	}
}

fn remove_toolchain_args(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	cfg.remove_toolchain(matches.value_of("toolchain").unwrap())
}

fn default_(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	let toolchain = matches.value_of("toolchain").unwrap();
	if !try!(common_install_args(cfg, "default", matches)) {
		try!(cfg.install_toolchain_if_not_installed(toolchain));
	}
	
	cfg.set_default_toolchain(toolchain)
}

fn override_(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	let toolchain = matches.value_of("toolchain").unwrap();
	if !try!(common_install_args(cfg, "override", matches)) {
		try!(cfg.install_toolchain_if_not_installed(toolchain));
	}
	
	cfg.set_override(&try!(current_dir()), toolchain)
}

fn common_install_args(cfg: &Cfg, _: &str, matches: &ArgMatches) -> Result<bool> {
	let toolchain = matches.value_of("toolchain").unwrap();
	
	if let Some(installers) = matches.values_of("installer") {
		let is: Vec<_> = installers.iter().map(|i| i.as_ref()).collect();
		try!(cfg.update_custom_toolchain_from_installers(toolchain, &*is));
	} else if let Some(path) = matches.value_of("copy-local") {
		try!(cfg.update_custom_toolchain_from_dir(toolchain, Path::new(path), false));
	} else if let Some(path) = matches.value_of("link-local") {
		try!(cfg.update_custom_toolchain_from_dir(toolchain, Path::new(path), true));
	} else {
		return Ok(false);
	}
	Ok(true)
}

fn doc(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	cfg.open_docs_for_dir(&try!(current_dir()), matches.is_present("all"))
}

fn ctl_home(cfg: &Cfg) -> Result<()> {
	println!("{}", cfg.multirust_dir.display());
	Ok(())
}

fn ctl_overide_toolchain(cfg: &Cfg) -> Result<()> {
	let (toolchain, _, _) = try!(cfg.toolchain_for_dir(&try!(current_dir())));
	
	println!("{}", toolchain);
	Ok(())
}

fn ctl_default_toolchain(cfg: &Cfg) -> Result<()> {
	let (toolchain, _) = try!(try!(cfg.find_default()).ok_or(Error::NoDefaultToolchain));
	
	println!("{}", toolchain);
	Ok(())
}

fn ctl_toolchain_sysroot(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	let toolchain = matches.value_of("toolchain").unwrap();
	
	let toolchain_dir = try!(cfg.get_toolchain_dir(toolchain, false));
	println!("{}", toolchain_dir.display());
	Ok(())
}

fn ctl(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	match matches.subcommand() {
		("home", Some(_)) => ctl_home(cfg),
		("override-toolchain", Some(_)) => ctl_overide_toolchain(cfg),
		("default-toolchain", Some(_)) => ctl_default_toolchain(cfg),
		("toolchain-sysroot", Some(matches)) => ctl_toolchain_sysroot(cfg, matches),
		_ => Ok(()),
	}
}

fn which(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	let binary = matches.value_of("binary").unwrap();
	
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

fn delete_data(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	if !matches.is_present("no-prompt") {
		print!("This will delete all toolchains, overrides, aliases, and other multirust data associated with this user. Continue? (y/n) ");
		let input = read_line();
		
		match &*input {
			"y"|"Y" => {},
			_ => {
				println!("aborting");
				return Ok(());
			}
		}
	}
	
	cfg.delete_data()
}

fn remove_override(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	if let Some(path) = matches.value_of("override") {
		cfg.remove_override(path.as_ref())
	} else {
		cfg.remove_override(&try!(current_dir()))
	}.map(|_|())
}

fn show_tool_versions(cfg: &Cfg, toolchain: &str) -> Result<()> {
	println!("");
	if try!(cfg.is_toolchain_installed(toolchain)) {
		let toolchain_dir = try!(cfg.get_toolchain_dir(toolchain, false));
		let rustc_path = toolchain_dir.join(&cfg.bin_path("rustc"));
		let cargo_path = toolchain_dir.join(&cfg.bin_path("cargo"));

		try!(cfg.with_toolchain_ldpath(toolchain, || {
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
	if let Some((toolchain, sysroot)) = try!(cfg.find_default()) {
		println!("default toolchain: {}", &toolchain);
		println!("default location: {}", sysroot.display());
		
		show_tool_versions(cfg, &toolchain)
	} else {
		println!("no default toolchain configured. run `multirust helpdefault`");
		Ok(())
	}
}

fn show_override(cfg: &Cfg) -> Result<()> {
	if let Some((toolchain, sysroot, reason)) = try!(cfg.find_override(&try!(current_dir()))) {
		println!("override toolchain: {}", &toolchain);
		println!("override location: {}", sysroot.display());
		println!("override reason: {}", reason);
		
		show_tool_versions(cfg, &toolchain)
	} else {
		println!("no override");
		show_default(cfg)
	}
}

fn list_overrides(cfg: &Cfg) -> Result<()> {
	let mut overrides = try!(cfg.list_overrides());
		
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
	try!(show_tool_versions(cfg, "stable"));
	println!("beta revision:");
	try!(show_tool_versions(cfg, "beta"));
	println!("nightly revision:");
	try!(show_tool_versions(cfg, "nightly"));
	Ok(())
}

fn update(cfg: &Cfg, matches: &ArgMatches) -> Result<()> {
	if let Some(toolchain) = matches.value_of("toolchain") {
		if !try!(common_install_args(cfg, "update", matches)) {
			try!(cfg.update_toolchain(toolchain))
		}
	} else {
		try!(update_all_channels(cfg))
	}
	Ok(())
}
