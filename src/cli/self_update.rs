//! Self-installation and updating
//!
//! This is the installer at the heart of Rust. If it breaks
//! everything breaks. It is conceptually very simple, as rustup is
//! distributed as a single binary, and installation mostly requires
//! copying it into place. There are some tricky bits though, mostly
//! because of workarounds to self-delete an exe on Windows.
//!
//! During install (as `rustup-init`):
//!
//! * copy the self exe to the Rustup bin home
//! * hardlink rustc, etc. to *that*
//! * update the PATH in a system-specific way
//! * run the equivalent of `rustup default stable`
//!
//! During upgrade (`rustup self update`):
//!
//! * download rustup-init to `self-update` under the Rustup state home
//! * run the downloaded binary in replacement mode
//! * atomically replace rustup and update its proxy links. On Windows
//!   this happens after the update command exits.
//!
//! During uninstall (`rustup self uninstall`):
//!
//! * Clean the resolved Rustup homes.
//! * Remove Rustup and its proxies from the current and legacy bin homes.
//! * In non-category mode, also clean Cargo home, preserving other bin programs.
//!
//! Deleting the running binary during uninstall is tricky
//! and racy on Windows.

use std::{
    borrow::Cow,
    env::{self, consts::EXE_SUFFIX},
    fmt, fs,
    io::{self, Write},
    path::{MAIN_SEPARATOR, Path, PathBuf},
    process::Command,
    str::FromStr,
};

use anstyle::Style;
use anyhow::{Context, anyhow};
use clap::{ValueEnum, builder::PossibleValue};
use clap_cargo::style::{GOOD, WARN};
use itertools::Itertools;
use same_file::{Handle, is_same_file};
use serde::{Deserialize, Serialize};
use tracing::{error, info, trace, warn};

use crate::{
    DUP_TOOLS, TOOLS,
    cli::{
        common::{self, Confirm, PackageUpdate, ignorable_error, report_error},
        errors::CliError,
        markdown::md,
    },
    config::{Cfg, default_host_tuple},
    dist::{
        ChannelToolchainName, DistOptions, PartialChannelToolchainName, Profile, TargetTuple,
        download::DownloadCfg,
    },
    download::{DownloadError, DownloadOptions},
    errors::RustupError,
    install::{InstallMethod, UpdateStatus},
    process::Process,
    settings::SettingsFile,
    toolchain::{ChannelToolchain, PartialToolchainName, Toolchain, ToolchainNameExt as _},
    utils::{self, ExitCode},
};

#[macro_use]
mod msg;

mod stage;
#[cfg(feature = "test")]
pub use stage::{Marker, SELF_UPDATE_DIRECTORY, updater_path};
use stage::{PreparedUpdater, SelfUpdateLock};

#[cfg(unix)]
mod shell;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::self_replace;
#[cfg(unix)]
use unix::{add_to_path, run_update};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::complete_windows_uninstall;
#[cfg(windows)]
pub(crate) use windows::self_replace;
#[cfg(all(windows, feature = "test"))]
pub use windows::{RUSTUP_REGISTRY_TEST_ID, RegistryValueId, USER_PATH, get_path};
#[cfg(windows)]
use windows::{add_to_path, add_uninstall_registry_entry, run_update};

pub(super) struct InstallOpts<'a> {
    pub default_host_tuple: Option<String>,
    pub default_toolchain: Option<DefaultChannelToolchain>,
    pub profile: Profile,
    pub no_modify_path: bool,
    pub no_update_toolchain: bool,
    pub components: &'a [&'a str],
    pub targets: &'a [&'a str],
}

