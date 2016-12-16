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
//! * run rustu-init with appropriate flags to indicate
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

use common::{self, Confirm};
use errors::*;
use rustup_dist::dist;
use rustup_utils::utils;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::fs;
use tempdir::TempDir;
use term2;
use regex::Regex;

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

",
$platform_msg
,
r#"

You can uninstall at any time with `rustup self uninstall` and
these changes will be reverted.
"#
    )};
}

macro_rules! pre_install_msg_unix {
    () => {
pre_install_msg_template!(
"This path will then be added to your `PATH` environment variable by
modifying the profile file located at:

    {rcfiles}"
    )};
}

macro_rules! pre_install_msg_win {
    () => {
pre_install_msg_template!(
"This path will then be added to your `PATH` environment variable by
modifying the `HKEY_CURRENT_USER/Environment/PATH` registry key."
    )};
}

macro_rules! pre_install_msg_no_modify_path {
    () => {
pre_install_msg_template!(
"This path needs to be in your `PATH` environment variable,
but will not be added automatically."
    )};
}

macro_rules! post_install_msg_unix {
    () => {
r"# Rust is installed now. Great!

To get started you need Cargo's bin directory in your `PATH`
environment variable. Next time you log in this will be done
automatically.

To configure your current shell run `source {cargo_home}/env`
"
    };
}

macro_rules! post_install_msg_win {
    () => {
r"# Rust is installed now. Great!

To get started you need Cargo's bin directory in your `PATH`
environment variable. Future applications will automatically have the
correct environment, but you may need to restart your current shell.
"
    };
}

macro_rules! post_install_msg_unix_no_modify_path {
    () => {
r"# Rust is installed now. Great!

To get started you need Cargo's bin directory in your `PATH`
environment variable.

To configure your current shell run `source {cargo_home}/env`
"
    };
}

macro_rules! post_install_msg_win_no_modify_path {
    () => {
r"# Rust is installed now. Great!

To get started you need Cargo's bin directory in your `PATH`
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
    }
}

#[cfg(windows)]
static MSVC_MESSAGE: &'static str =
r#"# Rust Visual C++ prerequisites

Rust requires the Microsoft C++ build tools for Visual Studio 2013 or
later, but they don't seem to be installed.

The easiest way to acquire the build tools is by installing Microsoft
Visual C++ Build Tools 2015 which provides just the Visual C++ build
tools:

    http://landinghub.visualstudio.com/visual-cpp-build-tools

Alternately, you can install Visual Studio 2015 or Visual
Studio 2013 and during install select the "C++ tools":

    https://www.visualstudio.com/downloads/

_Install the C++ build tools before proceeding_.

If you will be targetting the GNU ABI or otherwise know what you are
doing then it is fine to continue installation without the build
tools, but otherwise, install the C++ build tools before proceeding.
"#;

static TOOLS: &'static [&'static str]
    = &["rustc", "rustdoc", "cargo", "rust-lldb", "rust-gdb"];

static UPDATE_ROOT: &'static str
    = "https://static.rust-lang.org/rustup";

/// CARGO_HOME suitable for display, possibly with $HOME
/// substituted for the directory prefix
fn canonical_cargo_home() -> Result<String> {
    let path = try!(utils::cargo_home());
    let mut path_str = path.to_string_lossy().to_string();

    let default_cargo_home = utils::home_dir().unwrap_or(PathBuf::from(".")).join(".cargo");
    if default_cargo_home == path {
        path_str = String::from("$HOME/.cargo");
    }

    Ok(path_str)
}

