use std::env::{consts::EXE_SUFFIX, split_paths};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::Write;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::Path;
use std::process::Command;
use std::slice;
use std::sync::{Arc, Mutex};
#[cfg(any(test, feature = "test"))]
use std::sync::{LockResult, MutexGuard};

use anyhow::{anyhow, Context, Result};
use tracing::{info, warn};
use windows_registry::{Key, Type, Value, CURRENT_USER};
use windows_result::HRESULT;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;

use super::super::errors::*;
use super::common;
use super::{install_bins, report_error, InstallOpts};
use crate::cli::{download_tracker::DownloadTracker, markdown::md};
use crate::dist::TargetTriple;
use crate::process::{terminalsource::ColorableTerminal, Process};
use crate::utils::utils;
use crate::utils::Notification;

pub(crate) fn ensure_prompt(process: &Process) -> Result<()> {
    writeln!(process.stdout().lock(),)?;
    writeln!(process.stdout().lock(), "Press the Enter key to continue.")?;
    common::read_line(process)?;
    Ok(())
}

fn choice(max: u8, process: &Process) -> Result<Option<u8>> {
    write!(process.stdout().lock(), ">")?;

    let _ = std::io::stdout().flush();
    let input = common::read_line(process)?;

    let r = match str::parse(&input) {
        Ok(n) if n <= max => Some(n),
        _ => None,
    };

    writeln!(process.stdout().lock())?;
    Ok(r)
}

pub(crate) fn choose_vs_install(process: &Process) -> Result<Option<VsInstallPlan>> {
    writeln!(
        process.stdout().lock(),
        "\n1) Quick install via the Visual Studio Community installer"
    )?;
    writeln!(
        process.stdout().lock(),
        "   (free for individuals, academic uses, and open source)."
    )?;
    writeln!(
        process.stdout().lock(),
        "\n2) Manually install the prerequisites"
    )?;
    writeln!(
        process.stdout().lock(),
        "   (for enterprise and advanced users)."
    )?;
    writeln!(
        process.stdout().lock(),
        "\n3) Don't install the prerequisites"
    )?;
    writeln!(
        process.stdout().lock(),
        "   (if you're targeting the GNU ABI).\n"
    )?;

    let choice = loop {
        if let Some(n) = choice(3, process)? {
            break n;
        }
        writeln!(process.stdout().lock(), "Select option 1, 2 or 3")?;
    };
    let plan = match choice {
        1 => Some(VsInstallPlan::Automatic),
        2 => Some(VsInstallPlan::Manual),
        _ => None,
    };
    Ok(plan)
}

pub(super) async fn maybe_install_msvc(
    term: &mut ColorableTerminal,
    no_prompt: bool,
    quiet: bool,
    opts: &InstallOpts<'_>,
    process: &Process,
) -> Result<()> {
    let Some(plan) = do_msvc_check(opts, process) else {
        return Ok(());
    };

    if no_prompt {
        warn!("installing msvc toolchain without its prerequisites");
    } else if !quiet && plan == VsInstallPlan::Automatic {
        md(term, MSVC_AUTO_INSTALL_MESSAGE);
        match choose_vs_install(process)? {
            Some(VsInstallPlan::Automatic) => {
                match try_install_msvc(opts, process).await {
                    Err(e) => {
                        // Make sure the console doesn't exit before the user can
                        // see the error and give the option to continue anyway.
                        report_error(&e, process);
                        if !common::question_bool("\nContinue?", false, process)? {
                            info!("aborting installation");
                        }
                    }
                    Ok(ContinueInstall::No) => ensure_prompt(process)?,
                    _ => {}
                }
            }
            Some(VsInstallPlan::Manual) => {
                md(term, MSVC_MANUAL_INSTALL_MESSAGE);
                if !common::question_bool("\nContinue?", false, process)? {
                    info!("aborting installation");
                }
            }
            None => {}
        }
    } else {
        md(term, MSVC_MESSAGE);
        md(term, MSVC_MANUAL_INSTALL_MESSAGE);
        if !common::question_bool("\nContinue?", false, process)? {
            info!("aborting installation");
        }
    }

    Ok(())
}

