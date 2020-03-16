use std::borrow::Cow;
use std::fmt::{self, Display};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use pgp::{Deserializable, SignedPublicKey};
use serde::Deserialize;
use thiserror::Error as ThisError;

use crate::dist::download::DownloadCfg;
use crate::dist::{
    dist::{self, valid_profile_names},
    temp,
};
use crate::errors::RustupError;
use crate::fallback_settings::FallbackSettings;
use crate::notifications::*;
use crate::process;
use crate::settings::{Settings, SettingsFile, DEFAULT_METADATA_VERSION};
use crate::toolchain::{DistributableToolchain, Toolchain, UpdateStatus};
use crate::utils::utils;

#[derive(Debug, ThisError)]
enum ConfigError {
    #[error("empty toolchain override file detected. Please remove it, or else specify the desired toolchain properties in the file")]
    EmptyOverrideFile,
    #[error("missing toolchain properties in toolchain override file")]
    InvalidOverrideFile,
    #[error("error parsing override file")]
    ParsingOverrideFile,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
struct OverrideFile {
    toolchain: ToolchainSection,
}

impl OverrideFile {
    fn is_empty(&self) -> bool {
        self.toolchain.is_empty()
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
struct ToolchainSection {
    channel: Option<String>,
    path: Option<PathBuf>,
    components: Option<Vec<String>>,
    targets: Option<Vec<String>>,
    profile: Option<String>,
}

impl ToolchainSection {
    fn is_empty(&self) -> bool {
        self.channel.is_none()
            && self.components.is_none()
            && self.targets.is_none()
            && self.path.is_none()
    }
}

impl<T: Into<String>> From<T> for OverrideFile {
    fn from(channel: T) -> Self {
        let override_ = channel.into();
        if Path::new(&override_).is_absolute() {
            Self {
                toolchain: ToolchainSection {
                    path: Some(PathBuf::from(override_)),
                    ..Default::default()
                },
            }
        } else {
            Self {
                toolchain: ToolchainSection {
                    channel: Some(override_),
                    ..Default::default()
                },
            }
        }
    }
}

#[derive(Debug)]
pub enum OverrideReason {
    Environment,
    CommandLine,
    OverrideDB(PathBuf),
    ToolchainFile(PathBuf),
}

impl Display for OverrideReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        match self {
            Self::Environment => write!(f, "environment override by RUSTUP_TOOLCHAIN"),
            Self::CommandLine => write!(f, "overridden by +toolchain on the command line"),
            Self::OverrideDB(path) => write!(f, "directory override for '{}'", path.display()),
            Self::ToolchainFile(path) => write!(f, "overridden by '{}'", path.display()),
        }
    }
}

#[derive(Default, Debug)]
struct OverrideCfg<'a> {
    toolchain: Option<Toolchain<'a>>,
    components: Vec<String>,
    targets: Vec<String>,
    profile: Option<dist::Profile>,
}

impl<'a> OverrideCfg<'a> {
    fn from_file(
        cfg: &'a Cfg,
        cfg_path: Option<impl AsRef<Path>>,
        file: OverrideFile,
    ) -> Result<Self> {
        Ok(Self {
            toolchain: match (file.toolchain.channel, file.toolchain.path) {
                (Some(name), None) => Some(Toolchain::from(cfg, &name)?),
                (None, Some(path)) => {
                    if file.toolchain.targets.is_some()
                        || file.toolchain.components.is_some()
                        || file.toolchain.profile.is_some()
                    {
                        bail!(
                            "toolchain options are ignored for path toolchain ({})",
                            path.display()
                        )
                    }
                    Some(Toolchain::from_path(cfg, cfg_path, &path)?)
                }
                (Some(channel), Some(path)) => {
                    bail!(
                        "cannot specify both channel ({}) and path ({}) simultaneously",
                        channel,
                        path.display()
                    )
                }
                (None, None) => None,
            },
            components: file.toolchain.components.unwrap_or_default(),
            targets: file.toolchain.targets.unwrap_or_default(),
            profile: file
                .toolchain
                .profile
                .as_deref()
                .map(dist::Profile::from_str)
                .transpose()?,
        })
    }
}

lazy_static::lazy_static! {
    static ref BUILTIN_PGP_KEY: SignedPublicKey = pgp::SignedPublicKey::from_armor_single(
        io::Cursor::new(&include_bytes!("rust-key.pgp.ascii")[..])
    ).unwrap().0;
}

