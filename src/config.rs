use std::borrow::Cow;
use std::env;
use std::fmt::{self, Display};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use log::{debug, error, info};

use crate::dist::{dist, temp};
use crate::errors::*;
use crate::notifications::*;
use crate::settings::{Settings, SettingsFile, DEFAULT_METADATA_VERSION};
use crate::toolchain::{Toolchain, UpdateStatus};
use crate::utils::utils;

#[derive(Clone, Copy, Debug)]
pub enum Verbosity {
    Verbose,
    NotVerbose,
}

#[derive(Debug)]
pub enum OverrideReason {
    Environment,
    OverrideDB(PathBuf),
    ToolchainFile(PathBuf),
}

impl Display for OverrideReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> ::std::result::Result<(), fmt::Error> {
        match *self {
            OverrideReason::Environment => write!(f, "environment override by RUSTUP_TOOLCHAIN"),
            OverrideReason::OverrideDB(ref path) => {
                write!(f, "directory override for '{}'", path.display())
            }
            OverrideReason::ToolchainFile(ref path) => {
                write!(f, "overridden by '{}'", path.display())
            }
        }
    }
}

pub struct Cfg {
    pub rustup_dir: PathBuf,
    pub settings_file: SettingsFile,
    pub toolchains_dir: PathBuf,
    pub update_hash_dir: PathBuf,
    pub download_dir: PathBuf,
    pub temp_cfg: temp::Cfg,
    pub gpg_key: Cow<'static, str>,
    pub env_override: Option<String>,
    pub dist_root_url: String,
    pub dist_root_server: String,
    pub verbosity: Verbosity,
    pub notify_handler: Arc<dyn Fn(Notification<'_>)>,
}

impl Cfg {
    pub fn from_env(
        verbosity: Verbosity,
        notify_handler: Arc<dyn Fn(Notification<'_>)>,
    ) -> Result<Self> {
        // Set up the rustup home directory
        let rustup_dir = utils::rustup_home()?;

        utils::ensure_dir_exists("home", &rustup_dir, verbosity)?;

        let settings_file = SettingsFile::new(rustup_dir.join("settings.toml"));
        // Convert from old settings format if necessary
        settings_file.maybe_upgrade_from_legacy(&rustup_dir)?;

        let toolchains_dir = rustup_dir.join("toolchains");
        let update_hash_dir = rustup_dir.join("update-hashes");
        let download_dir = rustup_dir.join("downloads");

        // GPG key
        let gpg_key =
            if let Some(path) = env::var_os("RUSTUP_GPG_KEY").and_then(utils::if_not_empty) {
                Cow::Owned(utils::read_file("public key", Path::new(&path))?)
            } else {
                Cow::Borrowed(include_str!("rust-key.gpg.ascii"))
            };

        // Environment override
        let env_override = env::var("RUSTUP_TOOLCHAIN")
            .ok()
            .and_then(utils::if_not_empty);

        let dist_root_server = match env::var("RUSTUP_DIST_SERVER") {
            Ok(ref s) if !s.is_empty() => s.clone(),
            _ => {
                // For backward compatibility
                env::var("RUSTUP_DIST_ROOT")
                    .ok()
                    .and_then(utils::if_not_empty)
                    .map_or(Cow::Borrowed(dist::DEFAULT_DIST_ROOT), Cow::Owned)
                    .as_ref()
                    .trim_end_matches("/dist")
                    .to_owned()
            }
        };

        let temp_cfg = temp::Cfg::new(rustup_dir.join("tmp"), dist_root_server.as_str(), verbosity);
        let dist_root = dist_root_server.clone() + "/dist";

        let cfg = Cfg {
            rustup_dir: rustup_dir,
            settings_file: settings_file,
            toolchains_dir: toolchains_dir,
            update_hash_dir: update_hash_dir,
            download_dir: download_dir,
            temp_cfg: temp_cfg,
            gpg_key: gpg_key,
            verbosity,
            notify_handler: notify_handler,
            env_override: env_override,
            dist_root_url: dist_root,
            dist_root_server: dist_root_server,
        };

        // Run some basic checks against the constructed configuration
        // For now, that means simply checking that 'stable' can resolve
        // for the current configuration.
        cfg.resolve_toolchain("stable")
            .map_err(|e| format!("Unable parse configuration: {}", e))?;

        Ok(cfg)
    }

    pub fn set_default(&self, toolchain: &str) -> Result<()> {
        self.settings_file.with_mut(|s| {
            s.default_toolchain = Some(toolchain.to_owned());
            Ok(())
        })?;
        info!("default toolchain set to '{}'", toolchain);
        Ok(())
    }

    pub fn get_toolchain(&self, name: &str, create_parent: bool) -> Result<Toolchain<'_>> {
        if create_parent {
            utils::ensure_dir_exists("toolchains", &self.toolchains_dir, self.verbosity)?;
        }

        Toolchain::from(self, name)
    }

