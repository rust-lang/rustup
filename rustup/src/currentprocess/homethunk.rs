/// Adapts currentprocess to the trait home::Env
use std::ffi::OsString;
use std::io;
#[cfg(feature = "test")]
use std::ops::Deref;
use std::path::PathBuf;

use home::env as home;

use super::OSProcess;
use super::Process;
#[cfg(feature = "test")]
use super::{CurrentDirSource, TestProcess, VarSource};

impl home::Env for Process {
    fn home_dir(&self) -> Option<PathBuf> {
        match self {
            Process::OSProcess(p) => p.home_dir(),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => p.home_dir(),
        }
    }
    fn current_dir(&self) -> Result<PathBuf, io::Error> {
        match self {
            Process::OSProcess(p) => home::Env::current_dir(p),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => home::Env::current_dir(p),
        }
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        match self {
            Process::OSProcess(p) => home::Env::var_os(p, key),
            #[cfg(feature = "test")]
            Process::TestProcess(p) => home::Env::var_os(p, key),
        }
    }
}

#[cfg(feature = "test")]
impl home::Env for TestProcess {
    fn home_dir(&self) -> Option<PathBuf> {
        self.var("HOME").ok().map(|v| v.into())
    }
    fn current_dir(&self) -> Result<PathBuf, io::Error> {
        CurrentDirSource::current_dir(self.deref())
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        VarSource::var_os(self.deref(), key)
    }
}

impl home::Env for OSProcess {
    fn home_dir(&self) -> Option<PathBuf> {
        home::OS_ENV.home_dir()
    }
    fn current_dir(&self) -> Result<PathBuf, io::Error> {
        home::OS_ENV.current_dir()
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        home::OS_ENV.var_os(key)
    }
}
