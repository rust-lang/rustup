use std::{io, path::PathBuf};

use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum CliError {
    #[error("couldn't determine self executable name")]
    NoExeName,
    #[error("rustup is not installed at '{}'", .p.display())]
    NotSelfInstalled { p: PathBuf },
    #[error("failure reading directory {}", .p.display())]
    ReadDirError { p: PathBuf, source: io::Error },
    #[error("failure during windows uninstall")]
    WindowsUninstallMadness,
}
