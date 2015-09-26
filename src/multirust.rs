#![feature(fs_canonicalize)]

#[macro_use]
extern crate clap;
extern crate rand;
extern crate regex;
extern crate hyper;

use clap::{App, ArgMatches};
use std::env;
use std::path::{Path, PathBuf};
use std::fs;
use std::borrow::Cow;
use std::io;
use std::io::{Read, Write, BufRead};
use regex::Regex;
use rand::random;
use hyper::Client;
use std::ops;
use std::process::{Command, Stdio};
use std::ffi::{OsStr, OsString};
use std::cell::RefCell;
use std::collections::HashMap;

struct BaseCfg {
	verbose: bool,
}

struct Cfg {
	base: BaseCfg,
	home_dir: PathBuf,
	// Use 'str' rather than 'Path' because this is a remote path, and so
	// restrict to valid UTF-8
	dist_dir: Cow<'static, str>,
	multirust_dir: PathBuf,
	version_file: PathBuf,
	override_db: PathBuf,
	default_file: PathBuf,
	toolchains_dir: PathBuf,
	update_hash_dir: PathBuf,
	temp_dir: PathBuf,
	delim: Cow<'static, str>,
	gpg_key: Cow<'static, str>,
	var_stack: RefCell<HashMap<&'static str, Vec<Option<OsString>>>>,
	metadata_version: &'static str,
}

struct TempDir(PathBuf);
struct TempFile(PathBuf);

impl TempDir {
	fn new(cfg: &Cfg) -> Self {
		loop {
			let temp_name = random_string(16) + "_dir";
			
			let temp_dir = cfg.temp_dir.join(temp_name);
			
			// This is technically racey, but the probability of getting the same
			// random names at exactly the same time is... low.
			if !is_directory(&temp_dir) {
				ensure_dir_exists(&cfg.base, &temp_dir, "temp");
				return TempDir(temp_dir);
			}
		}
	}
}

impl TempFile {
	fn new(cfg: &Cfg) -> Self {
		loop {
			let temp_name = random_string(16) + "_file";
			
			ensure_dir_exists(&cfg.base, &cfg.temp_dir, "temp");
			
			let temp_file = cfg.temp_dir.join(temp_name);
			
			// This is technically racey, but the probability of getting the same
			// random names at exactly the same time is... low.
			if !is_file(&temp_file) {
				fs::File::create(&temp_file).ok().expect("could not create temp file");
				return TempFile(temp_file);
			}
		}
	}
}

impl ops::Deref for TempDir {
	type Target = Path;
	
	fn deref(&self) -> &Path {
		ops::Deref::deref(&self.0)
	}
}

impl ops::Deref for TempFile {
	type Target = Path;
	
	fn deref(&self) -> &Path {
		ops::Deref::deref(&self.0)
	}
}

impl Drop for TempDir {
	fn drop(&mut self) {
		if is_directory(&self.0) {
			if let Err(_) = fs::remove_dir_all(&self.0) {
				say(&format!("error: failed to remove temp directory '{}'", self.display()));
			}
		}
	}
}

impl Drop for TempFile {
	fn drop(&mut self) {
		if is_file(&self.0) {
			if let Err(_) = fs::remove_dir_all(&self.0) {
				say(&format!("error: failed to remove temp file '{}'", self.display()));
			}
		}
	}
}

fn to_absolute<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
	env::current_dir().map(|mut v| {
			v.push(path);
			v
		}).ok()
}

fn if_not_empty<S: PartialEq<str>>(s: S) -> Option<S> {
	if s == *"" {
		None
	} else {
		Some(s)
	}
}

fn ensure_dir_exists<P: AsRef<Path>>(cfg: &BaseCfg, path: P, name: &str) {
	verbose_say(cfg, &format!("creating {} directory '{}'", name, path.as_ref().display()));
	fs::create_dir_all(path.as_ref()).ok()
		.expect(&format!("failed to create {} directory '{}'", name, path.as_ref().display()));
}

fn set_globals(matches: Option<&ArgMatches>) -> Cfg {
	// Base config
	let verbose = matches.map(|m| m.is_present("verbose")).unwrap_or(false);
	let base = BaseCfg {
		verbose: verbose,
	};
	
	// Get absolute home directory
	let home_dir = env::home_dir()
		.map(PathBuf::from)
		.and_then(to_absolute)
		.expect("could not locate home directory");
		
	// The directory on the server containing the dist artifacts
	let dist_dir = Cow::Borrowed("dist");
	
	// Set up the multirust home directory
	let multirust_dir = env::var_os("MULTIRUST_HOME")
		.and_then(if_not_empty)
		.map(PathBuf::from)
		.unwrap_or_else(|| home_dir.join(".multirust"));
		
	ensure_dir_exists(&base, &multirust_dir, "home");
	
	// Export RUSTUP_HOME to configure rustup.sh to store it's stuff
	// in the MULTIRUST_HOME directory.
	env::set_var("RUSTUP_HOME", multirust_dir.join("rustup"));
	
	// Data locations
	let version_file = multirust_dir.join("version");
	let override_db = multirust_dir.join("overrides");
	let default_file = multirust_dir.join("default");
	let toolchains_dir = multirust_dir.join("toolchains");
	let update_hash_dir = multirust_dir.join("update-hashes");
	let temp_dir = multirust_dir.join("tmp");
	
	// Used for delimiting fields in override_db
	let delim = Cow::Borrowed(";");
	
	// GPG key
	let gpg_key = if let Some(path) = env::var_os("MULTIRUST_GPG_KEY").and_then(if_not_empty) {
		Cow::Owned(read_file(Path::new(&path)).ok().expect("could not read public key file"))
	} else {
		Cow::Borrowed(include_str!("rust-key.gpg.ascii"))
	};
	
	Cfg {
		base: base,
		home_dir: home_dir,
		dist_dir: dist_dir,
		multirust_dir: multirust_dir,
		version_file: version_file,
		override_db: override_db,
		default_file: default_file,
		toolchains_dir: toolchains_dir,
		update_hash_dir: update_hash_dir,
		temp_dir: temp_dir,
		delim: delim,
		gpg_key: gpg_key,
		var_stack: RefCell::new(HashMap::new()),
		metadata_version: "2",
	}
}

