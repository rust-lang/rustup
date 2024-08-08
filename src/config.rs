use std::fmt::{self, Debug, Display};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::{env, io};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use thiserror::Error as ThisError;
use tokio_stream::StreamExt;
use tracing::trace;

use crate::{
    cli::{common, self_update::SelfUpdateMode},
    dist::{
        self, download::DownloadCfg, temp, PartialToolchainDesc, Profile, TargetTriple,
        ToolchainDesc,
    },
    errors::RustupError,
    fallback_settings::FallbackSettings,
    install::UpdateStatus,
    notifications::*,
    process::Process,
    settings::{MetadataVersion, Settings, SettingsFile},
    toolchain::{
        CustomToolchainName, DistributableToolchain, LocalToolchainName, PathBasedToolchainName,
        ResolvableLocalToolchainName, ResolvableToolchainName, Toolchain, ToolchainName,
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
            Self::Environment => write!(f, "overridden by environment variable RUSTUP_TOOLCHAIN"),
            Self::CommandLine => write!(f, "overridden by +toolchain on the command line"),
            Self::OverrideDB(path) => write!(f, "directory override for '{}'", path.display()),
            Self::ToolchainFile(path) => write!(f, "overridden by '{}'", path.display()),
        }
    }
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
        profile: Option<Profile>,
    },
}

impl OverrideCfg {
    fn from_file(cfg: &Cfg<'_>, file: OverrideFile) -> Result<Self> {
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
                .ok_or_else(|| no_toolchain_error(cfg.process))?,
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
                        .map(Profile::from_str)
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

#[cfg(unix)]
pub(crate) const UNIX_FALLBACK_SETTINGS: &str = "/etc/rustup/settings.toml";

pub(crate) struct Cfg<'a> {
    profile_override: Option<Profile>,
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
    pub current_dir: PathBuf,
    pub process: &'a Process,
}

impl<'a> Cfg<'a> {
    pub(crate) fn from_env(
        current_dir: PathBuf,
        notify_handler: Arc<dyn Fn(Notification<'_>)>,
        process: &'a Process,
    ) -> Result<Self> {
        // Set up the rustup home directory
        let rustup_dir = process.rustup_home()?;

        utils::ensure_dir_exists("home", &rustup_dir, notify_handler.as_ref())?;

        let settings_file = SettingsFile::new(rustup_dir.join("settings.toml"));
        settings_file.with(|s| {
            (notify_handler)(Notification::ReadMetadataVersion(s.version));
            if s.version == MetadataVersion::default() {
                Ok(())
            } else {
                Err(anyhow!(
                    "rustup's metadata is out of date. run `rustup self upgrade-data`"
                ))
            }
        })?;

        // Centralised file for multi-user systems to provide admin/distributor set initial values.
        #[cfg(unix)]
        let fallback_settings = FallbackSettings::new(
            // If present, use the RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS environment
            // variable as settings path, or UNIX_FALLBACK_SETTINGS otherwise
            match process.var("RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS") {
                Ok(s) => PathBuf::from(s),
                Err(_) => PathBuf::from(UNIX_FALLBACK_SETTINGS),
            },
        )?;
        #[cfg(windows)]
        let fallback_settings = None;

        let toolchains_dir = rustup_dir.join("toolchains");
        let update_hash_dir = rustup_dir.join("update-hashes");
        let download_dir = rustup_dir.join("downloads");

        // Figure out get_default_host_triple before Config is populated
        let default_host_triple =
            settings_file.with(|s| Ok(get_default_host_triple(s, process)))?;
        // Environment override
        let env_override = non_empty_env_var("RUSTUP_TOOLCHAIN", process)?
            .map(ResolvableLocalToolchainName::try_from)
            .transpose()?
            .map(|t| t.resolve(&default_host_triple))
            .transpose()?;

        let dist_root_server = dist_root_server(process)?;

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
            current_dir,
            process,
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
    pub(crate) fn download_cfg(
        &'a self,
        notify_handler: &'a dyn Fn(crate::dist::Notification<'_>),
    ) -> DownloadCfg<'a> {
        DownloadCfg {
            dist_root: &self.dist_root_url,
            tmp_cx: &self.tmp_cx,
            download_dir: &self.download_dir,
            notify_handler,
            process: self.process,
        }
    }

