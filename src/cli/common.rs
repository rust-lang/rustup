//! Just a dumping ground for cli stuff

use std::fs;
use std::io::{BufRead, ErrorKind, Write};
use std::path::Path;
use std::sync::Arc;
use std::{cmp, env, iter};

use anyhow::{anyhow, Context, Result};
use git_testament::{git_testament, render_testament};
use lazy_static::lazy_static;
use term2::Terminal;

use super::self_update;
use super::term2;
use crate::dist::notifications as dist_notifications;
use crate::process;
use crate::toolchain::DistributableToolchain;
use crate::utils::notifications as util_notifications;
use crate::utils::notify::NotificationLevel;
use crate::utils::utils;
use crate::{Cfg, Notification, Toolchain, UpdateStatus};

pub const WARN_COMPLETE_PROFILE: &str = "downloading with complete profile isn't recommended unless you are a developer of the rust language";

pub fn confirm(question: &str, default: bool) -> Result<bool> {
    write!(process().stdout(), "{} ", question)?;
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input.to_lowercase() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => false,
    };

    writeln!(process().stdout())?;

    Ok(r)
}

pub enum Confirm {
    Yes,
    No,
    Advanced,
}

pub fn confirm_advanced() -> Result<Confirm> {
    writeln!(process().stdout())?;
    writeln!(process().stdout(), "1) Proceed with installation (default)")?;
    writeln!(process().stdout(), "2) Customize installation")?;
    writeln!(process().stdout(), "3) Cancel installation")?;
    write!(process().stdout(), ">")?;

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input {
        "1" | "" => Confirm::Yes,
        "2" => Confirm::Advanced,
        _ => Confirm::No,
    };

    writeln!(process().stdout())?;

    Ok(r)
}

pub fn question_str(question: &str, default: &str) -> Result<String> {
    writeln!(process().stdout(), "{} [{}]", question, default)?;
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    writeln!(process().stdout())?;

    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

pub fn question_bool(question: &str, default: bool) -> Result<bool> {
    let default_text = if default { "(Y/n)" } else { "(y/N)" };
    writeln!(process().stdout(), "{} {}", question, default_text)?;

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    writeln!(process().stdout())?;

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

pub fn read_line() -> Result<String> {
    let stdin = process().stdin();
    let stdin = stdin.lock();
    let mut lines = stdin.lines();
    let lines = lines.next().transpose()?;
    match lines {
        None => Err(anyhow!("no lines found from stdin")),
        Some(v) => Ok(v),
    }
    .context("unable to read from stdin for confirmation")
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
                NotificationLevel::Debug => {
                    debug!("{}", n);
                }
            }
        }
    }
}

pub fn set_globals(verbose: bool, quiet: bool) -> Result<Cfg> {
    use std::cell::RefCell;

    use super::download_tracker::DownloadTracker;

    let download_tracker = RefCell::new(DownloadTracker::new().with_display_progress(!quiet));
    let console_notifier = RefCell::new(NotifyOnConsole {
        verbose,
        ..Default::default()
    });

    Cfg::from_env(Arc::new(move |n: Notification<'_>| {
        if download_tracker.borrow_mut().handle_notification(&n) {
            return;
        }
        console_notifier.borrow_mut().handle(n);
    }))
}

pub fn show_channel_update(cfg: &Cfg, name: &str, updated: Result<UpdateStatus>) -> Result<()> {
    show_channel_updates(cfg, vec![(name.to_string(), updated)])
}

