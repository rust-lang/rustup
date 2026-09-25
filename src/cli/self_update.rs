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
//! * Delete all resolved Rustup homes.
//! * Delete all entries in `$CARGO_HOME` except `bin`.
//! * Delete rustup tool links and binary from `$CARGO_HOME/bin`.
//! * Delete `$CARGO_HOME/bin` if it is empty after uninstall.
//! * Delete `$CARGO_HOME` if it is empty after uninstall.
//!
//! Deleting the running binary during uninstall is tricky
//! and racy on Windows.

#[cfg(unix)]
use std::borrow::Cow;
use std::{
    env::{self, consts::EXE_SUFFIX},
    fmt, fs,
    io::{self, Write},
    path::{Component, MAIN_SEPARATOR, Path, PathBuf},
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
        DistOptions, PartialToolchainDesc, Profile, TargetTuple, ToolchainDesc,
        download::DownloadCfg,
    },
    download::DownloadOptions,
    errors::RustupError,
    install::{InstallMethod, UpdateStatus},
    process::Process,
    settings::SettingsFile,
    toolchain::{
        DistributableToolchain, MaybeOfficialToolchainName, ResolvableToolchainName, Toolchain,
    },
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
use unix::{add_to_path, remove_from_path, run_update};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::complete_windows_uninstall;
#[cfg(windows)]
pub(crate) use windows::self_replace;
#[cfg(all(windows, feature = "test"))]
pub use windows::{RUSTUP_REGISTRY_TEST_ID, RegistryValueId, USER_PATH, get_path};
#[cfg(windows)]
use windows::{add_to_path, add_uninstall_registry_entry, remove_from_path, run_update};