static MSVC_MESSAGE: &str = r#"# Rust Visual C++ prerequisites

Rust requires the Microsoft C++ build tools for Visual Studio 2017 or
later, but they don't seem to be installed.

"#;

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

static MSVC_AUTO_INSTALL_MESSAGE: &str = r#"# Rust Visual C++ prerequisites

Rust requires a linker and Windows API libraries but they don't seem to be available.

These components can be acquired through a Visual Studio installer.

"#;

#[derive(PartialEq, Eq)]
pub(crate) enum VsInstallPlan {
    Automatic,
    Manual,
}

// Provide guidance about setting up MSVC if it doesn't appear to be
// installed
pub(crate) fn do_msvc_check(opts: &InstallOpts<'_>, process: &Process) -> Option<VsInstallPlan> {
    // Test suite skips this since it's env dependent
    if process.var("RUSTUP_INIT_SKIP_MSVC_CHECK").is_ok() {
        return None;
    }

    use cc::windows_registry;
    let host_triple = if let Some(trip) = opts.default_host_triple.as_ref() {
        trip.to_owned()
    } else {
        TargetTriple::from_host_or_build(process).to_string()
    };
    let installing_msvc = host_triple.contains("msvc");
    let have_msvc = windows_registry::find_tool(&host_triple, "cl.exe").is_some();
    if installing_msvc && !have_msvc {
        // Visual Studio build tools are required.
        // If the user does not have Visual Studio installed and their host
        // machine is i686 or x86_64 then it's OK to try an auto install.
        // Otherwise a manual install will be required.
        let has_any_vs = windows_registry::find_vs_version().is_ok();
        let is_x86 = host_triple.contains("i686") || host_triple.contains("x86_64");
        if is_x86 && !has_any_vs {
            Some(VsInstallPlan::Automatic)
        } else {
            Some(VsInstallPlan::Manual)
        }
    } else {
        None
    }
}

#[derive(Debug, Eq, PartialEq)]
struct VsInstallError(i32);
impl std::error::Error for VsInstallError {}
impl fmt::Display for VsInstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // See https://docs.microsoft.com/en-us/visualstudio/install/use-command-line-parameters-to-install-visual-studio?view=vs-2022#error-codes
        let message = match self.0 {
            740 => "elevation required",
            1001 => "Visual Studio installer process is running",
            1003 => "Visual Studio is in use",
            1602 => "operation was canceled",
            1618 => "another installation running",
            1641 => "operation completed successfully, and reboot was initiated",
            3010 => "operation completed successfully, but install requires reboot before it can be used",
            5003 => "bootstrapper failed to download installer",
            5004 => "operation was canceled",
            5005 => "bootstrapper command-line parse error",
            5007 => "operation was blocked - the computer does not meet the requirements",
            8001 => "arm machine check failure",
            8002 => "background download precheck failure",
            8003 => "out of support selectable failure",
            8004 => "target directory failure",
            8005 => "verifying source payloads failure",
            8006 => "Visual Studio processes running",
            -1073720687 => "connectivity failure",
            -1073741510 => "Microsoft Visual Studio Installer was terminated",
            _ => "error installing Visual Studio"
        };
        write!(f, "{} (exit code {})", message, self.0)
    }
}
impl VsInstallError {
    const REBOOTING_NOW: Self = Self(1641);
    const REBOOT_REQUIRED: Self = Self(3010);
}

pub(crate) enum ContinueInstall {
    Yes,
    No,
}

