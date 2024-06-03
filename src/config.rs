use std::borrow::Cow;
use std::fmt::{self, Debug, Display};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use thiserror::Error as ThisError;
use tokio_stream::StreamExt;

use crate::{
    cli::self_update::SelfUpdateMode,
    currentprocess::{process, varsource::VarSource},
    dist::{
        dist::{self, PartialToolchainDesc, Profile, ToolchainDesc},
        download::DownloadCfg,
        temp,
    },
    errors::RustupError,
    fallback_settings::FallbackSettings,
    install::UpdateStatus,
    notifications::*,
    settings::{Settings, SettingsFile, DEFAULT_METADATA_VERSION},
    toolchain::{
        distributable::DistributableToolchain,
        names::{
            CustomToolchainName, LocalToolchainName, PathBasedToolchainName,
            ResolvableLocalToolchainName, ResolvableToolchainName, ToolchainName,
        },
        toolchain::Toolchain,
    },
    utils::utils,
};

#[derive(Debug, ThisError)]
enum OverrideFileConfigError {
    #[error("empty toolchain override file detected. Please remove it, or else specify the desired toolchain properties in the file")]
    Empty,
    #[error("missing toolchain properties in toolchain override file")]
    Invalid,
    #[error("error parsing override file")]
    Parsing,
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

// Represents the reason why the active toolchain is active.
#[derive(Debug)]
pub(crate) enum ActiveReason {
    Default,
    Environment,
    CommandLine,
    OverrideDB(PathBuf),
    ToolchainFile(PathBuf),
}

impl Display for ActiveReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), fmt::Error> {
        match self {
            Self::Default => write!(f, "it's the default toolchain"),
            Self::Environment => write!(f, "overriden by environment variable RUSTUP_TOOLCHAIN"),
            Self::CommandLine => write!(f, "overridden by +toolchain on the command line"),
            Self::OverrideDB(path) => write!(f, "directory override for '{}'", path.display()),
            Self::ToolchainFile(path) => write!(f, "overridden by '{}'", path.display()),
        }
    }
}

/// Calls Toolchain::new(), but augments the error message with more context
/// from the ActiveReason if the toolchain isn't installed.
pub(crate) fn new_toolchain_with_reason<'a>(
    cfg: &'a Cfg,
    name: LocalToolchainName,
    reason: &ActiveReason,
) -> Result<Toolchain<'a>> {
    match Toolchain::new(cfg, name.clone()) {
        Err(RustupError::ToolchainNotInstalled(_)) => (),
        result => {
            return Ok(result?);
        }
    }

    let reason_err = match reason {
        ActiveReason::Environment => {
            "the RUSTUP_TOOLCHAIN environment variable specifies an uninstalled toolchain"
                .to_string()
        }
        ActiveReason::CommandLine => {
            "the +toolchain on the command line specifies an uninstalled toolchain".to_string()
        }
        ActiveReason::OverrideDB(ref path) => format!(
            "the directory override for '{}' specifies an uninstalled toolchain",
            utils::canonicalize_path(path, cfg.notify_handler.as_ref()).display(),
        ),
        ActiveReason::ToolchainFile(ref path) => format!(
            "the toolchain file at '{}' specifies an uninstalled toolchain",
            utils::canonicalize_path(path, cfg.notify_handler.as_ref()).display(),
        ),
        ActiveReason::Default => {
            "the default toolchain does not describe an installed toolchain".to_string()
        }
    };

    Err(anyhow!(reason_err).context(format!("override toolchain '{name}' is not installed")))
}

// Represents a toolchain override from a +toolchain command line option,
// RUSTUP_TOOLCHAIN environment variable, or rust-toolchain.toml file etc. Can
// include components and targets from a rust-toolchain.toml that should be
// downloaded and installed.
#[derive(Debug)]
enum OverrideCfg {
    PathBased(PathBasedToolchainName),
    Custom(CustomToolchainName),
    Official {
        toolchain: ToolchainDesc,
        components: Vec<String>,
        targets: Vec<String>,
        profile: Option<dist::Profile>,
    },
}

