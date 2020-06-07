/// Adapts currentprocess to the trait home::Env
use std::ffi::OsString;
use std::io;
use std::ops::Deref;
use std::path::PathBuf;

use super::CurrentDirSource;
use super::CurrentProcess;
use super::VarSource;

impl home::Env for Box<dyn CurrentProcess + 'static> {
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