    pub fn verify_toolchain(&self, name: &str) -> Result<Toolchain<'_>> {
        let toolchain = self.get_toolchain(name, false)?;
        toolchain.verify()?;
        Ok(toolchain)
    }

    pub fn get_hash_file(&self, toolchain: &str, create_parent: bool) -> Result<PathBuf> {
        if create_parent {
            utils::ensure_dir_exists("update-hash", &self.update_hash_dir, self.verbosity)?;
        }

        Ok(self.update_hash_dir.join(toolchain))
    }

    pub fn which_binary(&self, path: &Path, binary: &str) -> Result<Option<PathBuf>> {
        if let Some((toolchain, _)) = self.find_override_toolchain_or_default(path)? {
            Ok(Some(toolchain.binary_file(binary)))
        } else {
            Ok(None)
        }
    }

    pub fn upgrade_data(&self) -> Result<()> {
        let current_version = self.settings_file.with(|s| Ok(s.version.clone()))?;

        if current_version == DEFAULT_METADATA_VERSION {
            info!(
                "nothing to upgrade: metadata version is already '{}'",
                current_version
            );
            return Ok(());
        }

        info!(
            "upgrading metadata version from '{}' to '{}'",
            current_version, DEFAULT_METADATA_VERSION
        );

        match &*current_version {
            "2" => {
                // The toolchain installation format changed. Just delete them all.
                (self.notify_handler)(Notification::UpgradeRemovesToolchains);

                let dirs = utils::read_dir("toolchains", &self.toolchains_dir)?;
                for dir in dirs {
                    let dir = dir.chain_err(|| ErrorKind::UpgradeIoError)?;
                    utils::remove_dir("toolchain", &dir.path(), self.verbosity)?;
                }

                // Also delete the update hashes
                let files = utils::read_dir("update hashes", &self.update_hash_dir)?;
                for file in files {
                    let file = file.chain_err(|| ErrorKind::UpgradeIoError)?;
                    utils::remove_file("update hash", &file.path())?;
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
        if utils::path_exists(&self.rustup_dir) {
            Ok(utils::remove_dir("home", &self.rustup_dir, self.verbosity)?)
        } else {
            Ok(())
        }
    }

    pub fn find_default(&self) -> Result<Option<Toolchain<'_>>> {
        let opt_name = self
            .settings_file
            .with(|s| Ok(s.default_toolchain.clone()))?;

        if let Some(name) = opt_name {
            let toolchain = self
                .verify_toolchain(&name)
                .chain_err(|| ErrorKind::ToolchainNotInstalled(name.to_string()))?;

            Ok(Some(toolchain))
        } else {
            Ok(None)
        }
    }

    pub fn find_override(&self, path: &Path) -> Result<Option<(Toolchain<'_>, OverrideReason)>> {
        let mut override_ = None;

        // First check RUSTUP_TOOLCHAIN
        if let Some(ref name) = self.env_override {
            override_ = Some((name.to_string(), OverrideReason::Environment));
        }

        // Then walk up the directory tree from 'path' looking for either the
        // directory in override database, or a `rust-toolchain` file.
        if override_.is_none() {
            self.settings_file.with(|s| {
                override_ = self.find_override_from_dir_walk(path, s)?;

                Ok(())
            })?;
        }

        if let Some((name, reason)) = override_ {
            // This is hackishly using the error chain to provide a bit of
            // extra context about what went wrong. The CLI will display it
            // on a line after the proximate error.

            let reason_err = match reason {
                OverrideReason::Environment => format!(
                    "the RUSTUP_TOOLCHAIN environment variable specifies an uninstalled toolchain"
                ),
                OverrideReason::OverrideDB(ref path) => format!(
                    "the directory override for '{}' specifies an uninstalled toolchain",
                    path.display()
                ),
                OverrideReason::ToolchainFile(ref path) => format!(
                    "the toolchain file at '{}' specifies an uninstalled toolchain",
                    path.display()
                ),
            };

            match self.get_toolchain(&name, false) {
                Ok(toolchain) => {
                    if toolchain.exists() {
                        Ok(Some((toolchain, reason)))
                    } else if toolchain.is_custom() {
                        // Strip the confusing NotADirectory error and only mention that the
                        // override toolchain is not installed.
                        Err(Error::from(reason_err)).chain_err(|| {
                            ErrorKind::OverrideToolchainNotInstalled(name.to_string())
                        })
                    } else {
                        toolchain.install_from_dist(false)?;
                        Ok(Some((toolchain, reason)))
                    }
                }
                Err(e) => Err(e)
                    .chain_err(|| Error::from(reason_err))
                    .chain_err(|| ErrorKind::OverrideToolchainNotInstalled(name.to_string())),
            }
        } else {
            Ok(None)
        }
    }

    fn find_override_from_dir_walk(
        &self,
        dir: &Path,
        settings: &Settings,
    ) -> Result<Option<(String, OverrideReason)>> {
        let dir = utils::canonicalize_path(dir);
        let mut dir = Some(&*dir);

        while let Some(d) = dir {
            // First check the override database
            if let Some(name) = settings.dir_override(d) {
                let reason = OverrideReason::OverrideDB(d.to_owned());
                return Ok(Some((name, reason)));
            }

            // Then look for 'rust-toolchain'
            let toolchain_file = d.join("rust-toolchain");
            if let Ok(s) = utils::read_file("toolchain file", &toolchain_file) {
                if let Some(s) = s.lines().next() {
                    let toolchain_name = s.trim();
                    dist::validate_channel_name(&toolchain_name).chain_err(|| {
                        format!(
                            "invalid channel name '{}' in '{}'",
                            toolchain_name,
                            toolchain_file.display()
                        )
                    })?;

                    let reason = OverrideReason::ToolchainFile(toolchain_file);
                    return Ok(Some((toolchain_name.to_string(), reason)));
                }
            }

            dir = d.parent();
        }

        Ok(None)
    }

    pub fn find_override_toolchain_or_default(
        &self,
        path: &Path,
    ) -> Result<Option<(Toolchain<'_>, Option<OverrideReason>)>> {
        Ok(
            if let Some((toolchain, reason)) = self.find_override(path)? {
                Some((toolchain, Some(reason)))
            } else {
                self.find_default()?.map(|toolchain| (toolchain, None))
            },
        )
    }

    pub fn get_default(&self) -> Result<String> {
        self.settings_file
            .with(|s| Ok(s.default_toolchain.clone().unwrap()))
    }

    pub fn list_toolchains(&self) -> Result<Vec<String>> {
        if utils::is_directory(&self.toolchains_dir) {
            let mut toolchains: Vec<_> = utils::read_dir("toolchains", &self.toolchains_dir)?
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

    pub fn update_all_channels(
        &self,
        force_update: bool,
    ) -> Result<Vec<(String, Result<UpdateStatus>)>> {
        let toolchains = self.list_toolchains()?;

        // Convert the toolchain strings to Toolchain values
        let toolchains = toolchains.into_iter();
        let toolchains = toolchains.map(|n| (n.clone(), self.get_toolchain(&n, true)));

        // Filter out toolchains that don't track a release channel
        let toolchains =
            toolchains.filter(|&(_, ref t)| t.as_ref().map(|t| t.is_tracking()).unwrap_or(false));

        // Update toolchains and collect the results
        let toolchains = toolchains.map(|(n, t)| {
            let t = t.and_then(|t| {
                let t = t.install_from_dist(force_update);
                if let Err(ref e) = t {
                    error!("{}", e);
                }
                t
            });

            (n, t)
        });

        Ok(toolchains.collect())
    }

    pub fn check_metadata_version(&self) -> Result<()> {
        utils::assert_is_directory(&self.rustup_dir)?;

        self.settings_file.with(|s| {
            match self.verbosity {
                Verbosity::Verbose => debug!("read metadata version: '{}'", s.version),
                Verbosity::NotVerbose => (),
            };
            if s.version == DEFAULT_METADATA_VERSION {
                Ok(())
            } else {
                Err(ErrorKind::NeedMetadataUpgrade.into())
            }
        })
    }

    pub fn toolchain_for_dir(
        &self,
        path: &Path,
    ) -> Result<(Toolchain<'_>, Option<OverrideReason>)> {
        self.find_override_toolchain_or_default(path)
            .and_then(|r| r.ok_or("no default toolchain configured".into()))
    }

    pub fn create_command_for_dir(&self, path: &Path, binary: &str) -> Result<Command> {
        let (ref toolchain, _) = self.toolchain_for_dir(path)?;

        if let Some(cmd) = self.maybe_do_cargo_fallback(toolchain, binary)? {
            Ok(cmd)
        } else {
            toolchain.create_command(binary)
        }
    }

    pub fn create_command_for_toolchain(
        &self,
        toolchain: &str,
        install_if_missing: bool,
        binary: &str,
    ) -> Result<Command> {
        let ref toolchain = self.get_toolchain(toolchain, false)?;
        if install_if_missing && !toolchain.exists() {
            toolchain.install_from_dist(false)?;
        }

        if let Some(cmd) = self.maybe_do_cargo_fallback(toolchain, binary)? {
            Ok(cmd)
        } else {
            toolchain.create_command(binary)
        }
    }

    // Custom toolchains don't have cargo, so here we detect that situation and
    // try to find a different cargo.
    fn maybe_do_cargo_fallback(
        &self,
        toolchain: &Toolchain<'_>,
        binary: &str,
    ) -> Result<Option<Command>> {
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
            let fallback = self.get_toolchain(fallback, false)?;
            if fallback.exists() {
                let cmd = fallback.create_fallback_command("cargo", toolchain)?;
                return Ok(Some(cmd));
            }
        }

        Ok(None)
    }

    pub fn set_default_host_triple(&self, host_triple: &str) -> Result<()> {
        // Ensure that the provided host_triple is capable of resolving
        // against the 'stable' toolchain.  This provides early errors
        // if the supplied triple is insufficient / bad.
        dist::PartialToolchainDesc::from_str("stable")?
            .resolve(&dist::TargetTriple::from_str(host_triple))?;
        self.settings_file.with_mut(|s| {
            s.default_host_triple = Some(host_triple.to_owned());
            Ok(())
        })
    }

    pub fn get_default_host_triple(&self) -> Result<dist::TargetTriple> {
        Ok(self
            .settings_file
            .with(|s| {
                Ok(s.default_host_triple
                    .as_ref()
                    .map(|s| dist::TargetTriple::from_str(&s)))
            })?
            .unwrap_or_else(dist::TargetTriple::from_build))
    }

    pub fn resolve_toolchain(&self, name: &str) -> Result<String> {
        if let Ok(desc) = dist::PartialToolchainDesc::from_str(name) {
            let host = self.get_default_host_triple()?;
            Ok(desc.resolve(&host)?.to_string())
        } else {
            Ok(name.to_owned())
        }
    }
}
