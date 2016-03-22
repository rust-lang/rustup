//! Just a dumping ground for cli stuff

use multirust::{Cfg, Result, Notification, Toolchain, Error};
use multirust_utils::{self, utils};
use multirust_utils::notify::NotificationLevel;
use self_update;
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
    println!("{} revision:", name);
    println!("");
    try!(show_tool_versions(&try!(cfg.get_toolchain(&name, false))));
    println!("");
    Ok(())
}

pub fn show_channel_update(cfg: &Cfg, name: &str,
                           updated: Result<bool>) -> Result<()> {
    let tty = tty::stdout_isatty();
    let mut t = term::stdout().unwrap();
    match updated {
        Ok(true) => {
            if tty { let _ = t.fg(term::color::BRIGHT_GREEN); }
            let _ = write!(t, "{} updated", name);
        }
        Ok(false) => {
            let _ = write!(t, "{} unchanged", name);
        }
        Err(_) => {
            if tty { let _ = t.fg(term::color::BRIGHT_RED); }
            let _ = write!(t, "{} update failed", name);
        }
    }
    if tty {let _ = t.reset(); }
    println!(":");
    println!("");
    try!(show_tool_versions(&try!(cfg.get_toolchain(&name, false))));
    println!("");
    Ok(())
}

pub fn update_all_channels(cfg: &Cfg, self_update: bool) -> Result<()> {

    let toolchains = try!(cfg.update_all_channels());

    if toolchains.is_empty() {
        info!("no updatable toolchains installed");
    }

    let setup_path = if self_update {
        try!(self_update::prepare_update())
    } else {
        None
    };

    if !toolchains.is_empty() {
        println!("");

        for (name, result) in toolchains {
            try!(show_channel_update(cfg, &name, result));
        }
    }

    if let Some(ref setup_path) = setup_path {
        try!(self_update::run_update(setup_path));

        unreachable!(); // update exits on success
    }

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

pub fn list_targets(toolchain: &Toolchain) -> Result<()> {
    for component in try!(toolchain.list_components()) {
        if component.component.pkg == "rust-std" {
            if component.required {
                println!("{} (default)", component.component.target);
            } else if component.installed {
                println!("{} (installed)", component.component.target);
            } else {
                println!("{}", component.component.target);
            }
        }
    }

    Ok(())
}

pub fn list_toolchains(cfg: &Cfg) -> Result<()> {
    let mut toolchains = try!(cfg.list_toolchains());

    toolchains.sort();

    if toolchains.is_empty() {
        println!("no installed toolchains");
    } else {
        for toolchain in toolchains {
            println!("{}", &toolchain);
        }
    }
    Ok(())
}

pub fn list_overrides(cfg: &Cfg) -> Result<()> {
    let mut overrides = try!(cfg.override_db.list());

    overrides.sort();

    if overrides.is_empty() {
        println!("no overrides");
    } else {
        for o in overrides {
            println!("{}", o);
        }
    }
    Ok(())
}