/// Tries to install the needed Visual Studio components.
///
/// Returns `Ok(ContinueInstall::No)` if installing Visual Studio was successful
/// but the rustup install should not be continued at this time.
pub(crate) async fn try_install_msvc(
    opts: &InstallOpts<'_>,
    process: &Process,
) -> Result<ContinueInstall> {
    // download the installer
    let visual_studio_url = utils::parse_url("https://aka.ms/vs/17/release/vs_community.exe")?;

    let tempdir = tempfile::Builder::new()
        .prefix("rustup-visualstudio")
        .tempdir()
        .context("error creating temp directory")?;

    let visual_studio = tempdir.path().join("vs_setup.exe");
    let download_tracker = Arc::new(Mutex::new(DownloadTracker::new_with_display_progress(
        true, process,
    )));
    download_tracker.lock().unwrap().download_finished();

    info!("downloading Visual Studio installer");
    utils::download_file(
        &visual_studio_url,
        &visual_studio,
        None,
        &move |n| {
            download_tracker.lock().unwrap().handle_notification(
                &crate::notifications::Notification::Install(crate::dist::Notification::Utils(n)),
            );
        },
        process,
    )
    .await?;

    // Run the installer. Arguments are documented at:
    // https://docs.microsoft.com/en-us/visualstudio/install/use-command-line-parameters-to-install-visual-studio
    let mut cmd = Command::new(visual_studio);
    cmd.arg("--wait")
        // Display an interactive GUI focused on installing just the selected components.
        .arg("--focusedUi")
        // Add the English language pack
        .args(["--addProductLang", "En-us"])
        // Add the linker and C runtime libraries.
        .args(["--add", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"]);

    // It's possible an earlier or later version of the Windows SDK has been
    // installed separately from Visual Studio so installing it can be skipped.
    if !has_windows_sdk_libs(process) {
        cmd.args([
            "--add",
            "Microsoft.VisualStudio.Component.Windows11SDK.22000",
        ]);
    }
    info!("running the Visual Studio install");
    info!("rustup will continue once Visual Studio installation is complete\n");
    let exit_status = cmd
        .spawn()
        .and_then(|mut child| child.wait())
        .context("error running Visual Studio installer")?;

    if exit_status.success() {
        Ok(ContinueInstall::Yes)
    } else {
        match VsInstallError(exit_status.code().unwrap()) {
            err @ VsInstallError::REBOOT_REQUIRED => {
                // A reboot is required but the user opted to delay it.
                warn!("{}", err);
                Ok(ContinueInstall::Yes)
            }
            err @ VsInstallError::REBOOTING_NOW => {
                // The user is wanting to reboot right now, so we should
                // not continue the install.
                warn!("{}", err);
                info!("\nRun rustup-init after restart to continue install");
                Ok(ContinueInstall::No)
            }
            err => {
                // It's possible that the installer returned a non-zero exit code
                // even though the required components were successfully installed.
                // In that case we warn about the error but continue on.
                let have_msvc = do_msvc_check(opts, process).is_none();
                let has_libs = has_windows_sdk_libs(process);
                if have_msvc && has_libs {
                    warn!("Visual Studio is installed but a problem occurred during installation");
                    warn!("{}", err);
                    Ok(ContinueInstall::Yes)
                } else {
                    Err(err).context("failed to install Visual Studio")
                }
            }
        }
    }
}

fn has_windows_sdk_libs(process: &Process) -> bool {
    if let Some(paths) = process.var_os("lib") {
        for mut path in split_paths(&paths) {
            path.push("kernel32.lib");
            if path.exists() {
                return true;
            }
        }
    };
    false
}

/// Run by rustup-gc-$num.exe to delete CARGO_HOME
#[tracing::instrument(level = "trace")]
pub fn complete_windows_uninstall(process: &Process) -> Result<utils::ExitCode> {
    use std::process::Stdio;

    wait_for_parent()?;

    // Now that the parent has exited there are hopefully no more files open in CARGO_HOME
    let cargo_home = process.cargo_home()?;
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
        .context(CLIError::WindowsUninstallMadness)?;

    Ok(utils::ExitCode(0))
}

pub(crate) fn wait_for_parent() -> Result<()> {
    use std::io;
    use std::mem;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
    use windows_sys::Win32::Storage::FileSystem::SYNCHRONIZE;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, WaitForSingleObject, INFINITE,
    };

    unsafe {
        // Take a snapshot of system processes, one of which is ours
        // and contains our parent's pid
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).context(CLIError::WindowsUninstallMadness);
        }

        let snapshot = scopeguard::guard(snapshot, |h| {
            let _ = CloseHandle(h);
        });

        let mut entry: PROCESSENTRY32 = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32>() as u32;

        // Iterate over system processes looking for ours
        let success = Process32First(*snapshot, &mut entry);
        if success == 0 {
            let err = io::Error::last_os_error();
            return Err(err).context(CLIError::WindowsUninstallMadness);
        }

        let this_pid = GetCurrentProcessId();
        while entry.th32ProcessID != this_pid {
            let success = Process32Next(*snapshot, &mut entry);
            if success == 0 {
                let err = io::Error::last_os_error();
                return Err(err).context(CLIError::WindowsUninstallMadness);
            }
        }

        // FIXME: Using the process ID exposes a race condition
        // wherein the parent process already exited and the OS
        // reassigned its ID.
        let parent_id = entry.th32ParentProcessID;

        // Get a handle to the parent process
        let parent = OpenProcess(SYNCHRONIZE, 0, parent_id);
        if parent.is_null() {
            // This just means the parent has already exited.
            return Ok(());
        }

        let parent = scopeguard::guard(parent, |h| {
            let _ = CloseHandle(h);
        });

        // Wait for our parent to exit
        let res = WaitForSingleObject(*parent, INFINITE);

        if res != WAIT_OBJECT_0 {
            let err = io::Error::last_os_error();
            return Err(err).context(CLIError::WindowsUninstallMadness);
        }
    }

    Ok(())
}

