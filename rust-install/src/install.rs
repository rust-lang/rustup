use utils;
use errors::*;
use temp;
use env_var;
use dist;
#[cfg(windows)]
use msi;
use distv2::DistV2;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::env;

#[cfg(not(windows))]
const REL_MANIFEST_DIR: &'static str = "lib/rustlib";
#[cfg(windows)]
const REL_MANIFEST_DIR: &'static str = "bin\\rustlib";

pub struct InstallPrefix {
	path: PathBuf,
	install_type: InstallType,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum InstallType {
	// Must be uninstalled by deleting the entire directory
	Owned,
	// Must be uninstalled via `uninstall.sh` on linux or `msiexec /x` on windows
	Shared,
}

pub enum Uninstaller {
	Sh(PathBuf),
	Msi(String),
}

impl Uninstaller {
	pub fn run(&self) -> Result<()> {
		match *self {
			Uninstaller::Sh(ref path) => {
				Ok(try!(
					utils::cmd_status("uninstall.sh", Command::new("sudo").arg("sh").arg(path))
				))
			},
			Uninstaller::Msi(ref id) => {
				Ok(try!(
					utils::cmd_status("msiexec", Command::new("msiexec").arg("/x").arg(id))
				))
			},
		}
	}
}

pub enum InstallMethod<'a> {
	Copy(&'a Path),
	Link(&'a Path),
	Installer(&'a Path, &'a temp::Cfg),
	Dist(&'a str, Option<&'a Path>, dist::DownloadCfg<'a>),
}

impl<'a> InstallMethod<'a> {
	pub fn install_type_possible(&self, install_type: InstallType) -> bool {
		match *self {
			InstallMethod::Copy(_)|InstallMethod::Link(_) => install_type == InstallType::Owned,
			InstallMethod::Installer(_, _)|InstallMethod::Dist(_,_,_) => true,
		}
	}
	pub fn run(self, prefix: &InstallPrefix, notify_handler: NotifyHandler) -> Result<()> {
		if prefix.is_installed_here() {
			// Don't uninstall first for Dist method
			match self {
				InstallMethod::Dist(_,_,_) => {},
				_ => { try!(prefix.uninstall(notify_handler)); },
			}
		}
		
		if !self.install_type_possible(prefix.install_type) {
			return Err(Error::InstallTypeNotPossible);
		}
		
		match self {
			InstallMethod::Copy(src) => {
				try!(utils::copy_dir(src, &prefix.path, ntfy!(&notify_handler)));
				Ok(())
			},
			InstallMethod::Link(src) => {
				try!(utils::symlink_dir(src, &prefix.path, ntfy!(&notify_handler)));
				Ok(())
			},
			InstallMethod::Installer(src, temp_cfg) => {
				notify_handler.call(Notification::Extracting(src, prefix.path()));
				let temp_dir = try!(temp_cfg.new_directory());
				match src.extension().and_then(OsStr::to_str) {
					Some("gz") => InstallMethod::tar_gz(src, &temp_dir, prefix),
					Some("msi") => InstallMethod::msi(src, &temp_dir, prefix),
					_ => Err(Error::InvalidFileExtension),
				}
			},
			InstallMethod::Dist(toolchain, update_hash, dl_cfg) => {
				if let Some((installer, hash)) = try!(dist::download_dist(toolchain, update_hash, dl_cfg)) {
					try!(InstallMethod::Installer(&*installer, dl_cfg.temp_cfg).run(prefix, dl_cfg.notify_handler));
					if let Some(hash_file) = update_hash {
						try!(utils::write_file("update hash", hash_file, &hash));
					}
				}
				Ok(())
			}
		}
	}
	
	fn tar_gz(src: &Path, work_dir: &Path, prefix: &InstallPrefix) -> Result<()> {
		let installer_dir = Path::new(try!(Path::new(src.file_stem().unwrap()).file_stem()
			.ok_or(Error::InvalidFileExtension)));
			
		try!(utils::cmd_status("tar",
			Command::new("tar")
				.arg("xzf").arg(src)
				.arg("-C").arg(work_dir)
		));
		
		// Find the root Rust folder within the subfolder
		let root_dir = try!(try!(utils::read_dir("install", work_dir))
			.filter_map(io::Result::ok)
			.map(|e| e.path())
			.filter(|p| utils::is_directory(&p))
			.next()
			.ok_or(Error::InvalidInstaller));
		
		let mut cmd = Command::new("sh");
		let mut arg = OsString::from("--prefix=\"");
		arg.push(&prefix.path);
		arg.push("\"");
		cmd
			.arg(root_dir.join("install.sh"))
			.arg(arg);
		
		if prefix.install_type != InstallType::Shared {
			cmd.arg("--disable-ldconfig");
		}

		let result = Ok(try!(utils::cmd_status("sh", &mut cmd)));
		
		let _ = fs::remove_dir_all(&installer_dir);
			
		if result.is_err() && prefix.install_type == InstallType::Owned {
			let _ = fs::remove_dir_all(&prefix.path);
		}
		
		result
	}
	fn msi(src: &Path, work_dir: &Path, prefix: &InstallPrefix) -> Result<()> {
		let msi_owned = || -> Result<()> {
			let target_arg = utils::prefix_arg("TARGETDIR=", work_dir);
			
			// Extract the MSI to the subfolder
			let mut cmd = Command::new("msiexec");
			cmd
				.arg("/a").arg(src)
				.arg("/qn")
				.arg(&target_arg);
			
			try!(utils::cmd_status("msiexec",
				Command::new("msiexec")
					.arg("/a").arg(src)
					.arg("/qn")
					.arg(&target_arg)
			));
			
			// Find the root Rust folder within the subfolder
			let root_dir = try!(try!(utils::read_dir("install", work_dir))
				.filter_map(io::Result::ok)
				.map(|e| e.path())
				.filter(|p| utils::is_directory(&p))
				.next()
				.ok_or(Error::InvalidInstaller));
			
			// Rename and move it to the toolchain directory
			Ok(try!(utils::rename_dir("install", &root_dir, &prefix.path)))
		};
		
		let msi_shared = || -> Result<()> {
			let target_arg = utils::prefix_arg("TARGETDIR=", &prefix.path);
			
			// Extract the MSI to the subfolder
			Ok(try!(utils::cmd_status("msiexec", 
				Command::new("msiexec")
					.arg("/i").arg(src)
					.arg("/qn")
					.arg(&target_arg)
			)))
		};
		
		match prefix.install_type {
			InstallType::Owned => {
				let result = msi_owned();
				if result.is_err() {
					let _ = fs::remove_dir_all(&prefix.path);
				}
				result
			},
			InstallType::Shared => {
				msi_shared()
			}
		}
	}
}

pub fn bin_path(name: &str) -> PathBuf {
	let mut path = PathBuf::from("bin");
	path.push(name.to_owned() + env::consts::EXE_SUFFIX);
	path
}

impl InstallPrefix {
	pub fn from(path: PathBuf, install_type: InstallType) -> Self {
		InstallPrefix {
			path: path,
			install_type: install_type,
		}
	}
	pub fn path(&self) -> &Path {
		&self.path
	}
	pub fn manifest_dir(&self) -> PathBuf {
		let mut path = self.path.clone();
		path.push(REL_MANIFEST_DIR);
		path
	}
	pub fn manifest_file(&self, name: &str) -> PathBuf {
		let mut path = self.manifest_dir();
		path.push(name);
		path
	}
	pub fn rel_manifest_file(&self, name: &str) -> String {
		let mut path = PathBuf::from(REL_MANIFEST_DIR);
		path.push(name);
		path.into_os_string().into_string().unwrap()
	}
	pub fn binary_file(&self, name: &str) -> PathBuf {
		let mut path = self.path.clone();
		path.push(bin_path(name));
		path
	}
	pub fn doc_path(&self, relative: &str) -> Result<PathBuf> {
		let parts = vec!["share", "doc", "rust", "html"];
		let mut doc_dir = self.path.clone();
		for part in parts {
			doc_dir.push(part);
		}
		doc_dir.push(relative);
		
		Ok(doc_dir)
	}
	pub fn is_installed_here(&self) -> bool {
		match self.install_type {
			InstallType::Owned => utils::is_directory(&self.path),
			InstallType::Shared => utils::is_directory(&self.manifest_dir()),
		}
	}
	pub fn get_uninstall_sh(&self) -> Option<PathBuf> {
		let path = self.manifest_file("uninstall.sh");
		if utils::is_file(&path) {
			Some(path)
		} else {
			None
		}
	}
	
	#[cfg(windows)]
	pub fn get_uninstall_msi(&self, notify_handler: NotifyHandler) -> Option<String> {
		let canon_path = utils::canonicalize_path(&self.path, ntfy!(&notify_handler));
		
		if let Ok(installers) = msi::all_installers() {
			for installer in &installers {
				if let Ok(loc) = installer.install_location() {
					let path = utils::canonicalize_path(&loc, ntfy!(&notify_handler));
					
					if path == canon_path {
						return Some(installer.product_id().to_owned());
					}
				}
			}
		}
		
		None
	}
	#[cfg(not(windows))]
	pub fn get_uninstall_msi(&self, _: NotifyHandler) -> Option<String> {
		None
	}
	pub fn get_uninstaller(&self, notify_handler: NotifyHandler) -> Option<Uninstaller> {
		self.get_uninstall_sh().map(Uninstaller::Sh).or_else(
			|| self.get_uninstall_msi(notify_handler).map(Uninstaller::Msi)
			)
	}
	pub fn uninstall(&self, notify_handler: NotifyHandler) -> Result<()> {
		if self.is_installed_here() {
			match self.install_type {
				InstallType::Owned => {
					Ok(try!(utils::remove_dir("install", &self.path, ntfy!(&notify_handler))))
				},
				InstallType::Shared => {
					if let Some(uninstaller) = self.get_uninstaller(notify_handler) {
						uninstaller.run()
					} else {
						Err(Error::NotInstalledHere)
					}
				},
			}
		} else {
			Err(Error::NotInstalledHere)
		}
	}
	pub fn install(&self, method: InstallMethod, notify_handler: NotifyHandler) -> Result<()> {
		method.run(self, notify_handler)
	}
	
	pub fn set_ldpath(&self, cmd: &mut Command) {
		let new_path = self.path.join("lib");
		
		env_var::set_path("LD_LIBRARY_PATH", &new_path, cmd);
		env_var::set_path("DYLD_LIBRARY_PATH", &new_path, cmd);
	}
	
	pub fn set_env(&self, cmd: &mut Command) {
		let cargo_path = self.path.join("cargo");
		
		self.set_ldpath(cmd);
		env_var::set_default("CARGO_HOME", cargo_path.as_ref(), cmd);
	}
	
	pub fn create_command(&self, binary: &str) -> Command {
		let binary_path = self.binary_file(binary);
		let mut cmd = Command::new(binary_path);
		
		self.set_env(&mut cmd);
		cmd
	}
	
	pub fn open_docs(&self, relative: &str) -> Result<()> {
		Ok(try!(utils::open_browser(&try!(self.doc_path(relative)))))
	}
	
	pub fn as_distv2_install(&self) -> Option<DistV2> {
		DistV2::new(self)
	}
}
