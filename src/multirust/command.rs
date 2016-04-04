use std;
use std::env;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{self, Command, Output};
use std::time::Instant;

use Cfg;
use errors::*;
use multirust_utils;
use telemetry;
use telemetry::TelemetryEvent;


pub fn run_command_for_dir<S: AsRef<OsStr>>(cmd: Command,
                                            args: &[S],
                                            cfg: &Cfg) -> Result<()> {
    let arg0 = env::args().next().map(|a| PathBuf::from(a));
    let arg0 = arg0.as_ref()
        .and_then(|a| a.file_name())
        .and_then(|a| a.to_str());
    let arg0 = try!(arg0.ok_or(Error::NoExeName));
    if (arg0 == "rustc" || arg0 == "rustc.exe") && cfg.telemetry_enabled() {
        return telemetry_rustc(cmd, &args, &cfg);
    }
    
    run_command_for_dir_without_telemetry(cmd, &args)
}

fn telemetry_rustc<S: AsRef<OsStr>>(cmd: Command, args: &[S], cfg: &Cfg) -> Result<()> {
    let now = Instant::now();

    let output = bare_run_command_for_dir(cmd, &args);

    let duration = now.elapsed();

    let ms = (duration.as_secs() as u64 * 1000) + (duration.subsec_nanos() as u64 / 1000 / 1000);

    match output {
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(1);

            let errors = match out.status.success() {
                true => None,
                _ => Some(String::from_utf8_lossy(&out.stderr).to_string()),
            };

            let _ = io::stdout().write(&out.stdout);
            let _ = io::stdout().flush();
            let _ = io::stderr().write(&out.stderr);
            let _ = io::stderr().flush();

            let te = TelemetryEvent::RustcRun { duration_ms: ms, 
                                                exit_code: exit_code,
                                                errors: errors };
            telemetry::log_telemetry(te, cfg);
            process::exit(exit_code);
        },
        Err(e) => {
            let exit_code = e.raw_os_error().unwrap_or(1);
            let te = TelemetryEvent::RustcRun { duration_ms: ms,
                                                exit_code: exit_code,
                                                errors: Some(format!("{}", e)) };
            telemetry::log_telemetry(te, cfg);
            Err(multirust_utils::Error::RunningCommand {    
                name: args[0].as_ref().to_owned(),
                error: multirust_utils::raw::CommandError::Io(e),
            }.into())
        },
    }
}

fn run_command_for_dir_without_telemetry<S: AsRef<OsStr>>(cmd: Command, args: &[S]) -> Result<()>  {
    let output = bare_run_command_for_dir(cmd, &args);

    match output {
        Ok(out) => {
            let _ = io::stdout().write(&out.stdout);
            let _ = io::stdout().flush();
            let _ = io::stderr().write(&out.stderr);
            let _ = io::stderr().flush();

            let status = out.status;
            // Ensure correct exit code is returned
            let code = status.code().unwrap_or(1);
            process::exit(code);
        }
        Err(e) => {
            Err(multirust_utils::Error::RunningCommand {
                name: args[0].as_ref().to_owned(),
                error: multirust_utils::raw::CommandError::Io(e),
            }.into())
        }
    }    
}

fn bare_run_command_for_dir<S: AsRef<OsStr>>(mut cmd: Command, args: &[S]) -> std::result::Result<Output, std::io::Error> {
    cmd.args(&args[1..]);

    // FIXME rust-lang/rust#32254. It's not clear to me
    // when and why this is needed.
    cmd.stdin(process::Stdio::inherit());

    cmd.output()
}



