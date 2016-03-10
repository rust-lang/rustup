//! Just a dumping ground for cli stuff

use multirust::{Cfg, Result, Notification};
use multirust_utils;
use multirust_utils::notify::NotificationLevel;
use std::ffi::OsStr;
use std::io::{Write, BufRead};
use std::process::Command;
use std;

pub fn ask(question: &str) -> Option<bool> {
    print!("{} (y/n) ", question);
    let _ = std::io::stdout().flush();
    let input = read_line();

    match &*input {
        "y" | "Y" => Some(true),
        "n" | "N" => Some(false),
        _ => None,
    }
}

fn read_line() -> String {
    let stdin = std::io::stdin();
    let stdin = stdin.lock();
    let mut lines = stdin.lines();
    lines.next().unwrap().unwrap()
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

pub fn run_inner<S: AsRef<OsStr>>(_: &Cfg, command: Result<Command>,
                                  args: &[S]) -> Result<()> {
    if let Ok(mut command) = command {
        for arg in &args[1..] {
            if arg.as_ref() == OsStr::new("--multirust") {
                println!("Proxied via multirust");
                std::process::exit(0);
            } else {
                command.arg(arg);
            }
        }
        match command.status() {
            Ok(result) => {
                // Ensure correct exit code is returned
                std::process::exit(result.code().unwrap_or(1));
            }
            Err(e) => {
                Err(multirust_utils::Error::RunningCommand {
                        name: args[0].as_ref().to_owned(),
                        error: multirust_utils::raw::CommandError::Io(e),
                    }
                    .into())
            }
        }

    } else {
        for arg in &args[1..] {
            if arg.as_ref() == OsStr::new("--multirust") {
                println!("Proxied via multirust");
                std::process::exit(0);
            }
        }
        command.map(|_| ())
    }
}