pub(crate) struct InstallOpts<'a> {
    pub default_host_tuple: Option<String>,
    pub default_toolchain: Option<MaybeOfficialToolchainName>,
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

        let bin_home = process.rustup_bin_home()?;
        let home_dir = process.home_dir();
        let msg = if no_modify_path {
            format!(
                post_install_msg_no_modify_path!(),
                rustup_bin_home = HomeDisplay::new(&bin_home, home_dir.as_deref()),
            )
        } else {
            format!(
                post_install_msg!(),
                rustup_bin_home = HomeDisplay::new(&bin_home, home_dir.as_deref()),
            )
        };
        md(&mut term, msg);
        #[cfg(not(windows))]
        {
            let env_home = process.rustup_env_home()?;
            md(
                &mut term,
                format!(
                    post_install_msg_unix!(),
                    env_dir = HomeDisplay::new(&env_home, home_dir.as_deref()),
                    source_env_lines = shell::build_source_env_lines(process, &env_home),
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
        let bin_home = process.rustup_bin_home()?;
        install_bins(process, &bin_home, force_hard_links(process))?;

        #[cfg(unix)]
        unix::write_env_files(process)?;

        if !self.no_modify_path {
            add_to_path(process)?;
        }

        #[cfg(windows)]
        add_uninstall_registry_entry(process)?;

        let mut cfg = Cfg::from_env(current_dir, quiet, false, process)?;

        let (components, targets) = (self.components, self.targets);
        let toolchain = self.select_toolchain(&mut cfg)?;
        if let Some(partial_desc) = toolchain {
            let desc = partial_desc.clone().resolve(&cfg.default_host_tuple()?)?;
            let options =
                DistOptions::new(components, targets, &desc, cfg.get_profile()?, true, &cfg)?;
            let status = if Toolchain::exists(&cfg, &desc.clone().into())? {
                warn!("Updating existing toolchain, profile choice will be ignored");
                // If we have a partial install we might not be able to read content here. We could:
                // - fail and folk have to delete the partially present toolchain to recover
                // - silently ignore it (and provide inconsistent metadata for reporting the install/update change)
                // - delete the partial install and start over
                // For now, we error.
                let toolchain = DistributableToolchain::new(&cfg, desc.clone())?;
                InstallMethod::Dist(options.for_update(&toolchain, false))
                    .install(None)
                    .await?
            } else {
                DistributableToolchain::install(options).await?.status
            };

            check_proxy_sanity(&bin_home, components, &desc)?;

            cfg.set_default(Some(&partial_desc.into()))?;
            writeln!(cfg.process.stdout().lock())?;
            common::show_channel_update(&cfg, PackageUpdate::Toolchain(desc), Ok(status))?;
        }
        Ok(())
    }

    /// Selects the toolchain to install based on the user's intent.
    ///
    /// This function first initializes the default profile and default host tuple in the
    /// configuration, then returns the toolchain that should be installed, or `None` if none is
    /// specified by the user.
    fn select_toolchain(self, cfg: &mut Cfg<'_>) -> anyhow::Result<Option<PartialToolchainDesc>> {
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
        if matches!(default_toolchain, Some(MaybeOfficialToolchainName::None)) {
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
                Some(s) => {
                    let toolchain_name = match s {
                        MaybeOfficialToolchainName::None => unreachable!(),
                        MaybeOfficialToolchainName::Some(n) => n,
                    };
                    Some(toolchain_name)
                }
                None => match cfg.get_default_resolvable()? {
                    // Default is installable
                    Some(ResolvableToolchainName::Official(t)) => Some(t),
                    // Default is custom, presumably from a prior install. Do nothing.
                    Some(ResolvableToolchainName::Custom(_)) => None,
                    None => Some(PartialToolchainDesc::from_str("stable")?),
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

        self.default_toolchain = Some(MaybeOfficialToolchainName::from_str(
            &common::question_str(
                "Default toolchain? (stable/beta/nightly/none)",
                &match &self.default_toolchain {
                    Some(name) => name.to_string(),
                    None => "stable".to_owned(),
                },
                process,
            )?,
        )?);

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
        let partial_channel = match &self.default_toolchain {
            None | Some(MaybeOfficialToolchainName::None) => {
                ResolvableToolchainName::from_str("stable")?
            }
            Some(MaybeOfficialToolchainName::Some(s)) => s.into(),
        };
        let resolved = partial_channel.resolve(&host_tuple)?;
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
                Some(name) => name.to_string(),
                None => "stable (default)".to_owned(),
            },
            self.profile,
            if !self.no_modify_path { "yes" } else { "no" }
        )
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
                home_prefix: Some(cfg_select! {
                    windows => "%USERPROFILE%",
                    _ => "$HOME",
                }),
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
    // Ignore rustc and cargo if present in $HOME/.cargo/bin or a few other directories
    #[allow(clippy::ptr_arg)]
    fn ignore_paths(path: &PathBuf) -> bool {
        !path
            .components()
            .any(|c| c == Component::Normal(".cargo".as_ref()))
    }

    let rustup_bin_home = process.rustup_bin_home()?;
    if let Some(paths) = process.var_os("PATH") {
        let paths =
            env::split_paths(&paths).filter(|path| ignore_paths(path) && path != &rustup_bin_home);

        for path in paths {
            let rustc = path.join(format!("rustc{EXE_SUFFIX}"));
            let cargo = path.join(format!("cargo{EXE_SUFFIX}"));

            if rustc.exists() || cargo.exists() {
                return Err(anyhow!("{}", path.to_str().unwrap().to_owned()));
            }
        }
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
    let inferred = PartialToolchainDesc::from_str("stable")?.resolve(&default_host_tuple)?;
    if default_toolchain != inferred.to_string() {
        warn!("rustup will install the default toolchain as specified in the settings file,");
        warn!("instead of the one inferred from the default host tuple.");
    }
    Ok(())
}

fn pre_install_msg(no_modify_path: bool, process: &Process) -> anyhow::Result<String> {
    let rustup_bin_home = process.rustup_bin_home()?;
    let home_dirs = process.home_dirs()?;
    let rustup_home_message = if !process.use_category_home() {
        // In legacy mode, all four category homes equal the resolved RUSTUP_HOME.
        format!(
            concat!(
                "Rustup metadata and toolchains will be installed into the Rustup\n",
                "home directory, located at:\n\n",
                "    {}\n\n",
                "This can be modified with the `RUSTUP_HOME` environment variable."
            ),
            home_dirs.data.display()
        )
    } else {
        format!(
            concat!(
                "Rustup will use these directories:\n\n",
                "      config: {}\n",
                "      state:  {}\n",
                "      data:   {}\n",
                "      cache:  {}\n\n",
                "They can be modified individually with\n",
                "`RUSTUP_CONFIG_HOME`, `RUSTUP_STATE_HOME`, `RUSTUP_DATA_HOME`, and\n",
                "`RUSTUP_CACHE_HOME`."
            ),
            home_dirs.config.display(),
            home_dirs.state.display(),
            home_dirs.data.display(),
            home_dirs.cache.display(),
        )
    };

    if !no_modify_path {
        // Brittle code warning: some duplication in unix::add_to_path
        #[cfg(not(windows))]
        {
            let rcfiles = shell::get_available_shells(process)
                .flat_map(|sh| sh.rcs(process).into_iter())
                .map(|rc| format!("    {}", rc.display()))
                .collect::<Vec<_>>();
            let plural = if rcfiles.len() > 1 { "s" } else { "" };
            let rcfiles = rcfiles.join("\n");
            Ok(format!(
                pre_install_msg_unix!(),
                rustup_bin_home = rustup_bin_home.display(),
                plural = plural,
                rcfiles = rcfiles,
                rustup_home_message = rustup_home_message,
            ))
        }
        #[cfg(windows)]
        Ok(format!(
            pre_install_msg_win!(),
            rustup_bin_home = rustup_bin_home.display(),
            rustup_home_message = rustup_home_message,
        ))
    } else {
        Ok(format!(
            pre_install_msg_no_modify_path!(),
            rustup_bin_home = rustup_bin_home.display(),
            rustup_home_message = rustup_home_message,
        ))
    }
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
    let bin_path = process.rustup_bin_home()?;
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
    desc: &ToolchainDesc,
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

/// Uninstall process:
/// 1. Remove all installed toolchains.
/// 2. Remove all resolved Rustup homes.
/// 3. Remove Cargo home data, preserving both bin directories.
/// 4. Remove rustup tool links and binaries from both bin directories.
/// 5. Remove empty bin directories and clean up PATH as appropriate.
/// 6. Try to remove the Cargo home directory if it's empty.
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
    let cargo_home = process.cargo_home()?;
    let legacy_bin = cargo_home.join("bin");
    let category_bin = process.rustup_bin_home()?;
    let rustup_exe = format!("rustup{EXE_SUFFIX}");
    let legacy_rustup = legacy_bin.join(&rustup_exe);
    let category_rustup = category_bin.join(rustup_exe);
    let rustup_is_self_installed = legacy_rustup.try_exists()?
        || (category_bin != legacy_bin && category_rustup.try_exists()?);
    if !rustup_is_self_installed {
        return Err(CliError::NotSelfInstalled { p: cargo_home }.into());
    }

    // Resolve the legacy home before displaying the uninstall notice.
    let legacy_home = process.rustup_home()?;
    let rustup_homes = [
        ("rustup home", &legacy_home),
        ("rustup cache home", &cfg.rustup_cache_dir),
        ("rustup config home", &cfg.rustup_config_dir),
        ("rustup data home", &cfg.rustup_data_dir),
        ("rustup state home", &cfg.rustup_state_dir),
    ];

    if !no_prompt {
        writeln!(process.stdout().lock())?;
        let msg = if process.use_category_home() {
            let rustup_homes = format!(
                concat!(
                    "      config: {}\n",
                    "      state:  {}\n",
                    "      data:   {}\n",
                    "      cache:  {}\n\n",
                    "      legacy Rustup home: {}"
                ),
                cfg.rustup_config_dir.display(),
                cfg.rustup_state_dir.display(),
                cfg.rustup_data_dir.display(),
                cfg.rustup_cache_dir.display(),
                legacy_home.display(),
            );
            let mut bin_homes = format!("- `{}`", legacy_bin.display());
            if category_bin != legacy_bin {
                bin_homes.push_str(&format!("\n- `{}`", category_bin.display()));
            }
            let path_message = if no_modify_path {
                "Your `PATH` environment variable and shell profiles will not be modified."
            } else {
                "Rustup shell setup and PATH entries will be cleaned up where applicable."
            };
            format!(
                pre_uninstall_category_msg!(),
                rustup_homes = rustup_homes,
                cargo_home = cargo_home.display(),
                bin_homes = bin_homes,
                path_message = path_message,
            )
        } else if no_modify_path {
            pre_uninstall_msg_no_modify_path!().to_owned()
        } else {
            let bin_home = cargo_home.join("bin");
            let cargo_bin_dir = HomeDisplay::new(&bin_home, process.home_dir().as_deref());
            format!(pre_uninstall_msg!(), cargo_bin_dir = cargo_bin_dir)
        };
        md(&mut process.stdout(), msg);
        if !common::confirm("\nContinue? (y/N)", false, process)? {
            info!("aborting uninstallation");
            return Ok(ExitCode::SUCCESS);
        }
    }

    #[cfg(unix)]
    if process.use_category_home() && !no_modify_path {
        // Remove both current and legacy env script references, but process a
        // shared data/Cargo home only once.
        let data_home = &cfg.rustup_data_dir;
        if data_home == &cargo_home {
            remove_from_path(process, &[data_home], &cargo_home)?;
        } else {
            remove_from_path(process, &[data_home, &cargo_home], &cargo_home)?;
        }
    }

    info!("removing toolchains");
    for toolchain in cfg.list_toolchains(true)? {
        Toolchain::ensure_removed(cfg, toolchain.into())?;
    }

    info!("removing rustup home");

    // Delete the legacy Rustup home and all resolved category homes.
    for (name, rustup_dir) in rustup_homes {
        if rustup_dir.try_exists()? {
            utils::remove_dir(name, rustup_dir)?;
        }
    }

    // Delete rustup.
    #[cfg(unix)]
    {
        clean_cargo_home_data(&cargo_home, &category_bin)?;
        clean_bin_homes(no_modify_path, process, &cargo_home, &category_bin)?;
        remove_empty_cargo_home(&cargo_home)?;
    }
    // NOTE: On windows, this is tricky because this is *probably*
    // the running executable and on Windows can't be unlinked until
    // the process exits.
    // see: windows::{complete_windows_uninstall,spawn_uninstall_gc}
    #[cfg(windows)]
    windows::spawn_uninstall_gc(no_modify_path)?;

    info!("rustup is uninstalled");

    Ok(ExitCode::SUCCESS)
}

/// Remove Cargo home data while preserving both bin directories and their contents.
fn clean_cargo_home_data(cargo_home: &Path, category_bin: &Path) -> anyhow::Result<()> {
    info!("removing cargo home");

    // Check every entry before deleting any of them.
    for path in cargo_home_entries_to_remove(cargo_home, category_bin)? {
        if path.is_dir() {
            utils::remove_dir("cargo_home", &path)?;
        } else {
            utils::remove_file("cargo_home", &path)?;
        }
    }

    Ok(())
}

/// Remove Rustup binaries from both bin homes, preserving unrelated programs.
/// Update PATH where appropriate when a bin directory is removed.
fn clean_bin_homes(
    no_modify_path: bool,
    process: &Process,
    cargo_home: &Path,
    category_bin: &Path,
) -> anyhow::Result<()> {
    let legacy_bin = cargo_home.join("bin");

    info!("removing rustup tool links and binary");

    for bin in std::iter::once(legacy_bin.as_path())
        .chain((category_bin != legacy_bin).then_some(category_bin))
    {
        let bin_removed = clean_rustup_binaries(bin)?;
        if bin_removed && !no_modify_path {
            #[cfg(windows)]
            remove_from_path(process, bin)?;
            #[cfg(unix)]
            if !process.use_category_home() && bin == legacy_bin {
                remove_from_path(process, &[cargo_home], cargo_home)?;
            }
        }
    }

    Ok(())
}

/// Remove Cargo home only if no entries remain after data and binary cleanup.
fn remove_empty_cargo_home(cargo_home: &Path) -> anyhow::Result<()> {
    let cargo_home_display = cargo_home.display();
    info!("removing empty cargo home directory `{cargo_home_display}`");

    match fs::remove_dir(cargo_home) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) if e.kind() == io::ErrorKind::DirectoryNotEmpty => {
            warn!("keeping non-empty cargo home directory `{cargo_home_display}`");
        }
        Err(e) => {
            return Err(e).with_context(|| {
                format!("failed to remove cargo home directory `{cargo_home_display}`")
            });
        }
        Ok(()) => {}
    }

    Ok(())
}

/// Find Cargo home entries that can be removed without breaking either bin directory.
/// This only inspects paths; the caller deletes them after all checks succeed.
fn cargo_home_entries_to_remove(
    cargo_home: &Path,
    category_bin: &Path,
) -> anyhow::Result<Vec<PathBuf>> {
    let category_bin_location = match fs::canonicalize(category_bin) {
        Ok(path) => Some(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "could not resolve bin directory '{}'",
                    category_bin.display()
                )
            });
        }
    };

    let entries = match fs::read_dir(cargo_home) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(CliError::ReadDirError {
                p: cargo_home.to_owned(),
                source,
            }
            .into());
        }
    };
    let cargo_home_location = fs::canonicalize(cargo_home)
        .with_context(|| format!("could not resolve cargo home '{}'", cargo_home.display()))?;
    let cargo_home_is_bin = category_bin_location.as_ref() == Some(&cargo_home_location);

    let mut paths_to_remove = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| CliError::ReadDirError {
            p: cargo_home.to_owned(),
            source,
        })?;
        let path = entry.path();

        // Keep the legacy bin entry, including a symlink to the migrated bin home.
        // If Cargo home itself is the bin home, keep all its entries.
        if entry.file_name() == "bin" || cargo_home_is_bin {
            continue;
        }

        // Keep any subtree containing the actual category bin directory.
        if path.is_dir()
            && let Some(bin_location) = &category_bin_location
        {
            let location = fs::canonicalize(&path).with_context(|| {
                format!("could not resolve cargo directory '{}'", path.display())
            })?;
            if bin_location.starts_with(location) {
                continue;
            }
        }

        paths_to_remove.push(path);
    }

    Ok(paths_to_remove)
}

