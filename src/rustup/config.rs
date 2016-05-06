use std::path::{Path, PathBuf};
use std::borrow::Cow;
use std::env;
use std::io;
use std::process::Command;
use std::fmt::{self, Display};
use std::str::FromStr;

use itertools::Itertools;

use errors::*;
use notifications::*;
use rustup_dist::{temp, dist};
use rustup_utils::utils;
use override_db::OverrideDB;
use toolchain::{Toolchain, UpdateStatus};
use telemetry::{TelemetryMode};
use telemetry_analysis::*;

// Note: multirust-rs jumped from 2 to 12 to leave multirust.sh room to diverge
pub const METADATA_VERSION: &'static str = "12";

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

#[derive(Debug)]
pub struct Cfg {
    pub multirust_dir: PathBuf,
    pub version_file: PathBuf,
    pub override_db: OverrideDB,
    pub default_file: PathBuf,
    pub toolchains_dir: PathBuf,
    pub update_hash_dir: PathBuf,
    pub temp_cfg: temp::Cfg,
    pub gpg_key: Cow<'static, str>,
    pub env_override: Option<String>,
    pub dist_root_url: Cow<'static, str>,
    pub notify_handler: SharedNotifyHandler,
    pub telemetry_mode: TelemetryMode,
}

impl Cfg {
    pub fn from_env(notify_handler: SharedNotifyHandler) -> Result<Self> {
        // Set up the multirust home directory
        let multirust_dir = try!(utils::multirust_home());

        try!(utils::ensure_dir_exists("home", &multirust_dir, ntfy!(&notify_handler)));

        // Data locations
        let version_file = multirust_dir.join("version");
        let override_db = OverrideDB::new(multirust_dir.join("overrides"));
        let default_file = multirust_dir.join("default");
        let toolchains_dir = multirust_dir.join("toolchains");
        let update_hash_dir = multirust_dir.join("update-hashes");

        let notify_clone = notify_handler.clone();
        let temp_cfg = temp::Cfg::new(multirust_dir.join("tmp"),
                                      shared_ntfy!(move |n: temp::Notification| {
                                          notify_clone.call(Notification::Temp(n));
                                      }));

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

        let dist_root_url = env::var("RUSTUP_DIST_ROOT")
                                .ok()
                                .and_then(utils::if_not_empty)
                                .map_or(Cow::Borrowed(dist::DEFAULT_DIST_ROOT), Cow::Owned);

        let telemetry_mode = Cfg::find_telemetry(&multirust_dir);

        Ok(Cfg {
            multirust_dir: multirust_dir,
            version_file: version_file,
            override_db: override_db,
            default_file: default_file,
            toolchains_dir: toolchains_dir,
            update_hash_dir: update_hash_dir,
            temp_cfg: temp_cfg,
            gpg_key: gpg_key,
            notify_handler: notify_handler,
            env_override: env_override,
            telemetry_mode: telemetry_mode,
            dist_root_url: dist_root_url,
        })
    }

    pub fn set_default(&self, toolchain: &str) -> Result<()> {
        let work_file = try!(self.temp_cfg.new_file());

        try!(utils::write_file("temp", &work_file, toolchain));

        try!(utils::rename_file("default", &*work_file, &self.default_file));

        self.notify_handler.call(Notification::SetDefaultToolchain(toolchain));

        Ok(())
    }

    pub fn get_toolchain(&self, name: &str, create_parent: bool) -> Result<Toolchain> {
        if create_parent {
            try!(utils::ensure_dir_exists("toolchains",
                                          &self.toolchains_dir,
                                          ntfy!(&self.notify_handler)));
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
                                          ntfy!(&self.notify_handler)));
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
        if !utils::is_file(&self.version_file) {
            return Ok(());
        }

        let mut current_version = try!(utils::read_file("version", &self.version_file));
        let len = current_version.trim_right().len();
        current_version.truncate(len);

        if current_version == METADATA_VERSION {
            self.notify_handler
                .call(Notification::MetadataUpgradeNotNeeded(METADATA_VERSION));
            return Ok(());
        }

        self.notify_handler
            .call(Notification::UpgradingMetadata(&current_version, METADATA_VERSION));