fn main() {
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
					run_proxy(stem, arg_iter);
				} else {
					run_multirust();
				}
			} else {
				run_multirust();
			}
		},
		"rustc" | "rustdoc" | "cargo" | "rust-lldb" | "rust-gdb" => {
			run_proxy(arg0_stem, arg_iter);
		},
		other => {
			panic!("don't know how to proxy that binary: {}", other);
		},
	}
}

fn default_var<S: AsRef<OsStr>>(name: &str, value: S) {
	if env::var_os(name).and_then(if_not_empty).is_none() {
		env::set_var(name, value);
	}
}

fn run_proxy<I: Iterator<Item=OsString>>(binary: &str, arg_iter: I) {
	let cfg = set_globals(None);
	
	let (toolchain, toolchain_dir, _) = find_override_toolchain_or_default(&cfg)
		.expect("no default toolchain configured");
	
	let binary_path = toolchain_dir.join(bin_path(binary));
	
	if is_file(&binary_path) {
		push_toolchain_ldpath(&cfg, &toolchain);
		
		default_var("CARGO_HOME", &toolchain_dir.join("cargo"));
		env::set_var("MULTIRUST_TOOLCHAIN", &toolchain);
		env::set_var("MULTIRUST_HOME", &cfg.multirust_dir);
		
		let mut command = Command::new(&binary_path);
		for arg in arg_iter {
			command.arg(arg);
		}
		let result = command.status()
			.ok().expect(&format!("failed to run `{}`", binary_path.display()));
			
		// Ensure correct exit code is returned
		std::process::exit(result.code().unwrap_or(1));
	} else {
		say_err(&format!("command \"{}\" does not exist", binary_path.display()));
		panic!();
	}
}

fn run_multirust() {
	let yaml = load_yaml!("cli.yml");
	let app_matches = App::from_yaml(yaml).get_matches();
	
	let cfg = set_globals(Some(&app_matches));
	
	match app_matches.subcommand_name() {
		Some("upgrade-data")|Some("delete-data") => {}, // Don't need consistent metadata
		Some(_) => { check_metadata_version(&cfg) },
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
		("upgrade-data", Some(_)) => upgrade_data(&cfg),
		("delete-data", Some(matches)) => delete_data(&cfg, matches),
		("which", Some(matches)) => which(&cfg, matches),
		("ctl", Some(matches)) => ctl(&cfg, matches),
		("doc", Some(matches)) => doc(&cfg, matches),
		_ => {},
	}
}

fn remove_toolchain_args(cfg: &Cfg, matches: &ArgMatches) {
	remove_toolchain(cfg, matches.value_of("toolchain").unwrap());
}

fn say(msg: &str) {
	println!("{}", msg);
}

fn say_err(msg: &str) {
	writeln!(&mut std::io::stderr(), "{}", msg);
}


fn open_browser(path: &Path) {
	#[cfg(not(windows))]
	fn has_cmd(cmd: &str) -> bool {
		Command::new("command")
			.arg("-v").arg(cmd)
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.ok().expect("failed to check for command")
			.succeeded()
	}
	#[cfg(not(windows))]
	fn inner(path: &Path) {
		let commands = ["xdg-open", "open", "firefox", "chromium"];
		if let Some(cmd) = commands.filter(has_cmd).next() {
			Command::new(cmd)
				.arg(path)
				.stdin(Stdio::null())
				.stdout(Stdio::null())
				.stderr(Stdio::null())
				.spawn()
				.ok().expect("failed to launch browser");
		} else {
			panic!("Need xdg-open, open, firefox, or chromium");
		}
	}
	#[cfg(windows)]
	fn inner(path: &Path) {
		Command::new("start")
			.arg(path)
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.ok().expect("failed to launch browser");
	}
	inner(path);
}

fn verbose_say(cfg: &BaseCfg, msg: &str) {
	if cfg.verbose {
		say(msg);
	}
}

fn is_directory<P: AsRef<Path>>(path: P) -> bool {
	fs::metadata(path).ok().as_ref().map(fs::Metadata::is_dir) == Some(true)
}

