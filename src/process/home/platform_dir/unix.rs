//! Unix XDG platform defaults.
//!
//! Empty and relative XDG values are ignored; fallback HOME must be absolute.

use std::{
    io::{self, Result},
    path::PathBuf,
};

use tracing::warn;

use crate::process::home::{
    HomeCategory,
    env::{Env, home_dir_with_env},
    path_from_env,
};

pub fn category_home_with_env(category: HomeCategory, env: &impl Env) -> Result<PathBuf> {
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

#[cfg(test)]
mod tests {
    use std::{assert_matches, ffi::OsString, path::Path};

    use super::*;

    #[test]
    fn explicit_xdg_values_do_not_need_home() -> Result<()> {
        let env = TestEnv {
            xdg: XdgStatus::Explicit,
            home: None,
        };

        for category in CATEGORIES {
            assert_eq!(
                category_home_with_env(category, &env)?,
                category.explicit_path(),
            );
        }
        Ok(())
    }

    #[test]
    fn missing_xdg_values_use_defaults() -> Result<()> {
        assert_category_fallbacks(XdgStatus::Missing)
    }

    #[test]
    fn empty_xdg_values_use_defaults() -> Result<()> {
        assert_category_fallbacks(XdgStatus::Empty)
    }

    #[test]
    fn relative_xdg_values_use_defaults() -> Result<()> {
        assert_category_fallbacks(XdgStatus::Relative)
    }

    #[test]
    fn missing_home_errors() {
        let env = TestEnv {
            xdg: XdgStatus::Missing,
            home: None,
        };

        for category in CATEGORIES {
            assert_matches!(
                category_home_with_env(category, &env),
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        && error.to_string() == "home directory is not set"
            );
        }
    }

    #[test]
    fn relative_home_errors() {
        let env = TestEnv {
            xdg: XdgStatus::Missing,
            home: Some(Path::new("relative/home")),
        };

        for category in CATEGORIES {
            assert_matches!(
                category_home_with_env(category, &env),
                Err(error)
                    if error.kind() == io::ErrorKind::InvalidData
                        && error.to_string() == "home directory is not absolute"
            );
        }
    }

    fn assert_category_fallbacks(xdg: XdgStatus) -> Result<()> {
        let env = TestEnv {
            xdg,
            home: Some(Path::new(TEST_HOME)),
        };

        for category in CATEGORIES {
            assert_eq!(
                category_home_with_env(category, &env)?,
                Path::new(TEST_HOME).join(category.fallback_subdir()),
            );
        }
        Ok(())
    }

    struct TestEnv<'a> {
        xdg: XdgStatus,
        home: Option<&'a Path>,
    }

    impl Env for TestEnv<'_> {
        fn home_dir(&self) -> Option<PathBuf> {
            self.home.map(Path::to_path_buf)
        }

        fn current_dir(&self) -> Result<PathBuf> {
            panic!("current_dir must not be queried")
        }

        fn var_os(&self, key: &str) -> Option<OsString> {
            let category = CATEGORIES
                .into_iter()
                .find(|category| key == category.xdg_env_var())?;
            match self.xdg {
                XdgStatus::Empty => Some(OsString::new()),
                XdgStatus::Explicit => Some(category.explicit_path().into()),
                XdgStatus::Missing => None,
                XdgStatus::Relative => Some("relative/path".into()),
            }
        }
    }

    #[derive(Clone, Copy)]
    enum XdgStatus {
        Empty,
        Explicit,
        Missing,
        Relative,
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