fn show_channel_updates(cfg: &Cfg, toolchains: Vec<(String, Result<UpdateStatus>)>) -> Result<()> {
    let data = toolchains.into_iter().map(|(name, result)| {
        let toolchain = cfg.get_toolchain(&name, false)?;
        let mut version: String = toolchain.rustc_version();

        let banner;
        let color;
        let mut previous_version: Option<String> = None;
        match result {
            Ok(UpdateStatus::Installed) => {
                banner = "installed";
                color = Some(term2::color::GREEN);
            }
            Ok(UpdateStatus::Updated(v)) => {
                if name == "rustup" {
                    previous_version = Some(env!("CARGO_PKG_VERSION").into());
                    version = v;
                } else {
                    previous_version = Some(v);
                }
                banner = "updated";
                color = Some(term2::color::GREEN);
            }
            Ok(UpdateStatus::Unchanged) => {
                if name == "rustup" {
                    version = env!("CARGO_PKG_VERSION").into();
                }
                banner = "unchanged";
                color = None;
            }
            Err(_) => {
                banner = "update failed";
                color = Some(term2::color::RED);
            }
        }

        let width = name.len() + 1 + banner.len();

        Ok((name, banner, width, color, version, previous_version))
    });

    let mut t = term2::stdout();

    let data: Vec<_> = data.collect::<Result<_>>()?;
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
) -> Result<utils::ExitCode> {
    let toolchains = cfg.update_all_channels(force_update)?;

    if toolchains.is_empty() {
        info!("no updatable toolchains installed");
    }

    let show_channel_updates = || {
        if !toolchains.is_empty() {
            writeln!(process().stdout())?;

            show_channel_updates(cfg, toolchains)?;
        }
        Ok(utils::ExitCode(0))
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
        match process().var("SNAP") {
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
            Err(e) => return Err(e).context("Could not interrogate SNAP environment variable"),
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

pub fn self_update<F>(before_restart: F) -> Result<utils::ExitCode>
where
    F: FnOnce() -> Result<utils::ExitCode>,
{
    match self_update_permitted(false)? {
        SelfUpdatePermission::HardFail => {
            err!("Unable to self-update.  STOP");
            return Ok(utils::ExitCode(1));
        }
        SelfUpdatePermission::Skip => return Ok(utils::ExitCode(0)),
        SelfUpdatePermission::Permit => {}
    }

    let setup_path = self_update::prepare_update()?;

    before_restart()?;

    if let Some(ref setup_path) = setup_path {
        return self_update::run_update(setup_path);
    } else {
        // Try again in case we emitted "tool `{}` is already installed" last time.
        self_update::install_proxies()?;
    }

    Ok(utils::ExitCode(0))
}

pub fn list_targets(toolchain: &Toolchain<'_>) -> Result<utils::ExitCode> {
    let mut t = term2::stdout();
    let distributable = DistributableToolchain::new_for_components(&toolchain)?;
    let components = distributable.list_components()?;
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

    Ok(utils::ExitCode(0))
}

pub fn list_installed_targets(toolchain: &Toolchain<'_>) -> Result<utils::ExitCode> {
    let mut t = term2::stdout();
    let distributable = DistributableToolchain::new_for_components(&toolchain)?;
    let components = distributable.list_components()?;
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
    Ok(utils::ExitCode(0))
}

pub fn list_components(toolchain: &Toolchain<'_>) -> Result<utils::ExitCode> {
    let mut t = term2::stdout();
    let distributable = DistributableToolchain::new_for_components(&toolchain)?;
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

    Ok(utils::ExitCode(0))
}

pub fn list_installed_components(toolchain: &Toolchain<'_>) -> Result<utils::ExitCode> {
    let mut t = term2::stdout();
    let distributable = DistributableToolchain::new_for_components(&toolchain)?;
    let components = distributable.list_components()?;
    for component in components {
        if component.installed {
            writeln!(t, "{}", component.name)?;
        }
    }
    Ok(utils::ExitCode(0))
}

fn print_toolchain_path(
    cfg: &Cfg,
    toolchain: &str,
    if_default: &str,
    if_override: &str,
    verbose: bool,
) -> Result<()> {
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
    writeln!(
        process().stdout(),
        "{}{}{}{}",
        &toolchain,
        if_default,
        if_override,
        toolchain_path
    )?;
    Ok(())
}

pub fn list_toolchains(cfg: &Cfg, verbose: bool) -> Result<utils::ExitCode> {
    let toolchains = cfg.list_toolchains()?;
    if toolchains.is_empty() {
        writeln!(process().stdout(), "no installed toolchains")?;
    } else {
        let def_toolchain_name = if let Ok(Some(def_toolchain)) = cfg.find_default() {
            def_toolchain.name().to_string()
        } else {
            String::new()
        };
        let cwd = utils::current_dir()?;
        let ovr_toolchain_name = if let Ok(Some((toolchain, _reason))) = cfg.find_override(&cwd) {
            toolchain.name().to_string()
        } else {
            String::new()
        };
        for toolchain in toolchains {
            let if_default = if def_toolchain_name == *toolchain {
                " (default)"
            } else {
                ""
            };
            let if_override = if ovr_toolchain_name == *toolchain {
                " (override)"
            } else {
                ""
            };

            print_toolchain_path(cfg, &toolchain, if_default, if_override, verbose)
                .context("Failed to list toolchains' directories")?;
        }
    }
    Ok(utils::ExitCode(0))
}

pub fn list_overrides(cfg: &Cfg) -> Result<utils::ExitCode> {
    let overrides = cfg.settings_file.with(|s| Ok(s.overrides.clone()))?;

    if overrides.is_empty() {
        writeln!(process().stdout(), "no overrides")?;
    } else {
        let mut any_not_exist = false;
        for (k, v) in overrides {
            let dir_exists = Path::new(&k).is_dir();
            if !dir_exists {
                any_not_exist = true;
            }
            writeln!(
                process().stdout(),
                "{:<40}\t{:<20}",
                utils::format_path_for_display(&k)
                    + if dir_exists { "" } else { " (not a directory)" },
                v
            )?
        }
        if any_not_exist {
            writeln!(process().stdout())?;
            info!(
                "you may remove overrides for non-existent directories with
`rustup override unset --nonexistent`"
            );
        }
    }
    Ok(utils::ExitCode(0))
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

pub fn dump_testament() -> Result<utils::ExitCode> {
    use git_testament::GitModification::*;
    writeln!(
        process().stdout(),
        "Rustup version renders as: {}",
        version()
    )?;
    writeln!(
        process().stdout(),
        "Current crate version: {}",
        env!("CARGO_PKG_VERSION")
    )?;
    if TESTAMENT.branch_name.is_some() {
        writeln!(
            process().stdout(),
            "Built from branch: {}",
            TESTAMENT.branch_name.unwrap()
        )?;
    } else {
        writeln!(process().stdout(), "Branch information missing")?;
    }
    writeln!(process().stdout(), "Commit info: {}", TESTAMENT.commit)?;
    if TESTAMENT.modifications.is_empty() {
        writeln!(process().stdout(), "Working tree is clean")?;
    } else {
        for fmod in TESTAMENT.modifications {
            match fmod {
                Added(f) => writeln!(process().stdout(), "Added: {}", String::from_utf8_lossy(f))?,
                Removed(f) => writeln!(
                    process().stdout(),
                    "Removed: {}",
                    String::from_utf8_lossy(f)
                )?,
                Modified(f) => writeln!(
                    process().stdout(),
                    "Modified: {}",
                    String::from_utf8_lossy(f)
                )?,
                Untracked(f) => writeln!(
                    process().stdout(),
                    "Untracked: {}",
                    String::from_utf8_lossy(f)
                )?,
            }
        }
    }
    Ok(utils::ExitCode(0))
}

fn show_backtrace() -> bool {
    if let Ok(true) = process().var("RUSTUP_NO_BACKTRACE").map(|s| s == "1") {
        return false;
    }

    if let Ok(true) = process().var("RUST_BACKTRACE").map(|s| s == "1") {
        return true;
    }

    for arg in process().args() {
        if arg == "-v" || arg == "--verbose" {
            return true;
        }
    }

    false
}

pub fn report_error(e: &anyhow::Error) {
    // NB: This shows one error: even for multiple causes and backtraces etc,
    // rather than one per cause, and one for the backtrace. This seems like a
    // reasonable tradeoff, but if we want to do differently, this is the code
    // hunk to revisit, that and a similar build.rs auto-detect glue as anyhow
    // has to detect when backtrace is available.
    if show_backtrace() {
        err!("{:?}", e);
    } else {
        err!("{:#}", e);
    }
}

pub fn ignorable_error(error: &'static str, no_prompt: bool) -> Result<()> {
    let error = anyhow!(error);
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