pub(crate) fn do_add_to_path(process: &Process) -> Result<()> {
    let new_path = _with_path_cargo_home_bin(_add_to_path, process)?;
    _apply_new_path(new_path)?;
    do_add_to_programs(process)
}

fn _apply_new_path(new_path: Option<Vec<u16>>) -> Result<()> {
    use std::ptr;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutA, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };

    let new_path = match new_path {
        Some(new_path) => new_path,
        None => return Ok(()), // No need to set the path
    };

    let environment = CURRENT_USER.create("Environment")?;

    if new_path.is_empty() {
        environment.remove_value("PATH")?;
    } else {
        environment.set_bytes("PATH", Type::ExpandString, &to_winreg_bytes(new_path))?;
    }

    // Tell other processes to update their environment
    #[allow(clippy::unnecessary_cast)]
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
// this returns None then the PATH variable is not a string and we
// should not mess with it.
fn get_windows_path_var() -> Result<Option<Vec<u16>>> {
    let environment = CURRENT_USER
        .create("Environment")
        .context("Failed opening Environment key")?;

    let reg_value = environment.get_value("PATH");
    match reg_value {
        Ok(val) => {
            if let Some(s) = from_winreg_value(val) {
                Ok(Some(s))
            } else {
                warn!(
                    "the registry key HKEY_CURRENT_USER\\Environment\\PATH is not a string. \
                       Not modifying the PATH variable"
                );
                Ok(None)
            }
        }
        Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND) => Ok(Some(Vec::new())),
        Err(e) => Err(e).context(CLIError::WindowsUninstallMadness),
    }
}

// Returns None if the existing old_path does not need changing, otherwise
// prepends the path_str to old_path, handling empty old_path appropriately.
fn _add_to_path(old_path: Vec<u16>, path_str: Vec<u16>) -> Option<Vec<u16>> {
    if old_path.is_empty() {
        Some(path_str)
    } else if old_path
        .windows(path_str.len())
        .any(|path| path == path_str)
    {
        None
    } else {
        let mut new_path = path_str;
        new_path.push(b';' as u16);
        new_path.extend_from_slice(&old_path);
        Some(new_path)
    }
}

