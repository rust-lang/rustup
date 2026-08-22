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
//! When category mode is disabled, `Process` uses the `home` crate
//! APIs: `RUSTUP_HOME` or `~/.rustup` for all four categories, and `CARGO_HOME/bin`
//! or `~/.cargo/bin` for binaries. Category overrides have no effect in that mode.

use std::{io, path::PathBuf};

use home::env::{Env, cargo_home_with_env, home_dir_with_env, rustup_home_with_env};

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

pub(super) fn category_homes(env: &impl Env) -> io::Result<HomeDirs> {
    Ok(HomeDirs {
        cache: category_home(HomeCategory::Cache, env)?,
        config: category_home(HomeCategory::Config, env)?,
        data: category_home(HomeCategory::Data, env)?,
        state: category_home(HomeCategory::State, env)?,
    })
}

/// Resolves a category directory in category mode.
///
/// Respects an explicit `RUSTUP_HOME` unless `RUSTUP_<CATEGORY>_HOME` overrides it.
/// Ignores empty overrides, preserves relative category paths, and resolves
/// relative `RUSTUP_HOME` paths against the current directory.
///
/// Otherwise, appends `rustup` to the platform's category directory: an XDG
/// directory on Unix or a Known Folder on Windows. Falls back to the legacy
/// Rustup home if the platform directory cannot be determined.
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
    category_dir(category, env)
        .map(|path| path.join("rustup"))
        .or_else(|_| rustup_home_with_env(env))
}