fn is_file<P: AsRef<Path>>(path: P) -> bool {
	fs::metadata(path).ok().as_ref().map(fs::Metadata::is_file) == Some(true)
}

fn download_file<P: AsRef<Path>>(url: hyper::Url, path: P) -> Result<(),()> {
	let client = Client::new();

	let mut res = try!(client.get(url).send().map_err(|_|()));
	if res.status != hyper::Ok { return Err(()); }
	
	let buffer_size = 0x1000;
	let mut buffer = vec![0u8; buffer_size];
	
	let mut file = try!(fs::File::create(path).map_err(|_|()));
	
	loop {
		let bytes_read = try!(io::Read::read(&mut res, &mut buffer).map_err(|_|()));
		if bytes_read != 0 {
			try!(io::Write::write_all(&mut file, &mut buffer[0..bytes_read]).map_err(|_|()));
		} else {
			try!(file.sync_data().map_err(|_|()));
			return Ok(());
		}
	}
}

fn get_toolchain_dir(cfg: &Cfg, toolchain: &str, create_parent: bool) -> PathBuf {
	if create_parent {
		ensure_dir_exists(&cfg.base, &cfg.toolchains_dir, "toolchains");
	}
	
	cfg.toolchains_dir.join(toolchain)
}

fn get_hash_file(cfg: &Cfg, toolchain: &str, create_parent: bool) -> PathBuf {
	if create_parent {
		ensure_dir_exists(&cfg.base, &cfg.update_hash_dir, "update-hash");
	}
	
	cfg.toolchains_dir.join(toolchain)
}

fn random_string(length: usize) -> String {
	let chars = b"abcdefghijklmnopqrstuvwxyz0123456789_";
	(0..length).map(|_| std::char::from_u32(chars[random::<usize>() % chars.len()] as u32).unwrap()).collect()
}

fn is_toolchain_installed(cfg: &Cfg, toolchain: &str) -> bool {
	verbose_say(&cfg.base, &format!("looking for installed toolchain '{}'", toolchain));
	
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, false);
	is_directory(toolchain_dir)
}

fn call_rustup(toolchain_dir: &Path, toolchain: &str, hash_file: &Path) -> bool {
	let rustup = env::current_exe()
		.ok().expect("could not find location of self")
		.with_file_name("rustup");
	
	Command::new(rustup)
		.arg("--prefix").arg(toolchain_dir)
		.arg("--spec").arg(toolchain)
		.arg("--update-hash-file").arg(hash_file)
		.arg("--disable-ldconfig")
		.arg("-y").arg("--disable-sudo")
		.status()
		.ok().expect("could not run `rustup`")
		.success()
}

fn install_toolchain_from_dist(cfg: &Cfg, toolchain: &str) -> bool {
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, true);
	let hash_file = get_hash_file(cfg, toolchain, true);
	
	verbose_say(&cfg.base, &format!("toolchain dir is '{}'", toolchain_dir.display()));
	call_rustup(&toolchain_dir, toolchain, &hash_file)
}

fn update_toolchain(cfg: &Cfg, toolchain: &str) -> bool {
	if is_toolchain_installed(cfg, toolchain) {
		say(&format!("updating existing install for '{}'", toolchain));
	} else {
		say(&format!("installing toolchain '{}'", toolchain));
	}
	install_toolchain_from_dist(cfg, toolchain)
}

fn install_toolchain_if_not_installed(cfg: &Cfg, toolchain: &str) {
	if is_toolchain_installed(cfg, toolchain) {
		say(&format!("using existing install for '{}'", toolchain));
	} else {
		update_toolchain(cfg, toolchain);
	}
}

fn write_file(path: &Path, contents: &str) -> io::Result<()> {
	let mut file = try!(fs::OpenOptions::new()
		.write(true)
		.truncate(true)
		.create(true)
		.open(path));
	
	try!(io::Write::write_all(&mut file, contents.as_bytes()));
	
	try!(file.sync_data());
	
	Ok(())
}

fn read_file(path: &Path) -> io::Result<String> {
	let mut file = try!(fs::OpenOptions::new()
		.read(true)
		.open(path));
	
	let mut contents = String::new();
	
	try!(io::Read::read_to_string(&mut file, &mut contents));
	
	Ok(contents)
}

fn set_default(cfg: &Cfg, toolchain: &str) {
	let work_file = TempFile::new(cfg);
	
	write_file(&work_file, toolchain)
		.ok().expect("couldn't write default toolchain to tempfile");
	
	fs::rename(&*work_file, &cfg.default_file)
		.ok().expect("couldn't set default toolchain");
	
	say(&format!("default toolchain set to '{}'", toolchain));
}

fn default_(cfg: &Cfg, matches: &ArgMatches) {
	let toolchain = matches.value_of("toolchain").unwrap();
	if !common_install_args(cfg, "default", matches) {
		install_toolchain_if_not_installed(cfg, toolchain);
	}
	
	set_default(cfg, toolchain);
}

