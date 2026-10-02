//! Unix XDG platform defaults.
//!
//! Empty and relative XDG values are ignored; defaults are relative to the
//! user home directory returned by [`Env::home_dir`].

use std::{
    io::{self, Result},
    path::PathBuf,
};

use home::env::Env;
use tracing::warn;

use super::{HomeCategory, path_from_env};

pub(super) fn category_dir(category: HomeCategory, env: &impl Env) -> Result<PathBuf> {
    let xdg_env_var = category.xdg_env_var();
    let relative_xdg_path = match path_from_env(xdg_env_var, env) {
        Some(path) if path.is_absolute() => return Ok(path),
        path => path,
    };
    let Some(home_dir) = env.home_dir() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "home directory is not set",
        ));
    };

    let fallback = home_dir.join(category.fallback_subdir());
    if let Some(relative) = relative_xdg_path {
        warn!(
            "ignoring relative {xdg_env_var} path {}; falling back to {}",
            relative.display(),
            fallback.display()
        );
    }
    Ok(fallback)
}

impl HomeCategory {
    const fn xdg_env_var(self) -> &'static str {
        match self {
            Self::Cache => "XDG_CACHE_HOME",
            Self::Config => "XDG_CONFIG_HOME",
            Self::Data => "XDG_DATA_HOME",
            Self::State => "XDG_STATE_HOME",
        }
    }

    const fn fallback_subdir(self) -> &'static str {
        match self {
            Self::Cache => ".cache",
            Self::Config => ".config",
            Self::Data => ".local/share",
            Self::State => ".local/state",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, env, path::Path};

    use super::*;
    use crate::{process::TestProcess, test::Env as _};

    #[test]
    fn category_dir_resolves_absolute_xdg_home() -> Result<()> {
        let cwd = env::current_dir()?;
        for category in [
            HomeCategory::Cache,
            HomeCategory::Config,
            HomeCategory::Data,
            HomeCategory::State,
        ] {
            let xdg_home = cwd.join(category.fallback_subdir());
            let mut vars = HashMap::new();
            vars.env(category.xdg_env_var(), &xdg_home);
            let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");

            assert_eq!(category_dir(category, &tp.process)?, xdg_home);
        }
        Ok(())
    }

    #[test]
    fn category_dir_resolves_platform_default() -> Result<()> {
        let cwd = env::current_dir()?;
        let mut vars = HashMap::new();
        vars.env("HOME", &cwd);
        let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");

        for category in [
            HomeCategory::Cache,
            HomeCategory::Config,
            HomeCategory::Data,
            HomeCategory::State,
        ] {
            assert_eq!(
                category_dir(category, &tp.process)?,
                cwd.join(category.fallback_subdir())
            );
        }
        Ok(())
    }

    #[test]
    fn category_dir_ignores_relative_xdg_home() -> Result<()> {
        let cwd = env::current_dir()?;
        let relative_dir = "theory/of/relativity";
        for category in [
            HomeCategory::Cache,
            HomeCategory::Config,
            HomeCategory::Data,
            HomeCategory::State,
        ] {
            let key = category.xdg_env_var();
            let mut vars = HashMap::new();
            vars.env("HOME", &cwd);
            vars.env(key, relative_dir);
            let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");
            let expected = cwd.join(category.fallback_subdir());

            assert_eq!(category_dir(category, &tp.process)?, expected);
            assert_eq!(
                tp.stderr(),
                format!(
                    "warn: ignoring relative {key} path {relative_dir}; falling back to {}\n",
                    expected.display()
                )
                .as_bytes()
            );
        }
        Ok(())
    }

    #[test]
    fn category_dir_resolves_relative_home() -> Result<()> {
        let cwd = env::current_dir()?;
        let home_dir = Path::new("theory/of/relativity");
        let mut vars = HashMap::new();
        vars.env("HOME", home_dir);
        let tp = TestProcess::new(&cwd, &[] as &[&str], vars, "");

        for category in [
            HomeCategory::Cache,
            HomeCategory::Config,
            HomeCategory::Data,
            HomeCategory::State,
        ] {
            assert_eq!(
                category_dir(category, &tp.process)?,
                home_dir.join(category.fallback_subdir())
            );
        }
        Ok(())
    }

    #[test]
    fn category_dir_without_home_errors() {
        let tp = TestProcess::default();

        for category in [
            HomeCategory::Cache,
            HomeCategory::Config,
            HomeCategory::Data,
            HomeCategory::State,
        ] {
            let error = category_dir(category, &tp.process).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
            assert_eq!(error.to_string(), "home directory is not set");
        }
    }
}
