#[allow(clippy::wildcard_imports)]
pub(crate) use ::home::*;

#[cfg(unix)]
use std::{io, path::PathBuf};

#[cfg(unix)]
#[allow(dead_code)]
pub(crate) fn data_home() -> io::Result<PathBuf> {
    data_home_with_env(&env::OS_ENV)
}

#[cfg(unix)]
#[allow(dead_code)]
pub(crate) fn config_home() -> io::Result<PathBuf> {
    config_home_with_env(&env::OS_ENV)
}

#[cfg(unix)]
#[allow(dead_code)]
pub(crate) fn state_home() -> io::Result<PathBuf> {
    state_home_with_env(&env::OS_ENV)
}

#[cfg(unix)]
#[allow(dead_code)]
pub(crate) fn cache_home() -> io::Result<PathBuf> {
    cache_home_with_env(&env::OS_ENV)
}

#[cfg(unix)]
#[allow(dead_code)]
pub(crate) fn bin_home() -> io::Result<PathBuf> {
    bin_home_with_env(&env::OS_ENV)
}

#[cfg(unix)]
#[allow(dead_code)]
pub(crate) use platform_dir::{
    bin_home_with_env, cache_home_with_env, config_home_with_env, data_home_with_env,
    state_home_with_env,
};

#[cfg(unix)]
#[path = "platform_dir/unix.rs"]
mod platform_dir;

#[cfg(windows)]
#[path = "platform_dir/windows.rs"]
mod platform_dir;
