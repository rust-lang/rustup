use std::path::{Path, PathBuf};
use std::borrow::Cow;
use std::env;
use std::io;
use std::process::Command;
use std::fmt::{self, Display};
use std::sync::Arc;

use errors::*;
use notifications::*;
use rustup_dist::{temp, dist};
use rustup_utils::utils;
use toolchain::{Toolchain, UpdateStatus};
use telemetry_analysis::*;
use settings::{TelemetryMode, SettingsFile, DEFAULT_METADATA_VERSION};

#[derive(Debug)]
pub enum OverrideReason {
    Environment,
    OverrideDB(PathBuf),
}



impl Display for OverrideReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        match *self {
            OverrideReason::Environment => write!(f, "environment override by RUSTUP_TOOLCHAIN"),
            OverrideReason::OverrideDB(ref path) => {
                write!(f, "directory override for '{}'", path.display())
            }
        }
    }
}

pub struct Cfg {
    pub multirust_dir: PathBuf,
    pub settings_file: SettingsFile,
    pub toolchains_dir: PathBuf,
    pub update_hash_dir: PathBuf,
    pub temp_cfg: temp::Cfg,
    pub gpg_key: Cow<'static, str>,
    pub env_override: Option<String>,
    pub dist_root_url: String,
    pub dist_root_server: String,
    pub notify_handler: Arc<Fn(Notification)>,
}

impl Cfg {
    pub fn from_env(notify_handler: Arc<Fn(Notification)>) -> Result<Self> {
        // Set up the multirust home directory
        let multirust_dir = try!(utils::multirust_home());

        try!(utils::ensure_dir_exists("home", &multirust_dir,
                                      &|n| notify_handler(n.into())));

        let settings_file = SettingsFile::new(multirust_dir.join("settings.toml"));
        // Convert from old settings format if necessary
        try!(settings_file.maybe_upgrade_from_legacy(&multirust_dir));

        let toolchains_dir = multirust_dir.join("toolchains");
        let update_hash_dir = multirust_dir.join("update-hashes");

        // GPG key
        let gpg_key = if let Some(path) = env::var_os("RUSTUP_GPG_KEY")
                                              .and_then(utils::if_not_empty) {
            Cow::Owned(try!(utils::read_file("public key", Path::new(&path))))
        } else {
            Cow::Borrowed(include_str!("rust-key.gpg.ascii"))
        };

        // Environment override
        let env_override = env::var("RUSTUP_TOOLCHAIN")
                               .ok()
                               .and_then(utils::if_not_empty);

        let dist_root_server = match env::var("RUSTUP_DIST_SERVER") {
            Ok(ref s) if !s.is_empty() => {
                s.clone()
            }
            _ => {
                // For backward compatibility
                env::var("RUSTUP_DIST_ROOT")
                    .ok()
                    .and_then(utils::if_not_empty)
                    .map_or(Cow::Borrowed(dist::DEFAULT_DIST_ROOT), Cow::Owned)
                    .as_ref()
                    .trim_right_matches("/dist")
                    .to_owned()
            }
        };

        let notify_clone = notify_handler.clone();
        let temp_cfg = temp::Cfg::new(multirust_dir.join("tmp"),
                                      dist_root_server.as_str(),
                                      Box::new(move |n| {
                                          (notify_clone)(n.into())
                                      }));
        let dist_root = dist_root_server.clone() + "/dist";

        Ok(Cfg {
            multirust_dir: multirust_dir,
            settings_file: settings_file,
            toolchains_dir: toolchains_dir,
            update_hash_dir: update_hash_dir,
            temp_cfg: temp_cfg,
            gpg_key: gpg_key,
            notify_handler: notify_handler,
            env_override: env_override,
            dist_root_url: dist_root,
            dist_root_server: dist_root_server,
        })
    }

    pub fn set_default(&self, toolchain: &str) -> Result<()> {
        try!(self.settings_file.with_mut(|s| {
            s.default_toolchain = Some(toolchain.to_owned());
            Ok(())
        }));
        (self.notify_handler)(Notification::SetDefaultToolchain(toolchain));
        Ok(())
    }

    pub fn get_toolchain(&self, name: &str, create_parent: bool) -> Result<Toolchain> {
        if create_parent {
            try!(utils::ensure_dir_exists("toolchains",
                                          &self.toolchains_dir,
                                          &|n| (self.notify_handler)(n.into())));
        }

        Toolchain::from(self, name)
    }

    pub fn verify_toolchain(&self, name: &str) -> Result<Toolchain> {
        let toolchain = try!(self.get_toolchain(name, false));
        try!(toolchain.verify());
        Ok(toolchain)
    }

    pub fn get_hash_file(&self, toolchain: &str, create_parent: bool) -> Result<PathBuf> {
        if create_parent {
            try!(utils::ensure_dir_exists("update-hash",
                                          &self.update_hash_dir,
                                          &|n| (self.notify_handler)(n.into())));
        }

        Ok(self.update_hash_dir.join(toolchain))
    }

