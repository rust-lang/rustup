//! Support for functional tests.

use std::sync::Mutex;

#[cfg(windows)]
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    RegKey, RegValue,
};

#[cfg(windows)]
pub fn get_path() -> std::io::Result<Option<RegValue>> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .unwrap();
    match environment.get_raw_value("PATH") {
        Ok(val) => Ok(Some(val)),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(windows)]
fn restore_path(p: Option<RegValue>) {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .unwrap();
    if let Some(p) = p.as_ref() {
        environment.set_raw_value("PATH", p).unwrap();
    } else {
        let _ = environment.delete_value("PATH");
    }
}

/// Support testing of code that mutates global path state
pub fn with_saved_path(f: &mut dyn FnMut()) {
    // Lock protects concurrent mutation of registry
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock();

    // On windows these tests mess with the user's PATH. Save
    // and restore them here to keep from trashing things.
    let saved_path = get_path().expect("Error getting PATH: Better abort to avoid trashing it.");
    let _g = scopeguard::guard(saved_path, restore_path);

    f();
}

#[cfg(unix)]
pub fn get_path() -> std::io::Result<Option<()>> {
    Ok(None)
}

#[cfg(unix)]
fn restore_path(_: Option<()>) {}
