use utils;
use errors::*;
use msi;
use temp;
use env_var;
use dist;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::env;

const REL_MANIFEST_DIR: &'static str = "rustlib";

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
				let mut cmd = Command::new("sudo");
				cmd.arg("sh").arg(path);
				utils::cmd_status("uninstall.sh", cmd)
			},
			Uninstaller::Msi(ref id) => {
				let mut cmd = Command::new("msiexec");
				cmd.arg("/x").arg(id);
				utils::cmd_status("msiexec", cmd)
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
	pub fn run(self, prefix: &InstallPrefix, notify_handler: &NotifyHandler) -> Result<()> {
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
				utils::copy_dir(src, &prefix.path, notify_handler)
			},
			InstallMethod::Link(src) => {
				utils::symlink_dir(&prefix.path, src, notify_handler)
			},
			InstallMethod::Installer(src, temp_cfg) => {
				let temp_dir = try!(temp_cfg.new_directory());
				match src.extension().and_then(OsStr::to_str) {
					Some("gz") => InstallMethod::tar_gz(src, &temp_dir, prefix),
					Some("msi") => InstallMethod::msi(src, &temp_dir, prefix),
					_ => Err(Error::InvalidFileExtension),
				}
			},
			InstallMethod::Dist(toolchain, update_hash, dl_cfg) => {
				if let Some(installer) = try!(dist::download_dist(toolchain, update_hash, dl_cfg)) {
					try!(InstallMethod::Installer(&*installer, dl_cfg.temp_cfg).run(prefix, dl_cfg.notify_handler));
				}
				Ok(())
			}
		}
	}
	
	fn tar_gz(src: &Path, work_dir: &Path, prefix: &InstallPrefix) -> Result<()> {
		let installer_dir = Path::new(try!(Path::new(src.file_stem().unwrap()).file_stem()
			.ok_or(Error::InvalidFileExtension)));
			
		let mut cmd = Command::new("tar");
		cmd
			.arg("xzf").arg(src)
			.arg("-C").arg(work_dir);
		
		try!(utils::cmd_status("tar", cmd));
		
		let mut cmd = Command::new("sh");
		cmd
			.arg(installer_dir.join("install.sh"))
			.arg("--prefix").arg(&prefix.path);
		
		if prefix.install_type != InstallType::Shared {
			cmd.arg("--disable-ldconfig");
		}

		let result = utils::cmd_status("sh", cmd);
			
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
			
			try!(utils::cmd_status("msiexec", cmd));
			
			// Find the root Rust folder within the subfolder
			let root_dir = try!(try!(utils::read_dir("install", work_dir))
				.filter_map(io::Result::ok)
				.map(|e| e.path())
				.filter(|p| utils::is_directory(&p))
				.next()
				.ok_or(Error::InvalidInstaller));
				
			// Rename and move it to the toolchain directory
			utils::rename_dir("install", &root_dir, &prefix.path)
		};
		
		let msi_shared = || -> Result<()> {
			let target_arg = utils::prefix_arg("TARGETDIR=", &prefix.path);
			
			// Extract the MSI to the subfolder
			let mut cmd = Command::new("msiexec");
			cmd
				.arg("/i").arg(src)
				.arg("/qn")
				.arg(&target_arg);
			
			utils::cmd_status("msiexec", cmd)
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
		path.push("bin");
		path.push(REL_MANIFEST_DIR);
		path
	}
	pub fn manifest_file(&self, name: &str) -> PathBuf {
		let mut path = self.manifest_dir();
		path.push(name);
		path
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
		utils::is_directory(&self.manifest_dir())
	}
	pub fn get_uninstall_sh(&self) -> Option<PathBuf> {
		let path = self.manifest_file("uninstall.sh");
		if utils::is_file(&path) {
			Some(path)
		} else {
			None
		}
	}
	pub fn get_uninstall_msi(&self, notify_handler: &NotifyHandler) -> Option<String> {
		if cfg!(windows) {
			let canon_path = utils::canonicalize_path(&self.path, notify_handler);
			
			if let Ok(installers) = msi::all_installers() {
				for installer in &installers {
					if let Ok(loc) = installer.install_location() {
						let path = utils::canonicalize_path(&loc, notify_handler);
						
						if path == canon_path {
							return Some(installer.product_id().to_owned());
						}
					}
				}
			}
		}
		
		None
	}
	pub fn get_uninstaller(&self, notify_handler: &NotifyHandler) -> Option<Uninstaller> {
		self.get_uninstall_sh().map(Uninstaller::Sh).or_else(
			|| self.get_uninstall_msi(notify_handler).map(Uninstaller::Msi)
			)
	}
	pub fn uninstall(&self, notify_handler: &NotifyHandler) -> Result<()> {
		if self.is_installed_here() {
			match self.install_type {
				InstallType::Owned => {
					utils::remove_dir("install", &self.path, notify_handler)
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
	pub fn install(&self, method: InstallMethod, notify_handler: &NotifyHandler) -> Result<()> {
		method.run(self, notify_handler)
	}
	
	pub fn with_ldpath<T, F: FnOnce() -> Result<T>>(&self, f: F) -> Result<T> {
		let new_path = self.path.join("lib");
		
		env_var::with_path("LD_LIBRARY_PATH", &new_path, || {
			env_var::with_path("DYLD_LIBRARY_PATH", &new_path, || {
				f()
			})
		})
	}
	
	pub fn with_env<T, F: FnOnce() -> Result<T>>(&self, f: F) -> Result<T> {
		let cargo_path = self.path.join("cargo");
		
		self.with_ldpath(|| {
			env_var::with_default("CARGO_HOME", cargo_path.as_ref(), || {
				f()
			})
		})
	}
	
	pub fn create_command(&self, binary: &str) -> Result<Command> {
		let binary_path = self.binary_file(binary);
		
		self.with_env(|| Ok(Command::new(binary_path)))
	}
	
	pub fn open_docs(&self, relative: &str) -> Result<()> {
		utils::open_browser(&try!(self.doc_path(relative)))
	}
}
