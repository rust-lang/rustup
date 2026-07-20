//! Resolves category-specific Cargo and rustup home directories.
//!
//! Category overrides take precedence over explicit legacy homes. On Unix, an existing
//! implicit legacy home keeps every category monolithic; otherwise resolution uses the
//! platform default. Empty override values are ignored, and relative Cargo/rustup paths
//! are resolved against the current directory. Relative XDG paths remain invalid.
//!
//! Cargo's bin directory is the legacy Cargo home joined with `bin` or the platform bin
//! default. Rustup uses the same bin directory unless `RUSTUP_BIN_HOME` is set. Windows
//! honors explicit overrides, then reports that native split-home defaults are unsupported.

use std::{io, path::PathBuf};

/// Parameterized resolvers for callers that provide an environment implementation.
///
/// The top-level functions below call these resolvers with the process environment.

pub(crate) mod env {
    #[cfg(unix)]
    pub(crate) use ::home::env::home_dir_with_env;
    pub(crate) use ::home::env::{Env, OS_ENV};

    use super::{
        PathBuf, cargo_legacy_home_with_env, category_home_with_env, io, path_from_env,
        rustup_legacy_home_with_env,
    };

    pub(crate) fn cargo_config_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        category_home_with_env(
            "CARGO_CONFIG_HOME",
            "cargo",
            env,
            cargo_legacy_home_with_env,
            super::platform_dir::config_home_with_env,
        )
    }

    pub(crate) fn cargo_state_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        category_home_with_env(
            "CARGO_STATE_HOME",
            "cargo",
            env,
            cargo_legacy_home_with_env,
            super::platform_dir::state_home_with_env,
        )
    }

    pub(crate) fn cargo_data_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        category_home_with_env(
            "CARGO_DATA_HOME",
            "cargo",
            env,
            cargo_legacy_home_with_env,
            super::platform_dir::data_home_with_env,
        )
    }

    pub(crate) fn cargo_cache_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        category_home_with_env(
            "CARGO_CACHE_HOME",
            "cargo",
            env,
            cargo_legacy_home_with_env,
            super::platform_dir::cache_home_with_env,
        )
    }

    /// Resolves Cargo's executable directory, appending `bin` only to a legacy Cargo home.
    pub(crate) fn cargo_bin_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        if let Some(path) = path_from_env("CARGO_BIN_HOME", env)? {
            return Ok(path);
        }
        if let Some(path) = cargo_legacy_home_with_env(env)? {
            return Ok(path.join("bin"));
        }
        super::platform_dir::bin_home_with_env(env)
    }

    pub(crate) fn rustup_config_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        category_home_with_env(
            "RUSTUP_CONFIG_HOME",
            "rustup",
            env,
            rustup_legacy_home_with_env,
            super::platform_dir::config_home_with_env,
        )
    }

    pub(crate) fn rustup_state_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        category_home_with_env(
            "RUSTUP_STATE_HOME",
            "rustup",
            env,
            rustup_legacy_home_with_env,
            super::platform_dir::state_home_with_env,
        )
    }

    pub(crate) fn rustup_data_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        category_home_with_env(
            "RUSTUP_DATA_HOME",
            "rustup",
            env,
            rustup_legacy_home_with_env,
            super::platform_dir::data_home_with_env,
        )
    }

    pub(crate) fn rustup_cache_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        category_home_with_env(
            "RUSTUP_CACHE_HOME",
            "rustup",
            env,
            rustup_legacy_home_with_env,
            super::platform_dir::cache_home_with_env,
        )
    }

    /// Resolves rustup's executable directory, falling back to Cargo's resolved bin home.
    pub(crate) fn rustup_bin_home_with_env(env: &dyn Env) -> io::Result<PathBuf> {
        match path_from_env("RUSTUP_BIN_HOME", env)? {
            Some(path) => Ok(path),
            None => cargo_bin_home_with_env(env),
        }
    }
}

/// Reads a non-empty override, resolving relative paths only when a current directory is needed.
fn path_from_env(variable: &str, env: &dyn env::Env) -> io::Result<Option<PathBuf>> {
    let Some(value) = env
        .var_os(variable)
        .filter(|value| !value.as_os_str().is_empty())
    else {
        return Ok(None);
    };

    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(Some(path))
    } else {
        Ok(Some(env.current_dir()?.join(path)))
    }
}

