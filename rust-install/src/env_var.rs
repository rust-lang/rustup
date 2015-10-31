use std::ffi::{OsStr, OsString};
use std::env;
use std::path::Path;
use std::process::Command;

use utils;

pub fn set_default(name: &str, value: &OsStr, cmd: &mut Command) {
	let new_value = env::var_os(name)
		.and_then(utils::if_not_empty)
		.unwrap_or(value.to_owned());
	cmd.env(name, new_value);
}

pub fn set_path(name: &str, value: &Path, cmd: &mut Command) {
	let old_value = env::var_os(name);
	let mut parts = vec![value.to_owned()];
	if let Some(ref v) = old_value {
		parts.extend(env::split_paths(v));
	}
	let new_value = env::join_paths(parts).unwrap_or_else(|_| OsString::from(value));
	
	cmd.env(name, new_value);
}

pub fn inc(name: &str, cmd: &mut Command) {
	let old_value = env::var(name).ok()
		.and_then(|v| v.parse().ok())
		.unwrap_or(0);
	
	cmd.env(name, (old_value+1).to_string());
}
