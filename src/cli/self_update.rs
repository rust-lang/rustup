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

use crate::common::{self, Confirm};
use crate::errors::*;
use crate::markdown::md;
use crate::term2;
use rustup::dist::dist;
use rustup::utils::utils;
use rustup::utils::Notification;
use rustup::{DUP_TOOLS, TOOLS};
use same_file::Handle;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{self, Command};
use tempdir::TempDir;

pub struct InstallOpts {
    pub default_host_triple: String,
    pub default_toolchain: String,
    pub no_modify_path: bool,
}

#[cfg(feature = "no-self-update")]
pub const NEVER_SELF_UPDATE: bool = true;
#[cfg(not(feature = "no-self-update"))]
pub const NEVER_SELF_UPDATE: bool = false;

// The big installation messages. These are macros because the first
// argument of format! needs to be a literal.

macro_rules! pre_install_msg_template {
    ($platform_msg: expr) => {
        concat!(
            r"
# Welcome to Rust!

This will download and install the official compiler for the Rust
programming language, and its package manager, Cargo.

It will add the `cargo`, `rustc`, `rustup` and other commands to
Cargo's bin directory, located at:

    {cargo_home_bin}

This can be modified with the CARGO_HOME environment variable.

Rustup metadata and toolchains will be installed into the Rustup
home directory, located at:

    {rustup_home}

This can be modified with the RUSTUP_HOME environment variable.

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

To get started you need Cargo's bin directory ({cargo_home}\bin) in your `PATH`
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

To get started you need Cargo's bin directory ({cargo_home}\bin) in your `PATH`
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

    https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2019

Please ensure the Windows 10 SDK component is included when installing
the Visual C++ Build Tools.

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
fn canonical_cargo_home() -> Result<String> {
    let path = utils::cargo_home()?;
    let mut path_str = path.to_string_lossy().to_string();

    let default_cargo_home = utils::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cargo");
    if default_cargo_home == path {
        if cfg!(unix) {
            path_str = String::from("$HOME/.cargo");
        } else {
            path_str = String::from(r"%USERPROFILE%\.cargo");
        }
    }

    Ok(path_str)
}

/// Installing is a simple matter of copying the running binary to
/// `CARGO_HOME`/bin, hard-linking the various Rust tools to it,
/// and adding `CARGO_HOME`/bin to PATH.
pub fn install(no_prompt: bool, verbose: bool, mut opts: InstallOpts) -> Result<()> {
    do_pre_install_sanity_checks()?;
    do_pre_install_options_sanity_checks(&opts)?;
    check_existence_of_rustc_or_cargo_in_path(no_prompt)?;
    do_anti_sudo_check(no_prompt)?;

    let mut term = term2::stdout();
    if !do_msvc_check(&opts)? {
        if no_prompt {
            warn!("installing msvc toolchain without its prerequisites");
        } else {
            md(&mut term, MSVC_MESSAGE);
            if !common::confirm("\nContinue? (Y/n)", true)? {
                info!("aborting installation");
                return Ok(());
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
                    return Ok(());
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

    let install_res: Result<()> = (|| {
        install_bins()?;
        if !opts.no_modify_path {
            do_add_to_path(&get_add_path_methods())?;
        }
        utils::create_rustup_home()?;
        maybe_install_rust(&opts.default_toolchain, &opts.default_host_triple, verbose)?;

        if cfg!(unix) {
            let env_file = utils::cargo_home()?.join("env");
            let env_str = format!("{}\n", shell_export_string()?);
            utils::write_file("env", &env_file, &env_str)?;
        }

        Ok(())
    })();

    if let Err(ref e) = install_res {
        common::report_error(e);

        // On windows, where installation happens in a console
        // that may have opened just for this purpose, give
        // the user an opportunity to see the error before the
        // window closes.
        if cfg!(windows) && !no_prompt {
            println!();
            println!("Press the Enter key to continue.");
            common::read_line()?;
        }

        process::exit(1);
    }

    let cargo_home = canonical_cargo_home()?;
    let msg = if !opts.no_modify_path {
        if cfg!(unix) {
            format!(post_install_msg_unix!(), cargo_home = cargo_home)
        } else {
            format!(post_install_msg_win!(), cargo_home = cargo_home)
        }
    } else if cfg!(unix) {
        format!(
            post_install_msg_unix_no_modify_path!(),
            cargo_home = cargo_home
        )
    } else {
        format!(
            post_install_msg_win_no_modify_path!(),
            cargo_home = cargo_home
        )
    };
    md(&mut term, msg);

    if !no_prompt {
        // On windows, where installation happens in a console
        // that may have opened just for this purpose, require
        // the user to press a key to continue.
        if cfg!(windows) {
            println!();
            println!("Press the Enter key to continue.");
            common::read_line()?;
        }
    }

    Ok(())
}

fn rustc_or_cargo_exists_in_path() -> Result<()> {
    // Ignore rustc and cargo if present in $HOME/.cargo/bin or a few other directories
    fn ignore_paths(path: &PathBuf) -> bool {
        !path
            .components()
            .any(|c| c == Component::Normal(".cargo".as_ref()))
    }

    if let Some(paths) = env::var_os("PATH") {
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
    let skip_check = env::var_os("RUSTUP_INIT_SKIP_PATH_CHECK");

    // Ignore this check if called with no prompt (-y) or if the environment variable is set
    if no_prompt || skip_check == Some("yes".into()) {
        return Ok(());
    }

    if let Err(path) = rustc_or_cargo_exists_in_path() {
        err!("it looks like you have an existing installation of Rust at:");
        err!("{}", path);
        err!("rustup cannot be installed alongside Rust. Please uninstall first");
        err!("if this is what you want, restart the installation with `-y'");
        Err("cannot install while Rust is installed".into())
    } else {
        Ok(())
    }
}

fn do_pre_install_sanity_checks() -> Result<()> {
    let rustc_manifest_path = PathBuf::from("/usr/local/lib/rustlib/manifest-rustc");
    let uninstaller_path = PathBuf::from("/usr/local/lib/rustlib/uninstall.sh");
    let rustup_sh_path = utils::home_dir().map(|d| d.join(".rustup"));
    let rustup_sh_version_path = rustup_sh_path.as_ref().map(|p| p.join("rustup-version"));

    let rustc_exists = rustc_manifest_path.exists() && uninstaller_path.exists();
    let rustup_sh_exists = rustup_sh_version_path.map(|p| p.exists()) == Some(true);

    if rustc_exists {
        warn!("it looks like you have an existing installation of Rust");
        warn!("rustup cannot be installed alongside Rust. Please uninstall first");
        warn!(
            "run `{}` as root to uninstall Rust",
            uninstaller_path.display()
        );
        return Err("cannot install while Rust is installed".into());
    }

    if rustup_sh_exists {
        warn!("it looks like you have existing rustup.sh metadata");
        warn!("rustup cannot be installed while rustup.sh metadata exists");
        warn!(
            "delete `{}` to remove rustup.sh",
            rustup_sh_path.expect("").display()
        );
        warn!("or, if you already have rustup installed, you can run");
        warn!("`rustup self update` and `rustup toolchain list` to upgrade");
        warn!("your directory structure");
        return Err("cannot install while rustup.sh is installed".into());
    }

    Ok(())
}

fn do_pre_install_options_sanity_checks(opts: &InstallOpts) -> Result<()> {
    use std::str::FromStr;
    // Verify that the installation options are vaguely sane
    (|| {
        let host_triple = dist::TargetTriple::new(&opts.default_host_triple);
        let toolchain_to_use = if opts.default_toolchain == "none" {
            "stable"
        } else {
            &opts.default_toolchain
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

// If the user is trying to install with sudo, on some systems this will
// result in writing root-owned files to the user's home directory, because
// sudo is configured not to change $HOME. Don't let that bogosity happen.
#[allow(dead_code)]
fn do_anti_sudo_check(no_prompt: bool) -> Result<()> {
    use std::ffi::OsString;

    #[cfg(unix)]
    pub fn home_mismatch() -> (bool, OsString, String) {
        use std::ffi::CStr;
        use std::mem;
        use std::ops::Deref;
        use std::ptr;

        // test runner should set this, nothing else
        if env::var("RUSTUP_INIT_SKIP_SUDO_CHECK")
            .as_ref()
            .map(Deref::deref)
            .ok()
            == Some("yes")
        {
            return (false, OsString::new(), String::new());
        }
        let mut buf = [0u8; 1024];
        let mut pwd = unsafe { mem::uninitialized::<libc::passwd>() };
        let mut pwdp: *mut libc::passwd = ptr::null_mut();
        let rv = unsafe {
            libc::getpwuid_r(
                libc::geteuid(),
                &mut pwd,
                &mut buf as *mut [u8] as *mut libc::c_char,
                buf.len(),
                &mut pwdp,
            )
        };
        if rv != 0 || pwdp.is_null() {
            warn!("getpwuid_r: couldn't get user data");
            return (false, OsString::new(), String::new());
        }
        let pw_dir = unsafe { CStr::from_ptr(pwd.pw_dir) }.to_str().ok();
        let env_home = env::var_os("HOME");
        let env_home = env_home.as_ref().map(Deref::deref);
        match (env_home, pw_dir) {
            (None, _) | (_, None) => (false, OsString::new(), String::new()),
            (Some(eh), Some(pd)) => (eh != pd, OsString::from(eh), String::from(pd)),
        }
    }

    #[cfg(not(unix))]
    pub fn home_mismatch() -> (bool, OsString, String) {
        (false, OsString::new(), String::new())
    }

    match (home_mismatch(), no_prompt) {
        ((false, _, _), _) => (),
        ((true, env_home, euid_home), false) => {
            err!("$HOME differs from euid-obtained home directory: you may be using sudo");
            err!("$HOME directory: {:?}", env_home);
            err!("euid-obtained home directory: {}", euid_home);
            err!("if this is what you want, restart the installation with `-y'");
            process::exit(1);
        }
        ((true, env_home, euid_home), true) => {
            warn!("$HOME differs from euid-obtained home directory: you may be using sudo");
            warn!("$HOME directory: {:?}", env_home);
            warn!("euid-obtained home directory: {}", euid_home);
        }
    }

    Ok(())
}

// Provide guidance about setting up MSVC if it doesn't appear to be
// installed
#[cfg(windows)]
fn do_msvc_check(opts: &InstallOpts) -> Result<bool> {
    // Test suite skips this since it's env dependent
    if env::var("RUSTUP_INIT_SKIP_MSVC_CHECK").is_ok() {
        return Ok(true);
    }

    use cc::windows_registry;
    let installing_msvc = opts.default_host_triple.contains("msvc");
    let have_msvc = windows_registry::find_tool(&opts.default_host_triple, "cl.exe").is_some();
    if installing_msvc && !have_msvc {
        return Ok(false);
    }

    Ok(true)
}

#[cfg(not(windows))]
fn do_msvc_check(_opts: &InstallOpts) -> Result<bool> {
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
                cargo_home_bin = cargo_home_bin.display(),
                plural = plural,
                rcfiles = rcfiles,
                rustup_home = rustup_home.display(),
            ))
        } else {
            Ok(format!(
                pre_install_msg_win!(),
                cargo_home_bin = cargo_home_bin.display(),
                rustup_home = rustup_home.display(),
            ))
        }
    } else {
        Ok(format!(
            pre_install_msg_no_modify_path!(),
            cargo_home_bin = cargo_home_bin.display(),
            rustup_home = rustup_home.display(),
        ))
    }
}

fn current_install_opts(opts: &InstallOpts) -> String {
    format!(
        r"Current installation options:

- ` `default host triple: `{}`
- `   `default toolchain: `{}`
- modify PATH variable: `{}`
",
        opts.default_host_triple,
        opts.default_toolchain,
        if !opts.no_modify_path { "yes" } else { "no" }
    )
}

// Interactive editing of the install options
fn customize_install(mut opts: InstallOpts) -> Result<InstallOpts> {
    println!(
        "I'm going to ask you the value of each of these installation options.\n\
         You may simply press the Enter key to leave unchanged."
    );

    println!();

    opts.default_host_triple =
        common::question_str("Default host triple?", &opts.default_host_triple)?;

    opts.default_toolchain = common::question_str(
        "Default toolchain? (stable/beta/nightly/none)",
        &opts.default_toolchain,
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
                      tool, bin_path.to_string_lossy());
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

fn maybe_install_rust(toolchain_str: &str, default_host_triple: &str, verbose: bool) -> Result<()> {
    let cfg = common::set_globals(verbose)?;

    // If there is already an install, then `toolchain_str` may not be
    // a toolchain the user actually wants. Don't do anything.  FIXME:
    // This logic should be part of InstallOpts so that it isn't
    // possible to select a toolchain then have it not be installed.
    if toolchain_str == "none" {
        info!("skipping toolchain installation");
        println!();
    } else if cfg.find_default()?.is_none() {
        // Set host triple first as it will affect resolution of toolchain_str
        cfg.set_default_host_triple(default_host_triple)?;
        let toolchain = cfg.get_toolchain(toolchain_str, false)?;
        let status = toolchain.install_from_dist(false)?;
        cfg.set_default(toolchain_str)?;
        println!();
        common::show_channel_update(&cfg, toolchain_str, Ok(status))?;
    } else {
        info!("updating existing rustup installation");
        println!();
    }

    Ok(())
}

pub fn uninstall(no_prompt: bool) -> Result<()> {
    if NEVER_SELF_UPDATE {
        err!("self-uninstall is disabled for this build of rustup");
        err!("you should probably use your system package manager to uninstall rustup");
        process::exit(1);
    }

    let cargo_home = utils::cargo_home()?;

    if !cargo_home
        .join(&format!("bin/rustup{}", EXE_SUFFIX))
        .exists()
    {
        return Err(ErrorKind::NotSelfInstalled(cargo_home.clone()).into());
    }

    if !no_prompt {
        println!();
        let msg = format!(pre_uninstall_msg!(), cargo_home = canonical_cargo_home()?);
        md(&mut term2::stdout(), msg);
        if !common::confirm("\nContinue? (y/N)", false)? {
            info!("aborting uninstallation");
            return Ok(());
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
    for dirent in fs::read_dir(&cargo_home).chain_err(|| read_dir_err)? {
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
    for dirent in fs::read_dir(&cargo_home.join("bin")).chain_err(|| read_dir_err)? {
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

    process::exit(0);
}

#[cfg(unix)]
fn delete_rustup_and_cargo_home() -> Result<()> {
    let cargo_home = utils::cargo_home()?;
    utils::remove_dir("cargo_home", &cargo_home, &|_: Notification<'_>| ())?;

    Ok(())
}

// The last step of uninstallation is to delete *this binary*,
// rustup.exe and the CARGO_HOME that contains it. On Unix, this
// works fine. On Windows you can't delete files while they are open,
// like when they are running.
//
// Here's what we're going to do:
// - Copy rustup to a temporary file in
//   CARGO_HOME/../rustup-gc-$random.exe.
// - Open the gc exe with the FILE_FLAG_DELETE_ON_CLOSE and
//   FILE_SHARE_DELETE flags. This is going to be the last
//   file to remove, and the OS is going to do it for us.
//   This file is opened as inheritable so that subsequent
//   processes created with the option to inherit handles
//   will also keep them open.
// - Run the gc exe, which waits for the original rustup
//   process to close, then deletes CARGO_HOME. This process
//   has inherited a FILE_FLAG_DELETE_ON_CLOSE handle to itself.
// - Finally, spawn yet another system binary with the inherit handles
//   flag, so *it* inherits the FILE_FLAG_DELETE_ON_CLOSE handle to
//   the gc exe. If the gc exe exits before the system exe then at
//   last it will be deleted when the handle closes.
//
// This is the DELETE_ON_CLOSE method from
// http://www.catch22.net/tuts/self-deleting-executables
//
// ... which doesn't actually work because Windows won't really
// delete a FILE_FLAG_DELETE_ON_CLOSE process when it exits.
//
// .. augmented with this SO answer
// http://stackoverflow.com/questions/10319526/understanding-a-self-deleting-program-in-c
#[cfg(windows)]
fn delete_rustup_and_cargo_home() -> Result<()> {
    use std::thread;
    use std::time::Duration;

    // CARGO_HOME, hopefully empty except for bin/rustup.exe
    let cargo_home = utils::cargo_home()?;
    // The rustup.exe bin
    let rustup_path = cargo_home.join(&format!("bin/rustup{}", EXE_SUFFIX));

    // The directory containing CARGO_HOME
    let work_path = cargo_home
        .parent()
        .expect("CARGO_HOME doesn't have a parent?");

    // Generate a unique name for the files we're about to move out
    // of CARGO_HOME.
    let numbah: u32 = rand::random();
    let gc_exe = work_path.join(&format!("rustup-gc-{:x}.exe", numbah));

    use std::io;
    use std::mem;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
    use winapi::um::winbase::FILE_FLAG_DELETE_ON_CLOSE;
    use winapi::um::winnt::{FILE_SHARE_DELETE, FILE_SHARE_READ, GENERIC_READ};

    unsafe {
        // Copy rustup (probably this process's exe) to the gc exe
        utils::copy_file(&rustup_path, &gc_exe)?;

        let mut gc_exe_win: Vec<_> = gc_exe.as_os_str().encode_wide().collect();
        gc_exe_win.push(0);

        // Open an inheritable handle to the gc exe marked
        // FILE_FLAG_DELETE_ON_CLOSE. This will be inherited
        // by subsequent processes.
        let mut sa = mem::zeroed::<SECURITY_ATTRIBUTES>();
        sa.nLength = mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD;
        sa.bInheritHandle = 1;

        let gc_handle = CreateFileW(
            gc_exe_win.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            &mut sa,
            OPEN_EXISTING,
            FILE_FLAG_DELETE_ON_CLOSE,
            ptr::null_mut(),
        );

        if gc_handle == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let _g = scopeguard::guard(gc_handle, |h| {
            let _ = CloseHandle(h);
        });

        Command::new(gc_exe)
            .spawn()
            .chain_err(|| ErrorKind::WindowsUninstallMadness)?;

        // The catch 22 article says we must sleep here to give
        // Windows a chance to bump the processes file reference
        // count. acrichto though is in disbelief and *demanded* that
        // we not insert a sleep. If Windows failed to uninstall
        // correctly it is because of him.

        // (.. and months later acrichto owes me a beer).
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

/// Run by rustup-gc-$num.exe to delete CARGO_HOME
#[cfg(windows)]
pub fn complete_windows_uninstall() -> Result<()> {
    use std::ffi::OsStr;
    use std::process::Stdio;

    wait_for_parent()?;

    // Now that the parent has exited there are hopefully no more files open in CARGO_HOME
    let cargo_home = utils::cargo_home()?;
    utils::remove_dir("cargo_home", &cargo_home, &|_: Notification<'_>| ())?;

    // Now, run a *system* binary to inherit the DELETE_ON_CLOSE
    // handle to *this* process, then exit. The OS will delete the gc
    // exe when it exits.
    let rm_gc_exe = OsStr::new("net");

    Command::new(rm_gc_exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .chain_err(|| ErrorKind::WindowsUninstallMadness)?;

    process::exit(0);
}

#[cfg(windows)]
fn wait_for_parent() -> Result<()> {
    use std::io;
    use std::mem;
    use std::ptr;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::processthreadsapi::{GetCurrentProcessId, OpenProcess};
    use winapi::um::synchapi::WaitForSingleObject;
    use winapi::um::tlhelp32::{
        CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
    };
    use winapi::um::winbase::{INFINITE, WAIT_OBJECT_0};
    use winapi::um::winnt::SYNCHRONIZE;

    unsafe {
        // Take a snapshot of system processes, one of which is ours
        // and contains our parent's pid
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let _g = scopeguard::guard(snapshot, |h| {
            let _ = CloseHandle(h);
        });

        let mut entry: PROCESSENTRY32 = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32>() as DWORD;

        // Iterate over system processes looking for ours
        let success = Process32First(snapshot, &mut entry);
        if success == 0 {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let this_pid = GetCurrentProcessId();
        while entry.th32ProcessID != this_pid {
            let success = Process32Next(snapshot, &mut entry);
            if success == 0 {
                let err = io::Error::last_os_error();
                return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
            }
        }

        // FIXME: Using the process ID exposes a race condition
        // wherein the parent process already exited and the OS
        // reassigned its ID.
        let parent_id = entry.th32ParentProcessID;

        // Get a handle to the parent process
        let parent = OpenProcess(SYNCHRONIZE, 0, parent_id);
        if parent == ptr::null_mut() {
            // This just means the parent has already exited.
            return Ok(());
        }

        let _g = scopeguard::guard(parent, |h| {
            let _ = CloseHandle(h);
        });

        // Wait for our parent to exit
        let res = WaitForSingleObject(parent, INFINITE);

        if res != WAIT_OBJECT_0 {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }
    }

    Ok(())
}

#[cfg(unix)]
pub fn complete_windows_uninstall() -> Result<()> {
    panic!("stop doing that")
}

#[derive(PartialEq)]
enum PathUpdateMethod {
    RcFile(PathBuf),
    Windows,
}

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
fn get_add_path_methods() -> Vec<PathUpdateMethod> {
    if cfg!(windows) {
        return vec![PathUpdateMethod::Windows];
    }

    let profile = utils::home_dir().map(|p| p.join(".profile"));
    let mut profiles = vec![profile];

    if let Ok(shell) = env::var("SHELL") {
        if shell.contains("zsh") {
            let zdotdir = env::var("ZDOTDIR")
                .ok()
                .map(PathBuf::from)
                .or_else(utils::home_dir);
            let zprofile = zdotdir.map(|p| p.join(".zprofile"));
            profiles.push(zprofile);
        }
    }

    if let Some(bash_profile) = utils::home_dir().map(|p| p.join(".bash_profile")) {
        // Only update .bash_profile if it exists because creating .bash_profile
        // will cause .profile to not be read
        if bash_profile.exists() {
            profiles.push(Some(bash_profile));
        }
    }

    let rcfiles = profiles.into_iter().filter_map(|f| f);
    rcfiles.map(PathUpdateMethod::RcFile).collect()
}

fn shell_export_string() -> Result<String> {
    let path = format!("{}/bin", canonical_cargo_home()?);
    // The path is *prepended* in case there are system-installed
    // rustc's that need to be overridden.
    Ok(format!(r#"export PATH="{}:$PATH""#, path))
}

#[cfg(unix)]
fn do_add_to_path(methods: &[PathUpdateMethod]) -> Result<()> {
    for method in methods {
        if let PathUpdateMethod::RcFile(ref rcpath) = *method {
            let file = if rcpath.exists() {
                utils::read_file("rcfile", rcpath)?
            } else {
                String::new()
            };
            let addition = format!("\n{}", shell_export_string()?);
            if !file.contains(&addition) {
                utils::append_file("rcfile", rcpath, &addition)?;
            }
        } else {
            unreachable!()
        }
    }

    Ok(())
}

#[cfg(windows)]
fn do_add_to_path(methods: &[PathUpdateMethod]) -> Result<()> {
    assert!(methods.len() == 1 && methods[0] == PathUpdateMethod::Windows);

    use std::ptr;
    use winapi::shared::minwindef::*;
    use winapi::um::winuser::{
        SendMessageTimeoutA, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::{RegKey, RegValue};

    let old_path = if let Some(s) = get_windows_path_var()? {
        s
    } else {
        // Non-unicode path
        return Ok(());
    };

    let mut new_path = utils::cargo_home()?
        .join("bin")
        .to_string_lossy()
        .to_string();
    if old_path.contains(&new_path) {
        return Ok(());
    }

    if !old_path.is_empty() {
        new_path.push_str(";");
        new_path.push_str(&old_path);
    }

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .chain_err(|| ErrorKind::PermissionDenied)?;

    let reg_value = RegValue {
        bytes: utils::string_to_winreg_bytes(&new_path),
        vtype: RegType::REG_EXPAND_SZ,
    };

    environment
        .set_raw_value("PATH", &reg_value)
        .chain_err(|| ErrorKind::PermissionDenied)?;

    // Tell other processes to update their environment
    unsafe {
        SendMessageTimeoutA(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0 as WPARAM,
            "Environment\0".as_ptr() as LPARAM,
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }

    Ok(())
}

// Get the windows PATH variable out of the registry as a String. If
// this returns None then the PATH varible is not unicode and we
// should not mess with it.
#[cfg(windows)]
fn get_windows_path_var() -> Result<Option<String>> {
    use std::io;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .chain_err(|| ErrorKind::PermissionDenied)?;

    let reg_value = environment.get_raw_value("PATH");
    match reg_value {
        Ok(val) => {
            if let Some(s) = utils::string_from_winreg_value(&val) {
                Ok(Some(s))
            } else {
                warn!("the registry key HKEY_CURRENT_USER\\Environment\\PATH does not contain valid Unicode. \
                       Not modifying the PATH variable");
                return Ok(None);
            }
        }
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(Some(String::new())),
        Err(e) => Err(e).chain_err(|| ErrorKind::WindowsUninstallMadness),
    }
}

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
fn get_remove_path_methods() -> Result<Vec<PathUpdateMethod>> {
    if cfg!(windows) {
        return Ok(vec![PathUpdateMethod::Windows]);
    }

    let profile = utils::home_dir().map(|p| p.join(".profile"));
    let bash_profile = utils::home_dir().map(|p| p.join(".bash_profile"));

    let rcfiles = vec![profile, bash_profile];
    let existing_rcfiles = rcfiles.into_iter().filter_map(|f| f).filter(|f| f.exists());

    let export_str = shell_export_string()?;
    let matching_rcfiles = existing_rcfiles.filter(|f| {
        let file = utils::read_file("rcfile", f).unwrap_or_default();
        let addition = format!("\n{}", export_str);
        file.contains(&addition)
    });

    Ok(matching_rcfiles.map(PathUpdateMethod::RcFile).collect())
}

#[cfg(windows)]
fn do_remove_from_path(methods: &[PathUpdateMethod]) -> Result<()> {
    assert!(methods.len() == 1 && methods[0] == PathUpdateMethod::Windows);

    use std::ptr;
    use winapi::shared::minwindef::*;
    use winapi::um::winuser::{
        SendMessageTimeoutA, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::{RegKey, RegValue};

    let old_path = if let Some(s) = get_windows_path_var()? {
        s
    } else {
        // Non-unicode path
        return Ok(());
    };

    let path_str = utils::cargo_home()?
        .join("bin")
        .to_string_lossy()
        .to_string();
    let idx = if let Some(i) = old_path.find(&path_str) {
        i
    } else {
        return Ok(());
    };

    // If there's a trailing semicolon (likely, since we added one during install),
    // include that in the substring to remove.
    let mut len = path_str.len();
    if old_path.as_bytes().get(idx + path_str.len()) == Some(&b';') {
        len += 1;
    }

    let mut new_path = old_path[..idx].to_string();
    new_path.push_str(&old_path[idx + len..]);

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .chain_err(|| ErrorKind::PermissionDenied)?;

    if new_path.is_empty() {
        environment
            .delete_value("PATH")
            .chain_err(|| ErrorKind::PermissionDenied)?;
    } else {
        let reg_value = RegValue {
            bytes: utils::string_to_winreg_bytes(&new_path),
            vtype: RegType::REG_EXPAND_SZ,
        };
        environment
            .set_raw_value("PATH", &reg_value)
            .chain_err(|| ErrorKind::PermissionDenied)?;
    }

    // Tell other processes to update their environment
    unsafe {
        SendMessageTimeoutA(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0 as WPARAM,
            "Environment\0".as_ptr() as LPARAM,
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }

    Ok(())
}

#[cfg(unix)]
fn do_remove_from_path(methods: &[PathUpdateMethod]) -> Result<()> {
    for method in methods {
        if let PathUpdateMethod::RcFile(ref rcpath) = *method {
            let file = utils::read_file("rcfile", rcpath)?;
            let addition = format!("\n{}\n", shell_export_string()?);

            let file_bytes = file.into_bytes();
            let addition_bytes = addition.into_bytes();

            let idx = file_bytes
                .windows(addition_bytes.len())
                .position(|w| w == &*addition_bytes);
            if let Some(i) = idx {
                let mut new_file_bytes = file_bytes[..i].to_vec();
                new_file_bytes.extend(&file_bytes[i + addition_bytes.len()..]);
                let new_file = String::from_utf8(new_file_bytes).unwrap();
                utils::write_file("rcfile", rcpath, &new_file)?;
            } else {
                // Weird case. rcfile no longer needs to be modified?
            }
        } else {
            unreachable!()
        }
    }

    Ok(())
}

/// Self update downloads rustup-init to `CARGO_HOME`/bin/rustup-init
/// and runs it.
///
/// It does a few things to accomodate self-delete problems on windows:
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
pub fn update() -> Result<()> {
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
            process::exit(1);
        }
        Skip => {
            info!("Skipping self-update at this time");
            return Ok(());
        }
        Permit => {}
    }

    let setup_path = prepare_update()?;
    if let Some(ref p) = setup_path {
        let version = match get_new_rustup_version(p) {
            Some(new_version) => parse_new_rustup_version(new_version),
            None => {
                err!("failed to get rustup version");
                process::exit(1);
            }
        };

        info!("rustup updated successfully to {}", version);
        run_update(p)?;
    } else {
        // Try again in case we emitted "tool `{}` is already installed" last time.
        install_proxies()?
    }

    Ok(())
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
    let rustup_path = cargo_home.join(&format!("bin/rustup{}", EXE_SUFFIX));
    let setup_path = cargo_home.join(&format!("bin/rustup-init{}", EXE_SUFFIX));

    if !rustup_path.exists() {
        return Err(ErrorKind::NotSelfInstalled(cargo_home.clone()).into());
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

    let update_root = env::var("RUSTUP_UPDATE_ROOT").unwrap_or_else(|_| String::from(UPDATE_ROOT));

    let tempdir = TempDir::new("rustup-update").chain_err(|| "error creating temp directory")?;

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

/// Tell the upgrader to replace the rustup bins, then delete
/// itself. Like with uninstallation, on Windows we're going to
/// have to jump through hoops to make everything work right.
///
/// On windows we're not going to wait for it to finish before exiting
/// successfully, so it should not do much, and it should try
/// really hard to succeed, because at this point the upgrade is
/// considered successful.
#[cfg(unix)]
pub fn run_update(setup_path: &Path) -> Result<()> {
    let status = Command::new(setup_path)
        .arg("--self-replace")
        .status()
        .chain_err(|| "unable to run updater")?;

    if !status.success() {
        return Err("self-updated failed to replace rustup executable".into());
    }

    process::exit(0);
}

#[cfg(windows)]
pub fn run_update(setup_path: &Path) -> Result<()> {
    Command::new(setup_path)
        .arg("--self-replace")
        .spawn()
        .chain_err(|| "unable to run updater")?;

    process::exit(0);
}

/// This function is as the final step of a self-upgrade. It replaces
/// `CARGO_HOME`/bin/rustup with the running exe, and updates the the
/// links to it. On windows this will run *after* the original
/// rustup process exits.
#[cfg(unix)]
pub fn self_replace() -> Result<()> {
    install_bins()?;

    Ok(())
}

#[cfg(windows)]
pub fn self_replace() -> Result<()> {
    wait_for_parent()?;
    install_bins()?;

    Ok(())
}

pub fn cleanup_self_updater() -> Result<()> {
    let cargo_home = utils::cargo_home()?;
    let setup = cargo_home.join(&format!("bin/rustup-init{}", EXE_SUFFIX));

    if setup.exists() {
        utils::remove_file("setup", &setup)?;
    }

    Ok(())
}
