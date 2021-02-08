//! Support for functional tests.

use std::sync::Mutex;

use lazy_static::lazy_static;
#[cfg(not(unix))]
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    RegKey, RegValue,
};

#[cfg(not(unix))]
pub fn get_path() -> Option<RegValue> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .unwrap();
    match environment.get_raw_value("PATH") {
        Ok(val) => Some(val),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!(
            "Error getting PATH: {}\nBetter abort to avoid trashing it.",
            e
        ),
    }
}

#[cfg(not(unix))]
fn restore_path(p: Option<RegValue>) {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .unwrap();
    if let Some(p) = p.as_ref() {
        environment.set_raw_value("PATH", &p).unwrap();
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
pub fn get_path() -> Option<()> {
    None
}

#[cfg(unix)]
fn restore_path(_: Option<()>) {}
