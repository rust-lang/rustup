//! Just a dumping ground for cli stuff

use std::cell::RefCell;
use std::fmt::Display;
use std::fs;
#[cfg(not(windows))]
use std::io::ErrorKind;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::{cmp, env};

use anyhow::{anyhow, Context, Result};
use git_testament::{git_testament, render_testament};
use tracing::{debug, error, info, trace, warn};

use super::self_update;
use crate::{
    cli::download_tracker::DownloadTracker,
    config::Cfg,
    dist::{
        manifest::ComponentStatus, notifications as dist_notifications, TargetTriple, ToolchainDesc,
    },
    install::UpdateStatus,
    notifications::Notification,
    process::{terminalsource, Process},
    toolchain::{DistributableToolchain, LocalToolchainName, Toolchain, ToolchainName},
    utils::{notifications as util_notifications, notify::NotificationLevel, utils},
};

pub(crate) const WARN_COMPLETE_PROFILE: &str = "downloading with complete profile isn't recommended unless you are a developer of the rust language";

pub(crate) fn confirm(question: &str, default: bool, process: &Process) -> Result<bool> {
    write!(process.stdout().lock(), "{question} ")?;
    let _ = std::io::stdout().flush();
    let input = read_line(process)?;

    let r = match &*input.to_lowercase() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => false,
    };

    writeln!(process.stdout().lock())?;

    Ok(r)
}

pub(crate) enum Confirm {
    Yes,
    No,
    Advanced,
}

pub(crate) fn confirm_advanced(customized_install: bool, process: &Process) -> Result<Confirm> {
    writeln!(process.stdout().lock())?;
    let first_option = match customized_install {
        true => "1) Proceed with selected options (default - just press enter)",
        false => "1) Proceed with standard installation (default - just press enter)",
    };
    writeln!(process.stdout().lock(), "{first_option}")?;
    writeln!(process.stdout().lock(), "2) Customize installation")?;
    writeln!(process.stdout().lock(), "3) Cancel installation")?;
    write!(process.stdout().lock(), ">")?;

    let _ = std::io::stdout().flush();
    let input = read_line(process)?;

    let r = match &*input {
        "1" | "" => Confirm::Yes,
        "2" => Confirm::Advanced,
        _ => Confirm::No,
    };

    writeln!(process.stdout().lock())?;

    Ok(r)
}

