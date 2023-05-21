/// Adapts currentprocess to the trait home::Env
use std::ffi::OsString;
use std::io;
#[cfg(feature = "test")]
use std::ops::Deref;
use std::path::PathBuf;

use home::env as home;

use super::HomeProcess;
use super::OSProcess;
#[cfg(feature = "test")]
use super::{CurrentDirSource, TestProcess, VarSource};

impl home::Env for Box<dyn HomeProcess + 'static> {
    fn home_dir(&self) -> Option<PathBuf> {
        (**self).home_dir()
    }
    fn current_dir(&self) -> Result<PathBuf, io::Error> {
        home::Env::current_dir(&(**self))
    }
    fn var_os(&self, key: &str) -> Option<OsString> {
        home::Env::var_os(&(**self), key)
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
