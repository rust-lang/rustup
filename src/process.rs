#[cfg(feature = "test")]
use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use std::{
    env,
    ffi::{OsStr, OsString},
    fmt::Debug,
    io::{self, IsTerminal},
    num::NonZero,
    path::PathBuf,
    str::FromStr,
    thread,
};

use anstream::ColorChoice;
use anyhow::{Context, bail};
use indicatif::ProgressDrawTarget;
#[cfg(feature = "test")]
use tracing::subscriber::DefaultGuard;
#[cfg(feature = "test")]
use tracing_subscriber::util::SubscriberInitExt;
#[cfg(feature = "test")]
use tracing_subscriber::{EnvFilter, Registry, reload::Handle};

#[cfg(feature = "test")]
use crate::{
    cli::log,
    test::{CHECKPOINT_ENV, checkpoint_path},
};

mod file_source;
mod home;
pub(crate) use home::HomeDirs;
mod terminal_source;
pub use terminal_source::ColorableTerminal;

/// Allows concrete types for the process abstraction.
#[derive(Clone, Debug)]
pub enum Process {
    OsProcess(OsProcess),
    #[cfg(feature = "test")]
    TestProcess(TestContext),
}

impl Process {
    pub fn os() -> Self {
        Self::OsProcess(OsProcess::new())
    }

    pub fn name(&self) -> Option<String> {
        let arg0 = match self.var("RUSTUP_FORCE_ARG0") {
            Ok(v) => Some(v),
            Err(_) => self.args().next(),
        }
        .map(PathBuf::from);

        arg0.as_ref()
            .and_then(|a| a.file_stem())
            .and_then(OsStr::to_str)
            .map(String::from)
    }

    pub(crate) fn home_dir(&self) -> Option<PathBuf> {
        home::env::home_dir_with_env(self)
    }

    pub(crate) fn cargo_home(&self) -> anyhow::Result<PathBuf> {
        home::env::cargo_home_with_env(self).context("failed to determine cargo home")
    }

    pub(crate) fn rustup_home(&self) -> anyhow::Result<PathBuf> {
        home::env::rustup_home_with_env(self).context("failed to determine rustup home dir")
    }

    /// Returns Rustup's cache, config, data, and state directories.
    ///
    /// Category mode uses each non-empty `RUSTUP_<CATEGORY>_HOME`, otherwise
    /// the platform default. Resolution errors propagate without legacy fallback.
    /// Legacy mode uses the resolved Rustup home for all four categories.
    /// See [`home`] for platform defaults and path rules.
    pub(crate) fn home_dirs(&self) -> io::Result<HomeDirs> {
        if self.use_category_home() {
            Ok(HomeDirs {
                cache: home::category_home(home::HomeCategory::Cache, self)?,
                config: home::category_home(home::HomeCategory::Config, self)?,
                data: home::category_home(home::HomeCategory::Data, self)?,
                state: home::category_home(home::HomeCategory::State, self)?,
            })
        } else {
            let home = home::env::rustup_home_with_env(self)?;
            Ok(HomeDirs {
                cache: home.clone(),
                config: home.clone(),
                data: home.clone(),
                state: home,
            })
        }
    }

    /// Returns Rustup's binary installation directory.
    ///
    /// Category mode uses a non-empty `RUSTUP_BIN_HOME`, otherwise the platform
    /// default, with no Cargo home fallback. Legacy mode appends `bin` to the
    /// resolved Cargo home.
    pub(crate) fn rustup_bin_home(&self) -> io::Result<PathBuf> {
        if self.use_category_home() {
            home::env::rustup_bin_home_with_env(self)
        } else {
            Ok(home::env::cargo_home_with_env(self)?.join("bin"))
        }
    }