impl InstallOpts<'_> {
    /// Installs rustup to the system according to the current options.
    ///
    /// The installation process is composed of the following steps:
    /// - Copying the running binary to the binary directory.
    /// - Hard-linking the various Rust tools to that copied binary.
    /// - Adding the binary directory to the `$PATH` unless `no_modify_path` is set.
    pub(crate) async fn install(
        mut self,
        current_dir: PathBuf,
        no_prompt: bool,
        quiet: bool,
        process: &Process,
    ) -> anyhow::Result<ExitCode> {
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut exit_code = ExitCode::SUCCESS;

        self.validate(process).map_err(|e| {
            anyhow!(
                "Pre-checks for host and toolchain failed: {e:#}\n\
            If you are unsure of suitable values, the 'stable' toolchain is the default.\n\
            Valid host tuples look something like: {}",
                TargetTuple::from_host_or_build(process)
            )
        })?;

        if process
            .var_os("RUSTUP_INIT_SKIP_EXISTENCE_CHECKS")
            .is_none_or(|s| s != "yes")
        {
            check_existence_of_rustc_or_cargo_in_path(no_prompt, process)?;
            check_existence_of_settings_file(process)?;
        }

        #[cfg(unix)]
        {
            exit_code &= unix::anti_sudo_check(no_prompt, process)?;
        }

        let mut term = process.stdout();

        #[cfg(windows)]
        windows::maybe_install_msvc(&mut term, no_prompt, quiet, &self, process).await?;

        if !no_prompt {
            let msg = pre_install_msg(self.no_modify_path, process)?;

            md(&mut term, msg);
            let mut customized_install = false;
            loop {
                md(&mut term, self.display(process));
                match common::confirm_advanced(customized_install, process)? {
                    Confirm::No => {
                        info!("aborting installation");
                        return Ok(ExitCode::SUCCESS);
                    }
                    Confirm::Yes => break,
                    Confirm::Advanced => {
                        customized_install = true;
                        self.customize(process)?;
                    }
                }
            }
        }

        let no_modify_path = self.no_modify_path;
        if let Err(e) = self.install_rust(current_dir, quiet, process).await {
            report_error(&e, process);

            // On windows, where installation happens in a console
            // that may have opened just for this purpose, give
            // the user an opportunity to see the error before the
            // window closes.
            #[cfg(windows)]
            if !no_prompt {
                windows::ensure_prompt(process)?;
            }

            return Ok(ExitCode::FAILURE);
        }

        let bin_home = process.bin_home()?;
        let home_dir = process.home_dir();
        let msg = if no_modify_path {
            format!(
                post_install_msg_no_modify_path!(),
                cargo_bin_dir = HomeDisplay::new(&bin_home, home_dir.as_deref()),
            )
        } else {
            format!(
                post_install_msg!(),
                cargo_bin_dir = HomeDisplay::new(&bin_home, home_dir.as_deref()),
            )
        };
        md(&mut term, msg);
        #[cfg(not(windows))]
        {
            let env_home = process.env_home()?;
            md(
                &mut term,
                format!(
                    post_install_msg_unix!(),
                    env_dir = HomeDisplay::new(&env_home, home_dir.as_deref()),
                    source_env_lines =
                        shell::build_source_env_lines(process, &env_home, home_dir.as_deref()),
                ),
            );
        }

        #[cfg(unix)]
        warn_if_default_linker_missing(process);

        #[cfg(windows)]
        if !no_prompt {
            // On windows, where installation happens in a console
            // that may have opened just for this purpose, require
            // the user to press a key to continue.
            windows::ensure_prompt(process)?;
        }

        Ok(exit_code)
    }

    /// Installs the rustup binary and proxies, and installs a toolchain if specified.
    async fn install_rust(
        self,
        current_dir: PathBuf,
        quiet: bool,
        process: &Process,
    ) -> anyhow::Result<()> {
        let bin_home = process.bin_home()?;
        install_bins(process, &bin_home, force_hard_links(process))?;

        #[cfg(unix)]
        unix::write_env_files(process)?;

        if !self.no_modify_path {
            add_to_path(process)?;
        }

        #[cfg(windows)]
        add_uninstall_registry_entry(process)?;

        let home_dirs = process.home_dirs()?;
        utils::ensure_dir_exists("cache home", &home_dirs.cache)?;
        utils::ensure_dir_exists("data home", &home_dirs.data)?;

        let mut cfg = Cfg::from_env(current_dir, quiet, false, process)?;

        let (components, targets) = (self.components, self.targets);
        let toolchain = self.select_toolchain(&mut cfg)?;
        if let Some(partial_desc) = toolchain {
            let desc = partial_desc.clone().complete(&cfg.default_host_tuple()?)?;
            let options =
                DistOptions::new(components, targets, &desc, cfg.get_profile()?, true, &cfg)?;
            let status = if desc.exists(&cfg)? {
                warn!("Updating existing toolchain, profile choice will be ignored");
                // If we have a partial install we might not be able to read content here. We could:
                // - fail and folk have to delete the partially present toolchain to recover
                // - silently ignore it (and provide inconsistent metadata for reporting the install/update change)
                // - delete the partial install and start over
                // For now, we error.
                let toolchain = ChannelToolchain::new(&cfg, desc.clone())?;
                InstallMethod::Dist(options.for_update(&toolchain, false))
                    .install(None)
                    .await?
            } else {
                ChannelToolchain::install(options).await?.status
            };

            check_proxy_sanity(&bin_home, components, &desc)?;

            cfg.set_default(Some(&partial_desc.into()))?;
            writeln!(cfg.process.stdout().lock())?;
            common::show_channel_update(&cfg, PackageUpdate::Toolchain(desc), Ok(status)).await?;
        }
        Ok(())
    }

    /// Selects the toolchain to install based on the user's intent.
    ///
    /// This function first initializes the default profile and default host tuple in the
    /// configuration, then returns the toolchain that should be installed, or `None` if none is
    /// specified by the user.
    fn select_toolchain(
        self,
        cfg: &mut Cfg<'_>,
    ) -> anyhow::Result<Option<PartialChannelToolchainName>> {
        let Self {
            default_host_tuple,
            default_toolchain,
            profile,
            no_modify_path: _no_modify_path,
            no_update_toolchain,
            components,
            targets,
        } = self;

        cfg.set_profile(profile)?;

        if let Some(default_host_tuple) = &default_host_tuple {
            // Set host tuple now as it will affect resolution of toolchain_str
            info!("setting default host tuple to {}", default_host_tuple);
            cfg.set_default_host_tuple(default_host_tuple.to_owned())?;
        } else {
            info!("default host tuple is {}", cfg.default_host_tuple()?);
        }

        let user_specified_something = default_toolchain.is_some()
            || !targets.is_empty()
            || !components.is_empty()
            || !no_update_toolchain;

        // If the user specified they want no toolchain, we skip this, otherwise
        // if they specify something directly, or we have no default, then we install
        // a toolchain (updating if it's already present) and then if neither of
        // those are true, we have a user who doesn't mind, and already has an
        // install, so we leave their setup alone.
        if matches!(default_toolchain, Some(DefaultChannelToolchain(None))) {
            info!("skipping toolchain installation");
            if !components.is_empty() {
                warn!(
                    "ignoring requested component{}: {}",
                    if components.len() == 1 { "" } else { "s" },
                    components.join(", ")
                );
            }
            if !targets.is_empty() {
                warn!(
                    "ignoring requested target{}: {}",
                    if targets.len() == 1 { "" } else { "s" },
                    targets.join(", ")
                );
            }
            writeln!(cfg.process.stdout().lock())?;
            Ok(None)
        } else if user_specified_something
            || (!no_update_toolchain && cfg.find_default()?.is_none())
        {
            Ok(match default_toolchain {
                Some(s) => Some(s.0.unwrap()),
                None => match cfg.get_default_resolvable()? {
                    // Default is installable
                    Some(PartialToolchainName::Channel(t)) => Some(t),
                    // Default is custom, presumably from a prior install. Do nothing.
                    Some(PartialToolchainName::Custom(_)) => None,
                    None => Some(PartialChannelToolchainName::from_str("stable")?),
                },
            })
        } else {
            info!("updating existing rustup installation - leaving toolchains alone");
            writeln!(cfg.process.stdout().lock())?;
            Ok(None)
        }
    }

    // Interactive editing of the install options
    fn customize(&mut self, process: &Process) -> anyhow::Result<()> {
        writeln!(
            process.stdout().lock(),
            "I'm going to ask you the value of each of these installation options.\n\
         You may simply press the Enter key to leave unchanged."
        )?;

        writeln!(process.stdout().lock())?;

        self.default_host_tuple = Some(common::question_str(
            "Default host tuple?",
            &self
                .default_host_tuple
                .take()
                .unwrap_or_else(|| TargetTuple::from_host_or_build(process).to_string()),
            process,
        )?);

        self.default_toolchain = Some(DefaultChannelToolchain::from_str(&common::question_str(
            "Default toolchain? (stable/beta/nightly/none)",
            &match &self.default_toolchain {
                Some(DefaultChannelToolchain(Some(t))) => Cow::Owned(t.to_string()),
                Some(DefaultChannelToolchain(None)) => Cow::Borrowed("none"),
                None => Cow::Borrowed("stable"),
            },
            process,
        )?)?);

        self.profile = <Profile as FromStr>::from_str(&common::question_str(
            &format!(
                "Profile (which tools and data to install)? ({})",
                Profile::value_variants().iter().join("/"),
            ),
            self.profile.as_str(),
            process,
        )?)?;

        self.no_modify_path =
            !common::question_bool("Modify PATH variable?", !self.no_modify_path, process)?;

        Ok(())
    }

    fn validate(&self, process: &Process) -> anyhow::Result<()> {
        common::warn_if_host_is_emulated(process);

        let host_tuple = self
            .default_host_tuple
            .as_ref()
            .map(TargetTuple::new)
            .unwrap_or_else(|| TargetTuple::from_host_or_build(process));
        let partial_channel = match self.default_toolchain.as_ref().and_then(|t| t.0.as_ref()) {
            None => PartialToolchainName::from_str("stable")?,
            Some(s) => s.into(),
        };
        let resolved = partial_channel.complete(&host_tuple)?;
        trace!("Successfully resolved installation toolchain as: {resolved}");
        Ok(())
    }

    fn display(&self, process: &Process) -> String {
        format!(
            r"Current installation options:

- `  `default host tuple: `{}`
- `   `default toolchain: `{}`
- `             `profile: `{}`
- modify PATH variable: `{}`
",
            self.default_host_tuple.as_ref().map_or_else(
                || TargetTuple::from_host_or_build(process),
                TargetTuple::new,
            ),
            match &self.default_toolchain {
                Some(DefaultChannelToolchain(Some(name))) => Cow::Owned(name.to_string()),
                Some(DefaultChannelToolchain(None)) => Cow::Borrowed("none"),
                None => Cow::Borrowed("stable (default)"),
            },
            self.profile,
            if !self.no_modify_path { "yes" } else { "no" }
        )
    }
}

