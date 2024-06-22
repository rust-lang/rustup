//! Support for functional tests.

#[cfg(windows)]
use std::{
    io,
    sync::{LockResult, Mutex, MutexGuard},
};

#[cfg(windows)]
use winreg::{
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
    types::{FromRegValue, ToRegValue},
    RegKey, RegValue,
};

#[cfg(windows)]
pub fn get_path() -> io::Result<Option<RegValue>> {
    USER_PATH.get()
}

#[cfg(windows)]
pub struct RegistryGuard<'a> {
    _locked: LockResult<MutexGuard<'a, ()>>,
    id: &'static RegistryValueId,
    prev: Option<RegValue>,
}

#[cfg(windows)]
impl<'a> RegistryGuard<'a> {
    pub fn new(id: &'static RegistryValueId) -> io::Result<Self> {
        Ok(Self {
            _locked: REGISTRY_LOCK.lock(),
            id,
            prev: id.get()?,
        })
    }
}

#[cfg(windows)]
impl<'a> Drop for RegistryGuard<'a> {
    fn drop(&mut self) {
        self.id.set(self.prev.as_ref()).unwrap();
    }
}

#[cfg(windows)]
static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

#[cfg(windows)]
pub const USER_PATH: RegistryValueId = RegistryValueId {
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