fn filter_file<F: FnMut(&str) -> bool>(src: &Path, dest: &Path, mut filter: F) -> io::Result<usize> {
	let src_file = try!(fs::File::open(src));
	let dest_file = try!(fs::File::create(dest));
	
	let mut reader = io::BufReader::new(src_file);
	let mut writer = io::BufWriter::new(dest_file);
	let mut removed = 0;
	
	for result in io::BufRead::lines(&mut reader) {
		let line = try!(result);
		if filter(&line) {
			try!(writeln!(&mut writer, "{}", &line));
		} else {
			removed += 1;
		}
	}
	
	try!(writer.flush());
	
	Ok(removed)
}

fn match_file<T, F: FnMut(&str) -> Option<T>>(src: &Path, mut f: F) -> io::Result<Option<T>> {
	let src_file = try!(fs::File::open(src));
	
	let mut reader = io::BufReader::new(src_file);
	
	for result in io::BufRead::lines(&mut reader) {
		let line = try!(result);
		if let Some(r) = f(&line) {
			return Ok(Some(r));
		}
	}
	
	Ok(None)
}

fn append_file(dest: &Path, line: &str) -> io::Result<()> {
	let mut dest_file = try!(fs::OpenOptions::new()
		.write(true)
		.append(true)
		.create(true)
		.open(dest)
		);
	
	try!(writeln!(&mut dest_file, "{}", line));
	
	try!(dest_file.sync_data());
	
	Ok(())
}

fn path_to_db_key(cfg: &Cfg, path: &Path) -> String {
	fs::canonicalize(path).ok().expect("could not canonicalize path")
		.display().to_string() + &cfg.delim
}

fn set_override(cfg: &Cfg, toolchain: &str) {
	let path = current_dir();
	let key = path_to_db_key(cfg, &path);
	
	let work_file = TempFile::new(cfg);
	
	if is_file(&cfg.override_db) {
		filter_file(&cfg.override_db, &work_file, |line| {
			!line.starts_with(&key)
		}).ok().expect("unable to edit override db");
	}
	
	append_file(&work_file, &(key + toolchain))
		.ok().expect("unable to edit override db");
		
	fs::rename(&*work_file, &cfg.override_db)
		.ok().expect("unable to edit override db");
		
	say(&format!("override toolchain for '{}' set to '{}'", path.display(), toolchain));
}

fn override_(cfg: &Cfg, matches: &ArgMatches) {
	let toolchain = matches.value_of("toolchain").unwrap();
	if !common_install_args(cfg, "override", matches) {
		install_toolchain_if_not_installed(cfg, toolchain);
	}
	
	set_override(cfg, toolchain);
}

#[cfg(windows)]
fn bin_path(name: &str) -> String { format!("bin\\{}.exe", name) }
#[cfg(not(windows))]
fn bin_path(name: &str) -> String { format!("bin/{}", name) }

fn symlink_dir(src: &Path, dest: &Path) -> Option<()> {
	#[cfg(windows)]
	fn symlink_dir_inner(src: &Path, dest: &Path) -> Option<()> {
		std::os::windows::fs::symlink_dir(src, dest).ok()
	}
	#[cfg(not(windows))]
	fn symlink_dir_inner(src: &Path, dest: &Path) -> Option<()> {
		std::os::unix::fs::symlink(src, dest).ok()
	}
	
	symlink_dir_inner(src, dest)
}

fn copy_dir(src: &Path, dest: &Path) -> Option<()> {
	#[cfg(windows)]
	fn copy_dir_inner(src: &Path, dest: &Path) -> Option<()> {
		Command::new("robocopy")
			.arg(src).arg(dest).arg("/E")
			.status().ok()
			.and_then(|code| if code.success() { Some(()) } else { None })
	}
	#[cfg(not(windows))]
	fn copy_dir_inner(src: &Path, dest: &Path) -> Option<()> {
		Command::new("cp")
			.arg("-R").arg(src).arg(dest)
			.status().ok()
			.and_then(|code| if code.success() { Some(()) } else { None })
	}
	
	copy_dir_inner(src, dest)
}

fn update_custom_toolchain_from_dir(cfg: &Cfg, toolchain: &str, path: &Path, link: bool) {
	assert!(is_directory(path), "specified path does not exist: '{}'", path.display());
	
	let expected_rustc = path.join(&bin_path("rustc"));
	assert!(is_file(&expected_rustc), "no rustc in custom toolchain at '{}'", expected_rustc.display());
	
	maybe_remove_existing_custom_toolchain(cfg, toolchain);
	
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, true);
	let abs_path = to_absolute(path).expect("could not convert to absolute path");
	
	if link {
		say(&format!("creating link from '{}'", abs_path.display()));
		symlink_dir(&toolchain_dir, &abs_path)
			.expect("failed to create link to toolchain");
	} else {
		say(&format!("copying from '{}'", abs_path.display()));
		copy_dir(&abs_path, &toolchain_dir)
			.expect("failed to copy toolchain directory");
	}
}