/// User input to be set as the default channel toolchain.
///
/// A value of "none" will be parsed into `Self(None)` which means that the user has requested to
/// unset the default toolchain.
#[derive(Debug, Clone)]
pub(super) struct DefaultChannelToolchain(Option<PartialChannelToolchainName>);

impl FromStr for DefaultChannelToolchain {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(match value.trim_end_matches('/') {
            "none" => None,
            _ => Some(PartialChannelToolchainName::from_str(value)?),
        }))
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelfUpdateMode {
    #[default]
    Enable,
    Disable,
    CheckOnly,
}

impl SelfUpdateMode {
    pub(crate) fn from_cfg(cfg: &Cfg<'_>) -> anyhow::Result<Self> {
        if cfg.process.is_ci() {
            return Ok(Self::Disable);
        }

        cfg.settings_file.with(|s| {
            Ok(match s.auto_self_update {
                Some(mode) => mode,
                None => Self::Enable,
            })
        })
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Enable => "enable",
            Self::Disable => "disable",
            Self::CheckOnly => "check-only",
        }
    }

    /// Optionally performs a self-update: check policy, download, apply and exit.
    ///
    /// Whether the self-update is executed is based on both compile-time and runtime
    /// configurations, where the priority is as follows:
    /// no-self-update feature > self update mode > CLI flag
    ///
    /// i.e. update only if rustup does **not** have the no-self-update feature,
    /// and self update mode is configured to **enable**
    /// and has **no** `--no-self-update` CLI flag.
    pub(crate) async fn update(
        &self,
        should_self_update: bool,
        dl_cfg: &DownloadCfg<'_>,
    ) -> anyhow::Result<ExitCode> {
        if cfg!(feature = "no-self-update") {
            info!("self-update is disabled for this build of rustup");
            info!("any updates to rustup will need to be fetched with your system package manager");
            return Ok(ExitCode::SUCCESS);
        }
        match self {
            Self::Enable if should_self_update => (),
            Self::CheckOnly => {
                check_rustup_update(dl_cfg).await?;
                return Ok(ExitCode::SUCCESS);
            }
            _ => return Ok(ExitCode::SUCCESS),
        }

        match self_update_permitted(false)? {
            SelfUpdatePermission::HardFail => {
                error!("Unable to self-update.  STOP");
                return Ok(ExitCode::FAILURE);
            }
            #[cfg(not(windows))]
            SelfUpdatePermission::Skip => return Ok(ExitCode::SUCCESS),
            SelfUpdatePermission::Permit => {}
        }

        let prepared_updater = prepare_update(dl_cfg).await?;

        if let Some(prepared_updater) = prepared_updater {
            return run_update(prepared_updater, dl_cfg.process);
        } else {
            // Try again in case we emitted "tool `{}` is already installed" last time.
            install_proxies(dl_cfg.process)?;
        }

        Ok(ExitCode::SUCCESS)
    }
}

impl ValueEnum for SelfUpdateMode {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Enable, Self::Disable, Self::CheckOnly]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.as_str()))
    }

    fn from_str(input: &str, _: bool) -> Result<Self, String> {
        <Self as FromStr>::from_str(input).map_err(|e| e.to_string())
    }
}

impl FromStr for SelfUpdateMode {
    type Err = anyhow::Error;

    fn from_str(mode: &str) -> anyhow::Result<Self> {
        match mode {
            "enable" => Ok(Self::Enable),
            "disable" => Ok(Self::Disable),
            "check-only" => Ok(Self::CheckOnly),
            _ => Err(anyhow!(format!(
                "unknown self update mode: '{}'; valid modes are {}",
                mode,
                Self::value_variants().iter().join(", ")
            ))),
        }
    }
}

impl fmt::Display for SelfUpdateMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn update_root(process: &Process) -> String {
    process
        .var("RUSTUP_UPDATE_ROOT")
        .inspect(|url| trace!("`RUSTUP_UPDATE_ROOT` has been set to `{url}`"))
        .unwrap_or_else(|_| String::from(DEFAULT_UPDATE_ROOT))
}

/// The platform-specific abbreviation of the user's home directory in displayed paths.
pub(crate) const HOME_PREFIX: &str = cfg_select! {
    windows => "%USERPROFILE%",
    _ => "$HOME",
};

/// Displays an installation path with a platform-specific home abbreviation.
struct HomeDisplay<'a> {
    path: &'a Path,
    home_prefix: Option<&'a str>,
}

impl<'a> HomeDisplay<'a> {
    fn new(path: &'a Path, home_dir: Option<&Path>) -> Self {
        match home_dir.and_then(|home| path.strip_prefix(home).ok()) {
            Some(relative) => Self {
                path: relative,
                home_prefix: Some(HOME_PREFIX),
            },
            None => Self {
                path,
                home_prefix: None,
            },
        }
    }
}

impl fmt::Display for HomeDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(home_prefix) = self.home_prefix {
            f.write_str(home_prefix)?;
            if !self.path.is_empty() {
                write!(f, "{MAIN_SEPARATOR}")?;
            }
        }
        self.path.display().fmt(f)
    }
}

