//! Just a dumping ground for cli stuff

use std::cell::RefCell;
use std::fmt::Display;
use std::fs;
use std::io::{BufRead, ErrorKind, Write};
use std::path::Path;
use std::sync::Arc;
use std::{cmp, env};

use anyhow::{anyhow, Context, Result};
use git_testament::{git_testament, render_testament};
use lazy_static::lazy_static;

use super::self_update;
use crate::cli::download_tracker::DownloadTracker;
use crate::currentprocess::{
    argsource::ArgSource,
    filesource::{StdinSource, StdoutSource},
    terminalsource,
    varsource::VarSource,
};
use crate::utils::notifications as util_notifications;
use crate::utils::notify::NotificationLevel;
use crate::utils::utils;
use crate::{dist::dist::ToolchainDesc, install::UpdateStatus};
use crate::{
    dist::notifications as dist_notifications, toolchain::distributable::DistributableToolchain,
};
use crate::{process, toolchain::toolchain::Toolchain};
use crate::{Cfg, Notification};

pub(crate) const WARN_COMPLETE_PROFILE: &str = "downloading with complete profile isn't recommended unless you are a developer of the rust language";

pub(crate) fn confirm(question: &str, default: bool) -> Result<bool> {
    write!(process().stdout().lock(), "{question} ")?;
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input.to_lowercase() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => false,
    };

    writeln!(process().stdout().lock())?;

    Ok(r)
}

pub(crate) enum Confirm {
    Yes,
    No,
    Advanced,
}

pub(crate) fn confirm_advanced() -> Result<Confirm> {
    writeln!(process().stdout().lock())?;
    writeln!(
        process().stdout().lock(),
        "1) Proceed with installation (default)"
    )?;
    writeln!(process().stdout().lock(), "2) Customize installation")?;
    writeln!(process().stdout().lock(), "3) Cancel installation")?;
    write!(process().stdout().lock(), ">")?;

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    let r = match &*input {
        "1" | "" => Confirm::Yes,
        "2" => Confirm::Advanced,
        _ => Confirm::No,
    };

    writeln!(process().stdout().lock())?;

    Ok(r)
}

pub(crate) fn question_str(question: &str, default: &str) -> Result<String> {
    writeln!(process().stdout().lock(), "{question} [{default}]")?;
    let _ = std::io::stdout().flush();
    let input = read_line()?;

    writeln!(process().stdout().lock())?;

    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input)
    }
}