    /// Returns the directory containing Rustup's shell environment scripts.
    /// Uses the config home in category mode, or the Cargo home in legacy mode.
    #[cfg(any(unix, test))]
    pub(crate) fn rustup_env_home(&self) -> io::Result<PathBuf> {
        if self.use_category_home() {
            home::category_home(home::HomeCategory::Config, self)
        } else {
            home::env::cargo_home_with_env(self)
        }
    }

    /// Category mode is enabled when `RUSTUP_USE_CATEGORY_HOME` is non-empty
    /// and not "0"; values such as "false" also enable it.
    pub(crate) fn use_category_home(&self) -> bool {
        self.var_os("RUSTUP_USE_CATEGORY_HOME")
            .is_some_and(|value| value != "0")
    }

    pub fn io_thread_count(&self) -> anyhow::Result<IoThreadCount> {
        if let Ok(n) = self.var("RUSTUP_IO_THREADS") {
            let threads = usize::from_str(&n).context(
                "invalid value in RUSTUP_IO_THREADS -- must be a natural number greater than zero",
            )?;
            match threads {
                0 => bail!("RUSTUP_IO_THREADS must be a natural number greater than zero"),
                _ => return Ok(IoThreadCount::UserSpecified(threads)),
            }
        };

        let count = match thread::available_parallelism() {
            // Don't spawn more than 8 I/O threads unless the user tells us to.
            // Feel free to increase this value if it improves performance.
            Ok(threads) => Ord::min(threads.get(), 8),
            // Unknown for target platform or no permission to query.
            Err(_) => 1,
        };
        Ok(IoThreadCount::Default(count))
    }

    pub(crate) fn unpack_ram(&self) -> anyhow::Result<Option<usize>, env::VarError> {
        Ok(match self.var_opt("RUSTUP_UNPACK_RAM")? {
            Some(budget) => usize::from_str(&budget).ok(),
            None => None,
        })
    }

    pub fn var_opt(&self, key: &str) -> anyhow::Result<Option<String>, env::VarError> {
        match self.var(key) {
            Ok(val) => Ok(Some(val)),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn var(&self, key: &str) -> Result<String, env::VarError> {
        let value = match self {
            Self::OsProcess(_) => env::var(key)?,
            #[cfg(feature = "test")]
            Self::TestProcess(p) => match p.vars.get(key) {
                Some(val) => val.to_owned(),
                None => return Err(env::VarError::NotPresent),
            },
        };

        match value.is_empty() {
            false => Ok(value),
            true => Err(env::VarError::NotPresent),
        }
    }

    /// Determines if the current process is running in a CI environment.
    ///
    /// # Note
    ///
    /// This function returns true iff the `CI` environment variable is set _and_ `RUSTUP_CI` is
    /// not set.
    ///
    /// The `RUSTUP_CI` bit is required because it is set by rustup itself in test suites in
    /// order to reproduce normal behavior even in a CI environment.
    pub fn is_ci(&self) -> bool {
        self.var("CI").is_ok() && self.var("RUSTUP_CI").is_err()
    }

    #[cfg(not(target_os = "linux"))]
    pub fn permit_copy_rename(&self) -> bool {
        false
    }

    #[cfg(target_os = "linux")]
    pub fn permit_copy_rename(&self) -> bool {
        match self {
            // HACK: On Linux CI machines, rustup is sometimes installed in a Docker image and we
            // may hit OverlayFS restrictions that prevent renaming. Thus, we allow falling back to
            // copying to avoid this issue.
            // See: <https://github.com/dtolnay/rust-toolchain/pull/177>
            _ if self.is_ci() => true,
            Self::OsProcess(_) => env::var_os("RUSTUP_PERMIT_COPY_RENAME").is_some(),
            #[cfg(feature = "test")]
            Self::TestProcess(p) => p.vars.contains_key("RUSTUP_PERMIT_COPY_RENAME"),
        }
    }

    pub(crate) fn var_os(&self, key: &str) -> Option<OsString> {
        let value = match self {
            Self::OsProcess(_) => env::var_os(key)?,
            #[cfg(feature = "test")]
            Self::TestProcess(p) => p.vars.get(key).map(OsString::from)?,
        };

        match value.is_empty() {
            false => Some(value),
            true => None,
        }
    }

    pub(crate) fn args(&self) -> Box<dyn Iterator<Item = String> + '_> {
        match self {
            Self::OsProcess(_) => Box::new(env::args()),
            #[cfg(feature = "test")]
            Self::TestProcess(p) => Box::new(p.args.iter().cloned()),
        }
    }

    pub(crate) fn args_os(&self) -> Box<dyn Iterator<Item = OsString> + '_> {
        match self {
            Self::OsProcess(_) => Box::new(env::args_os()),
            #[cfg(feature = "test")]
            Self::TestProcess(p) => Box::new(p.args.iter().map(OsString::from)),
        }
    }