    pub(crate) fn set_profile_override(&mut self, profile: Profile) {
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

    pub(crate) fn set_profile(&mut self, profile: Profile) -> Result<()> {
        self.profile_override = None;
        self.settings_file.with_mut(|s| {
            s.profile = Some(profile);
            Ok(())
        })?;
        (self.notify_handler)(Notification::SetProfile(profile.as_str()));
        Ok(())
    }

    pub(crate) fn set_auto_self_update(&mut self, mode: SelfUpdateMode) -> Result<()> {
        self.settings_file.with_mut(|s| {
            s.auto_self_update = Some(mode);
            Ok(())
        })?;
        (self.notify_handler)(Notification::SetSelfUpdate(mode.as_str()));
        Ok(())
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
    pub(crate) fn get_profile(&self) -> Result<Profile> {
        if let Some(p) = self.profile_override {
            return Ok(p);
        }
        self.settings_file
            .with(|s| Ok(s.profile.unwrap_or_default()))
    }

    pub(crate) fn get_self_update_mode(&self) -> Result<SelfUpdateMode> {
        if self.process.var("CI").is_ok() && self.process.var("RUSTUP_CI").is_err() {
            // If we're in CI (but not rustup's own CI, which wants to test this stuff!),
            // disable automatic self updates.
            return Ok(SelfUpdateMode::Disable);
        }

        self.settings_file.with(|s| {
            Ok(match s.auto_self_update {
                Some(mode) => mode,
                None => SelfUpdateMode::Enable,
            })
        })
    }

    pub(crate) fn ensure_toolchains_dir(&self) -> Result<(), anyhow::Error> {
        utils::ensure_dir_exists("toolchains", &self.toolchains_dir, &|n| {
            (self.notify_handler)(n)
        })?;
        Ok(())
    }

    pub(crate) fn installed_paths<'b>(
        &self,
        desc: &ToolchainDesc,
        path: &'b Path,
    ) -> anyhow::Result<Vec<InstalledPath<'b>>> {
        Ok(vec![
            InstalledPath::File {
                name: "update hash",
                path: self.get_hash_file(desc, false)?,
            },
            InstalledPath::Dir { path },
        ])
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

    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn upgrade_data(&self) -> Result<()> {
        let current_version = self.settings_file.with(|s| Ok(s.version))?;
        if current_version == MetadataVersion::default() {
            (self.notify_handler)(Notification::MetadataUpgradeNotNeeded(current_version));
            return Ok(());
        }

        (self.notify_handler)(Notification::UpgradingMetadata(
            current_version,
            MetadataVersion::default(),
        ));

        match current_version {
            MetadataVersion::V2 => {
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
                    s.version = MetadataVersion::default();
                    Ok(())
                })
            }
            MetadataVersion::V12 => unreachable!(),
        }
    }

