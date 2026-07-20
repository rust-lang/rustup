//! Windows has no native split-home defaults yet. The shared resolver handles explicit
//! category and legacy overrides before reaching these unsupported fallbacks.

use std::{io, path::PathBuf};

use super::env::Env;

const UNSUPPORTED: &str =
    "split home directories are not supported on Windows without an explicit override";

pub(super) fn data_home_with_env(_env: &dyn Env) -> io::Result<PathBuf> {
    unsupported()
}

pub(super) fn config_home_with_env(_env: &dyn Env) -> io::Result<PathBuf> {
    unsupported()
}

pub(super) fn state_home_with_env(_env: &dyn Env) -> io::Result<PathBuf> {
    unsupported()
}

pub(super) fn cache_home_with_env(_env: &dyn Env) -> io::Result<PathBuf> {
    unsupported()
}

pub(super) fn bin_home_with_env(_env: &dyn Env) -> io::Result<PathBuf> {
    unsupported()
}

// Implicit legacy-directory detection is intentionally Unix-only; explicit legacy homes
// have already been handled by the shared resolver.

pub(super) fn cargo_home_if_exists(_env: &dyn Env) -> Option<PathBuf> {
    None
}

pub(super) fn rustup_home_if_exists(_env: &dyn Env) -> Option<PathBuf> {
    None
}

fn unsupported<T>() -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::Unsupported, UNSUPPORTED))
}
