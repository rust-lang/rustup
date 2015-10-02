use std::ffi::{OsStr, OsString};
use std::env;
use std::path::Path;

use errors::*;
use utils;

struct EnvVarSetting<'a, F> {
	key: &'a OsStr,
	old_value: Option<OsString>,
	f: Option<F>,
}

impl<'a, T, F: FnOnce() -> T> EnvVarSetting<'a, F> {
	fn new<V: AsRef<OsStr>>(k: &'a OsStr, v: V, f: F) -> Self {
		let old_value = env::var_os(k);
		env::set_var(k, v);
		
		EnvVarSetting {
			key: k,
			old_value: old_value,
			f: Some(f),
		}
	}
	fn call(&mut self) -> T {
		(self.f.take().unwrap())()
	}
}

impl<'a, F> Drop for EnvVarSetting<'a, F> {
	fn drop(&mut self) {
		if let Some(ref v) = self.old_value {
			env::set_var(self.key, v);
		} else {
			env::remove_var(self.key);
		}
	}
}

pub fn with<T, F: FnOnce() -> Result<T>>(name: &str, value: &OsStr, f: F) -> Result<T> {
	let mut s = EnvVarSetting::new(name.as_ref(), value, f);
	s.call()
}

pub fn with_default<T, F: FnOnce() -> Result<T>>(name: &str, value: &OsStr, f: F) -> Result<T> {
	let new_value = env::var_os(name)
		.and_then(utils::if_not_empty)
		.unwrap_or(value.to_owned());
	let mut s = EnvVarSetting::new(name.as_ref(), new_value, f);
	s.call()
}

pub fn with_path<T, F: FnOnce() -> Result<T>>(name: &str, value: &Path, f: F) -> Result<T> {
	let old_value = env::var_os(name);
	let mut parts = vec![value.to_owned()];
	if let Some(ref v) = old_value {
		parts.extend(env::split_paths(v));
	}
	let new_value = try!(env::join_paths(parts)
		.map_err(|_| Error::InvalidEnvironment));
	
	let mut s = EnvVarSetting::new(name.as_ref(), new_value, f);
	s.call()
}
