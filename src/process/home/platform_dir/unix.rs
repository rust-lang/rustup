use std::io;
use std::path::PathBuf;

use crate::process::home::env::Env;

#[allow(dead_code)]
pub(crate) fn data_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
    xdg_dir("XDG_DATA_HOME", ".local/share", env)
}

#[allow(dead_code)]
pub(crate) fn config_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config", env)
}

#[allow(dead_code)]
pub(crate) fn state_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
    xdg_dir("XDG_STATE_HOME", ".local/state", env)
}

#[allow(dead_code)]
pub(crate) fn cache_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
    xdg_dir("XDG_CACHE_HOME", ".cache", env)
}

#[allow(dead_code)]
pub(crate) fn bin_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
    Ok(xdg_home_dir(env)?.join(".local/bin"))
}

fn xdg_dir(variable: &str, fallback_subdir: &str, env: &dyn Env) -> io::Result<PathBuf> {
    let Some(value) = env
        .var_os(variable)
        .filter(|value| !value.as_os_str().is_empty())
    else {
        return Ok(xdg_home_dir(env)?.join(fallback_subdir));
    };

    let path = PathBuf::from(value);
    // We don't accept relative dirs for xdg vars.
    // See: https://specifications.freedesktop.org/basedir/latest/#basics
    // > All paths set in these environment variables must be absolute.
    // > If an implementation encounters a relative path in any of these variables
    // > it should consider the path invalid and ignore it.
    if path.is_absolute() {
        return Ok(path);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{variable} must be an absolute path"),
    ))
}

fn xdg_home_dir(env: &dyn Env) -> io::Result<PathBuf> {
    crate::process::home::env::home_dir_with_env(env)
        .ok_or_else(|| io::Error::other("could not find home dir"))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, ffi::OsString, path::Path};

    use super::*;

    const TEST_HOME: &str = "/home/rustup-test";

    #[derive(Copy, Clone)]
    struct XdgDir {
        variable: &'static str,
        explicit: &'static str,
        fallback: &'static str,
        resolve: fn(&dyn Env) -> io::Result<PathBuf>,
    }

    const XDG_DIRS: [XdgDir; 4] = [
        XdgDir {
            variable: "XDG_DATA_HOME",
            explicit: "/srv/rustup/data",
            fallback: ".local/share",
            resolve: data_home_with_env,
        },
        XdgDir {
            variable: "XDG_CONFIG_HOME",
            explicit: "/etc/rustup/config",
            fallback: ".config",
            resolve: config_home_with_env,
        },
        XdgDir {
            variable: "XDG_STATE_HOME",
            explicit: "/var/lib/rustup/state",
            fallback: ".local/state",
            resolve: state_home_with_env,
        },
        XdgDir {
            variable: "XDG_CACHE_HOME",
            explicit: "/var/cache/rustup",
            fallback: ".cache",
            resolve: cache_home_with_env,
        },
    ];

    #[derive(Default)]
    struct Fixture {
        home: Option<PathBuf>,
        vars: HashMap<&'static str, OsString>,
    }

    impl Fixture {
        fn with_home(home: Option<&str>) -> Self {
            Self {
                home: home.map(PathBuf::from),
                ..Self::default()
            }
        }
    }

    impl Env for Fixture {
        fn home_dir(&self) -> Option<PathBuf> {
            self.home.clone()
        }

        fn current_dir(&self) -> io::Result<PathBuf> {
            Err(io::Error::other("current_dir must not be queried"))
        }

        fn var_os(&self, key: &str) -> Option<OsString> {
            self.vars.get(key).cloned()
        }
    }

    fn assert_error(result: io::Result<PathBuf>, kind: io::ErrorKind, message: &str) {
        let error = result.unwrap_err();
        assert_eq!(error.kind(), kind);
        assert_eq!(error.to_string(), message);
    }

    fn assert_fallback_paths(fixture: &Fixture) -> io::Result<()> {
        for case in &XDG_DIRS {
            assert_eq!(
                (case.resolve)(fixture)?,
                Path::new(TEST_HOME).join(case.fallback),
                "{}",
                case.variable,
            );
        }
        assert_eq!(
            bin_home_with_env(fixture)?,
            Path::new(TEST_HOME).join(".local/bin")
        );

        Ok(())
    }

    #[test]
    fn explicit_env_vars() -> io::Result<()> {
        let mut fixture = Fixture::with_home(None);
        for case in &XDG_DIRS {
            fixture.vars.insert(case.variable, case.explicit.into());
        }

        for case in &XDG_DIRS {
            assert_eq!(
                (case.resolve)(&fixture)?,
                Path::new(case.explicit),
                "{}",
                case.variable,
            );
        }

        Ok(())
    }

    #[test]
    fn fallback_home_dir() -> io::Result<()> {
        let missing = Fixture::with_home(Some(TEST_HOME));
        assert_fallback_paths(&missing)?;

        let mut empty = Fixture::with_home(Some(TEST_HOME));
        for case in &XDG_DIRS {
            empty.vars.insert(case.variable, OsString::new());
        }
        assert_fallback_paths(&empty)?;

        Ok(())
    }

    #[test]
    fn reject_relative_paths() {
        for case in &XDG_DIRS {
            let mut fixture = Fixture::with_home(Some(TEST_HOME));
            fixture.vars.insert(case.variable, "relative/path".into());

            assert_error(
                (case.resolve)(&fixture),
                io::ErrorKind::InvalidInput,
                &format!("{} must be an absolute path", case.variable),
            );
        }
    }

    #[test]
    fn missing_home_errors() {
        let fixture = Fixture::with_home(None);

        for case in &XDG_DIRS {
            assert_error(
                (case.resolve)(&fixture),
                io::ErrorKind::Other,
                "could not find home dir",
            );
        }
        assert_error(
            bin_home_with_env(&fixture),
            io::ErrorKind::Other,
            "could not find home dir",
        );
    }
}