/// Returns an explicit Cargo home, or an implicit legacy home only when it already exists.
fn cargo_legacy_home_with_env(env: &dyn env::Env) -> io::Result<Option<PathBuf>> {
    match path_from_env("CARGO_HOME", env)? {
        Some(path) => Ok(Some(path)),
        None => Ok(platform_dir::cargo_home_if_exists(env)),
    }
}

/// Returns an explicit rustup home, or an implicit legacy home only when it already exists.
fn rustup_legacy_home_with_env(env: &dyn env::Env) -> io::Result<Option<PathBuf>> {
    match path_from_env("RUSTUP_HOME", env)? {
        Some(path) => Ok(Some(path)),
        None => Ok(platform_dir::rustup_home_if_exists(env)),
    }
}

/// Resolves a non-bin category by override, legacy home, then product-specific platform default.
fn category_home_with_env(
    variable: &str,
    product: &str,
    env: &dyn env::Env,
    legacy_home: fn(&dyn env::Env) -> io::Result<Option<PathBuf>>,
    platform_home: fn(&dyn env::Env) -> io::Result<PathBuf>,
) -> io::Result<PathBuf> {
    if let Some(path) = path_from_env(variable, env)? {
        return Ok(path);
    }
    if let Some(path) = legacy_home(env)? {
        return Ok(path);
    }
    Ok(platform_home(env)?.join(product))
}

// OS-environment entry points mirror the parameterized `env` interface.

