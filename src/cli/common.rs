//! Just a dumping ground for cli stuff

use crate::errors::*;
use crate::self_update;
use crate::term2;
use anyhow;
use anyhow::Context;
use git_testament::{git_testament, render_testament};
use lazy_static::lazy_static;
use rustup::dist::notifications as dist_notifications;
use rustup::errors::SyncError;
use rustup::toolchain::DistributableToolchain;
use rustup::utils::notifications as util_notifications;
use rustup::utils::notify::NotificationLevel;
use rustup::utils::utils;
use rustup::{Cfg, Notification, Toolchain, UpdateStatus};
use std::fs;
use std::io::{stdin, ErrorKind, Write};
use std::path::Path;
use std::sync::Arc;
use std::{cmp, env, iter};
use term2::Terminal;

pub const WARN_COMPLETE_PROFILE: &str = "downloading with complete profile isn't recommended unless you are a developer of the rust language";

pub fn confirm(question: &str, default: bool) -> anyhow::Result<bool> {
    print!("{} ", question);
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input.to_lowercase() {
        "y" | "yes" => true,
        "n" | "no" => false,
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

pub fn confirm_advanced() -> anyhow::Result<Confirm> {
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

pub fn question_str(question: &str, default: &str) -> anyhow::Result<String> {
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

pub fn question_bool(question: &str, default: bool) -> anyhow::Result<bool> {
    println!("{}", question);

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    println!();

    if input.is_empty() {
        Ok(default)
    } else {
        match &*input.to_lowercase() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => Ok(default),
        }
    }
}

pub fn read_line() -> anyhow::Result<String> {
    let mut line = String::new();
    match stdin().read_line(&mut line) {
        Ok(0) => Err(anyhow::anyhow!("End of file on stdin")),
        Ok(_) => Ok(line.trim().to_owned()),
        Err(error) => Err(error).context("unable to read from stdin for confirmation"),
    }
}

#[derive(Default)]
struct NotifyOnConsole {
    ram_notice_shown: bool,
    verbose: bool,
}

impl NotifyOnConsole {
    fn handle(&mut self, n: Notification<'_>) {
        if let Notification::Install(dist_notifications::Notification::Utils(
            util_notifications::Notification::SetDefaultBufferSize(_),
        )) = &n
        {
            if self.ram_notice_shown {
                return;
            } else {
                self.ram_notice_shown = true;
            }
        };
        let level = n.level();
        for n in format!("{}", n).lines() {
            match level {
                NotificationLevel::Verbose => {
                    if self.verbose {
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
        }
    }
}

pub fn set_globals(verbose: bool, quiet: bool) -> anyhow::Result<Cfg> {
    use crate::download_tracker::DownloadTracker;
    use std::cell::RefCell;

    let download_tracker = RefCell::new(DownloadTracker::new().with_display_progress(!quiet));
    let console_notifier = RefCell::new(NotifyOnConsole {
        verbose,
        ..Default::default()
    });

    Ok(Cfg::from_env(Arc::new(move |n: Notification<'_>| {
        if download_tracker.borrow_mut().handle_notification(&n) {
            return;
        }
        console_notifier.borrow_mut().handle(n);
    }))?)
}

pub fn show_channel_update(
    cfg: &Cfg,
    name: &str,
    updated: anyhow::Result<UpdateStatus>,
) -> anyhow::Result<()> {
    show_channel_updates(cfg, vec![(name.to_string(), updated)])
}

fn show_channel_updates(
    cfg: &Cfg,
    toolchains: Vec<(String, anyhow::Result<UpdateStatus>)>,
) -> anyhow::Result<()> {
    let data = toolchains.into_iter().map(|(name, result)| {
        let toolchain = cfg.get_toolchain(&name, false).unwrap();
        let version = toolchain.rustc_version();

        let banner;
        let color;
        let mut previous_version: Option<String> = None;
        match result {
            Ok(UpdateStatus::Installed) => {
                banner = "installed";
                color = Some(term2::color::GREEN);
            }
            Ok(UpdateStatus::Updated(v)) => {
                previous_version = Some(v);
                banner = "updated";
                color = Some(term2::color::GREEN);
            }
            Ok(UpdateStatus::Unchanged) => {
                banner = "unchanged";
                color = None;
            }
            Err(_) => {
                banner = "update failed";
                color = Some(term2::color::RED);
            }
        }

        let width = name.len() + 1 + banner.len();

        (name, banner, width, color, version, previous_version)
    });

    let mut t = term2::stdout();

    let data: Vec<_> = data.collect();
    let max_width = data
        .iter()
        .fold(0, |a, &(_, _, width, _, _, _)| cmp::max(a, width));

    for (name, banner, width, color, version, previous_version) in data {
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
        let _ = write!(t, " - {}", version);
        if let Some(previous_version) = previous_version {
            let _ = write!(t, " (from {})", previous_version);
        }
        let _ = writeln!(t);
    }
    let _ = writeln!(t);

    Ok(())
}

pub fn update_all_channels(
    cfg: &Cfg,
    do_self_update: bool,
    force_update: bool,
) -> anyhow::Result<()> {
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

#[derive(Clone, Copy, Debug)]
pub enum SelfUpdatePermission {
    HardFail,
    Skip,
    Permit,
}

pub fn self_update_permitted(explicit: bool) -> Result<SelfUpdatePermission> {
    if cfg!(windows) {
        Ok(SelfUpdatePermission::Permit)
    } else {
        // Detect if rustup is not meant to self-update
        match env::var("SNAP") {
            Ok(_) => {
                // We're running under snappy so don't even bother
                // trying to self-update
                // TODO: Report this to the user?
                // TODO: Maybe ask snapd if there's an update and report
                //       that to the user instead?
                debug!("Skipping self-update because SNAP was detected");
                if explicit {
                    return Ok(SelfUpdatePermission::HardFail);
                } else {
                    return Ok(SelfUpdatePermission::Skip);
                }
            }
            Err(env::VarError::NotPresent) => {}
            Err(e) => {
                return Err(
                    format!("Could not interrogate SNAP environment variable: {}", e).into(),
                )
            }
        }
        let current_exe = env::current_exe()?;
        let current_exe_dir = current_exe.parent().expect("Rustup isn't in a directory‽");
        if let Err(e) = tempfile::Builder::new()
            .prefix("updtest")
            .tempdir_in(current_exe_dir)
        {
            match e.kind() {
                ErrorKind::PermissionDenied => {
                    debug!("Skipping self-update because we cannot write to the rustup dir");
                    if explicit {
                        return Ok(SelfUpdatePermission::HardFail);
                    } else {
                        return Ok(SelfUpdatePermission::Skip);
                    }
                }
                _ => return Err(e.into()),
            }
        }
        Ok(SelfUpdatePermission::Permit)
    }
}

pub fn self_update<F>(before_restart: F) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    match SyncError::maybe(self_update_permitted(false))? {
        SelfUpdatePermission::HardFail => {
            err!("Unable to self-update.  STOP");
            std::process::exit(1);
        }
        SelfUpdatePermission::Skip => return Ok(()),
        SelfUpdatePermission::Permit => {}
    }

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

pub fn list_targets(toolchain: &Toolchain<'_>) -> anyhow::Result<()> {
    let mut t = term2::stdout();
    let distributable = DistributableToolchain::new_for_components(&toolchain)?;
    let components = SyncError::maybe(distributable.list_components())?;
    for component in components {
        if component.component.short_name_in_manifest() == "rust-std" {
            let target = component
                .component
                .target
                .as_ref()
                .expect("rust-std should have a target");
            if component.installed {
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

pub fn list_installed_targets(toolchain: &Toolchain<'_>) -> anyhow::Result<()> {
    let mut t = term2::stdout();
    let distributable = DistributableToolchain::new_for_components(&toolchain)?;
    let components = SyncError::maybe(distributable.list_components())?;
    for component in components {
        if component.component.short_name_in_manifest() == "rust-std" {
            let target = component
                .component
                .target
                .as_ref()
                .expect("rust-std should have a target");
            if component.installed {
                writeln!(t, "{}", target)?;
            }
        }
    }
    Ok(())
}

pub fn list_components(toolchain: &Toolchain<'_>) -> Result<()> {
    let mut t = term2::stdout();
    let distributable = DistributableToolchain::new(&toolchain)
        .chain_err(|| rustup::ErrorKind::ComponentsUnsupported(toolchain.name().to_string()))?;
    let components = distributable.list_components()?;
    for component in components {
        let name = component.name;
        if component.installed {
            t.attr(term2::Attr::Bold)?;
            writeln!(t, "{} (installed)", name)?;
            t.reset()?;
        } else if component.available {
            writeln!(t, "{}", name)?;
        }
    }

    Ok(())
}

pub fn list_installed_components(toolchain: &Toolchain<'_>) -> anyhow::Result<()> {
    let mut t = term2::stdout();
    let distributable = DistributableToolchain::new_for_components(&toolchain)?;
    let components = SyncError::maybe(distributable.list_components())?;
    for component in components {
        if component.installed {
            writeln!(t, "{}", component.name)?;
        }
    }
    Ok(())
}

fn print_toolchain_path(cfg: &Cfg, toolchain: &str, if_default: &str, verbose: bool) -> Result<()> {
    let toolchain_path = {
        let mut t_path = cfg.toolchains_dir.clone();
        t_path.push(&toolchain);
        t_path
    };
    let toolchain_meta = fs::symlink_metadata(&toolchain_path)?;
    let toolchain_path = if verbose {
        if toolchain_meta.is_dir() {
            format!("\t{}", toolchain_path.display())
        } else {
            format!("\t{}", fs::read_link(toolchain_path)?.display())
        }
    } else {
        String::new()
    };
    println!("{}{}{}", &toolchain, if_default, toolchain_path);
    Ok(())
}

pub fn list_toolchains(cfg: &Cfg, verbose: bool) -> anyhow::Result<()> {
    let toolchains = SyncError::maybe(cfg.list_toolchains())?;
    if toolchains.is_empty() {
        println!("no installed toolchains");
    } else if let Ok(Some(def_toolchain)) = cfg.find_default() {
        for toolchain in toolchains {
            let if_default = if def_toolchain.name() == &*toolchain {
                " (default)"
            } else {
                ""
            };
            print_toolchain_path(cfg, &toolchain, if_default, verbose)
                .expect("Failed to list toolchains' directories");
        }
    } else {
        for toolchain in toolchains {
            print_toolchain_path(cfg, &toolchain, "", verbose)
                .expect("Failed to list toolchains' directories");
        }
    }
    Ok(())
}

pub fn list_overrides(cfg: &Cfg) -> anyhow::Result<()> {
    let overrides = SyncError::maybe(cfg.settings_file.with(|s| Ok(s.overrides.clone())))?;

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

git_testament!(TESTAMENT);

pub fn version() -> &'static str {
    lazy_static! {
        // Because we trust our `stable` branch given the careful release
        // process, we mark it trusted here so that our version numbers look
        // right when built from CI before the tag is pushed
        static ref RENDERED: String = render_testament!(TESTAMENT, "stable");
    }
    &RENDERED
}

pub fn dump_testament() {
    use git_testament::GitModification::*;
    println!("Rustup version renders as: {}", version());
    println!("Current crate version: {}", env!("CARGO_PKG_VERSION"));
    if TESTAMENT.branch_name.is_some() {
        println!("Built from branch: {}", TESTAMENT.branch_name.unwrap());
    } else {
        println!("Branch information missing");
    }
    println!("Commit info: {}", TESTAMENT.commit);
    if TESTAMENT.modifications.is_empty() {
        println!("Working tree is clean");
    } else {
        for fmod in TESTAMENT.modifications {
            match fmod {
                Added(f) => println!("Added: {}", String::from_utf8_lossy(f)),
                Removed(f) => println!("Removed: {}", String::from_utf8_lossy(f)),
                Modified(f) => println!("Modified: {}", String::from_utf8_lossy(f)),
                Untracked(f) => println!("Untracked: {}", String::from_utf8_lossy(f)),
            }
        }
    }
}

fn show_backtrace() -> bool {
    if let Ok(true) = env::var("RUSTUP_NO_BACKTRACE").map(|s| s == "1") {
        return false;
    }

    if let Ok(true) = env::var("RUST_BACKTRACE").map(|s| s == "1") {
        return true;
    }

    for arg in env::args() {
        if arg == "-v" || arg == "--verbose" {
            return true;
        }
    }

    false
}

pub fn report_error(e: &anyhow::Error) {
    // NB: This shows one error: but we get to skip all the backtrace config
    // logic etc, rather than per line of the backtrace. This seems like a
    // reasonable tradeoff, but if we want to do differently, this is the code
    // hunk to revisit, that and a similar build.rs auto-detect glue as anyhow
    // has to detect when backtrace is available.
    if show_backtrace() {
        err!("{:?}", e);
    } else {
        err!("{}", e);
        let causes = e.chain().skip(1).collect::<Vec<_>>();
        if causes.len() > 0 {
            err!("Caused by:");
        }
        for cause in causes.into_iter() {
            err!("{}", cause);
        }
    }
}

pub fn ignorable_error(error: &'static str, no_prompt: bool) -> anyhow::Result<()> {
    let error = anyhow::anyhow!(error);
    report_error(&error);
    if no_prompt {
        warn!("continuing (because the -y flag is set and the error is ignorable)");
        Ok(())
    } else if confirm("\nContinue? (y/N)", false).unwrap_or(false) {
        Ok(())
    } else {
        Err(error)
    }
}
