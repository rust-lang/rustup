use errors::*;
use notifications::*;
use rustup_dist;
use rustup_dist::dist;
use rustup_utils::utils;
use rustup_dist::prefix::InstallPrefix;
use rustup_dist::dist::{ToolchainDesc};
use rustup_dist::manifestation::{Manifestation, Changes};
use rustup_dist::manifest::Component;
use config::Cfg;
use env_var;
use install::{self, InstallMethod};
use telemetry;
use telemetry::{Telemetry, TelemetryEvent};

use std::env::consts::EXE_SUFFIX;
use std::ffi::OsString;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::env;

use url::Url;

/// A fully resolved reference to a toolchain which may or may not exist
pub struct Toolchain<'a> {
    cfg: &'a Cfg,
    name: String,
    path: PathBuf,
    telemetry: telemetry::Telemetry,
    dist_handler: Box<Fn(rustup_dist::Notification) + 'a>,
}

/// Used by the `list_component` function
pub struct ComponentStatus {
    pub component: Component,
    pub required: bool,
    pub installed: bool,
    pub available: bool,
}

pub enum UpdateStatus {
    Installed,
    Updated,
    Unchanged,
}

impl<'a> Toolchain<'a> {
    pub fn from(cfg: &'a Cfg, name: &str) -> Result<Self> {
        let resolved_name = try!(cfg.resolve_toolchain(name));
        let path = cfg.toolchains_dir.join(&resolved_name);
        Ok(Toolchain {
            cfg: cfg,
            name: resolved_name,
            path: path.clone(),
            telemetry: Telemetry::new(cfg.multirust_dir.join("telemetry")),
            dist_handler: Box::new(move |n| {
                (cfg.notify_handler)(n.into())
            })
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn desc(&self) -> Result<ToolchainDesc> {
        Ok(try!(ToolchainDesc::from_str(&self.name)))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn exists(&self) -> bool {
        // HACK: linked toolchains are symlinks, and, contrary to what std docs
        // lead me to believe `fs::metadata`, used by `is_directory` does not
        // seem to follow symlinks on windows.
        let is_symlink = if cfg!(windows) {
            use std::fs;
            fs::symlink_metadata(&self.path).map(|m| m.file_type().is_symlink()).unwrap_or(false)
        } else {
            false
        };
        utils::is_directory(&self.path) || is_symlink
    }
    pub fn verify(&self) -> Result<()> {
        Ok(try!(utils::assert_is_directory(&self.path)))
    }
    pub fn remove(&self) -> Result<()> {
        if self.exists() {
            (self.cfg.notify_handler)(Notification::UninstallingToolchain(&self.name));
        } else {
            (self.cfg.notify_handler)(Notification::ToolchainNotInstalled(&self.name));
            return Ok(());
        }
        if let Some(update_hash) = try!(self.update_hash()) {
            try!(utils::remove_file("update hash", &update_hash));
        }
        let result = install::uninstall(&self.path,
                                        &|n| (self.cfg.notify_handler)(n.into()));
        if !self.exists() {
            (self.cfg.notify_handler)(Notification::UninstalledToolchain(&self.name));
        }
        Ok(try!(result))
    }
    fn install(&self, install_method: InstallMethod) -> Result<UpdateStatus> {
        assert!(self.is_valid_install_method(install_method));
        let exists = self.exists();
        if exists {
            (self.cfg.notify_handler)(Notification::UpdatingToolchain(&self.name));
        } else {
            (self.cfg.notify_handler)(Notification::InstallingToolchain(&self.name));
        }
        (self.cfg.notify_handler)
            (Notification::ToolchainDirectory(&self.path, &self.name));
        let updated = try!(install_method.run(&self.path,
                                              &|n| (self.cfg.notify_handler)(n.into())));

        if !updated {
            (self.cfg.notify_handler)(Notification::UpdateHashMatches);
        } else {
            (self.cfg.notify_handler)(Notification::InstalledToolchain(&self.name));
        }

        let status = match (updated, exists) {
            (true, false) => UpdateStatus::Installed,
            (true, true) => UpdateStatus::Updated,
            (false, true) => UpdateStatus::Unchanged,
            (false, false) => unreachable!(),
        };

        Ok(status)
    }
    fn install_if_not_installed(&self, install_method: InstallMethod) -> Result<UpdateStatus> {
        assert!(self.is_valid_install_method(install_method));
        (self.cfg.notify_handler)(Notification::LookingForToolchain(&self.name));
        if !self.exists() {
            Ok(try!(self.install(install_method)))
        } else {
            (self.cfg.notify_handler)(Notification::UsingExistingToolchain(&self.name));
            Ok(UpdateStatus::Unchanged)
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
            notify_handler: &*self.dist_handler,
        }
    }

    pub fn install_from_dist(&self) -> Result<UpdateStatus> {
        if try!(self.cfg.telemetry_enabled()) {
            return self.install_from_dist_with_telemetry();
        }
        self.install_from_dist_inner()
    }

    pub fn install_from_dist_inner(&self) -> Result<UpdateStatus> {
        let update_hash = try!(self.update_hash());
        self.install(InstallMethod::Dist(&try!(self.desc()),
                                         update_hash.as_ref().map(|p| &**p),
                                         self.download_cfg()))
    }

    pub fn install_from_dist_with_telemetry(&self) -> Result<UpdateStatus> {
        let result = self.install_from_dist_inner();

        match result {
            Ok(us) => {
                let te = TelemetryEvent::ToolchainUpdate { toolchain: self.name().to_string() ,
                                                           success: true };
                match self.telemetry.log_telemetry(te) {
                    Ok(_) => Ok(us),
                    Err(e) => {
                        (self.cfg.notify_handler)(Notification::TelemetryCleanupError(&e));
                        Ok(us)
                    }
                }
            }
            Err(e) => {
                let te = TelemetryEvent::ToolchainUpdate { toolchain: self.name().to_string() ,
                                                           success: true };
                let _ = self.telemetry.log_telemetry(te).map_err(|xe| {
                    (self.cfg.notify_handler)(Notification::TelemetryCleanupError(&xe));
                });
                Err(e)
            }
        }
    }

    pub fn install_from_dist_if_not_installed(&self) -> Result<UpdateStatus> {
        let update_hash = try!(self.update_hash());
        self.install_if_not_installed(InstallMethod::Dist(&try!(self.desc()),
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
            Err(ErrorKind::Dist(::rustup_dist::ErrorKind::InvalidCustomToolchainName(self.name.to_string())).into())
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
            match installer_str.rfind('.') {
                Some(i) => {
                    let extension = &installer_str[i+1..];
                    if extension != "gz" {
                        return Err(ErrorKind::BadInstallerType(extension.to_string()).into());
                    }
                }
                None => return Err(ErrorKind::BadInstallerType(String::from("(none)")).into())
            }

            // FIXME: Pretty hacky
            let is_url = installer_str.starts_with("file://")
                || installer_str.starts_with("http://")
                || installer_str.starts_with("https://");
            let url = Url::parse(installer_str).ok();
            let url = if is_url { url } else { None };
            if let Some(url) = url {

                // Download to a local file
                let local_installer = try!(self.cfg.temp_cfg.new_file_with_ext("", ".tar.gz"));
                try!(utils::download_file(&url,
                                          &local_installer,
                                          None,
                                          &|n| (self.cfg.notify_handler)(n.into())));
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
            try!(self.install(InstallMethod::Link(&try!(utils::to_absolute(src)))));
        } else {
            try!(self.install(InstallMethod::Copy(src)));
        }

        Ok(())
    }

    pub fn create_command<T: AsRef<OsStr>>(&self, binary: T) -> Result<Command> {
        if !self.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(self.name.to_owned()).into());
        }

        // Create the path to this binary within the current toolchain sysroot
        let binary = if let Some(binary_str) = binary.as_ref().to_str() {
            if binary_str.ends_with(EXE_SUFFIX) {
                binary.as_ref().to_owned()
            } else {
                OsString::from(format!("{}{}", binary_str, EXE_SUFFIX))
            }
        } else {
            // Very weird case. Non-unicode command.
            binary.as_ref().to_owned()
        };

        let bin_path = self.path.join("bin").join(&binary);
        let mut cmd = Command::new(if utils::is_file(&bin_path) {
            &bin_path
        } else {
            // If the bin doesn't actually exist in the sysroot, let the OS try
            // to resolve it globally for us
            Path::new(&binary)
        });
        self.set_env(&mut cmd);
        Ok(cmd)
    }

    // Create a command as a fallback for another toolchain. This is used
    // to give custom toolchains access to cargo
    pub fn create_fallback_command<T: AsRef<OsStr>>(&self, binary: T,
                                                    primary_toolchain: &Toolchain) -> Result<Command> {
        // With the hacks below this only works for cargo atm
        assert!(binary.as_ref() == "cargo" || binary.as_ref() == "cargo.exe");

        if !self.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(self.name.to_owned()).into());
        }
        if !primary_toolchain.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(self.name.to_owned()).into());
        }

        let src_file = self.path.join("bin").join(format!("cargo{}", EXE_SUFFIX));

        // MAJOR HACKS: Copy cargo.exe to its own directory on windows before
        // running it. This is so that the fallback cargo, when it in turn runs
        // rustc.exe, will run the rustc.exe out of the PATH environment
        // variable, _not_ the rustc.exe sitting in the same directory as the
        // fallback. See the `fallback_cargo_calls_correct_rustc` testcase and
        // PR 812.
        //
        // On Windows, spawning a process will search the running application's
        // directory for the exe to spawn before searching PATH, and we don't want
        // it to do that, because cargo's directory contains the _wrong_ rustc. See
        // the documantation for the lpCommandLine argument of CreateProcess.
        let exe_path = if cfg!(windows) {
            use std::fs;
            let fallback_dir = self.cfg.multirust_dir.join("fallback");
            try!(fs::create_dir_all(&fallback_dir)
                 .chain_err(|| "unable to create dir to hold fallback exe"));
            let fallback_file = fallback_dir.join("cargo.exe");
            if fallback_file.exists() {
                try!(fs::remove_file(&fallback_file)
                     .chain_err(|| "unable to unlink old fallback exe"));
            }
            try!(fs::hard_link(&src_file, &fallback_file)
                 .chain_err(|| "unable to hard link fallback exe"));
            fallback_file
        } else {
            src_file
        };
        let mut cmd = Command::new(exe_path);
        self.set_env(&mut cmd);
        cmd.env("RUSTUP_TOOLCHAIN", &primary_toolchain.name);
        Ok(cmd)
    }

