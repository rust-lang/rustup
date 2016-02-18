use std::path::{Path, PathBuf};
use std::borrow::Cow;
use std::env;
use std::io;
use std::fs;
use std::process::Command;
use std::fmt::{self, Display};

use itertools::Itertools;

use errors::*;
use rust_install::{temp, utils, dist};
use override_db::OverrideDB;
use toolchain::Toolchain;

pub const METADATA_VERSION: &'static str = "2";

#[derive(Debug)]
pub enum OverrideReason {
    Environment,
    OverrideDB(PathBuf),
}

impl Display for OverrideReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
        match *self {
            OverrideReason::Environment => write!(f, "environment override by MULTIRUST_TOOLCHAIN"),
            OverrideReason::OverrideDB(ref path) => {
                write!(f, "directory override due to '{}'", path.display())
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
}

impl Cfg {
    pub fn from_env(notify_handler: SharedNotifyHandler) -> Result<Self> {
        // Get absolute home directory
        let data_dir = try!(utils::get_local_data_path());

        // Set up the multirust home directory
        let multirust_dir = env::var_os("MULTIRUST_HOME")
                                .and_then(utils::if_not_empty)
                                .map_or_else(|| data_dir.join(".multirust"), 
                                             PathBuf::from);

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
        let gpg_key = if let Some(path) = env::var_os("MULTIRUST_GPG_KEY")
                                              .and_then(utils::if_not_empty) {
            Cow::Owned(try!(utils::read_file("public key", Path::new(&path))))
        } else {
            Cow::Borrowed(include_str!("rust-key.gpg.ascii"))
        };

        // Environment override
        let env_override = env::var("MULTIRUST_TOOLCHAIN")
                               .ok()
                               .and_then(utils::if_not_empty);

        let dist_root_url = env::var("MULTIRUST_DIST_ROOT")
                                .ok()
                                .and_then(utils::if_not_empty)
                                .map_or(Cow::Borrowed(dist::DEFAULT_DIST_ROOT),
                                        Cow::Owned);

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

        Ok(Toolchain::from(self, name))
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
            Ok(Some(toolchain.prefix().binary_file(binary)))
        } else {
            Ok(None)
        }
    }

    pub fn upgrade_data(&self) -> Result<bool> {
        if !utils::is_file(&self.version_file) {
            return Ok(false);
        }

        let mut current_version = try!(utils::read_file("version", &self.version_file));
        let len = current_version.trim_right().len();
        current_version.truncate(len);

        self.notify_handler
            .call(Notification::UpgradingMetadata(&current_version, METADATA_VERSION));

        match &*current_version {
            "2" => {
                // Current version. Do nothing
                Ok(false)
            }
            "1" => {
                // Ignore errors. These files may not exist.
                let _ = fs::remove_dir_all(self.multirust_dir.join("available-updates"));
                let _ = fs::remove_dir_all(self.multirust_dir.join("update-sums"));
                let _ = fs::remove_dir_all(self.multirust_dir.join("channel-sums"));
                let _ = fs::remove_dir_all(self.multirust_dir.join("manifests"));

                try!(utils::write_file("version", &self.version_file, METADATA_VERSION));

                Ok(true)
            }
            _ => Err(Error::UnknownMetadataVersion(current_version)),
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

        let toolchain = try!(self.verify_toolchain(name));

        Ok(Some(toolchain))
    }

    pub fn find_override(&self, path: &Path) -> Result<Option<(Toolchain, OverrideReason)>> {
        if let Some(ref name) = self.env_override {
            let toolchain = try!(self.verify_toolchain(name));

            return Ok(Some((toolchain, OverrideReason::Environment)));
        }

        if let Some((name, reason_path)) = try!(self.override_db
                                                    .find(path, self.notify_handler.as_ref())) {
            let toolchain = try!(self.verify_toolchain(&name));
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
            let toolchains: Vec<_> = try!(utils::read_dir("toolchains", &self.toolchains_dir))
                                         .filter_map(io::Result::ok)
                                         .filter_map(|e| e.file_name().into_string().ok())
                                         .collect();

            Ok(toolchains)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn update_all_channels(&self) -> Result<Vec<(String, Result<()>)>> {
        let mut toolchains = try!(self.list_toolchains());
        toolchains.sort();

        Ok(toolchains.into_iter()
                     .merge(["beta", "nightly", "stable"].into_iter().map(|s| (*s).to_owned()))
                     .dedup()
                     .filter(|name| {
                         dist::ToolchainDesc::from_str(&name).map(|d| d.is_tracking()) == Some(true)
                     })
                     .map(|name| {
                         let result = self.get_toolchain(&name, true)
                                          .and_then(|t| t.install_from_dist());
                         if let Err(ref e) = result {
                             self.notify_handler.call(Notification::NonFatalError(e));
                         }
                         (name, result)
                     })
                     .collect())
    }

    pub fn check_metadata_version(&self) -> Result<bool> {
        try!(utils::assert_is_directory(&self.multirust_dir));

        if !utils::is_file(&self.version_file) {
            self.notify_handler.call(Notification::WritingMetadataVersion(METADATA_VERSION));

            try!(utils::write_file("metadata version", &self.version_file, METADATA_VERSION));

            Ok(true)
        } else {
            let current_version = try!(utils::read_file("metadata version", &self.version_file));

            self.notify_handler.call(Notification::ReadMetadataVersion(&current_version));

            Ok(&*current_version == METADATA_VERSION)
        }
    }

    pub fn toolchain_for_dir(&self, path: &Path) -> Result<(Toolchain, Option<OverrideReason>)> {
        self.find_override_toolchain_or_default(path)
            .and_then(|r| r.ok_or(Error::NoDefaultToolchain))
    }

    pub fn create_command_for_dir(&self, path: &Path, binary: &str) -> Result<Command> {
        let (toolchain, _) = try!(self.toolchain_for_dir(path));
        toolchain.create_command(binary)
    }

    pub fn doc_path_for_dir(&self, path: &Path, relative: &str) -> Result<PathBuf> {
        let (toolchain, _) = try!(self.toolchain_for_dir(path));
        toolchain.doc_path(relative)
    }

    pub fn open_docs_for_dir(&self, path: &Path, relative: &str) -> Result<()> {
        let (toolchain, _) = try!(self.toolchain_for_dir(path));
        toolchain.open_docs(relative)
    }
}
