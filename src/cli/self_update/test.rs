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

pub fn with_saved_path(f: &mut dyn FnMut()) {
    with_saved_global_state(get_path, restore_path, f)
}

#[cfg(windows)]
pub fn get_path() -> io::Result<Option<RegValue>> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let environment = root
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .unwrap();
    match environment.get_raw_value("PATH") {
        Ok(val) => Ok(Some(val)),
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
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

#[cfg(unix)]
pub fn get_path() -> io::Result<Option<()>> {
    Ok(None)
}

#[cfg(unix)]
fn restore_path(_: Option<()>) {}
