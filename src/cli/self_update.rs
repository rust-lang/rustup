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
//! * copy the self exe to $CARGO_HOME/bin
//! * hardlink rustc, etc to *that*
//! * update the PATH in a system-specific way
//! * run the equivalent of `rustup default stable`
//!
//! During upgrade (`rustup self upgrade`):
//!
//! * download rustup-init to $CARGO_HOME/bin/rustup-init
//! * run rustup-init with appropriate flags to indicate
//!   this is a self-upgrade
//! * rustup-init copies bins and hardlinks into place. On windows
//!   this happens *after* the upgrade command exits successfully.
//!
//! During uninstall (`rustup self uninstall`):
//!
//! * Delete `$RUSTUP_HOME`.
//! * Delete everything in `$CARGO_HOME`, including
//!   the rustup binary and its hardlinks
//!
//! Deleting the running binary during uninstall is tricky
//! and racy on Windows.

use std::borrow::Cow;
use std::env::{self, consts::EXE_SUFFIX};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Component, MAIN_SEPARATOR, Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use cfg_if::cfg_if;
use clap::ValueEnum;
use clap::builder::PossibleValue;
use itertools::Itertools;
use same_file::Handle;
use serde::{Deserialize, Serialize};
use tracing::{error, info, trace, warn};

use crate::download::download_file;
use crate::{
    DUP_TOOLS, TOOLS,
    cli::{
        common::{self, Confirm, PackageUpdate, ignorable_error, report_error},
        errors::*,
        markdown::md,
    },
    config::{Cfg, non_empty_env_var},
    dist::{self, PartialToolchainDesc, Profile, TargetTriple, ToolchainDesc},
    errors::RustupError,
    install::UpdateStatus,
    process::{Process, terminalsource},
    toolchain::{
        DistributableToolchain, MaybeOfficialToolchainName, ResolvableToolchainName, Toolchain,
        ToolchainName,
    },
    utils::{self, Notification},
};

#[cfg(unix)]
mod shell;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::{delete_rustup_and_cargo_home, do_add_to_path, do_remove_from_path};
#[cfg(unix)]
pub(crate) use unix::{run_update, self_replace};

#[cfg(unix)]
use self::shell::{Nu, UnixShell};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::complete_windows_uninstall;
#[cfg(all(windows, feature = "test"))]
pub use windows::{RegistryGuard, RegistryValueId, USER_PATH, get_path};
#[cfg(windows)]
use windows::{delete_rustup_and_cargo_home, do_add_to_path, do_remove_from_path};
#[cfg(windows)]
pub(crate) use windows::{run_update, self_replace};

pub(crate) struct InstallOpts<'a> {
    pub default_host_triple: Option<String>,
    pub default_toolchain: Option<MaybeOfficialToolchainName>,
    pub profile: Profile,
    pub no_modify_path: bool,
    pub no_update_toolchain: bool,
    pub components: &'a [&'a str],
    pub targets: &'a [&'a str],
}

