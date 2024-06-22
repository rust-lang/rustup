//! Support for functional tests.

#[cfg(windows)]
use std::{io, sync::Mutex};

#[cfg(windows)]
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    types::{FromRegValue, ToRegValue},
    RegKey, RegValue,
};

#[cfg(windows)]
pub fn with_saved_path(f: &mut dyn FnMut()) {
    with_saved_reg_value(&USER_PATH, f)
}

#[cfg(unix)]
pub fn with_saved_path(f: &mut dyn FnMut()) {
    f()
}

#[cfg(windows)]
pub fn get_path() -> io::Result<Option<RegValue>> {
    USER_PATH.get()
}

#[cfg(windows)]
pub fn with_saved_reg_value(id: &RegistryValueId, f: &mut dyn FnMut()) {
    // Lock protects concurrent mutation of registry
    static LOCK: Mutex<()> = Mutex::new(());
    let _g = LOCK.lock();

    // Save and restore the global state here to keep from trashing things.
    let saved_state = id
        .get()
        .expect("Error getting global state: Better abort to avoid trashing it");
    let _g = scopeguard::guard(saved_state, |p| id.set(p.as_ref()).unwrap());

    f();
}

#[cfg(windows)]
const USER_PATH: RegistryValueId = RegistryValueId {
    sub_key: "Environment",
    value_name: "PATH",
};

#[cfg(windows)]
pub struct RegistryValueId {
    pub sub_key: &'static str,
    pub value_name: &'static str,
}

#[cfg(windows)]
impl RegistryValueId {
    pub fn get_value<T: FromRegValue>(&self) -> io::Result<Option<T>> {
        self.get()?.map(|v| T::from_reg_value(&v)).transpose()
    }

    pub fn get(&self) -> io::Result<Option<RegValue>> {
        let sub_key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(self.sub_key, KEY_READ | KEY_WRITE)?;
        match sub_key.get_raw_value(self.value_name) {
            Ok(val) => Ok(Some(val)),
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn set_value(&self, new: Option<impl ToRegValue>) -> io::Result<()> {
        self.set(new.map(|s| s.to_reg_value()).as_ref())
    }

    pub fn set(&self, new: Option<&RegValue>) -> io::Result<()> {
        let sub_key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(self.sub_key, KEY_READ | KEY_WRITE)?;
        match new {
            Some(new) => sub_key.set_raw_value(self.value_name, new),
            None => sub_key.delete_value(self.value_name),
        }
    }
}
