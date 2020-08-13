//! Support for functional tests.

use std::sync::Mutex;

use lazy_static::lazy_static;
#[cfg(not(unix))]
use winreg::{
    enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    RegKey, RegValue,
};

#[cfg(not(unix))]
use crate::utils::utils;

#[cfg(not(unix))]
pub fn get_path() -> Option<String> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .unwrap();
    // XXX: copied from the mock support crate, but I am suspicous of this
    // code: This uses ok to allow signalling None for 'delete', but this
    // can fail e.g. with !(winerror::ERROR_BAD_FILE_TYPE) or other
    // failures; which will lead to attempting to delete the users path
    // rather than aborting the test suite.
    environment.get_value("PATH").ok()
}

#[cfg(not(unix))]
fn restore_path(p: Option<String>) {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .unwrap();
    if let Some(p) = p.as_ref() {
        let reg_value = RegValue {
            bytes: utils::string_to_winreg_bytes(&p),
            vtype: RegType::REG_EXPAND_SZ,
        };
        environment.set_raw_value("PATH", &reg_value).unwrap();
    } else {
        let _ = environment.delete_value("PATH");
    }
}

/// Support testing of code that mutates global path state
pub fn with_saved_path(f: &dyn Fn()) {
    // Lock protects concurrent mutation of registry
    lazy_static! {
        static ref LOCK: Mutex<()> = Mutex::new(());
    }
    let _g = LOCK.lock();

    // On windows these tests mess with the user's PATH. Save
    // and restore them here to keep from trashing things.
    let saved_path = get_path();
    let _g = scopeguard::guard(saved_path, restore_path);

    f();
}

#[cfg(unix)]
pub fn get_path() -> Option<String> {
    None
}

#[cfg(unix)]
fn restore_path(_: Option<String>) {}
