use std::path::{Path, PathBuf};
use std::borrow::Cow;
use std::sync::Mutex;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::env;
use std::io;
use std::fs;
use std::process::Command;
use std::fmt::{self, Display};
use hyper;
use regex::Regex;
use utils;
use temp;
use errors::*;

pub const DB_DELIMITER: &'static str = ";";
pub const METADATA_VERSION: &'static str = "2";

pub enum OverrideReason {
	Environment,
	OverrideDB(PathBuf),
}

impl Display for OverrideReason {
	fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
		match *self {
			OverrideReason::Environment =>
				write!(f, "environment override by MULTIRUST_TOOLCHAIN"),
			OverrideReason::OverrideDB(ref path) =>
				write!(f, "directory override due to '{}'", path.display()),
		}
	}
}

struct EnvVarSetting<'a, F> {
	key: &'a OsStr,
	old_value: Option<OsString>,
	f: Option<F>,
}

impl<'a, T, F: FnOnce() -> T> EnvVarSetting<'a, F> {
	fn new<V: AsRef<OsStr>>(k: &'a OsStr, v: V, f: F) -> Self {
		let old_value = env::var_os(k);
		env::set_var(k, v);
		
		EnvVarSetting {
			key: k,
			old_value: old_value,
			f: Some(f),
		}
	}
	fn call(&mut self) -> T {
		(self.f.take().unwrap())()
	}
}

impl<'a, F> Drop for EnvVarSetting<'a, F> {
	fn drop(&mut self) {
		if let Some(ref v) = self.old_value {
			env::set_var(self.key, v);
		} else {
			env::remove_var(self.key);
		}
	}
}

pub struct Cfg {
	pub home_dir: PathBuf,
	pub multirust_dir: PathBuf,
	pub version_file: PathBuf,
	pub override_db: PathBuf,
	pub default_file: PathBuf,
	pub toolchains_dir: PathBuf,
	pub update_hash_dir: PathBuf,
	pub temp_cfg: temp::Cfg,
	pub gpg_key: Cow<'static, str>,
	pub var_stack: Mutex<HashMap<&'static str, Vec<Option<OsString>>>>,
	pub notify_handler: NotifyHandler,
	pub env_override: Option<String>,
}

impl Cfg {
	pub fn from_env(notify_handler: NotifyHandler) -> Result<Self> {
		// Get absolute home directory
		let home_dir = try!(env::home_dir()
			.map(PathBuf::from)
			.and_then(utils::to_absolute)
			.ok_or(Error::LocatingHome));
		
		// Set up the multirust home directory
		let multirust_dir = env::var_os("MULTIRUST_HOME")
			.and_then(utils::if_not_empty)
			.map(PathBuf::from)
			.unwrap_or_else(|| home_dir.join(".multirust"));
			
		try!(utils::ensure_dir_exists("home", &multirust_dir, &notify_handler));
		
		// Export RUSTUP_HOME to configure rustup.sh to store it's stuff
		// in the MULTIRUST_HOME directory.
		env::set_var("RUSTUP_HOME", multirust_dir.join("rustup"));
		
		// Data locations
		let version_file = multirust_dir.join("version");
		let override_db = multirust_dir.join("overrides");
		let default_file = multirust_dir.join("default");
		let toolchains_dir = multirust_dir.join("toolchains");
		let update_hash_dir = multirust_dir.join("update-hashes");

		let notify_clone = notify_handler.clone();
		let temp_cfg = temp::Cfg::new(
			multirust_dir.join("tmp"),
			temp::NotifyHandler::from(move |n: temp::Notification| {
				notify_clone.call(Temp(n));
			}),
		);
		
		// GPG key
		let gpg_key = if let Some(path) = env::var_os("MULTIRUST_GPG_KEY").and_then(utils::if_not_empty) {
			Cow::Owned(try!(utils::read_file("public key", Path::new(&path))))
		} else {
			Cow::Borrowed(include_str!("rust-key.gpg.ascii"))
		};
		
		// Environment override
		let env_override = env::var("MULTIRUST_TOOLCHAIN")
			.ok().and_then(utils::if_not_empty);
		
		Ok(Cfg {
			home_dir: home_dir,
			multirust_dir: multirust_dir,
			version_file: version_file,
			override_db: override_db,
			default_file: default_file,
			toolchains_dir: toolchains_dir,
			update_hash_dir: update_hash_dir,
			temp_cfg: temp_cfg,
			gpg_key: gpg_key,
			var_stack: Mutex::new(HashMap::new()),
			notify_handler: notify_handler,
			env_override: env_override,
		})
	}
	
