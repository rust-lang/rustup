use std::env;
use std::ffi::OsString;
use std::fmt::Debug;
use std::io;
use std::io::IsTerminal;
use std::path::PathBuf;
#[cfg(feature = "test")]
use std::{
    collections::HashMap,
    io::Cursor,
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
#[cfg(feature = "test")]
use tracing::subscriber::DefaultGuard;
#[cfg(feature = "test")]
use tracing_subscriber::util::SubscriberInitExt;
#[cfg(feature = "test")]
use tracing_subscriber::{EnvFilter, Registry, reload::Handle};

#[cfg(all(feature = "test", feature = "otel"))]
use crate::cli::log;

pub mod filesource;
pub mod terminalsource;

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
            .and_then(std::ffi::OsStr::to_str)
            .map(String::from)
    }

    pub(crate) fn home_dir(&self) -> Option<PathBuf> {
        home::env::home_dir_with_env(self)
    }

    pub(crate) fn cargo_home(&self) -> Result<PathBuf> {
        home::env::cargo_home_with_env(self).context("failed to determine cargo home")
    }

    pub(crate) fn rustup_home(&self) -> Result<PathBuf> {
        home::env::rustup_home_with_env(self).context("failed to determine rustup home dir")
    }

    pub fn var(&self, key: &str) -> Result<String, env::VarError> {
        match self {
            Process::OsProcess(_) => env::var(key),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => match p.vars.get(key) {
                Some(val) => Ok(val.to_owned()),
                None => Err(env::VarError::NotPresent),
            },
        }
    }

    pub(crate) fn var_os(&self, key: &str) -> Option<OsString> {
        match self {
            Process::OsProcess(_) => env::var_os(key),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => p.vars.get(key).map(OsString::from),
        }
    }

    pub(crate) fn args(&self) -> Box<dyn Iterator<Item = String> + '_> {
        match self {
            Process::OsProcess(_) => Box::new(env::args()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(p.args.iter().cloned()),
        }
    }

    pub(crate) fn args_os(&self) -> Box<dyn Iterator<Item = OsString> + '_> {
        match self {
            Process::OsProcess(_) => Box::new(env::args_os()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(p.args.iter().map(OsString::from)),
        }
    }

    pub(crate) fn stdin(&self) -> Box<dyn filesource::Stdin> {
        match self {
            Process::OsProcess(_) => Box::new(io::stdin()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(filesource::TestStdin(p.stdin.clone())),
        }
    }

    pub(crate) fn stdout(&self) -> Box<dyn filesource::Writer> {
        match self {
            Process::OsProcess(_) => Box::new(io::stdout()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(filesource::TestWriter(p.stdout.clone())),
        }
    }

    pub(crate) fn stderr(&self) -> Box<dyn filesource::Writer> {
        match self {
            Process::OsProcess(_) => Box::new(io::stderr()),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Box::new(filesource::TestWriter(p.stderr.clone())),
        }
    }

    pub fn current_dir(&self) -> io::Result<PathBuf> {
        match self {
            Process::OsProcess(_) => env::current_dir(),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => Ok(p.cwd.clone()),
        }
    }
}

impl home::env::Env for Process {
    fn home_dir(&self) -> Option<PathBuf> {
        match self {
            Process::OsProcess(_) => home::env::OS_ENV.home_dir(),
            #[cfg(feature = "test")]
            Process::TestProcess(_) => self.var("HOME").ok().map(|v| v.into()),
        }
    }

    fn current_dir(&self) -> Result<PathBuf, io::Error> {
        match self {
            Process::OsProcess(_) => home::env::OS_ENV.current_dir(),
            #[cfg(feature = "test")]
            Process::TestProcess(_) => self.current_dir(),
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
        OsProcess {
            stderr_is_a_tty: io::stderr().is_terminal(),
            stdout_is_a_tty: io::stdout().is_terminal(),
        }
    }
}

impl Default for OsProcess {
    fn default() -> Self {
        OsProcess::new()
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
        let tp = match &self.process {
            Process::TestProcess(tp) => tp,
            _ => unreachable!(),
        };

        tp.stdout.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Extracts the stderr from the process
    pub fn stderr(&self) -> Vec<u8> {
        let tp = match &self.process {
            Process::TestProcess(tp) => tp,
            _ => unreachable!(),
        };

        tp.stderr.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(feature = "test")]
impl From<TestContext> for TestProcess {
    fn from(inner: TestContext) -> Self {
        let inner = Process::TestProcess(inner);
        let (tracing_subscriber, console_filter) = crate::cli::log::tracing_subscriber(&inner);
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
    stdin: filesource::TestStdinInner,
    stdout: filesource::TestWriterInner,
    stderr: filesource::TestWriterInner,
}