impl InstallOpts<'_> {
    fn install(self, cfg: &mut Cfg<'_>) -> Result<Option<ToolchainDesc>> {
        let Self {
            default_host_triple,
            default_toolchain,
            profile,
            no_modify_path: _no_modify_path,
            no_update_toolchain,
            components,
            targets,
        } = self;

        cfg.set_profile(profile)?;

        if let Some(default_host_triple) = &default_host_triple {
            // Set host triple now as it will affect resolution of toolchain_str
            info!("setting default host triple to {}", default_host_triple);
            cfg.set_default_host_triple(default_host_triple.to_owned())?;
        } else {
            info!("default host triple is {}", cfg.get_default_host_triple()?);
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
                    Some(toolchain_name.resolve(&cfg.get_default_host_triple()?)?)
                }
                None => match cfg.get_default()? {
                    // Default is installable
                    Some(ToolchainName::Official(t)) => Some(t),
                    // Default is custom, presumably from a prior install. Do nothing.
                    Some(ToolchainName::Custom(_)) => None,
                    None => Some(
                        "stable"
                            .parse::<PartialToolchainDesc>()?
                            .resolve(&cfg.get_default_host_triple()?)?,
                    ),
                },
            })
        } else {
            info!("updating existing rustup installation - leaving toolchains alone");
            writeln!(cfg.process.stdout().lock())?;
            Ok(None)
        }
    }

    // Interactive editing of the install options
    fn customize(&mut self, process: &Process) -> Result<()> {
        writeln!(
            process.stdout().lock(),
            "I'm going to ask you the value of each of these installation options.\n\
         You may simply press the Enter key to leave unchanged."
        )?;

        writeln!(process.stdout().lock())?;

        self.default_host_triple = Some(common::question_str(
            "Default host triple?",
            &self
                .default_host_triple
                .take()
                .unwrap_or_else(|| TargetTriple::from_host_or_build(process).to_string()),
            process,
        )?);

        self.default_toolchain = Some(MaybeOfficialToolchainName::try_from(common::question_str(
            "Default toolchain? (stable/beta/nightly/none)",
            &self
                .default_toolchain
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or("stable".into()),
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

    fn validate(&self, process: &Process) -> Result<()> {
        common::warn_if_host_is_emulated(process);

        let host_triple = self
            .default_host_triple
            .as_ref()
            .map(dist::TargetTriple::new)
            .unwrap_or_else(|| TargetTriple::from_host_or_build(process));
        let partial_channel = match &self.default_toolchain {
            None | Some(MaybeOfficialToolchainName::None) => {
                ResolvableToolchainName::try_from("stable")?
            }
            Some(MaybeOfficialToolchainName::Some(s)) => s.into(),
        };
        let resolved = partial_channel.resolve(&host_triple)?;
        trace!("Successfully resolved installation toolchain as: {resolved}");
        Ok(())
    }
}

#[cfg(feature = "no-self-update")]
pub(crate) const NEVER_SELF_UPDATE: bool = true;
#[cfg(not(feature = "no-self-update"))]
pub(crate) const NEVER_SELF_UPDATE: bool = false;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelfUpdateMode {
    #[default]
    Enable,
    Disable,
    CheckOnly,
}

impl SelfUpdateMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Enable => "enable",
            Self::Disable => "disable",
            Self::CheckOnly => "check-only",
        }
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

    fn from_str(mode: &str) -> Result<Self> {
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

impl std::fmt::Display for SelfUpdateMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// The big installation messages. These are macros because the first
// argument of format! needs to be a literal.

macro_rules! pre_install_msg_template {
    ($platform_msg:literal) => {
        concat!(
            r"
# Welcome to Rust!

This will download and install the official compiler for the Rust
programming language, and its package manager, Cargo.

Rustup metadata and toolchains will be installed into the Rustup
home directory, located at:

    {rustup_home}

This can be modified with the RUSTUP_HOME environment variable.

The Cargo home directory is located at:

    {cargo_home}

This can be modified with the CARGO_HOME environment variable.

The `cargo`, `rustc`, `rustup` and other commands will be added to
Cargo's bin directory, located at:

    {cargo_home_bin}

",
            $platform_msg,
            r#"

You can uninstall at any time with `rustup self uninstall` and
these changes will be reverted.
"#
        )
    };
}

#[cfg(not(windows))]
macro_rules! pre_install_msg_unix {
    () => {
        pre_install_msg_template!(
            "This path will then be added to your `PATH` environment variable by
modifying the profile file{plural} located at:

{rcfiles}"
        )
    };
}

#[cfg(windows)]
macro_rules! pre_install_msg_win {
    () => {
        pre_install_msg_template!(
            r#"This path will then be added to your `PATH` environment variable by
modifying the `PATH` registry key at `HKEY_CURRENT_USER\Environment`."#
        )
    };
}

macro_rules! pre_install_msg_no_modify_path {
    () => {
        pre_install_msg_template!(
            "This path needs to be in your `PATH` environment variable,
but will not be added automatically."
        )
    };
}

#[cfg(not(windows))]
macro_rules! post_install_msg_unix_source_env {
    () => {
        r#"To configure your current shell, you need to source
the corresponding `env` file under {cargo_home}.

This is usually done by running one of the following (note the leading DOT):
    . "{cargo_home}/env"            # For sh/bash/zsh/ash/dash/pdksh
    source "{cargo_home}/env.fish"  # For fish
    source $"{cargo_home_nushell}/env.nu"  # For nushell
"#
    };
}

#[cfg(not(windows))]
macro_rules! post_install_msg_unix {
    () => {
        concat!(
            r"# Rust is installed now. Great!

To get started you may need to restart your current shell.
This would reload your `PATH` environment variable to include
Cargo's bin directory ({cargo_home}/bin).

",
            post_install_msg_unix_source_env!(),
        )
    };
}

#[cfg(windows)]
macro_rules! post_install_msg_win {
    () => {
        r"# Rust is installed now. Great!


To get started you may need to restart your current shell.
This would reload its `PATH` environment variable to include
Cargo's bin directory ({cargo_home}\\bin).
"
    };
}

#[cfg(not(windows))]
macro_rules! post_install_msg_unix_no_modify_path {
    () => {
        concat!(
            r"# Rust is installed now. Great!

To get started you need Cargo's bin directory ({cargo_home}/bin) in your `PATH`
environment variable. This has not been done automatically.

",
            post_install_msg_unix_source_env!(),
        )
    };
}

#[cfg(windows)]
macro_rules! post_install_msg_win_no_modify_path {
    () => {
        r"# Rust is installed now. Great!

To get started you need Cargo's bin directory ({cargo_home}\\bin) in your `PATH`
environment variable. This has not been done automatically.
"
    };
}

macro_rules! pre_uninstall_msg {
    () => {
        r"# Thanks for hacking in Rust!

This will uninstall all Rust toolchains and data, and remove
`{cargo_home}/bin` from your `PATH` environment variable.

"
    };
}

static DEFAULT_UPDATE_ROOT: &str = "https://static.rust-lang.org/rustup";

fn update_root(process: &Process) -> String {
    process
        .var("RUSTUP_UPDATE_ROOT")
        .inspect(|url| trace!("`RUSTUP_UPDATE_ROOT` has been set to `{url}`"))
        .unwrap_or_else(|_| String::from(DEFAULT_UPDATE_ROOT))
}

/// `CARGO_HOME` suitable for display, possibly with $HOME
/// substituted for the directory prefix
fn canonical_cargo_home(process: &Process) -> Result<Cow<'static, str>> {
    let path = process.cargo_home()?;

    let default_cargo_home = process
        .home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cargo");
    Ok(if default_cargo_home == path {
        cfg_if! {
            if #[cfg(windows)] {
                r"%USERPROFILE%\.cargo".into()
            } else {
                "$HOME/.cargo".into()
            }
        }
    } else {
        path.to_string_lossy().into_owned().into()
    })
}

/// Installing is a simple matter of copying the running binary to
/// `CARGO_HOME`/bin, hard-linking the various Rust tools to it,
/// and adding `CARGO_HOME`/bin to PATH.
pub(crate) async fn install(
    current_dir: PathBuf,
    no_prompt: bool,
    quiet: bool,
    mut opts: InstallOpts<'_>,
    process: &Process,
) -> Result<utils::ExitCode> {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut exit_code = utils::ExitCode(0);

    opts.validate(process).map_err(|e| {
        anyhow!(
            "Pre-checks for host and toolchain failed: {e}\n\
            If you are unsure of suitable values, the 'stable' toolchain is the default.\n\
            Valid host triples look something like: {}",
            TargetTriple::from_host_or_build(process)
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
        exit_code &= unix::do_anti_sudo_check(no_prompt, process)?;
    }

    let mut term = process.stdout().terminal(process);

    #[cfg(windows)]
    windows::maybe_install_msvc(&mut term, no_prompt, quiet, &opts, process).await?;

    if !no_prompt {
        let msg = pre_install_msg(opts.no_modify_path, process)?;

        md(&mut term, msg);
        let mut customized_install = false;
        loop {
            md(&mut term, current_install_opts(&opts, process));
            match common::confirm_advanced(customized_install, process)? {
                Confirm::No => {
                    info!("aborting installation");
                    return Ok(utils::ExitCode(0));
                }
                Confirm::Yes => {
                    break;
                }
                Confirm::Advanced => {
                    customized_install = true;
                    opts.customize(process)?;
                }
            }
        }
    }

    let no_modify_path = opts.no_modify_path;
    if let Err(e) = maybe_install_rust(current_dir, quiet, opts, process).await {
        report_error(&e, process);

        // On windows, where installation happens in a console
        // that may have opened just for this purpose, give
        // the user an opportunity to see the error before the
        // window closes.
        #[cfg(windows)]
        if !no_prompt {
            windows::ensure_prompt(process)?;
        }

        return Ok(utils::ExitCode(1));
    }

    let cargo_home = canonical_cargo_home(process)?;
    #[cfg(windows)]
    let cargo_home = cargo_home.replace('\\', r"\\");
    #[cfg(windows)]
    let msg = if no_modify_path {
        format!(
            post_install_msg_win_no_modify_path!(),
            cargo_home = cargo_home
        )
    } else {
        format!(post_install_msg_win!(), cargo_home = cargo_home)
    };
    #[cfg(not(windows))]
    let cargo_home_nushell = Nu.cargo_home_str(process)?;
    #[cfg(not(windows))]
    let msg = if no_modify_path {
        format!(
            post_install_msg_unix_no_modify_path!(),
            cargo_home = cargo_home,
            cargo_home_nushell = cargo_home_nushell,
        )
    } else {
        format!(
            post_install_msg_unix!(),
            cargo_home = cargo_home,
            cargo_home_nushell = cargo_home_nushell,
        )
    };
    md(&mut term, msg);

    #[cfg(windows)]
    if !no_prompt {
        // On windows, where installation happens in a console
        // that may have opened just for this purpose, require
        // the user to press a key to continue.
        windows::ensure_prompt(process)?;
    }

    Ok(exit_code)
}

fn rustc_or_cargo_exists_in_path(process: &Process) -> Result<()> {
    // Ignore rustc and cargo if present in $HOME/.cargo/bin or a few other directories
    #[allow(clippy::ptr_arg)]
    fn ignore_paths(path: &PathBuf) -> bool {
        !path
            .components()
            .any(|c| c == Component::Normal(".cargo".as_ref()))
    }

    if let Some(paths) = process.var_os("PATH") {
        let paths = env::split_paths(&paths).filter(ignore_paths);

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

fn check_existence_of_rustc_or_cargo_in_path(no_prompt: bool, process: &Process) -> Result<()> {
    // Only the test runner should set this
    let skip_check = process.var_os("RUSTUP_INIT_SKIP_PATH_CHECK");

    // Skip this if the environment variable is set
    if skip_check == Some("yes".into()) {
        return Ok(());
    }

    if let Err(path) = rustc_or_cargo_exists_in_path(process) {
        warn!("It looks like you have an existing installation of Rust at:");
        warn!("{}", path);
        warn!("It is recommended that rustup be the primary Rust installation.");
        warn!("Otherwise you may have confusion unless you are careful with your PATH.");
        warn!("If you are sure that you want both rustup and your already installed Rust");
        warn!("then please reply `y' or `yes' or set RUSTUP_INIT_SKIP_PATH_CHECK to yes");
        warn!("or pass `-y' to ignore all ignorable checks.");
        ignorable_error("cannot install while Rust is installed", no_prompt, process)?;
    }
    Ok(())
}

fn check_existence_of_settings_file(process: &Process) -> Result<()> {
    let rustup_dir = process.rustup_home()?;
    let settings_file_path = rustup_dir.join("settings.toml");
    if utils::path_exists(&settings_file_path) {
        warn!("It looks like you have an existing rustup settings file at:");
        warn!("{}", settings_file_path.display());
        warn!("Rustup will install the default toolchain as specified in the settings file,");
        warn!("instead of the one inferred from the default host triple.");
    }
    Ok(())
}

fn pre_install_msg(no_modify_path: bool, process: &Process) -> Result<String> {
    let cargo_home = process.cargo_home()?;
    let cargo_home_bin = cargo_home.join("bin");
    let rustup_home = home::rustup_home()?;

    if !no_modify_path {
        // Brittle code warning: some duplication in unix::do_add_to_path
        #[cfg(not(windows))]
        {
            let rcfiles = shell::get_available_shells(process)
                .flat_map(|sh| sh.update_rcs(process).into_iter())
                .map(|rc| format!("    {}", rc.display()))
                .collect::<Vec<_>>();
            let plural = if rcfiles.len() > 1 { "s" } else { "" };
            let rcfiles = rcfiles.join("\n");
            Ok(format!(
                pre_install_msg_unix!(),
                cargo_home = cargo_home.display(),
                cargo_home_bin = cargo_home_bin.display(),
                plural = plural,
                rcfiles = rcfiles,
                rustup_home = rustup_home.display(),
            ))
        }
        #[cfg(windows)]
        Ok(format!(
            pre_install_msg_win!(),
            cargo_home = cargo_home.display(),
            cargo_home_bin = cargo_home_bin.display(),
            rustup_home = rustup_home.display(),
        ))
    } else {
        Ok(format!(
            pre_install_msg_no_modify_path!(),
            cargo_home = cargo_home.display(),
            cargo_home_bin = cargo_home_bin.display(),
            rustup_home = rustup_home.display(),
        ))
    }
}

fn current_install_opts(opts: &InstallOpts<'_>, process: &Process) -> String {
    format!(
        r"Current installation options:

- ` `default host triple: `{}`
- `   `default toolchain: `{}`
- `             `profile: `{}`
- modify PATH variable: `{}`
",
        opts.default_host_triple
            .as_ref()
            .map(TargetTriple::new)
            .unwrap_or_else(|| TargetTriple::from_host_or_build(process)),
        opts.default_toolchain
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or("stable (default)".into()),
        opts.profile,
        if !opts.no_modify_path { "yes" } else { "no" }
    )
}

fn install_bins(process: &Process) -> Result<()> {
    let bin_path = process.cargo_home()?.join("bin");
    let this_exe_path = utils::current_exe()?;
    let rustup_path = bin_path.join(format!("rustup{EXE_SUFFIX}"));

    utils::ensure_dir_exists("bin", &bin_path, &|_: Notification<'_>| {})?;
    // NB: Even on Linux we can't just copy the new binary over the (running)
    // old binary; we must unlink it first.
    if rustup_path.exists() {
        utils::remove_file("rustup-bin", &rustup_path)?;
    }
    utils::copy_file(&this_exe_path, &rustup_path)?;
    utils::make_executable(&rustup_path)?;
    install_proxies(process)
}

pub(crate) fn install_proxies(process: &Process) -> Result<()> {
    install_proxies_with_opts(process, process.var_os("RUSTUP_HARDLINK_PROXIES").is_some())
}

fn install_proxies_with_opts(process: &Process, force_hard_links: bool) -> Result<()> {
    let bin_path = process.cargo_home()?.join("bin");
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
            return install_proxies_with_opts(process, true);
        }
    }

    Ok(())
}

fn check_proxy_sanity(process: &Process, components: &[&str], desc: &ToolchainDesc) -> Result<()> {
    let bin_path = process.cargo_home()?.join("bin");

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

async fn maybe_install_rust(
    current_dir: PathBuf,
    quiet: bool,
    opts: InstallOpts<'_>,
    process: &Process,
) -> Result<()> {
    install_bins(process)?;

    #[cfg(unix)]
    unix::do_write_env_files(process)?;

    if !opts.no_modify_path {
        do_add_to_path(process)?;
    }

    // If RUSTUP_HOME is not set, make sure it exists
    if process.var_os("RUSTUP_HOME").is_none() {
        let home = process
            .home_dir()
            .map(|p| p.join(".rustup"))
            .ok_or_else(|| anyhow::anyhow!("could not find home dir to put .rustup in"))?;

        fs::create_dir_all(home).context("unable to create ~/.rustup")?;
    }

    let mut cfg = common::set_globals(current_dir, quiet, process)?;

    let (components, targets) = (opts.components, opts.targets);
    let toolchain = opts.install(&mut cfg)?;
    if let Some(ref desc) = toolchain {
        let status = if Toolchain::exists(&cfg, &desc.into())? {
            warn!("Updating existing toolchain, profile choice will be ignored");
            // If we have a partial install we might not be able to read content here. We could:
            // - fail and folk have to delete the partially present toolchain to recover
            // - silently ignore it (and provide inconsistent metadata for reporting the install/update change)
            // - delete the partial install and start over
            // For now, we error.
            let mut toolchain = DistributableToolchain::new(&cfg, desc.clone())?;
            toolchain
                .update(components, targets, cfg.get_profile()?)
                .await?
        } else {
            DistributableToolchain::install(
                &cfg,
                desc,
                components,
                targets,
                cfg.get_profile()?,
                true,
            )
            .await?
            .0
        };

        check_proxy_sanity(process, components, desc)?;

        cfg.set_default(Some(&desc.into()))?;
        writeln!(process.stdout().lock())?;
        common::show_channel_update(&cfg, PackageUpdate::Toolchain(desc.clone()), Ok(status))?;
    }
    Ok(())
}

pub(crate) fn uninstall(no_prompt: bool, process: &Process) -> Result<utils::ExitCode> {
    if NEVER_SELF_UPDATE {
        error!("self-uninstall is disabled for this build of rustup");
        error!("you should probably use your system package manager to uninstall rustup");
        return Ok(utils::ExitCode(1));
    }

    let cargo_home = process.cargo_home()?;

    if !cargo_home.join(format!("bin/rustup{EXE_SUFFIX}")).exists() {
        return Err(CLIError::NotSelfInstalled { p: cargo_home }.into());
    }

    if !no_prompt {
        writeln!(process.stdout().lock())?;
        let msg = format!(
            pre_uninstall_msg!(),
            cargo_home = canonical_cargo_home(process)?
        );
        md(&mut process.stdout().terminal(process), msg);
        if !common::confirm("\nContinue? (y/N)", false, process)? {
            info!("aborting uninstallation");
            return Ok(utils::ExitCode(0));
        }
    }

    info!("removing rustup home");

    // Delete RUSTUP_HOME
    let rustup_dir = home::rustup_home()?;
    if rustup_dir.exists() {
        utils::remove_dir("rustup_home", &rustup_dir, &|_: Notification<'_>| {})?;
    }

    info!("removing cargo home");

    // Remove CARGO_HOME/bin from PATH
    do_remove_from_path(process)?;

    // Delete everything in CARGO_HOME *except* the rustup bin

    // First everything except the bin directory
    let diriter = fs::read_dir(&cargo_home).map_err(|e| CLIError::ReadDirError {
        p: cargo_home.clone(),
        source: e,
    })?;
    for dirent in diriter {
        let dirent = dirent.map_err(|e| CLIError::ReadDirError {
            p: cargo_home.clone(),
            source: e,
        })?;
        if dirent.file_name().to_str() != Some("bin") {
            if dirent.path().is_dir() {
                utils::remove_dir("cargo_home", &dirent.path(), &|_: Notification<'_>| {})?;
            } else {
                utils::remove_file("cargo_home", &dirent.path())?;
            }
        }
    }

    // Then everything in bin except rustup and tools. These can't be unlinked
    // until this process exits (on windows).
    let tools = TOOLS
        .iter()
        .chain(DUP_TOOLS.iter())
        .map(|t| format!("{t}{EXE_SUFFIX}"));
    let tools: Vec<_> = tools.chain(vec![format!("rustup{EXE_SUFFIX}")]).collect();
    let bin_dir = cargo_home.join("bin");
    let diriter = fs::read_dir(&bin_dir).map_err(|e| CLIError::ReadDirError {
        p: bin_dir.clone(),
        source: e,
    })?;
    for dirent in diriter {
        let dirent = dirent.map_err(|e| CLIError::ReadDirError {
            p: bin_dir.clone(),
            source: e,
        })?;
        let name = dirent.file_name();
        let file_is_tool = name.to_str().map(|n| tools.iter().any(|t| *t == n));
        if file_is_tool == Some(false) {
            if dirent.path().is_dir() {
                utils::remove_dir("cargo_home", &dirent.path(), &|_: Notification<'_>| {})?;
            } else {
                utils::remove_file("cargo_home", &dirent.path())?;
            }
        }
    }

    info!("removing rustup binaries");

    // Delete rustup. This is tricky because this is *probably*
    // the running executable and on Windows can't be unlinked until
    // the process exits.
    delete_rustup_and_cargo_home(process)?;

    info!("rustup is uninstalled");

    Ok(utils::ExitCode(0))
}

/// Self update downloads rustup-init to `CARGO_HOME`/bin/rustup-init
/// and runs it.
///
/// It does a few things to accommodate self-delete problems on windows:
///
/// rustup-init is run in two stages, first with `--self-upgrade`,
/// which displays update messages and asks for confirmations, etc;
/// then with `--self-replace`, which replaces the rustup binary and
/// hardlinks. The last step is done without waiting for confirmation
/// on windows so that the running exe can be deleted.
///
/// Because it's again difficult for rustup-init to delete itself
/// (and on windows this process will not be running to do it),
/// rustup-init is stored in `CARGO_HOME`/bin, and then deleted next
/// time rustup runs.
pub(crate) async fn update(cfg: &Cfg<'_>) -> Result<utils::ExitCode> {
    common::warn_if_host_is_emulated(cfg.process);

    use common::SelfUpdatePermission::*;
    let update_permitted = if NEVER_SELF_UPDATE {
        HardFail
    } else {
        common::self_update_permitted(true)?
    };
    match update_permitted {
        HardFail => {
            // TODO: Detect which package manager and be more useful.
            error!("self-update is disabled for this build of rustup");
            error!("you should probably use your system package manager to update rustup");
            return Ok(utils::ExitCode(1));
        }
        #[cfg(not(windows))]
        Skip => {
            info!("Skipping self-update at this time");
            return Ok(utils::ExitCode(0));
        }
        Permit => {}
    }

    match prepare_update(cfg.process).await? {
        Some(setup_path) => {
            let Some(version) = get_and_parse_new_rustup_version(&setup_path) else {
                error!("failed to get rustup version");
                return Ok(utils::ExitCode(1));
            };

            let _ = common::show_channel_update(
                cfg,
                PackageUpdate::Rustup,
                Ok(UpdateStatus::Updated(version)),
            );
            return run_update(&setup_path);
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

    Ok(utils::ExitCode(0))
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

pub(crate) async fn prepare_update(process: &Process) -> Result<Option<PathBuf>> {
    let cargo_home = process.cargo_home()?;
    let rustup_path = cargo_home.join(format!("bin{MAIN_SEPARATOR}rustup{EXE_SUFFIX}"));
    let setup_path = cargo_home.join(format!("bin{MAIN_SEPARATOR}rustup-init{EXE_SUFFIX}"));

    if !rustup_path.exists() {
        return Err(CLIError::NotSelfInstalled { p: cargo_home }.into());
    }

    if setup_path.exists() {
        utils::remove_file("setup", &setup_path)?;
    }

    // Get build triple
    let triple = dist::TargetTriple::from_build();

    // For windows x86 builds seem slow when used with windows defender.
    // The website defaulted to i686-windows-gnu builds for a long time.
    // This ensures that we update to a version that's appropriate for users
    // and also works around if the website messed up the detection.
    // If someone really wants to use another version, they still can enforce
    // that using the environment variable RUSTUP_OVERRIDE_HOST_TRIPLE.
    #[cfg(windows)]
    let triple = dist::TargetTriple::from_host(process).unwrap_or(triple);

    // Get update root.
    let update_root = update_root(process);

    // Get current version
    let current_version = env!("CARGO_PKG_VERSION");

    // Get available version
    info!("checking for self-update (current version: {current_version})");
    let available_version = if let Some(ver) = non_empty_env_var("RUSTUP_VERSION", process)? {
        info!("`RUSTUP_VERSION` has been set to `{ver}`");
        ver
    } else {
        get_available_rustup_version(process).await?
    };

    // If up-to-date
    if available_version == current_version {
        return Ok(None);
    }

    // Get download URL
    let url = format!("{update_root}/archive/{available_version}/{triple}/rustup-init{EXE_SUFFIX}");

    // Get download path
    let download_url = utils::parse_url(&url)?;

    // Download new version
    info!("downloading self-update (new version: {available_version})");
    download_file(&download_url, &setup_path, None, &|_| (), process).await?;

    // Mark as executable
    utils::make_executable(&setup_path)?;

    Ok(Some(setup_path))
}

async fn get_available_rustup_version(process: &Process) -> Result<String> {
    let update_root = update_root(process);
    let tempdir = tempfile::Builder::new()
        .prefix("rustup-update")
        .tempdir()
        .context("error creating temp directory")?;

    // Parse the release file.
    let release_file_url = format!("{update_root}/release-stable.toml");
    let release_file_url = utils::parse_url(&release_file_url)?;
    let release_file = tempdir.path().join("release-stable.toml");
    download_file(&release_file_url, &release_file, None, &|_| (), process).await?;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RustupUpdateAvailable {
    True,
    False,
}

pub(crate) async fn check_rustup_update(process: &Process) -> Result<RustupUpdateAvailable> {
    let mut update_available = RustupUpdateAvailable::False;

    let mut t = process.stdout().terminal(process);
    // Get current rustup version
    let current_version = env!("CARGO_PKG_VERSION");

    // Get available rustup version
    let available_version = get_available_rustup_version(process).await?;

    let _ = t.attr(terminalsource::Attr::Bold);
    write!(t.lock(), "rustup - ")?;

    if current_version != available_version {
        update_available = RustupUpdateAvailable::True;

        let _ = t.fg(terminalsource::Color::Yellow);
        write!(t.lock(), "Update available")?;
        let _ = t.reset();
        writeln!(t.lock(), " : {current_version} -> {available_version}")?;
    } else {
        let _ = t.fg(terminalsource::Color::Green);
        write!(t.lock(), "Up to date")?;
        let _ = t.reset();
        writeln!(t.lock(), " : {current_version}")?;
    }

    Ok(update_available)
}

#[tracing::instrument(level = "trace")]
pub(crate) fn cleanup_self_updater(process: &Process) -> Result<()> {
    let cargo_home = process.cargo_home()?;
    let setup = cargo_home.join(format!("bin/rustup-init{EXE_SUFFIX}"));

    if setup.exists() {
        utils::remove_file("setup", &setup)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::cli::common;
    use crate::cli::self_update::InstallOpts;
    use crate::dist::{PartialToolchainDesc, Profile};
    use crate::test::{Env, test_dir, with_rustup_home};
    use crate::{for_host, process::TestProcess};

    #[test]
    fn default_toolchain_is_stable() {
        with_rustup_home(|home| {
            let mut vars = HashMap::new();
            home.apply(&mut vars);
            let tp = TestProcess::with_vars(vars);
            let mut cfg =
                common::set_globals(tp.process.current_dir().unwrap(), false, &tp.process).unwrap();

            let opts = InstallOpts {
                default_host_triple: None,
                default_toolchain: None,   // No toolchain specified
                profile: Profile::Default, // default profile
                no_modify_path: false,
                components: &[],
                targets: &[],
                no_update_toolchain: false,
            };

            assert_eq!(
                "stable"
                    .parse::<PartialToolchainDesc>()
                    .unwrap()
                    .resolve(&cfg.get_default_host_triple().unwrap())
                    .unwrap(),
                opts.install(&mut cfg)
                    .unwrap() // result
                    .unwrap() // option
            );
            assert_eq!(
                for_host!(
                    r"info: profile set to 'default'
info: default host triple is {0}
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
        let mut vars = HashMap::new();
        vars.env("CARGO_HOME", cargo_home.to_string_lossy().to_string());
        let tp = TestProcess::with_vars(vars);
        super::install_bins(&tp.process).unwrap();
        assert!(cargo_home.exists());
    }
}