    pub(crate) fn stdin(&self) -> Box<dyn file_source::Stdin> {
        match self {
            Self::OsProcess(_) => Box::new(io::stdin()),
            #[cfg(feature = "test")]
            Self::TestProcess(p) => Box::new(file_source::TestStdin(p.stdin.clone())),
        }
    }

    pub(crate) fn stdout(&self) -> ColorableTerminal {
        match self {
            Self::OsProcess(_) => ColorableTerminal::stdout(self),
            #[cfg(feature = "test")]
            Self::TestProcess(p) => {
                ColorableTerminal::test(file_source::TestWriter(p.stdout.clone()), self)
            }
        }
    }

    pub(crate) fn stderr(&self) -> ColorableTerminal {
        match self {
            Self::OsProcess(_) => ColorableTerminal::stderr(self),
            #[cfg(feature = "test")]
            Self::TestProcess(p) => {
                ColorableTerminal::test(file_source::TestWriter(p.stderr.clone()), self)
            }
        }
    }

    pub fn current_dir(&self) -> io::Result<PathBuf> {
        match self {
            Self::OsProcess(_) => env::current_dir(),
            #[cfg(feature = "test")]
            Self::TestProcess(p) => Ok(p.cwd.clone()),
        }
    }

    pub fn progress_draw_target(&self) -> ProgressDrawTarget {
        match self {
            Self::OsProcess(_) => (),
            #[cfg(feature = "test")]
            Self::TestProcess(_) => return ProgressDrawTarget::hidden(),
        }

        let term = self.stdout();
        let term = match self.var("RUSTUP_TERM_PROGRESS_WHEN") {
            Ok(s) if s.eq_ignore_ascii_case("always") => Some(term),
            Ok(s) if s.eq_ignore_ascii_case("never") => None,
            _ if term.is_a_tty() => Some(term),
            _ => None,
        };

        match term {
            Some(t) => ProgressDrawTarget::term_like_with_hz(Box::new(t), 20),
            None => ProgressDrawTarget::hidden(),
        }
    }

    fn color_choice(&self, is_a_tty: bool) -> ColorChoice {
        match self.var("RUSTUP_TERM_COLOR") {
            Ok(s) if s.eq_ignore_ascii_case("always") => ColorChoice::Always,
            Ok(s) if s.eq_ignore_ascii_case("never") => ColorChoice::Never,
            _ if is_a_tty => ColorChoice::Auto,
            _ => ColorChoice::Never,
        }
    }

    pub fn concurrent_downloads(&self) -> Option<usize> {
        let s = self.var("RUSTUP_CONCURRENT_DOWNLOADS").ok()?;
        Some(NonZero::from_str(&s).ok()?.get())
    }