    pub fn which_binary(&self, path: &Path, binary: &str) -> Result<Option<PathBuf>> {

        if let Some((toolchain, _)) = try!(self.find_override_toolchain_or_default(path)) {
            Ok(Some(toolchain.binary_file(binary)))
        } else {
            Ok(None)
        }
    }

    pub fn upgrade_data(&self) -> Result<()> {

        let current_version = try!(self.settings_file.with(|s| Ok(s.version.clone())));

        if current_version == DEFAULT_METADATA_VERSION {
            (self.notify_handler)
                (Notification::MetadataUpgradeNotNeeded(&current_version));
            return Ok(());
        }

        (self.notify_handler)
            (Notification::UpgradingMetadata(&current_version, DEFAULT_METADATA_VERSION));

        match &*current_version {
            "2" => {
                // The toolchain installation format changed. Just delete them all.
                (self.notify_handler)(Notification::UpgradeRemovesToolchains);

                let dirs = try!(utils::read_dir("toolchains", &self.toolchains_dir));
                for dir in dirs {
                    let dir = try!(dir.chain_err(|| ErrorKind::UpgradeIoError));
                    try!(utils::remove_dir("toolchain", &dir.path(),
                                           &|n| (self.notify_handler)(n.into())));
                }

                // Also delete the update hashes
                let files = try!(utils::read_dir("update hashes", &self.update_hash_dir));
                for file in files {
                    let file = try!(file.chain_err(|| ErrorKind::UpgradeIoError));
                    try!(utils::remove_file("update hash", &file.path()));
                }

                self.settings_file.with_mut(|s| {
                    s.version = DEFAULT_METADATA_VERSION.to_owned();
                    Ok(())
                })
            }
            _ => Err(ErrorKind::UnknownMetadataVersion(current_version).into()),
        }
    }

    pub fn delete_data(&self) -> Result<()> {
        if utils::path_exists(&self.multirust_dir) {
            Ok(try!(utils::remove_dir("home", &self.multirust_dir,
                                      &|n| (self.notify_handler)(n.into()))))
        } else {
            Ok(())
        }
    }

    pub fn find_default(&self) -> Result<Option<Toolchain>> {
        let opt_name = try!(self.settings_file.with(|s| Ok(s.default_toolchain.clone())));

        if let Some(name) = opt_name {
            let toolchain = try!(self.verify_toolchain(&name)
                                 .chain_err(|| ErrorKind::ToolchainNotInstalled(name.to_string())));

            Ok(Some(toolchain))
        } else {
            Ok(None)
        }
    }

    pub fn find_override(&self, path: &Path) -> Result<Option<(Toolchain, OverrideReason)>> {
        if let Some(ref name) = self.env_override {
            let toolchain = try!(self.verify_toolchain(name).chain_err(|| ErrorKind::ToolchainNotInstalled(name.to_string())));

            return Ok(Some((toolchain, OverrideReason::Environment)));
        }

        let result = try!(self.settings_file.with(|s| {
            Ok(s.find_override(path, self.notify_handler.as_ref()))
        }));
        if let Some((name, reason_path)) = result {
            let toolchain = try!(self.verify_toolchain(&name).chain_err(|| ErrorKind::ToolchainNotInstalled(name.to_string())));
            return Ok(Some((toolchain, OverrideReason::OverrideDB(reason_path))));
        }

        Ok(None)
    }

    pub fn find_override_toolchain_or_default
        (&self,
         path: &Path)
         -> Result<Option<(Toolchain, Option<OverrideReason>)>> {
        Ok(if let Some((toolchain, reason)) = try!(self.find_override(path)) {
            Some((toolchain, Some(reason)))
        } else {
            try!(self.find_default()).map(|toolchain| (toolchain, None))
        })
    }