fn common_install_args(cfg: &Cfg, cmd: &str, matches: &ArgMatches) -> bool {
	let toolchain = matches.value_of("toolchain").unwrap();
	
	verbose_say(&cfg.base, &format!("cmd: {}", cmd));
	verbose_say(&cfg.base, &format!("toolchain: {}", toolchain));
	
	if let Some(installers) = matches.values_of("installer") {
		update_custom_toolchain_from_installers(cfg, toolchain, installers);
	}
	else if let Some(path) = matches.value_of("copy-local") {
		update_custom_toolchain_from_dir(cfg, toolchain, Path::new(path), false);
	}
	else if let Some(path) = matches.value_of("link-local") {
		update_custom_toolchain_from_dir(cfg, toolchain, Path::new(path), true);
	} else {
		return false;
	}
	true
}

fn check_custom_toolchain_name(toolchain: &str) {
	let re = Regex::new(r"^(nightly|beta|stable)(-\d{4}-\d{2}-\d{2})?$").unwrap();
	assert!(!re.is_match(toolchain), "invalid custom toolchain name: '{}'", toolchain);
}

fn remove_toolchain(cfg: &Cfg, toolchain: &str) {
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, false);
	
	if is_directory(&toolchain_dir) {
		verbose_say(&cfg.base, &format!("removing directory '{}'", toolchain_dir.display()));
		
		fs::remove_dir_all(&toolchain_dir)
			.ok().expect("failed to remove toolchain");
		
		say(&format!("toolchain '{}' uninstalled", toolchain));
	} else {
		say(&format!("no toolchain installed for '{}'", toolchain));
	}
	
	let hash_file = get_hash_file(cfg, toolchain, false);
	if is_file(&hash_file) {
		fs::remove_file(&hash_file)
			.ok().expect("failed to remove update hash");
	}
}

fn maybe_remove_existing_custom_toolchain(cfg: &Cfg, toolchain: &str) {
	if is_toolchain_installed(cfg, toolchain) {
		say("removing existing toolchain before the update");
		remove_toolchain(cfg, toolchain);
	}
}

fn install_toolchain_tar_gz(cfg: &Cfg, toolchain: &str, installer: &Path, work_dir: &Path) {
	let installer_dir = Path::new(Path::new(installer.file_stem().unwrap()).file_stem()
		.expect("unrecognized file extension, expected '.tar.gz'"));
	let result = Command::new("tar")
		.arg("xzf").arg(installer)
		.arg("-C").arg(work_dir)
		.status().ok()
		.expect("failed to run `tar` while extracting installer").success();
	
	assert!(result, "failed to extract installer");
	
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, true);
	verbose_say(&cfg.base, &format!("installing toolchain to '{}'", toolchain_dir.display()));
	say(&format!("installing toolchain for '{}'", toolchain));
	
	let result = Command::new("sh").arg(installer_dir.join("install.sh"))
		.arg("--prefix").arg(&toolchain_dir)
		.arg("--disable-ldconfig")
		.status().ok()
		.expect("failed to run `sh` while installing toolchain").success();
		
	if !result {
		let _ = fs::remove_dir_all(&toolchain_dir);
		panic!("failed to install toolchain");
	}
}

fn prefix_arg<S: AsRef<OsStr>>(name: &str, s: S) -> OsString {
	let mut arg = OsString::from(name);
	arg.push(s);
	arg
}

fn install_toolchain_msi(cfg: &Cfg, toolchain: &str, installer: &Path, work_dir: &Path) {
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, true);
	verbose_say(&cfg.base, &format!("installing toolchain to '{}'", toolchain_dir.display()));
	say(&format!("installing toolchain for '{}'", toolchain));

	let inner = || -> Result<(),&'static str> {
		// Can't install to same folder as installer, so create subfolder
		let install_dir = work_dir.join("install");
		
		let installer_arg = installer;
		let target_arg = prefix_arg("TARGETDIR=", &install_dir);
		
		let mut command = Command::new("msiexec");
		command
			.arg("/a").arg(&installer_arg)
			.arg("/qn")
			.arg(&target_arg);
			
		verbose_say(&cfg.base, &format!("command: {:?}", &command));
		
		// Extract the MSI to the subfolder
		let result = try!(command.status().map_err(|_|"could not run msiexec")).success();
			
		if !result {
			return Err("could not extract from msi");
		}
		
		// Find the root Rust folder within the subfolder
		let root_dir = try!(try!(fs::read_dir(install_dir).map_err(|_|"could not inspect install directory"))
			.filter_map(Result::ok)
			.map(|e| e.path())
			.filter(|p| is_directory(&p))
			.next()
			.ok_or("could not locate rust directory within msi"));
			
		// Rename and move it to the toolchain directory
		try!(fs::rename(&root_dir, &toolchain_dir).map_err(|_|"could not rename directory"));
			
		Ok(())
	};
	
	if let Err(msg) = inner() {
		let _ = fs::remove_dir_all(&toolchain_dir);
		panic!("failed to install toolchain: {}", msg);
	}
}

fn install_toolchain(cfg: &Cfg, toolchain: &str, installer: &Path, work_dir: &Path) {
	match installer.extension().and_then(OsStr::to_str) {
		Some("gz") => install_toolchain_tar_gz(cfg, toolchain, installer, work_dir),
		Some("msi") => install_toolchain_msi(cfg, toolchain, installer, work_dir),
		ext => panic!("don't know how to handle the extension '{}'", ext.unwrap_or("")),
	}
}

