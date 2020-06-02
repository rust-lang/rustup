use std::env;
use std::env::consts::EXE_SUFFIX;
use std::path::Path;
use std::process::{self, Command};

use super::super::errors::*;
use super::path_update::PathUpdateMethod;
use super::{install_bins, InstallOpts};
use crate::dist::dist::TargetTriple;
use crate::utils::utils;
use crate::utils::Notification;

// Provide guidance about setting up MSVC if it doesn't appear to be
// installed
pub fn do_msvc_check(opts: &InstallOpts<'_>) -> Result<bool> {
    // Test suite skips this since it's env dependent
    if env::var("RUSTUP_INIT_SKIP_MSVC_CHECK").is_ok() {
        return Ok(true);
    }

    use cc::windows_registry;
    let host_triple = if let Some(trip) = opts.default_host_triple.as_ref() {
        trip.to_owned()
    } else {
        TargetTriple::from_host_or_build().to_string()
    };
    let installing_msvc = host_triple.contains("msvc");
    let have_msvc = windows_registry::find_tool(&host_triple, "cl.exe").is_some();
    if installing_msvc && !have_msvc {
        return Ok(false);
    }

    Ok(true)
}

/// Run by rustup-gc-$num.exe to delete CARGO_HOME
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

pub fn wait_for_parent() -> Result<()> {
    use std::io;
    use std::mem;
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

        let snapshot = scopeguard::guard(snapshot, |h| {
            let _ = CloseHandle(h);
        });

        let mut entry: PROCESSENTRY32 = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32>() as DWORD;

        // Iterate over system processes looking for ours
        let success = Process32First(*snapshot, &mut entry);
        if success == 0 {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        let this_pid = GetCurrentProcessId();
        while entry.th32ProcessID != this_pid {
            let success = Process32Next(*snapshot, &mut entry);
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
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }
    }

    Ok(())
}

pub fn do_add_to_path(methods: &[PathUpdateMethod]) -> Result<()> {
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
        .into_owned();
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
// this returns None then the PATH variable is not unicode and we
// should not mess with it.
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

pub fn do_remove_from_path(methods: &[PathUpdateMethod]) -> Result<()> {
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
        .into_owned();
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

pub fn run_update(setup_path: &Path) -> Result<()> {
    Command::new(setup_path)
        .arg("--self-replace")
        .spawn()
        .chain_err(|| "unable to run updater")?;

    process::exit(0);
}

pub fn self_replace() -> Result<()> {
    wait_for_parent()?;
    install_bins()?;

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
// https://www.catch22.net/tuts/win32/self-deleting-executables
//
// ... which doesn't actually work because Windows won't really
// delete a FILE_FLAG_DELETE_ON_CLOSE process when it exits.
//
// .. augmented with this SO answer
// https://stackoverflow.com/questions/10319526/understanding-a-self-deleting-program-in-c
pub fn delete_rustup_and_cargo_home() -> Result<()> {
    use std::io;
    use std::mem;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use std::thread;
    use std::time::Duration;
    use winapi::shared::minwindef::DWORD;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
    use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
    use winapi::um::winbase::FILE_FLAG_DELETE_ON_CLOSE;
    use winapi::um::winnt::{FILE_SHARE_DELETE, FILE_SHARE_READ, GENERIC_READ};

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
    // Copy rustup (probably this process's exe) to the gc exe
    utils::copy_file(&rustup_path, &gc_exe)?;
    let gc_exe_win: Vec<_> = gc_exe.as_os_str().encode_wide().chain(Some(0)).collect();

    // Make the sub-process opened by gc exe inherit its attribute.
    let mut sa = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
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
            &mut sa,
            OPEN_EXISTING,
            FILE_FLAG_DELETE_ON_CLOSE,
            ptr::null_mut(),
        );

        if gc_handle == INVALID_HANDLE_VALUE {
            let err = io::Error::last_os_error();
            return Err(err).chain_err(|| ErrorKind::WindowsUninstallMadness);
        }

        scopeguard::guard(gc_handle, |h| {
            let _ = CloseHandle(h);
        })
    };

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

    Ok(())
}

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
pub fn get_add_path_methods() -> Vec<PathUpdateMethod> {
    vec![PathUpdateMethod::Windows]
}

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
pub fn get_remove_path_methods() -> Result<Vec<PathUpdateMethod>> {
    Ok(vec![PathUpdateMethod::Windows])
}
