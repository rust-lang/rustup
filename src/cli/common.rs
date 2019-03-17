//! Just a dumping ground for cli stuff

use crate::errors::*;
use crate::self_update;
use crate::term2;
use log::{error, info};
use rustup::utils::utils;
use rustup::{Cfg, Notification, Toolchain, UpdateStatus, Verbosity};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use std::{cmp, iter};
use wait_timeout::ChildExt;

pub fn confirm(question: &str, default: bool) -> Result<bool> {
    print!("{} ", question);
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input {
        "y" | "Y" => true,
        "n" | "N" => false,
        "" => default,
        _ => false,
    };

    println!();

    Ok(r)
}

pub enum Confirm {
    Yes,
    No,
    Advanced,
}

pub fn confirm_advanced() -> Result<Confirm> {
    println!();
    println!("1) Proceed with installation (default)");
    println!("2) Customize installation");
    println!("3) Cancel installation");
    print!(">");

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input {
        "1" | "" => Confirm::Yes,
        "2" => Confirm::Advanced,
        _ => Confirm::No,
    };

    println!();

    Ok(r)
}

pub fn question_str(question: &str, default: &str) -> Result<String> {
    println!("{}", question);
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    println!();

    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

pub fn question_bool(question: &str, default: bool) -> Result<bool> {
    println!("{}", question);

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    println!();

    if input.is_empty() {
        Ok(default)
    } else {
        match &*input {
            "y" | "Y" | "yes" => Ok(true),
            "n" | "N" | "no" => Ok(false),
            _ => Ok(default),
        }
    }
}

pub fn read_line() -> Result<String> {
    let stdin = std::io::stdin();
    let stdin = stdin.lock();
    let mut lines = stdin.lines();
    lines
        .next()
        .and_then(|l| l.ok())
        .ok_or("unable to read from stdin for confirmation".into())
}

pub fn set_globals(verbose: bool) -> Result<Cfg> {
    use crate::download_tracker::DownloadTracker;
    use std::cell::RefCell;

    let download_tracker = RefCell::new(DownloadTracker::new());

    let verbosity = if verbose {
        Verbosity::Verbose
    } else {
        Verbosity::NotVerbose
    };

    Ok(Cfg::from_env(
        verbosity,
        Arc::new(move |n: Notification<'_>| {
            if download_tracker.borrow_mut().handle_notification(&n) {
                return;
            }

            n.log_with_verbosity(verbosity);
        }),
    )?)
}

pub fn show_channel_update(
    cfg: &Cfg,
    name: &str,
    updated: rustup::Result<UpdateStatus>,
) -> Result<()> {
    show_channel_updates(cfg, vec![(name.to_string(), updated)])
}

