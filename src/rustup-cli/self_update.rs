//! Self-installation and updating
//!
//! This is the installer at the heart of Rust. If it breaks
//! everything breaks. It is conceptually very simple, as multirust is
//! distributed as a single binary, and installation mostly requires
//! copying it into place. There are some tricky bits though, mostly
//! because of workarounds to self-delete an exe on Windows.
//!
//! During install (as `multirust-setup`):
//!
//! * copy the self exe to $CARGO_HOME/bin
//! * hardlink rustc, etc to *that*
//! * update the PATH in a system-specific way
//! * run the equivalent of `multirust default stable`
//!
//! During upgrade (`multirust self upgrade`):
//!
//! * download multirust-setup to $CARGO_HOME/bin/multirust-setup
//! * run multirust-setup with appropriate flags to indicate
//!   this is a self-upgrade
//! * multirust-setup copies bins and hardlinks into place. On windows
//!   this happens *after* the upgrade command exits successfully.
//!
//! During uninstall (`multirust self uninstall`):
//!
//! * Delete `$RUSTUP_HOME`.
//! * Delete everything in `$CARGO_HOME`, including
//!   the multirust binary and its hardlinks
//!
//! Deleting the running binary during uninstall is tricky
//! and racy on Windows.

use common::{self, Confirm};
use itertools::Itertools;
use rustup::{NotifyHandler};
use errors::*;
use rustup_dist::dist;
use rustup_dist;
use rustup_utils::utils;
use openssl::crypto::hash::{Type, Hasher};
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::fs;
use tempdir::TempDir;

pub struct InstallOpts {
    pub default_toolchain: String,
    pub no_modify_path: bool,
}

// The big installation messages. These are macros because the first
// argument of format! needs to be a literal.

macro_rules! pre_install_msg_template {
    ($platform_msg: expr) => {
concat!(
r"
Welcome to Rust!

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

WARNING: This is an early beta. Expect breakage.

To cancel installation, type "n", or for more options type "a",
then press the Enter key to continue.

Press the Enter key to install Rust."#
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
modifying the HKEY_CURRENT_USER/Environment/PATH registry key."
    )};
}

macro_rules! pre_install_msg_no_modify_path {
    () => {
pre_install_msg_template!(
"This path needs to be in your PATH environment variable,
but will not be added automatically."
    )};
}

macro_rules! post_install_msg_unix {
    () => {
r"Rust is installed now. Great!

To get started you need Cargo's bin directory in your `PATH`
environment variable. Next time you log in this will be done
automatically.

To configure your current shell run `source {cargo_home}/env`.
"
    };
}

macro_rules! post_install_msg_win {
    () => {
r"Rust is installed now. Great!

To get started you need Cargo's bin directory in your `PATH`
environment variable. Future applications will automatically have the
correct environment, but you may need to restart your current shell.
"
    };
}

macro_rules! post_install_msg_unix_no_modify_path {
    () => {
r"Rust is installed now. Great!

To get started you need Cargo's bin directory in your `PATH`
environment variable.

To configure your current shell run `source {cargo_home}/env`.
"
    };
}

macro_rules! post_install_msg_win_no_modify_path {
    () => {
r"Rust is installed now. Great!

To get started you need Cargo's bin directory in your `PATH`
environment variable. This has not been done automatically.
"
    };
}

macro_rules! pre_uninstall_msg {
    () => {
r"Thanks for hacking in Rust!

This will uninstall all Rust toolchains and data, and remove
`{cargo_home}/bin` from your `PATH` environment variable.

Continue? (y/N)"
    }
}

static TOOLS: &'static [&'static str]
    = &["rustup", "rustc", "rustdoc", "cargo", "rust-lldb", "rust-gdb"];