        match &*current_version {
            "1" => {
                // This corresponds to an old version of multirust.sh.
                Err(ErrorKind::UnknownMetadataVersion(current_version).into())
            }
            "2" => {
                // The toolchain installation format changed. Just delete them all.
                self.notify_handler
                    .call(Notification::UpgradeRemovesToolchains);

                let dirs = try!(utils::read_dir("toolchains", &self.toolchains_dir));
                for dir in dirs {
                    let dir = try!(dir.chain_err(|| ErrorKind::UpgradeIoError));
                    try!(utils::remove_dir("toolchain", &dir.path(),
                                           ::rustup_utils::NotifyHandler::some(&self.notify_handler)));
                }

                // Also delete the update hashes
                let files = try!(utils::read_dir("update hashes", &self.update_hash_dir));
                for file in files {
                    let file = try!(file.chain_err(|| ErrorKind::UpgradeIoError));
                    try!(utils::remove_file("update hash", &file.path()));
                }

                try!(utils::write_file("version", &self.version_file, METADATA_VERSION));

                Ok(())
            }
            _ => Err(ErrorKind::UnknownMetadataVersion(current_version).into()),
        }
    }

    pub fn delete_data(&self) -> Result<()> {
        if utils::path_exists(&self.multirust_dir) {
            Ok(try!(utils::remove_dir("home", &self.multirust_dir, ntfy!(&self.notify_handler))))
        } else {
            Ok(())
        }
    }

    pub fn find_default(&self) -> Result<Option<Toolchain>> {
        if !utils::is_file(&self.default_file) {
            return Ok(None);
        }
        let content = try!(utils::read_file("default", &self.default_file));
        let name = content.trim_matches('\n');
        if name.is_empty() {
            return Ok(None);
        }

        let toolchain = try!(self.verify_toolchain(name)
                             .chain_err(|| ErrorKind::ToolchainNotInstalled(name.to_string())));

        Ok(Some(toolchain))
    }

    pub fn find_override(&self, path: &Path) -> Result<Option<(Toolchain, OverrideReason)>> {
        if let Some(ref name) = self.env_override {
            let toolchain = try!(self.verify_toolchain(name).chain_err(|| ErrorKind::ToolchainNotInstalled(name.to_string())));

            return Ok(Some((toolchain, OverrideReason::Environment)));
        }

        if let Some((name, reason_path)) = try!(self.override_db
                                                    .find(path, self.notify_handler.as_ref())) {
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

        let updates = toolchains.into_iter()
            .map(|name| {
                let result = self.get_toolchain(&name, true)
                    .and_then(|t| t.install_from_dist());
                if let Err(ref e) = result {
                    self.notify_handler.call(Notification::NonFatalError(e));
                }
                (name, result)
            }).collect();

        Ok(updates)
    }

    pub fn check_metadata_version(&self) -> Result<()> {
        try!(utils::assert_is_directory(&self.multirust_dir));

        if !utils::is_file(&self.version_file) {
            self.notify_handler.call(Notification::WritingMetadataVersion(METADATA_VERSION));

            try!(utils::write_file("metadata version", &self.version_file, METADATA_VERSION));

            Ok(())
        } else {
            let current_version = try!(utils::read_file("metadata version", &self.version_file));

            self.notify_handler.call(Notification::ReadMetadataVersion(&current_version));

            if &*current_version == METADATA_VERSION {
                Ok(())
            } else {
                Err(ErrorKind::NeedMetadataUpgrade.into())
            }
        }
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

    pub fn resolve_toolchain(&self, name: &str) -> Result<String> {
        if let Ok(desc) = dist::PartialToolchainDesc::from_str(name) {
            let host = dist::TargetTriple::from_host();
            Ok(desc.resolve(&host).to_string())
        } else {
            Ok(name.to_owned())
        }
    }

    pub fn set_telemetry(&self, telemetry_enabled: bool) -> Result<()> {
        match telemetry_enabled {
            true => self.enable_telemetry(),
            false => self.disable_telemetry(),
        }
    }

    fn enable_telemetry(&self) -> Result<()> {
        let work_file = try!(self.temp_cfg.new_file());
        
        let _ = utils::ensure_dir_exists("telemetry", &self.multirust_dir.join("telemetry"), ntfy!(&NotifyHandler::none()));

        try!(utils::write_file("temp", &work_file, ""));

        try!(utils::rename_file("telemetry", &*work_file, &self.multirust_dir.join("telemetry-on")));

        self.notify_handler.call(Notification::SetTelemetry("on"));

        Ok(())
    }

    fn disable_telemetry(&self) -> Result<()> {
        let _ = utils::remove_file("telemetry-on", &self.multirust_dir.join("telemetry-on"));

        self.notify_handler.call(Notification::SetTelemetry("off"));

        Ok(())
    }

    pub fn telemetry_enabled(&self) -> bool {
        match self.telemetry_mode {
            TelemetryMode::On => true,
            TelemetryMode::Off => false,
        }
    }

    fn find_telemetry(multirust_dir: &PathBuf) -> TelemetryMode {
        // default telemetry should be off - if no telemetry file is found, it's off
        let telemetry_file = multirust_dir.join("telemetry-on");

        if utils::is_file(telemetry_file) {
            return TelemetryMode::On;
        }

        TelemetryMode::Off
    }

    pub fn analyze_telemetry(&self) -> Result<TelemetryAnalysis> {
        let mut t = TelemetryAnalysis::new(self.multirust_dir.join("telemetry"));

        let events = try!(t.import_telemery());
        try!(t.analyze_telemetry_events(&events));

        Ok(t)
    }
}
