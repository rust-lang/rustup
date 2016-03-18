//! Just a dumping ground for cli stuff

use multirust::{Cfg, Result, Notification, Toolchain, Error};
use multirust_utils::{self, utils};
use multirust_utils::notify::NotificationLevel;
use std::ffi::OsStr;
use std::io::{Write, Read, BufRead};
use std::process::{self, Command};
use std;
use tty;
use term;

pub fn confirm(question: &str, default: bool) -> Result<bool> {
    print!("{} ", question);
    let _ = std::io::stdout().flush();
    let input = try!(read_line());

    let r = match &*input {
        "y" | "Y" => true,
        "n" | "N" => false,
        "" => default,
        _ => false,
    };

    println!("");

    Ok(r)
}

fn read_line() -> Result<String> {
    let stdin = std::io::stdin();
    let stdin = stdin.lock();
    let mut lines = stdin.lines();
    lines.next().and_then(|l| l.ok()).ok_or(Error::ReadStdin)
}

pub fn wait_for_keypress() -> Result<()> {
    let stdin = std::io::stdin();
    if stdin.bytes().next().is_some() {
        Ok(())
    } else {
        Err(Error::ReadStdin)
    }
}

pub fn set_globals(verbose: bool) -> Result<Cfg> {
    use download_tracker::DownloadTracker;
    use std::cell::RefCell;

    let download_tracker = RefCell::new(DownloadTracker::new());

    Cfg::from_env(shared_ntfy!(move |n: Notification| {
        if download_tracker.borrow_mut().handle_notification(&n) {
            return;
        }

        match n.level() {
            NotificationLevel::Verbose => {
                if verbose {
                    verbose!("{}", n);
                }
            }
            NotificationLevel::Info => {
                info!("{}", n);
            }
            NotificationLevel::Warn => {
                warn!("{}", n);
            }
            NotificationLevel::Error => {
                err!("{}", n);
            }
        }
    }))

}

pub fn run_inner<S: AsRef<OsStr>>(mut command: Command,
                                  args: &[S]) -> Result<()> {
    command.args(&args[1..]);
    // FIXME rust-lang/rust#32254. It's not clear to me
    // when and why this is needed.
    command.stdin(process::Stdio::inherit());

    let status = command.status();

    match status {
        Ok(status) => {
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

pub fn show_channel_version(cfg: &Cfg, name: &str) -> Result<()> {
    println!("");
    let mut t = term::stdout().unwrap();
    if tty::stdout_isatty() { let _ = t.fg(term::color::BRIGHT_WHITE); }
    if tty::stdout_isatty() { let _ = t.bg(term::color::BLACK); }
    let _ = write!(t, "{}", name);
    if tty::stdout_isatty() { let _ = t.reset(); }
    let _ = writeln!(t, " revision:");
    println!("");
    try!(show_tool_versions(&try!(cfg.get_toolchain(&name, false))));
    println!("");
    Ok(())
}

pub fn show_tool_versions(toolchain: &Toolchain) -> Result<()> {
    if toolchain.exists() {
        let rustc_path = toolchain.binary_file("rustc");
        let cargo_path = toolchain.binary_file("cargo");

        if utils::is_file(&rustc_path) {
            let mut cmd = Command::new(&rustc_path);
            cmd.arg("--version");
            toolchain.set_ldpath(&mut cmd);

            if utils::cmd_status("rustc", &mut cmd).is_err() {
                println!("(failed to run rustc)");
            }
        } else {
            println!("(no rustc command in toolchain?)");
        }
        if utils::is_file(&cargo_path) {
            let mut cmd = Command::new(&cargo_path);
            cmd.arg("--version");
            // cargo invokes rustc during --version, this
            // makes sure it can find it since it may not
            // be on the `PATH` and multirust does not
            // manipulate `PATH`.
            cmd.env("RUSTC", rustc_path);
            toolchain.set_ldpath(&mut cmd);

            if utils::cmd_status("cargo", &mut cmd).is_err() {
                println!("(failed to run cargo)");
            }
        } else {
            println!("(no cargo command in toolchain?)");
        }
    } else {
        println!("(toolchain not installed)");
    }
    Ok(())
}

