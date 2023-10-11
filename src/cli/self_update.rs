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

#[cfg(unix)]
mod shell;
#[cfg(feature = "test")]
pub(crate) mod test;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
mod os {
    #[cfg(unix)]
    pub(crate) use super::unix::*;
    #[cfg(windows)]
    pub(crate) use super::windows::*;
}

use std::borrow::Cow;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf, MAIN_SEPARATOR};
use std::process::Command;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use cfg_if::cfg_if;
use same_file::Handle;

use crate::currentprocess::terminalsource;
use crate::{
    cli::{
        common::{self, ignorable_error, report_error, Confirm, PackageUpdate},
        errors::*,
        markdown::md,
    },
    currentprocess::{filesource::StdoutSource, varsource::VarSource},
    dist::dist::{self, PartialToolchainDesc, Profile, TargetTriple, ToolchainDesc},
    install::UpdateStatus,
    process,
    toolchain::{
        distributable::DistributableToolchain,
        names::{MaybeOfficialToolchainName, ResolvableToolchainName, ToolchainName},
        toolchain::Toolchain,
    },
    utils::{utils, Notification},
    Cfg, DUP_TOOLS, TOOLS,
};

use os::*;
pub(crate) use os::{delete_rustup_and_cargo_home, run_update, self_replace};
#[cfg(windows)]
pub use windows::complete_windows_uninstall;

pub(crate) struct InstallOpts<'a> {
    pub default_host_triple: Option<String>,
    pub default_toolchain: Option<MaybeOfficialToolchainName>,
    pub profile: String,
    pub no_modify_path: bool,
    pub no_update_toolchain: bool,
    pub components: &'a [&'a str],
    pub targets: &'a [&'a str],
}

#[cfg(feature = "no-self-update")]
pub(crate) const NEVER_SELF_UPDATE: bool = true;
#[cfg(not(feature = "no-self-update"))]
pub(crate) const NEVER_SELF_UPDATE: bool = false;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelfUpdateMode {
    Enable,
    Disable,
    CheckOnly,
}

impl SelfUpdateMode {
    pub(crate) fn modes() -> &'static [&'static str] {
        &["enable", "disable", "check-only"]
    }

    pub(crate) fn default_mode() -> &'static str {
        "enable"
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
                valid_self_update_modes(),
            ))),
        }
    }
}