    fn set_env(&self, cmd: &mut Command) {
        self.set_ldpath(cmd);

        // Because multirust and cargo use slightly different
        // definitions of cargo home (multirust doesn't read HOME on
        // windows), we must set it here to ensure cargo and
        // multirust agree.
        if let Ok(cargo_home) = utils::cargo_home() {
            cmd.env("CARGO_HOME", &cargo_home);
        }

        env_var::inc("RUST_RECURSION_COUNT", cmd);

        cmd.env("RUSTUP_TOOLCHAIN", &self.name);
        cmd.env("RUSTUP_HOME", &self.cfg.multirust_dir);
    }

    pub fn set_ldpath(&self, cmd: &mut Command) {
        let new_path = self.path.join("lib");

        #[cfg(not(target_os = "macos"))]
        mod sysenv {
            pub const LOADER_PATH: &'static str = "LD_LIBRARY_PATH";
        }
        #[cfg(target_os = "macos")]
        mod sysenv {
            pub const LOADER_PATH: &'static str = "DYLD_LIBRARY_PATH";
        }
        env_var::prepend_path(sysenv::LOADER_PATH, &new_path, cmd);

        // Prepend CARGO_HOME/bin to the PATH variable so that we're sure to run
        // cargo/rustc via the proxy bins. There is no fallback case for if the
        // proxy bins don't exist. We'll just be running whatever happens to
        // be on the PATH.
        if let Ok(cargo_home) = utils::cargo_home() {
            env_var::prepend_path("PATH", &cargo_home.join("bin"), cmd);
        }
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
        Ok(try!(self.cfg.settings_file.with_mut(|s| {
            s.add_override(path, self.name.clone(), self.cfg.notify_handler.as_ref());
            Ok(())
        })))
    }

