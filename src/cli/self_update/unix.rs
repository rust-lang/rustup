use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use tracing::{error, warn};

use super::{
    shell,
    stage::{self, PreparedUpdater, SelfUpdateLock},
};
use crate::{process::Process, utils};

// If the user is trying to install with sudo, on some systems this will
// result in writing root-owned files to the user's home directory, because
// sudo is configured not to change $HOME. Don't let that bogosity happen.
pub(crate) fn anti_sudo_check(
    no_prompt: bool,
    process: &Process,
) -> anyhow::Result<utils::ExitCode> {
    pub(crate) fn home_mismatch(process: &Process) -> (bool, PathBuf, PathBuf) {
        let fallback = || (false, PathBuf::new(), PathBuf::new());
        // test runner should set this, nothing else
        if process
            .var_os("RUSTUP_INIT_SKIP_SUDO_CHECK")
            .is_some_and(|s| s == "yes")
        {
            return fallback();
        }

        match (utils::home_dir_from_passwd(), process.var_os("HOME")) {
            (Some(pw), Some(eh)) if eh != pw => return (true, PathBuf::from(eh), pw),
            (None, _) => warn!("getpwuid_r: couldn't get user data"),
            _ => {}
        }
        fallback()
    }

    match home_mismatch(process) {
        (false, _, _) => {}
        (true, env_home, euid_home) => {
            error!("$HOME differs from euid-obtained home directory: you may be using sudo");
            error!("$HOME directory: {}", env_home.display());
            error!("euid-obtained home directory: {}", euid_home.display());
            if !no_prompt {
                error!("if this is what you want, restart the installation with `-y'");
                return Ok(utils::ExitCode(1));
            }
        }
    }

    Ok(utils::ExitCode(0))
}

pub(crate) fn remove_from_path(process: &Process) -> anyhow::Result<()> {
    let cargo_home = process.rustup_env_home()?;
    let home_dir = process.home_dir();
    for sh in shell::get_available_shells(process) {
        let commands = [
            sh.source_string(&cargo_home.display()),
            sh.legacy_source_string(&cargo_home, home_dir.as_deref())?,
        ];
        // Check more files for cleanup than normally are updated.
        for source_cmd in commands {
            remove_source_command(&source_cmd, &sh.rc_candidates(process))?;
        }
    }

    remove_legacy_paths(process, &cargo_home, home_dir.as_deref())
}

pub(crate) fn add_to_path(process: &Process) -> anyhow::Result<()> {
    let env_home = process.rustup_env_home()?;
    let cargo_home = process.cargo_home()?;
    let home_dir = process.home_dir();
    for sh in shell::get_available_shells(process) {
        let source_cmd = sh.source_string(&env_home.display());
        let legacy_cmd = sh.legacy_source_string(&env_home, home_dir.as_deref())?;
        let source_cmd_with_newline = format!("\n{source_cmd}");

        for rc in sh.rcs(process) {
            let cmd_to_write = match utils::read_file("rcfile", &rc) {
                Ok(contents)
                    if contents
                        .lines()
                        .any(|line| line == source_cmd || line == legacy_cmd) =>
                {
                    continue;
                }
                Ok(contents) if !contents.ends_with('\n') => &source_cmd_with_newline,
                _ => &source_cmd,
            };

            let rc_dir = rc.parent().with_context(|| {
                format!(
                    "parent directory doesn't exist for rcfile path: `{}`",
                    rc.display()
                )
            })?;
            utils::ensure_dir_exists("rcfile dir", rc_dir)?;
            utils::append_file("rcfile", &rc, cmd_to_write)
                .with_context(|| format!("could not amend shell profile: '{}'", rc.display()))?;
        }
    }

    remove_legacy_paths(process, &cargo_home, home_dir.as_deref())?;

    Ok(())
}

