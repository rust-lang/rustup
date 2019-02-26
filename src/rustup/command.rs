use std::ffi::OsStr;
use std::io;
use std::process::{self, Command};

use crate::errors::*;
use rustup_utils::utils::ExitCode;

pub fn run_command_for_dir<S: AsRef<OsStr>>(
    mut cmd: Command,
    arg0: &str,
    args: &[S],
) -> Result<ExitCode> {
    cmd.args(args);

    // FIXME rust-lang/rust#32254. It's not clear to me
    // when and why this is needed.
    cmd.stdin(process::Stdio::inherit());

    return exec(&mut cmd).chain_err(|| rustup_utils::ErrorKind::RunningCommand {
        name: OsStr::new(arg0).to_owned(),
    });

    #[cfg(unix)]
    fn exec(cmd: &mut Command) -> io::Result<ExitCode> {
        use std::os::unix::prelude::*;
        Err(cmd.exec())
    }

    #[cfg(windows)]
    fn exec(cmd: &mut Command) -> io::Result<ExitCode> {
        let status = cmd.status()?;
        Ok(ExitCode(status.code().unwrap()))
    }
}