static UPDATE_ROOT: &'static str
    = "https://static.rust-lang.org/rustup/dist";

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
               opts: InstallOpts) -> Result<()> {

    let selected_opts;

    if !no_prompt {
        let ref msg = try!(pre_install_msg(opts.no_modify_path));
        println!("{}", msg);
        match try!(common::confirm_advanced(Confirm::Yes)) {
            Confirm::No => {
                info!("aborting installation");
                return Ok(());
            }
            Confirm::Yes => {
                selected_opts = opts;
            }
            Confirm::Advanced => {
                if let Some(opts) = try!(advanced_install(opts)) {
                    selected_opts = opts;
                } else {
                    info!("aborting installation");
                    return Ok(());
                }
            }
        }
    } else {
        selected_opts = opts;
    }

    let install_res: Result<()> = (|| {
        try!(cleanup_legacy());
        try!(install_bins());
        if !selected_opts.no_modify_path {
            try!(do_add_to_path(&get_add_path_methods()));
        }
        try!(maybe_install_rust(&selected_opts.default_toolchain, verbose));

        if cfg!(unix) {
            let ref env_file = try!(utils::cargo_home()).join("env");
            let ref env_str = format!(
                "{}\n\
                 printf \"PATH environment variable set.\\n\"
                 printf \"You're ready to Rust!\\n\\n\"",
                try!(shell_export_string()));
            try!(utils::write_file("env", env_file, env_str));
        }

        Ok(())
    })();

    if let Err(e) = install_res {
        err!("{}", e);

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
        if !selected_opts.no_modify_path {
            if cfg!(unix) {
                let cargo_home = try!(canonical_cargo_home());
                println!(post_install_msg_unix!(),
                         cargo_home = cargo_home);
            } else {
                println!(post_install_msg_win!());
            }
        } else {
            if cfg!(unix) {
                let cargo_home = try!(canonical_cargo_home());
                println!(post_install_msg_unix_no_modify_path!(),
                         cargo_home = cargo_home);
            } else {
                println!(post_install_msg_win_no_modify_path!());
            }
        }

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

// Interactive editing of the install options
fn advanced_install(mut opts: InstallOpts) -> Result<Option<InstallOpts>> {

    fn print_opts(opts: &InstallOpts) {
        println!(
            r"Selected installation options:

     default toolchain: {}
  modify PATH variable: {}
",
            opts.default_toolchain,
            if !opts.no_modify_path { "yes" } else { "no" }
            );
    }

    loop {

        print_opts(&opts);

        println!(
            "I'm going to ask you the value of each these installation options.\n\
             You may simply press the Enter key to accept the default.");

        println!("");

        opts.default_toolchain = try!(common::question_str(
            "Default toolchain? (stable/beta/nightly)",
            &opts.default_toolchain));

        opts.no_modify_path = !try!(common::question_bool(
            "Modify PATH variable? (y/n)",
            !opts.no_modify_path));

        println!("That's it! We're ready to install Rust.");
        println!("");

        print_opts(&opts);

        println!(
r#"To cancel installation, type "n", or for more options type "a",
then press the Enter key to continue.

Press the Enter key to install Rust. "#);

        match try!(common::confirm_advanced(Confirm::Yes)) {
            Confirm::No => {
                info!("aborting installation");
                return Ok(None);
            }
            Confirm::Yes => {
                return Ok(Some(opts));
            }
            Confirm::Advanced => {
                continue;
            }
        }
    }
}

// Before multirust-rs installed bins to $CARGO_HOME/bin it installed
// them to $RUSTUP_HOME/bin. If those bins continue to exist after
// upgrade and are on the $PATH, it would cause major confusion. This
// method silently deletes them.
fn cleanup_legacy() -> Result<()> {
    let legacy_bin_dir = try!(legacy_multirust_home_dir()).join("bin");

    for tool in TOOLS.iter().cloned().chain(Some("multirust")) {
        let ref file = legacy_bin_dir.join(&format!("{}{}", tool, EXE_SUFFIX));
        if file.exists() {
            try!(utils::remove_file("legacy-bin", file));
        }
    }

    return Ok(());

    #[cfg(unix)]
    fn legacy_multirust_home_dir() -> Result<PathBuf> {
        Ok(try!(utils::multirust_home()))
    }

    #[cfg(windows)]
    fn legacy_multirust_home_dir() -> Result<PathBuf> {
        use rustup_utils::raw::windows::{
            get_special_folder, FOLDERID_LocalAppData
        };

        Ok(get_special_folder(&FOLDERID_LocalAppData).unwrap_or(PathBuf::from(".")))
    }
}

fn install_bins() -> Result<()> {
    let ref bin_path = try!(utils::cargo_home()).join("bin");
    let ref this_exe_path = try!(utils::current_exe());
    let ref multirust_path = bin_path.join(&format!("multirust{}", EXE_SUFFIX));

    try!(utils::ensure_dir_exists("bin", bin_path, ntfy!(&NotifyHandler::none())));
    // NB: Even on Linux we can't just copy the new binary over the (running)
    // old binary; we must unlink it first.
    if multirust_path.exists() {
        try!(utils::remove_file("multirust-bin", multirust_path));
    }
    try!(utils::copy_file(this_exe_path, multirust_path));
    try!(utils::make_executable(multirust_path));

    // Hardlink all the Rust exes to the multirust exe. Using hardlinks
    // because they work on Windows.
    for tool in TOOLS {
        let ref tool_path = bin_path.join(&format!("{}{}", tool, EXE_SUFFIX));
        try!(utils::hardlink_file(multirust_path, tool_path))
    }

    Ok(())
}

fn maybe_install_rust(toolchain_str: &str, verbose: bool) -> Result<()> {
    let ref cfg = try!(common::set_globals(verbose));

    // If there is already an install, then `toolchain_str` may not be
    // a toolchain the user actually wants. Don't do anything.  FIXME:
    // This logic should be part of InstallOpts so that it isn't
    // possible to select a toolchain then have it not be installed.
    if try!(cfg.find_default()).is_none() {
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
    let ref cargo_home = try!(utils::cargo_home());

    if !cargo_home.join(&format!("bin/multirust{}", EXE_SUFFIX)).exists() {
        return Err(ErrorKind::NotSelfInstalled(cargo_home.clone()).into());
    }

    if !no_prompt {
        println!("");
        let ref msg = format!(pre_uninstall_msg!(),
                              cargo_home = try!(canonical_cargo_home()));
        if !try!(common::confirm(msg, false)) {
            info!("aborting uninstallation");
            return Ok(());
        }
    }

    info!("removing multirust home");

    // Delete RUSTUP_HOME
    let ref multirust_dir = try!(utils::multirust_home());
    if multirust_dir.exists() {
        try!(utils::remove_dir("multirust_home", multirust_dir, ntfy!(&NotifyHandler::none())));
    }

    let read_dir_err = "failure reading directory";

    info!("removing cargo home");

    // Remove CARGO_HOME/bin from PATH
    let ref remove_path_methods = try!(get_remove_path_methods());
    try!(do_remove_from_path(remove_path_methods));

    // Delete everything in CARGO_HOME *except* the multirust bin

    // First everything except the bin directory
    for dirent in try!(fs::read_dir(cargo_home).chain_err(|| read_dir_err)) {
        let dirent = try!(dirent.chain_err(|| read_dir_err));
        if dirent.file_name().to_str() != Some("bin") {
            if dirent.path().is_dir() {
                try!(utils::remove_dir("cargo_home", &dirent.path(), ntfy!(&NotifyHandler::none())));
            } else {
                try!(utils::remove_file("cargo_home", &dirent.path()));
            }
        }
    }

    // Then everything in bin except multirust and tools. These can't be unlinked
    // until this process exits (on windows).
    let tools = TOOLS.iter().map(|t| format!("{}{}", t, EXE_SUFFIX));
    let tools: Vec<_> = tools.chain(vec![format!("multirust{}", EXE_SUFFIX)]).collect();
    for dirent in try!(fs::read_dir(&cargo_home.join("bin")).chain_err(|| read_dir_err)) {
        let dirent = try!(dirent.chain_err(|| read_dir_err));
        let name = dirent.file_name();
        let file_is_tool = name.to_str().map(|n| tools.iter().any(|t| *t == n));
        if file_is_tool == Some(false) {
            if dirent.path().is_dir() {
                try!(utils::remove_dir("cargo_home", &dirent.path(), ntfy!(&NotifyHandler::none())));
            } else {
                try!(utils::remove_file("cargo_home", &dirent.path()));
            }
        }
    }

    info!("removing multirust binaries");

    // Delete multirust. This is tricky because this is *probably*
    // the running executable and on Windows can't be unlinked until
    // the process exits.
    try!(delete_multirust_and_cargo_home());

    info!("multirust is uninstalled");

    process::exit(0);
}

#[cfg(unix)]
fn delete_multirust_and_cargo_home() -> Result<()> {
    let ref cargo_home = try!(utils::cargo_home());
    try!(utils::remove_dir("cargo_home", cargo_home, ntfy!(&NotifyHandler::none())));

    Ok(())
}

// The last step of uninstallation is to delete *this binary*,
// multirust.exe and the CARGO_HOME that contains it. On Unix, this
// works fine. On Windows you can't delete files while they are open,
// like when they are running.
//
// Here's what we're going to do:
// - Copy multirust to a temporary file in
//   CARGO_HOME/../multirust-gc-$random.exe.
// - Open the gc exe with the FILE_FLAG_DELETE_ON_CLOSE and
//   FILE_SHARE_DELETE flags. This is going to be the last
//   file to remove, and the OS is going to do it for us.
//   This file is opened as inheritable so that subsequent
//   processes created with the option to inherit handles
//   will also keep them open.
// - Run the gc exe, which waits for the original multirust
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
fn delete_multirust_and_cargo_home() -> Result<()> {
    use rand;
    use scopeguard;

    // CARGO_HOME, hopefully empty except for bin/multirust.exe
    let ref cargo_home = try!(utils::cargo_home());
    // The multirust.exe bin
    let ref multirust_path = cargo_home.join(&format!("bin/multirust{}", EXE_SUFFIX));

    // The directory containing CARGO_HOME
    let work_path = cargo_home.parent().expect("CARGO_HOME doesn't have a parent?");

    // Generate a unique name for the files we're about to move out
    // of CARGO_HOME.
    let numbah: u32 = rand::random();
    let gc_exe = work_path.join(&format!("multirust-gc-{:x}.exe", numbah));

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
        // Copy multirust (probably this process's exe) to the gc exe
        try!(utils::copy_file(multirust_path, &gc_exe));

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
            return Err(ErrorKind::WindowsUninstallMadness.chained(err));
        }

        let _g = scopeguard::guard(gc_handle, |h| { let _ = CloseHandle(*h); });

        try!(Command::new(gc_exe).spawn()
             .chain_err(|| ErrorKind::WindowsUninstallMadness));

        // The catch 22 article says we must sleep here to give
        // Windows a chance to bump the processes file reference
        // count. acrichto though is in disbelief and *demanded* that
        // we not insert a sleep. If Windows failed to uninstall
        // correctly it is because of him.
    }

    Ok(())
}

/// Run by multirust-gc-$num.exe to delete CARGO_HOME
#[cfg(windows)]
pub fn complete_windows_uninstall() -> Result<()> {
    use rustup::NotifyHandler;
    use std::ffi::OsStr;
    use std::process::Stdio;

    try!(wait_for_parent());

    // Now that the parent has exited there are hopefully no more files open in CARGO_HOME
    let ref cargo_home = try!(utils::cargo_home());
    try!(utils::remove_dir("cargo_home", cargo_home, ntfy!(&NotifyHandler::none())));

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
            return Err(ErrorKind::WindowsUninstallMadness.chained(err));
        }

        let _g = scopeguard::guard(snapshot, |h| { let _ = CloseHandle(*h); });

        let mut entry: PROCESSENTRY32 = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32>() as DWORD;

        // Iterate over system processes looking for ours
        let success = Process32First(snapshot, &mut entry);
        if success == 0 {
            let err = io::Error::last_os_error();
            return Err(ErrorKind::WindowsUninstallMadness.chained(err));
        }

        let this_pid = GetCurrentProcessId();
        while entry.th32ProcessID != this_pid {
            let success = Process32Next(snapshot, &mut entry);
            if success == 0 {
                let err = io::Error::last_os_error();
                return Err(ErrorKind::WindowsUninstallMadness.chained(err));
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
            return Err(ErrorKind::WindowsUninstallMadness.chained(err));
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
            Err(ErrorKind::WindowsUninstallMadness.chained(e))
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

/// Self update downloads multirust-setup to CARGO_HOME/bin/multirust-setup
/// and runs it.
///
/// It does a few things to accomodate self-delete problems on windows:
///
/// multirust-setup is run in two stages, first with `--self-upgrade`,
/// which displays update messages and asks for confirmations, etc;
/// then with `--self-replace`, which replaces the multirust binary and
/// hardlinks. The last step is done without waiting for confirmation
/// on windows so that the running exe can be deleted.
///
/// Because it's again difficult for multirust-setup to delete itself
/// (and on windows this process will not be running to do it),
/// multirust-setup is stored in CARGO_HOME/bin, and then deleted next
/// time multirust runs.
pub fn update() -> Result<()> {

    let setup_path = try!(prepare_update());
    if let Some(ref p) = setup_path {
        info!("rustup updated successfully");
        try!(run_update(p));
    }

    Ok(())
}

pub fn prepare_update() -> Result<Option<PathBuf>> {
    let ref cargo_home = try!(utils::cargo_home());
    let ref multirust_path = cargo_home.join(&format!("bin/multirust{}", EXE_SUFFIX));
    let ref setup_path = cargo_home.join(&format!("bin/rustup-init{}", EXE_SUFFIX));

    if !multirust_path.exists() {
        return Err(ErrorKind::NotSelfInstalled(cargo_home.clone()).into());
    }

    if setup_path.exists() {
        try!(utils::remove_file("setup", setup_path));
    }

    // Get host triple
    let triple = dist::TargetTriple::from_host();

    let update_root = env::var("RUSTUP_UPDATE_ROOT")
        .unwrap_or(String::from(UPDATE_ROOT));

    let tempdir = try!(TempDir::new("multirust-update")
        .chain_err(|| "error creating temp directory"));

    // Get download URL
    let url = format!("{}/{}/rustup-init{}", update_root, triple, EXE_SUFFIX);

    // Calculate own hash
    let mut hasher = Hasher::new(Type::SHA256);
    try!(utils::tee_file("self", multirust_path, &mut hasher));
    let current_hash = hasher.finish()
                             .iter()
                             .map(|b| format!("{:02x}", b))
                             .join("");

    // Download latest hash
    info!("checking for self-updates");
    let hash_url = try!(utils::parse_url(&(url.clone() + ".sha256")));
    let hash_file = tempdir.path().join("hash");
    try!(utils::download_file(hash_url, &hash_file, None, ntfy!(&NotifyHandler::none())));
    let mut latest_hash = try!(utils::read_file("hash", &hash_file));
    latest_hash.truncate(64);

    // If up-to-date
    if latest_hash == current_hash {
        info!("rustup is up to date");
        return Ok(None);
    }

    // Get download path
    let download_url = try!(utils::parse_url(&url));

    // Download new version
    info!("downloading self-update");
    let mut hasher = Hasher::new(Type::SHA256);
    try!(utils::download_file(download_url,
                              &setup_path,
                              Some(&mut hasher),
                              ntfy!(&NotifyHandler::none())));
    let download_hash = hasher.finish()
                              .iter()
                              .map(|b| format!("{:02x}", b))
                              .join("");

    // Check that hash is correct
    if latest_hash != download_hash {
        return Err(ErrorKind::Dist(rustup_dist::ErrorKind::ChecksumFailed {
            url: url,
            expected: latest_hash,
            calculated: download_hash,
        }).into());
    }

    // Mark as executable
    try!(utils::make_executable(setup_path));

    Ok(Some(setup_path.to_owned()))
}

/// Tell the upgrader to replace the multirust bins, then delete
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
        return Err("self-updated failed to replace multirust executable".into());
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
/// CARGO_HOME/bin/multirust with the running exe, and updates the the
/// links to it. On windows this will run *after* the original
/// multirust process exits.
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