// Returns None if the existing old_path does not need changing
fn _remove_from_path(old_path: Vec<u16>, path_str: Vec<u16>) -> Option<Vec<u16>> {
    let idx = old_path
        .windows(path_str.len())
        .position(|path| path == path_str)?;
    // If there's a trailing semicolon (likely, since we probably added one
    // during install), include that in the substring to remove. We don't search
    // for that to find the string, because if it's the last string in the path,
    // there may not be.
    let mut len = path_str.len();
    if old_path.get(idx + path_str.len()) == Some(&(b';' as u16)) {
        len += 1;
    }

    let mut new_path = old_path[..idx].to_owned();
    new_path.extend_from_slice(&old_path[idx + len..]);
    // Don't leave a trailing ; though, we don't want an empty string in the
    // path.
    if new_path.last() == Some(&(b';' as u16)) {
        new_path.pop();
    }
    Some(new_path)
}

fn _with_path_cargo_home_bin<F>(f: F, process: &Process) -> Result<Option<Vec<u16>>>
where
    F: FnOnce(Vec<u16>, Vec<u16>) -> Option<Vec<u16>>,
{
    let windows_path = get_windows_path_var()?;
    let mut path_str = process.cargo_home()?;
    path_str.push("bin");
    Ok(windows_path
        .and_then(|old_path| f(old_path, OsString::from(path_str).encode_wide().collect())))
}

pub(crate) fn do_remove_from_path(process: &Process) -> Result<()> {
    let new_path = _with_path_cargo_home_bin(_remove_from_path, process)?;
    _apply_new_path(new_path)?;
    do_remove_from_programs()
}

const RUSTUP_UNINSTALL_ENTRY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Rustup";

fn rustup_uninstall_reg_key() -> Result<Key> {
    CURRENT_USER
        .create(RUSTUP_UNINSTALL_ENTRY)
        .context("Failed creating uninstall key")
}

pub(crate) fn do_update_programs_display_version(version: &str) -> Result<()> {
    rustup_uninstall_reg_key()?
        .set_string("DisplayVersion", version)
        .context("Failed to set `DisplayVersion`")
}

pub(crate) fn do_add_to_programs(process: &Process) -> Result<()> {
    use std::path::PathBuf;

    let key = rustup_uninstall_reg_key()?;

    // Don't overwrite registry if Rustup is already installed
    let prev = key.get_value("UninstallString").map(from_winreg_value);
    if let Ok(Some(s)) = prev {
        let mut path = PathBuf::from(OsString::from_wide(&s));
        path.pop();
        if path.exists() {
            return Ok(());
        }
    }

    let mut path = process.cargo_home()?;
    path.push("bin\\rustup.exe");
    let mut uninstall_cmd = OsString::from("\"");
    uninstall_cmd.push(path);
    uninstall_cmd.push("\" self uninstall");

    key.set_bytes(
        "UninstallString",
        Type::String,
        &to_winreg_bytes(uninstall_cmd.encode_wide().collect()),
    )
    .context("Failed to set `UninstallString`")?;
    key.set_string("DisplayName", "Rustup: the Rust toolchain installer")
        .context("Failed to set `DisplayName`")?;
    do_update_programs_display_version(env!("CARGO_PKG_VERSION"))?;

    Ok(())
}

pub(crate) fn do_remove_from_programs() -> Result<()> {
    match CURRENT_USER.remove_tree(RUSTUP_UNINSTALL_ENTRY) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND) => Ok(()),
        Err(e) => Err(anyhow!(e)),
    }
}

/// Convert a vector UCS-2 chars to a null-terminated UCS-2 string in bytes
pub(crate) fn to_winreg_bytes(mut v: Vec<u16>) -> Vec<u8> {
    v.push(0);
    unsafe { slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 2).to_vec() }
}

