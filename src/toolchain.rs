use errors::*;
use rust_install::{utils, dist, InstallPrefix, InstallType, InstallMethod};
use config::Cfg;

use std::process::Command;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::borrow::Cow;

use regex::Regex;
use hyper;
use rust_install;

pub struct Toolchain<'a> {
	cfg: &'a Cfg,
	name: String,
	prefix: InstallPrefix,
}

impl<'a> Toolchain<'a> {
	pub fn from(cfg: &'a Cfg, name: &str) -> Self {
		Toolchain {
			cfg: cfg,
			name: name.to_owned(),
			prefix: InstallPrefix::from(cfg.toolchains_dir.join(name), InstallType::Owned),
		}
	}
	pub fn cfg(&self) -> &'a Cfg {
		self.cfg
	}
	pub fn name(&self) -> &str {
		&self.name
	}
	pub fn prefix(&self) -> &InstallPrefix {
		&self.prefix
	}
	pub fn exists(&self) -> bool {
		utils::is_directory(self.prefix.path())
	}
	pub fn verify(&self) -> Result<()> {
		Ok(try!(utils::assert_is_directory(self.prefix.path())))
	}
	pub fn remove(&self) -> Result<()> {
		if self.exists() {
			self.cfg.notify_handler.call(Notification::UninstallingToolchain(&self.name));
		} else {
			self.cfg.notify_handler.call(Notification::ToolchainNotInstalled(&self.name));
			return Ok(());
		}
		if let Some(update_hash) = try!(self.update_hash()) {
			try!(utils::remove_file("update hash", &update_hash));
		}
		let handler = self.cfg.notify_handler.as_ref();
		let result = self.prefix.uninstall(ntfy!(&handler));
		if !self.exists() {
			self.cfg.notify_handler.call(Notification::UninstalledToolchain(&self.name));
		}
		Ok(try!(result))
	}
	pub fn remove_if_exists(&self) -> Result<()> {
		if self.exists() {
			self.remove()
		} else {
			Ok(())
		}
	}
	pub fn install(&self, install_method: InstallMethod) -> Result<()> {
		if self.exists() {
			self.cfg.notify_handler.call(Notification::UpdatingToolchain(&self.name));
		} else {
			self.cfg.notify_handler.call(Notification::InstallingToolchain(&self.name));
		}
		self.cfg.notify_handler.call(Notification::ToolchainDirectory(self.prefix.path(), &self.name));
		let handler = self.cfg.notify_handler.as_ref();
		Ok(try!(self.prefix.install(install_method, ntfy!(&handler))))
	}
	pub fn install_if_not_installed(&self, install_method: InstallMethod) -> Result<()> {
		self.cfg.notify_handler.call(Notification::LookingForToolchain(&self.name));
		if !self.exists() {
			self.install(install_method)
		} else {
			self.cfg.notify_handler.call(Notification::UsingExistingToolchain(&self.name));
			Ok(())
		}
	}
	pub fn update_hash(&self) -> Result<Option<PathBuf>> {
		if self.is_custom() {
			Ok(None)
		} else {
			Ok(Some(try!(self.cfg.get_hash_file(&self.name, true))))
		}
	}
	
	fn download_cfg(&self) -> dist::DownloadCfg {
		dist::DownloadCfg {
			dist_root: &self.cfg.dist_root_url,
			temp_cfg: &self.cfg.temp_cfg,
			notify_handler: ntfy!(&self.cfg.notify_handler),
		}
	}
	
	pub fn install_from_dist(&self) -> Result<()> {
		let update_hash = try!(self.update_hash());
		self.install(InstallMethod::Dist(&self.name, update_hash.as_ref().map(|p| &**p), self.download_cfg()))
	}
	pub fn install_from_dist_if_not_installed(&self) -> Result<()> {
		let update_hash = try!(self.update_hash());
		self.install_if_not_installed(InstallMethod::Dist(&self.name, update_hash.as_ref().map(|p| &**p), self.download_cfg()))
	}
	pub fn is_custom(&self) -> bool {
		dist::ToolchainDesc::from_str(&self.name).is_none()
	}
	pub fn is_tracking(&self) -> bool {
		dist::ToolchainDesc::from_str(&self.name).map(|d| d.is_tracking()) == Some(true)
	}
	
	pub fn ensure_custom(&self) -> Result<()> {
		if !self.is_custom() {
			Err(Error::Install(rust_install::Error::InvalidToolchainName))
		} else {
			Ok(())
		}
	}
	
	pub fn install_from_installers(&self, installers: &[&OsStr]) -> Result<()> {
		try!(self.ensure_custom());
		
		try!(self.remove_if_exists());
		
		let work_dir = try!(self.cfg.temp_cfg.new_directory());
		
		for installer in installers {
			let local_installer;
			let installer_str = installer.to_str();
			if let Some(Ok(url)) = installer_str.map(hyper::Url::parse) {
				// If installer is a URL
				
				// Extract basename from url (eg. 'rust-1.3.0-x86_64-unknown-linux-gnu.tar.gz')
				let re = Regex::new(r"[\\/]([^\\/?]+)(\?.*)?$").unwrap();
				let basename = try!(re.captures(installer_str.unwrap())
					.ok_or(Error::Utils(utils::Error::InvalidUrl { url: installer_str.unwrap().to_owned() }))).at(1).unwrap();
				
				// Download to a local file
				local_installer = Cow::Owned(work_dir.join(basename));
				try!(utils::download_file(url, &local_installer, None, ntfy!(&self.cfg.notify_handler)));
			} else {
				// If installer is a filename
				
				// No need to download
				local_installer = Cow::Borrowed(Path::new(installer));
			}
			
			// Install from file
			try!(self.install(InstallMethod::Installer(&local_installer, &self.cfg.temp_cfg)));
		}
		
		Ok(())
	}
	
	pub fn install_from_dir(&self, src: &Path, link: bool) -> Result<()> {
		if link {
			self.install(InstallMethod::Link(&try!(utils::to_absolute(src))))
		} else {
			self.install(InstallMethod::Copy(src))
		}
	}
	
	pub fn set_env(&self, cmd: &mut Command) {
		self.prefix.set_env(cmd);
		cmd.env("MULTIRUST_TOOLCHAIN", self.prefix.path());
		cmd.env("MULTIRUST_HOME", &self.cfg.multirust_dir);
	}
	
	pub fn create_command(&self, binary: &str) -> Result<Command> {
		if !self.exists() {
			return Err(Error::ToolchainNotInstalled(self.name.to_owned()));
		}
		
		let binary_path = self.prefix.binary_file(binary);
		let mut cmd = Command::new(binary_path);
		self.set_env(&mut cmd);
		Ok(cmd)
	}
	
	pub fn doc_path(&self, relative: &str) -> Result<PathBuf> {
		try!(self.verify());
		Ok(try!(self.prefix.doc_path(relative)))
	}
	pub fn open_docs(&self, relative: &str) -> Result<()> {
		try!(self.verify());
		Ok(try!(self.prefix.open_docs(relative)))
	}
	
	pub fn make_default(&self) -> Result<()> {
		self.cfg.set_default(&self.name)
	}
	pub fn make_override(&self, path: &Path) -> Result<()> {
		Ok(try!(self.cfg.override_db.set(path, &self.name, &self.cfg.temp_cfg, self.cfg.notify_handler.as_ref())))
	}
}