/// Remove rustup-owned binaries from a bin directory.
///
/// Returns whether the directory was removed after becoming empty.
fn clean_rustup_binaries(bin_dir: &Path) -> anyhow::Result<bool> {
    let rustup_path = bin_dir.join(format!("rustup{EXE_SUFFIX}"));
    if !rustup_path.try_exists()? {
        return Ok(false);
    }

    let proxy_paths = TOOLS
        .iter()
        .chain(DUP_TOOLS.iter())
        .map(|tool| bin_dir.join(format!("{tool}{EXE_SUFFIX}")));

    for proxy_path in proxy_paths {
        if is_same_file(&proxy_path, &rustup_path).unwrap_or(false) {
            utils::remove_file("rustup tool proxy", &proxy_path)?;
        }
    }

    for entry in fs::read_dir(bin_dir)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(stage::PENDING_BINARY_PREFIX)
        {
            utils::remove_file("pending rustup binary", &entry.path())?;
        }
    }

    utils::remove_file("rustup_bin", &rustup_path)?;

    let bin_dir_display = bin_dir.display();
    info!("removing empty cargo bin directory `{bin_dir_display}`");

    // Remove the actual directory only if it is empty, including when bin_dir
    // is a symlink. Keep the alias itself, which may have been created by the user.
    match fs::remove_dir(fs::canonicalize(bin_dir)?) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {
            warn!("keeping non-empty cargo bin directory `{bin_dir_display}`");
            Ok(false)
        }
        Err(error) => Err(error)
            .with_context(|| format!("failed to remove cargo bin directory `{bin_dir_display}`")),
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
            );
            return run_update(prepared_updater, cfg.process);
        }
        None => {
            let _ = common::show_channel_update(
                cfg,
                PackageUpdate::Rustup,
                Ok(UpdateStatus::Unchanged),
            );
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
    let bin_home = dl_cfg.process.rustup_bin_home()?;
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
    let setup_path: &Path = &prepared_updater;

    // Download new version
    info!("downloading self-update (new version: {available_version})");
    DownloadOptions::try_from(dl_cfg.process)?
        .start(&download_url, setup_path)
        .download()
        .await?;

    // Mark as executable
    utils::make_executable(setup_path)?;

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
        dist::{PartialToolchainDesc, Profile},
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
                "stable".parse::<PartialToolchainDesc>().unwrap(),
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