pub(crate) fn question_str(question: &str, default: &str, process: &Process) -> Result<String> {
    writeln!(process.stdout().lock(), "{question} [{default}]")?;
    let _ = std::io::stdout().flush();
    let input = read_line(process)?;

    writeln!(process.stdout().lock())?;

    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

pub(crate) fn question_bool(question: &str, default: bool, process: &Process) -> Result<bool> {
    let default_text = if default { "(Y/n)" } else { "(y/N)" };
    writeln!(process.stdout().lock(), "{question} {default_text}")?;

    let _ = std::io::stdout().flush();
    let input = read_line(process)?;

    writeln!(process.stdout().lock())?;

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

pub(crate) fn read_line(process: &Process) -> Result<String> {
    let stdin = process.stdin();
    let stdin = stdin.lock();
    let mut lines = stdin.lines();
    let lines = lines.next().transpose()?;
    match lines {
        None => Err(anyhow!("no lines found from stdin")),
        Some(v) => Ok(v),
    }
    .context("unable to read from stdin for confirmation")
}

pub(super) struct Notifier {
    tracker: Mutex<DownloadTracker>,
    ram_notice_shown: RefCell<bool>,
    verbose: bool,
}

impl Notifier {
    pub(super) fn new(verbose: bool, quiet: bool, process: &Process) -> Self {
        Self {
            tracker: Mutex::new(DownloadTracker::new_with_display_progress(!quiet, process)),
            ram_notice_shown: RefCell::new(false),
            verbose,
        }
    }

    pub(super) fn handle(&self, n: Notification<'_>) {
        if self.tracker.lock().unwrap().handle_notification(&n) {
            return;
        }

        if let Notification::Install(dist_notifications::Notification::Utils(
            util_notifications::Notification::SetDefaultBufferSize(_),
        )) = &n
        {
            if *self.ram_notice_shown.borrow() {
                return;
            } else {
                *self.ram_notice_shown.borrow_mut() = true;
            }
        };
        let level = n.level();
        for n in format!("{n}").lines() {
            match level {
                NotificationLevel::Debug => {
                    if self.verbose {
                        debug!("{}", n);
                    }
                }
                NotificationLevel::Info => {
                    info!("{}", n);
                }
                NotificationLevel::Warn => {
                    warn!("{}", n);
                }
                NotificationLevel::Error => {
                    error!("{}", n);
                }
                NotificationLevel::Trace => {
                    trace!("{}", n);
                }
            }
        }
    }
}

#[tracing::instrument(level = "trace")]
pub(crate) fn set_globals(
    current_dir: PathBuf,
    verbose: bool,
    quiet: bool,
    process: &Process,
) -> Result<Cfg<'_>> {
    let notifier = Notifier::new(verbose, quiet, process);
    Cfg::from_env(current_dir, Arc::new(move |n| notifier.handle(n)), process)
}

pub(crate) fn show_channel_update(
    cfg: &Cfg<'_>,
    name: PackageUpdate,
    updated: Result<UpdateStatus>,
) -> Result<()> {
    show_channel_updates(cfg, vec![(name, updated)])
}

pub(crate) enum PackageUpdate {
    Rustup,
    Toolchain(ToolchainDesc),
}

impl Display for PackageUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageUpdate::Rustup => write!(f, "rustup"),
            PackageUpdate::Toolchain(t) => write!(f, "{t}"),
        }
    }
}

fn show_channel_updates(
    cfg: &Cfg<'_>,
    updates: Vec<(PackageUpdate, Result<UpdateStatus>)>,
) -> Result<()> {
    let data = updates.into_iter().map(|(pkg, result)| {
        let (banner, color) = match &result {
            Ok(UpdateStatus::Installed) => ("installed", Some(terminalsource::Color::Green)),
            Ok(UpdateStatus::Updated(_)) => ("updated", Some(terminalsource::Color::Green)),
            Ok(UpdateStatus::Unchanged) => ("unchanged", None),
            Err(_) => ("update failed", Some(terminalsource::Color::Red)),
        };

        let (previous_version, version) = match &pkg {
            PackageUpdate::Rustup => {
                let previous_version: Option<String> = match result {
                    Ok(UpdateStatus::Installed) | Ok(UpdateStatus::Unchanged) | Err(_) => None,
                    _ => Some(env!("CARGO_PKG_VERSION").into()),
                };
                let version = match result {
                    Err(_) | Ok(UpdateStatus::Installed) | Ok(UpdateStatus::Unchanged) => {
                        env!("CARGO_PKG_VERSION").into()
                    }
                    Ok(UpdateStatus::Updated(v)) => v,
                };
                (previous_version, version)
            }
            PackageUpdate::Toolchain(name) => {
                // this is a bit strange: we don't supply the version we
                // presumably had (for Installed and Unchanged), so we query it
                // again. Perhaps we can do better.
                let version = match Toolchain::new(cfg, name.into()) {
                    Ok(t) => t.rustc_version(),
                    Err(_) => String::from("(toolchain not installed)"),
                };
                let previous_version: Option<String> = match result {
                    Ok(UpdateStatus::Installed) | Ok(UpdateStatus::Unchanged) | Err(_) => None,
                    Ok(UpdateStatus::Updated(v)) => Some(v),
                };
                (previous_version, version)
            }
        };

        let width = pkg.to_string().len() + 1 + banner.len();

        Ok((pkg, banner, width, color, version, previous_version))
    });

    let mut t = cfg.process.stdout().terminal(cfg.process);

    let data: Vec<_> = data.collect::<Result<_>>()?;
    let max_width = data
        .iter()
        .fold(0, |a, &(_, _, width, _, _, _)| cmp::max(a, width));

    for (pkg, banner, width, color, version, previous_version) in data {
        let padding = max_width - width;
        let padding: String = " ".repeat(padding);
        let _ = write!(t.lock(), "  {padding}");
        let _ = t.attr(terminalsource::Attr::Bold);
        if let Some(color) = color {
            let _ = t.fg(color);
        }
        let _ = write!(t.lock(), "{pkg} {banner}");
        let _ = t.reset();
        let _ = write!(t.lock(), " - {version}");
        if let Some(previous_version) = previous_version {
            let _ = write!(t.lock(), " (from {previous_version})");
        }
        let _ = writeln!(t.lock());
    }
    let _ = writeln!(t.lock());

    Ok(())
}

