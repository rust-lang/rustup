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
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::{Component, Path, PathBuf, MAIN_SEPARATOR};
use std::process::Command;

use same_file::Handle;

use super::common::{self, ignorable_error, Confirm};
use super::errors::*;
use super::markdown::md;
use super::term2;
use crate::dist::dist::{self, Profile, TargetTriple};
use crate::process;
use crate::toolchain::{DistributableToolchain, Toolchain};
use crate::utils::utils;
use crate::utils::Notification;
use crate::{Cfg, UpdateStatus};
use crate::{DUP_TOOLS, TOOLS};

mod path_update;
use path_update::PathUpdateMethod;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
mod os {
    #[cfg(unix)]
    pub use super::unix::*;
    #[cfg(windows)]
    pub use super::windows::*;
}
use os::*;
pub use os::{complete_windows_uninstall, delete_rustup_and_cargo_home, run_update, self_replace};

pub struct InstallOpts<'a> {
    pub default_host_triple: Option<String>,
    pub default_toolchain: Option<String>,
    pub profile: String,
    pub no_modify_path: bool,
    pub no_update_toolchain: bool,
    pub components: &'a [&'a str],
    pub targets: &'a [&'a str],
}

#[cfg(feature = "no-self-update")]
pub const NEVER_SELF_UPDATE: bool = true;
#[cfg(not(feature = "no-self-update"))]
pub const NEVER_SELF_UPDATE: bool = false;

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

The Cargo home directory located at:

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

macro_rules! pre_install_msg_unix {
    () => {
        pre_install_msg_template!(
            "This path will then be added to your `PATH` environment variable by
modifying the profile file{plural} located at:

{rcfiles}"
        )
    };
}

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

macro_rules! post_install_msg_unix {
    () => {
        r"# Rust is installed now. Great!

To get started you need Cargo's bin directory ({cargo_home}/bin) in your `PATH`
environment variable. Next time you log in this will be done
automatically.

To configure your current shell run `source {cargo_home}/env`
"
    };
}

macro_rules! post_install_msg_win {
    () => {
        r"# Rust is installed now. Great!

To get started you need Cargo's bin directory ({cargo_home}\\bin) in your `PATH`
environment variable. Future applications will automatically have the
correct environment, but you may need to restart your current shell.
"
    };
}

macro_rules! post_install_msg_unix_no_modify_path {
    () => {
        r"# Rust is installed now. Great!

To get started you need Cargo's bin directory ({cargo_home}/bin) in your `PATH`
environment variable.

To configure your current shell run `source {cargo_home}/env`
"
    };
}

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

static MSVC_MESSAGE: &str = r#"# Rust Visual C++ prerequisites

Rust requires the Microsoft C++ build tools for Visual Studio 2013 or
later, but they don't seem to be installed.

The easiest way to acquire the build tools is by installing Microsoft
Visual C++ Build Tools 2019 which provides just the Visual C++ build
tools:

    https://visualstudio.microsoft.com/visual-cpp-build-tools/

Please ensure the Windows 10 SDK and the English language pack components
are included when installing the Visual C++ Build Tools.

Alternately, you can install Visual Studio 2019, Visual Studio 2017,
Visual Studio 2015, or Visual Studio 2013 and during install select
the "C++ tools":

    https://visualstudio.microsoft.com/downloads/

_Install the C++ build tools before proceeding_.

If you will be targeting the GNU ABI or otherwise know what you are
doing then it is fine to continue installation without the build
tools, but otherwise, install the C++ build tools before proceeding.
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
        if cfg!(unix) {
            "$HOME/.cargo".into()
        } else {
            r"%USERPROFILE%\.cargo".into()
        }
    } else {
        path.to_string_lossy().into_owned().into()
    })
}

