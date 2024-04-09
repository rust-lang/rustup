/// Adapts currentprocess to the trait home::Env
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use home::env as home;

use super::{CurrentDirSource, Process};

impl home::Env for Process {
    fn home_dir(&self) -> Option<PathBuf> {
        match self {
            Process::OSProcess(_) => self.var("HOME").ok().map(|v| v.into()),
            #[cfg(feature = "test")]
            Process::TestProcess(_) => home::OS_ENV.home_dir(),
        }
    }

    fn current_dir(&self) -> Result<PathBuf, io::Error> {
        match self {
            Process::OSProcess(_) => CurrentDirSource::current_dir(self),
            #[cfg(feature = "test")]
            Process::TestProcess(_) => home::OS_ENV.current_dir(),
        }
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        match self {
            Process::OSProcess(_) => self.var_os(key),
            #[cfg(feature = "test")]
            Process::TestProcess(_) => self.var_os(key),
        }
    }
}