pub(crate) async fn update_all_channels(
    cfg: &Cfg<'_>,
    do_self_update: bool,
    force_update: bool,
) -> Result<utils::ExitCode> {
    let toolchains = cfg.update_all_channels(force_update).await?;
    let has_update_error = toolchains.iter().any(|(_, r)| r.is_err());
    let mut exit_code = utils::ExitCode(if has_update_error { 1 } else { 0 });

    if toolchains.is_empty() {
        info!("no updatable toolchains installed");
    }

    let show_channel_updates = || {
        if !toolchains.is_empty() {
            writeln!(cfg.process.stdout().lock())?;

            let t = toolchains
                .into_iter()
                .map(|(p, s)| (PackageUpdate::Toolchain(p), s))
                .collect();
            show_channel_updates(cfg, t)?;
        }
        Ok(())
    };

    if do_self_update {
        exit_code &= self_update(show_channel_updates, cfg.process).await?;
    } else {
        show_channel_updates()?;
    }
    Ok(exit_code)
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum SelfUpdatePermission {
    HardFail,
    #[cfg(not(windows))]
    Skip,
    Permit,
}

#[cfg(windows)]
pub(crate) fn self_update_permitted(_explicit: bool) -> Result<SelfUpdatePermission> {
    Ok(SelfUpdatePermission::Permit)
}

#[cfg(not(windows))]
pub(crate) fn self_update_permitted(explicit: bool) -> Result<SelfUpdatePermission> {
    // Detect if rustup is not meant to self-update
    let current_exe = env::current_exe()?;
    let current_exe_dir = current_exe.parent().expect("Rustup isn't in a directory‽");
    if let Err(e) = tempfile::Builder::new()
        .prefix("updtest")
        .tempdir_in(current_exe_dir)
    {
        match e.kind() {
            ErrorKind::PermissionDenied => {
                trace!("Skipping self-update because we cannot write to the rustup dir");
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

/// Performs all of a self-update: check policy, download, apply and exit.
pub(crate) async fn self_update<F>(before_restart: F, process: &Process) -> Result<utils::ExitCode>
where
    F: FnOnce() -> Result<()>,
{
    match self_update_permitted(false)? {
        SelfUpdatePermission::HardFail => {
            error!("Unable to self-update.  STOP");
            return Ok(utils::ExitCode(1));
        }
        #[cfg(not(windows))]
        SelfUpdatePermission::Skip => return Ok(utils::ExitCode(0)),
        SelfUpdatePermission::Permit => {}
    }

    let setup_path = self_update::prepare_update(process).await?;

    before_restart()?;

    if let Some(ref setup_path) = setup_path {
        return self_update::run_update(setup_path);
    } else {
        // Try again in case we emitted "tool `{}` is already installed" last time.
        self_update::install_proxies(process)?;
    }

    Ok(utils::ExitCode(0))
}

pub(super) fn list_items(
    distributable: DistributableToolchain<'_>,
    f: impl Fn(&ComponentStatus) -> Option<&str>,
    installed_only: bool,
    quiet: bool,
    process: &Process,
) -> Result<utils::ExitCode> {
    let mut t = process.stdout().terminal(process);
    for component in distributable.components()? {
        let Some(name) = f(&component) else { continue };
        match (component.available, component.installed, installed_only) {
            (false, _, _) | (_, false, true) => continue,
            (true, true, false) if !quiet => {
                t.attr(terminalsource::Attr::Bold)?;
                writeln!(t.lock(), "{name} (installed)")?;
                t.reset()?;
            }
            (true, _, false) | (_, true, true) => {
                writeln!(t.lock(), "{name}")?;
            }
        }
    }

    Ok(utils::ExitCode(0))
}

pub(crate) fn list_toolchains(
    cfg: &Cfg<'_>,
    verbose: bool,
    quiet: bool,
) -> Result<utils::ExitCode> {
    let toolchains = cfg.list_toolchains()?;
    if toolchains.is_empty() {
        writeln!(cfg.process.stdout().lock(), "no installed toolchains")?;
    } else {
        let default_toolchain_name = cfg.get_default()?;
        let active_toolchain_name: Option<ToolchainName> =
            if let Ok(Some((LocalToolchainName::Named(toolchain), _reason))) =
                cfg.find_active_toolchain()
            {
                Some(toolchain)
            } else {
                None
            };

        for toolchain in toolchains {
            let is_default_toolchain = default_toolchain_name.as_ref() == Some(&toolchain);
            let is_active_toolchain = active_toolchain_name.as_ref() == Some(&toolchain);

            print_toolchain(
                cfg,
                &toolchain.to_string(),
                is_default_toolchain,
                is_active_toolchain,
                verbose,
                quiet,
            )
            .context("Failed to list toolchains' directories")?;
        }
    }

    fn print_toolchain(
        cfg: &Cfg<'_>,
        toolchain: &str,
        is_default: bool,
        is_active: bool,
        verbose: bool,
        quiet: bool,
    ) -> Result<()> {
        if quiet {
            writeln!(cfg.process.stdout().lock(), "{toolchain}")?;
            return Ok(());
        }

        let toolchain_path = cfg.toolchains_dir.join(toolchain);
        let toolchain_meta = fs::symlink_metadata(&toolchain_path)?;
        let toolchain_path = if verbose {
            if toolchain_meta.is_dir() {
                format!(" {}", toolchain_path.display())
            } else {
                format!(" {}", fs::read_link(toolchain_path)?.display())
            }
        } else {
            String::new()
        };
        let status_str = match (is_default, is_active) {
            (true, true) => " (active, default)",
            (true, false) => " (default)",
            (false, true) => " (active)",
            (false, false) => "",
        };

        writeln!(
            cfg.process.stdout().lock(),
            "{}{}{}",
            &toolchain,
            status_str,
            toolchain_path
        )?;
        Ok(())
    }

    Ok(utils::ExitCode(0))
}

pub(crate) fn list_overrides(cfg: &Cfg<'_>) -> Result<utils::ExitCode> {
    let overrides = cfg.settings_file.with(|s| Ok(s.overrides.clone()))?;

    if overrides.is_empty() {
        writeln!(cfg.process.stdout().lock(), "no overrides")?;
    } else {
        let mut any_not_exist = false;
        for (k, v) in overrides {
            let dir_exists = Path::new(&k).is_dir();
            if !dir_exists {
                any_not_exist = true;
            }
            writeln!(
                cfg.process.stdout().lock(),
                "{:<40}\t{:<20}",
                utils::format_path_for_display(&k)
                    + if dir_exists { "" } else { " (not a directory)" },
                v
            )?
        }
        if any_not_exist {
            writeln!(cfg.process.stdout().lock())?;
            info!(
                "you may remove overrides for non-existent directories with
`rustup override unset --nonexistent`"
            );
        }
    }
    Ok(utils::ExitCode(0))
}

git_testament!(TESTAMENT);

pub(crate) fn version() -> &'static str {
    // Because we trust our `stable` branch given the careful release
    // process, we mark it trusted here so that our version numbers look
    // right when built from CI before the tag is pushed
    static RENDERED: LazyLock<String> = LazyLock::new(|| render_testament!(TESTAMENT, "stable"));
    &RENDERED
}

pub(crate) fn dump_testament(process: &Process) -> Result<utils::ExitCode> {
    use git_testament::GitModification::*;
    writeln!(
        process.stdout().lock(),
        "Rustup version renders as: {}",
        version()
    )?;
    writeln!(
        process.stdout().lock(),
        "Current crate version: {}",
        env!("CARGO_PKG_VERSION")
    )?;
    if TESTAMENT.branch_name.is_some() {
        writeln!(
            process.stdout().lock(),
            "Built from branch: {}",
            TESTAMENT.branch_name.unwrap()
        )?;
    } else {
        writeln!(process.stdout().lock(), "Branch information missing")?;
    }
    writeln!(process.stdout().lock(), "Commit info: {}", TESTAMENT.commit)?;
    if TESTAMENT.modifications.is_empty() {
        writeln!(process.stdout().lock(), "Working tree is clean")?;
    } else {
        for fmod in TESTAMENT.modifications {
            match fmod {
                Added(f) => writeln!(
                    process.stdout().lock(),
                    "Added: {}",
                    String::from_utf8_lossy(f)
                )?,
                Removed(f) => writeln!(
                    process.stdout().lock(),
                    "Removed: {}",
                    String::from_utf8_lossy(f)
                )?,
                Modified(f) => writeln!(
                    process.stdout().lock(),
                    "Modified: {}",
                    String::from_utf8_lossy(f)
                )?,
                Untracked(f) => writeln!(
                    process.stdout().lock(),
                    "Untracked: {}",
                    String::from_utf8_lossy(f)
                )?,
            }
        }
    }
    Ok(utils::ExitCode(0))
}

fn show_backtrace(process: &Process) -> bool {
    if let Ok(true) = process.var("RUSTUP_NO_BACKTRACE").map(|s| s == "1") {
        return false;
    }

    if let Ok(true) = process.var("RUST_BACKTRACE").map(|s| s == "1") {
        return true;
    }

    for arg in process.args() {
        if arg == "-v" || arg == "--verbose" {
            return true;
        }
    }

    false
}

pub fn report_error(e: &anyhow::Error, process: &Process) {
    // NB: This shows one error: even for multiple causes and backtraces etc,
    // rather than one per cause, and one for the backtrace. This seems like a
    // reasonable tradeoff, but if we want to do differently, this is the code
    // hunk to revisit, that and a similar build.rs auto-detect glue as anyhow
    // has to detect when backtrace is available.
    if show_backtrace(process) {
        error!("{:?}", e);
    } else {
        error!("{:#}", e);
    }
}

pub(crate) fn ignorable_error(
    error: &'static str,
    no_prompt: bool,
    process: &Process,
) -> Result<()> {
    let error = anyhow!(error);
    report_error(&error, process);
    if no_prompt {
        warn!("continuing (because the -y flag is set and the error is ignorable)");
        Ok(())
    } else if confirm("\nContinue? (y/N)", false, process).unwrap_or(false) {
        Ok(())
    } else {
        Err(error)
    }
}

/// Warns if rustup is trying to install a toolchain that might not be
/// able to run on the host system.
pub(crate) fn warn_if_host_is_incompatible(
    toolchain: impl Display,
    host_arch: &TargetTriple,
    target_triple: &TargetTriple,
    force_non_host: bool,
) -> Result<()> {
    if force_non_host || host_arch.can_run(target_triple)? {
        return Ok(());
    }
    error!("DEPRECATED: future versions of rustup will require --force-non-host to install a non-host toolchain.");
    warn!("toolchain '{toolchain}' may not be able to run on this system.");
    warn!("If you meant to build software to target that platform, perhaps try `rustup target add {target_triple}` instead?");
    Ok(())
}

/// Warns if rustup is running under emulation, such as macOS Rosetta
pub(crate) fn warn_if_host_is_emulated(process: &Process) {
    if TargetTriple::is_host_emulated() {
        warn!(
            "Rustup is not running natively. It's running under emulation of {}.",
            TargetTriple::from_host_or_build(process)
        );
        warn!("For best compatibility and performance you should reinstall rustup for your native CPU.");
    }
}