fn rustc_or_cargo_exists_in_path(process: &Process) -> anyhow::Result<()> {
    let Some(paths) = process.var_os("PATH") else {
        return Ok(());
    };
    let bin_home = process.bin_home()?;

    let conflict = env::split_paths(&paths).find(|path| {
        path != &bin_home
            && (path.join(format!("rustc{EXE_SUFFIX}")).exists()
                || path.join(format!("cargo{EXE_SUFFIX}")).exists())
    });

    if let Some(path) = conflict {
        anyhow::bail!("{}", path.display());
    }

    Ok(())
}

fn check_existence_of_rustc_or_cargo_in_path(
    no_prompt: bool,
    process: &Process,
) -> anyhow::Result<()> {
    // Only the test runner should set this
    let skip_check = process.var_os("RUSTUP_INIT_SKIP_PATH_CHECK");

    // Skip this if the environment variable is set
    if skip_check == Some("yes".into()) {
        return Ok(());
    }

    if let Err(path) = rustc_or_cargo_exists_in_path(process) {
        warn!("It looks like you have an existing installation of Rust at:");
        warn!("{}", path);
        warn!("This could mean one of the following:");
        warn!("- You have already installed rustup: you can quit the installer right now.");
        warn!("- You haven't installed rustup yet, but have Rust installed from elsewhere.");
        warn!("In the latter case, rustup can coexist with and adopt your existing Rust.");
        warn!("To adopt, please reply `yes`, or set RUSTUP_INIT_SKIP_PATH_CHECK to yes,");
        warn!("or pass `-y` to ignore all ignorable checks, then follow the instructions at:");
        warn!("<https://rust-lang.github.io/rustup/installation/already-installed-rust.html>");
        ignorable_error("cannot install while Rust is installed", no_prompt, process)?;
    }
    Ok(())
}

fn check_existence_of_settings_file(process: &Process) -> anyhow::Result<()> {
    // TODO: Setting file's category is still under discussion
    // See: [discussion issue](https://github.com/rust-lang/rustup/issues/4944)
    let settings_file = SettingsFile::new(process.home_dirs()?.config.join("settings.toml"));
    if !utils::path_exists(&settings_file.path) {
        return Ok(());
    }
    let settings_toolchain = settings_file.with(|s| Ok(s.default_toolchain.clone()))?;
    // If there is already a non-empty `settings.toml` file (e.g., not a fresh install),
    // then we warn the user that there was an already configured default toolchain.
    let Some(default_toolchain) = settings_toolchain else {
        return Ok(());
    };
    warn!("it looks like you have an existing rustup settings file at:");
    warn!("{}", settings_file.path.display());
    let default_host_tuple = settings_file.with(|s| Ok(default_host_tuple(s, process)))?;
    let inferred =
        PartialChannelToolchainName::from_str("stable")?.complete(&default_host_tuple)?;
    if default_toolchain != inferred.to_string() {
        warn!("rustup will install the default toolchain as specified in the settings file,");
        warn!("instead of the one inferred from the default host tuple.");
    }
    Ok(())
}

fn pre_install_msg(no_modify_path: bool, process: &Process) -> anyhow::Result<String> {
    let cargo_home = process.cargo_home()?;
    let cargo_bin_dir = process.bin_home()?;
    let home_dirs = process.home_dirs()?;
    let home_dir = process.home_dir();
    let rustup_home_message = if process.use_category_home() {
        format!(
            "Rustup will use these directories:

      config: {}
      state:  {}
      data:   {}
      cache:  {}

They can be modified individually with
`RUSTUP_CONFIG_HOME`, `RUSTUP_STATE_HOME`, `RUSTUP_DATA_HOME`, and
`RUSTUP_CACHE_HOME`.",
            HomeDisplay::new(&home_dirs.config, home_dir.as_deref()),
            HomeDisplay::new(&home_dirs.state, home_dir.as_deref()),
            HomeDisplay::new(&home_dirs.data, home_dir.as_deref()),
            HomeDisplay::new(&home_dirs.cache, home_dir.as_deref()),
        )
    } else {
        format!(
            "Rustup metadata and toolchains will be installed into the Rustup
home directory, located at:

    {}

This can be modified with the `RUSTUP_HOME` environment variable.",
            HomeDisplay::new(&home_dirs.data, home_dir.as_deref()),
        )
    };

    if no_modify_path {
        return Ok(format!(
            pre_install_msg_no_modify_path!(),
            cargo_home = HomeDisplay::new(&cargo_home, home_dir.as_deref()),
            cargo_bin_dir = HomeDisplay::new(&cargo_bin_dir, home_dir.as_deref()),
            rustup_home_message = rustup_home_message,
        ));
    }

    // Brittle code warning: some duplication in unix::add_to_path
    #[cfg(not(windows))]
    {
        let rcfiles = shell::get_available_shells(process)
            .flat_map(|sh| sh.rcs(process).into_iter())
            .map(|rc| format!("    {}", HomeDisplay::new(&rc, home_dir.as_deref())))
            .collect::<Vec<_>>();
        let plural = if rcfiles.len() > 1 { "s" } else { "" };
        let rcfiles = rcfiles.join("\n");
        Ok(format!(
            pre_install_msg_unix!(),
            cargo_home = HomeDisplay::new(&cargo_home, home_dir.as_deref()),
            cargo_bin_dir = HomeDisplay::new(&cargo_bin_dir, home_dir.as_deref()),
            plural = plural,
            rcfiles = rcfiles,
            rustup_home_message = rustup_home_message,
        ))
    }
    #[cfg(windows)]
    Ok(format!(
        pre_install_msg_win!(),
        cargo_home = HomeDisplay::new(&cargo_home, home_dir.as_deref()),
        cargo_bin_dir = HomeDisplay::new(&cargo_bin_dir, home_dir.as_deref()),
        rustup_home_message = rustup_home_message,
    ))
}

#[cfg(unix)]
fn warn_if_default_linker_missing(process: &Process) {
    // Search for linker in PATH
    let Some(path) = process.var_os("PATH") else {
        warn!("unable to search PATH for a default linker");
        warn!("many Rust crates require a system C toolchain to build");
        return;
    };

    // If we have the host tuple, attempt to determine the CC/linker path that
    // `cc-rs` would use, as this is *usually* why we need a linker to invoke.
    //
    // Doing it this way allows us to correctly diagnose quirky systems like
    // solaris and illumos that use `gcc` rather than `cc` for historical reasons.
    let cc_tool = TargetTuple::from_host(process).and_then(|tuple| {
        // Fill in some dummy settings for `Build`/`Tool` to be able to properly
        // give us the metadata we want
        cc::Build::new()
            .cargo_metadata(false)
            .opt_level(0)
            .target(&tuple)
            .host(&tuple)
            .try_get_compiler()
            .ok()
    });

    let cc_binary = if let Some(cc_tool) = &cc_tool {
        Cow::Borrowed(cc_tool.path())
    } else {
        // If we don't get info from cc-rs, fall back to just looking for the literal `cc`.
        Cow::Owned(format!("cc{EXE_SUFFIX}").into())
    };

    // Search the path for the selected binary
    let found = env::split_paths(&path).any(|mut p| {
        p.push(&cc_binary);
        p.is_file()
    });

    if !found {
        let bin_disp = cc_binary.display();
        warn!("no default linker (`{bin_disp}`) was found in your PATH");
        warn!("many Rust crates require a system C toolchain to build");
    }
}

