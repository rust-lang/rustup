//! Resolve Rustup's directories for the opt-in category-home layout.
//!
//! `Process` selects the layout using `RUSTUP_USE_CATEGORY_HOME`: a non-empty
//! value other than "0" enables category mode. This module provides the path
//! resolvers; it does not check the mode switch itself.
//!
//! In category mode, cache, config, data, and state each resolve independently:
//!
//! 1. Use a non-empty `RUSTUP_<CATEGORY>_HOME` as the complete directory path.
//! 2. Otherwise, use a non-empty `RUSTUP_HOME`, resolving relative paths against
//!    the current directory.
//! 3. Otherwise, use the platform's category directory with `rustup` appended.
//! 4. If the platform directory cannot be determined, use `~/.rustup`.
//!
//! On Unix, the platform directory comes from an absolute `XDG_<CATEGORY>_HOME`,
//! or defaults to `~/.cache`, `~/.config`, `~/.local/share`, or `~/.local/state`.
//! Empty or relative XDG values are ignored. Windows uses Known Folders and
//! does not consult XDG variables.
//!
//! The bin directory resolves in this order:
//!
//! 1. Use a non-empty `RUSTUP_BIN_HOME` as the complete directory path.
//! 2. Otherwise, use a non-empty `CARGO_HOME` with `bin` appended, resolving
//!    relative paths against the current directory.
//! 3. Otherwise, use `~/.local/bin` (currently `%USERPROFILE%/.local/bin` on
//!    Windows).
//! 4. If the platform directory cannot be determined, use `~/.cargo/bin`.
//!
//! TODO: The Windows bin default remains to be decided between
//! `%LOCALAPPDATA%/rustup/bin` and `%LOCALAPPDATA%/Programs/Rustup/bin`.
//! Explicit category and bin overrides are used as supplied, including relative
//! paths.
//!
//! When category mode is disabled, `Process` uses the re-exported `home` crate
//! APIs: `RUSTUP_HOME` or `~/.rustup` for all four categories, and `CARGO_HOME/bin`
//! or `~/.cargo/bin` for binaries. Category overrides have no effect in that mode.

use std::{io, path::PathBuf};

use home::env::Env;

mod platform_dir;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HomeDirs {
    pub(crate) cache: PathBuf,
    pub(crate) config: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) state: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HomeCategory {
    Cache,
    Config,
    Data,
    State,
}

impl HomeCategory {
    const fn override_env_var(self) -> &'static str {
        match self {
            Self::Cache => "RUSTUP_CACHE_HOME",
            Self::Config => "RUSTUP_CONFIG_HOME",
            Self::Data => "RUSTUP_DATA_HOME",
            Self::State => "RUSTUP_STATE_HOME",
        }
    }

    #[cfg(unix)]
    const fn xdg_env_var(self) -> &'static str {
        match self {
            Self::Cache => "XDG_CACHE_HOME",
            Self::Config => "XDG_CONFIG_HOME",
            Self::Data => "XDG_DATA_HOME",
            Self::State => "XDG_STATE_HOME",
        }
    }

    #[cfg(unix)]
    const fn fallback_subdir(self) -> &'static str {
        match self {
            Self::Cache => ".cache",
            Self::Config => ".config",
            Self::Data => ".local/share",
            Self::State => ".local/state",
        }
    }
}

pub(super) fn category_home(category: HomeCategory, env: &impl Env) -> io::Result<PathBuf> {
    if let Some(path) = path_from_env(category.override_env_var(), env) {
        return Ok(path);
    }
    if let Some(path) = path_from_env("RUSTUP_HOME", env) {
        if path.is_absolute() {
            return Ok(path);
        }
        let mut cwd = env.current_dir()?;
        cwd.push(path);
        return Ok(cwd);
    }
    platform_dir::category_home_with_env(category, env)
        .map(|path| path.join("rustup"))
        .or_else(|_| env::rustup_home_with_env(env))
}

pub(crate) mod env {
    use std::{io::Result, path::PathBuf};

    pub(crate) use home::env::{
        Env, OS_ENV, cargo_home_with_env, home_dir_with_env, rustup_home_with_env,
    };

    use super::path_from_env;

    pub(crate) fn rustup_bin_home_with_env(env: &impl Env) -> Result<PathBuf> {
        if let Some(path) = path_from_env("RUSTUP_BIN_HOME", env) {
            return Ok(path);
        }
        if path_from_env("CARGO_HOME", env).is_none()
            && let Some(path) = home_dir_with_env(env).filter(|path| path.is_absolute())
        {
            return Ok(path.join(".local/bin"));
        }
        Ok(cargo_home_with_env(env)?.join("bin"))
    }
}

fn path_from_env(key: &str, env: &impl Env) -> Option<PathBuf> {
    env.var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