impl OverrideCfg {
    fn from_file(cfg: &Cfg, file: OverrideFile) -> Result<Self> {
        let toolchain_name = match (file.toolchain.channel, file.toolchain.path) {
            (Some(name), None) => {
                ResolvableToolchainName::try_from(name)?.resolve(&cfg.get_default_host_triple()?)?
            }
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
                // We -do not- support relative paths, they permit trivial
                // completely arbitrary code execution in a directory.
                // Longer term we'll not support path based toolchains at
                // all, because they also permit arbitrary code execution,
                // though with more challenges to exploit.
                return Ok(Self::PathBased(PathBasedToolchainName::try_from(
                    &path as &Path,
                )?));
            }
            (Some(channel), Some(path)) => {
                bail!(
                    "cannot specify both channel ({}) and path ({}) simultaneously",
                    channel,
                    path.display()
                )
            }
            (None, None) => cfg
                .get_default()?
                .ok_or(RustupError::ToolchainNotSelected)?,
        };
        Ok(match toolchain_name {
            ToolchainName::Official(desc) => {
                let components = file.toolchain.components.unwrap_or_default();
                let targets = file.toolchain.targets.unwrap_or_default();
                Self::Official {
                    toolchain: desc,
                    components,
                    targets,
                    profile: file
                        .toolchain
                        .profile
                        .as_deref()
                        .map(dist::Profile::from_str)
                        .transpose()?,
                }
            }
            ToolchainName::Custom(name) => {
                if file.toolchain.targets.is_some()
                    || file.toolchain.components.is_some()
                    || file.toolchain.profile.is_some()
                {
                    bail!(
                        "toolchain options are ignored for a custom toolchain ({})",
                        name
                    )
                }
                Self::Custom(name)
            }
        })
    }

    fn into_local_toolchain_name(self) -> LocalToolchainName {
        match self {
            Self::PathBased(path_based_name) => path_based_name.into(),
            Self::Custom(custom_name) => custom_name.into(),
            Self::Official { ref toolchain, .. } => toolchain.into(),
        }
    }
}

impl From<ToolchainName> for OverrideCfg {
    fn from(value: ToolchainName) -> Self {
        match value {
            ToolchainName::Official(desc) => Self::Official {
                toolchain: desc,
                components: vec![],
                targets: vec![],
                profile: None,
            },
            ToolchainName::Custom(name) => Self::Custom(name),
        }
    }
}

impl From<LocalToolchainName> for OverrideCfg {
    fn from(value: LocalToolchainName) -> Self {
        match value {
            LocalToolchainName::Named(name) => Self::from(name),
            LocalToolchainName::Path(path_name) => Self::PathBased(path_name),
        }
    }
}

pub(crate) const UNIX_FALLBACK_SETTINGS: &str = "/etc/rustup/settings.toml";