fn install_bins(process: &Process, bin_path: &Path, force_hard_links: bool) -> anyhow::Result<()> {
    SelfUpdateLock::lock(process)?.install_bins(bin_path, force_hard_links)
}

pub(crate) fn install_proxies(process: &Process) -> anyhow::Result<()> {
    let bin_path = process.bin_home()?;
    install_proxies_with_opts(&bin_path, force_hard_links(process))
}

fn force_hard_links(process: &Process) -> bool {
    // HACK: On Windows CI machines, some Docker setups don't like symlinks, so we force hard
    // links in this case.
    // See: <https://github.com/rust-lang/rustup/issues/4291>
    (cfg!(windows) && process.is_ci()) || process.var_os("RUSTUP_FORCE_HARDLINK_PROXIES").is_some()
}

fn install_proxies_with_opts(bin_path: &Path, force_hard_links: bool) -> anyhow::Result<()> {
    let rustup_path = bin_path.join(format!("rustup{EXE_SUFFIX}"));

    let rustup = Handle::from_path(&rustup_path)?;

    let mut tool_handles = Vec::new();
    let mut link_afterwards = Vec::new();

    // Try to symlink all the Rust exes to the rustup exe. Some systems,
    // like Windows, do not always support symlinks, so we fallback to hard links.
    //
    // Note that this function may not be running in the context of a fresh
    // self update but rather as part of a normal update to fill in missing
    // proxies. In that case our process may actually have the `rustup.exe`
    // file open, and on systems like Windows that means that you can't
    // even remove other hard links to the same file. Basically if we have
    // `rustup.exe` open and running and `cargo.exe` is a hard link to that
    // file, we can't remove `cargo.exe`.
    //
    // To avoid unnecessary errors from being returned here we use the
    // `same-file` crate and its `Handle` type to avoid clobbering hard links
    // that are already valid. If a hard link already points to the
    // `rustup.exe` file then we leave it alone and move to the next one.
    //
    // As yet one final caveat, when we're looking at handles for files we can't
    // actually delete files (they'll say they're deleted but they won't
    // actually be on Windows). As a result we manually drop all the
    // `tool_handles` later on. This'll allow us, afterwards, to actually
    // overwrite all the previous soft or hard links with new ones.
    for tool in TOOLS {
        let tool_path = bin_path.join(format!("{tool}{EXE_SUFFIX}"));
        if let Ok(handle) = Handle::from_path(&tool_path) {
            tool_handles.push(handle);
            if rustup == *tool_handles.last().unwrap() {
                continue;
            }
        }
        link_afterwards.push(tool_path);
    }

    // Normally we attempt to symlink files first but this can be overridden
    // by using an environment variable.
    let link_proxy = if force_hard_links {
        |src: &Path, dest: &Path| {
            let _ = fs::remove_file(dest);
            utils::hardlink_file(src, dest)
        }
    } else {
        utils::symlink_or_hardlink_file
    };

    for tool in DUP_TOOLS {
        let tool_path = bin_path.join(format!("{tool}{EXE_SUFFIX}"));
        if let Ok(handle) = Handle::from_path(&tool_path) {
            // Like above, don't clobber anything that's already linked to
            // avoid extraneous errors from being returned.
            if rustup == handle {
                continue;
            }

            // If this file exists and is *not* equivalent to all other
            // preexisting tools we found, then we're going to assume that it
            // was preinstalled and actually pointing to a totally different
            // binary. This is intended for cases where historically users
            // ran `cargo install rustfmt` and so they had custom `rustfmt`
            // and `cargo-fmt` executables lying around, but we as rustup have
            // since started managing these tools.
            //
            // If the file is managed by rustup it should be equivalent to some
            // previous file, and if it's not equivalent to anything then it's
            // pretty likely that it needs to be dealt with manually.
            if tool_handles.iter().all(|h| *h != handle) {
                warn!(
                    "tool `{}` is already installed, remove it from `{}`, then run `rustup update` \
                       to have rustup manage this tool.",
                    tool,
                    bin_path.display()
                );
                continue;
            }
        }
        link_proxy(&rustup_path, &tool_path)?;
    }

    drop(tool_handles);
    for path in link_afterwards {
        link_proxy(&rustup_path, &path)?;
    }

    if !force_hard_links {
        // Verify that the proxies are reachable.
        // This may fail for symlinks in some circumstances.
        let path = bin_path.join(format!("{tool}{EXE_SUFFIX}", tool = TOOLS[0]));
        if fs::File::open(path).is_err() {
            return install_proxies_with_opts(bin_path, true);
        }
    }

    Ok(())
}

fn check_proxy_sanity(
    bin_path: &Path,
    components: &[&str],
    desc: &ChannelToolchainName,
) -> anyhow::Result<()> {
    // Sometimes linking a proxy produces an unpredictable result, where the proxy
    // is in place, but manages to not call rustup correctly. One way to make sure we
    // don't run headfirst into the wall is to at least try and run our freshly
    // installed proxies, to see if they return some manner of reasonable output.
    // We limit ourselves to the most common two installed components (cargo and rustc),
    // because their binary names also happen to match up, which is not necessarily
    // a given.
    for component in components.iter().filter(|c| ["cargo", "rustc"].contains(c)) {
        let cmd = Command::new(bin_path.join(format!("{component}{EXE_SUFFIX}")))
            .args([&format!("+{desc}"), "--version"])
            .status();

        if !cmd.is_ok_and(|status| status.success()) {
            return Err(RustupError::BrokenProxy.into());
        }
    }

    Ok(())
}