#[allow(dead_code)]
pub(crate) fn cargo_config_home() -> io::Result<PathBuf> {
    env::cargo_config_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn cargo_state_home() -> io::Result<PathBuf> {
    env::cargo_state_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn cargo_data_home() -> io::Result<PathBuf> {
    env::cargo_data_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn cargo_cache_home() -> io::Result<PathBuf> {
    env::cargo_cache_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn cargo_bin_home() -> io::Result<PathBuf> {
    env::cargo_bin_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn rustup_config_home() -> io::Result<PathBuf> {
    env::rustup_config_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn rustup_state_home() -> io::Result<PathBuf> {
    env::rustup_state_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn rustup_data_home() -> io::Result<PathBuf> {
    env::rustup_data_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn rustup_cache_home() -> io::Result<PathBuf> {
    env::rustup_cache_home_with_env(&env::OS_ENV)
}

#[allow(dead_code)]
pub(crate) fn rustup_bin_home() -> io::Result<PathBuf> {
    env::rustup_bin_home_with_env(&env::OS_ENV)
}

#[cfg(unix)]
#[path = "home/platform_dir/unix.rs"]
mod platform_dir;

#[cfg(windows)]
#[path = "home/platform_dir/windows.rs"]
mod platform_dir;

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, ffi::OsString, path::Path};

    use super::env::{
        Env, cargo_bin_home_with_env, cargo_cache_home_with_env, cargo_config_home_with_env,
        cargo_data_home_with_env, cargo_state_home_with_env, rustup_bin_home_with_env,
        rustup_cache_home_with_env, rustup_config_home_with_env, rustup_data_home_with_env,
        rustup_state_home_with_env,
    };
    use super::*;

    type Resolve = fn(&dyn Env) -> io::Result<PathBuf>;

    const CATEGORY_HOMES: [(&str, Resolve); 10] = [
        ("CARGO_CONFIG_HOME", cargo_config_home_with_env),
        ("CARGO_STATE_HOME", cargo_state_home_with_env),
        ("CARGO_DATA_HOME", cargo_data_home_with_env),
        ("CARGO_CACHE_HOME", cargo_cache_home_with_env),
        ("CARGO_BIN_HOME", cargo_bin_home_with_env),
        ("RUSTUP_CONFIG_HOME", rustup_config_home_with_env),
        ("RUSTUP_STATE_HOME", rustup_state_home_with_env),
        ("RUSTUP_DATA_HOME", rustup_data_home_with_env),
        ("RUSTUP_CACHE_HOME", rustup_cache_home_with_env),
        ("RUSTUP_BIN_HOME", rustup_bin_home_with_env),
    ];

    #[cfg(unix)]
    const ABSOLUTE_OVERRIDE: &str = "/split-home";
    #[cfg(windows)]
    const ABSOLUTE_OVERRIDE: &str = r"C:\split-home";

    #[cfg(unix)]
    const ABSOLUTE_CWD: &str = "/work";
    #[cfg(windows)]
    const ABSOLUTE_CWD: &str = r"C:\work";

    #[cfg(unix)]
    const CARGO_LEGACY: &str = "/legacy/cargo";
    #[cfg(windows)]
    const CARGO_LEGACY: &str = r"C:\legacy\cargo";

    #[cfg(unix)]
    const RUSTUP_LEGACY: &str = "/legacy/rustup";
    #[cfg(windows)]
    const RUSTUP_LEGACY: &str = r"C:\legacy\rustup";

    #[derive(Default)]
    struct Fixture {
        home: Option<PathBuf>,
        current_dir: Option<PathBuf>,
        vars: HashMap<&'static str, OsString>,
    }

    impl Fixture {
        fn set(&mut self, key: &'static str, value: impl Into<OsString>) {
            self.vars.insert(key, value.into());
        }
    }

    impl Env for Fixture {
        fn home_dir(&self) -> Option<PathBuf> {
            self.home.clone()
        }

        fn current_dir(&self) -> io::Result<PathBuf> {
            self.current_dir
                .clone()
                .ok_or_else(|| io::Error::other("could not find current dir"))
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

    #[test]
    fn category_overrides_cover_the_full_matrix_without_home_or_cwd() -> io::Result<()> {
        for &(variable, resolve) in &CATEGORY_HOMES {
            let mut fixture = Fixture::default();
            fixture.set(variable, ABSOLUTE_OVERRIDE);

            assert_eq!(
                resolve(&fixture)?,
                Path::new(ABSOLUTE_OVERRIDE),
                "{variable}"
            );
        }
        Ok(())
    }

    #[test]
    fn relative_category_overrides_use_cwd() -> io::Result<()> {
        for &(variable, resolve) in &CATEGORY_HOMES {
            let mut fixture = Fixture {
                current_dir: Some(ABSOLUTE_CWD.into()),
                ..Fixture::default()
            };
            fixture.set(variable, "relative/home");

            assert_eq!(
                resolve(&fixture)?,
                Path::new(ABSOLUTE_CWD).join("relative/home"),
                "{variable}",
            );
        }
        Ok(())
    }

    #[test]
    fn relative_override_propagates_cwd_errors() {
        let mut fixture = Fixture::default();
        fixture.set("CARGO_CONFIG_HOME", "relative/home");

        assert_error(
            cargo_config_home_with_env(&fixture),
            io::ErrorKind::Other,
            "could not find current dir",
        );
    }

    #[test]
    fn category_overrides_beat_explicit_legacy_homes() -> io::Result<()> {
        let mut fixture = Fixture::default();
        fixture.set("CARGO_HOME", CARGO_LEGACY);
        fixture.set("RUSTUP_HOME", RUSTUP_LEGACY);
        fixture.set("CARGO_CONFIG_HOME", ABSOLUTE_OVERRIDE);
        fixture.set("RUSTUP_CACHE_HOME", ABSOLUTE_OVERRIDE);

        assert_eq!(
            cargo_config_home_with_env(&fixture)?,
            Path::new(ABSOLUTE_OVERRIDE)
        );
        assert_eq!(
            rustup_cache_home_with_env(&fixture)?,
            Path::new(ABSOLUTE_OVERRIDE)
        );
        Ok(())
    }

    #[test]
    fn explicit_legacy_homes_keep_categories_monolithic() -> io::Result<()> {
        let mut fixture = Fixture::default();
        fixture.set("CARGO_HOME", CARGO_LEGACY);
        fixture.set("RUSTUP_HOME", RUSTUP_LEGACY);

        for resolve in [
            cargo_config_home_with_env,
            cargo_state_home_with_env,
            cargo_data_home_with_env,
            cargo_cache_home_with_env,
        ] {
            assert_eq!(resolve(&fixture)?, Path::new(CARGO_LEGACY));
        }
        assert_eq!(
            cargo_bin_home_with_env(&fixture)?,
            Path::new(CARGO_LEGACY).join("bin")
        );

        for resolve in [
            rustup_config_home_with_env,
            rustup_state_home_with_env,
            rustup_data_home_with_env,
            rustup_cache_home_with_env,
        ] {
            assert_eq!(resolve(&fixture)?, Path::new(RUSTUP_LEGACY));
        }
        assert_eq!(
            rustup_bin_home_with_env(&fixture)?,
            Path::new(CARGO_LEGACY).join("bin")
        );
        Ok(())
    }

    #[test]
    fn relative_legacy_homes_use_cwd() -> io::Result<()> {
        let mut fixture = Fixture {
            current_dir: Some(ABSOLUTE_CWD.into()),
            ..Fixture::default()
        };
        fixture.set("CARGO_HOME", "relative/cargo");
        fixture.set("RUSTUP_HOME", "relative/rustup");

        assert_eq!(
            cargo_config_home_with_env(&fixture)?,
            Path::new(ABSOLUTE_CWD).join("relative/cargo")
        );
        assert_eq!(
            rustup_config_home_with_env(&fixture)?,
            Path::new(ABSOLUTE_CWD).join("relative/rustup")
        );
        assert_eq!(
            rustup_bin_home_with_env(&fixture)?,
            Path::new(ABSOLUTE_CWD).join("relative/cargo/bin")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn implicit_legacy_roots_beat_all_xdg_values() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        std::fs::create_dir(home.path().join(".cargo"))?;
        std::fs::create_dir(home.path().join(".rustup"))?;

        let mut fixture = Fixture {
            home: Some(home.path().into()),
            ..Fixture::default()
        };
        for variable in [
            "XDG_CONFIG_HOME",
            "XDG_STATE_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
        ] {
            fixture.set(variable, "relative/ignored");
        }

        for resolve in [
            cargo_config_home_with_env,
            cargo_state_home_with_env,
            cargo_data_home_with_env,
            cargo_cache_home_with_env,
        ] {
            assert_eq!(resolve(&fixture)?, home.path().join(".cargo"));
        }
        for resolve in [
            rustup_config_home_with_env,
            rustup_state_home_with_env,
            rustup_data_home_with_env,
            rustup_cache_home_with_env,
        ] {
            assert_eq!(resolve(&fixture)?, home.path().join(".rustup"));
        }
        assert_eq!(
            cargo_bin_home_with_env(&fixture)?,
            home.path().join(".cargo/bin")
        );
        assert_eq!(
            rustup_bin_home_with_env(&fixture)?,
            home.path().join(".cargo/bin")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn xdg_defaults_apply_when_legacy_roots_do_not_exist() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        let mut fixture = Fixture {
            home: Some(home.path().into()),
            ..Fixture::default()
        };
        for (variable, subdir) in [
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
        ] {
            fixture.set(variable, home.path().join(subdir).into_os_string());
        }

        assert_eq!(
            cargo_config_home_with_env(&fixture)?,
            home.path().join("config/cargo")
        );
        assert_eq!(
            cargo_state_home_with_env(&fixture)?,
            home.path().join("state/cargo")
        );
        assert_eq!(
            cargo_data_home_with_env(&fixture)?,
            home.path().join("data/cargo")
        );
        assert_eq!(
            cargo_cache_home_with_env(&fixture)?,
            home.path().join("cache/cargo")
        );
        assert_eq!(
            rustup_config_home_with_env(&fixture)?,
            home.path().join("config/rustup")
        );
        assert_eq!(
            rustup_state_home_with_env(&fixture)?,
            home.path().join("state/rustup")
        );
        assert_eq!(
            rustup_data_home_with_env(&fixture)?,
            home.path().join("data/rustup")
        );
        assert_eq!(
            rustup_cache_home_with_env(&fixture)?,
            home.path().join("cache/rustup")
        );
        assert_eq!(
            cargo_bin_home_with_env(&fixture)?,
            home.path().join(".local/bin")
        );
        assert_eq!(
            rustup_bin_home_with_env(&fixture)?,
            home.path().join(".local/bin")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn empty_overrides_are_unset() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        let mut fixture = Fixture {
            home: Some(home.path().into()),
            ..Fixture::default()
        };
        fixture.set("CARGO_CONFIG_HOME", OsString::new());
        fixture.set("CARGO_HOME", OsString::new());
        fixture.set(
            "XDG_CONFIG_HOME",
            home.path().join("config").into_os_string(),
        );

        assert_eq!(
            cargo_config_home_with_env(&fixture)?,
            home.path().join("config/cargo")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn missing_home_preserves_the_existing_error() {
        let fixture = Fixture::default();

        for &(_, resolve) in &CATEGORY_HOMES {
            assert_error(
                resolve(&fixture),
                io::ErrorKind::Other,
                "could not find home dir",
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_ignores_xdg_and_errors_after_overrides() {
        const MESSAGE: &str =
            "split home directories are not supported on Windows without an explicit override";

        let mut fixture = Fixture::default();
        for variable in [
            "XDG_CONFIG_HOME",
            "XDG_STATE_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
        ] {
            fixture.set(variable, ABSOLUTE_OVERRIDE);
        }

        for &(_, resolve) in &CATEGORY_HOMES {
            assert_error(resolve(&fixture), io::ErrorKind::Unsupported, MESSAGE);
        }
    }
}
