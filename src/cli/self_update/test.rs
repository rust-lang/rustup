//! Support for functional tests.

use std::{io, sync::Mutex};

#[cfg(windows)]
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    RegKey, RegValue,
};

/// Support testing of code that mutates global state
pub fn with_saved_global_state<S>(
    getter: impl Fn() -> io::Result<S>,
    setter: impl Fn(S),
    f: &mut dyn FnMut(),
) {
    // Lock protects concurrent mutation of registry
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock();

    // Save and restore the global state here to keep from trashing things.
    let saved_state =
        getter().expect("Error getting global state: Better abort to avoid trashing it");
    let _g = scopeguard::guard(saved_state, setter);

    f();
}

#[cfg(windows)]
pub fn with_saved_path(f: &mut dyn FnMut()) {
    with_saved_reg_value(&RegKey::predef(HKEY_CURRENT_USER), "Environment", "PATH", f)
}

#[cfg(unix)]
pub fn with_saved_path(f: &mut dyn FnMut()) {
    f()
}

#[cfg(windows)]
pub fn get_path() -> io::Result<Option<RegValue>> {
    get_reg_value(&RegKey::predef(HKEY_CURRENT_USER), "Environment", "PATH")
}

#[cfg(windows)]
pub fn with_saved_reg_value(root: &RegKey, subkey: &str, name: &str, f: &mut dyn FnMut()) {
    with_saved_global_state(
        || get_reg_value(root, subkey, name),
        |p| restore_reg_value(root, subkey, name, p),
        f,
    )
}

#[cfg(windows)]
fn get_reg_value(root: &RegKey, subkey: &str, name: &str) -> io::Result<Option<RegValue>> {
    let subkey = root.open_subkey_with_flags(subkey, KEY_READ | KEY_WRITE)?;
    match subkey.get_raw_value(name) {
        Ok(val) => Ok(Some(val)),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(windows)]
fn restore_reg_value(root: &RegKey, subkey: &str, name: &str, p: Option<RegValue>) {
    let subkey = root
        .open_subkey_with_flags(subkey, KEY_READ | KEY_WRITE)
        .unwrap();
    if let Some(p) = p.as_ref() {
        subkey.set_raw_value(name, p).unwrap();
    } else {
        let _ = subkey.delete_value(name);
    }
}
