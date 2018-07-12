#![allow(non_snake_case)]

extern crate rustup;
extern crate winapi;

use std::ffi::CString;
use std::path::PathBuf;
use std::collections::HashMap;

use winapi::shared::ntdef::{HRESULT, LPCWSTR, LPWSTR, PCSTR};
use winapi::shared::minwindef::{LPVOID, UINT};

pub type MSIHANDLE = u32;

pub const LOGMSG_TRACEONLY: i32 = 0;
pub const LOGMSG_VERBOSE: i32 = 1;
pub const LOGMSG_STANDARD: i32 = 2;

// TODO: share this with self_update.rs
static TOOLS: &'static [&'static str] = &[
    "rustc",
    "rustdoc",
    "cargo",
    "rust-lldb",
    "rust-gdb",
    "rls",
    "rustfmt",
    "cargo-fmt",
    "cargo-clippy",
];

#[no_mangle]
/// This is be run as a `deferred` action after `InstallFiles` on install and upgrade
pub unsafe extern "system" fn RustupInstall(hInstall: MSIHANDLE) -> UINT {
    let name = CString::new("RustupInstall").unwrap();
    let hr = WcaInitialize(hInstall, name.as_ptr());
    // For deferred custom actions, all data must be passed through the `CustomActionData` property
    let custom_action_data = get_property("CustomActionData");
    let parsed_ca_data = parse_custom_action_data(&custom_action_data);
    let path = PathBuf::from(parsed_ca_data.get("INSTALLLOCATION").unwrap());
    let bin_path = path.join("bin");
    let rustup_path = bin_path.join("rustup.exe");
    let exe_installed = rustup_path.exists();
    log(&format!("Hello World from RustupInstall, confirming that rustup.exe has been installed: {}! CustomActionData: {}", exe_installed, custom_action_data));
    log(&format!("Parsed CA data: {:?}", parsed_ca_data));
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
    let parsed_ca_data = parse_custom_action_data(&custom_action_data);
    let path = PathBuf::from(parsed_ca_data.get("INSTALLLOCATION").unwrap());
    let exe_deleted = !path.join("bin").join("rustup.exe").exists();
    log(&format!("Hello World from RustupUninstall, confirming that rustup.exe has been deleted: {}! CustomActionData: {}", exe_deleted, custom_action_data));
    log(&format!("Parsed CA data: {:?}", parsed_ca_data));
    ::rustup::utils::remove_dir("cargo_home", &path, &|_| {});
    // TODO: also remove RUSTUP_HOME
    //::rustup::utils::remove_dir("rustup_home", &rustup_home, &|_| {});
    WcaFinalize(hr)
}

fn parse_custom_action_data(ca_data: &str) -> HashMap<&str, &str> {
    let mut map = HashMap::new();
    for v in ca_data.split(";") {
        let idx = v.find('=').unwrap();
        map.insert(&v[..idx], &v[(idx + 1)..]);
    }
    map
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

fn log(message: &str) {
    let msg = CString::new(message).unwrap();
    unsafe { WcaLog(LOGMSG_STANDARD, msg.as_ptr()) }
}
fn from_wide_ptr(ptr: *const u16) -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    unsafe {
        assert!(!ptr.is_null());
        let len = (0..std::isize::MAX)
            .position(|i| *ptr.offset(i) == 0)
            .unwrap();
        let slice = std::slice::from_raw_parts(ptr, len);
        OsString::from_wide(slice).to_string_lossy().into_owned()
    }
}

fn to_wide_chars(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<_>>()
}

extern "system" {
    fn WcaInitialize(hInstall: MSIHANDLE, szCustomActionLogName: PCSTR) -> HRESULT;
    fn WcaFinalize(iReturnValue: HRESULT) -> UINT;
    fn WcaGetProperty(wzProperty: LPCWSTR, ppwzData: *mut LPWSTR) -> HRESULT; // see documentation for MsiGetProperty
    fn StrFree(p: LPVOID) -> HRESULT;
}

extern "cdecl" {
    fn WcaLog(llv: i32, fmt: PCSTR);
}