/// Uninstall Rustup:
///
/// 1. Remove all installed toolchains, then delete the resolved Rustup homes
///    (config, state, data, cache, and Rustup home).
/// 2. Remove Rustup and its proxies from the resolved bin directory and from
///    `$CARGO_HOME/bin`. Leave other programs in place and remove each bin
///    directory only if it is empty.
/// 3. Unless `--no-modify-path` is set, remove PATH setup (shell setup on Unix
///    or PATH entries on Windows) for bin directories removed or absent.
/// 4. Outside category mode, delete the remaining entries in `$CARGO_HOME`
///    except `bin`, then remove `$CARGO_HOME` if it is empty.
///
/// On Windows, a helper performs steps 2–4 and removes the uninstall registry
/// entry after the running Rustup process exits.
pub(crate) fn uninstall(
    no_prompt: bool,
    no_modify_path: bool,
    cfg: &Cfg<'_>,
) -> anyhow::Result<ExitCode> {
    if cfg!(feature = "no-self-update") {
        error!("self-uninstall is disabled for this build of rustup");
        error!("you should probably use your system package manager to uninstall rustup");
        return Ok(ExitCode::FAILURE);
    }

    let process = cfg.process;
    let category_mode = process.use_category_home();
    let cargo_home = process.cargo_home()?;
    let rustup_bin_home = &cfg.bin_dir;
    let cargo_bin = cargo_home.join("bin");

    if !rustup_bin_home.join(format!("rustup{EXE_SUFFIX}")).exists() {
        return Err(CliError::NotSelfInstalled {
            p: rustup_bin_home.clone(),
        }
        .into());
    }

    if !no_prompt {
        writeln!(process.stdout().lock())?;
        let msg = pre_uninstall_msg(no_modify_path, category_mode, &cargo_home, &cargo_bin, cfg);
        md(&mut process.stdout(), msg);
        if !common::confirm("\nContinue? (y/N)", false, process)? {
            info!("aborting uninstallation");
            return Ok(ExitCode::SUCCESS);
        }
    }

    info!("removing toolchains");
    for toolchain in cfg.list_toolchains(true)? {
        Toolchain::ensure_removed(cfg, toolchain.into())?;
    }

    let homes = [
        ("config home", &cfg.config_dir),
        ("state home", &cfg.state_dir),
        ("data home", &cfg.data_dir),
        ("cache home", &cfg.cache_dir),
        ("rustup home", &cfg.rustup_dir),
    ];
    for &(name, home) in homes.iter() {
        let home = cfg.current_dir.join(home);
        info!("removing {name} `{}`", home.display());
        // Resolved homes can coincide, so an earlier removal may have already removed this one.
        if home.try_exists()? {
            utils::remove_dir(name, &home)?;
        } else {
            warn!("{name} might be already removed, or doesn't exist.")
        }
    }

    // Unix can unlink the running executable; Windows must finish this tail after exit.
    #[cfg(unix)]
    {
        // TODO(#5221): The env script is removed with its home, but its source line stays
        // in the rc files when PATH cleanup is skipped. See
        // `category_uninstall_doesnt_modify_rcs_with_no_modify_path` and
        // `category_uninstall_keeps_source_in_rcs_when_bin_home_is_non_empty` in
        // `tests/suite/cli_paths.rs`.
        clean_bin_home(rustup_bin_home)?;
        if !no_modify_path && !rustup_bin_home.try_exists()? {
            let env_home = process.env_home()?;
            unix::remove_from_path(&env_home, process)?;
        }

        clean_bin_home(&cargo_bin)?;
        if !no_modify_path && !cargo_bin.try_exists()? {
            // TODO: move this clean up into `remove_legacy_paths` once category mode is stable.
            unix::remove_from_path(&cargo_home, process)?;
            unix::remove_legacy_paths(process, &cargo_home, process.home_dir().as_deref())?;
        }

        if !category_mode {
            clean_cargo_home(&cargo_home)?;
        }
    }
    #[cfg(windows)]
    windows::spawn_uninstall_gc(no_modify_path)?;

    #[cfg(unix)]
    info!("rustup is uninstalled");
    #[cfg(windows)]
    info!("uninstallation started; final cleanup will run after this process exits");
    Ok(ExitCode::SUCCESS)
}

fn pre_uninstall_msg(
    no_modify_path: bool,
    category_mode: bool,
    cargo_home: &Path,
    cargo_bin: &Path,
    cfg: &Cfg<'_>,
) -> String {
    let home_dir = cfg.process.home_dir();
    let display_path = |path: &Path| {
        HomeDisplay::new(&cfg.current_dir.join(path), home_dir.as_deref()).to_string()
    };
    let rustup_home = display_path(&cfg.rustup_dir);
    let bin_home = display_path(&cfg.bin_dir);
    let bin_dirs = if cfg.current_dir.join(&cfg.bin_dir) == cargo_bin {
        format!("`{bin_home}`")
    } else {
        format!("`{bin_home}` and `{}`", display_path(cargo_bin))
    };
    let path_message = if no_modify_path {
        "Your `PATH` environment variable will not be touched."
    } else {
        "Rustup's shell PATH setup will be removed."
    };

    if category_mode {
        format!(
            pre_uninstall_msg_category_mode!(),
            config_home = display_path(&cfg.config_dir),
            state_home = display_path(&cfg.state_dir),
            data_home = display_path(&cfg.data_dir),
            cache_home = display_path(&cfg.cache_dir),
            rustup_home = rustup_home,
            bin_dirs = bin_dirs,
            path_message = path_message,
        )
    } else {
        format!(
            pre_uninstall_msg_legacy!(),
            rustup_home = rustup_home,
            cargo_home = display_path(cargo_home),
            bin_dirs = bin_dirs,
            path_message = path_message,
        )
    }
}

/// Clean the remaining Cargo home entries, preserving a nonempty bin directory.
fn clean_cargo_home(home: &Path) -> anyhow::Result<()> {
    let name = "cargo home";
    info!("cleaning {name} `{}`", home.display());
    let read_dir_error = |source| CliError::ReadDirError {
        p: home.to_owned(),
        source,
    };
    for entry in fs::read_dir(home).map_err(read_dir_error)? {
        let entry = entry.map_err(read_dir_error)?;
        if entry.file_name() == "bin" {
            continue;
        }
        let path = entry.path();
        // Use the directory remover for directory symlinks too; it unlinks them.
        if path.is_dir() {
            utils::remove_dir("home entry", &path)?;
        } else {
            utils::remove_file("home entry", &path)?;
        }
    }

    let home_display = home.display();
    match fs::remove_dir(home) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::DirectoryNotEmpty => {
            warn!("keeping non-empty cargo home directory `{home_display}`");
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("failed to remove {name} `{home_display}`")),
    }
}