    pub fn list_toolchains(&self) -> Result<Vec<String>> {
        if utils::is_directory(&self.toolchains_dir) {
            let mut toolchains: Vec<_> = try!(utils::read_dir("toolchains", &self.toolchains_dir))
                                         .filter_map(io::Result::ok)
                                         .filter(|e| e.file_type().map(|f| !f.is_file()).unwrap_or(false))
                                         .filter_map(|e| e.file_name().into_string().ok())
                                         .collect();

            utils::toolchain_sort(&mut toolchains);

            Ok(toolchains)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn update_all_channels(&self) -> Result<Vec<(String, Result<UpdateStatus>)>> {
        let toolchains = try!(self.list_toolchains());

        // Convert the toolchain strings to Toolchain values
        let toolchains = toolchains.into_iter();
        let toolchains = toolchains.map(|n| (n.clone(), self.get_toolchain(&n, true)));

        // Filter out toolchains that don't track a release channel
        let toolchains = toolchains.filter(|&(_, ref t)| {
            t.as_ref().map(|t| t.is_tracking()).unwrap_or(false)
        });

        // Update toolchains and collect the results
        let toolchains = toolchains.map(|(n, t)| {
            let t = t.and_then(|t| {
                let t = t.install_from_dist();
                if let Err(ref e) = t {
                    (self.notify_handler)(Notification::NonFatalError(e));
                }
                t
            });

            (n, t)
        });

        Ok(toolchains.collect())
    }

    pub fn check_metadata_version(&self) -> Result<()> {
        try!(utils::assert_is_directory(&self.multirust_dir));

        self.settings_file.with(|s| {
            (self.notify_handler)(Notification::ReadMetadataVersion(&s.version));
            if s.version == DEFAULT_METADATA_VERSION {
                Ok(())
            } else {
                Err(ErrorKind::NeedMetadataUpgrade.into())
            }
        })
    }

    pub fn toolchain_for_dir(&self, path: &Path) -> Result<(Toolchain, Option<OverrideReason>)> {
        self.find_override_toolchain_or_default(path)
            .and_then(|r| r.ok_or("no default toolchain configured".into()))
    }

    pub fn create_command_for_dir(&self, path: &Path, binary: &str) -> Result<Command> {
        let (ref toolchain, _) = try!(self.toolchain_for_dir(path));

        if let Some(cmd) = try!(self.maybe_do_cargo_fallback(toolchain, binary)) {
            Ok(cmd)
        } else {
            toolchain.create_command(binary)
        }
    }

    pub fn create_command_for_toolchain(&self, toolchain: &str, binary: &str) -> Result<Command> {
        let ref toolchain = try!(self.get_toolchain(toolchain, false));

        if let Some(cmd) = try!(self.maybe_do_cargo_fallback(toolchain, binary)) {
            Ok(cmd)
        } else {
            toolchain.create_command(binary)
        }
    }

    // Custom toolchains don't have cargo, so here we detect that situation and
    // try to find a different cargo.
    fn maybe_do_cargo_fallback(&self, toolchain: &Toolchain, binary: &str) -> Result<Option<Command>> {
        if !toolchain.is_custom() {
            return Ok(None);
        }

        if binary != "cargo" && binary != "cargo.exe" {
            return Ok(None);
        }

        let cargo_path = toolchain.path().join("bin/cargo");
        let cargo_exe_path = toolchain.path().join("bin/cargo.exe");

        if cargo_path.exists() || cargo_exe_path.exists() {
            return Ok(None);
        }

        for fallback in &["nightly", "beta", "stable"] {
            let fallback = try!(self.get_toolchain(fallback, false));
            if fallback.exists() {
                let cmd = try!(fallback.create_fallback_command("cargo", toolchain));
                return Ok(Some(cmd));
            }
        }

        Ok(None)
    }

    pub fn doc_path_for_dir(&self, path: &Path, relative: &str) -> Result<PathBuf> {
        let (toolchain, _) = try!(self.toolchain_for_dir(path));
        toolchain.doc_path(relative)
    }

    pub fn open_docs_for_dir(&self, path: &Path, relative: &str) -> Result<()> {
        let (toolchain, _) = try!(self.toolchain_for_dir(path));
        toolchain.open_docs(relative)
    }

    pub fn set_default_host_triple(&self, host_triple: &str) -> Result<()> {
        self.settings_file.with_mut(|s| {
            s.default_host_triple = Some(host_triple.to_owned());
            Ok(())
        })
    }

    pub fn get_default_host_triple(&self) -> Result<dist::TargetTriple> {
        Ok(try!(self.settings_file.with(|s| {
            Ok(s.default_host_triple.as_ref().map(|s| dist::TargetTriple::from_str(&s)))
        })).unwrap_or_else(dist::TargetTriple::from_build))
    }

    pub fn resolve_toolchain(&self, name: &str) -> Result<String> {
        if let Ok(desc) = dist::PartialToolchainDesc::from_str(name) {
            let host = try!(self.get_default_host_triple());
            Ok(desc.resolve(&host).to_string())
        } else {
            Ok(name.to_owned())
        }
    }

    pub fn set_telemetry(&self, telemetry_enabled: bool) -> Result<()> {
        if telemetry_enabled { self.enable_telemetry() } else { self.disable_telemetry() }
    }

    fn enable_telemetry(&self) -> Result<()> {
        try!(self.settings_file.with_mut(|s| {
            s.telemetry = TelemetryMode::On;
            Ok(())
        }));

        let _ = utils::ensure_dir_exists("telemetry", &self.multirust_dir.join("telemetry"),
                                         &|_| ());

        (self.notify_handler)(Notification::SetTelemetry("on"));

        Ok(())
    }

    fn disable_telemetry(&self) -> Result<()> {
        try!(self.settings_file.with_mut(|s| {
            s.telemetry = TelemetryMode::Off;
            Ok(())
        }));

        (self.notify_handler)(Notification::SetTelemetry("off"));

        Ok(())
    }

    pub fn telemetry_enabled(&self) -> Result<bool> {
        Ok(match try!(self.settings_file.with(|s| Ok(s.telemetry))) {
            TelemetryMode::On => true,
            TelemetryMode::Off => false,
        })
    }

    pub fn analyze_telemetry(&self) -> Result<TelemetryAnalysis> {
        let mut t = TelemetryAnalysis::new(self.multirust_dir.join("telemetry"));

        let events = try!(t.import_telemery());
        try!(t.analyze_telemetry_events(&events));

        Ok(t)
    }
}
