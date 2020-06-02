/// Abstracts over reading the current process cwd as a zero-cost abstraction to
/// support threaded in-process testing.
use std::env;
use std::io;
use std::path::PathBuf;

pub trait CurrentDirSource {
    fn current_dir(&self) -> io::Result<PathBuf>;
}

/// Implements VarSource with `std::env::env`
impl CurrentDirSource for super::OSProcess {
    fn current_dir(&self) -> io::Result<PathBuf> {
        env::current_dir()
    }
}

impl CurrentDirSource for super::TestProcess {
    fn current_dir(&self) -> io::Result<PathBuf> {
        Ok(self.cwd.clone())
    }
}