pub(crate) struct Cfg {
    profile_override: Option<dist::Profile>,
    pub rustup_dir: PathBuf,
    pub settings_file: SettingsFile,
    pub fallback_settings: Option<FallbackSettings>,
    pub toolchains_dir: PathBuf,
    pub update_hash_dir: PathBuf,
    pub download_dir: PathBuf,
    pub tmp_cx: temp::Context,
    pub toolchain_override: Option<ResolvableToolchainName>,
    pub env_override: Option<LocalToolchainName>,
    pub dist_root_url: String,
    pub notify_handler: Arc<dyn Fn(Notification<'_>)>,
}

impl Cfg {
    pub(crate) fn from_env(notify_handler: Arc<dyn Fn(Notification<'_>)>) -> Result<Self> {
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

        // Figure out get_default_host_triple before Config is populated
        let default_host_triple = settings_file.with(|s| Ok(get_default_host_triple(s)))?;
        // Environment override
        let env_override = process()
            .var("RUSTUP_TOOLCHAIN")
            .ok()
            .and_then(utils::if_not_empty)
            .map(ResolvableLocalToolchainName::try_from)
            .transpose()?
            .map(|t| t.resolve(&default_host_triple))
            .transpose()?;

        let dist_root_server = match process().var("RUSTUP_DIST_SERVER") {
            Ok(s) if !s.is_empty() => {
                debug!("`RUSTUP_DIST_SERVER` has been set to `{s}`");
                s
            }
            _ => {
                // For backward compatibility
                process()
                    .var("RUSTUP_DIST_ROOT")
                    .ok()
                    .and_then(utils::if_not_empty)
                    .inspect(|url| debug!("`RUSTUP_DIST_ROOT` has been set to `{url}`"))
                    .map_or(Cow::Borrowed(dist::DEFAULT_DIST_ROOT), Cow::Owned)
                    .as_ref()
                    .trim_end_matches("/dist")
                    .to_owned()
            }
        };

        let notify_clone = notify_handler.clone();
        let tmp_cx = temp::Context::new(
            rustup_dir.join("tmp"),
            dist_root_server.as_str(),
            Box::new(move |n| (notify_clone)(n.into())),
        );
        let dist_root = dist_root_server + "/dist";

        let cfg = Self {
            profile_override: None,
            rustup_dir,
            settings_file,
            fallback_settings,
            toolchains_dir,
            update_hash_dir,
            download_dir,
            tmp_cx,
            notify_handler,
            toolchain_override: None,
            env_override,
            dist_root_url: dist_root,
        };

        // Run some basic checks against the constructed configuration
        // For now, that means simply checking that 'stable' can resolve
        // for the current configuration.
        ResolvableToolchainName::try_from("stable")?.resolve(
            &cfg.get_default_host_triple()
                .context("Unable parse configuration")?,
        )?;

        Ok(cfg)
    }

    /// construct a download configuration
    pub(crate) fn download_cfg<'a>(
        &'a self,
        notify_handler: &'a dyn Fn(crate::dist::Notification<'_>),
    ) -> DownloadCfg<'a> {
        DownloadCfg {
            dist_root: &self.dist_root_url,
            tmp_cx: &self.tmp_cx,
            download_dir: &self.download_dir,
            notify_handler,
        }
    }

    pub(crate) fn set_profile_override(&mut self, profile: dist::Profile) {
        self.profile_override = Some(profile);
    }

    pub(crate) fn set_default(&self, toolchain: Option<&ToolchainName>) -> Result<()> {
        self.settings_file.with_mut(|s| {
            s.default_toolchain = toolchain.map(|t| t.to_string());
            Ok(())
        })?;
        (self.notify_handler)(Notification::SetDefaultToolchain(toolchain));
        Ok(())
    }

    pub(crate) fn set_profile(&mut self, profile: &str) -> Result<()> {
        match Profile::from_str(profile) {
            Ok(p) => {
                self.profile_override = None;
                self.settings_file.with_mut(|s| {
                    s.profile = Some(p);
                    Ok(())
                })?;
                (self.notify_handler)(Notification::SetProfile(profile));
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub(crate) fn set_auto_self_update(&mut self, mode: &str) -> Result<()> {
        match SelfUpdateMode::from_str(mode) {
            Ok(update_mode) => {
                self.settings_file.with_mut(|s| {
                    s.auto_self_update = Some(update_mode);
                    Ok(())
                })?;
                (self.notify_handler)(Notification::SetSelfUpdate(mode));
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub(crate) fn set_toolchain_override(&mut self, toolchain_override: &ResolvableToolchainName) {
        self.toolchain_override = Some(toolchain_override.to_owned());
    }

    // Returns a profile, if one exists in the settings file.
    //
    // Returns `Err` if the settings file could not be read or the profile is
    // invalid. Returns `Ok(...)` if there is a valid profile, and `Ok(Profile::Default)`
    // if there is no profile in the settings file. The last variant happens when
    // a user upgrades from a version of Rustup without profiles to a version of
    // Rustup with profiles.
    pub(crate) fn get_profile(&self) -> Result<dist::Profile> {
        if let Some(p) = self.profile_override {
            return Ok(p);
        }
        self.settings_file.with(|s| {
            let p = match s.profile {
                Some(p) => p,
                None => Profile::Default,
            };
            Ok(p)
        })
    }

    pub(crate) fn get_self_update_mode(&self) -> Result<SelfUpdateMode> {
        self.settings_file.with(|s| {
            let mode = match &s.auto_self_update {
                Some(mode) => mode.clone(),
                None => SelfUpdateMode::Enable,
            };
            Ok(mode)
        })
    }

    pub(crate) fn ensure_toolchains_dir(&self) -> Result<(), anyhow::Error> {
        utils::ensure_dir_exists("toolchains", &self.toolchains_dir, &|n| {
            (self.notify_handler)(n)
        })?;
        Ok(())
    }

    pub(crate) fn get_hash_file(
        &self,
        toolchain: &ToolchainDesc,
        create_parent: bool,
    ) -> Result<PathBuf> {
        if create_parent {
            utils::ensure_dir_exists(
                "update-hash",
                &self.update_hash_dir,
                self.notify_handler.as_ref(),
            )?;
        }

        Ok(self.update_hash_dir.join(toolchain.to_string()))
    }

    pub(crate) async fn which_binary(&self, path: &Path, binary: &str) -> Result<PathBuf> {
        let (toolchain, _) = self.find_or_install_active_toolchain(path).await?;
        Ok(toolchain.binary_file(binary))
    }

    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
    pub(crate) fn upgrade_data(&self) -> Result<()> {
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
                    DEFAULT_METADATA_VERSION.clone_into(&mut s.version);
                    Ok(())
                })
            }
            _ => Err(RustupError::UnknownMetadataVersion(current_version).into()),
        }
    }

    pub(crate) fn find_default(&self) -> Result<Option<Toolchain<'_>>> {
        Ok(self
            .get_default()?
            .map(|n| Toolchain::new(self, (&n).into()))
            .transpose()?)
    }

    pub(crate) fn find_active_toolchain(
        &self,
        path: &Path,
    ) -> Result<Option<(LocalToolchainName, ActiveReason)>> {
        Ok(
            if let Some((override_config, reason)) = self.find_override_config(path)? {
                Some((override_config.into_local_toolchain_name(), reason))
            } else {
                self.get_default()?
                    .map(|x| (x.into(), ActiveReason::Default))
            },
        )
    }

    fn find_override_config(&self, path: &Path) -> Result<Option<(OverrideCfg, ActiveReason)>> {
        let override_config: Option<(OverrideCfg, ActiveReason)> =
            // First check +toolchain override from the command line
            if let Some(ref name) = self.toolchain_override {
                let override_config = name.resolve(&self.get_default_host_triple()?)?.into();
                Some((override_config, ActiveReason::CommandLine))
            }
            // Then check the RUSTUP_TOOLCHAIN environment variable
            else if let Some(ref name) = self.env_override {
                // Because path based toolchain files exist, this has to support
                // custom, distributable, and absolute path toolchains otherwise
                // rustup's export of a RUSTUP_TOOLCHAIN when running a process will
                // error when a nested rustup invocation occurs
                Some((name.clone().into(), ActiveReason::Environment))
            }
            // Then walk up the directory tree from 'path' looking for either the
            // directory in the override database, or a `rust-toolchain{.toml}` file,
            // in that order.
            else if let Some((override_cfg, active_reason)) = self.settings_file.with(|s| {
                    self.find_override_from_dir_walk(path, s)
                })? {
                Some((override_cfg, active_reason))
            }
            // Otherwise, there is no override.
            else {
                None
            };

        Ok(override_config)
    }

    fn find_override_from_dir_walk(
        &self,
        dir: &Path,
        settings: &Settings,
    ) -> Result<Option<(OverrideCfg, ActiveReason)>> {
        let notify = self.notify_handler.as_ref();
        let mut dir = Some(dir);

        while let Some(d) = dir {
            // First check the override database
            if let Some(name) = settings.dir_override(d, notify) {
                let reason = ActiveReason::OverrideDB(d.to_owned());
                // Note that `rustup override set` fully resolves it's input
                // before writing to settings.toml, so resolving here may not
                // be strictly necessary (could instead model as ToolchainName).
                // However, settings.toml could conceivably be hand edited to
                // have an unresolved name. I'm just preserving pre-existing
                // behaviour by choosing ResolvableToolchainName here.
                let toolchain_name = ResolvableToolchainName::try_from(name)?
                    .resolve(&get_default_host_triple(settings))?;
                let override_cfg = toolchain_name.into();
                return Ok(Some((override_cfg, reason)));
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
                // XXX Should not return the unvalidated contents; but a new
                // internal only safe struct
                let override_file =
                    Cfg::parse_override_file(contents, parse_mode).with_context(|| {
                        RustupError::ParsingFile {
                            name: "override",
                            path: toolchain_file.clone(),
                        }
                    })?;
                if let Some(toolchain_name_str) = &override_file.toolchain.channel {
                    let toolchain_name = ResolvableToolchainName::try_from(toolchain_name_str)?;
                    let default_host_triple = get_default_host_triple(settings);
                    // Do not permit architecture/os selection in channels as
                    // these are host specific and toolchain files are portable.
                    if let ResolvableToolchainName::Official(ref name) = toolchain_name {
                        if name.has_triple() {
                            // Permit fully qualified names IFF the toolchain is installed. TODO(robertc): consider
                            // disabling this and backing out https://github.com/rust-lang/rustup/pull/2141 (but provide
                            // the base name in the error to help users)
                            let resolved_name = &ToolchainName::try_from(toolchain_name_str)?;
                            if !self.list_toolchains()?.iter().any(|s| s == resolved_name) {
                                return Err(anyhow!(format!(
                                    "target triple in channel name '{name}'"
                                )));
                            }
                        }
                    }

                    // XXX: this awkwardness deals with settings file being locked already
                    let toolchain_name = toolchain_name.resolve(&default_host_triple)?;
                    match Toolchain::new(self, (&toolchain_name).into()) {
                        Err(RustupError::ToolchainNotInstalled(_)) => {
                            if matches!(toolchain_name, ToolchainName::Custom(_)) {
                                bail!(
                                    "Toolchain {toolchain_name} in {} is custom and not installed",
                                    toolchain_file.display()
                                )
                            }
                        }
                        Ok(_) => {}
                        Err(e) => Err(e)?,
                    }
                }

                let reason = ActiveReason::ToolchainFile(toolchain_file);
                let override_cfg = OverrideCfg::from_file(self, override_file)?;
                return Ok(Some((override_cfg, reason)));
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
            (0, _) => Err(anyhow!(OverrideFileConfigError::Empty)),
            (1, ParseMode::Both) => {
                let channel = contents.trim();

                if channel.is_empty() {
                    Err(anyhow!(OverrideFileConfigError::Empty))
                } else {
                    Ok(channel.into())
                }
            }
            _ => {
                let override_file = toml::from_str::<OverrideFile>(contents)
                    .context(OverrideFileConfigError::Parsing)?;

                if override_file.is_empty() {
                    Err(anyhow!(OverrideFileConfigError::Invalid))
                } else {
                    Ok(override_file)
                }
            }
        }
    }

    pub(crate) async fn find_or_install_active_toolchain(
        &self,
        path: &Path,
    ) -> Result<(Toolchain<'_>, ActiveReason)> {
        self.maybe_find_or_install_active_toolchain(path)
            .await?
            .ok_or(RustupError::ToolchainNotSelected.into())
    }

    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
    pub(crate) async fn maybe_find_or_install_active_toolchain(
        &self,
        path: &Path,
    ) -> Result<Option<(Toolchain<'_>, ActiveReason)>> {
        match self.find_override_config(path)? {
            Some((override_config, reason)) => match override_config {
                OverrideCfg::PathBased(path_based_name) => {
                    let toolchain =
                        new_toolchain_with_reason(self, path_based_name.into(), &reason)?;
                    Ok(Some((toolchain, reason)))
                }
                OverrideCfg::Custom(custom_name) => {
                    let toolchain = new_toolchain_with_reason(self, custom_name.into(), &reason)?;
                    Ok(Some((toolchain, reason)))
                }
                OverrideCfg::Official {
                    toolchain,
                    components,
                    targets,
                    profile,
                } => {
                    let toolchain = self
                        .ensure_installed(toolchain, components, targets, profile)
                        .await?;
                    Ok(Some((toolchain, reason)))
                }
            },
            None => match self.get_default()? {
                None => Ok(None),
                Some(ToolchainName::Custom(custom_name)) => {
                    let reason = ActiveReason::Default;
                    let toolchain = new_toolchain_with_reason(self, custom_name.into(), &reason)?;
                    Ok(Some((toolchain, reason)))
                }
                Some(ToolchainName::Official(toolchain_desc)) => {
                    let reason = ActiveReason::Default;
                    let toolchain = self
                        .ensure_installed(toolchain_desc, vec![], vec![], None)
                        .await?;
                    Ok(Some((toolchain, reason)))
                }
            },
        }
    }

    // Returns a Toolchain matching the given ToolchainDesc, installing it and
    // the given components and targets if they aren't already installed.
    async fn ensure_installed(
        &self,
        toolchain: ToolchainDesc,
        components: Vec<String>,
        targets: Vec<String>,
        profile: Option<Profile>,
    ) -> Result<Toolchain<'_>> {
        let components: Vec<_> = components.iter().map(AsRef::as_ref).collect();
        let targets: Vec<_> = targets.iter().map(AsRef::as_ref).collect();
        let toolchain = match DistributableToolchain::new(self, toolchain.clone()) {
            Err(RustupError::ToolchainNotInstalled(_)) => {
                DistributableToolchain::install(
                    self,
                    &toolchain,
                    &components,
                    &targets,
                    profile.unwrap_or(Profile::Default),
                    false,
                )
                .await?
                .1
            }
            Ok(mut distributable) => {
                if !distributable.components_exist(&components, &targets)? {
                    utils::run_future(distributable.update(
                        &components,
                        &targets,
                        profile.unwrap_or(Profile::Default),
                    ))?;
                }
                distributable
            }
            Err(e) => return Err(e.into()),
        }
        .into();
        Ok(toolchain)
    }

    /// Get the configured default toolchain.
    /// If none is configured, returns None
    /// If a bad toolchain name is configured, errors.
    pub(crate) fn get_default(&self) -> Result<Option<ToolchainName>> {
        let user_opt = self.settings_file.with(|s| Ok(s.default_toolchain.clone()));
        let toolchain_maybe_str = if let Some(fallback_settings) = &self.fallback_settings {
            match user_opt {
                Err(_) | Ok(None) => Ok(fallback_settings.default_toolchain.clone()),
                o => o,
            }
        } else {
            user_opt
        }?;
        toolchain_maybe_str
            .map(ResolvableToolchainName::try_from)
            .transpose()?
            .map(|t| t.resolve(&self.get_default_host_triple()?))
            .transpose()
    }

    /// List all the installed toolchains: that is paths in the toolchain dir
    /// that are:
    /// - not files
    /// - named with a valid resolved toolchain name
    /// Currently no notification of incorrect names or entry type is done.
    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
    pub(crate) fn list_toolchains(&self) -> Result<Vec<ToolchainName>> {
        if utils::is_directory(&self.toolchains_dir) {
            let mut toolchains: Vec<_> = utils::read_dir("toolchains", &self.toolchains_dir)?
                // TODO: this discards errors reading the directory, is that
                // correct? could we get a short-read and report less toolchains
                // than exist?
                .filter_map(io::Result::ok)
                .filter(|e| e.file_type().map(|f| !f.is_file()).unwrap_or(false))
                .filter_map(|e| e.file_name().into_string().ok())
                .filter_map(|n| ToolchainName::try_from(&n).ok())
                .collect();

            crate::toolchain::names::toolchain_sort(&mut toolchains);

            Ok(toolchains)
        } else {
            Ok(Vec::new())
        }
    }

    pub(crate) fn list_channels(&self) -> Result<Vec<(ToolchainDesc, DistributableToolchain<'_>)>> {
        self.list_toolchains()?
            .into_iter()
            .filter_map(|t| {
                if let ToolchainName::Official(desc) = t {
                    Some(desc)
                } else {
                    None
                }
            })
            .filter(ToolchainDesc::is_tracking)
            .map(|n| {
                DistributableToolchain::new(self, n.clone())
                    .map_err(Into::into)
                    .map(|t| (n.clone(), t))
            })
            .collect::<Result<Vec<_>>>()
    }

    /// Create an override for a toolchain
    pub(crate) fn make_override(&self, path: &Path, toolchain: &ToolchainName) -> Result<()> {
        self.settings_file.with_mut(|s| {
            s.add_override(path, toolchain.to_string(), self.notify_handler.as_ref());
            Ok(())
        })
    }

    pub(crate) async fn update_all_channels(
        &self,
        force_update: bool,
    ) -> Result<Vec<(ToolchainDesc, Result<UpdateStatus>)>> {
        let channels = self.list_channels()?;
        let channels = channels.into_iter();
        let profile = self.get_profile()?;

        // Update toolchains and collect the results
        let channels = tokio_stream::iter(channels).then(|(desc, mut distributable)| async move {
            let st = distributable
                .update_extra(&[], &[], profile, force_update, false)
                .await;
            if let Err(ref e) = st {
                (self.notify_handler)(Notification::NonFatalError(e));
            }
            (desc, st)
        });

        Ok(channels.collect().await)
    }

    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
    pub(crate) fn check_metadata_version(&self) -> Result<()> {
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

    pub(crate) async fn create_command_for_dir(
        &self,
        path: &Path,
        binary: &str,
    ) -> Result<Command> {
        let (toolchain, _) = self.find_or_install_active_toolchain(path).await?;
        self.create_command_for_toolchain_(toolchain, binary)
    }

    pub(crate) fn create_command_for_toolchain(
        &self,
        toolchain_name: &LocalToolchainName,
        install_if_missing: bool,
        binary: &str,
    ) -> Result<Command> {
        match toolchain_name {
            LocalToolchainName::Named(ToolchainName::Official(desc)) => {
                match DistributableToolchain::new(self, desc.clone()) {
                    Err(RustupError::ToolchainNotInstalled(_)) => {
                        if install_if_missing {
                            utils::run_future(DistributableToolchain::install(
                                self,
                                desc,
                                &[],
                                &[],
                                self.get_profile()?,
                                true,
                            ))?;
                        }
                    }
                    o => {
                        o?;
                    }
                }
            }
            n => {
                if !Toolchain::exists(self, n)? {
                    return Err(RustupError::ToolchainNotInstallable(n.to_string()).into());
                }
            }
        }

        let toolchain = Toolchain::new(self, toolchain_name.clone())?;

        // NB this can only fail in race conditions since we handle existence above
        // for dir.
        self.create_command_for_toolchain_(toolchain, binary)
    }

    fn create_command_for_toolchain_(
        &self,
        toolchain: Toolchain<'_>,
        binary: &str,
    ) -> Result<Command> {
        // Should push the cargo fallback into a custom toolchain type? And then
        // perhaps a trait that create command layers on?
        if !matches!(
            toolchain.name(),
            LocalToolchainName::Named(ToolchainName::Official(_))
        ) {
            if let Some(cmd) = self.maybe_do_cargo_fallback(&toolchain, binary)? {
                return Ok(cmd);
            }
        }

        toolchain.create_command(binary)
    }

    // Custom toolchains don't have cargo, so here we detect that situation and
    // try to find a different cargo.
    fn maybe_do_cargo_fallback(
        &self,
        toolchain: &Toolchain<'_>,
        binary: &str,
    ) -> Result<Option<Command>> {
        if binary != "cargo" && binary != "cargo.exe" {
            return Ok(None);
        }

        let cargo_path = toolchain.binary_file("cargo");

        // breadcrumb in case of regression: we used to get the cargo path and
        // cargo.exe path separately, not using the binary_file helper. This may
        // matter if calling a binary with some personality that allows .exe and
        // not .exe to coexist (e.g. wine) - but that's not something we aim to
        // support : the host should always be correct.
        if cargo_path.exists() {
            return Ok(None);
        }

        let default_host_triple = self.get_default_host_triple()?;
        // XXX: This could actually consider all installed distributable
        // toolchains in principle.
        for fallback in ["nightly", "beta", "stable"] {
            let resolved =
                PartialToolchainDesc::from_str(fallback)?.resolve(&default_host_triple)?;
            if let Ok(fallback) =
                crate::toolchain::distributable::DistributableToolchain::new(self, resolved)
            {
                let cmd = fallback.create_fallback_command("cargo", toolchain)?;
                return Ok(Some(cmd));
            }
        }

        Ok(None)
    }

    pub(crate) fn set_default_host_triple(&self, host_triple: String) -> Result<()> {
        // Ensure that the provided host_triple is capable of resolving
        // against the 'stable' toolchain.  This provides early errors
        // if the supplied triple is insufficient / bad.
        dist::PartialToolchainDesc::from_str("stable")?
            .resolve(&dist::TargetTriple::new(host_triple.clone()))?;
        self.settings_file.with_mut(|s| {
            s.default_host_triple = Some(host_triple);
            Ok(())
        })
    }

    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
    pub(crate) fn get_default_host_triple(&self) -> Result<dist::TargetTriple> {
        self.settings_file.with(|s| Ok(get_default_host_triple(s)))
    }

    /// The path on disk of any concrete toolchain
    pub(crate) fn toolchain_path(&self, toolchain: &LocalToolchainName) -> PathBuf {
        match toolchain {
            LocalToolchainName::Named(name) => self.toolchains_dir.join(name.to_string()),
            LocalToolchainName::Path(p) => p.to_path_buf(),
        }
    }
}

impl Debug for Cfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            profile_override,
            rustup_dir,
            settings_file,
            fallback_settings,
            toolchains_dir,
            update_hash_dir,
            download_dir,
            tmp_cx,
            toolchain_override,
            env_override,
            dist_root_url,
            notify_handler: _,
        } = self;

        f.debug_struct("Cfg")
            .field("profile_override", profile_override)
            .field("rustup_dir", rustup_dir)
            .field("settings_file", settings_file)
            .field("fallback_settings", fallback_settings)
            .field("toolchains_dir", toolchains_dir)
            .field("update_hash_dir", update_hash_dir)
            .field("download_dir", download_dir)
            .field("tmp_cx", tmp_cx)
            .field("toolchain_override", toolchain_override)
            .field("env_override", env_override)
            .field("dist_root_url", dist_root_url)
            .finish()
    }
}