    /// Registers a testing checkpoint with the given name and parks the current thread.
    ///
    /// Usually, the current process will be killed by the test driver.
    #[cfg(feature = "test")]
    pub(crate) fn checkpoint(&self, name: &str) {
        if self.var(CHECKPOINT_ENV).as_deref() != Ok(name) {
            return;
        }

        let rustup_home = self
            .rustup_home()
            .expect("selected test checkpoint requires RUSTUP_HOME");
        let test_root = rustup_home
            .parent()
            .expect("test RUSTUP_HOME must be inside the test root");
        fs::write(checkpoint_path(test_root, name), name)
            .expect("failed to write test checkpoint marker");

        let start_time = Instant::now();
        let max_wait = Duration::from_mins(5);
        while start_time.elapsed() < max_wait {
            thread::sleep(Duration::from_secs(10));
        }
        panic!(
            "test checkpoint '{name}' timed out after {max_wait:?} without being killed by the test driver",
        );
    }
}

pub enum IoThreadCount {
    Default(usize),
    UserSpecified(usize),
}

impl From<IoThreadCount> for usize {
    fn from(c: IoThreadCount) -> Self {
        match c {
            IoThreadCount::Default(n) | IoThreadCount::UserSpecified(n) => n,
        }
    }
}

impl home::env::Env for Process {
    fn home_dir(&self) -> Option<PathBuf> {
        match self {
            Self::OsProcess(_) => home::env::OS_ENV.home_dir(),
            #[cfg(feature = "test")]
            Self::TestProcess(_) => self.var("HOME").ok().map(|v| v.into()),
        }
    }

    fn current_dir(&self) -> Result<PathBuf, io::Error> {
        match self {
            Self::OsProcess(_) => home::env::OS_ENV.current_dir(),
            #[cfg(feature = "test")]
            Self::TestProcess(_) => self.current_dir(),
        }
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        self.var_os(key)
    }
}

// ----------- real process -----------------

#[derive(Clone, Debug)]
pub struct OsProcess {
    pub(self) stderr_is_a_tty: bool,
    pub(self) stdout_is_a_tty: bool,
}

impl OsProcess {
    pub fn new() -> Self {
        Self {
            stderr_is_a_tty: io::stderr().is_terminal(),
            stdout_is_a_tty: io::stdout().is_terminal(),
        }
    }
}

impl Default for OsProcess {
    fn default() -> Self {
        Self::new()
    }
}

// ------------ test process ----------------

#[cfg(feature = "test")]
pub struct TestProcess {
    pub process: Process,
    pub console_filter: Handle<EnvFilter, Registry>,
    // These guards are dropped _in order_ at the end of the test.
    #[cfg(feature = "otel")]
    _telemetry_guard: log::GlobalTelemetryGuard,
    _tracing_guard: DefaultGuard,
}

#[cfg(feature = "test")]
impl TestProcess {
    pub fn new<P: AsRef<Path>, A: AsRef<str>>(
        cwd: P,
        args: &[A],
        vars: HashMap<String, String>,
        stdin: &str,
    ) -> Self {
        Self::from(TestContext {
            cwd: cwd.as_ref().to_path_buf(),
            args: args.iter().map(|s| s.as_ref().to_string()).collect(),
            vars,
            stdin: Arc::new(Mutex::new(Cursor::new(stdin.to_string()))),
            stdout: Arc::default(),
            stderr: Arc::default(),
        })
    }

    pub fn with_vars(vars: HashMap<String, String>) -> Self {
        Self::from(TestContext {
            vars,
            ..Default::default()
        })
    }

