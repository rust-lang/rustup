use errors::*;
use rust_install::{utils, dist, InstallPrefix, InstallType, InstallMethod};
use rust_install::dist::ToolchainDesc;
use rust_install::manifestation::{Manifestation, Changes};
use rust_manifest::Component;
use config::Cfg;

use std::process::Command;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;

use hyper;
use rust_install;

#[derive(Debug)]
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
        self.cfg
            .notify_handler
            .call(Notification::ToolchainDirectory(self.prefix.path(), &self.name));
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
        self.install(InstallMethod::Dist(&self.name,
                                         update_hash.as_ref().map(|p| &**p),
                                         self.download_cfg()))
    }
    pub fn install_from_dist_if_not_installed(&self) -> Result<()> {
        let update_hash = try!(self.update_hash());
        self.install_if_not_installed(InstallMethod::Dist(&self.name,
                                                          update_hash.as_ref().map(|p| &**p),
                                                          self.download_cfg()))
    }
    pub fn is_custom(&self) -> bool {
        ToolchainDesc::from_str(&self.name).is_err()
    }
    pub fn is_tracking(&self) -> bool {
        ToolchainDesc::from_str(&self.name).ok().map(|d| d.is_tracking()) == Some(true)
    }

    pub fn ensure_custom(&self) -> Result<()> {
        if !self.is_custom() {
            Err(Error::Install(rust_install::Error::InvalidToolchainName(self.name.to_string())))
        } else {
            Ok(())
        }
    }

    pub fn install_from_installers(&self, installers: &[&OsStr]) -> Result<()> {
        try!(self.ensure_custom());

        try!(self.remove_if_exists());

        // FIXME: This should do all downloads first, then do
        // installs, and do it all in a single transaction.
        for installer in installers {
            let installer_str = installer.to_str().unwrap_or("bogus");
            match installer_str.rfind(".") {
                Some(i) => {
                    let extension = &installer_str[i+1..];
                    if extension != "gz" {
                        return Err(Error::BadInstallerType(extension.to_string()));
                    }
                }
                None => return Err(Error::BadInstallerType(String::from("(none)")))
            }

            // FIXME: Pretty hacky
            let is_url = installer_str.starts_with("file://")
                || installer_str.starts_with("http://")
                || installer_str.starts_with("https://");
            let url = hyper::Url::parse(installer_str).ok();
            let url = if is_url { url } else { None };
            if let Some(url) = url {

                // Download to a local file
                let local_installer = try!(self.cfg.temp_cfg.new_file_with_ext("", ".tar.gz"));
                try!(utils::download_file(url,
                                          &local_installer,
                                          None,
                                          ntfy!(&self.cfg.notify_handler)));
                try!(self.install(InstallMethod::Installer(&local_installer, &self.cfg.temp_cfg)));
            } else {
                // If installer is a filename

                // No need to download
                let local_installer = Path::new(installer);

                // Install from file
                try!(self.install(InstallMethod::Installer(&local_installer, &self.cfg.temp_cfg)));
            }
        }

        Ok(())
    }

    pub fn install_from_dir(&self, src: &Path, link: bool) -> Result<()> {
        try!(self.ensure_custom());

        if link {
            self.install(InstallMethod::Link(&try!(utils::to_absolute(src))))
        } else {
            self.install(InstallMethod::Copy(src))
        }
    }

    fn set_env_inner(&self, cmd: &mut Command) {
        cmd.env("MULTIRUST_TOOLCHAIN", self.prefix.path());
        cmd.env("MULTIRUST_HOME", &self.cfg.multirust_dir);
    }

    pub fn set_env(&self, cmd: &mut Command) {
        self.prefix.set_env(cmd, &self.cfg.multirust_dir.join("cargo"));
        self.set_env_inner(cmd);
    }

    pub fn create_command<T: AsRef<OsStr>>(&self, binary: T) -> Result<Command> {
        if !self.exists() {
            return Err(Error::ToolchainNotInstalled(self.name.to_owned()));
        }

        let mut cmd = self.prefix.create_command(binary, &self.cfg.multirust_dir.join("cargo"));
        self.set_env_inner(&mut cmd);
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
        Ok(try!(self.cfg.override_db.set(path,
                                         &self.name,
                                         &self.cfg.temp_cfg,
                                         self.cfg.notify_handler.as_ref())))
    }

    pub fn list_components(&self) -> Result<Vec<Component>> {
        if !self.exists() {
            return Err(Error::ToolchainNotInstalled(self.name.to_owned()));
        }

        // FIXME: This toolchain handling is a mess. Just do it once
        // when the toolchain is created.
        let ref toolchain = self.name;
        let ref toolchain = try!(ToolchainDesc::from_str(toolchain));
        let trip = toolchain.target_triple();
        let manifestation = try!(Manifestation::open(self.prefix.clone(), &trip));

        if let Some(manifest) = try!(manifestation.load_manifest()) {
            // Return all optional components of the "rust" package for the
            // toolchain's target triple.
            let mut res = Vec::new();

            let rust_pkg = manifest.packages.get("rust")
                .expect("manifest should cantain a rust package");
            let targ_pkg = rust_pkg.targets.get(&trip)
                .expect("installed manifest should have a known target");

            for extension in &targ_pkg.extensions {
                res.push(extension.clone())
            }

            Ok(res)
        } else {
            Err(Error::ComponentsUnsupported(self.name.to_string()))
        }
    }

    pub fn add_component(&self, component: Component) -> Result<()> {
        if !self.exists() {
            return Err(Error::ToolchainNotInstalled(self.name.to_owned()));
        }

        let ref toolchain = self.name;
        let ref toolchain = try!(ToolchainDesc::from_str(toolchain));
        let trip = toolchain.target_triple();
        let manifestation = try!(Manifestation::open(self.prefix.clone(), &trip));

        if let Some(manifest) = try!(manifestation.load_manifest()) {

            // Validate the component name
            let rust_pkg = manifest.packages.get("rust")
                .expect("manifest should cantain a rust package");
            let targ_pkg = rust_pkg.targets.get(&trip)
                .expect("installed manifest should have a known target");

            if targ_pkg.components.contains(&component) {
                return Err(Error::AddingRequiredComponent(self.name.to_string(), component));
            }

            if !targ_pkg.extensions.contains(&component) {
                return Err(Error::UnknownComponent(self.name.to_string(), component));
            }

            let changes = Changes {
                add_extensions: vec![component],
                remove_extensions: vec![]
            };

            try!(manifestation.update(&self.name,
                                      &manifest,
                                      changes,
                                      self.download_cfg().temp_cfg,
                                      self.download_cfg().notify_handler.clone()));

            Ok(())
        } else {
            Err(Error::ComponentsUnsupported(self.name.to_string()))
        }
    }

    pub fn remove_component(&self, component: Component) -> Result<()> {
        if !self.exists() {
            return Err(Error::ToolchainNotInstalled(self.name.to_owned()));
        }

        let ref toolchain = self.name;
        let ref toolchain = try!(ToolchainDesc::from_str(toolchain));
        let trip = toolchain.target_triple();
        let manifestation = try!(Manifestation::open(self.prefix.clone(), &trip));

        if let Some(manifest) = try!(manifestation.load_manifest()) {

            // Validate the component name
            let rust_pkg = manifest.packages.get("rust")
                .expect("manifest should cantain a rust package");
            let targ_pkg = rust_pkg.targets.get(&trip)
                .expect("installed manifest should have a known target");

            if targ_pkg.components.contains(&component) {
                return Err(Error::RemovingRequiredComponent(self.name.to_string(), component));
            }

            let dist_config = try!(manifestation.read_config()).unwrap();
            if !dist_config.components.contains(&component) {
                return Err(Error::UnknownComponent(self.name.to_string(), component));
            }

            let changes = Changes {
                add_extensions: vec![],
                remove_extensions: vec![component]
            };

            try!(manifestation.update(&self.name,
                                      &manifest,
                                      changes,
                                      self.download_cfg().temp_cfg,
                                      self.download_cfg().notify_handler.clone()));

            Ok(())
        } else {
            Err(Error::ComponentsUnsupported(self.name.to_string()))
        }
    }
}