fn get_default_host_triple(s: &Settings) -> dist::TargetTriple {
    s.default_host_triple
        .as_ref()
        .map(dist::TargetTriple::new)
        .unwrap_or_else(dist::TargetTriple::from_host_or_build)
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
    use rustup_macros::unit_test as test;

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
            result.unwrap_err().downcast::<OverrideFileConfigError>(),
            Ok(OverrideFileConfigError::Invalid)
        ));
    }

    #[test]
    fn parse_empty_toolchain_file() {
        let contents = "";

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert!(matches!(
            result.unwrap_err().downcast::<OverrideFileConfigError>(),
            Ok(OverrideFileConfigError::Empty)
        ));
    }

    #[test]
    fn parse_whitespace_toolchain_file() {
        let contents = "   ";

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert!(matches!(
            result.unwrap_err().downcast::<OverrideFileConfigError>(),
            Ok(OverrideFileConfigError::Empty)
        ));
    }

    #[test]
    fn parse_toml_syntax_error() {
        let contents = r#"[toolchain]
channel = nightly
"#;

        let result = Cfg::parse_override_file(contents, ParseMode::Both);
        assert!(matches!(
            result.unwrap_err().downcast::<OverrideFileConfigError>(),
            Ok(OverrideFileConfigError::Parsing)
        ));
    }
}
