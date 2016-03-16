use errors::*;
use multirust_dist::dist;
use multirust_utils::utils;
use multirust_dist::prefix::InstallPrefix;
use multirust_dist::dist::ToolchainDesc;
use multirust_dist::manifestation::{Manifestation, Changes};
use multirust_dist::manifest::Component;
use config::Cfg;
use env_var;
use install::{self, InstallMethod};

use std::process::Command;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::env;

use hyper;

#[derive(Debug)]
pub struct Toolchain<'a> {
    cfg: &'a Cfg,
    name: String,
    path: PathBuf,
}

impl<'a> Toolchain<'a> {
    pub fn from(cfg: &'a Cfg, name: &str) -> Self {
        let path = cfg.toolchains_dir.join(name);
        Toolchain {
            cfg: cfg,
            name: name.to_owned(),
            path: path.clone(),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn exists(&self) -> bool {
        utils::is_directory(&self.path)
    }
    pub fn verify(&self) -> Result<()> {
        Ok(try!(utils::assert_is_directory(&self.path)))
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
        let result = install::uninstall(&self.path, ntfy!(&handler));
        if !self.exists() {
            self.cfg.notify_handler.call(Notification::UninstalledToolchain(&self.name));
        }
        Ok(try!(result))
    }
    fn install(&self, install_method: InstallMethod) -> Result<()> {
        assert!(self.is_valid_install_method(install_method));
        if self.exists() {
            self.cfg.notify_handler.call(Notification::UpdatingToolchain(&self.name));
        } else {
            self.cfg.notify_handler.call(Notification::InstallingToolchain(&self.name));
        }
        self.cfg
            .notify_handler
            .call(Notification::ToolchainDirectory(&self.path, &self.name));
        let handler = self.cfg.notify_handler.as_ref();
        let updated = try!(install_method.run(&self.path, ntfy!(&handler)));

        if !updated {
            self.cfg.notify_handler.call(Notification::UpdateHashMatches);
        } else {
            self.cfg.notify_handler.call(Notification::InstalledToolchain(&self.name));
        }

        Ok(())
    }
    fn install_if_not_installed(&self, install_method: InstallMethod) -> Result<()> {
        assert!(self.is_valid_install_method(install_method));
        self.cfg.notify_handler.call(Notification::LookingForToolchain(&self.name));
        if !self.exists() {
            self.install(install_method)
        } else {
            self.cfg.notify_handler.call(Notification::UsingExistingToolchain(&self.name));
            Ok(())
        }
    }
    fn is_valid_install_method(&self, install_method: InstallMethod) -> bool {
        match install_method {
            InstallMethod::Copy(_) |
            InstallMethod::Link(_) |
            InstallMethod::Installer(_, _) => self.is_custom(),
            InstallMethod::Dist(_, _, _) => !self.is_custom(),
        }
    }
    fn update_hash(&self) -> Result<Option<PathBuf>> {
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

    fn ensure_custom(&self) -> Result<()> {
        if !self.is_custom() {
            Err(Error::Install(::multirust_dist::Error::InvalidToolchainName(self.name.to_string())))
        } else {
            Ok(())
        }
    }

    pub fn install_from_installers(&self, installers: &[&OsStr]) -> Result<()> {
        try!(self.ensure_custom());

        try!(self.remove());

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

    pub fn create_command<T: AsRef<OsStr>>(&self, binary: T) -> Result<Command> {
        if !self.exists() {
            return Err(Error::ToolchainNotInstalled(self.name.to_owned()));
        }

        let mut cmd = Command::new(binary);
        self.set_env(&mut cmd);
        Ok(cmd)
    }

    fn set_env(&self, cmd: &mut Command) {
        let ref bin_path = self.path.join("bin");

        self.set_ldpath(cmd);

        env_var::set_path("PATH", bin_path, cmd);
        env_var::inc("RUST_RECURSION_COUNT", cmd);

        // FIXME: This should not be a path, but a toolchain name.
        // Not sure what's going on here.
        cmd.env("MULTIRUST_TOOLCHAIN", &self.path);
        cmd.env("MULTIRUST_HOME", &self.cfg.multirust_dir);
    }

    pub fn set_ldpath(&self, cmd: &mut Command) {
        let new_path = self.path.join("lib");

        env_var::set_path("LD_LIBRARY_PATH", &new_path, cmd);
        env_var::set_path("DYLD_LIBRARY_PATH", &new_path, cmd);
    }

    pub fn doc_path(&self, relative: &str) -> Result<PathBuf> {
        try!(self.verify());

        let parts = vec!["share", "doc", "rust", "html"];
        let mut doc_dir = self.path.clone();
        for part in parts {
            doc_dir.push(part);
        }
        doc_dir.push(relative);

        Ok(doc_dir)
    }
    pub fn open_docs(&self, relative: &str) -> Result<()> {
        try!(self.verify());

        Ok(try!(utils::open_browser(&try!(self.doc_path(relative)))))
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
        let prefix = InstallPrefix::from(self.path.to_owned());
        let manifestation = try!(Manifestation::open(prefix, &trip));

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
        let prefix = InstallPrefix::from(self.path.to_owned());
        let manifestation = try!(Manifestation::open(prefix, &trip));

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

            try!(manifestation.update(&manifest,
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
        let prefix = InstallPrefix::from(self.path.to_owned());
        let manifestation = try!(Manifestation::open(prefix, &trip));

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

            try!(manifestation.update(&manifest,
                                      changes,
                                      self.download_cfg().temp_cfg,
                                      self.download_cfg().notify_handler.clone()));

            Ok(())
        } else {
            Err(Error::ComponentsUnsupported(self.name.to_string()))
        }
    }

    pub fn binary_file(&self, name: &str) -> PathBuf {
        let mut path = self.path.clone();
        path.push("bin");
        path.push(name.to_owned() + env::consts::EXE_SUFFIX);
        path
    }
}
