use std::{
    ffi::OsStr,
    fmt::Debug,
    io,
    process::{self, Command},
};

use anyhow::{Context, Result};

use crate::errors::*;
use crate::utils::utils::ExitCode;

#[cfg_attr(feature = "otel", tracing::instrument(err))]
pub(crate) fn run_command_for_dir<S: AsRef<OsStr> + Debug>(
    mut cmd: Command,
    arg0: &str,
    args: &[S],
) -> Result<ExitCode> {
    cmd.args(args);

    // FIXME rust-lang/rust#32254. It's not clear to me
    // when and why this is needed.
    // TODO: currentprocess support for mocked file descriptor inheritance here: until
    // then tests that depend on rustups stdin being inherited won't work in-process.
    cmd.stdin(process::Stdio::inherit());

    return exec(&mut cmd).with_context(|| RustupError::RunningCommand {
        name: OsStr::new(arg0).to_owned(),
    });

    #[cfg(unix)]
    fn exec(cmd: &mut Command) -> io::Result<ExitCode> {
        use std::os::unix::prelude::*;
        Err(cmd.exec())
    }

    #[cfg(windows)]
    fn exec(cmd: &mut Command) -> io::Result<ExitCode> {
        use windows_sys::Win32::Foundation::{BOOL, FALSE, TRUE};
        use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

        unsafe extern "system" fn ctrlc_handler(_: u32) -> BOOL {
            // Do nothing. Let the child process handle it.
            TRUE
        }
        unsafe {
            if SetConsoleCtrlHandler(Some(ctrlc_handler), TRUE) == FALSE {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Unable to set console handler",
                ));
            }
        }
        let status = cmd.status()?;
        Ok(ExitCode(status.code().unwrap()))
    }
}