fn update_custom_toolchain_from_installers(cfg: &Cfg, toolchain: &str, installers: Vec<&str>) {
	check_custom_toolchain_name(toolchain);
	maybe_remove_existing_custom_toolchain(cfg, toolchain);
	
	let work_dir = TempDir::new(cfg);
	
	verbose_say(&cfg.base, &format!("download work dir: {}", work_dir.display()));
	
	for installer in installers {
		let local_installer;
		if let Ok(url) = hyper::Url::parse(&installer) {
			// If installer is a URL
			
			// Extract basename from url (eg. 'rust-1.3.0-x86_64-unknown-linux-gnu.tar.gz')
			let re = Regex::new(r"[\\/]([^\\/?]+)(\?.*)?$").unwrap();
			let basename = re.captures(&installer).expect("invalid basename in url").at(1).unwrap();
			
			// Download to a local file
			local_installer = Cow::Owned(work_dir.join(basename));
			download_file(url, &local_installer)
				.ok().expect("failed to download toolchain");
		} else {
			// If installer is a filename
			
			// No need to download
			local_installer = Cow::Borrowed(installer.as_ref());
		}
		
		// Install from file
		install_toolchain(cfg, toolchain, &local_installer, &work_dir);
	}
}

fn doc(cfg: &Cfg, matches: &ArgMatches) {
	let (_, toolchain_dir, _) = find_override_toolchain_or_default(cfg)
		.expect("no default toolchain configured");
	let show_all = matches.is_present("all");
	let parts = if show_all {
		vec!["share", "doc", "rust", "html", "std", "index.html"]
	} else {
		vec!["share", "doc", "rust", "html", "index.html"]
	};
	let mut doc_dir = toolchain_dir;
	for part in parts {
		doc_dir.push(part);
	}
	
	open_browser(&doc_dir);
}

fn ctl_home(cfg: &Cfg) {
	println!("{}", cfg.multirust_dir.display());
}

fn ctl_overide_toolchain(cfg: &Cfg) {
	let (toolchain, _, _) = find_override_toolchain_or_default(cfg)
		.expect("no default toolchain configured");
	
	println!("{}", toolchain);
}

fn ctl_default_toolchain(cfg: &Cfg) {
	let (toolchain, _) = find_default(cfg)
		.expect("no default toolchain configured");
	
	println!("{}", toolchain);
}

fn ctl_toolchain_sysroot(cfg: &Cfg, matches: &ArgMatches) {
	let toolchain = matches.value_of("toolchain").unwrap();
	
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, false);
	println!("{}", toolchain_dir.display());
}

fn ctl(cfg: &Cfg, matches: &ArgMatches) {
	match matches.subcommand() {
		("home", Some(_)) => ctl_home(cfg),
		("override-toolchain", Some(_)) => ctl_overide_toolchain(cfg),
		("default-toolchain", Some(_)) => ctl_default_toolchain(cfg),
		("toolchain-sysroot", Some(matches)) => ctl_toolchain_sysroot(cfg, matches),
		_ => {},
	}
}

fn which(cfg: &Cfg, matches: &ArgMatches) {
	let binary = matches.value_of("binary").unwrap();
	
	let (_, toolchain_dir, _) = find_override_toolchain_or_default(cfg)
		.expect("no default toolchain configured");
	
	let binary_path = toolchain_dir.join(bin_path(binary));
	
	if is_file(&binary_path) {
		println!("{}", binary_path.display());
	} else {
		say_err(&format!("command \"{}\" does not exist", binary_path.display()));
		panic!();
	}
}

fn upgrade_data(cfg: &Cfg) {
	if !is_file(&cfg.version_file) {
		say(&format!("no need to upgrade. {} does not exist", cfg.multirust_dir.display()));
		return;
	}
	
	let current_version = read_file(&cfg.version_file)
		.ok().expect("failed to read metadata version");
	
	say(&format!("upgrading metadata from version {} to {}", &current_version, cfg.metadata_version));
	match &*current_version {
		"2" => {
			// Current version. Do nothing
			say(&format!("metadata is updated to version {}", current_version));
		},
		"1" => {
			// Ignore errors. These files may not exist.
			let _ = fs::remove_dir_all(cfg.multirust_dir.join("available-updates"));
			let _ = fs::remove_dir_all(cfg.multirust_dir.join("update-sums"));
			let _ = fs::remove_dir_all(cfg.multirust_dir.join("channel-sums"));
			let _ = fs::remove_dir_all(cfg.multirust_dir.join("manifests"));
			
			write_file(&cfg.version_file, &format!("{}", cfg.metadata_version))
				.ok().expect("failed to write metadata version");
		}
		ver => {
			panic!("unknown metadata version: {}", ver);
		}
	}
	
	say("success");
}

fn read_line() -> String {
	let stdin = std::io::stdin();
	let stdin = stdin.lock();
	let mut lines = stdin.lines();
	lines.next().unwrap().unwrap()
}