/// Installing is a simple matter of coping the running binary to
/// CARGO_HOME/bin, hardlinking the various Rust tools to it,
/// and and adding CARGO_HOME/bin to PATH.
pub fn install(no_prompt: bool, verbose: bool,
               mut opts: InstallOpts) -> Result<()> {

    try!(do_pre_install_sanity_checks());
    try!(do_anti_sudo_check(no_prompt));

    if !try!(do_msvc_check(&opts)) {
        info!("aborting installation");
        return Ok(());
    }

    if !no_prompt {
        let ref msg = try!(pre_install_msg(opts.no_modify_path));

        term2::stdout().md(msg);

        loop {
            term2::stdout().md(current_install_opts(&opts));
            match try!(common::confirm_advanced()) {
                Confirm::No => {
                    info!("aborting installation");
                    return Ok(());
                },
                Confirm::Yes => {
                    break;
                },
                Confirm::Advanced => {
                    opts = try!(customize_install(opts));
                }
            }
        }
    }

    let install_res: Result<()> = (|| {
        try!(cleanup_legacy());
        try!(install_bins());
        if !opts.no_modify_path {
            try!(do_add_to_path(&get_add_path_methods()));
        }
        // Create ~/.rustup and a compatibility ~/.multirust symlink.
        // FIXME: Someday we can stop setting up the symlink, and when
        // we do that we can stop creating ~/.rustup as well.
        try!(utils::create_rustup_home());
        try!(maybe_install_rust(&opts.default_toolchain, &opts.default_host_triple, verbose));

        if cfg!(unix) {
            let ref env_file = try!(utils::cargo_home()).join("env");
            let ref env_str = format!(
                "{}\n",
                try!(shell_export_string()));
            try!(utils::write_file("env", env_file, env_str));
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
            println!("");
            println!("Press the Enter key to continue.");
            try!(common::read_line());
        }

        process::exit(1);
    }

    // More helpful advice, skip if -y
    if !no_prompt {
        let msg = if !opts.no_modify_path {
            if cfg!(unix) {
                let cargo_home = try!(canonical_cargo_home());
                format!(post_install_msg_unix!(),
                         cargo_home = cargo_home)
            } else {
                format!(post_install_msg_win!())
            }
        } else {
            if cfg!(unix) {
                let cargo_home = try!(canonical_cargo_home());
                format!(post_install_msg_unix_no_modify_path!(),
                         cargo_home = cargo_home)
            } else {
                format!(post_install_msg_win_no_modify_path!())
            }
        };
        term2::stdout().md(msg);

        // On windows, where installation happens in a console
        // that may have opened just for this purpose, require
        // the user to press a key to continue.
        if cfg!(windows) {
            println!("");
            println!("Press the Enter key to continue.");
            try!(common::read_line());
        }
    }

    Ok(())
}

fn do_pre_install_sanity_checks() -> Result<()> {

    let multirust_manifest_path
        = PathBuf::from("/usr/local/lib/rustlib/manifest-multirust");
    let rustc_manifest_path
        = PathBuf::from("/usr/local/lib/rustlib/manifest-rustc");
    let uninstaller_path
        = PathBuf::from("/usr/local/lib/rustlib/uninstall.sh");
    let multirust_meta_path
        = env::home_dir().map(|d| d.join(".multirust"));
    let multirust_version_path
        = multirust_meta_path.as_ref().map(|p| p.join("version"));
    let rustup_sh_path
        = env::home_dir().map(|d| d.join(".rustup"));
    let rustup_sh_version_path = rustup_sh_path.as_ref().map(|p| p.join("rustup-version"));

    let multirust_exists =
        multirust_manifest_path.exists() && uninstaller_path.exists();
    let rustc_exists =
        rustc_manifest_path.exists() && uninstaller_path.exists();
    let rustup_sh_exists =
        rustup_sh_version_path.map(|p| p.exists()) == Some(true);
    let old_multirust_meta_exists = if let Some(ref multirust_version_path) = multirust_version_path {
        multirust_version_path.exists() && {
            let version = utils::read_file("old-multirust", &multirust_version_path);
            let version = version.unwrap_or(String::new());
            let version = version.parse().unwrap_or(0);
            let cutoff_version = 12; // First rustup version

            version < cutoff_version
        }
    } else {
        false
    };

    match (multirust_exists, old_multirust_meta_exists) {
        (true, false) => {
            warn!("it looks like you have an existing installation of multirust");
            warn!("rustup cannot be installed alongside multirust");
            warn!("run `{}` as root to uninstall multirust before installing rustup", uninstaller_path.display());
            return Err("cannot install while multirust is installed".into());
        }
        (false, true) => {
            warn!("it looks like you have existing multirust metadata");
            warn!("rustup cannot be installed alongside multirust");
            warn!("delete `{}` before installing rustup", multirust_meta_path.expect("").display());
            return Err("cannot install while multirust is installed".into());
        }
        (true, true) => {
            warn!("it looks like you have an existing installation of multirust");
            warn!("rustup cannot be installed alongside multirust");
            warn!("run `{}` as root and delete `{}` before installing rustup", uninstaller_path.display(), multirust_meta_path.expect("").display());
            return Err("cannot install while multirust is installed".into());
        }
        (false, false) => ()
    }

    if rustc_exists {
        warn!("it looks like you have an existing installation of Rust");
        warn!("rustup cannot be installed alongside Rust. Please uninstall first");
        warn!("run `{}` as root to uninstall Rust", uninstaller_path.display());
        return Err("cannot install while Rust is installed".into());
    }

    if rustup_sh_exists {
        warn!("it looks like you have existing rustup.sh metadata");
        warn!("rustup cannot be installed while rustup.sh metadata exists");
        warn!("delete `{}` to remove rustup.sh", rustup_sh_path.expect("").display());
        return Err("cannot install while rustup.sh is installed".into());
    }

    Ok(())
}

// If the user is trying to install with sudo, on some systems this will
// result in writing root-owned files to the user's home directory, because
// sudo is configured not to change $HOME. Don't let that bogosity happen.
#[allow(dead_code)]
fn do_anti_sudo_check(no_prompt: bool) -> Result<()> {
    #[cfg(unix)]
    pub fn home_mismatch() -> bool {
        extern crate libc as c;

        use std::env;
        use std::ffi::CStr;
        use std::mem;
        use std::ops::Deref;
        use std::ptr;

        // test runner should set this, nothing else
        if env::var("RUSTUP_INIT_SKIP_SUDO_CHECK").as_ref().map(Deref::deref).ok() == Some("yes") {
            return false;
        }
        let mut buf = [0u8; 1024];
        let mut pwd = unsafe { mem::uninitialized::<c::passwd>() };
        let mut pwdp: *mut c::passwd = ptr::null_mut();
        let rv = unsafe { c::getpwuid_r(c::geteuid(), &mut pwd, mem::transmute(&mut buf), buf.len(), &mut pwdp) };
        if rv != 0 || pwdp == ptr::null_mut() {
            warn!("getpwuid_r: couldn't get user data");
            return false;
        }
        let pw_dir = unsafe { CStr::from_ptr(pwd.pw_dir) }.to_str().ok();
        let env_home = env::var_os("HOME");
        let env_home = env_home.as_ref().map(Deref::deref);
        match (env_home, pw_dir) {
            (None, _) | (_, None) => false,
            (Some(ref eh), Some(ref pd)) => eh != pd
        }
    }

    #[cfg(not(unix))]
    pub fn home_mismatch() -> bool {
        false
    }

    match (home_mismatch(), no_prompt) {
        (false, _) => (),
        (true, false) => {
            err!("$HOME differs from euid-obtained home directory: you may be using sudo");
            err!("if this is what you want, restart the installation with `-y'");
            process::exit(1);
        },
        (true, true) => {
            warn!("$HOME differs from euid-obtained home directory: you may be using sudo");
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

    use gcc::windows_registry;
    let installing_msvc = opts.default_host_triple.contains("msvc");
    let have_msvc = windows_registry::find_tool(&opts.default_host_triple, "cl.exe").is_some();
    if installing_msvc && !have_msvc {
        term2::stdout().md(MSVC_MESSAGE);
        if !try!(common::confirm("\nContinue? (Y/n)", true)) {
            return Ok(false);
        }
    }

    Ok(true)
}

#[cfg(not(windows))]
fn do_msvc_check(_opts: &InstallOpts) -> Result<bool> {
    Ok(true)
}

fn pre_install_msg(no_modify_path: bool) -> Result<String> {
    let cargo_home = try!(utils::cargo_home());
    let cargo_home_bin = cargo_home.join("bin");

    if !no_modify_path {
        if cfg!(unix) {
            let add_path_methods = get_add_path_methods();
            let rcfiles = add_path_methods.into_iter()
                .filter_map(|m| {
                    if let PathUpdateMethod::RcFile(path) = m {
                        Some(format!("{}", path.display()))
                    } else {
                        None
                    }
                }).collect::<Vec<_>>();
            assert!(rcfiles.len() == 1); // Only modifying .profile
            Ok(format!(pre_install_msg_unix!(),
                       cargo_home_bin = cargo_home_bin.display(),
                       rcfiles = rcfiles[0]))
        } else {
            Ok(format!(pre_install_msg_win!(),
                       cargo_home_bin = cargo_home_bin.display()))
        }
    } else {
        Ok(format!(pre_install_msg_no_modify_path!(),
                   cargo_home_bin = cargo_home_bin.display()))
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
        "I'm going to ask you the value of each these installation options.\n\
         You may simply press the Enter key to leave unchanged.");

    println!("");

    opts.default_host_triple = try!(common::question_str(
        "Default host triple?",
        &opts.default_host_triple));

    opts.default_toolchain = try!(common::question_str(
        "Default toolchain? (stable/beta/nightly)",
        &opts.default_toolchain));

    opts.no_modify_path = !try!(common::question_bool(
        "Modify PATH variable? (y/n)",
        !opts.no_modify_path));

    Ok(opts)
}

// Before rustup-rs installed bins to $CARGO_HOME/bin it installed
// them to $RUSTUP_HOME/bin. If those bins continue to exist after
// upgrade and are on the $PATH, it would cause major confusion. This
// method silently deletes them.
fn cleanup_legacy() -> Result<()> {
    let legacy_bin_dir = try!(legacy_multirust_home_dir()).join("bin");

    for tool in TOOLS.iter().cloned().chain(vec!["multirust", "rustup"]) {
        let ref file = legacy_bin_dir.join(&format!("{}{}", tool, EXE_SUFFIX));
        if file.exists() {
            try!(utils::remove_file("legacy-bin", file));
        }
    }

    return Ok(());

    #[cfg(unix)]
    fn legacy_multirust_home_dir() -> Result<PathBuf> {
        Ok(try!(utils::legacy_multirust_home()))
    }

    #[cfg(windows)]
    fn legacy_multirust_home_dir() -> Result<PathBuf> {
        use rustup_utils::raw::windows::{
            get_special_folder, FOLDERID_LocalAppData
        };

        // FIXME: This looks bogus. Where is the .multirust dir?
        Ok(get_special_folder(&FOLDERID_LocalAppData).unwrap_or(PathBuf::from(".")))
    }
}

fn install_bins() -> Result<()> {
    let ref bin_path = try!(utils::cargo_home()).join("bin");
    let ref this_exe_path = try!(utils::current_exe());
    let ref rustup_path = bin_path.join(&format!("rustup{}", EXE_SUFFIX));

    try!(utils::ensure_dir_exists("bin", bin_path, &|_| {}));
    // NB: Even on Linux we can't just copy the new binary over the (running)
    // old binary; we must unlink it first.
    if rustup_path.exists() {
        try!(utils::remove_file("rustup-bin", rustup_path));
    }
    try!(utils::copy_file(this_exe_path, rustup_path));
    try!(utils::make_executable(rustup_path));

    // Hardlink all the Rust exes to the rustup exe. Using hardlinks
    // because they work on Windows.
    for tool in TOOLS {
        let ref tool_path = bin_path.join(&format!("{}{}", tool, EXE_SUFFIX));
        try!(utils::hardlink_file(rustup_path, tool_path))
    }

    Ok(())
}

fn maybe_install_rust(toolchain_str: &str, default_host_triple: &str, verbose: bool) -> Result<()> {
    let ref cfg = try!(common::set_globals(verbose));

    // If there is already an install, then `toolchain_str` may not be
    // a toolchain the user actually wants. Don't do anything.  FIXME:
    // This logic should be part of InstallOpts so that it isn't
    // possible to select a toolchain then have it not be installed.
    if try!(cfg.find_default()).is_none() {
        // Set host triple first as it will affect resolution of toolchain_str
        try!(cfg.set_default_host_triple(default_host_triple));
        let toolchain = try!(cfg.get_toolchain(toolchain_str, false));
        let status = try!(toolchain.install_from_dist());
        try!(cfg.set_default(toolchain_str));
        println!("");
        try!(common::show_channel_update(cfg, toolchain_str, Ok(status)));
    } else {
        info!("updating existing rustup installation");
        println!("");
    }

    Ok(())
}

pub fn uninstall(no_prompt: bool) -> Result<()> {
    if NEVER_SELF_UPDATE {
        err!("self-uninstall is disabled for this build of rustup");
        err!("you should probably use your system package manager to uninstall rustup");
        process::exit(1);
    }

    if cfg!(feature = "msi-installed") {
        // Get the product code of the MSI installer from the registry
        // and spawn `msiexec /x`, then exit immediately
        let product_code = try!(get_msi_product_code());
        try!(Command::new("msiexec")
                .arg("/x")
                .arg(product_code)
                .spawn()
                .chain_err(|| ErrorKind::WindowsUninstallMadness));
        process::exit(0);
    }
    
    let ref cargo_home = try!(utils::cargo_home());

    if !cargo_home.join(&format!("bin/rustup{}", EXE_SUFFIX)).exists() {
        return Err(ErrorKind::NotSelfInstalled(cargo_home.clone()).into());
    }

    if !no_prompt {
        println!("");
        let ref msg = format!(pre_uninstall_msg!(),
                              cargo_home = try!(canonical_cargo_home()));
        term2::stdout().md(msg);
        if !try!(common::confirm("\nContinue? (y/N)", false)) {
            info!("aborting uninstallation");
            return Ok(());
        }
    }

    info!("removing rustup home");

    try!(utils::delete_legacy_multirust_symlink());

    // Delete RUSTUP_HOME
    let ref rustup_dir = try!(utils::multirust_home());
    if rustup_dir.exists() {
        try!(utils::remove_dir("rustup_home", rustup_dir, &|_| {}));
    }

    let read_dir_err = "failure reading directory";

    info!("removing cargo home");

    // Remove CARGO_HOME/bin from PATH
    let ref remove_path_methods = try!(get_remove_path_methods());
    try!(do_remove_from_path(remove_path_methods));

    // Delete everything in CARGO_HOME *except* the rustup bin

    // First everything except the bin directory
    for dirent in try!(fs::read_dir(cargo_home).chain_err(|| read_dir_err)) {
        let dirent = try!(dirent.chain_err(|| read_dir_err));
        if dirent.file_name().to_str() != Some("bin") {
            if dirent.path().is_dir() {
                try!(utils::remove_dir("cargo_home", &dirent.path(), &|_| {}));
            } else {
                try!(utils::remove_file("cargo_home", &dirent.path()));
            }
        }
    }

    // Then everything in bin except rustup and tools. These can't be unlinked
    // until this process exits (on windows).
    let tools = TOOLS.iter().map(|t| format!("{}{}", t, EXE_SUFFIX));
    let tools: Vec<_> = tools.chain(vec![format!("rustup{}", EXE_SUFFIX)]).collect();
    for dirent in try!(fs::read_dir(&cargo_home.join("bin")).chain_err(|| read_dir_err)) {
        let dirent = try!(dirent.chain_err(|| read_dir_err));
        let name = dirent.file_name();
        let file_is_tool = name.to_str().map(|n| tools.iter().any(|t| *t == n));
        if file_is_tool == Some(false) {
            if dirent.path().is_dir() {
                try!(utils::remove_dir("cargo_home", &dirent.path(), &|_| {}));
            } else {
                try!(utils::remove_file("cargo_home", &dirent.path()));
            }
        }
    }

    info!("removing rustup binaries");

    // Delete rustup. This is tricky because this is *probably*
    // the running executable and on Windows can't be unlinked until
    // the process exits.
    try!(delete_rustup_and_cargo_home());

    info!("rustup is uninstalled");

    process::exit(0);
}

#[cfg(not(feature = "msi-installed"))]
fn get_msi_product_code() -> Result<String> {
    unreachable!()
}

#[cfg(feature = "msi-installed")]
fn get_msi_product_code() -> Result<String> {
    use winreg::RegKey;
    use winapi::*;

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root.open_subkey_with_flags("SOFTWARE\\rustup", KEY_READ);

    match environment {
        Ok(env) => {
            match env.get_value("InstalledProductCode") {
                Ok(val) => {
                    Ok(val)
                }
                Err(e) => {
                    Err(e).chain_err(|| ErrorKind::WindowsUninstallMadness)
                }
            }
        }
        Err(e) => {
            Err(e).chain_err(|| ErrorKind::WindowsUninstallMadness)
        }
    }
}

#[cfg(unix)]
fn delete_rustup_and_cargo_home() -> Result<()> {
    let ref cargo_home = try!(utils::cargo_home());
    try!(utils::remove_dir("cargo_home", cargo_home, &|_| ()));

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
    use rand;
    use scopeguard;
    use std::thread;
    use std::time::Duration;

    // CARGO_HOME, hopefully empty except for bin/rustup.exe
    let ref cargo_home = try!(utils::cargo_home());
    // The rustup.exe bin
    let ref rustup_path = cargo_home.join(&format!("bin/rustup{}", EXE_SUFFIX));

    // The directory containing CARGO_HOME
    let work_path = cargo_home.parent().expect("CARGO_HOME doesn't have a parent?");

    // Generate a unique name for the files we're about to move out
    // of CARGO_HOME.
    let numbah: u32 = rand::random();
    let gc_exe = work_path.join(&format!("rustup-gc-{:x}.exe", numbah));

    use winapi::{FILE_SHARE_DELETE, FILE_SHARE_READ,
                 INVALID_HANDLE_VALUE, FILE_FLAG_DELETE_ON_CLOSE,
                 DWORD, SECURITY_ATTRIBUTES, OPEN_EXISTING,
                 GENERIC_READ};
    use kernel32::{CreateFileW, CloseHandle};
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use std::io;
    use std::mem;

    unsafe {
        // Copy rustup (probably this process's exe) to the gc exe
        try!(utils::copy_file(rustup_path, &gc_exe));

        let mut gc_exe_win: Vec<_> = gc_exe.as_os_str().encode_wide().collect();
        gc_exe_win.push(0);

        // Open an inheritable handle to the gc exe marked
        // FILE_FLAG_DELETE_ON_CLOSE. This will be inherited
        // by subsequent processes.
        let mut sa = mem::zeroed::<SECURITY_ATTRIBUTES>();
        sa.nLength = mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD;
        sa.bInheritHandle = 1;

        let gc_handle = CreateFileW(gc_exe_win.as_ptr(),
                                    GENERIC_READ,
                                    FILE_SHARE_READ | FILE_SHARE_DELETE,
                                    &mut sa,
                                    OPEN_EXISTING,
                                    FILE_FLAG_DELETE_ON_CLOSE,
                                    ptr::null_mut());

        if gc_handle == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let _g = scopeguard::guard(gc_handle, |h| { let _ = CloseHandle(*h); });

        try!(Command::new(gc_exe).spawn()
             .chain_err(|| ErrorKind::WindowsUninstallMadness));

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

    try!(wait_for_parent());

    // Now that the parent has exited there are hopefully no more files open in CARGO_HOME
    let ref cargo_home = try!(utils::cargo_home());
    try!(utils::remove_dir("cargo_home", cargo_home, &|_| ()));

    // Now, run a *system* binary to inherit the DELETE_ON_CLOSE
    // handle to *this* process, then exit. The OS will delete the gc
    // exe when it exits.
    let rm_gc_exe = OsStr::new("net");

    try!(Command::new(rm_gc_exe)
         .stdin(Stdio::null())
         .stdout(Stdio::null())
         .stderr(Stdio::null())
         .spawn()
         .chain_err(|| ErrorKind::WindowsUninstallMadness));

    process::exit(0);
}

#[cfg(windows)]
fn wait_for_parent() -> Result<()> {
    use kernel32::{Process32First, Process32Next,
                   CreateToolhelp32Snapshot, CloseHandle, OpenProcess,
                   GetCurrentProcessId, WaitForSingleObject};
    use winapi::{PROCESSENTRY32, INVALID_HANDLE_VALUE, DWORD, INFINITE,
                 TH32CS_SNAPPROCESS, SYNCHRONIZE, WAIT_OBJECT_0};
    use std::io;
    use std::mem;
    use std::ptr;
    use scopeguard;

    unsafe {
        // Take a snapshot of system processes, one of which is ours
        // and contains our parent's pid
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let _g = scopeguard::guard(snapshot, |h| { let _ = CloseHandle(*h); });

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

        let _g = scopeguard::guard(parent, |h| { let _ = CloseHandle(*h); });

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
    let rcfiles = vec![profile].into_iter().filter_map(|f|f);

    rcfiles.map(|f| PathUpdateMethod::RcFile(f)).collect()
}

fn shell_export_string() -> Result<String> {
    let path = format!("{}/bin", try!(canonical_cargo_home()));
    // The path is *prepended* in case there are system-installed
    // rustc's that need to be overridden.
    Ok(format!(r#"export PATH="{}:$PATH""#, path))
}

#[cfg(unix)]
fn do_add_to_path(methods: &[PathUpdateMethod]) -> Result<()> {

    for method in methods {
        if let PathUpdateMethod::RcFile(ref rcpath) = *method {
            let file = if rcpath.exists() {
                try!(utils::read_file("rcfile", rcpath))
            } else {
                String::new()
            };
            let ref addition = format!("\n{}", try!(shell_export_string()));
            if !file.contains(addition) {
                try!(utils::append_file("rcfile", rcpath, addition));
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

    use winreg::{RegKey, RegValue};
    use winreg::enums::RegType;
    use winapi::*;
    use user32::*;
    use std::ptr;

    let old_path = if let Some(s) = try!(get_windows_path_var()) {
        s
    } else {
        // Non-unicode path
        return Ok(());
    };

    let mut new_path = try!(utils::cargo_home()).join("bin").to_string_lossy().to_string();
    if old_path.contains(&new_path) {
        return Ok(());
    }

    if !old_path.is_empty() {
        new_path.push_str(";");
        new_path.push_str(&old_path);
    }

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = try!(root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                           .chain_err(|| ErrorKind::PermissionDenied));
    let reg_value = RegValue {
        bytes: utils::string_to_winreg_bytes(&new_path),
        vtype: RegType::REG_EXPAND_SZ,
    };
    try!(environment.set_raw_value("PATH", &reg_value)
         .chain_err(|| ErrorKind::PermissionDenied));

    // Tell other processes to update their environment
    unsafe {
        SendMessageTimeoutA(HWND_BROADCAST,
                            WM_SETTINGCHANGE,
                            0 as WPARAM,
                            "Environment\0".as_ptr() as LPARAM,
                            SMTO_ABORTIFHUNG,
                            5000,
                            ptr::null_mut());
    }

    Ok(())
}

// Get the windows PATH variable out of the registry as a String. If
// this returns None then the PATH varible is not unicode and we
// should not mess with it.
#[cfg(windows)]
fn get_windows_path_var() -> Result<Option<String>> {
    use winreg::RegKey;
    use winapi::*;
    use std::io;

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = try!(root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                           .chain_err(|| ErrorKind::PermissionDenied));

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
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
            Ok(Some(String::new()))
        }
        Err(e) => {
            Err(e).chain_err(|| ErrorKind::WindowsUninstallMadness)
        }
    }
}

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
fn get_remove_path_methods() -> Result<Vec<PathUpdateMethod>> {
    if cfg!(windows) {
        return Ok(vec![PathUpdateMethod::Windows]);
    }

    let profile = utils::home_dir().map(|p| p.join(".profile"));

    let rcfiles = vec![profile];
    let existing_rcfiles = rcfiles.into_iter()
        .filter_map(|f|f)
        .filter(|f| f.exists());

    let export_str = try!(shell_export_string());
    let matching_rcfiles = existing_rcfiles
        .filter(|f| {
            let file = utils::read_file("rcfile", f).unwrap_or(String::new());
            let ref addition = format!("\n{}", export_str);
            file.contains(addition)
        });

    Ok(matching_rcfiles.map(|f| PathUpdateMethod::RcFile(f)).collect())
}

#[cfg(windows)]
fn do_remove_from_path(methods: &[PathUpdateMethod]) -> Result<()> {
    assert!(methods.len() == 1 && methods[0] == PathUpdateMethod::Windows);

    use winreg::{RegKey, RegValue};
    use winreg::enums::RegType;
    use winapi::*;
    use user32::*;
    use std::ptr;

    let old_path = if let Some(s) = try!(get_windows_path_var()) {
        s
    } else {
        // Non-unicode path
        return Ok(());
    };

    let ref path_str = try!(utils::cargo_home()).join("bin").to_string_lossy().to_string();
    let idx = if let Some(i) = old_path.find(path_str) {
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
    new_path.push_str(&old_path[idx + len ..]);

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = try!(root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                           .chain_err(|| ErrorKind::PermissionDenied));
    if new_path.is_empty() {
        try!(environment.delete_value("PATH")
             .chain_err(|| ErrorKind::PermissionDenied));
    } else {
        let reg_value = RegValue {
            bytes: utils::string_to_winreg_bytes(&new_path),
            vtype: RegType::REG_EXPAND_SZ,
        };
        try!(environment.set_raw_value("PATH", &reg_value)
        .chain_err(|| ErrorKind::PermissionDenied));
    }

    // Tell other processes to update their environment
    unsafe {
        SendMessageTimeoutA(HWND_BROADCAST,
                            WM_SETTINGCHANGE,
                            0 as WPARAM,
                            "Environment\0".as_ptr() as LPARAM,
                            SMTO_ABORTIFHUNG,
                            5000,
                            ptr::null_mut());
    }

    Ok(())
}

#[cfg(unix)]
fn do_remove_from_path(methods: &[PathUpdateMethod]) -> Result<()> {
    for method in methods {
        if let PathUpdateMethod::RcFile(ref rcpath) = *method {
            let file = try!(utils::read_file("rcfile", rcpath));
            let addition = format!("\n{}\n", try!(shell_export_string()));

            let file_bytes = file.into_bytes();
            let addition_bytes = addition.into_bytes();

            let idx = file_bytes.windows(addition_bytes.len())
                .position(|w| w == &*addition_bytes);
            if let Some(i) = idx {
                let mut new_file_bytes = file_bytes[..i].to_vec();
                new_file_bytes.extend(&file_bytes[i + addition_bytes.len()..]);
                let ref new_file = String::from_utf8(new_file_bytes).unwrap();
                try!(utils::write_file("rcfile", rcpath, new_file));
            } else {
                // Weird case. rcfile no longer needs to be modified?
            }
        } else {
            unreachable!()
        }
    }

    Ok(())
}

/// Self update downloads rustup-init to CARGO_HOME/bin/rustup-init
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
/// rustup-init is stored in CARGO_HOME/bin, and then deleted next
/// time rustup runs.
pub fn update() -> Result<()> {
    if NEVER_SELF_UPDATE {
        err!("self-update is disabled for this build of rustup");
        err!("you should probably use your system package manager to update rustup");
        process::exit(1);
    }
    let setup_path = try!(prepare_update());
    if let Some(ref p) = setup_path {
        let version = match get_new_rustup_version(&p) {
            Some(new_version) => parse_new_rustup_version(new_version),
            None => {
                err!("failed to get rustup version");
                process::exit(1);
            }
        };

        info!("rustup updated successfully to {}", version);
        try!(run_update(p));
    }

    Ok(())
}

fn get_new_rustup_version(path: &Path) -> Option<String> {
    match Command::new(path).arg("--version").output() {
        Err(_) => None,
        Ok(output) => match String::from_utf8(output.stdout) {
            Ok(version) => Some(version),
            Err(_) => None
        }
    }
}

fn parse_new_rustup_version(version: String) -> String {
    let re = Regex::new(r"\d+.\d+.\d+[0-9a-zA-Z-]*").unwrap();
    let capture = re.captures(&version);
    let matched_version = match capture {
        Some(cap) => cap.at(0).unwrap(),
        None => "(unknown)"
    };
    String::from(matched_version)
}

pub fn prepare_update() -> Result<Option<PathBuf>> {
    use toml;

    let ref cargo_home = try!(utils::cargo_home());
    let ref rustup_path = cargo_home.join(&format!("bin/rustup{}", EXE_SUFFIX));
    let ref setup_path = cargo_home.join(&format!("bin/rustup-init{}", EXE_SUFFIX));

    if !rustup_path.exists() {
        return Err(ErrorKind::NotSelfInstalled(cargo_home.clone()).into());
    }

    if setup_path.exists() {
        try!(utils::remove_file("setup", setup_path));
    }

    // Get build triple
    let triple = dist::TargetTriple::from_build();

    let update_root = env::var("RUSTUP_UPDATE_ROOT")
        .unwrap_or(String::from(UPDATE_ROOT));

    let tempdir = try!(TempDir::new("rustup-update")
        .chain_err(|| "error creating temp directory"));

    // Get current version
    let current_version = env!("CARGO_PKG_VERSION");

    // Download available version
    info!("checking for self-updates");
    let release_file_url = format!("{}/release-stable.toml", update_root);
    let release_file_url = try!(utils::parse_url(&release_file_url));
    let release_file = tempdir.path().join("release-stable.toml");
    try!(utils::download_file(&release_file_url, &release_file, None, &|_| ()));
    let release_toml_str = try!(utils::read_file("rustup release", &release_file));
    let release_toml = try!(toml::Parser::new(&release_toml_str).parse()
                            .ok_or(Error::from("unable to parse rustup release file")));
    let schema = try!(release_toml.get("schema-version")
                      .ok_or(Error::from("no schema key in rustup release file")));
    let schema = try!(schema.as_str()
                      .ok_or(Error::from("invalid schema key in rustup release file")));
    let available_version = try!(release_toml.get("version")
                                 .ok_or(Error::from("no version key in rustup release file")));
    let available_version = try!(available_version.as_str()
                                 .ok_or(Error::from("invalid version key in rustup release file")));

    if schema != "1" {
        return Err(Error::from(&*format!("unknown schema version '{}' in rustup release file", schema)));
    }

    // If up-to-date
    if available_version == current_version {
        return Ok(None);
    }

    // Get download URL
    let url = format!("{}/archive/{}/{}/rustup-init{}", update_root,
                      available_version, triple, EXE_SUFFIX);

    // Get download path
    let download_url = try!(utils::parse_url(&url));

    // Download new version
    info!("downloading self-update");
    try!(utils::download_file(&download_url,
                              &setup_path,
                              None,
                              &|_| ()));

    // Mark as executable
    try!(utils::make_executable(setup_path));

    Ok(Some(setup_path.to_owned()))
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
    let status = try!(Command::new(setup_path)
        .arg("--self-replace")
        .status().chain_err(|| "unable to run updater"));

    if !status.success() {
        return Err("self-updated failed to replace rustup executable".into());
    }

    process::exit(0);
}

#[cfg(windows)]
pub fn run_update(setup_path: &Path) -> Result<()> {
    try!(Command::new(setup_path)
        .arg("--self-replace")
        .spawn().chain_err(|| "unable to run updater"));

    process::exit(0);
}

/// This function is as the final step of a self-upgrade. It replaces
/// CARGO_HOME/bin/rustup with the running exe, and updates the the
/// links to it. On windows this will run *after* the original
/// rustup process exits.
#[cfg(unix)]
pub fn self_replace() -> Result<()> {
    try!(install_bins());

    Ok(())
}

#[cfg(windows)]
pub fn self_replace() -> Result<()> {
    try!(wait_for_parent());
    try!(install_bins());

    Ok(())
}

pub fn cleanup_self_updater() -> Result<()> {
    let cargo_home = try!(utils::cargo_home());
    let ref setup = cargo_home.join(&format!("bin/rustup-init{}", EXE_SUFFIX));

    if setup.exists() {
        try!(utils::remove_file("setup", setup));
    }

    // Transitional
    let ref old_setup = cargo_home.join(&format!("bin/multirust-setup{}", EXE_SUFFIX));

    if old_setup.exists() {
        try!(utils::remove_file("setup", old_setup));
    }

    Ok(())
}