	pub fn set_default_toolchain(&self, toolchain: &str) -> Result<()> {
		let work_file = try!(self.temp_cfg.new_file());
		
		try!(utils::write_file("temp", &work_file, toolchain));
		
		try!(utils::rename_file("default", &*work_file, &self.default_file));
		
		self.notify_handler.call(SetDefaultToolchain(toolchain));
		
		Ok(())
	}

	fn path_to_db_key(&self, path: &Path) -> Result<String> {
		Ok(try!(utils::canonicalize_path(path))
			.display().to_string() + DB_DELIMITER)
	}
	
	pub fn set_override(&self, path: &Path, toolchain: &str) -> Result<()> {
		let key = try!(self.path_to_db_key(path));
		
		let work_file = try!(self.temp_cfg.new_file());
		
		if utils::is_file(&self.override_db) {
			try!(utils::filter_file("override db", &self.override_db, &work_file, |line| {
				!line.starts_with(&key)
			}));
		}
		
		try!(utils::append_file("override db", &work_file, &(key + toolchain)));
			
		try!(utils::rename_file("override db", &*work_file, &self.override_db));
			
		self.notify_handler.call(SetOverrideToolchain(path, toolchain));
		
		Ok(())
	}

	pub fn get_toolchain_dir(&self, toolchain: &str, create_parent: bool) -> Result<PathBuf> {
		if create_parent {
			try!(utils::ensure_dir_exists("toolchains", &self.toolchains_dir, &self.notify_handler));
		}
		
		Ok(self.toolchains_dir.join(toolchain))
	}
	
	pub fn get_hash_file(&self, toolchain: &str, create_parent: bool) -> Result<PathBuf> {
		if create_parent {
			try!(utils::ensure_dir_exists("update-hash", &self.update_hash_dir, &self.notify_handler));
		}
		
		Ok(self.toolchains_dir.join(toolchain))
	}
	