/// This is used to decode the value of HKCU\Environment\PATH. If that key is
/// not REG_SZ | REG_EXPAND_SZ then this returns None. The winreg library itself
/// does a lossy unicode conversion.
pub(crate) fn from_winreg_value(val: Value) -> Option<Vec<u16>> {
    match val.ty() {
        Type::String | Type::ExpandString => {
            // Copied from winreg
            let mut words = unsafe {
                slice::from_raw_parts(val.as_ptr().cast::<u16>(), val.as_ref().len() / 2).to_owned()
            };
            while words.last() == Some(&0) {
                words.pop();
            }
            Some(words)
        }
        _ => None,
    }
}

pub(crate) fn run_update(setup_path: &Path) -> Result<utils::ExitCode> {
    Command::new(setup_path)
        .arg("--self-replace")
        .spawn()
        .context("unable to run updater")?;

    let Some(version) = super::get_and_parse_new_rustup_version(setup_path) else {
        warn!("failed to get the new rustup version in order to update `DisplayVersion`");
        return Ok(utils::ExitCode(1));
    };
    do_update_programs_display_version(&version)?;

    Ok(utils::ExitCode(0))
}

pub(crate) fn self_replace(process: &Process) -> Result<utils::ExitCode> {
    wait_for_parent()?;
    install_bins(process)?;

    Ok(utils::ExitCode(0))
}

// The last step of uninstallation is to delete *this binary*,
// rustup.exe and the CARGO_HOME that contains it. On Unix, this
// works fine. On Windows you can't delete files while they are open,
// like when they are running.
//
// Here's what we're going to do:
// - Copy rustup.exe to a temporary file in
//   CARGO_HOME/../rustup-gc-$random.exe.
// - Open the gc exe with the FILE_FLAG_DELETE_ON_CLOSE and
//   FILE_SHARE_DELETE flags. This is going to be the last
//   file to remove, and the OS is going to do it for us.
//   This file is opened as inheritable so that subsequent
//   processes created with the option to inherit handles
//   will also keep them open.
// - Run the gc exe, which waits for the original rustup.exe
//   process to close, then deletes CARGO_HOME. This process
//   has inherited a FILE_FLAG_DELETE_ON_CLOSE handle to itself.
// - Finally, spawn yet another system binary with the inherit handles
//   flag, so *it* inherits the FILE_FLAG_DELETE_ON_CLOSE handle to
//   the gc exe. If the gc exe exits before the system exe then at
//   last it will be deleted when the handle closes.
//
// This is the DELETE_ON_CLOSE method from
// https://www.catch22.net/tuts/win32/self-deleting-executables
//
// ... which doesn't actually work because Windows won't really
// delete a FILE_FLAG_DELETE_ON_CLOSE process when it exits.
//
// .. augmented with this SO answer
// https://stackoverflow.com/questions/10319526/understanding-a-self-deleting-program-in-c
pub(crate) fn delete_rustup_and_cargo_home(process: &Process) -> Result<()> {
    use std::io;
    use std::mem;
    use std::ptr;
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_DELETE_ON_CLOSE, FILE_SHARE_DELETE, FILE_SHARE_READ, OPEN_EXISTING,
    };

    // CARGO_HOME, hopefully empty except for bin/rustup.exe
    let cargo_home = process.cargo_home()?;
    // The rustup.exe bin
    let rustup_path = cargo_home.join(format!("bin/rustup{EXE_SUFFIX}"));

    // The directory containing CARGO_HOME
    let work_path = cargo_home
        .parent()
        .expect("CARGO_HOME doesn't have a parent?");

    // Generate a unique name for the files we're about to move out
    // of CARGO_HOME.
    let numbah: u32 = rand::random();
    let gc_exe = work_path.join(format!("rustup-gc-{numbah:x}.exe"));
    // Copy rustup (probably this process's exe) to the gc exe
    utils::copy_file(&rustup_path, &gc_exe)?;
    let gc_exe_win: Vec<_> = gc_exe.as_os_str().encode_wide().chain(Some(0)).collect();

    // Make the sub-process opened by gc exe inherit its attribute.
    let sa = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: 1,
    };

    let _g = unsafe {
        // Open an inheritable handle to the gc exe marked
        // FILE_FLAG_DELETE_ON_CLOSE.
        let gc_handle = CreateFileW(
            gc_exe_win.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            &sa,
            OPEN_EXISTING,
            FILE_FLAG_DELETE_ON_CLOSE,
            ptr::null_mut(),
        );

        if gc_handle == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).context(CLIError::WindowsUninstallMadness);
        }

        scopeguard::guard(gc_handle, |h| {
            let _ = CloseHandle(h);
        })
    };

    Command::new(gc_exe)
        .spawn()
        .context(CLIError::WindowsUninstallMadness)?;

    // The catch 22 article says we must sleep here to give
    // Windows a chance to bump the processes file reference
    // count. acrichto though is in disbelief and *demanded* that
    // we not insert a sleep. If Windows failed to uninstall
    // correctly it is because of him.

    // (.. and months later acrichto owes me a beer).
    thread::sleep(Duration::from_millis(100));

    Ok(())
}