pub(crate) fn write_env_files(process: &Process) -> anyhow::Result<()> {
    let env_home = process.rustup_env_home()?;
    let bin_home = process.rustup_bin_home()?;
    let mut written = vec![];

    for sh in shell::get_available_shells(process) {
        let script = sh.env_script();
        // Only write each possible script once.
        if !written.contains(&script) {
            sh.write_script(&script, &env_home, &bin_home)?;
            written.push(script);
        }
    }

    Ok(())
}

/// Tell the updater to replace the rustup bins, then wait for it to finish.
pub(super) fn run_update(
    prepared_updater: PreparedUpdater,
    _process: &Process,
) -> anyhow::Result<utils::ExitCode> {
    let setup_path = prepared_updater.to_path_buf();
    let status = prepared_updater
        .spawn_replacer()?
        .wait()
        .with_context(|| format!("unable to wait for updater ({})", setup_path.display()))?;

    if !status.success() {
        bail!("self-updated failed to replace rustup executable");
    }

    Ok(utils::ExitCode(0))
}

/// This function is the final step of a self-upgrade. It replaces Rustup in
/// the Rustup bin home and updates the proxy links.
pub(crate) fn self_replace(process: &Process) -> anyhow::Result<utils::ExitCode> {
    let self_update_lock = SelfUpdateLock::lock(process)?;
    #[cfg(feature = "test")]
    process.checkpoint(super::CHECKPOINT_SELF_REPLACE_READY);
    let result = (|| {
        let bin_home = process.rustup_bin_home()?;
        self_update_lock.install_bins(&bin_home, super::force_hard_links(process))
    })();
    stage::mark_result(result.is_ok(), process);
    result?;

    Ok(utils::ExitCode(0))
}

/// Removes the first exact line matching `command` followed by a newline from each existing rcfile.
fn remove_source_command(command: &str, rcfiles: &[PathBuf]) -> anyhow::Result<()> {
    let command_bytes = format!("{command}\n").into_bytes();
    for rc in rcfiles.iter().filter(|rc| rc.is_file()) {
        let file = utils::read_file("rcfile", rc)?;
        let file_bytes = file.into_bytes();
        // FIXME: This is whitespace sensitive where it should not be.
        if let Some(idx) = find_exact_line(&file_bytes, &command_bytes) {
            // Here we rewrite the file without the offending line.
            let mut new_bytes = file_bytes[..idx].to_vec();
            new_bytes.extend(&file_bytes[idx + command_bytes.len()..]);
            let new_file = String::from_utf8(new_bytes).unwrap();
            utils::write_file("rcfile", rc, &new_file)?;
        }
    }
    Ok(())
}

fn find_exact_line(file: &[u8], line: &[u8]) -> Option<usize> {
    // The trailing newline enforces the end boundary; check the start boundary here.
    assert!(line.ends_with(b"\n"));
    file.windows(line.len())
        .enumerate()
        .find_map(|(idx, candidate)| {
            (candidate == line && (idx == 0 || file[idx - 1] == b'\n')).then_some(idx)
        })
}

fn remove_legacy_paths(
    process: &Process,
    cargo_home: &Path,
    home_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let cargo_home = match home_dir {
        Some(home) if cargo_home == home.join(".cargo") => "$HOME/.cargo",
        _ => cargo_home.to_str().context("Non-Unicode path!")?,
    };
    let rcfiles = shell::legacy_paths(process, home_dir).collect::<Vec<_>>();
    // Before the work to support more kinds of shells, which was released in
    // version 1.23.0 of Rustup, we always inserted this line instead, which is
    // now considered legacy
    remove_source_command(&format!("export PATH=\"{cargo_home}/bin:$PATH\""), &rcfiles)?;
    // Unfortunately in 1.23, we accidentally used `source` rather than `.`
    // which, while widely supported, isn't actually POSIX, so we also
    // clean that up here.  This issue was filed as #2623.
    remove_source_command(&format!("source \"{cargo_home}/env\""), &rcfiles)?;

    Ok(())
}