fn delete_data(cfg: &Cfg, matches: &ArgMatches) {
	if !matches.is_present("no-prompt") {
		print!("This will delete all toolchains, overrides, aliases, and other multirust data associated with this user. Continue? (y/n) ");
		let input = read_line();
		
		match &*input {
			"y"|"Y" => {},
			_ => {
				say("aborting");
				return;
			}
		}
	}
	
	fs::remove_dir_all(&cfg.multirust_dir)
		.ok().expect(&format!("failed to delete '{}'", cfg.multirust_dir.display()));
	
	say(&format!("deleted {}", cfg.multirust_dir.display()));
}

fn remove_override_dir(cfg: &Cfg, path: &Path) {
	let key = path_to_db_key(cfg, &path);
	
	let work_file = TempFile::new(cfg);
	
	let removed = if is_file(&cfg.override_db) {
		filter_file(&cfg.override_db, &work_file, |line| {
			!line.starts_with(&key)
		}).ok().expect("unable to edit override db")
	} else {
		0
	};
	
	if removed > 0 {
		fs::rename(&*work_file, &cfg.override_db)
			.ok().expect("unable to edit override db");
		say(&format!("override toolchain for '{}' removed", path.display()));
	} else {
		say(&format!("no override for directory '{}'", path.display()));
	}
}

fn remove_override(cfg: &Cfg, matches: &ArgMatches) {
	if let Some(path) = matches.value_of("override") {
		remove_override_dir(cfg, path.as_ref())
	} else {
		remove_override_dir(cfg, &current_dir())
	}
}

fn current_dir() -> PathBuf {
	env::current_dir().ok().expect("could not get current directory")
}

fn verify_toolchain_dir(cfg: &Cfg, toolchain: &str) -> PathBuf {
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, false);
	assert!(is_directory(&toolchain_dir), "toolchain '{}' not installed. run `multirust update {} to install", toolchain, toolchain);
	toolchain_dir
}

fn find_default(cfg: &Cfg) -> Option<(String, PathBuf)> {
	if !is_file(&cfg.default_file) {
		return None;
	}
	
	let toolchain = read_file(&cfg.default_file)
		.ok().expect("could not read default file");
	assert!(!toolchain.is_empty(), "default file is empty");
	
	let toolchain_dir = verify_toolchain_dir(cfg, &toolchain);
	
	Some((toolchain, toolchain_dir))
}

fn find_override(cfg: &Cfg) -> Option<(String, PathBuf, Cow<'static, str>)> {
	if let Some(toolchain) = env::var("MULTIRUST_TOOLCHAIN").ok().and_then(if_not_empty) {
		let toolchain_dir = verify_toolchain_dir(cfg, &toolchain);
		
		return Some((toolchain, toolchain_dir, Cow::Borrowed("environment override by MULTIRUST_TOOLCHAIN")));
	}
	
	if !is_file(&cfg.override_db) {
		return None;
	}
	
	let dir_unresolved = current_dir();
	let dir = fs::canonicalize(&dir_unresolved)
		.ok().expect("failed to canonicalize directory");
	let mut path = &*dir;
	while let Some(parent) = path.parent() {
		let key = path_to_db_key(cfg, path);
		if let Some(toolchain) = match_file(&cfg.override_db, |line| {
			if line.starts_with(&key) {
				Some(line[key.len()..].to_owned())
			} else {
				None
			}
		}).ok().expect("extracting record from db failed") {
			let toolchain_dir = verify_toolchain_dir(cfg, &toolchain);
			
			return Some((
				toolchain,
				toolchain_dir,
				Cow::Owned(format!("directory override for '{}' via '{}'", dir_unresolved.display(), path.display()))
				));
		}
		
		path = parent;
	}
	
	None
}

fn find_override_toolchain_or_default(cfg: &Cfg) -> Option<(String, PathBuf, Cow<'static, str>)> {
	find_override(cfg).or_else(|| {
		find_default(cfg).map(|(a,b)| (a, b, Cow::Borrowed("default toolchain")))
	})
}

fn push_path_var(cfg: &Cfg, name: &'static str, value: &Path) {
	let old_value = env::var_os(name);
	let mut parts = vec![value.to_owned()];
	if let Some(ref v) = old_value {
		parts.extend(env::split_paths(v));
	}
	let new_value = env::join_paths(parts)
		.ok().expect("failed to setup environment variables");
	
	let mut stack = cfg.var_stack.borrow_mut();
	let mut v = stack.entry(name).or_insert_with(|| Vec::new());
	
	v.push(old_value);
	env::set_var(name, &new_value);
}

fn pop_path_var(cfg: &Cfg, name: &'static str) {
	let mut stack = cfg.var_stack.borrow_mut();
	let old_value = stack.get_mut(name).unwrap().pop().unwrap();
	
	if let Some(v) = old_value {
		env::set_var(name, &v);
	} else {
		env::remove_var(name);
	}
}

fn push_toolchain_ldpath(cfg: &Cfg, toolchain: &str) {
	let toolchain_dir = get_toolchain_dir(cfg, toolchain, false);
	let new_path = toolchain_dir.join("lib");
	
	push_path_var(cfg, "LD_LIBRARY_PATH", &new_path);
	push_path_var(cfg, "DYLD_LIBRARY_PATH", &new_path);
}