/// Resolves the binary directory in category mode.
///
/// Respects an explicit `CARGO_HOME` unless `RUSTUP_BIN_HOME` overrides it,
/// consistent with Cargo's compatibility policy from the [Cargo XDG paths discussion].
///
/// XDG uses the shared `$HOME/.local/bin` directory without an `XDG_BIN_HOME`
/// variable or a `rustup` subdirectory. The Windows default is still undecided
/// (see the module-level TODO). These differences require separate bin directory
/// resolution.
///
/// See <https://specifications.freedesktop.org/basedir/latest/#environment-variables>.
///
/// [Cargo XDG paths discussion]: https://blog.rust-lang.org/inside-rust/2025/10/01/this-development-cycle-in-cargo-1.90/#all-hands-xdg-paths
pub(super) fn bin_home(env: &impl Env) -> io::Result<PathBuf> {
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

fn path_from_env(key: &str, env: &impl Env) -> Option<PathBuf> {
    env.var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use std::ffi::OsStr;
    #[cfg(unix)]
    use std::fs;
    use std::{collections::HashMap, path::Path};

    use super::*;
    #[cfg(unix)]
    use crate::process::TestProcess;
    use crate::{
        process::{Process, TestContext},
        test::Env as _,
    };

    #[test]
    fn uses_direct_rustup_precedence() -> io::Result<()> {
        let cwd = Path::new("/work");
        let mut vars = HashMap::new();
        vars.env("RUSTUP_STATE_HOME", "state");
        vars.env("RUSTUP_HOME", "rustup");
        vars.env("HOME", Path::new("/home"));
        vars.env("XDG_STATE_HOME", Path::new("/xdg/state"));

        let process = test_env(cwd, vars.clone());
        let homes = category_homes(&process)?;
        assert_eq!(homes.cache, Path::new("/work/rustup"));
        assert_eq!(homes.config, Path::new("/work/rustup"));
        assert_eq!(homes.data, Path::new("/work/rustup"));
        assert_eq!(homes.state, Path::new("state"));

        vars.env("RUSTUP_STATE_HOME", "");
        assert_eq!(
            category_homes(&test_env(cwd, vars))?,
            HomeDirs {
                cache: "/work/rustup".into(),
                config: "/work/rustup".into(),
                data: "/work/rustup".into(),
                state: "/work/rustup".into(),
            }
        );
        Ok(())
    }

    #[test]
    fn uses_rustup_bin_home_override() -> io::Result<()> {
        let cwd = Path::new("/work");
        let mut vars = HashMap::new();
        vars.env("RUSTUP_BIN_HOME", "bin");
        vars.env("CARGO_HOME", "cargo");

        let process = test_env(cwd, vars.clone());
        assert_eq!(bin_home(&process)?, Path::new("bin"));

        vars.env("RUSTUP_BIN_HOME", "");
        assert_eq!(
            bin_home(&test_env(cwd, vars))?,
            Path::new("/work/cargo/bin")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn uses_unix_platform_defaults() -> io::Result<()> {
        let mut vars = HashMap::new();
        vars.env("RUSTUP_HOME", "");
        vars.env("RUSTUP_STATE_HOME", "");
        vars.env("HOME", Path::new("/home"));
        vars.env("XDG_STATE_HOME", Path::new("/xdg/state"));

        let homes = category_homes(&test_env(Path::new("/work"), vars.clone()))?;
        assert_eq!(homes.state, Path::new("/xdg/state/rustup"));

        vars.env("XDG_STATE_HOME", Path::new("xdg/state"));
        let process = TestProcess::new(Path::new("/work"), &[] as &[&str], vars.clone(), "");
        let homes = category_homes(&process.process)?;
        assert_eq!(homes.state, Path::new("/home/.local/state/rustup"));
        assert_eq!(
            process.stderr(),
            b"warn: ignoring relative XDG_STATE_HOME path xdg/state; falling back to /home/.local/state\n"
        );

        vars.env("XDG_STATE_HOME", "");
        let homes = category_homes(&test_env(Path::new("/work"), vars))?;
        assert_eq!(homes.state, Path::new("/home/.local/state/rustup"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn ignores_existing_legacy_home() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        fs::create_dir(home.path().join(".rustup"))?;
        let mut vars = HashMap::new();
        vars.env("HOME", home.path());

        let homes = category_homes(&test_env(Path::new("/work"), vars))?;
        assert_eq!(homes.cache, home.path().join(".cache/rustup"));
        assert_eq!(homes.config, home.path().join(".config/rustup"));
        assert_eq!(homes.data, home.path().join(".local/share/rustup"));
        assert_eq!(homes.state, home.path().join(".local/state/rustup"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn without_absolute_home() -> io::Result<()> {
        let mut vars = HashMap::new();
        let process = test_env(Path::new("/work"), vars.clone());

        assert!(category_homes(&process).is_err());
        assert!(bin_home(&process).is_err());

        // Platform defaults require an absolute home, while legacy paths do not.
        vars.env("HOME", "relative");
        let process = test_env(Path::new("/work"), vars);
        assert_eq!(
            category_homes(&process)?,
            HomeDirs {
                cache: "relative/.rustup".into(),
                config: "relative/.rustup".into(),
                data: "relative/.rustup".into(),
                state: "relative/.rustup".into(),
            }
        );
        assert_eq!(bin_home(&process)?, Path::new("relative/.cargo/bin"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn uses_cargo_home_or_bin_platform_default() -> io::Result<()> {
        let mut vars = HashMap::new();
        vars.env("RUSTUP_BIN_HOME", "");
        vars.env("HOME", Path::new("/home"));

        for (cargo_home, expected_bin_home) in [("", "/home/.local/bin"), ("/cargo", "/cargo/bin")]
        {
            let mut vars = vars.clone();
            vars.env("CARGO_HOME", cargo_home);
            assert_eq!(
                bin_home(&test_env(Path::new("/work"), vars))?,
                Path::new(expected_bin_home)
            );
        }
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn uses_windows_platform_defaults() -> io::Result<()> {
        let mut vars = HashMap::new();
        vars.env("HOME", Path::new(r"C:\Users\rustup-test"));
        let process = test_env(Path::new(r"C:\work"), vars);

        let homes = category_homes(&process)?;
        assert!(homes.cache.is_absolute());
        assert!(homes.config.is_absolute());
        assert_eq!(homes.config, homes.data);
        assert_eq!(homes.config, homes.state);
        assert_ne!(homes.cache, homes.config);
        for home in [&homes.cache, &homes.config, &homes.data, &homes.state] {
            assert_eq!(home.file_name(), Some(OsStr::new("rustup")));
        }
        assert_eq!(
            bin_home(&process)?,
            Path::new(r"C:\Users\rustup-test").join(".local/bin")
        );
        Ok(())
    }

    fn test_env(cwd: &Path, vars: HashMap<String, String>) -> Process {
        Process::TestProcess(TestContext {
            cwd: cwd.into(),
            vars,
            ..Default::default()
        })
    }
}