/// Installing is a simple matter of copying the running binary to
/// `CARGO_HOME`/bin, hard-linking the various Rust tools to it,
/// and adding `CARGO_HOME`/bin to PATH.
pub fn install(
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

    let mut term = term2::stdout();
    if !do_msvc_check(&opts)? {
        if no_prompt {
            warn!("installing msvc toolchain without its prerequisites");
        } else {
            md(&mut term, MSVC_MESSAGE);
            if !common::confirm("\nContinue? (Y/n)", true)? {
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
        if !opts.no_modify_path {
            do_add_to_path(&get_add_path_methods())?;
        }
        utils::create_rustup_home()?;
        maybe_install_rust(
            opts.default_toolchain.as_deref(),
            &opts.profile,
            opts.default_host_triple.as_deref(),
            !opts.no_update_toolchain,
            opts.components,
            opts.targets,
            verbose,
            quiet,
        )?;

        #[cfg(unix)]
        write_env()?;

        Ok(utils::ExitCode(0))
    })();

    if let Err(ref e) = install_res {
        common::report_error(e);

        // On windows, where installation happens in a console
        // that may have opened just for this purpose, give
        // the user an opportunity to see the error before the
        // window closes.
        if cfg!(windows) && !no_prompt {
            writeln!(process().stdout(),)?;
            writeln!(process().stdout(), "Press the Enter key to continue.")?;
            common::read_line()?;
        }

        return Ok(utils::ExitCode(1));
    }

    let cargo_home = canonical_cargo_home()?;
    #[cfg(windows)]
    let cargo_home = cargo_home.replace('\\', r"\\");
    let msg = match (opts.no_modify_path, cfg!(unix)) {
        (false, true) => format!(post_install_msg_unix!(), cargo_home = cargo_home),
        (false, false) => format!(post_install_msg_win!(), cargo_home = cargo_home),
        (true, true) => format!(
            post_install_msg_unix_no_modify_path!(),
            cargo_home = cargo_home
        ),
        (true, false) => format!(
            post_install_msg_win_no_modify_path!(),
            cargo_home = cargo_home
        ),
    };
    md(&mut term, msg);

    if !no_prompt {
        // On windows, where installation happens in a console
        // that may have opened just for this purpose, require
        // the user to press a key to continue.
        if cfg!(windows) {
            writeln!(process().stdout())?;
            writeln!(process().stdout(), "Press the Enter key to continue.")?;
            common::read_line()?;
        }
    }

    Ok(utils::ExitCode(0))
}

fn rustc_or_cargo_exists_in_path() -> Result<()> {
    // Ignore rustc and cargo if present in $HOME/.cargo/bin or a few other directories
    fn ignore_paths(path: &PathBuf) -> bool {
        !path
            .components()
            .any(|c| c == Component::Normal(".cargo".as_ref()))
    }

    if let Some(paths) = process().var_os("PATH") {
        let paths = env::split_paths(&paths).filter(ignore_paths);

        for path in paths {
            let rustc = path.join(format!("rustc{}", EXE_SUFFIX));
            let cargo = path.join(format!("cargo{}", EXE_SUFFIX));

            if rustc.exists() || cargo.exists() {
                return Err(path.to_str().unwrap().into());
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
        warn!("rustup should not be installed alongside Rust. Please uninstall your existing Rust first.");
        warn!("Otherwise you may have confusion unless you are careful with your PATH");
        warn!("If you are sure that you want both rustup and your already installed Rust");
        warn!("then please reply `y' or `yes' or set RUSTUP_INIT_SKIP_PATH_CHECK to yes");
        warn!("or pass `-y' to ignore all ignorable checks.");
        ignorable_error("cannot install while Rust is installed".into(), no_prompt)?;
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
        ignorable_error("cannot install while Rust is installed".into(), no_prompt)?;
    }

    if rustup_sh_exists {
        warn!("it looks like you have existing rustup.sh metadata");
        warn!("rustup cannot be installed while rustup.sh metadata exists");
        warn!("delete `{}` to remove rustup.sh", rustup_sh_path.display());
        warn!("or, if you already have rustup installed, you can run");
        warn!("`rustup self update` and `rustup toolchain list` to upgrade");
        warn!("your directory structure");
        ignorable_error(
            "cannot install while rustup.sh is installed".into(),
            no_prompt,
        )?;
    }

    Ok(())
}

fn do_pre_install_options_sanity_checks(opts: &InstallOpts<'_>) -> Result<()> {
    use std::str::FromStr;
    // Verify that the installation options are vaguely sane
    (|| {
        let host_triple = opts
            .default_host_triple
            .as_ref()
            .map(|s| dist::TargetTriple::new(s))
            .unwrap_or_else(TargetTriple::from_host_or_build);
        let toolchain_to_use = match &opts.default_toolchain {
            None => "stable",
            Some(s) if s == "none" => "stable",
            Some(s) => &s,
        };
        let partial_channel = dist::PartialToolchainDesc::from_str(toolchain_to_use)?;
        let resolved = partial_channel.resolve(&host_triple)?.to_string();
        debug!(
            "Successfully resolved installation toolchain as: {}",
            resolved
        );
        Ok(())
    })()
    .map_err(|e: Box<dyn std::error::Error>| {
        format!(
            "Pre-checks for host and toolchain failed: {}\n\
             If you are unsure of suitable values, the 'stable' toolchain is the default.\n\
             Valid host triples look something like: {}",
            e,
            dist::TargetTriple::from_host_or_build()
        )
    })?;
    Ok(())
}

#[cfg(not(windows))]
fn do_msvc_check(_opts: &InstallOpts<'_>) -> Result<bool> {
    Ok(true)
}

fn pre_install_msg(no_modify_path: bool) -> Result<String> {
    let cargo_home = utils::cargo_home()?;
    let cargo_home_bin = cargo_home.join("bin");
    let rustup_home = utils::rustup_home()?;

    if !no_modify_path {
        if cfg!(unix) {
            let add_path_methods = get_add_path_methods();
            let rcfiles = add_path_methods
                .into_iter()
                .filter_map(|m| {
                    if let PathUpdateMethod::RcFile(path) = m {
                        Some(format!("{}", path.display()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let plural = if rcfiles.len() > 1 { "s" } else { "" };
            let rcfiles = rcfiles
                .into_iter()
                .map(|f| format!("    {}", f))
                .collect::<Vec<_>>();
            let rcfiles = rcfiles.join("\n");
            Ok(format!(
                pre_install_msg_unix!(),
                cargo_home = cargo_home.display(),
                cargo_home_bin = cargo_home_bin.display(),
                plural = plural,
                rcfiles = rcfiles,
                rustup_home = rustup_home.display(),
            ))
        } else {
            Ok(format!(
                pre_install_msg_win!(),
                cargo_home = cargo_home.display(),
                cargo_home_bin = cargo_home_bin.display(),
                rustup_home = rustup_home.display(),
            ))
        }
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
            .as_deref()
            .unwrap_or("stable (default)"),
        opts.profile,
        if !opts.no_modify_path { "yes" } else { "no" }
    )
}

// Interactive editing of the install options
fn customize_install(mut opts: InstallOpts<'_>) -> Result<InstallOpts<'_>> {
    writeln!(
        process().stdout(),
        "I'm going to ask you the value of each of these installation options.\n\
         You may simply press the Enter key to leave unchanged."
    )?;

    writeln!(process().stdout())?;

    opts.default_host_triple = Some(common::question_str(
        "Default host triple?",
        &opts
            .default_host_triple
            .unwrap_or_else(|| TargetTriple::from_host_or_build().to_string()),
    )?);

    opts.default_toolchain = Some(common::question_str(
        "Default toolchain? (stable/beta/nightly/none)",
        opts.default_toolchain.as_deref().unwrap_or("stable"),
    )?);

    opts.profile = common::question_str(
        &format!(
            "Profile (which tools and data to install)? ({})",
            Profile::names().join("/")
        ),
        &opts.profile,
    )?;

    opts.no_modify_path =
        !common::question_bool("Modify PATH variable? (y/n)", !opts.no_modify_path)?;

    Ok(opts)
}

fn install_bins() -> Result<()> {
    let bin_path = utils::cargo_home()?.join("bin");
    let this_exe_path = utils::current_exe()?;
    let rustup_path = bin_path.join(&format!("rustup{}", EXE_SUFFIX));

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

pub fn install_proxies() -> Result<()> {
    let bin_path = utils::cargo_home()?.join("bin");
    let rustup_path = bin_path.join(&format!("rustup{}", EXE_SUFFIX));

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
        let tool_path = bin_path.join(&format!("{}{}", tool, EXE_SUFFIX));
        if let Ok(handle) = Handle::from_path(&tool_path) {
            tool_handles.push(handle);
            if rustup == *tool_handles.last().unwrap() {
                continue;
            }
        }
        link_afterwards.push(tool_path);
    }

    for tool in DUP_TOOLS {
        let tool_path = bin_path.join(&format!("{}{}", tool, EXE_SUFFIX));
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
            // rand `cargo install rustfmt` and so they had custom `rustfmt`
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
    toolchain: Option<&str>,
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
    if let Some(toolchain) = toolchain {
        if toolchain.exists() {
            warn!("Updating existing toolchain, profile choice will be ignored");
        }
        let distributable = DistributableToolchain::new(&toolchain)?;
        let status = distributable.install_from_dist(true, false, components, targets)?;
        let toolchain_str = toolchain.name().to_owned();
        toolchain.cfg().set_default(&toolchain_str)?;
        writeln!(process().stdout())?;
        common::show_channel_update(&toolchain.cfg(), &toolchain_str, Ok(status))?;
    }
    Ok(())
}

fn _install_selection<'a>(
    cfg: &'a mut Cfg,
    toolchain_opt: Option<&str>,
    profile_str: &str,
    default_host_triple: Option<&str>,
    update_existing_toolchain: bool,
    components: &[&str],
    targets: &[&str],
) -> Result<Option<Toolchain<'a>>> {
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
    Ok(if toolchain_opt == Some("none") {
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
        writeln!(process().stdout())?;
        None
    } else if user_specified_something || cfg.find_default()?.is_none() {
        Some(match toolchain_opt {
            Some(s) => cfg.get_toolchain(s, false)?,
            None => match cfg.find_default()? {
                Some(t) => t,
                None => cfg.get_toolchain("stable", false)?,
            },
        })
    } else {
        info!("updating existing rustup installation - leaving toolchains alone");
        writeln!(process().stdout())?;
        None
    })
}

pub fn uninstall(no_prompt: bool) -> Result<utils::ExitCode> {
    if NEVER_SELF_UPDATE {
        err!("self-uninstall is disabled for this build of rustup");
        err!("you should probably use your system package manager to uninstall rustup");
        return Ok(utils::ExitCode(1));
    }

    let cargo_home = utils::cargo_home()?;

    if !cargo_home
        .join(&format!("bin/rustup{}", EXE_SUFFIX))
        .exists()
    {
        return Err(ErrorKind::NotSelfInstalled(cargo_home).into());
    }

    if !no_prompt {
        writeln!(process().stdout())?;
        let msg = format!(pre_uninstall_msg!(), cargo_home = canonical_cargo_home()?);
        md(&mut term2::stdout(), msg);
        if !common::confirm("\nContinue? (y/N)", false)? {
            info!("aborting uninstallation");
            return Ok(utils::ExitCode(0));
        }
    }

    info!("removing rustup home");

    // Delete RUSTUP_HOME
    let rustup_dir = utils::rustup_home()?;
    if rustup_dir.exists() {
        utils::remove_dir("rustup_home", &rustup_dir, &|_: Notification<'_>| {})?;
    }

    let read_dir_err = "failure reading directory";

    info!("removing cargo home");

    // Remove CARGO_HOME/bin from PATH
    let remove_path_methods = get_remove_path_methods()?;
    do_remove_from_path(&remove_path_methods)?;

    // Delete everything in CARGO_HOME *except* the rustup bin

    // First everything except the bin directory
    let diriter = fs::read_dir(&cargo_home).chain_err(|| read_dir_err)?;
    for dirent in diriter {
        let dirent = dirent.chain_err(|| read_dir_err)?;
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
        .map(|t| format!("{}{}", t, EXE_SUFFIX));
    let tools: Vec<_> = tools.chain(vec![format!("rustup{}", EXE_SUFFIX)]).collect();
    let diriter = fs::read_dir(&cargo_home.join("bin")).chain_err(|| read_dir_err)?;
    for dirent in diriter {
        let dirent = dirent.chain_err(|| read_dir_err)?;
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
pub fn update(cfg: &Cfg) -> Result<utils::ExitCode> {
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

            let _ = common::show_channel_update(cfg, "rustup", Ok(UpdateStatus::Updated(version)));
            return run_update(&setup_path);
        }
        None => {
            let _ = common::show_channel_update(cfg, "rustup", Ok(UpdateStatus::Unchanged));
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

pub fn prepare_update() -> Result<Option<PathBuf>> {
    let cargo_home = utils::cargo_home()?;
    let rustup_path = cargo_home.join(&format!("bin{}rustup{}", MAIN_SEPARATOR, EXE_SUFFIX));
    let setup_path = cargo_home.join(&format!("bin{}rustup-init{}", MAIN_SEPARATOR, EXE_SUFFIX));

    if !rustup_path.exists() {
        return Err(ErrorKind::NotSelfInstalled(cargo_home).into());
    }

    if setup_path.exists() {
        utils::remove_file("setup", &setup_path)?;
    }

    // Get build triple
    let build_triple = dist::TargetTriple::from_build();
    let triple = if cfg!(windows) {
        // For windows x86 builds seem slow when used with windows defender.
        // The website defaulted to i686-windows-gnu builds for a long time.
        // This ensures that we update to a version thats appropriate for users
        // and also works around if the website messed up the detection.
        // If someone really wants to use another version, he still can enforce
        // that using the environment variable RUSTUP_OVERRIDE_HOST_TRIPLE.

        dist::TargetTriple::from_host().unwrap_or(build_triple)
    } else {
        build_triple
    };

    let update_root = process()
        .var("RUSTUP_UPDATE_ROOT")
        .unwrap_or_else(|_| String::from(UPDATE_ROOT));

    let tempdir = tempfile::Builder::new()
        .prefix("rustup-update")
        .tempdir()
        .chain_err(|| "error creating temp directory")?;

    // Get current version
    let current_version = env!("CARGO_PKG_VERSION");

    // Download available version
    info!("checking for self-updates");
    let release_file_url = format!("{}/release-stable.toml", update_root);
    let release_file_url = utils::parse_url(&release_file_url)?;
    let release_file = tempdir.path().join("release-stable.toml");
    utils::download_file(&release_file_url, &release_file, None, &|_| ())?;
    let release_toml_str = utils::read_file("rustup release", &release_file)?;
    let release_toml: toml::Value = toml::from_str(&release_toml_str)
        .map_err(|_| Error::from("unable to parse rustup release file"))?;

    let schema = release_toml
        .get("schema-version")
        .ok_or_else(|| Error::from("no schema key in rustup release file"))?
        .as_str()
        .ok_or_else(|| Error::from("invalid schema key in rustup release file"))?;

    let available_version = release_toml
        .get("version")
        .ok_or_else(|| Error::from("no version key in rustup release file"))?
        .as_str()
        .ok_or_else(|| Error::from("invalid version key in rustup release file"))?;

    if schema != "1" {
        return Err(Error::from(&*format!(
            "unknown schema version '{}' in rustup release file",
            schema
        )));
    }

    // If up-to-date
    if available_version == current_version {
        return Ok(None);
    }

    // Get download URL
    let url = format!(
        "{}/archive/{}/{}/rustup-init{}",
        update_root, available_version, triple, EXE_SUFFIX
    );

    // Get download path
    let download_url = utils::parse_url(&url)?;

    // Download new version
    info!("downloading self-update");
    utils::download_file(&download_url, &setup_path, None, &|_| ())?;

    // Mark as executable
    utils::make_executable(&setup_path)?;

    Ok(Some(setup_path))
}

pub fn cleanup_self_updater() -> Result<()> {
    let cargo_home = utils::cargo_home()?;
    let setup = cargo_home.join(&format!("bin/rustup-init{}", EXE_SUFFIX));

    if setup.exists() {
        utils::remove_file("setup", &setup)?;
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::cli::common;
    use crate::dist::dist::ToolchainDesc;
    use crate::test::{test_dir, with_rustup_home, Env};
    use crate::{currentprocess, for_host};

    #[test]
    fn default_toolchain_is_stable() {
        with_rustup_home(|home| {
            let mut vars = HashMap::new();
            home.apply(&mut vars);
            let tp = Box::new(currentprocess::TestProcess {
                vars,
                ..Default::default()
            });
            currentprocess::with(tp.clone(), || -> anyhow::Result<()> {
                // TODO: we could pass in a custom cfg to get notification
                // callbacks rather than output to the tp sink.
                let mut cfg = common::set_globals(false, false).unwrap();
                assert_eq!(
                    "stable",
                    super::_install_selection(
                        &mut cfg,
                        None,      // No toolchain specified
                        "default", // default profile
                        None,
                        false,
                        &[],
                        &[],
                    )
                    .unwrap() // result
                    .unwrap() // option
                    .name()
                    .parse::<ToolchainDesc>()
                    .unwrap()
                    .channel
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
        let tp = Box::new(currentprocess::TestProcess {
            vars,
            ..Default::default()
        });
        currentprocess::with(tp.clone(), || -> anyhow::Result<()> {
            super::install_bins().unwrap();
            Ok(())
        })
        .unwrap();
        assert!(cargo_home.exists());
    }
}