    pub fn list_components(&self) -> Result<Vec<ComponentStatus>> {
        if !self.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(self.name.to_owned()).into());
        }

        let toolchain = &self.name;
        let ref toolchain = try!(ToolchainDesc::from_str(toolchain)
                                 .chain_err(|| ErrorKind::ComponentsUnsupported(self.name.to_string())));
        let prefix = InstallPrefix::from(self.path.to_owned());
        let manifestation = try!(Manifestation::open(prefix, toolchain.target.clone()));

        if let Some(manifest) = try!(manifestation.load_manifest()) {
            let config = try!(manifestation.read_config());

            // Return all optional components of the "rust" package for the
            // toolchain's target triple.
            let mut res = Vec::new();

            let rust_pkg = manifest.packages.get("rust")
                .expect("manifest should cantain a rust package");
            let targ_pkg = rust_pkg.targets.get(&toolchain.target)
                .expect("installed manifest should have a known target");

            for component in &targ_pkg.components {
                let installed = config.as_ref()
                    .map(|c| c.components.contains(component))
                    .unwrap_or(false);

                // Get the component so we can check if it is available
                let component_pkg = manifest.get_package(&component.pkg)
                    .expect(&format!("manifest should contain component {}", &component.pkg));
                let component_target_pkg = component_pkg.targets.get(&toolchain.target)
                    .expect("component should have target toolchain");

                res.push(ComponentStatus {
                    component: component.clone(),
                    required: true,
                    installed: installed,
                    available: component_target_pkg.available,
                });
            }

            for extension in &targ_pkg.extensions {
                let installed = config.as_ref()
                    .map(|c| c.components.contains(extension))
                    .unwrap_or(false);

                // Get the component so we can check if it is available
                let extension_pkg = manifest.get_package(&extension.pkg)
                    .expect(&format!("manifest should contain extension {}", &extension.pkg));
                let extension_target_pkg = extension_pkg.targets.get(&toolchain.target)
                    .expect("extension should have target toolchain");

                res.push(ComponentStatus {
                    component: extension.clone(),
                    required: false,
                    installed: installed,
                    available: extension_target_pkg.available,
                });
            }

            res.sort_by(|a, b| a.component.cmp(&b.component));

            Ok(res)
        } else {
            Err(ErrorKind::ComponentsUnsupported(self.name.to_string()).into())
        }
    }

    pub fn add_component(&self, component: Component) -> Result<()> {
        if try!(self.cfg.telemetry_enabled()) {
            return self.telemetry_add_component(component);
        }
        self.add_component_without_telemetry(component)
    }

    fn telemetry_add_component(&self, component: Component) -> Result<()> {
        let output = self.bare_add_component(component);

        match output {
            Ok(_) => {
                let te = TelemetryEvent::ToolchainUpdate { toolchain: self.name.to_owned(),
                                                           success: true };

                match self.telemetry.log_telemetry(te) {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        (self.cfg.notify_handler)(Notification::TelemetryCleanupError(&e));
                        Ok(())
                    }
                }
            },
            Err(e) => {
                let te = TelemetryEvent::ToolchainUpdate { toolchain: self.name.to_owned(),
                                                           success: false };

                let _ = self.telemetry.log_telemetry(te).map_err(|xe| {
                    (self.cfg.notify_handler)(Notification::TelemetryCleanupError(&xe));
                });
                Err(e)
            }
        }
    }

    fn add_component_without_telemetry(&self, component: Component) -> Result<()> {
        self.bare_add_component(component)
    }

    fn bare_add_component(&self, mut component: Component) -> Result<()> {
        if !self.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(self.name.to_owned()).into());
        }

        let toolchain = &self.name;
        let ref toolchain = try!(ToolchainDesc::from_str(toolchain)
                                 .chain_err(|| ErrorKind::ComponentsUnsupported(self.name.to_string())));
        let prefix = InstallPrefix::from(self.path.to_owned());
        let manifestation = try!(Manifestation::open(prefix, toolchain.target.clone()));

        if let Some(manifest) = try!(manifestation.load_manifest()) {

            // Validate the component name
            let rust_pkg = manifest.packages.get("rust")
                .expect("manifest should cantain a rust package");
            let targ_pkg = rust_pkg.targets.get(&toolchain.target)
                .expect("installed manifest should have a known target");

            if targ_pkg.components.contains(&component) {
                return Err(ErrorKind::AddingRequiredComponent(self.name.to_string(), component).into());
            }

            if !targ_pkg.extensions.contains(&component) {
                let wildcard_component = Component { target: None, ..component.clone() };
                if targ_pkg.extensions.contains(&wildcard_component) {
                    component = wildcard_component;
                } else {
                    return Err(ErrorKind::UnknownComponent(self.name.to_string(), component).into());
                }
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
            Err(ErrorKind::ComponentsUnsupported(self.name.to_string()).into())
        }
    }

    pub fn remove_component(&self, mut component: Component) -> Result<()> {
        if !self.exists() {
            return Err(ErrorKind::ToolchainNotInstalled(self.name.to_owned()).into());
        }

        let toolchain = &self.name;
        let ref toolchain = try!(ToolchainDesc::from_str(toolchain)
                                 .chain_err(|| ErrorKind::ComponentsUnsupported(self.name.to_string())));
        let prefix = InstallPrefix::from(self.path.to_owned());
        let manifestation = try!(Manifestation::open(prefix, toolchain.target.clone()));

        if let Some(manifest) = try!(manifestation.load_manifest()) {

            // Validate the component name
            let rust_pkg = manifest.packages.get("rust")
                .expect("manifest should cantain a rust package");
            let targ_pkg = rust_pkg.targets.get(&toolchain.target)
                .expect("installed manifest should have a known target");

            if targ_pkg.components.contains(&component) {
                return Err(ErrorKind::RemovingRequiredComponent(self.name.to_string(), component).into());
            }

            let dist_config = try!(manifestation.read_config()).unwrap();
            if !dist_config.components.contains(&component) {
                let wildcard_component = Component { target: None, ..component.clone() };
                if dist_config.components.contains(&wildcard_component) {
                    component = wildcard_component;
                } else {
                    return Err(ErrorKind::UnknownComponent(self.name.to_string(), component).into());
                }
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
            Err(ErrorKind::ComponentsUnsupported(self.name.to_string()).into())
        }
    }

    pub fn binary_file(&self, name: &str) -> PathBuf {
        let mut path = self.path.clone();
        path.push("bin");
        path.push(name.to_owned() + env::consts::EXE_SUFFIX);
        path
    }
}