/// Remove rustup and its proxies from a bin directory, removing the directory only if empty.
fn clean_bin_home(bin_home: &Path) -> anyhow::Result<()> {
    info!("removing rustup tool links and binary");

    let rustup_path = bin_home.join(format!("rustup{EXE_SUFFIX}"));
    let proxy_paths = TOOLS
        .iter()
        .chain(DUP_TOOLS.iter())
        .map(|tool| bin_home.join(format!("{tool}{EXE_SUFFIX}")));

    for proxy_path in proxy_paths {
        if is_same_file(&proxy_path, &rustup_path).unwrap_or(false) {
            utils::remove_file("rustup tool proxy", &proxy_path)?;
        }
    }

    utils::ensure_file_removed("rustup_bin", &rustup_path)?;

    let bin_home_display = bin_home.display();
    info!("removing empty bin directory `{bin_home_display}`");

    match fs::remove_dir(bin_home) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::DirectoryNotEmpty => {
            warn!("keeping non-empty bin directory `{bin_home_display}`");
            Ok(())
        }
        Err(e) => {
            Err(e).with_context(|| format!("failed to remove bin directory `{bin_home_display}`"))
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum SelfUpdatePermission {
    HardFail,
    #[cfg(not(windows))]
    Skip,
    Permit,
}

#[cfg(windows)]
pub(crate) fn self_update_permitted(_explicit: bool) -> anyhow::Result<SelfUpdatePermission> {
    Ok(SelfUpdatePermission::Permit)
}

#[cfg(not(windows))]
pub(crate) fn self_update_permitted(explicit: bool) -> anyhow::Result<SelfUpdatePermission> {
    // Detect if rustup is not meant to self-update
    let current_exe = env::current_exe()?;
    let current_exe_dir = current_exe.parent().expect("Rustup isn't in a directory‽");
    if let Err(e) = tempfile::Builder::new()
        .prefix("updtest")
        .tempdir_in(current_exe_dir)
    {
        match e.kind() {
            io::ErrorKind::PermissionDenied => {
                trace!("Skipping self-update because we cannot write to the rustup dir");
                if explicit {
                    return Ok(SelfUpdatePermission::HardFail);
                } else {
                    return Ok(SelfUpdatePermission::Skip);
                }
            }
            _ => return Err(e.into()),
        }
    }
    Ok(SelfUpdatePermission::Permit)
}

/// Downloads the managed updater and runs it in replacement mode.
///
/// The updater is removed by a later rustup invocation because Windows
/// cannot delete the updater while its process is still running.
pub(crate) async fn update(cfg: &Cfg<'_>) -> anyhow::Result<ExitCode> {
    common::warn_if_host_is_emulated(cfg.process);

    use SelfUpdatePermission::*;
    let update_permitted = if cfg!(feature = "no-self-update") {
        HardFail
    } else {
        self_update_permitted(true)?
    };
    match update_permitted {
        HardFail => {
            // TODO: Detect which package manager and be more useful.
            error!("self-update is disabled for this build of rustup");
            error!("you should probably use your system package manager to update rustup");
            return Ok(ExitCode::FAILURE);
        }
        #[cfg(not(windows))]
        Skip => {
            info!("Skipping self-update at this time");
            return Ok(ExitCode::SUCCESS);
        }
        Permit => {}
    }

    match prepare_update(&DownloadCfg::new(cfg)).await? {
        Some(prepared_updater) => {
            let Some(version) = get_and_parse_new_rustup_version(&prepared_updater) else {
                error!("failed to get rustup version");
                return Ok(ExitCode::FAILURE);
            };

            let _ = common::show_channel_update(
                cfg,
                PackageUpdate::Rustup,
                Ok(UpdateStatus::Updated(version)),
            )
            .await;
            return run_update(prepared_updater, cfg.process);
        }
        None => {
            let _ = common::show_channel_update(
                cfg,
                PackageUpdate::Rustup,
                Ok(UpdateStatus::Unchanged),
            )
            .await;
            // Try again in case we emitted "tool `{}` is already installed" last time.
            install_proxies(cfg.process)?
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn get_and_parse_new_rustup_version(path: &Path) -> Option<String> {
    get_new_rustup_version(path).map(parse_new_rustup_version)
}

fn get_new_rustup_version(path: &Path) -> Option<String> {
    let output = Command::new(path).arg("--version").output().ok()?;
    String::from_utf8(output.stdout).ok()
}

fn parse_new_rustup_version(version: String) -> String {
    use std::sync::LazyLock;

    use regex::Regex;

    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[0-9]+.[0-9]+.[0-9]+[0-9a-zA-Z-]*").unwrap());

    let capture = RE.captures(&version);
    let matched_version = match capture {
        Some(cap) => cap.get(0).unwrap().as_str(),
        None => "(unknown)",
    };
    String::from(matched_version)
}

async fn prepare_update(dl_cfg: &DownloadCfg<'_>) -> anyhow::Result<Option<PreparedUpdater>> {
    let bin_home = dl_cfg.process.bin_home()?;
    let rustup_path = bin_home.join(format!("rustup{EXE_SUFFIX}"));

    if !rustup_path.exists() {
        return Err(CliError::NotSelfInstalled { p: bin_home }.into());
    }
    let self_update_lock = SelfUpdateLock::lock(dl_cfg.process)?;

    // Get build tuple
    let tuple = TargetTuple::from_build();

    // For windows x86 builds seem slow when used with windows defender.
    // The website defaulted to i686-windows-gnu builds for a long time.
    // This ensures that we update to a version that's appropriate for users
    // and also works around if the website messed up the detection.
    // If someone really wants to use another version, they still can enforce
    // that using the environment variable RUSTUP_OVERRIDE_HOST_TUPLE.
    #[cfg(windows)]
    let tuple = TargetTuple::from_host(dl_cfg.process).unwrap_or(tuple);

    // Get update root.
    let update_root = update_root(dl_cfg.process);

    // Get current version
    let current_version = env!("CARGO_PKG_VERSION");

    // Get available version
    info!("checking for self-update (current version: {current_version})");
    let available_version = match dl_cfg.process.var_opt("RUSTUP_VERSION")? {
        Some(ver) => {
            info!("`RUSTUP_VERSION` has been set to `{ver}`");
            ver
        }
        None => get_available_rustup_version(dl_cfg).await?,
    };

    // If up-to-date
    if available_version == current_version {
        return Ok(None);
    }

    // Get download URL
    let url = format!("{update_root}/archive/{available_version}/{tuple}/rustup-init{EXE_SUFFIX}");

    // Get download path
    let download_url = utils::parse_url(&url)?;
    let prepared_updater = PreparedUpdater::try_from(self_update_lock)?;

    // Download the new rustup executable
    info!("downloading self-update (new version: {available_version})");
    match dl_cfg.download_hash(download_url.as_str()).await {
        Ok(hash) => {
            let name = "rustup";
            let status = dl_cfg.status_for(name, name.len());

            let installer = dl_cfg
                .download_with_retry(&download_url, &hash, &status, dl_cfg.max_retries())
                .await?;

            utils::rename(
                name,
                &installer,
                &prepared_updater,
                dl_cfg.permit_copy_rename,
            )?;
        }
        // If the corresponding `.sha256` file is not found, skip hash check and use simple download
        // as a fallback.
        Err(e)
            if matches!(
                e.downcast_ref::<DownloadError>(),
                Some(DownloadError::HttpStatus(404) | DownloadError::FileNotFound)
            ) =>
        {
            warn!(url = %download_url, "`.sha256` checksum not found, skipping hash check");
            DownloadOptions::try_from(dl_cfg.process)?
                .start(&download_url, &prepared_updater)
                .download()
                .await?;
        }
        Err(e) => return Err(e),
    };

    // Mark as executable
    utils::make_executable(&prepared_updater)?;

    #[cfg(feature = "test")]
    dl_cfg.process.checkpoint(CHECKPOINT_SELF_UPDATE_PREPARED);

    Ok(Some(prepared_updater))
}

async fn get_available_rustup_version(dl_cfg: &DownloadCfg<'_>) -> anyhow::Result<String> {
    let update_root = update_root(dl_cfg.process);
    let tempdir = tempfile::Builder::new()
        .prefix("rustup-update")
        .tempdir()
        .context("error creating temp directory")?;

    // Parse the release file.
    let release_file_url = format!("{update_root}/release-stable.toml");
    let release_file_url = utils::parse_url(&release_file_url)?;
    let release_file = tempdir.path().join("release-stable.toml");
    DownloadOptions::try_from(dl_cfg.process)?
        .start(&release_file_url, &release_file)
        .download()
        .await?;

    let release_toml_str = utils::read_file("rustup release", &release_file)?;
    let release_toml = toml::from_str::<RustupManifest>(&release_toml_str)
        .context("unable to parse rustup release file")?;

    Ok(release_toml.version)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
struct RustupManifest {
    schema_version: SchemaVersion,
    version: String,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum SchemaVersion {
    #[serde(rename = "1")]
    #[default]
    V1,
}

impl SchemaVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V1 => "1",
        }
    }
}

impl FromStr for SchemaVersion {
    type Err = RustupError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" => Ok(Self::V1),
            _ => Err(RustupError::UnsupportedVersion(s.to_owned())),
        }
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Returns whether an update was available
pub(crate) async fn check_rustup_update(dl_cfg: &DownloadCfg<'_>) -> anyhow::Result<bool> {
    let t = dl_cfg.process.stdout();
    let mut t = t.lock();
    // Get current rustup version
    let current_version = env!("CARGO_PKG_VERSION");

    // Get available rustup version
    let available_version = get_available_rustup_version(dl_cfg).await?;

    let bold = Style::new().bold();
    let yellow = WARN;
    let green = GOOD;

    write!(t, "{bold}rustup - {bold:#}")?;

    Ok(if current_version != available_version {
        writeln!(
            t,
            "{yellow}update available{yellow:#} : {current_version} -> {available_version}"
        )?;
        true
    } else {
        writeln!(t, "{green}up to date{green:#} : {current_version}")?;
        false
    })
}

#[tracing::instrument(level = "trace")]
pub(crate) fn cleanup_self_updater(process: &Process, bin_path: &Path) -> anyhow::Result<()> {
    stage::cleanup(process, bin_path)
}

static DEFAULT_UPDATE_ROOT: &str = "https://static.rust-lang.org/rustup";
#[cfg(feature = "test")]
pub const CHECKPOINT_SELF_UPDATE_PREPARED: &str = "self-update-prepared";
#[cfg(feature = "test")]
pub const CHECKPOINT_SELF_REPLACE_READY: &str = "self-replace-ready";

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::Path};

    use super::HomeDisplay;
    use crate::{
        cli::self_update::InstallOpts,
        config::Cfg,
        dist::{PartialChannelToolchainName, Profile},
        for_host,
        process::TestProcess,
        test::{Env, test_dir, with_rustup_home},
    };

    #[test]
    fn bin_home_display() {
        let home = Path::new("/home/user");
        let cargo_bin_dir = home.join(".cargo").join("bin");
        assert_eq!(
            HomeDisplay::new(&cargo_bin_dir, Some(home)).to_string(),
            cfg_select! {
                windows => r"%USERPROFILE%\.cargo\bin",
                _ => "$HOME/.cargo/bin",
            }
        );

        let local_bin_dir = home.join(".local").join("bin");
        assert_eq!(
            HomeDisplay::new(&local_bin_dir, Some(home)).to_string(),
            cfg_select! {
                windows => r"%USERPROFILE%\.local\bin",
                _ => "$HOME/.local/bin",
            }
        );
        assert_eq!(
            HomeDisplay::new(home, Some(home)).to_string(),
            cfg_select! {
                windows => "%USERPROFILE%",
                _ => "$HOME",
            }
        );

        for bin_home in ["/opt/rust/bin", "/home/username/bin", ".cargo/bin"] {
            let bin_home = Path::new(bin_home);
            assert_eq!(
                HomeDisplay::new(bin_home, Some(home)).to_string(),
                bin_home.display().to_string()
            );
        }

        assert_eq!(
            HomeDisplay::new(&cargo_bin_dir, None).to_string(),
            cargo_bin_dir.display().to_string()
        );
    }

    #[test]
    fn default_toolchain_is_stable() {
        with_rustup_home(|home| {
            let mut vars = HashMap::new();
            home.apply(&mut vars);
            vars.env("CARGO_HOME", &home.rustupdir);
            let tp = TestProcess::with_vars(vars);
            let mut cfg =
                Cfg::from_env(tp.process.current_dir().unwrap(), false, true, &tp.process).unwrap();

            let opts = InstallOpts {
                default_host_tuple: None,
                default_toolchain: None,   // No toolchain specified
                profile: Profile::Default, // default profile
                no_modify_path: false,
                components: &[],
                targets: &[],
                no_update_toolchain: false,
            };

            assert_eq!(
                "stable".parse::<PartialChannelToolchainName>().unwrap(),
                opts.select_toolchain(&mut cfg)
                    .unwrap() // result
                    .unwrap() // option
            );
            assert_eq!(
                for_host!(
                    r"info: profile set to default
info: default host tuple is {0}
"
                ),
                &String::from_utf8(tp.stderr()).unwrap()
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn install_bins_creates_cargo_home() {
        let root_dir = test_dir().unwrap();
        let cargo_home = root_dir.path().join("cargo");
        let rustup_home = root_dir.path().join("rustup");
        let mut vars = HashMap::new();
        vars.env("CARGO_HOME", cargo_home.to_string_lossy().to_string());
        vars.env("RUSTUP_HOME", rustup_home);
        let tp = TestProcess::with_vars(vars);
        super::install_bins(&tp.process, &cargo_home.join("bin"), false).unwrap();
        assert!(cargo_home.exists());
    }
}