#[cfg(any(test, feature = "test"))]
pub fn get_path() -> Result<Option<Value>> {
    USER_PATH.get()
}

#[cfg(any(test, feature = "test"))]
pub struct RegistryGuard<'a> {
    _locked: LockResult<MutexGuard<'a, ()>>,
    id: &'static RegistryValueId,
    prev: Option<Value>,
}

#[cfg(any(test, feature = "test"))]
impl<'a> RegistryGuard<'a> {
    pub fn new(id: &'static RegistryValueId) -> Result<Self> {
        Ok(Self {
            _locked: REGISTRY_LOCK.lock(),
            id,
            prev: id.get()?,
        })
    }
}

#[cfg(any(test, feature = "test"))]
impl<'a> Drop for RegistryGuard<'a> {
    fn drop(&mut self) {
        self.id.set(self.prev.as_ref()).unwrap();
    }
}

#[cfg(any(test, feature = "test"))]
static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

#[cfg(any(test, feature = "test"))]
pub const USER_PATH: RegistryValueId = RegistryValueId {
    sub_key: "Environment",
    value_name: "PATH",
};

#[cfg(any(test, feature = "test"))]
pub struct RegistryValueId {
    pub sub_key: &'static str,
    pub value_name: &'static str,
}

#[cfg(any(test, feature = "test"))]
impl RegistryValueId {
    pub fn get_value(&self) -> Result<Option<Value>> {
        self.get()
    }