#[allow(clippy::large_enum_variant)] // Builtin is tiny, the rest are sane
#[derive(Debug)]
pub enum PgpPublicKey {
    Builtin,
    FromEnvironment(PathBuf, SignedPublicKey),
    FromConfiguration(PathBuf, SignedPublicKey),
}

impl PgpPublicKey {
    /// Retrieve the key.
    pub fn key(&self) -> &SignedPublicKey {
        match self {
            Self::Builtin => &*BUILTIN_PGP_KEY,
            Self::FromEnvironment(_, k) => &k,
            Self::FromConfiguration(_, k) => &k,
        }
    }

    /// Display the key in detail for the user
    pub fn show_key(&self) -> Result<Vec<String>> {
        fn format_hex(bytes: &[u8], separator: &str, every: usize) -> Result<String> {
            use std::fmt::Write;
            let mut ret = String::new();
            let mut wait = every;
            for b in bytes.iter() {
                if wait == 0 {
                    ret.push_str(separator);
                    wait = every;
                }
                wait -= 1;
                write!(ret, "{:02X}", b)?;
            }
            Ok(ret)
        }
        use pgp::types::KeyTrait;
        let mut ret = vec![format!("from {}", self)];
        let key = self.key();
        let keyid = format_hex(&key.key_id().to_vec(), "-", 4)?;
        let algo = key.algorithm();
        let fpr = format_hex(&key.fingerprint(), " ", 2)?;
        let uid0 = key
            .details
            .users
            .get(0)
            .map(|u| u.id.id())
            .unwrap_or("<No User ID>");
        ret.push(format!("  {:?}/{} - {}", algo, keyid, uid0));
        ret.push(format!("  Fingerprint: {}", fpr));
        Ok(ret)
    }
}

impl Display for PgpPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin => write!(f, "builtin Rust release key"),
            Self::FromEnvironment(p, _) => {
                write!(f, "key specified in RUSTUP_PGP_KEY ({})", p.display())
            }
            Self::FromConfiguration(p, _) => {
                write!(f, "key specified in configuration file ({})", p.display())
            }
        }
    }
}

pub const UNIX_FALLBACK_SETTINGS: &str = "/etc/rustup/settings.toml";