fn show_channel_updates(
    cfg: &Cfg,
    toolchains: Vec<(String, rustup::Result<UpdateStatus>)>,
) -> Result<()> {
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
    let max_width = data
        .iter()
        .fold(0, |a, &(_, _, width, _, _)| cmp::max(a, width));

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

pub fn update_all_channels(cfg: &Cfg, do_self_update: bool, force_update: bool) -> Result<()> {
    let toolchains = cfg.update_all_channels(force_update)?;

    if toolchains.is_empty() {
        info!("no updatable toolchains installed");
    }

    let show_channel_updates = || {
        if !toolchains.is_empty() {
            println!();

            show_channel_updates(cfg, toolchains)?;
        }
        Ok(())
    };

    if do_self_update {
        self_update(show_channel_updates)
    } else {
        show_channel_updates()
    }
}

pub fn self_update<F>(before_restart: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let setup_path = self_update::prepare_update()?;

    before_restart()?;

    if let Some(ref setup_path) = setup_path {
        self_update::run_update(setup_path)?;

        unreachable!(); // update exits on success
    } else {
        // Try again in case we emitted "tool `{}` is already installed" last time.
        self_update::install_proxies()?;
    }

    Ok(())
}

pub fn rustc_version(toolchain: &Toolchain<'_>) -> String {
    if toolchain.exists() {
        let rustc_path = toolchain.binary_file("rustc");
        if utils::is_file(&rustc_path) {
            let mut cmd = Command::new(&rustc_path);
            cmd.arg("--version");
            cmd.stdin(Stdio::null());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            toolchain.set_ldpath(&mut cmd);

            // some toolchains are faulty with some combinations of platforms and
            // may fail to launch but also to timely terminate.
            // (known cases include Rust 1.3.0 through 1.10.0 in recent macOS Sierra.)
            // we guard against such cases by enforcing a reasonable timeout to read.
            let mut line1 = None;
            if let Ok(mut child) = cmd.spawn() {
                let timeout = Duration::new(10, 0);
                match child.wait_timeout(timeout) {
                    Ok(Some(status)) if status.success() => {
                        let out = child
                            .stdout
                            .expect("Child::stdout requested but not present");
                        let mut line = String::new();
                        if BufReader::new(out).read_line(&mut line).is_ok() {
                            let lineend = line.trim_end_matches(&['\r', '\n'][..]).len();
                            line.truncate(lineend);
                            line1 = Some(line);
                        }
                    }
                    Ok(None) => {
                        let _ = child.kill();
                        return String::from("(timeout reading rustc version)");
                    }
                    Ok(Some(_)) | Err(_) => {}
                }
            }

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

pub fn list_targets(toolchain: &Toolchain<'_>) -> Result<()> {
    let mut t = term2::stdout();
    for component in toolchain.list_components()? {
        if component.component.short_name_in_manifest() == "rust-std" {
            let target = component
                .component
                .target
                .as_ref()
                .expect("rust-std should have a target");
            if component.required {
                let _ = t.attr(term2::Attr::Bold);
                let _ = writeln!(t, "{} (default)", target);
                let _ = t.reset();
            } else if component.installed {
                let _ = t.attr(term2::Attr::Bold);
                let _ = writeln!(t, "{} (installed)", target);
                let _ = t.reset();
            } else if component.available {
                let _ = writeln!(t, "{}", target);
            }
        }
    }

    Ok(())
}

pub fn list_components(toolchain: &Toolchain<'_>) -> Result<()> {
    let mut t = term2::stdout();
    for component in toolchain.list_components()? {
        let name = component.name;
        if component.required {
            t.attr(term2::Attr::Bold)?;
            writeln!(t, "{} (default)", name)?;
            t.reset()?;
        } else if component.installed {
            t.attr(term2::Attr::Bold)?;
            writeln!(t, "{} (installed)", name)?;
            t.reset()?;
        } else if component.available {
            writeln!(t, "{}", name)?;
        }
    }

    Ok(())
}

pub fn list_installed_components(toolchain: &Toolchain<'_>) -> Result<()> {
    let mut t = term2::stdout();
    for component in toolchain.list_components()? {
        if component.installed {
            writeln!(t, "{}", component.name)?;
        }
    }
    Ok(())
}

pub fn list_toolchains(cfg: &Cfg) -> Result<()> {
    let toolchains = cfg.list_toolchains()?;

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
    let overrides = cfg.settings_file.with(|s| Ok(s.overrides.clone()))?;

    if overrides.is_empty() {
        println!("no overrides");
    } else {
        let mut any_not_exist = false;
        for (k, v) in overrides {
            let dir_exists = Path::new(&k).is_dir();
            if !dir_exists {
                any_not_exist = true;
            }
            println!(
                "{:<40}\t{:<20}",
                utils::format_path_for_display(&k)
                    + if dir_exists { "" } else { " (not a directory)" },
                v
            )
        }
        if any_not_exist {
            println!();
            info!(
                "you may remove overrides for non-existent directories with
`rustup override unset --nonexistent`"
            );
        }
    }
    Ok(())
}

pub fn version() -> &'static str {
    concat!(
        env!("CARGO_PKG_VERSION"),
        include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt"))
    )
}

pub fn report_error(e: &Error) {
    error!("{}", e);

    for e in e.iter().skip(1) {
        info!("caused by: {}", e);
    }

    if show_backtrace() {
        if let Some(backtrace) = e.backtrace() {
            info!("backtrace:");
            println!();
            println!("{:?}", backtrace);
        }
    }

    fn show_backtrace() -> bool {
        use std::env;
        use std::ops::Deref;

        if env::var("RUST_BACKTRACE").as_ref().map(Deref::deref) == Ok("1") {
            return true;
        }

        for arg in env::args() {
            if arg == "-v" || arg == "--verbose" {
                return true;
            }
        }

        false
    }
}