    pub(crate) fn find_default(&self) -> Result<Option<Toolchain<'_>>> {
        Ok(self
            .get_default()?
            .map(|n| Toolchain::new(self, (&n).into()))
            .transpose()?)
    }

    pub(crate) fn toolchain_from_partial(
        &self,
        toolchain: Option<PartialToolchainDesc>,
    ) -> anyhow::Result<Toolchain<'_>> {
        let toolchain = match toolchain {
            Some(toolchain) => {
                let desc = toolchain.resolve(&self.get_default_host_triple()?)?;
                Some(LocalToolchainName::Named(ToolchainName::Official(desc)))
            }
            None => None,
        };
        self.local_toolchain(toolchain)
    }

    pub(crate) fn find_active_toolchain(
        &self,
    ) -> Result<Option<(LocalToolchainName, ActiveReason)>> {
        Ok(
            if let Some((override_config, reason)) = self.find_override_config()? {
                Some((override_config.into_local_toolchain_name(), reason))
            } else {
                self.get_default()?
                    .map(|x| (x.into(), ActiveReason::Default))
            },
        )
    }

    fn find_override_config(&self) -> Result<Option<(OverrideCfg, ActiveReason)>> {
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
                    self.find_override_from_dir_walk(&self.current_dir, s)
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
                    .resolve(&get_default_host_triple(settings, self.process))?;
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
                    let default_host_triple = get_default_host_triple(settings, self.process);
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

    #[tracing::instrument(level = "trace")]
    pub(crate) fn active_rustc_version(&mut self) -> Result<Option<String>> {
        if let Some(t) = self.process.args().find(|x| x.starts_with('+')) {
            trace!("Fetching rustc version from toolchain `{}`", t);
            self.set_toolchain_override(&ResolvableToolchainName::try_from(&t[1..])?);
        }

        let Some((name, _)) = self.find_active_toolchain()? else {
            return Ok(None);
        };
        Ok(Some(Toolchain::new(self, name)?.rustc_version()))
    }

    pub(crate) fn resolve_toolchain(
        &self,
        name: Option<ResolvableToolchainName>,
    ) -> Result<Toolchain<'_>> {
        let toolchain = match name {
            Some(name) => {
                let desc = name.resolve(&self.get_default_host_triple()?)?;
                Some(desc.into())
            }
            None => None,
        };
        self.local_toolchain(toolchain)
    }

    pub(crate) fn resolve_local_toolchain(
        &self,
        name: Option<ResolvableLocalToolchainName>,
    ) -> Result<Toolchain<'_>> {
        let local = name
            .map(|name| name.resolve(&self.get_default_host_triple()?))
            .transpose()?;
        self.local_toolchain(local)
    }

    fn local_toolchain(&self, name: Option<LocalToolchainName>) -> Result<Toolchain<'_>> {
        let toolchain = match name {
            Some(tc) => tc,
            None => {
                self.find_active_toolchain()?
                    .ok_or_else(|| no_toolchain_error(self.process))?
                    .0
            }
        };
        Ok(Toolchain::new(self, toolchain)?)
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) async fn find_or_install_active_toolchain(
        &'a self,
        verbose: bool,
    ) -> Result<(Toolchain<'a>, ActiveReason)> {
        match self.find_override_config()? {
            Some((override_config, reason)) => match override_config {
                OverrideCfg::PathBased(path_based_name) => {
                    let toolchain = Toolchain::with_reason(self, path_based_name.into(), &reason)?;
                    Ok((toolchain, reason))
                }
                OverrideCfg::Custom(custom_name) => {
                    let toolchain = Toolchain::with_reason(self, custom_name.into(), &reason)?;
                    Ok((toolchain, reason))
                }
                OverrideCfg::Official {
                    toolchain,
                    components,
                    targets,
                    profile,
                } => {
                    let toolchain = self
                        .ensure_installed(&toolchain, components, targets, profile, verbose)
                        .await?
                        .1;
                    Ok((toolchain, reason))
                }
            },
            None => match self.get_default()? {
                None => Err(no_toolchain_error(self.process)),
                Some(ToolchainName::Custom(custom_name)) => {
                    let reason = ActiveReason::Default;
                    let toolchain = Toolchain::with_reason(self, custom_name.into(), &reason)?;
                    Ok((toolchain, reason))
                }
                Some(ToolchainName::Official(toolchain_desc)) => {
                    let reason = ActiveReason::Default;
                    let toolchain = self
                        .ensure_installed(&toolchain_desc, vec![], vec![], None, verbose)
                        .await?
                        .1;
                    Ok((toolchain, reason))
                }
            },
        }
    }

    // Returns a Toolchain matching the given ToolchainDesc, installing it and
    // the given components and targets if they aren't already installed.
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn ensure_installed(
        &self,
        toolchain: &ToolchainDesc,
        components: Vec<String>,
        targets: Vec<String>,
        profile: Option<Profile>,
        verbose: bool,
    ) -> Result<(UpdateStatus, Toolchain<'_>)> {
        common::warn_if_host_is_incompatible(
            toolchain,
            &TargetTriple::from_host_or_build(self.process),
            &toolchain.target,
            false,
        )?;
        if verbose {
            (self.notify_handler)(Notification::LookingForToolchain(toolchain));
        }
        let components: Vec<_> = components.iter().map(AsRef::as_ref).collect();
        let targets: Vec<_> = targets.iter().map(AsRef::as_ref).collect();
        let profile = match profile {
            Some(profile) => profile,
            None => self.get_profile()?,
        };
        let (status, toolchain) = match DistributableToolchain::new(self, toolchain.clone()) {
            Err(RustupError::ToolchainNotInstalled(_)) => {
                DistributableToolchain::install(
                    self,
                    toolchain,
                    &components,
                    &targets,
                    profile,
                    false,
                )
                .await?
            }
            Ok(mut distributable) => {
                if verbose {
                    (self.notify_handler)(Notification::UsingExistingToolchain(toolchain));
                }
                let status = if !distributable.components_exist(&components, &targets)? {
                    distributable.update(&components, &targets, profile).await?
                } else {
                    UpdateStatus::Unchanged
                };
                (status, distributable)
            }
            Err(e) => return Err(e.into()),
        };
        Ok((status, toolchain.into()))
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
    #[tracing::instrument(level = "trace", skip_all)]
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

            toolchains.sort();

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

    pub(crate) fn set_default_host_triple(&self, host_triple: String) -> Result<()> {
        // Ensure that the provided host_triple is capable of resolving
        // against the 'stable' toolchain.  This provides early errors
        // if the supplied triple is insufficient / bad.
        dist::PartialToolchainDesc::from_str("stable")?
            .resolve(&TargetTriple::new(host_triple.clone()))?;
        self.settings_file.with_mut(|s| {
            s.default_host_triple = Some(host_triple);
            Ok(())
        })
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn get_default_host_triple(&self) -> Result<TargetTriple> {
        self.settings_file
            .with(|s| Ok(get_default_host_triple(s, self.process)))
    }

    /// The path on disk of any concrete toolchain
    pub(crate) fn toolchain_path(&self, toolchain: &LocalToolchainName) -> PathBuf {
        match toolchain {
            LocalToolchainName::Named(name) => self.toolchains_dir.join(name.to_string()),
            LocalToolchainName::Path(p) => p.to_path_buf(),
        }
    }
}

pub(crate) fn dist_root_server(process: &Process) -> Result<String> {
    Ok(match non_empty_env_var("RUSTUP_DIST_SERVER", process)? {
        Some(s) => {
            trace!("`RUSTUP_DIST_SERVER` has been set to `{s}`");
            s
        }
        None => {
            // For backward compatibility
            non_empty_env_var("RUSTUP_DIST_ROOT", process)?
                .inspect(|url| trace!("`RUSTUP_DIST_ROOT` has been set to `{url}`"))
                .as_ref()
                .map(|root| root.trim_end_matches("/dist"))
                .unwrap_or(dist::DEFAULT_DIST_SERVER)
                .to_owned()
        }
    })
}

impl<'a> Debug for Cfg<'a> {
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
            current_dir,
            process: _,
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
            .field("current_dir", current_dir)
            .finish()
    }
}

fn get_default_host_triple(s: &Settings, process: &Process) -> TargetTriple {
    s.default_host_triple
        .as_ref()
        .map(TargetTriple::new)
        .unwrap_or_else(|| TargetTriple::from_host_or_build(process))
}

fn non_empty_env_var(name: &str, process: &Process) -> anyhow::Result<Option<String>> {
    match process.var(name) {
        Ok(s) if !s.is_empty() => Ok(Some(s)),
        Ok(_) => Ok(None),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn no_toolchain_error(process: &Process) -> anyhow::Error {
    RustupError::ToolchainNotSelected(process.name().unwrap_or_else(|| "Rust".into())).into()
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

/// Installed paths
pub(crate) enum InstalledPath<'a> {
    File { name: &'static str, path: PathBuf },
    Dir { path: &'a Path },
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