fn pop_toolchain_ldpath(cfg: &Cfg) {
	pop_path_var(cfg, "LD_LIBRARY_PATH");
	pop_path_var(cfg, "DYLD_LIBRARY_PATH");
}

fn show_tool_versions(cfg: &Cfg, toolchain: &str) {
	println!("");
	if is_toolchain_installed(cfg, toolchain) {
		let toolchain_dir = get_toolchain_dir(cfg, toolchain, false);
		let rustc_path = toolchain_dir.join(&bin_path("rustc"));
		let cargo_path = toolchain_dir.join(&bin_path("cargo"));
		
		push_toolchain_ldpath(cfg, toolchain);
		
		if is_file(&rustc_path) {
			Command::new(&rustc_path)
				.arg("--version")
				.status()
				.ok().expect("failed to run rustc");
		} else {
			println!("(no rustc command in toolchain?)");
		}
		if is_file(&cargo_path) {
			Command::new(&cargo_path)
				.arg("--version")
				.status()
				.ok().expect("failed to run cargo");
		} else {
			println!("(no cargo command in toolchain?)");
		}
		
		pop_toolchain_ldpath(cfg);
	} else {
		println!("(toolchain not installed)");
	}
	println!("");
}

fn show_default(cfg: &Cfg) {
	if let Some((toolchain, sysroot)) = find_default(cfg) {
		say(&format!("default toolchain: {}", &toolchain));
		say(&format!("default location: {}", sysroot.display()));
		
		show_tool_versions(cfg, &toolchain);
	} else {
		say("no default toolchain configured. run `multirust helpdefault`");
	}
}

fn show_override(cfg: &Cfg) {
	if let Some((toolchain, sysroot, reason)) = find_override(cfg) {
		say(&format!("override toolchain: {}", &toolchain));
		say(&format!("override location: {}", sysroot.display()));
		say(&format!("override reason: {}", &*reason));
		
		show_tool_versions(cfg, &toolchain);
	} else {
		say("no override");
		show_default(cfg);
	}
}

fn list_overrides(cfg: &Cfg) {
	if is_file(&cfg.override_db) {
		let contents = read_file(&cfg.override_db)
			.ok().expect("could not read overrides db");
		
		let mut overrides: Vec<_> = contents
			.lines()
			.collect();
			
		overrides.sort();
		
		if overrides.is_empty() {
			println!("no overrides");
		} else {
			for o in overrides {
				println!("{}", o);
			}
		}
	} else {
		println!("no overrides");
	}
}

fn list_toolchains(cfg: &Cfg) {
	if is_directory(&cfg.toolchains_dir) {
		let mut toolchains: Vec<_> = fs::read_dir(&cfg.toolchains_dir)
			.ok().expect("could not read toolchains directory")
			.filter_map(Result::ok)
			.filter_map(|e| e.file_name().into_string().ok())
			.collect();
			
		toolchains.sort();
		
		if toolchains.is_empty() {
			say("no installed toolchains");
		} else {
			for toolchain in toolchains {
				println!("{}", &toolchain);
			}
		}
	} else {
		say("no installed toolchains");
	}
}

fn update_all_channels(cfg: &Cfg) {
	let stable_ok = update_toolchain(cfg, "stable");
	let beta_ok = update_toolchain(cfg, "beta");
	let nightly_ok = update_toolchain(cfg, "nightly");
	
	if stable_ok {
		say("'stable' update succeeded");
	} else {
		say_err("'stable' update FAILED");
	}
	if beta_ok {
		say("'beta' update succeeded");
	} else {
		say_err("'beta' update FAILED");
	}
	if nightly_ok {
		say("'nightly' update succeeded");
	} else {
		say_err("'nightly' update FAILED");
	}
	
	say("stable revision:");
	show_tool_versions(cfg, "stable");
	say("beta revision:");
	show_tool_versions(cfg, "beta");
	say("nightly revision:");
	show_tool_versions(cfg, "nightly");
}

fn update(cfg: &Cfg, matches: &ArgMatches) {
	if let Some(toolchain) = matches.value_of("toolchain") {
		if !common_install_args(cfg, "update", matches) {
			update_toolchain(cfg, toolchain);
		}
	} else {
		update_all_channels(cfg);
	}
}

fn check_metadata_version(cfg: &Cfg) {
	verbose_say(&cfg.base, "checking metadata version");
	
	assert!(is_directory(&cfg.multirust_dir), "multirust_dir must exist");
	
	if !is_file(&cfg.version_file) {
		verbose_say(&cfg.base, &format!("writing metadata version {}", cfg.metadata_version));
		
		write_file(&cfg.version_file, &format!("{}", cfg.metadata_version))
			.ok().expect("failed to write metadata version");
	} else {
		let current_version = read_file(&cfg.version_file)
			.ok().expect("failed to read metadata version");
		
		verbose_say(&cfg.base, &format!("got metadata version {}", &current_version));
		
		assert!(&*current_version == cfg.metadata_version,
			"metadata version is {}, need {}. run `multirust upgrade-data`",
			&current_version, cfg.metadata_version);
	}
}