	pub fn is_toolchain_installed(&self, toolchain: &str) -> Result<bool> {
		self.notify_handler.call(LookingForToolchain(toolchain));
		
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, false));
		Ok(utils::is_directory(toolchain_dir))
	}
	
	pub fn call_rustup(&self, toolchain_dir: &Path, toolchain: &str, hash_file: &Path) -> Result<()> {
		let rustup = env::current_exe()
			.ok().expect("could not find location of self")
			.with_file_name("rustup");
		
		let mut cmd = Command::new(rustup);
		cmd
			.arg("--prefix").arg(toolchain_dir)
			.arg("--spec").arg(toolchain)
			.arg("--update-hash-file").arg(hash_file)
			.arg("--disable-ldconfig")
			.arg("-y").arg("--disable-sudo");
		
		utils::cmd_status("rustup", cmd)
	}
	
	pub fn install_toolchain_from_dist(&self, toolchain: &str) -> Result<()> {
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, true));
		let hash_file = try!(self.get_hash_file(toolchain, true));
		
		self.notify_handler.call(ToolchainDirectory(&toolchain_dir, toolchain));

		self.call_rustup(&toolchain_dir, toolchain, &hash_file)
	}
	
	pub fn update_toolchain(&self, toolchain: &str) -> Result<()> {
		if try!(self.is_toolchain_installed(toolchain)) {
			self.notify_handler.call(UpdatingToolchain(toolchain));
		} else {
			self.notify_handler.call(InstallingToolchain(toolchain));
		}
		self.install_toolchain_from_dist(toolchain)
	}
	
	pub fn install_toolchain_if_not_installed(&self, toolchain: &str) -> Result<()> {
		if try!(self.is_toolchain_installed(toolchain)) {
			self.notify_handler.call(UsingExistingToolchain(toolchain));
			Ok(())
		} else {
			self.update_toolchain(toolchain)
		}
	}
	
	pub fn bin_path(&self, name: &str) -> PathBuf {
		let mut path = PathBuf::from("bin");
		path.push(name.to_owned() + env::consts::EXE_SUFFIX);
		path
	}
	
	pub fn update_custom_toolchain_from_dir(&self, toolchain: &str, path: &Path, link: bool) -> Result<()> {
		try!(utils::assert_is_directory(path));
		
		let expected_rustc = path.join(&self.bin_path("rustc"));
		try!(utils::assert_is_file(&expected_rustc));
		
		try!(self.maybe_remove_existing_custom_toolchain(toolchain));
		
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, true));
		let abs_path = try!(utils::to_absolute(path)
			.ok_or(Error::LocatingWorkingDir));
		
		if link {
			utils::symlink_dir(&toolchain_dir, &abs_path, &self.notify_handler)
		} else {
			utils::copy_dir(&abs_path, &toolchain_dir, &self.notify_handler)
		}
	}
	
	pub fn remove_toolchain(&self, toolchain: &str) -> Result<()> {
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, false));
		
		if utils::is_directory(&toolchain_dir) {
			try!(utils::remove_dir("toolchain", &toolchain_dir, &self.notify_handler));

			self.notify_handler.call(UninstalledToolchain(toolchain));			
		} else {
			self.notify_handler.call(ToolchainNotInstalled(toolchain));			
		}
		
		let hash_file = try!(self.get_hash_file(toolchain, false));
		if utils::is_file(&hash_file) {
			try!(utils::remove_file("update hash", &hash_file));
		}
		
		Ok(())
	}
	
	pub fn maybe_remove_existing_custom_toolchain(&self, toolchain: &str) -> Result<()> {
		if try!(self.is_toolchain_installed(toolchain)) {
			self.notify_handler.call(UninstallingToolchain(toolchain));
			self.remove_toolchain(toolchain)
		} else {
			Ok(())
		}
	}
	
	pub fn install_toolchain_tar_gz(&self, toolchain: &str, installer: &Path, work_dir: &Path) -> Result<()> {
		let installer_dir = Path::new(try!(Path::new(installer.file_stem().unwrap()).file_stem()
			.ok_or(Error::InvalidFileExtension)));
			
		let mut cmd = Command::new("tar");
		cmd
			.arg("xzf").arg(installer)
			.arg("-C").arg(work_dir);
		
		try!(utils::cmd_status("tar", cmd));
		
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, true));
		
		self.notify_handler.call(InstallingToolchain(toolchain));
		self.notify_handler.call(ToolchainDirectory(&toolchain_dir, toolchain));
		
		let mut cmd = Command::new("sh");
		cmd
			.arg(installer_dir.join("install.sh"))
			.arg("--prefix").arg(&toolchain_dir)
			.arg("--disable-ldconfig");

		let result = utils::cmd_status("sh", cmd);
			
		if result.is_err() {
			let _ = fs::remove_dir_all(&toolchain_dir);
		}
		
		result
	}
	
	pub fn install_toolchain_msi(&self, toolchain: &str, installer: &Path, work_dir: &Path) -> Result<()> {
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, true));
		
		self.notify_handler.call(InstallingToolchain(toolchain));
		self.notify_handler.call(ToolchainDirectory(&toolchain_dir, toolchain));
			
		let inner = || -> Result<()> {
			// Can't install to same folder as installer, so create subfolder
			let install_dir = work_dir.join("install");
			
			let installer_arg = installer;
			let target_arg = utils::prefix_arg("TARGETDIR=", &install_dir);
			
			// Extract the MSI to the subfolder
			let mut cmd = Command::new("msiexec");
			cmd
				.arg("/a").arg(&installer_arg)
				.arg("/qn")
				.arg(&target_arg);
			
			try!(utils::cmd_status("msiexec", cmd));
			
			// Find the root Rust folder within the subfolder
			let root_dir = try!(try!(utils::read_dir("install", &install_dir))
				.filter_map(io::Result::ok)
				.map(|e| e.path())
				.filter(|p| utils::is_directory(&p))
				.next()
				.ok_or(Error::InvalidInstaller));
				
			// Rename and move it to the toolchain directory
			utils::rename_dir("toolchain", &root_dir, &toolchain_dir)
		};
		
		let result = inner();
		if result.is_err() {
			let _ = fs::remove_dir_all(&toolchain_dir);
		}
		result
	}
	
	pub fn install_toolchain(&self, toolchain: &str, installer: &Path, work_dir: &Path) -> Result<()> {
		match installer.extension().and_then(OsStr::to_str) {
			Some("gz") => self.install_toolchain_tar_gz(toolchain, installer, work_dir),
			Some("msi") => self.install_toolchain_msi(toolchain, installer, work_dir),
			_ => Err(Error::InvalidFileExtension),
		}
	}
	
	pub fn check_custom_toolchain_name(&self, toolchain: &str) -> Result<()> {
		let re = Regex::new(r"^(nightly|beta|stable)(-\d{4}-\d{2}-\d{2})?$").unwrap();
		if re.is_match(toolchain) {
			Err(Error::InvalidToolchainName)
		} else {
			Ok(())
		}
	}

	pub fn update_custom_toolchain_from_installers(&self, toolchain: &str, installers: &[&OsStr]) -> Result<()> {
		try!(self.check_custom_toolchain_name(toolchain));
		try!(self.maybe_remove_existing_custom_toolchain(toolchain));
		
		let work_dir = try!(self.temp_cfg.new_directory());
		
		for installer in installers {
			let local_installer;
			let installer_str = installer.to_str();
			if let Some(Ok(url)) = installer_str.map(hyper::Url::parse) {
				// If installer is a URL
				
				// Extract basename from url (eg. 'rust-1.3.0-x86_64-unknown-linux-gnu.tar.gz')
				let re = Regex::new(r"[\\/]([^\\/?]+)(\?.*)?$").unwrap();
				let basename = try!(re.captures(installer_str.unwrap())
					.ok_or(Error::InvalidInstallerUrl)).at(1).unwrap();
				
				// Download to a local file
				local_installer = Cow::Owned(work_dir.join(basename));
				try!(utils::download_file(url, &local_installer, &self.notify_handler));
			} else {
				// If installer is a filename
				
				// No need to download
				local_installer = Cow::Borrowed(Path::new(installer));
			}
			
			// Install from file
			try!(self.install_toolchain(toolchain, &local_installer, &work_dir));
		}
		
		Ok(())
	}
	
	pub fn which_binary(&self, path: &Path, binary: &str) -> Result<Option<PathBuf>> {
		
		if let Some((_, toolchain_dir, _)) = try!(self.find_override_toolchain_or_default(path)) {
			Ok(Some(toolchain_dir.join(self.bin_path(binary))))
		} else {
			Ok(None)
		}
	}
	
	pub fn upgrade_data(&self) -> Result<bool> {
		if !utils::is_file(&self.version_file) {
			return Ok(false);
		}
		
		let current_version = try!(utils::read_file("version", &self.version_file));
		
		self.notify_handler.call(UpgradingMetadata(&current_version, METADATA_VERSION));

		match &*current_version {
			"2" => {
				// Current version. Do nothing
				Ok(false)
			},
			"1" => {
				// Ignore errors. These files may not exist.
				let _ = fs::remove_dir_all(self.multirust_dir.join("available-updates"));
				let _ = fs::remove_dir_all(self.multirust_dir.join("update-sums"));
				let _ = fs::remove_dir_all(self.multirust_dir.join("channel-sums"));
				let _ = fs::remove_dir_all(self.multirust_dir.join("manifests"));
				
				try!(utils::write_file("version", &self.version_file, METADATA_VERSION));
				
				Ok(true)
			}
			_ => {
				Err(Error::UnknownMetadataVersion(current_version))
			}
		}
	}
	
	pub fn delete_data(&self) -> Result<()> {
		if utils::path_exists(&self.multirust_dir) {
			utils::remove_dir("home", &self.multirust_dir, &self.notify_handler)
		} else {
			Ok(())
		}
	}
	
	pub fn remove_override(&self, path: &Path) -> Result<bool> {
		let key = try!(self.path_to_db_key(&path));
		
		let work_file = try!(self.temp_cfg.new_file());
		
		let removed = if utils::is_file(&self.override_db) {
			try!(utils::filter_file("override db", &self.override_db, &work_file, |line| {
				!line.starts_with(&key)
			}))
		} else {
			0
		};
		
		if removed > 0 {
			try!(utils::rename_file("override db", &*work_file, &self.override_db));
			Ok(true)
		} else {
			Ok(false)
		}
	}
	
	pub fn verify_toolchain_dir(&self, toolchain: &str) -> Result<PathBuf> {
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, false));
		
		try!(utils::assert_is_directory(&toolchain_dir));
		
		Ok(toolchain_dir)
	}
	
	pub fn find_default(&self) -> Result<Option<(String, PathBuf)>> {
		if !utils::is_file(&self.default_file) {
			return Ok(None);
		}
		
		let toolchain = try!(utils::read_file("default", &self.default_file));
		if toolchain.is_empty() {
			return Ok(None);
		}
		
		let toolchain_dir = try!(self.verify_toolchain_dir(&toolchain));
		
		Ok(Some((toolchain, toolchain_dir)))
	}
	
	pub fn find_override(&self, path: &Path) -> Result<Option<(String, PathBuf, OverrideReason)>> {
		if let Some(ref toolchain) = self.env_override {
			let toolchain_dir = try!(self.verify_toolchain_dir(toolchain));
			
			return Ok(Some((toolchain.clone(), toolchain_dir, OverrideReason::Environment)));
		}
		
		if !utils::is_file(&self.override_db) {
			return Ok(None);
		}
		
		let dir_unresolved = path;
		let dir = try!(utils::canonicalize_path(dir_unresolved));
		let mut path = &*dir;
		while let Some(parent) = path.parent() {
			let key = try!(self.path_to_db_key(path));
			if let Some(toolchain) = try!(utils::match_file("override db", &self.override_db, |line| {
				if line.starts_with(&key) {
					Some(line[key.len()..].to_owned())
				} else {
					None
				}
			})) {
				let toolchain_dir = try!(self.verify_toolchain_dir(&toolchain));
				
				return Ok(Some((
					toolchain,
					toolchain_dir,
					OverrideReason::OverrideDB(path.to_owned())
					)));
			}
			
			path = parent;
		}
		
		Ok(None)
	}
	
	pub fn find_override_toolchain_or_default(&self, path: &Path) -> Result<Option<(String, PathBuf, Option<OverrideReason>)>> {
		Ok(if let Some((a, b, c)) = try!(self.find_override(path)) {
			Some((a, b, Some(c)))
		} else {
			try!(self.find_default()).map(|(a,b)| (a, b, None))
		})
	}
	
	pub fn toolchain_for_dir(&self, path: &Path) -> Result<(String, PathBuf, Option<OverrideReason>)> {
		self.find_override_toolchain_or_default(path)
			.and_then(|r| r.ok_or(Error::NoDefaultToolchain))
	}
	
	pub fn list_overrides(&self) -> Result<Vec<String>> {
		if utils::is_file(&self.override_db) {
			let contents = try!(utils::read_file("override db", &self.override_db));
			
			let overrides: Vec<_> = contents
				.lines()
				.map(|s| s.to_owned())
				.collect();
				
			Ok(overrides)
		} else {
			Ok(Vec::new())
		}
	}
	
	pub fn list_toolchains(&self) -> Result<Vec<String>> {
		if utils::is_directory(&self.toolchains_dir) {
			let toolchains: Vec<_> = try!(utils::read_dir("toolchains", &self.toolchains_dir))
				.filter_map(io::Result::ok)
				.filter_map(|e| e.file_name().into_string().ok())
				.collect();
			
			Ok(toolchains)
		} else {
			Ok(Vec::new())
		}
	}
	
	pub fn update_all_channels(&self) -> [Result<()>; 3] {
		let stable_result = self.update_toolchain("stable");
		let beta_result = self.update_toolchain("beta");
		let nightly_result = self.update_toolchain("nightly");
		
		[stable_result, beta_result, nightly_result]
	}
	
	pub fn check_metadata_version(&self) -> Result<bool> {
		try!(utils::assert_is_directory(&self.multirust_dir));
		
		if !utils::is_file(&self.version_file) {
			self.notify_handler.call(WritingMetadataVersion(METADATA_VERSION));
			
			try!(utils::write_file("metadata version", &self.version_file, METADATA_VERSION));
			
			Ok(true)
		} else {
			let current_version = try!(utils::read_file("metadata version", &self.version_file));
			
			self.notify_handler.call(ReadMetadataVersion(&current_version));
			
			Ok(&*current_version == METADATA_VERSION)
		}
	}
	
	pub fn with_var<T, F: FnOnce() -> Result<T>>(&self, name: &str, value: &OsStr, f: F) -> Result<T> {
		let mut s = EnvVarSetting::new(name.as_ref(), value, f);
		s.call()
	}
	
	pub fn with_default_var<T, F: FnOnce() -> Result<T>>(&self, name: &str, value: &OsStr, f: F) -> Result<T> {
		let new_value = env::var_os(name)
			.and_then(utils::if_not_empty)
			.unwrap_or(value.to_owned());
		let mut s = EnvVarSetting::new(name.as_ref(), new_value, f);
		s.call()
	}
	
	pub fn with_path_var<T, F: FnOnce() -> Result<T>>(&self, name: &str, value: &Path, f: F) -> Result<T> {
		let old_value = env::var_os(name);
		let mut parts = vec![value.to_owned()];
		if let Some(ref v) = old_value {
			parts.extend(env::split_paths(v));
		}
		let new_value = try!(env::join_paths(parts)
			.map_err(|_| Error::InvalidEnvironment));
		
		let mut s = EnvVarSetting::new(name.as_ref(), new_value, f);
		s.call()
	}
	
	pub fn with_toolchain_ldpath<T, F: FnOnce() -> Result<T>>(&self, toolchain: &str, f: F) -> Result<T> {
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, false));
		let new_path = toolchain_dir.join("lib");
		
		self.with_path_var("LD_LIBRARY_PATH", &new_path, || {
			self.with_path_var("DYLD_LIBRARY_PATH", &new_path, || {
				f()
			})
		})
	}
	
	pub fn with_toolchain_env<T, F: FnOnce() -> Result<T>>(&self, toolchain: &str, f: F) -> Result<T> {
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, false));
		let lib_path = toolchain_dir.join("lib");
		let cargo_path = toolchain_dir.join("cargo");
		
		self.with_path_var("LD_LIBRARY_PATH", &lib_path, || {
			self.with_path_var("DYLD_LIBRARY_PATH", &lib_path, || {
				self.with_default_var("CARGO_HOME", cargo_path.as_ref(), || {
					self.with_var("MULTIRUST_TOOLCHAIN", toolchain.as_ref(), || {
						self.with_var("MULTIRUST_HOME", self.multirust_dir.as_ref(), || {
							f()
						})
					})
				})
			})
		})
	}
	
	pub fn create_command(&self, toolchain: &str, binary: &str) -> Result<Command> {
		let toolchain_dir = try!(self.get_toolchain_dir(toolchain, false));
		let binary_path = toolchain_dir.join(self.bin_path(binary));
		
		self.with_toolchain_env(toolchain, || Ok(Command::new(binary_path)))
	}
	
	pub fn create_command_for_dir(&self, path: &Path, binary: &str) -> Result<Command> {
		let (toolchain, _, _) = try!(self.toolchain_for_dir(path));
		self.create_command(&toolchain, binary)
	}
	
	pub fn doc_path(&self, toolchain: &str, show_all: bool) -> Result<PathBuf> {
		let toolchain_dir = try!(self.verify_toolchain_dir(toolchain));

		let parts = if show_all {
			vec!["share", "doc", "rust", "html", "std", "index.html"]
		} else {
			vec!["share", "doc", "rust", "html", "index.html"]
		};
		let mut doc_dir = toolchain_dir;
		for part in parts {
			doc_dir.push(part);
		}
		
		Ok(doc_dir)
	}

	pub fn doc_path_for_dir(&self, path: &Path, show_all: bool) -> Result<PathBuf> {
		let (toolchain, _, _) = try!(self.toolchain_for_dir(path));
		self.doc_path(&toolchain, show_all)
	}
	
	pub fn open_docs(&self, toolchain: &str, show_all: bool) -> Result<()> {
		utils::open_browser(&try!(self.doc_path(toolchain, show_all)))
	}
	
	pub fn open_docs_for_dir(&self, path: &Path, show_all: bool) -> Result<()> {
		utils::open_browser(&try!(self.doc_path_for_dir(path, show_all)))
	}
}