pub struct Cfg {
    pub profile_override: Option<dist::Profile>,
    pub rustup_dir: PathBuf,
    pub settings_file: SettingsFile,
    pub fallback_settings: Option<FallbackSettings>,
    pub toolchains_dir: PathBuf,
    pub update_hash_dir: PathBuf,
    pub download_dir: PathBuf,
    pub temp_cfg: temp::Cfg,
    pgp_keys: Vec<PgpPublicKey>,
    pub toolchain_override: Option<String>,
    pub env_override: Option<String>,
    pub dist_root_url: String,
    pub dist_root_server: String,
    pub notify_handler: Arc<dyn Fn(Notification<'_>)>,
}

impl Cfg {
    pub fn from_env(notify_handler: Arc<dyn Fn(Notification<'_>)>) -> Result<Self> {
        // Set up the rustup home directory
        let rustup_dir = utils::rustup_home()?;

        utils::ensure_dir_exists("home", &rustup_dir, notify_handler.as_ref())?;

        let settings_file = SettingsFile::new(rustup_dir.join("settings.toml"));

        // Centralised file for multi-user systems to provide admin/distributor set initial values.
        let fallback_settings = if cfg!(not(windows)) {
            // If present, use the RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS environment
            // variable as settings path, or UNIX_FALLBACK_SETTINGS otherwise
            FallbackSettings::new(
                match process().var("RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS") {
                    Ok(s) => PathBuf::from(s),
                    Err(_) => PathBuf::from(UNIX_FALLBACK_SETTINGS),
                },
            )?
        } else {
            None
        };

        let toolchains_dir = rustup_dir.join("toolchains");
        let update_hash_dir = rustup_dir.join("update-hashes");
        let download_dir = rustup_dir.join("downloads");

        // PGP keys
        let mut pgp_keys: Vec<PgpPublicKey> = vec![PgpPublicKey::Builtin];

        if let Some(ref s_path) = process().var_os("RUSTUP_PGP_KEY") {
            let path = PathBuf::from(s_path);
            let file = utils::open_file("RUSTUP_PGP_KEY", &path)?;
            let (key, _) = SignedPublicKey::from_armor_single(file).map_err(|error| {
                RustupError::InvalidPgpKey {
                    path: s_path.into(),
                    source: error,
                }
            })?;

            pgp_keys.push(PgpPublicKey::FromEnvironment(path, key));
        }
        settings_file.with(|s| {
            if let Some(s) = &s.pgp_keys {
                let path = PathBuf::from(s);
                let file = utils::open_file("PGP Key from config", &path)?;
                let (key, _) = SignedPublicKey::from_armor_single(file).map_err(|error| {
                    anyhow!(RustupError::InvalidPgpKey {
                        path: s.into(),
                        source: error,
                    })
                })?;

                pgp_keys.push(PgpPublicKey::FromConfiguration(path, key));
            }
            Ok(())
        })?;

        // Environment override
        let env_override = process()
            .var("RUSTUP_TOOLCHAIN")
            .ok()
            .and_then(utils::if_not_empty);

        let dist_root_server = match process().var("RUSTUP_DIST_SERVER") {
            Ok(ref s) if !s.is_empty() => s.clone(),
            _ => {
                // For backward compatibility
                process()
                    .var("RUSTUP_DIST_ROOT")
                    .ok()
                    .and_then(utils::if_not_empty)
                    .map_or(Cow::Borrowed(dist::DEFAULT_DIST_ROOT), Cow::Owned)
                    .as_ref()
                    .trim_end_matches("/dist")
                    .to_owned()
            }
        };

        let notify_clone = notify_handler.clone();
        let temp_cfg = temp::Cfg::new(
            rustup_dir.join("tmp"),
            dist_root_server.as_str(),
            Box::new(move |n| (notify_clone)(n.into())),
        );
        let dist_root = dist_root_server.clone() + "/dist";

        let cfg = Self {
            profile_override: None,
            rustup_dir,
            settings_file,
            fallback_settings,
            toolchains_dir,
            update_hash_dir,
            download_dir,
            temp_cfg,
            pgp_keys,
            notify_handler,
            toolchain_override: None,
            env_override,
            dist_root_url: dist_root,
            dist_root_server,
        };

        // Run some basic checks against the constructed configuration
        // For now, that means simply checking that 'stable' can resolve
        // for the current configuration.
        cfg.resolve_toolchain("stable")
            .context("Unable parse configuration")?;

        Ok(cfg)
    }

    /// construct a download configuration
    pub fn download_cfg<'a>(
        &'a self,
        notify_handler: &'a dyn Fn(crate::dist::Notification<'_>),
    ) -> DownloadCfg<'a> {
        DownloadCfg {
            dist_root: &self.dist_root_url,
            temp_cfg: &self.temp_cfg,
            download_dir: &self.download_dir,
            notify_handler,
            pgp_keys: self.get_pgp_keys(),
        }
    }

    pub fn get_pgp_keys(&self) -> &[PgpPublicKey] {
        &self.pgp_keys
    }

    pub fn set_profile_override(&mut self, profile: dist::Profile) {
        self.profile_override = Some(profile);
    }

    pub fn set_default(&self, toolchain: &str) -> Result<()> {
        self.settings_file.with_mut(|s| {
            s.default_toolchain = Some(toolchain.to_owned());
            Ok(())
        })?;
        (self.notify_handler)(Notification::SetDefaultToolchain(toolchain));
        Ok(())
    }

    pub fn set_profile(&mut self, profile: &str) -> Result<()> {
        if !dist::Profile::names().contains(&profile) {
            return Err(anyhow!(
                "unknown profile name: '{}'; valid profile names are {}",
                profile.to_owned(),
                valid_profile_names(),
            ));
        }
        self.profile_override = None;
        self.settings_file.with_mut(|s| {
            s.profile = Some(profile.to_owned());
            Ok(())
        })?;
        (self.notify_handler)(Notification::SetProfile(profile));
        Ok(())
    }

    pub fn set_toolchain_override(&mut self, toolchain_override: &str) {
        self.toolchain_override = Some(toolchain_override.to_owned());
    }

    // Returns a profile, if one exists in the settings file.
    //
    // Returns `Err` if the settings file could not be read or the profile is
    // invalid. Returns `Ok(...)` if there is a valid profile, and `Ok(Profile::default())`
    // if there is no profile in the settings file. The last variant happens when
    // a user upgrades from a version of Rustup without profiles to a version of
    // Rustup with profiles.
    pub fn get_profile(&self) -> Result<dist::Profile> {
        if let Some(p) = self.profile_override {
            return Ok(p);
        }
        self.settings_file.with(|s| {
            let p = match &s.profile {
                Some(p) => p,
                None => dist::Profile::default_name(),
            };
            let p = dist::Profile::from_str(p)?;
            Ok(p)
        })
    }

    pub fn get_toolchain(&self, name: &str, create_parent: bool) -> Result<Toolchain<'_>> {
        if create_parent {
            utils::ensure_dir_exists("toolchains", &self.toolchains_dir, &|n| {
                (self.notify_handler)(n)
            })?;
        }

        Toolchain::from(self, name)
    }

    pub fn get_hash_file(&self, toolchain: &str, create_parent: bool) -> Result<PathBuf> {
        if create_parent {
            utils::ensure_dir_exists(
                "update-hash",
                &self.update_hash_dir,
                self.notify_handler.as_ref(),
            )?;
        }

        Ok(self.update_hash_dir.join(toolchain))
    }

    pub fn which_binary_by_toolchain(
        &self,
        toolchain: &str,
        binary: &str,
    ) -> Result<Option<PathBuf>> {
        let toolchain = self.get_toolchain(toolchain, false)?;
        if toolchain.exists() {
            Ok(Some(toolchain.binary_file(binary)))
        } else {
            Ok(None)
        }
    }

    pub fn which_binary(&self, path: &Path, binary: &str) -> Result<Option<PathBuf>> {
        let (toolchain, _) = self.find_or_install_override_toolchain_or_default(path)?;
        Ok(Some(toolchain.binary_file(binary)))
    }

    pub fn upgrade_data(&self) -> Result<()> {
        let current_version = self.settings_file.with(|s| Ok(s.version.clone()))?;

        if current_version == DEFAULT_METADATA_VERSION {
            (self.notify_handler)(Notification::MetadataUpgradeNotNeeded(&current_version));
            return Ok(());
        }

        (self.notify_handler)(Notification::UpgradingMetadata(
            &current_version,
            DEFAULT_METADATA_VERSION,
        ));

        match &*current_version {
            "2" => {
                // The toolchain installation format changed. Just delete them all.
                (self.notify_handler)(Notification::UpgradeRemovesToolchains);

                let dirs = utils::read_dir("toolchains", &self.toolchains_dir)?;
                for dir in dirs {
                    let dir = dir.context("IO Error reading toolchains")?;
                    utils::remove_dir("toolchain", &dir.path(), self.notify_handler.as_ref())?;
                }

                // Also delete the update hashes
                let files = utils::read_dir("update hashes", &self.update_hash_dir)?;
                for file in files {
                    let file = file.context("IO Error reading update hashes")?;
                    utils::remove_file("update hash", &file.path())?;
                }

                self.settings_file.with_mut(|s| {
                    s.version = DEFAULT_METADATA_VERSION.to_owned();
                    Ok(())
                })
            }
            _ => Err(RustupError::UnknownMetadataVersion(current_version).into()),
        }
    }

    pub fn find_default(&self) -> Result<Option<Toolchain<'_>>> {
        let opt_name = self.get_default()?;

        if let Some(name) = opt_name {
            let toolchain = Toolchain::from(self, &name)?;
            Ok(Some(toolchain))
        } else {
            Ok(None)
        }
    }

    pub fn find_override(&self, path: &Path) -> Result<Option<(Toolchain<'_>, OverrideReason)>> {
        self.find_override_config(path).map(|opt| {
            opt.and_then(|(override_cfg, reason)| {
                override_cfg.toolchain.map(|toolchain| (toolchain, reason))
            })
        })
    }

    fn find_override_config(
        &self,
        path: &Path,
    ) -> Result<Option<(OverrideCfg<'_>, OverrideReason)>> {
        let mut override_ = None;

        // First check toolchain override from command
        if let Some(ref name) = self.toolchain_override {
            override_ = Some((name.into(), OverrideReason::CommandLine));
        }

        // Check RUSTUP_TOOLCHAIN
        if let Some(ref name) = self.env_override {
            override_ = Some((name.into(), OverrideReason::Environment));
        }

        // Then walk up the directory tree from 'path' looking for either the
        // directory in override database, or a `rust-toolchain` file.
        if override_.is_none() {
            self.settings_file.with(|s| {
                override_ = self.find_override_from_dir_walk(path, s)?;

                Ok(())
            })?;
        }

        if let Some((file, reason)) = override_ {
            // This is hackishly using the error chain to provide a bit of
            // extra context about what went wrong. The CLI will display it
            // on a line after the proximate error.

            let reason_err = match reason {
                OverrideReason::Environment => {
                    "the RUSTUP_TOOLCHAIN environment variable specifies an uninstalled toolchain"
                        .to_string()
                }
                OverrideReason::CommandLine => {
                    "the +toolchain on the command line specifies an uninstalled toolchain"
                        .to_string()
                }
                OverrideReason::OverrideDB(ref path) => format!(
                    "the directory override for '{}' specifies an uninstalled toolchain",
                    utils::canonicalize_path(path, self.notify_handler.as_ref()).display(),
                ),
                OverrideReason::ToolchainFile(ref path) => format!(
                    "the toolchain file at '{}' specifies an uninstalled toolchain",
                    utils::canonicalize_path(path, self.notify_handler.as_ref()).display(),
                ),
            };

            let cfg_file = if let OverrideReason::ToolchainFile(ref path) = reason {
                Some(path)
            } else {
                None
            };

            let override_cfg = OverrideCfg::from_file(self, cfg_file, file)?;
            if let Some(toolchain) = &override_cfg.toolchain {
                // Overridden toolchains can be literally any string, but only
                // distributable toolchains will be auto-installed by the wrapping
                // code; provide a nice error for this common case. (default could
                // be set badly too, but that is much less common).
                if !toolchain.exists() && toolchain.is_custom() {
                    // Strip the confusing NotADirectory error and only mention that the
                    // override toolchain is not installed.
                    return Err(anyhow!(reason_err)).with_context(|| {
                        format!("override toolchain '{}' is not installed", toolchain.name())
                    });
                }
            }

            Ok(Some((override_cfg, reason)))
        } else {
            Ok(None)
        }
    }

    fn find_override_from_dir_walk(
        &self,
        dir: &Path,
        settings: &Settings,
    ) -> Result<Option<(OverrideFile, OverrideReason)>> {
        let notify = self.notify_handler.as_ref();
        let mut dir = Some(dir);

        while let Some(d) = dir {
            // First check the override database
            if let Some(name) = settings.dir_override(d, notify) {
                let reason = OverrideReason::OverrideDB(d.to_owned());
                return Ok(Some((name.into(), reason)));
            }

            // Then look for 'rust-toolchain' or 'rust-toolchain.toml'
            let path_rust_toolchain = d.join("rust-toolchain");
            let path_rust_toolchain_toml = d.join("rust-toolchain.toml");

            let (toolchain_file, contents, parse_mode) = match (
                utils::read_file("toolchain file", &path_rust_toolchain),
                utils::read_file("toolchain file", &path_rust_toolchain_toml),
            ) {
                (contents, Err(_)) => {
                    // no `rust-toolchain.toml` exists
                    (path_rust_toolchain, contents, ParseMode::Both)
                }
                (Err(_), Ok(contents)) => {
                    // only `rust-toolchain.toml` exists
                    (path_rust_toolchain_toml, Ok(contents), ParseMode::OnlyToml)
                }
                (Ok(contents), Ok(_)) => {
                    // both `rust-toolchain` and `rust-toolchain.toml` exist

                    notify(Notification::DuplicateToolchainFile {
                        rust_toolchain: &path_rust_toolchain,
                        rust_toolchain_toml: &path_rust_toolchain_toml,
                    });

                    (path_rust_toolchain, Ok(contents), ParseMode::Both)
                }
            };

            if let Ok(contents) = contents {
                let override_file = Cfg::parse_override_file(contents, parse_mode)?;
                if let Some(toolchain_name) = &override_file.toolchain.channel {
                    let all_toolchains = self.list_toolchains()?;
                    if !all_toolchains.iter().any(|s| s == toolchain_name) {
                        // The given name is not resolvable as a toolchain, so
                        // instead check it's plausible for installation later
                        dist::validate_channel_name(&toolchain_name)?;
                    }
                }

                let reason = OverrideReason::ToolchainFile(toolchain_file);
                return Ok(Some((override_file, reason)));
            }

            dir = d.parent();
        }

        Ok(None)
    }

    fn parse_override_file<S: AsRef<str>>(
        contents: S,
        parse_mode: ParseMode,
    ) -> Result<OverrideFile> {
        let contents = contents.as_ref();

        match (contents.lines().count(), parse_mode) {
            (0, _) => Err(anyhow!(ConfigError::EmptyOverrideFile)),
            (1, ParseMode::Both) => {
                let channel = contents.trim();

                if channel.is_empty() {
                    Err(anyhow!(ConfigError::EmptyOverrideFile))
                } else {
                    Ok(channel.into())
                }
            }
            _ => {
                let override_file = toml::from_str::<OverrideFile>(contents)
                    .context(ConfigError::ParsingOverrideFile)?;

                if override_file.is_empty() {
                    Err(anyhow!(ConfigError::InvalidOverrideFile))
                } else {
                    Ok(override_file)
                }
            }
        }
    }

    pub fn find_or_install_override_toolchain_or_default(
        &self,
        path: &Path,
    ) -> Result<(Toolchain<'_>, Option<OverrideReason>)> {
        fn components_exist(
            distributable: &DistributableToolchain<'_>,
            components: &[&str],
            targets: &[&str],
        ) -> Result<bool> {
            let components_requested = !components.is_empty() || !targets.is_empty();
            // If we're here, the toolchain exists on disk and is a dist toolchain
            // so we should attempt to load its manifest
            let manifest = if let Some(manifest) = distributable.get_manifest()? {
                manifest
            } else {
                // If we can't read the manifest we'd best try and install
                return Ok(false);
            };
            match (distributable.list_components(), components_requested) {
                // If the toolchain does not support components but there were components requested, bubble up the error
                (Err(e), true) => Err(e),
                // Otherwise check if all the components we want are installed
                (Ok(installed_components), _) => Ok(components.iter().all(|name| {
                    installed_components.iter().any(|status| {
                        let cname = status.component.short_name(&manifest);
                        let cname = cname.as_str();
                        let cnameim = status.component.short_name_in_manifest();
                        let cnameim = cnameim.as_str();
                        (cname == *name || cnameim == *name) && status.installed
                    })
                })
                // And that all the targets we want are installed
                && targets.iter().all(|name| {
                    installed_components
                        .iter()
                        .filter(|c| c.component.short_name_in_manifest() == "rust-std")
                        .any(|status| {
                            let ctarg = status.component.target();
                            (ctarg == *name) && status.installed
                        })
                })),
                _ => Ok(true),
            }
        }

        if let Some((toolchain, components, targets, reason, profile)) =
            match self.find_override_config(path)? {
                Some((
                    OverrideCfg {
                        toolchain,
                        components,
                        targets,
                        profile,
                    },
                    reason,
                )) => {
                    let default = if toolchain.is_none() {
                        self.find_default()?
                    } else {
                        None
                    };

                    toolchain
                        .or(default)
                        .map(|toolchain| (toolchain, components, targets, Some(reason), profile))
                }
                None => self
                    .find_default()?
                    .map(|toolchain| (toolchain, vec![], vec![], None, None)),
            }
        {
            if toolchain.is_custom() {
                if !toolchain.exists() {
                    return Err(
                        RustupError::ToolchainNotInstalled(toolchain.name().to_string()).into(),
                    );
                }
            } else {
                let components: Vec<_> = components.iter().map(AsRef::as_ref).collect();
                let targets: Vec<_> = targets.iter().map(AsRef::as_ref).collect();

                let distributable = DistributableToolchain::new(&toolchain)?;
                if !toolchain.exists() || !components_exist(&distributable, &components, &targets)?
                {
                    distributable.install_from_dist(true, false, &components, &targets, profile)?;
                }
            }

            Ok((toolchain, reason))
        } else {
            // No override and no default set
            Err(RustupError::ToolchainNotSelected.into())
        }
    }

    pub fn get_default(&self) -> Result<Option<String>> {
        let user_opt = self.settings_file.with(|s| Ok(s.default_toolchain.clone()));
        if let Some(fallback_settings) = &self.fallback_settings {
            match user_opt {
                Err(_) | Ok(None) => return Ok(fallback_settings.default_toolchain.clone()),
                _ => {}
            };
        };
        user_opt
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

    pub fn list_channels(&self) -> Result<Vec<(String, Result<Toolchain<'_>>)>> {
        let toolchains = self.list_toolchains()?;

        // Convert the toolchain strings to Toolchain values
        let toolchains = toolchains.into_iter();
        let toolchains = toolchains.map(|n| (n.clone(), self.get_toolchain(&n, true)));

        // Filter out toolchains that don't track a release channel
        Ok(toolchains
            .filter(|&(_, ref t)| t.as_ref().map(Toolchain::is_tracking).unwrap_or(false))
            .collect())
    }

    pub fn update_all_channels(
        &self,
        force_update: bool,
    ) -> Result<Vec<(String, Result<UpdateStatus>)>> {
        let channels = self.list_channels()?;
        let channels = channels.into_iter();

        // Update toolchains and collect the results
        let channels = channels.map(|(n, t)| {
            let st = t.and_then(|t| {
                let distributable = DistributableToolchain::new(&t)?;
                let st = distributable.install_from_dist(force_update, false, &[], &[], None);
                if let Err(ref e) = st {
                    (self.notify_handler)(Notification::NonFatalError(e));
                }
                st
            });

            (n, st)
        });

        Ok(channels.collect())
    }

    pub fn check_metadata_version(&self) -> Result<()> {
        utils::assert_is_directory(&self.rustup_dir)?;

        self.settings_file.with(|s| {
            (self.notify_handler)(Notification::ReadMetadataVersion(&s.version));
            if s.version == DEFAULT_METADATA_VERSION {
                Ok(())
            } else {
                Err(anyhow!(
                    "rustup's metadata is out of date. run `rustup self upgrade-data`"
                ))
            }
        })
    }

    pub fn toolchain_for_dir(
        &self,
        path: &Path,
    ) -> Result<(Toolchain<'_>, Option<OverrideReason>)> {
        self.find_or_install_override_toolchain_or_default(path)
    }

    pub fn create_command_for_dir(&self, path: &Path, binary: &str) -> Result<Command> {
        let (ref toolchain, _) = self.toolchain_for_dir(path)?;

        if let Some(cmd) = self.maybe_do_cargo_fallback(toolchain, binary)? {
            Ok(cmd)
        } else {
            // NB this can only fail in race conditions since we used toolchain
            // for dir.
            let installed = toolchain.as_installed_common()?;
            installed.create_command(binary)
        }
    }

    pub fn create_command_for_toolchain(
        &self,
        toolchain: &str,
        install_if_missing: bool,
        binary: &str,
    ) -> Result<Command> {
        let toolchain = self.get_toolchain(toolchain, false)?;
        if install_if_missing && !toolchain.exists() {
            let distributable = DistributableToolchain::new(&toolchain)?;
            distributable.install_from_dist(true, false, &[], &[], None)?;
        }

        if let Some(cmd) = self.maybe_do_cargo_fallback(&toolchain, binary)? {
            Ok(cmd)
        } else {
            // NB note this really can't fail due to to having installed the toolchain if needed
            let installed = toolchain.as_installed_common()?;
            installed.create_command(binary)
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

        // XXX: This could actually consider all distributable toolchains in principle.
        for fallback in &["nightly", "beta", "stable"] {
            let fallback = self.get_toolchain(fallback, false)?;
            if fallback.exists() {
                let distributable = DistributableToolchain::new(&fallback)?;
                let cmd = distributable.create_fallback_command("cargo", toolchain)?;
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
            .resolve(&dist::TargetTriple::new(host_triple))?;
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
                    .map(|s| dist::TargetTriple::new(&s)))
            })?
            .unwrap_or_else(dist::TargetTriple::from_host_or_build))
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

/// Specifies how a `rust-toolchain`/`rust-toolchain.toml` configuration file should be parsed.
enum ParseMode {
    /// Only permit TOML format in a configuration file.
    ///
    /// This variant is used for `rust-toolchain.toml` files (with `.toml` extension).
    OnlyToml,
    /// Permit both the legacy format (i.e. just the channel name) and the TOML format in
    /// a configuration file.
    ///
    /// This variant is used for `rust-toolchain` files (no file extension) for backwards
    /// compatibility.
    Both,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_legacy_toolchain_file() {
        let contents = "nightly-2020-07-10";

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert_eq!(
            result.unwrap(),
            OverrideFile {
                toolchain: ToolchainSection {
                    channel: Some(contents.into()),
                    path: None,
                    components: None,
                    targets: None,
                    profile: None,
                }
            }
        );
    }

    #[test]
    fn parse_toml_toolchain_file() {
        let contents = r#"[toolchain]
channel = "nightly-2020-07-10"
components = [ "rustfmt", "rustc-dev" ]
targets = [ "wasm32-unknown-unknown", "thumbv2-none-eabi" ]
profile = "default"
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert_eq!(
            result.unwrap(),
            OverrideFile {
                toolchain: ToolchainSection {
                    channel: Some("nightly-2020-07-10".into()),
                    path: None,
                    components: Some(vec!["rustfmt".into(), "rustc-dev".into()]),
                    targets: Some(vec![
                        "wasm32-unknown-unknown".into(),
                        "thumbv2-none-eabi".into()
                    ]),
                    profile: Some("default".into()),
                }
            }
        );
    }

    #[test]
    fn parse_toml_toolchain_file_only_channel() {
        let contents = r#"[toolchain]
channel = "nightly-2020-07-10"
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert_eq!(
            result.unwrap(),
            OverrideFile {
                toolchain: ToolchainSection {
                    channel: Some("nightly-2020-07-10".into()),
                    path: None,
                    components: None,
                    targets: None,
                    profile: None,
                }
            }
        );
    }

    #[test]
    fn parse_toml_toolchain_file_only_path() {
        let contents = r#"[toolchain]
path = "foobar"
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert_eq!(
            result.unwrap(),
            OverrideFile {
                toolchain: ToolchainSection {
                    channel: None,
                    path: Some("foobar".into()),
                    components: None,
                    targets: None,
                    profile: None,
                }
            }
        );
    }

    #[test]
    fn parse_toml_toolchain_file_empty_components() {
        let contents = r#"[toolchain]
channel = "nightly-2020-07-10"
components = []
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert_eq!(
            result.unwrap(),
            OverrideFile {
                toolchain: ToolchainSection {
                    channel: Some("nightly-2020-07-10".into()),
                    path: None,
                    components: Some(vec![]),
                    targets: None,
                    profile: None,
                }
            }
        );
    }

    #[test]
    fn parse_toml_toolchain_file_empty_targets() {
        let contents = r#"[toolchain]
channel = "nightly-2020-07-10"
targets = []
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert_eq!(
            result.unwrap(),
            OverrideFile {
                toolchain: ToolchainSection {
                    channel: Some("nightly-2020-07-10".into()),
                    path: None,
                    components: None,
                    targets: Some(vec![]),
                    profile: None,
                }
            }
        );
    }

    #[test]
    fn parse_toml_toolchain_file_no_channel() {
        let contents = r#"[toolchain]
components = [ "rustfmt" ]
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert_eq!(
            result.unwrap(),
            OverrideFile {
                toolchain: ToolchainSection {
                    channel: None,
                    path: None,
                    components: Some(vec!["rustfmt".into()]),
                    targets: None,
                    profile: None,
                }
            }
        );
    }

    #[test]
    fn parse_empty_toml_toolchain_file() {
        let contents = r#"
[toolchain]
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert!(matches!(
            result.unwrap_err().downcast::<ConfigError>(),
            Ok(ConfigError::InvalidOverrideFile)
        ));
    }

    #[test]
    fn parse_empty_toolchain_file() {
        let contents = "";

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert!(matches!(
            result.unwrap_err().downcast::<ConfigError>(),
            Ok(ConfigError::EmptyOverrideFile)
        ));
    }

    #[test]
    fn parse_whitespace_toolchain_file() {
        let contents = "   ";

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert!(matches!(
            result.unwrap_err().downcast::<ConfigError>(),
            Ok(ConfigError::EmptyOverrideFile)
        ));
    }

    #[test]
    fn parse_toml_syntax_error() {
        let contents = r#"[toolchain]
channel = nightly
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert!(matches!(
            result.unwrap_err().downcast::<ConfigError>(),
            Ok(ConfigError::ParsingOverrideFile)
        ));
    }
}