pub(crate) fn question_bool(question: &str, default: bool) -> Result<bool> {
    let default_text = if default { "(Y/n)" } else { "(y/N)" };
    writeln!(process().stdout().lock(), "{question} {default_text}")?;

    let _ = std::io::stdout().flush();
    let input = read_line()?;

    writeln!(process().stdout().lock())?;

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

pub(crate) fn read_line() -> Result<String> {
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
        for n in format!("{n}").lines() {
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

#[cfg_attr(feature = "otel", tracing::instrument)]
pub(crate) fn set_globals(verbose: bool, quiet: bool) -> Result<Cfg> {
    let download_tracker = RefCell::new(DownloadTracker::with_display_progress(
        DownloadTracker::new(),
        !quiet,
    ));
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

pub(crate) fn show_channel_update(
    cfg: &Cfg,
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
    cfg: &Cfg,
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

    let mut t = process().stdout().terminal();

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

pub(crate) fn update_all_channels(
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
            writeln!(process().stdout().lock())?;

            let t = toolchains
                .into_iter()
                .map(|(p, s)| (PackageUpdate::Toolchain(p), s))
                .collect();
            show_channel_updates(cfg, t)?;
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
pub(crate) enum SelfUpdatePermission {
    HardFail,
    Skip,
    Permit,
}

pub(crate) fn self_update_permitted(explicit: bool) -> Result<SelfUpdatePermission> {
    if cfg!(windows) {
        Ok(SelfUpdatePermission::Permit)
    } else {
        // Detect if rustup is not meant to self-update
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

pub(crate) fn self_update<F>(before_restart: F) -> Result<utils::ExitCode>
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

pub(crate) fn list_targets(distributable: DistributableToolchain<'_>) -> Result<utils::ExitCode> {
    let mut t = process().stdout().terminal();
    let manifestation = distributable.get_manifestation()?;
    let config = manifestation.read_config()?.unwrap_or_default();
    let manifest = distributable.get_manifest()?;
    let components = manifest.query_components(distributable.desc(), &config)?;
    for component in components {
        if component.component.short_name_in_manifest() == "rust-std" {
            let target = component
                .component
                .target
                .as_ref()
                .expect("rust-std should have a target");
            if component.installed {
                let _ = t.attr(terminalsource::Attr::Bold);
                let _ = writeln!(t.lock(), "{target} (installed)");
                let _ = t.reset();
            } else if component.available {
                let _ = writeln!(t.lock(), "{target}");
            }
        }
    }

    Ok(utils::ExitCode(0))
}

pub(crate) fn list_installed_targets(
    distributable: DistributableToolchain<'_>,
) -> Result<utils::ExitCode> {
    let t = process().stdout();
    let manifestation = distributable.get_manifestation()?;
    let config = manifestation.read_config()?.unwrap_or_default();
    let manifest = distributable.get_manifest()?;
    let components = manifest.query_components(distributable.desc(), &config)?;
    for component in components {
        if component.component.short_name_in_manifest() == "rust-std" {
            let target = component
                .component
                .target
                .as_ref()
                .expect("rust-std should have a target");
            if component.installed {
                writeln!(t.lock(), "{target}")?;
            }
        }
    }
    Ok(utils::ExitCode(0))
}

pub(crate) fn list_components(
    distributable: DistributableToolchain<'_>,
) -> Result<utils::ExitCode> {
    let mut t = process().stdout().terminal();

    let manifestation = distributable.get_manifestation()?;
    let config = manifestation.read_config()?.unwrap_or_default();
    let manifest = distributable.get_manifest()?;
    let components = manifest.query_components(distributable.desc(), &config)?;
    for component in components {
        let name = component.name;
        if component.installed {
            t.attr(terminalsource::Attr::Bold)?;
            writeln!(t.lock(), "{name} (installed)")?;
            t.reset()?;
        } else if component.available {
            writeln!(t.lock(), "{name}")?;
        }
    }

    Ok(utils::ExitCode(0))
}

pub(crate) fn list_installed_components(distributable: DistributableToolchain<'_>) -> Result<()> {
    let t = process().stdout();
    let manifestation = distributable.get_manifestation()?;
    let config = manifestation.read_config()?.unwrap_or_default();
    let manifest = distributable.get_manifest()?;
    let components = manifest.query_components(distributable.desc(), &config)?;

    for component in components {
        if component.installed {
            writeln!(t.lock(), "{}", component.name)?;
        }
    }
    Ok(())
}

fn print_toolchain_path(
    cfg: &Cfg,
    toolchain: &str,
    if_default: &str,
    if_override: &str,
    verbose: bool,
) -> Result<()> {
    let toolchain_path = cfg.toolchains_dir.join(toolchain);
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
        process().stdout().lock(),
        "{}{}{}{}",
        &toolchain,
        if_default,
        if_override,
        toolchain_path
    )?;
    Ok(())
}

pub(crate) fn list_toolchains(cfg: &Cfg, verbose: bool) -> Result<utils::ExitCode> {
    // Work with LocalToolchainName to accomdate path based overrides
    let toolchains = cfg
        .list_toolchains()?
        .iter()
        .map(Into::into)
        .collect::<Vec<_>>();
    if toolchains.is_empty() {
        writeln!(process().stdout().lock(), "no installed toolchains")?;
    } else {
        let def_toolchain_name = cfg.get_default()?.map(|t| (&t).into());
        let cwd = utils::current_dir()?;
        let ovr_toolchain_name = if let Ok(Some((toolchain, _reason))) = cfg.find_override(&cwd) {
            Some(toolchain)
        } else {
            None
        };
        for toolchain in toolchains {
            let if_default = if def_toolchain_name.as_ref() == Some(&toolchain) {
                " (default)"
            } else {
                ""
            };
            let if_override = if ovr_toolchain_name.as_ref() == Some(&toolchain) {
                " (override)"
            } else {
                ""
            };

            print_toolchain_path(
                cfg,
                &toolchain.to_string(),
                if_default,
                if_override,
                verbose,
            )
            .context("Failed to list toolchains' directories")?;
        }
    }
    Ok(utils::ExitCode(0))
}

pub(crate) fn list_overrides(cfg: &Cfg) -> Result<utils::ExitCode> {
    let overrides = cfg.settings_file.with(|s| Ok(s.overrides.clone()))?;

    if overrides.is_empty() {
        writeln!(process().stdout().lock(), "no overrides")?;
    } else {
        let mut any_not_exist = false;
        for (k, v) in overrides {
            let dir_exists = Path::new(&k).is_dir();
            if !dir_exists {
                any_not_exist = true;
            }
            writeln!(
                process().stdout().lock(),
                "{:<40}\t{:<20}",
                utils::format_path_for_display(&k)
                    + if dir_exists { "" } else { " (not a directory)" },
                v
            )?
        }
        if any_not_exist {
            writeln!(process().stdout().lock())?;
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
    lazy_static! {
        // Because we trust our `stable` branch given the careful release
        // process, we mark it trusted here so that our version numbers look
        // right when built from CI before the tag is pushed
        static ref RENDERED: String = render_testament!(TESTAMENT, "stable");
    }
    &RENDERED
}

pub(crate) fn dump_testament() -> Result<utils::ExitCode> {
    use git_testament::GitModification::*;
    writeln!(
        process().stdout().lock(),
        "Rustup version renders as: {}",
        version()
    )?;
    writeln!(
        process().stdout().lock(),
        "Current crate version: {}",
        env!("CARGO_PKG_VERSION")
    )?;
    if TESTAMENT.branch_name.is_some() {
        writeln!(
            process().stdout().lock(),
            "Built from branch: {}",
            TESTAMENT.branch_name.unwrap()
        )?;
    } else {
        writeln!(process().stdout().lock(), "Branch information missing")?;
    }
    writeln!(
        process().stdout().lock(),
        "Commit info: {}",
        TESTAMENT.commit
    )?;
    if TESTAMENT.modifications.is_empty() {
        writeln!(process().stdout().lock(), "Working tree is clean")?;
    } else {
        for fmod in TESTAMENT.modifications {
            match fmod {
                Added(f) => writeln!(
                    process().stdout().lock(),
                    "Added: {}",
                    String::from_utf8_lossy(f)
                )?,
                Removed(f) => writeln!(
                    process().stdout().lock(),
                    "Removed: {}",
                    String::from_utf8_lossy(f)
                )?,
                Modified(f) => writeln!(
                    process().stdout().lock(),
                    "Modified: {}",
                    String::from_utf8_lossy(f)
                )?,
                Untracked(f) => writeln!(
                    process().stdout().lock(),
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

pub(crate) fn ignorable_error(error: &'static str, no_prompt: bool) -> Result<()> {
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
