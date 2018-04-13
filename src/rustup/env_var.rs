use std::env;
use std::path::PathBuf;
use std::process::Command;

pub const RUST_RECURSION_COUNT_MAX: u32 = 5;

#[allow(unused)]
pub fn append_path(name: &str, value: Vec<PathBuf>, cmd: &mut Command) {
    let old_value = env::var_os(name);
    let mut parts: Vec<PathBuf>;
    if let Some(ref v) = old_value {
        parts = env::split_paths(v).collect();
        parts.extend(value);
    } else {
        parts = value;
    }
    if let Ok(new_value) = env::join_paths(parts) {
        cmd.env(name, new_value);
    }
}

pub fn prepend_path(name: &str, value: Vec<PathBuf>, cmd: &mut Command) {
    let old_value = env::var_os(name);
    let mut parts: Vec<PathBuf>;
    if let Some(ref v) = old_value {
        parts = value;
        parts.extend(env::split_paths(v).collect::<Vec<_>>());
    } else {
        parts = value;
    }

    if let Ok(new_value) = env::join_paths(parts) {
        cmd.env(name, new_value);
    }
}

pub fn inc(name: &str, cmd: &mut Command) {
    let old_value = env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    cmd.env(name, (old_value + 1).to_string());
}
