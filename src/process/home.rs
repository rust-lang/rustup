/*!
Resolve Rustup's category and binary directories in category mode.

`Process` selects the mode and handles legacy paths separately. This module
only computes paths; it does not check the mode switch, create directories,
or choose paths based on whether they exist.

# Category directories

Cache, config, data, and state each resolve independently, in this order:

1. `RUSTUP_<CATEGORY>_HOME`.
2. `RUSTUP_HOME`.
3. The platform's category directory with `rustup` appended.

# Binary directory

The bin directory resolves in this order:

1. `RUSTUP_BIN_HOME`.
2. `CARGO_HOME` with `bin` appended.
3. The user home directory with `.local/bin` appended.

Platform category directories:
    - follow XDG rules on Unix
    - use Known Folders on Windows
Bin Home uses `~/.local/bin` on both platforms

TODO: The Windows bin default is not yet finalized.
*/

use std::{io, path::PathBuf};

use home::env::Env;

#[cfg(unix)]
use self::unix::category_dir;
#[cfg(windows)]
use self::windows::category_dir;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HomeDirs {
    pub(crate) cache: PathBuf,
    pub(crate) config: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) state: PathBuf,
}

impl HomeDirs {
    /// Resolves each category's directory in category mode.
    pub(super) fn from_env(env: &impl Env) -> io::Result<Self> {
        Ok(Self {
            cache: category_home(HomeCategory::Cache, env)?,
            config: category_home(HomeCategory::Config, env)?,
            data: category_home(HomeCategory::Data, env)?,
            state: category_home(HomeCategory::State, env)?,
        })
    }
}

/// Resolves a category directory in category mode.
pub(super) fn category_home(category: HomeCategory, env: &impl Env) -> io::Result<PathBuf> {
    if let Some(explicit_override) = path_from_env(category.override_env_var(), env) {
        return Ok(explicit_override);
    }

    if let Some(rustup_home) = path_from_env("RUSTUP_HOME", env) {
        if rustup_home.is_absolute() {
            return Ok(rustup_home);
        }
        return Ok(env.current_dir()?.join(rustup_home));
    }

    category_dir(category, env).map(|platform_dir| platform_dir.join("rustup"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HomeCategory {
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
}

/// Resolves the binary directory in category mode.
pub(super) fn bin_home(env: &impl Env) -> io::Result<PathBuf> {
    if let Some(explicit_override) = path_from_env("RUSTUP_BIN_HOME", env) {
        return Ok(explicit_override);
    }

    if let Some(cargo_home) = path_from_env("CARGO_HOME", env) {
        let cargo_home = if cargo_home.is_absolute() {
            cargo_home
        } else {
            env.current_dir()?.join(cargo_home)
        };
        return Ok(cargo_home.join("bin"));
    }

    env.home_dir()
        .map(|home_dir| home_dir.join(".local/bin"))
        .ok_or_else(|| io::Error::other("could not find home dir"))
}

fn path_from_env(key: &str, env: &impl Env) -> Option<PathBuf> {
    env.var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, env};

    use super::*;
    use crate::{process::TestProcess, test::Env as _};

    #[test]
    fn category_home_resolves_explicit_override() -> io::Result<()> {
        let cwd = env::current_dir()?;
        let overrides = [
            (HomeCategory::Cache, "RUSTUP_CACHE_HOME", cwd.join("cache")),
            (HomeCategory::Config, "RUSTUP_CONFIG_HOME", "config".into()),
            (HomeCategory::Data, "RUSTUP_DATA_HOME", cwd.join("data")),
            (HomeCategory::State, "RUSTUP_STATE_HOME", "state".into()),
        ];
        let mut vars = HashMap::new();
        vars.env("RUSTUP_HOME", cwd.join("rustup"));
        vars.env("HOME", cwd.join("home"));
        for (_, key, explicit_override) in &overrides {
            vars.env(key, explicit_override);
        }

        let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");
        for (category, _, expected) in overrides {
            assert_eq!(category_home(category, &tp.process)?, expected);
        }
        Ok(())
    }

    #[test]
    fn category_home_resolves_absolute_rustup_home() -> io::Result<()> {
        let cwd = env::current_dir()?;
        let rustup_home = cwd.join("legacy");
        let home_dir = cwd.join("home");
        let mut vars = HashMap::new();
        vars.env("RUSTUP_HOME", &rustup_home);
        vars.env("HOME", &home_dir);

        let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");
        for category in [
            HomeCategory::Cache,
            HomeCategory::Config,
            HomeCategory::Data,
            HomeCategory::State,
        ] {
            assert_eq!(category_home(category, &tp.process)?, rustup_home);
        }
        Ok(())
    }

    #[test]
    fn category_home_resolves_relative_rustup_home() -> io::Result<()> {
        let cwd = env::current_dir()?;
        let rustup_home = "rustup";
        let mut vars = HashMap::new();
        vars.env("RUSTUP_HOME", rustup_home);
        vars.env("HOME", cwd.join("home"));

        let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");
        let expected = cwd.join("rustup");
        for category in [
            HomeCategory::Cache,
            HomeCategory::Config,
            HomeCategory::Data,
            HomeCategory::State,
        ] {
            assert_eq!(category_home(category, &tp.process)?, expected);
        }
        Ok(())
    }

    #[test]
    fn bin_home_resolves_explicit_override() -> io::Result<()> {
        let cwd = env::current_dir()?;
        for explicit_override in [cwd.join("tools"), "tools".into()] {
            let mut vars = HashMap::new();
            vars.env("RUSTUP_BIN_HOME", &explicit_override);
            vars.env("CARGO_HOME", cwd.join("cargo"));
            vars.env("HOME", cwd.join("home"));

            let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");
            assert_eq!(bin_home(&tp.process)?, explicit_override);
        }
        Ok(())
    }

    #[test]
    fn bin_home_resolves_absolute_cargo_home() -> io::Result<()> {
        let cwd = env::current_dir()?;
        let cargo_home = cwd.join("cargo-home");
        let mut vars = HashMap::new();
        vars.env("CARGO_HOME", &cargo_home);
        vars.env("HOME", cwd.join("home"));

        let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");
        assert_eq!(bin_home(&tp.process)?, cargo_home.join("bin"));
        Ok(())
    }

    #[test]
    fn bin_home_resolves_relative_cargo_home() -> io::Result<()> {
        let cwd = env::current_dir()?;
        let mut vars = HashMap::new();
        vars.env("CARGO_HOME", "cargo");
        vars.env("HOME", cwd.join("home"));

        let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");
        assert_eq!(bin_home(&tp.process)?, cwd.join("cargo/bin"));
        Ok(())
    }

    #[test]
    fn bin_home_resolves_platform_default() -> io::Result<()> {
        let cwd = env::current_dir()?;
        let home_dir = cwd.join("home");
        let mut vars = HashMap::new();
        vars.env("HOME", &home_dir);

        let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");
        assert_eq!(bin_home(&tp.process)?, home_dir.join(".local/bin"));
        Ok(())
    }

    #[test]
    fn bin_home_without_home_errors() {
        let tp = TestProcess::default();
        let error = bin_home(&tp.process).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "could not find home dir");
    }
}