impl ToString for SelfUpdateMode {
    fn to_string(&self) -> String {
        match self {
            SelfUpdateMode::Enable => "enable",
            SelfUpdateMode::Disable => "disable",
            SelfUpdateMode::CheckOnly => "check-only",
        }
        .into()
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
            "This path will then be added to your `PATH` environment variable by
modifying the `HKEY_CURRENT_USER/Environment/PATH` registry key."
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

This is usually done by running one of the following:
    source "{cargo_home}/env"        # For bash/zsh
    . "{cargo_home}/env"             # For ash/dash/pdksh (note the leading DOT)
    source "{cargo_home}/env.fish"   # For fish
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

#[cfg(windows)]
static MSVC_MESSAGE: &str = r#"# Rust Visual C++ prerequisites

Rust requires the Microsoft C++ build tools for Visual Studio 2013 or
later, but they don't seem to be installed.

"#;

#[cfg(windows)]
static MSVC_MANUAL_INSTALL_MESSAGE: &str = r#"
You can acquire the build tools by installing Microsoft Visual Studio.

    https://visualstudio.microsoft.com/downloads/

Check the box for "Desktop development with C++" which will ensure that the
needed components are installed. If your locale language is not English,
then additionally check the box for English under Language packs.

For more details see:

    https://rust-lang.github.io/rustup/installation/windows-msvc.html

_Install the C++ build tools before proceeding_.

If you will be targeting the GNU ABI or otherwise know what you are
doing then it is fine to continue installation without the build
tools, but otherwise, install the C++ build tools before proceeding.
"#;

#[cfg(windows)]
static MSVC_AUTO_INSTALL_MESSAGE: &str = r#"# Rust Visual C++ prerequisites

Rust requires a linker and Windows API libraries but they don't seem to be available.

These components can be acquired through a Visual Studio installer.

"#;

static UPDATE_ROOT: &str = "https://static.rust-lang.org/rustup";

/// `CARGO_HOME` suitable for display, possibly with $HOME
/// substituted for the directory prefix
fn canonical_cargo_home() -> Result<Cow<'static, str>> {
    let path = utils::cargo_home()?;

    let default_cargo_home = utils::home_dir()
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
pub(crate) fn install(
    no_prompt: bool,
    verbose: bool,
    quiet: bool,
    mut opts: InstallOpts<'_>,
) -> Result<utils::ExitCode> {
    if !process()
        .var_os("RUSTUP_INIT_SKIP_EXISTENCE_CHECKS")
        .map_or(false, |s| s == "yes")
    {
        do_pre_install_sanity_checks(no_prompt)?;
    }

    do_pre_install_options_sanity_checks(&opts)?;

    if !process()
        .var_os("RUSTUP_INIT_SKIP_EXISTENCE_CHECKS")
        .map_or(false, |s| s == "yes")
    {
        check_existence_of_rustc_or_cargo_in_path(no_prompt)?;
    }

    #[cfg(unix)]
    do_anti_sudo_check(no_prompt)?;

    let mut term = process().stdout().terminal();

    #[cfg(windows)]
    if let Some(plan) = do_msvc_check(&opts) {
        if no_prompt {
            warn!("installing msvc toolchain without its prerequisites");
        } else if !quiet && plan == VsInstallPlan::Automatic {
            md(&mut term, MSVC_AUTO_INSTALL_MESSAGE);
            match windows::choose_vs_install()? {
                Some(VsInstallPlan::Automatic) => {
                    match try_install_msvc(&opts) {
                        Err(e) => {
                            // Make sure the console doesn't exit before the user can
                            // see the error and give the option to continue anyway.
                            report_error(&e);
                            if !common::question_bool("\nContinue?", false)? {
                                info!("aborting installation");
                                return Ok(utils::ExitCode(0));
                            }
                        }
                        Ok(ContinueInstall::No) => {
                            ensure_prompt()?;
                            return Ok(utils::ExitCode(0));
                        }
                        _ => {}
                    }
                }
                Some(VsInstallPlan::Manual) => {
                    md(&mut term, MSVC_MANUAL_INSTALL_MESSAGE);
                    if !common::question_bool("\nContinue?", false)? {
                        info!("aborting installation");
                        return Ok(utils::ExitCode(0));
                    }
                }
                None => {}
            }
        } else {
            md(&mut term, MSVC_MESSAGE);
            md(&mut term, MSVC_MANUAL_INSTALL_MESSAGE);
            if !common::question_bool("\nContinue?", false)? {
                info!("aborting installation");
                return Ok(utils::ExitCode(0));
            }
        }
    }

    if !no_prompt {
        let msg = pre_install_msg(opts.no_modify_path)?;

        md(&mut term, msg);

        loop {
            md(&mut term, current_install_opts(&opts));
            match common::confirm_advanced()? {
                Confirm::No => {
                    info!("aborting installation");
                    return Ok(utils::ExitCode(0));
                }
                Confirm::Yes => {
                    break;
                }
                Confirm::Advanced => {
                    opts = customize_install(opts)?;
                }
            }
        }
    }

    let install_res: Result<utils::ExitCode> = (|| {
        install_bins()?;

        #[cfg(unix)]
        do_write_env_files()?;

        if !opts.no_modify_path {
            do_add_to_programs()?;
            do_add_to_path()?;
        }
        utils::create_rustup_home()?;
        maybe_install_rust(
            opts.default_toolchain,
            &opts.profile,
            opts.default_host_triple.as_deref(),
            !opts.no_update_toolchain,
            opts.components,
            opts.targets,
            verbose,
            quiet,
        )?;

        Ok(utils::ExitCode(0))
    })();

    if let Err(e) = install_res {
        report_error(&e);

        // On windows, where installation happens in a console
        // that may have opened just for this purpose, give
        // the user an opportunity to see the error before the
        // window closes.
        #[cfg(windows)]
        if !no_prompt {
            ensure_prompt()?;
        }

        return Ok(utils::ExitCode(1));
    }

    let cargo_home = canonical_cargo_home()?;
    #[cfg(windows)]
    let cargo_home = cargo_home.replace('\\', r"\\");
    #[cfg(windows)]
    let msg = if opts.no_modify_path {
        format!(
            post_install_msg_win_no_modify_path!(),
            cargo_home = cargo_home
        )
    } else {
        format!(post_install_msg_win!(), cargo_home = cargo_home)
    };
    #[cfg(not(windows))]
    let msg = if opts.no_modify_path {
        format!(
            post_install_msg_unix_no_modify_path!(),
            cargo_home = cargo_home
        )
    } else {
        format!(post_install_msg_unix!(), cargo_home = cargo_home)
    };
    md(&mut term, msg);

    #[cfg(windows)]
    if !no_prompt {
        // On windows, where installation happens in a console
        // that may have opened just for this purpose, require
        // the user to press a key to continue.
        ensure_prompt()?;
    }

    Ok(utils::ExitCode(0))
}

fn rustc_or_cargo_exists_in_path() -> Result<()> {
    // Ignore rustc and cargo if present in $HOME/.cargo/bin or a few other directories
    #[allow(clippy::ptr_arg)]
    fn ignore_paths(path: &PathBuf) -> bool {
        !path
            .components()
            .any(|c| c == Component::Normal(".cargo".as_ref()))
    }

    if let Some(paths) = process().var_os("PATH") {
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

fn check_existence_of_rustc_or_cargo_in_path(no_prompt: bool) -> Result<()> {
    // Only the test runner should set this
    let skip_check = process().var_os("RUSTUP_INIT_SKIP_PATH_CHECK");

    // Skip this if the environment variable is set
    if skip_check == Some("yes".into()) {
        return Ok(());
    }

    if let Err(path) = rustc_or_cargo_exists_in_path() {
        warn!("it looks like you have an existing installation of Rust at:");
        warn!("{}", path);
        warn!("It is recommended that rustup be the primary Rust installation.");
        warn!("Otherwise you may have confusion unless you are careful with your PATH");
        warn!("If you are sure that you want both rustup and your already installed Rust");
        warn!("then please reply `y' or `yes' or set RUSTUP_INIT_SKIP_PATH_CHECK to yes");
        warn!("or pass `-y' to ignore all ignorable checks.");
        ignorable_error("cannot install while Rust is installed", no_prompt)?;
    }
    Ok(())
}

fn do_pre_install_sanity_checks(no_prompt: bool) -> Result<()> {
    let rustc_manifest_path = PathBuf::from("/usr/local/lib/rustlib/manifest-rustc");
    let uninstaller_path = PathBuf::from("/usr/local/lib/rustlib/uninstall.sh");
    let rustup_sh_path = utils::home_dir().unwrap().join(".rustup");
    let rustup_sh_version_path = rustup_sh_path.join("rustup-version");

    let rustc_exists = rustc_manifest_path.exists() && uninstaller_path.exists();
    let rustup_sh_exists = rustup_sh_version_path.exists();

    if rustc_exists {
        warn!("it looks like you have an existing installation of Rust");
        warn!("rustup cannot be installed alongside Rust. Please uninstall first");
        warn!(
            "run `{}` as root to uninstall Rust",
            uninstaller_path.display()
        );
        ignorable_error("cannot install while Rust is installed", no_prompt)?;
    }

    if rustup_sh_exists {
        warn!("it looks like you have existing rustup.sh metadata");
        warn!("rustup cannot be installed while rustup.sh metadata exists");
        warn!("delete `{}` to remove rustup.sh", rustup_sh_path.display());
        warn!("or, if you already have rustup installed, you can run");
        warn!("`rustup self update` and `rustup toolchain list` to upgrade");
        warn!("your directory structure");
        ignorable_error("cannot install while rustup.sh is installed", no_prompt)?;
    }

    Ok(())
}

fn do_pre_install_options_sanity_checks(opts: &InstallOpts<'_>) -> Result<()> {
    // Verify that the installation options are vaguely sane
    (|| {
        let host_triple = opts
            .default_host_triple
            .as_ref()
            .map(|s| dist::TargetTriple::new(s))
            .unwrap_or_else(TargetTriple::from_host_or_build);
        let partial_channel = match &opts.default_toolchain {
            None | Some(MaybeOfficialToolchainName::None) => {
                ResolvableToolchainName::try_from("stable")?
            }
            Some(MaybeOfficialToolchainName::Some(s)) => s.into(),
        };
        let resolved = partial_channel.resolve(&host_triple)?;
        debug!("Successfully resolved installation toolchain as: {resolved}");
        Ok(())
    })()
    .map_err(|e: Box<dyn std::error::Error>| {
        anyhow!(
            "Pre-checks for host and toolchain failed: {}\n\
             If you are unsure of suitable values, the 'stable' toolchain is the default.\n\
             Valid host triples look something like: {}",
            e,
            TargetTriple::from_host_or_build()
        )
    })?;
    Ok(())
}

fn pre_install_msg(no_modify_path: bool) -> Result<String> {
    let cargo_home = utils::cargo_home()?;
    let cargo_home_bin = cargo_home.join("bin");
    let rustup_home = home::rustup_home()?;

    if !no_modify_path {
        // Brittle code warning: some duplication in unix::do_add_to_path
        #[cfg(not(windows))]
        {
            let rcfiles = shell::get_available_shells()
                .flat_map(|sh| sh.update_rcs().into_iter())
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

fn current_install_opts(opts: &InstallOpts<'_>) -> String {
    format!(
        r"Current installation options:

- ` `default host triple: `{}`
- `   `default toolchain: `{}`
- `             `profile: `{}`
- modify PATH variable: `{}`
",
        opts.default_host_triple
            .as_ref()
            .map(|s| TargetTriple::new(s))
            .unwrap_or_else(TargetTriple::from_host_or_build),
        opts.default_toolchain
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or("stable (default)".into()),
        opts.profile,
        if !opts.no_modify_path { "yes" } else { "no" }
    )
}

// Interactive editing of the install options
fn customize_install(mut opts: InstallOpts<'_>) -> Result<InstallOpts<'_>> {
    writeln!(
        process().stdout().lock(),
        "I'm going to ask you the value of each of these installation options.\n\
         You may simply press the Enter key to leave unchanged."
    )?;

    writeln!(process().stdout().lock())?;

    opts.default_host_triple = Some(common::question_str(
        "Default host triple?",
        &opts
            .default_host_triple
            .unwrap_or_else(|| TargetTriple::from_host_or_build().to_string()),
    )?);

    opts.default_toolchain = Some(MaybeOfficialToolchainName::try_from(common::question_str(
        "Default toolchain? (stable/beta/nightly/none)",
        &opts
            .default_toolchain
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or("stable".into()),
    )?)?);

    opts.profile = common::question_str(
        &format!(
            "Profile (which tools and data to install)? ({})",
            Profile::names().join("/")
        ),
        &opts.profile,
    )?;

    opts.no_modify_path = !common::question_bool("Modify PATH variable?", !opts.no_modify_path)?;

    Ok(opts)
}

fn install_bins() -> Result<()> {
    let bin_path = utils::cargo_home()?.join("bin");
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
    install_proxies()
}

pub(crate) fn install_proxies() -> Result<()> {
    let bin_path = utils::cargo_home()?.join("bin");
    let rustup_path = bin_path.join(format!("rustup{EXE_SUFFIX}"));

    let rustup = Handle::from_path(&rustup_path)?;

    let mut tool_handles = Vec::new();
    let mut link_afterwards = Vec::new();

    // Try to hardlink all the Rust exes to the rustup exe. Some systems,
    // like Android, does not support hardlinks, so we fallback to symlinks.
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
    // overwrite all the previous hard links with new ones.
    for tool in TOOLS {
        let tool_path = bin_path.join(&format!("{tool}{EXE_SUFFIX}"));
        if let Ok(handle) = Handle::from_path(&tool_path) {
            tool_handles.push(handle);
            if rustup == *tool_handles.last().unwrap() {
                continue;
            }
        }
        link_afterwards.push(tool_path);
    }

    for tool in DUP_TOOLS {
        let tool_path = bin_path.join(&format!("{tool}{EXE_SUFFIX}"));
        if let Ok(handle) = Handle::from_path(&tool_path) {
            // Like above, don't clobber anything that's already hardlinked to
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
                warn!("tool `{}` is already installed, remove it from `{}`, then run `rustup update` \
                       to have rustup manage this tool.",
                      tool, bin_path.display());
                continue;
            }
        }
        utils::hard_or_symlink_file(&rustup_path, &tool_path)?;
    }

    drop(tool_handles);
    for path in link_afterwards {
        utils::hard_or_symlink_file(&rustup_path, &path)?;
    }

    Ok(())
}

fn maybe_install_rust(
    toolchain: Option<MaybeOfficialToolchainName>,
    profile_str: &str,
    default_host_triple: Option<&str>,
    update_existing_toolchain: bool,
    components: &[&str],
    targets: &[&str],
    verbose: bool,
    quiet: bool,
) -> Result<()> {
    let mut cfg = common::set_globals(verbose, quiet)?;

    let toolchain = _install_selection(
        &mut cfg,
        toolchain,
        profile_str,
        default_host_triple,
        update_existing_toolchain,
        components,
        targets,
    )?;
    if let Some(ref desc) = toolchain {
        let status = if Toolchain::exists(&cfg, &desc.into())? {
            warn!("Updating existing toolchain, profile choice will be ignored");
            // If we have a partial install we might not be able to read content here. We could:
            // - fail and folk have to delete the partially present toolchain to recover
            // - silently ignore it (and provide inconsistent metadata for reporting the install/update change)
            // - delete the partial install and start over
            // For now, we error.
            let mut toolchain = DistributableToolchain::new(&cfg, desc.clone())?;
            toolchain.update(components, targets, cfg.get_profile()?)?
        } else {
            DistributableToolchain::install(
                &cfg,
                desc,
                components,
                targets,
                cfg.get_profile()?,
                true,
            )?
            .0
        };

        cfg.set_default(Some(&desc.into()))?;
        writeln!(process().stdout().lock())?;
        common::show_channel_update(&cfg, PackageUpdate::Toolchain(desc.clone()), Ok(status))?;
    }
    Ok(())
}

fn _install_selection(
    cfg: &mut Cfg,
    toolchain_opt: Option<MaybeOfficialToolchainName>,
    profile_str: &str,
    default_host_triple: Option<&str>,
    update_existing_toolchain: bool,
    components: &[&str],
    targets: &[&str],
) -> Result<Option<ToolchainDesc>> {
    cfg.set_profile(profile_str)?;

    if let Some(default_host_triple) = default_host_triple {
        // Set host triple now as it will affect resolution of toolchain_str
        info!("setting default host triple to {}", default_host_triple);
        cfg.set_default_host_triple(default_host_triple)?;
    } else {
        info!("default host triple is {}", cfg.get_default_host_triple()?);
    }

    let user_specified_something = toolchain_opt.is_some()
        || !targets.is_empty()
        || !components.is_empty()
        || update_existing_toolchain;

    // If the user specified they want no toolchain, we skip this, otherwise
    // if they specify something directly, or we have no default, then we install
    // a toolchain (updating if it's already present) and then if neither of
    // those are true, we have a user who doesn't mind, and already has an
    // install, so we leave their setup alone.
    Ok(
        if matches!(toolchain_opt, Some(MaybeOfficialToolchainName::None)) {
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
            writeln!(process().stdout().lock())?;
            None
        } else if user_specified_something
            || (update_existing_toolchain && cfg.find_default()?.is_none())
        {
            match toolchain_opt {
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
            }
        } else {
            info!("updating existing rustup installation - leaving toolchains alone");
            writeln!(process().stdout().lock())?;
            None
        },
    )
}

pub(crate) fn uninstall(no_prompt: bool) -> Result<utils::ExitCode> {
    if NEVER_SELF_UPDATE {
        err!("self-uninstall is disabled for this build of rustup");
        err!("you should probably use your system package manager to uninstall rustup");
        return Ok(utils::ExitCode(1));
    }

    let cargo_home = utils::cargo_home()?;

    if !cargo_home.join(format!("bin/rustup{EXE_SUFFIX}")).exists() {
        return Err(CLIError::NotSelfInstalled { p: cargo_home }.into());
    }

    if !no_prompt {
        writeln!(process().stdout().lock())?;
        let msg = format!(pre_uninstall_msg!(), cargo_home = canonical_cargo_home()?);
        md(&mut process().stdout().terminal(), msg);
        if !common::confirm("\nContinue? (y/N)", false)? {
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
    do_remove_from_path()?;
    do_remove_from_programs()?;

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
    delete_rustup_and_cargo_home()?;

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
pub(crate) fn update(cfg: &Cfg) -> Result<utils::ExitCode> {
    use common::SelfUpdatePermission::*;
    let update_permitted = if NEVER_SELF_UPDATE {
        HardFail
    } else {
        common::self_update_permitted(true)?
    };
    match update_permitted {
        HardFail => {
            // TODO: Detect which package manager and be more useful.
            err!("self-update is disabled for this build of rustup");
            err!("you should probably use your system package manager to update rustup");
            return Ok(utils::ExitCode(1));
        }
        Skip => {
            info!("Skipping self-update at this time");
            return Ok(utils::ExitCode(0));
        }
        Permit => {}
    }

    match prepare_update()? {
        Some(setup_path) => {
            let version = match get_new_rustup_version(&setup_path) {
                Some(new_version) => parse_new_rustup_version(new_version),
                None => {
                    err!("failed to get rustup version");
                    return Ok(utils::ExitCode(1));
                }
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
            install_proxies()?
        }
    }

    Ok(utils::ExitCode(0))
}

fn get_new_rustup_version(path: &Path) -> Option<String> {
    match Command::new(path).arg("--version").output() {
        Err(_) => None,
        Ok(output) => match String::from_utf8(output.stdout) {
            Ok(version) => Some(version),
            Err(_) => None,
        },
    }
}

fn parse_new_rustup_version(version: String) -> String {
    use lazy_static::lazy_static;
    use regex::Regex;

    lazy_static! {
        static ref RE: Regex = Regex::new(r"\d+.\d+.\d+[0-9a-zA-Z-]*").unwrap();
    }

    let capture = RE.captures(&version);
    let matched_version = match capture {
        Some(cap) => cap.get(0).unwrap().as_str(),
        None => "(unknown)",
    };
    String::from(matched_version)
}

pub(crate) fn prepare_update() -> Result<Option<PathBuf>> {
    let cargo_home = utils::cargo_home()?;
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
    // This ensures that we update to a version thats appropriate for users
    // and also works around if the website messed up the detection.
    // If someone really wants to use another version, they still can enforce
    // that using the environment variable RUSTUP_OVERRIDE_HOST_TRIPLE.
    #[cfg(windows)]
    let triple = dist::TargetTriple::from_host().unwrap_or(triple);

    // Get update root.
    let update_root = process()
        .var("RUSTUP_UPDATE_ROOT")
        .unwrap_or_else(|_| String::from(UPDATE_ROOT));

    // Get current version
    let current_version = env!("CARGO_PKG_VERSION");

    // Get available version
    info!("checking for self-update");
    let available_version = get_available_rustup_version()?;

    // If up-to-date
    if available_version == current_version {
        return Ok(None);
    }

    // Get download URL
    let url = format!("{update_root}/archive/{available_version}/{triple}/rustup-init{EXE_SUFFIX}");

    // Get download path
    let download_url = utils::parse_url(&url)?;

    // Download new version
    info!("downloading self-update");
    utils::download_file(&download_url, &setup_path, None, &|_| ())?;

    // Mark as executable
    utils::make_executable(&setup_path)?;

    Ok(Some(setup_path))
}

pub(crate) fn get_available_rustup_version() -> Result<String> {
    let update_root = process()
        .var("RUSTUP_UPDATE_ROOT")
        .unwrap_or_else(|_| String::from(UPDATE_ROOT));
    let tempdir = tempfile::Builder::new()
        .prefix("rustup-update")
        .tempdir()
        .context("error creating temp directory")?;

    // Parse the release file.
    let release_file_url = format!("{update_root}/release-stable.toml");
    let release_file_url = utils::parse_url(&release_file_url)?;
    let release_file = tempdir.path().join("release-stable.toml");
    utils::download_file(&release_file_url, &release_file, None, &|_| ())?;
    let release_toml_str = utils::read_file("rustup release", &release_file)?;
    let release_toml: toml::Value =
        toml::from_str(&release_toml_str).context("unable to parse rustup release file")?;

    // Check the release file schema.
    let schema = release_toml
        .get("schema-version")
        .ok_or_else(|| anyhow!("no schema key in rustup release file"))?
        .as_str()
        .ok_or_else(|| anyhow!("invalid schema key in rustup release file"))?;
    if schema != "1" {
        return Err(anyhow!(format!(
            "unknown schema version '{schema}' in rustup release file"
        )));
    }

    // Get the version.
    let available_version = release_toml
        .get("version")
        .ok_or_else(|| anyhow!("no version key in rustup release file"))?
        .as_str()
        .ok_or_else(|| anyhow!("invalid version key in rustup release file"))?;

    Ok(String::from(available_version))
}

pub(crate) fn check_rustup_update() -> Result<()> {
    let mut t = process().stdout().terminal();
    // Get current rustup version
    let current_version = env!("CARGO_PKG_VERSION");

    // Get available rustup version
    let available_version = get_available_rustup_version()?;

    let _ = t.attr(terminalsource::Attr::Bold);
    write!(t.lock(), "rustup - ")?;

    if current_version != available_version {
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

    Ok(())
}

#[cfg_attr(feature = "otel", tracing::instrument)]
pub(crate) fn cleanup_self_updater() -> Result<()> {
    let cargo_home = utils::cargo_home()?;
    let setup = cargo_home.join(format!("bin/rustup-init{EXE_SUFFIX}"));

    if setup.exists() {
        utils::remove_file("setup", &setup)?;
    }

    Ok(())
}

pub(crate) fn valid_self_update_modes() -> String {
    SelfUpdateMode::modes()
        .iter()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anyhow::Result;

    use rustup_macros::unit_test as test;

    use crate::cli::common;
    use crate::dist::dist::PartialToolchainDesc;
    use crate::test::{test_dir, with_rustup_home, Env};
    use crate::{currentprocess, for_host};

    #[test]
    fn default_toolchain_is_stable() {
        with_rustup_home(|home| {
            let mut vars = HashMap::new();
            home.apply(&mut vars);
            let tp = currentprocess::TestProcess {
                vars,
                ..Default::default()
            };
            currentprocess::with(tp.clone().into(), || -> Result<()> {
                // TODO: we could pass in a custom cfg to get notification
                // callbacks rather than output to the tp sink.
                let mut cfg = common::set_globals(false, false).unwrap();
                assert_eq!(
                    "stable"
                        .parse::<PartialToolchainDesc>()
                        .unwrap()
                        .resolve(&cfg.get_default_host_triple().unwrap())
                        .unwrap(),
                    super::_install_selection(
                        &mut cfg,
                        None,      // No toolchain specified
                        "default", // default profile
                        None,
                        true,
                        &[],
                        &[],
                    )
                    .unwrap() // result
                    .unwrap() // option
                );
                Ok(())
            })?;
            assert_eq!(
                for_host!(
                    r"info: profile set to 'default'
info: default host triple is {0}
"
                ),
                &String::from_utf8(tp.get_stderr()).unwrap()
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
        let tp = currentprocess::TestProcess {
            vars,
            ..Default::default()
        };
        currentprocess::with(tp.into(), || -> Result<()> {
            super::install_bins().unwrap();
            Ok(())
        })
        .unwrap();
        assert!(cargo_home.exists());
    }
}