    /// Extracts the stdout from the process
    pub fn stdout(&self) -> Vec<u8> {
        let Process::TestProcess(tp) = &self.process else {
            unreachable!()
        };

        tp.stdout.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Extracts the stderr from the process
    pub fn stderr(&self) -> Vec<u8> {
        let Process::TestProcess(tp) = &self.process else {
            unreachable!()
        };

        tp.stderr.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(feature = "test")]
impl From<TestContext> for TestProcess {
    fn from(inner: TestContext) -> Self {
        let inner = Process::TestProcess(inner);
        let (tracing_subscriber, console_filter) = log::tracing_subscriber(&inner);
        Self {
            process: inner,
            console_filter,
            #[cfg(feature = "otel")]
            _telemetry_guard: log::set_global_telemetry(),
            _tracing_guard: tracing_subscriber.set_default(),
        }
    }
}

#[cfg(feature = "test")]
impl Default for TestProcess {
    fn default() -> Self {
        Self::from(TestContext::default())
    }
}

#[cfg(feature = "test")]
#[derive(Clone, Debug, Default)]
pub struct TestContext {
    pub cwd: PathBuf,
    args: Vec<String>,
    vars: HashMap<String, String>,
    stdin: file_source::TestStdinInner,
    stdout: file_source::TestWriterInner,
    stderr: file_source::TestWriterInner,
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{assert_matches, fs};
    use std::{collections::HashMap, path::Path};

    use super::*;
    use crate::process::TestProcess;
    use crate::test::Env;

    #[test]
    fn term_color_choice() {
        fn assert_color_choice(env_val: &str, is_a_tty: bool, color_choice: ColorChoice) {
            let mut vars = HashMap::new();
            vars.env("RUSTUP_TERM_COLOR", env_val);
            let tp = TestProcess::with_vars(vars);
            assert_eq!(tp.process.color_choice(is_a_tty), color_choice);
        }

        assert_color_choice("aLWayS", false, ColorChoice::Always);
        assert_color_choice("neVer", false, ColorChoice::Never);
        // tty + `auto` enables the colors.
        assert_color_choice("AutO", true, ColorChoice::Auto);
        // non-tty + `auto` does not enable the colors.
        assert_color_choice("aUTo", false, ColorChoice::Never);
    }

    #[test]
    fn category_mode_disabled_uses_legacy_homes() -> io::Result<()> {
        let mut vars = HashMap::new();
        vars.env("HOME", Path::new("/home"));
        vars.env("RUSTUP_CACHE_HOME", Path::new("/split/cache"));
        vars.env("RUSTUP_CONFIG_HOME", Path::new("/split/config"));
        vars.env("RUSTUP_DATA_HOME", Path::new("/split/data"));
        vars.env("RUSTUP_STATE_HOME", Path::new("/split"));
        vars.env("RUSTUP_BIN_HOME", Path::new("/split/bin"));
        vars.env("XDG_CACHE_HOME", Path::new("/xdg/cache"));
        vars.env("XDG_CONFIG_HOME", Path::new("/xdg/config"));
        vars.env("XDG_DATA_HOME", Path::new("/xdg/data"));
        vars.env("XDG_STATE_HOME", Path::new("/xdg/state"));

        for gate in [None, Some(""), Some("0")] {
            if let Some(gate) = gate {
                vars.env("RUSTUP_USE_CATEGORY_HOME", gate);
            }
            let process = test_process(Path::new("/work"), vars.clone());
            assert_eq!(
                process.home_dirs()?,
                HomeDirs {
                    cache: "/home/.rustup".into(),
                    config: "/home/.rustup".into(),
                    data: "/home/.rustup".into(),
                    state: "/home/.rustup".into(),
                }
            );
            assert_eq!(process.rustup_bin_home()?, Path::new("/home/.cargo/bin"));
            assert_eq!(process.rustup_env_home()?, Path::new("/home/.cargo"));
        }

        vars.env("RUSTUP_HOME", Path::new("/legacy"));
        vars.env("CARGO_HOME", Path::new("/cargo"));
        let process = test_process(Path::new("/work"), vars);
        assert_eq!(
            process.home_dirs()?,
            HomeDirs {
                cache: "/legacy".into(),
                config: "/legacy".into(),
                data: "/legacy".into(),
                state: "/legacy".into(),
            }
        );
        assert_eq!(process.rustup_bin_home()?, Path::new("/cargo/bin"));
        assert_eq!(process.rustup_env_home()?, Path::new("/cargo"));
        Ok(())
    }

    #[test]
    fn category_mode_enabled_uses_explicit_homes_without_home() -> io::Result<()> {
        let cwd = Path::new("/work");
        let mut vars = HashMap::new();
        vars.env("RUSTUP_USE_CATEGORY_HOME", "1");
        vars.env("RUSTUP_HOME", "rustup");
        vars.env("XDG_CACHE_HOME", Path::new("/xdg/cache"));
        vars.env("XDG_CONFIG_HOME", Path::new("/xdg/config"));
        vars.env("XDG_DATA_HOME", Path::new("/xdg/data"));
        vars.env("XDG_STATE_HOME", Path::new("/xdg/state"));

        #[cfg(unix)]
        {
            let process = test_process(cwd, vars.clone());
            assert_eq!(
                process.home_dirs()?,
                HomeDirs {
                    cache: "/xdg/cache/rustup".into(),
                    config: "/xdg/config/rustup".into(),
                    data: "/xdg/data/rustup".into(),
                    state: "/xdg/state/rustup".into(),
                }
            );
            assert_eq!(process.rustup_env_home()?, Path::new("/xdg/config/rustup"));
        }

        vars.env("RUSTUP_CACHE_HOME", "cache");
        vars.env("RUSTUP_CONFIG_HOME", "config");
        vars.env("RUSTUP_DATA_HOME", "data");
        vars.env("RUSTUP_STATE_HOME", "state");
        let process = test_process(cwd, vars);
        assert_eq!(
            process.home_dirs()?,
            HomeDirs {
                cache: "cache".into(),
                config: "config".into(),
                data: "data".into(),
                state: "state".into(),
            }
        );
        Ok(())
    }

    #[test]
    fn category_mode_enabled_ignores_rustup_home() -> io::Result<()> {
        let mut vars = HashMap::new();
        vars.env("RUSTUP_USE_CATEGORY_HOME", "1");
        vars.env("HOME", Path::new("/home"));
        let cwd = Path::new("/work");
        let expected = test_process(cwd, vars.clone()).home_dirs()?;

        for legacy in ["/legacy", "legacy", ""] {
            vars.env("RUSTUP_HOME", legacy);
            let process = test_process(cwd, vars.clone());
            assert_eq!(process.home_dirs()?, expected);
            assert_eq!(process.rustup_env_home()?, expected.config);
        }

        for variable in [
            "RUSTUP_CACHE_HOME",
            "RUSTUP_CONFIG_HOME",
            "RUSTUP_DATA_HOME",
            "RUSTUP_STATE_HOME",
        ] {
            vars.env(variable, "");
        }
        vars.env("RUSTUP_HOME", "legacy");
        assert_eq!(test_process(cwd, vars).home_dirs()?, expected);
        Ok(())
    }

    #[test]
    fn category_mode_enabled_uses_rustup_bin_home_override() -> io::Result<()> {
        let cwd = Path::new("/work");
        let mut vars = HashMap::new();
        vars.env("RUSTUP_USE_CATEGORY_HOME", "1");
        vars.env("RUSTUP_BIN_HOME", "bin");
        vars.env("RUSTUP_CONFIG_HOME", "config");
        vars.env("CARGO_HOME", "cargo");

        let process = test_process(cwd, vars);
        assert_eq!(process.rustup_bin_home()?, Path::new("bin"));
        assert_eq!(process.rustup_env_home()?, Path::new("config"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn category_mode_enabled_uses_unix_platform_defaults() -> io::Result<()> {
        let mut vars = HashMap::new();
        vars.env("RUSTUP_USE_CATEGORY_HOME", "1");
        vars.env("HOME", Path::new("/home"));
        vars.env("RUSTUP_HOME", Path::new("/legacy"));
        vars.env("XDG_STATE_HOME", Path::new("/xdg/state"));

        let homes = test_process(Path::new("/work"), vars.clone()).home_dirs()?;
        assert_eq!(homes.state, Path::new("/xdg/state/rustup"));

        vars.env("XDG_STATE_HOME", Path::new("xdg/state"));
        let process = TestProcess::new(Path::new("/work"), &[] as &[&str], vars.clone(), "");
        let homes = process.process.home_dirs()?;
        assert_eq!(homes.state, Path::new("/home/.local/state/rustup"));
        assert_eq!(
            process.stderr(),
            b"warn: ignoring relative XDG_STATE_HOME path xdg/state; falling back to /home/.local/state\n"
        );

        vars.env("XDG_STATE_HOME", "");
        let homes = test_process(Path::new("/work"), vars).home_dirs()?;
        assert_eq!(homes.state, Path::new("/home/.local/state/rustup"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn category_mode_enabled_ignores_existing_legacy_home() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        fs::create_dir(home.path().join(".rustup"))?;
        let mut vars = HashMap::new();
        vars.env("RUSTUP_USE_CATEGORY_HOME", "1");
        vars.env("HOME", home.path());

        let homes = test_process(Path::new("/work"), vars).home_dirs()?;
        assert_eq!(homes.cache, home.path().join(".cache/rustup"));
        assert_eq!(homes.config, home.path().join(".config/rustup"));
        assert_eq!(homes.data, home.path().join(".local/share/rustup"));
        assert_eq!(homes.state, home.path().join(".local/state/rustup"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn category_mode_enabled_without_home_errors() {
        let mut vars = HashMap::new();
        vars.env("RUSTUP_USE_CATEGORY_HOME", "1");
        vars.env("RUSTUP_HOME", Path::new("/legacy"));
        vars.env("CARGO_HOME", Path::new("/cargo"));
        let process = test_process(Path::new("/work"), vars.clone());

        assert_matches!(
            process.home_dirs(),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && error.to_string() == "home directory is not set"
        );
        assert_matches!(
            process.rustup_env_home(),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && error.to_string() == "home directory is not set"
        );
        assert_matches!(
            process.rustup_bin_home(),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && error.to_string() == "home directory is not set"
        );

        vars.env("HOME", "relative");
        assert_matches!(
            test_process(Path::new("/work"), vars).rustup_bin_home(),
            Err(error)
                if error.kind() == io::ErrorKind::InvalidData
                    && error.to_string() == "home directory is not absolute"
        );
    }

    #[cfg(unix)]
    #[test]
    fn category_mode_enabled_uses_rustup_bin_platform_default() -> io::Result<()> {
        let mut vars = HashMap::new();
        vars.env("RUSTUP_USE_CATEGORY_HOME", "1");
        vars.env("RUSTUP_BIN_HOME", "");
        vars.env("CARGO_HOME", Path::new("/cargo"));
        vars.env("HOME", Path::new("/home"));

        assert_eq!(
            test_process(Path::new("/work"), vars).rustup_bin_home()?,
            Path::new("/home/.local/bin")
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn category_mode_enabled_uses_windows_platform_defaults() -> io::Result<()> {
        let mut vars = HashMap::new();
        vars.env("RUSTUP_USE_CATEGORY_HOME", "1");
        vars.env("CARGO_HOME", Path::new(r"C:\cargo"));
        vars.env("HOME", Path::new(r"C:\Users\rustup-test"));
        let process = test_process(Path::new(r"C:\work"), vars);

        let homes = process.home_dirs()?;
        assert!(homes.cache.is_absolute());
        assert!(homes.config.is_absolute());
        assert_eq!(homes.config, homes.data);
        assert_eq!(homes.config, homes.state);
        assert_ne!(homes.cache, homes.config);
        for home in [&homes.cache, &homes.config, &homes.data, &homes.state] {
            assert_eq!(home.file_name(), Some(OsStr::new("rustup")));
        }
        assert_eq!(
            process.rustup_bin_home()?,
            Path::new(r"C:\Users\rustup-test").join(".local/bin")
        );
        Ok(())
    }

    fn test_process(cwd: &Path, vars: HashMap<String, String>) -> Process {
        Process::TestProcess(TestContext {
            cwd: cwd.into(),
            vars,
            ..Default::default()
        })
    }
}
