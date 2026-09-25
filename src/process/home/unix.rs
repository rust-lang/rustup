//! Unix XDG platform defaults.
//!
//! Empty and relative XDG values are ignored; fallback HOME must be absolute.

use std::{
    io::{self, Result},
    path::PathBuf,
};

use home::env::{Env, home_dir_with_env};
use tracing::warn;

use super::{HomeCategory, path_from_env};

pub(super) fn category_dir(category: HomeCategory, env: &impl Env) -> Result<PathBuf> {
    let xdg_env_var = category.xdg_env_var();
    let relative_xdg_path = match path_from_env(xdg_env_var, env) {
        Some(path) if path.is_absolute() => return Ok(path),
        path => path,
    };
    let Some(path) = home_dir_with_env(env) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "home directory is not set",
        ));
    };
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "home directory is not absolute",
        ));
    }

    let fallback = path.join(category.fallback_subdir());
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
    use std::{assert_matches, collections::HashMap, path::Path};

    use super::*;
    use crate::{process::TestProcess, test::Env as _};

    #[test]
    fn explicit_xdg_values_do_not_need_home() -> Result<()> {
        let mut vars = HashMap::new();
        for category in CATEGORIES {
            vars.env(category.xdg_env_var(), category.explicit_path());
        }
        let tp = TestProcess::with_vars(vars);

        for category in CATEGORIES {
            assert_eq!(
                category_dir(category, &tp.process)?,
                category.explicit_path()
            );
        }
        Ok(())
    }

    #[test]
    fn non_absolute_xdg_values_use_defaults() -> Result<()> {
        for xdg in [None, Some(""), Some("relative/path")] {
            let mut vars = HashMap::new();
            vars.env("HOME", TEST_HOME);
            if let Some(value) = xdg {
                for category in CATEGORIES {
                    vars.env(category.xdg_env_var(), value);
                }
            }
            let tp = TestProcess::with_vars(vars);

            for category in CATEGORIES {
                assert_eq!(
                    category_dir(category, &tp.process)?,
                    Path::new(TEST_HOME).join(category.fallback_subdir()),
                    "{xdg:?}, {category:?}",
                );
            }
        }
        Ok(())
    }

    #[test]
    fn missing_home_errors() {
        let tp = TestProcess::default();

        for category in CATEGORIES {
            assert_matches!(
                category_dir(category, &tp.process),
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        && error.to_string() == "home directory is not set"
            );
        }
    }

    #[test]
    fn relative_home_errors() {
        let mut vars = HashMap::new();
        vars.env("HOME", "relative/home");
        let tp = TestProcess::with_vars(vars);

        for category in CATEGORIES {
            assert_matches!(
                category_dir(category, &tp.process),
                Err(error)
                    if error.kind() == io::ErrorKind::InvalidData
                        && error.to_string() == "home directory is not absolute"
            );
        }
    }

    const TEST_HOME: &str = "/home/rustup-test";

    const CATEGORIES: [HomeCategory; 4] = [
        HomeCategory::Cache,
        HomeCategory::Config,
        HomeCategory::Data,
        HomeCategory::State,
    ];

    impl HomeCategory {
        fn explicit_path(self) -> &'static Path {
            Path::new(match self {
                Self::Cache => "/srv/cache",
                Self::Config => "/srv/config",
                Self::Data => "/srv/data",
                Self::State => "/srv/state",
            })
        }
    }
}
