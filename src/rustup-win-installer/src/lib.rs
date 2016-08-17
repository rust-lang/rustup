#![allow(non_snake_case)]

extern crate winapi;
extern crate rustup;

use std::ffi::CString;
use std::path::PathBuf;
use ::winapi::{HRESULT, PCSTR, UINT, LPCWSTR, LPWSTR, LPVOID};

pub type MSIHANDLE = u32;

pub const LOGMSG_TRACEONLY: i32 = 0;
pub const LOGMSG_VERBOSE: i32 = 1;
pub const LOGMSG_STANDARD: i32 = 2;

// TODO: share this with self_update.rs
static TOOLS: &'static [&'static str]
    = &["rustc", "rustdoc", "cargo", "rust-lldb", "rust-gdb"];

#[no_mangle]
/// This is run as an `immediate` action early in the install sequence
pub unsafe extern "system" fn RustupSetInstallLocation(hInstall: MSIHANDLE) -> UINT {
    // TODO: error handling (get rid of unwrap)
    let name = CString::new("RustupSetInstallLocation").unwrap();
    let hr = WcaInitialize(hInstall, name.as_ptr());
    //let path = ::rustup::utils::cargo_home().unwrap();
    let path = PathBuf::from(::std::env::var_os("USERPROFILE").unwrap()).join(".rustup-test");
    set_property("RustupInstallLocation", path.to_str().unwrap());
    WcaFinalize(hr)
}

#[no_mangle]
/// This is be run as a `deferred` action after `InstallFiles` on install and upgrade
pub unsafe extern "system" fn RustupInstall(hInstall: MSIHANDLE) -> UINT {
    let name = CString::new("RustupInstall").unwrap();
    let hr = WcaInitialize(hInstall, name.as_ptr());
    // For deferred custom actions, all data must be passed through the `CustomActionData` property
    let custom_action_data = get_property("CustomActionData");
    // TODO: use rustup_utils::cargo_home() or pass through CustomActionData
    let path = PathBuf::from(::std::env::var_os("USERPROFILE").unwrap()).join(".rustup-test");
    let bin_path = path.join("bin");
    let rustup_path = bin_path.join("rustup.exe");
    let exe_installed = rustup_path.exists();
    log(&format!("Hello World from RustupInstall, confirming that rustup.exe has been installed: {}! CustomActionData: {}", exe_installed, custom_action_data));
    for tool in TOOLS {
        let ref tool_path = bin_path.join(&format!("{}.exe", tool));
        ::rustup::utils::hardlink_file(&rustup_path, tool_path);
    }
    // TODO: install default toolchain and report progress to UI
    WcaFinalize(hr)
}

#[no_mangle]
/// This is be run as a `deferred` action after `RemoveFiles` on uninstall (not on upgrade!)
pub unsafe extern "system" fn RustupUninstall(hInstall: MSIHANDLE) -> UINT {
    let name = CString::new("RustupUninstall").unwrap();
    let hr = WcaInitialize(hInstall, name.as_ptr());
    // For deferred custom actions, all data must be passed through the `CustomActionData` property
    let custom_action_data = get_property("CustomActionData");
    // TODO: use rustup_utils::cargo_home() or pass through CustomActionData
    let path = PathBuf::from(::std::env::var_os("USERPROFILE").unwrap()).join(".rustup-test");
    let exe_deleted = !path.join("bin").join("rustup.exe").exists();
    log(&format!("Hello World from RustupUninstall, confirming that rustup.exe has been deleted: {}! CustomActionData: {}", exe_deleted, custom_action_data));
    // TODO: Remove .cargo and .multirust
    ::rustup::utils::remove_dir("rustup-test", &path, &|_| {});
    WcaFinalize(hr)
}

// wrapper for WcaGetProperty (TODO: error handling)
fn get_property(name: &str) -> String {
    let encoded_name = to_wide_chars(name);
    let mut result_ptr = std::ptr::null_mut();
    unsafe { WcaGetProperty(encoded_name.as_ptr(), &mut result_ptr) };
    let result = from_wide_ptr(result_ptr);
    unsafe { StrFree(result_ptr as LPVOID) };
    result
}

// wrapper for WcaSetProperty
fn set_property(name: &str, value: &str) -> HRESULT {
    let encoded_name = to_wide_chars(name);
    let encoded_value = to_wide_chars(value);
    unsafe { WcaSetProperty(encoded_name.as_ptr(), encoded_value.as_ptr()) }
}


fn log(message: &str) {
    let msg = CString::new(message).unwrap();
    unsafe { WcaLog(LOGMSG_STANDARD, msg.as_ptr()) }
}
fn from_wide_ptr(ptr: *const u16) -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    unsafe {
        assert!(!ptr.is_null());
        let len = (0..std::isize::MAX).position(|i| *ptr.offset(i) == 0).unwrap();
        let slice = std::slice::from_raw_parts(ptr, len);
        OsString::from_wide(slice).to_string_lossy().into_owned()
    }
}

fn to_wide_chars(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(Some(0).into_iter()).collect::<Vec<_>>()
}

extern "system" {
    fn WcaInitialize(hInstall: MSIHANDLE, szCustomActionLogName: PCSTR) -> HRESULT;
    fn WcaFinalize(iReturnValue: HRESULT) -> UINT;
    fn WcaGetProperty(wzProperty: LPCWSTR, ppwzData: *mut LPWSTR) -> HRESULT; // see documentation for MsiGetProperty
    fn WcaSetProperty(wzPropertyName: LPCWSTR, wzPropertyValue: LPCWSTR) -> HRESULT;
    fn StrFree(p: LPVOID) -> HRESULT;
}

extern "cdecl" {
    fn WcaLog(llv: i32, fmt: PCSTR);
}