    fn get(&self) -> Result<Option<Value>> {
        let sub_key = CURRENT_USER.create(self.sub_key)?;
        match sub_key.get_value(self.value_name) {
            Ok(val) => Ok(Some(val)),
            Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_value(&self, new: Option<&Value>) -> Result<()> {
        self.set(new)
    }

    fn set(&self, new: Option<&Value>) -> Result<()> {
        let sub_key = CURRENT_USER.create(self.sub_key)?;
        match new {
            Some(new) => Ok(sub_key.set_value(self.value_name, new)?),
            None => Ok(sub_key.remove_value(self.value_name)?),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::TestProcess;

    fn wide(str: &str) -> Vec<u16> {
        OsString::from(str).encode_wide().collect()
    }

    #[test]
    fn windows_install_does_not_add_path_twice() {
        assert_eq!(
            None,
            super::_add_to_path(
                wide(r"c:\users\example\.cargo\bin;foo"),
                wide(r"c:\users\example\.cargo\bin")
            )
        );
    }

    #[test]
    fn windows_handle_non_unicode_path() {
        let initial_path = vec![
            0xD800, // leading surrogate
            0x0101, // bogus trailing surrogate
            0x0000, // null
        ];
        let cargo_home = wide(r"c:\users\example\.cargo\bin");
        let final_path = [&cargo_home, &[b';' as u16][..], &initial_path].join(&[][..]);

        assert_eq!(
            &final_path,
            &super::_add_to_path(initial_path.clone(), cargo_home.clone(),).unwrap()
        );
        assert_eq!(
            &initial_path,
            &super::_remove_from_path(final_path, cargo_home,).unwrap()
        );
    }

    #[test]
    fn windows_path_regkey_type() {
        // per issue #261, setting PATH should use REG_EXPAND_SZ.
        let _guard = RegistryGuard::new(&USER_PATH);
        let environment = CURRENT_USER.create("Environment").unwrap();
        environment.remove_value("PATH").unwrap();

        {
            // Can't compare the Results as Eq isn't derived; thanks error-chain.
            #![allow(clippy::unit_cmp)]
            assert_eq!((), super::_apply_new_path(Some(wide("foo"))).unwrap());
        }
        let environment = CURRENT_USER.create("Environment").unwrap();
        let path = environment.get_value("PATH").unwrap();
        assert_eq!(path.ty(), Type::ExpandString);
        assert_eq!(super::to_winreg_bytes(wide("foo")), path.as_ref());
    }

    #[test]
    fn windows_path_delete_key_when_empty() {
        // during uninstall the PATH key may end up empty; if so we should
        // delete it.
        let _guard = RegistryGuard::new(&USER_PATH);
        let environment = CURRENT_USER.create("Environment").unwrap();
        environment
            .set_bytes(
                "PATH",
                Type::ExpandString,
                &super::to_winreg_bytes(wide("foo")),
            )
            .unwrap();

        {
            // Can't compare the Results as Eq isn't derived; thanks error-chain.
            #![allow(clippy::unit_cmp)]
            assert_eq!((), super::_apply_new_path(Some(Vec::new())).unwrap());
        }
        let reg_value = environment.get_value("PATH");
        match reg_value {
            Ok(_) => panic!("key not deleted"),
            Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND) => {}
            Err(e) => panic!("error {e}"),
        }
    }

    #[test]
    fn windows_doesnt_mess_with_a_non_string_path() {
        // This writes an error, so we want a sink for it.
        let tp = TestProcess::with_vars(
            [("HOME".to_string(), "/unused".to_string())]
                .iter()
                .cloned()
                .collect(),
        );

        let _guard = RegistryGuard::new(&USER_PATH);
        let environment = CURRENT_USER.create("Environment").unwrap();
        environment
            .set_bytes("PATH", Type::Bytes, &[0x12, 0x34])
            .unwrap();
        // Ok(None) signals no change to the PATH setting layer
        assert_eq!(
            None,
            super::_with_path_cargo_home_bin(|_, _| panic!("called"), &tp.process).unwrap()
        );

        assert_eq!(
            r"warn: the registry key HKEY_CURRENT_USER\Environment\PATH is not a string. Not modifying the PATH variable
",
            String::from_utf8(tp.stderr()).unwrap()
        );
    }

    #[test]
    fn windows_treat_missing_path_as_empty() {
        // during install the PATH key may be missing; treat it as empty
        let _guard = RegistryGuard::new(&USER_PATH);
        let environment = CURRENT_USER.create("Environment").unwrap();
        environment.remove_value("PATH").unwrap();

        assert_eq!(Some(Vec::new()), super::get_windows_path_var().unwrap());
    }

    #[test]
    fn windows_uninstall_removes_semicolon_from_path_prefix() {
        assert_eq!(
            wide("foo"),
            super::_remove_from_path(
                wide(r"c:\users\example\.cargo\bin;foo"),
                wide(r"c:\users\example\.cargo\bin"),
            )
            .unwrap()
        )
    }

    #[test]
    fn windows_uninstall_removes_semicolon_from_path_suffix() {
        assert_eq!(
            wide("foo"),
            super::_remove_from_path(
                wide(r"foo;c:\users\example\.cargo\bin"),
                wide(r"c:\users\example\.cargo\bin"),
            )
            .unwrap()
        )
    }
}
