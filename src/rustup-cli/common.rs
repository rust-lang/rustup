//! Just a dumping ground for cli stuff

use rustup::{Cfg, Result, Notification, Toolchain, Error, UpdateStatus};
use rustup_utils::{self, utils};
use rustup_utils::notify::NotificationLevel;
use self_update;
use std::ffi::OsStr;
use std::io::{Write, Read, BufRead};
use std::process::{self, Command};
use std::{cmp, iter};
use std::str::FromStr;
use std;
use term2;

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

pub enum Confirm {
    Yes, No, Advanced
}

pub fn confirm_advanced(default: Confirm) -> Result<Confirm> {
    let _ = std::io::stdout().flush();
    let input = try!(read_line());

    let r = match &*input {
        "y" | "Y" | "yes" => Confirm::Yes,
        "n" | "N" | "no" => Confirm::No,
        "a" | "A" | "advanced" => Confirm::Advanced,
        "" => default,
        _ => Confirm::No,
    };

    println!("");

    Ok(r)
}

pub fn question_str(question: &str, default: &str) -> Result<String> {
    println!("{}", question);
    let _ = std::io::stdout().flush();
    let input = try!(read_line());

    println!("");

    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

pub fn question_bool(question: &str, default: bool) -> Result<bool> {
    println!("{}", question);

    let _ = std::io::stdout().flush();
    let input = try!(read_line());

    println!("");

    if input.is_empty() {
        Ok(default)
    } else {
        match &*input {
            "y" | "Y" | "yes" => Ok(true),
            "n" | "N" | "no" => Ok(false),
            _ => Ok(default)
        }
    }

}

pub fn read_line() -> Result<String> {
    let stdin = std::io::stdin();
    let stdin = stdin.lock();
    let mut lines = stdin.lines();
    lines.next().and_then(|l| l.ok()).ok_or(Error::ReadStdin)
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
            Err(rustup_utils::Error::RunningCommand {
                name: args[0].as_ref().to_owned(),
                error: rustup_utils::raw::CommandError::Io(e),
            }.into())
        }
    }
}

pub fn show_channel_update(cfg: &Cfg, name: &str,
                           updated: Result<UpdateStatus>) -> Result<()> {
    show_channel_updates(cfg, vec![(name.to_string(), updated)])
}

fn show_channel_updates(cfg: &Cfg, toolchains: Vec<(String, Result<UpdateStatus>)>) -> Result<()> {
    let data = toolchains.into_iter().map(|(name, result)| {
        let ref toolchain = cfg.get_toolchain(&name, false).expect("");
        let version = rustc_version(toolchain);

        let banner;
        let color;
        match result {
            Ok(UpdateStatus::Installed) => {
                banner = "installed";
                color = Some(term2::color::BRIGHT_GREEN);
            }
            Ok(UpdateStatus::Updated) => {
                banner = "updated";
                color = Some(term2::color::BRIGHT_GREEN);
            }
            Ok(UpdateStatus::Unchanged) => {
                banner = "unchanged";
                color = None;
            }
            Err(_) => {
                banner = "update failed";
                color = Some(term2::color::BRIGHT_RED);
            }
        }

        let width = name.len() + 1 + banner.len();

        (name, banner, width, color, version)
    });

    let mut t = term2::stdout();

    let data: Vec<_> = data.collect();
    let max_width = data.iter().fold(0, |a, &(_, _, width, _, _)| cmp::max(a, width));

    for (name, banner, width, color, version) in data {
        let padding = max_width - width;
        let padding: String = iter::repeat(' ').take(padding).collect();
        let _ = write!(t, "  {}", padding);
        let _ = t.attr(term2::Attr::Bold);
        if let Some(color) = color {
            let _ = t.fg(color);
        }
        let _ = write!(t, "{} ", name);
        let _ = write!(t, "{}", banner);
        let _ = t.reset();
        let _ = writeln!(t, " - {}", version);
    }
    let _ = writeln!(t, "");

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

        try!(show_channel_updates(cfg, toolchains));
    }

    if let Some(ref setup_path) = setup_path {
        try!(self_update::run_update(setup_path));

        unreachable!(); // update exits on success
    }

    Ok(())
}

fn rustc_version(toolchain: &Toolchain) -> String {
    if toolchain.exists() {
        let rustc_path = toolchain.binary_file("rustc");
        if utils::is_file(&rustc_path) {
            let mut cmd = Command::new(&rustc_path);
            cmd.arg("--version");
            toolchain.set_ldpath(&mut cmd);

            let out= cmd.output().ok();
            let out = out.into_iter().filter(|o| o.status.success()).next();
            let stdout = out.and_then(|o| String::from_utf8(o.stdout).ok());
            let line1 = stdout.and_then(|o| o.lines().next().map(|l| l.to_owned()));

            if let Some(line1) = line1 {
                line1.to_owned()
            } else {
                String::from("(error reading rustc version)")
            }
        } else {
            String::from("(rustc does not exist)")
        }
    } else {
        String::from("(toolchain not installed)")
    }
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
        if let Ok(Some(def_toolchain)) = cfg.find_default() {
            for toolchain in toolchains {
                let if_default = if def_toolchain.name() == &*toolchain { 
                    " (default)" 
                } else { 
                    "" 
                };
                println!("{}{}", &toolchain, if_default);
            }

        } else {
            for toolchain in toolchains {
                println!("{}", &toolchain);
            }
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
            split_override::<String>(&o, ';').map(|li| 
                println!("{:<40}\t{:<20}", 
                         utils::format_path_for_display(&li.0), 
                         li.1)
            );
        }
    }
    Ok(())
}


pub fn version() -> &'static str {
    option_env!("CARGO_PKG_VERSION").unwrap_or("unknown")
}

fn split_override<T: FromStr>(s: &str, separator: char) -> Option<(T, T)> {
    s.find(separator).and_then(|index| {
        match (T::from_str(&s[..index]), T::from_str(&s[index + 1..])) {
            (Ok(l), Ok(r)) => Some((l, r)),
            _ => None
        }
    })
}
