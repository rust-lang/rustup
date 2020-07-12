use std::path::{Path, PathBuf};
use std::process::Command;

use super::super::errors::*;
use super::install_bins;
use super::shell;
use crate::process;
use crate::utils::utils;
use crate::utils::Notification;

// If the user is trying to install with sudo, on some systems this will
// result in writing root-owned files to the user's home directory, because
// sudo is configured not to change $HOME. Don't let that bogosity happen.
pub fn do_anti_sudo_check(no_prompt: bool) -> Result<utils::ExitCode> {
    pub fn home_mismatch() -> (bool, PathBuf, PathBuf) {
        let fallback = || (false, PathBuf::new(), PathBuf::new());
        // test runner should set this, nothing else
        if process()
            .var_os("RUSTUP_INIT_SKIP_SUDO_CHECK")
            .map_or(false, |s| s == "yes")
        {
            return fallback();
        }

        match (utils::home_dir_from_passwd(), process().var_os("HOME")) {
            (Some(pw), Some(eh)) if eh != pw => return (true, PathBuf::from(eh), pw),
            (None, _) => warn!("getpwuid_r: couldn't get user data"),
            _ => {}
        }
        fallback()
    }

    match home_mismatch() {
        (false, _, _) => {}
        (true, env_home, euid_home) => {
            err!("$HOME differs from euid-obtained home directory: you may be using sudo");
            err!("$HOME directory: {}", env_home.display());
            err!("euid-obtained home directory: {}", euid_home.display());
            if !no_prompt {
                err!("if this is what you want, restart the installation with `-y'");
                return Ok(utils::ExitCode(1));
            }
        }
    }

    Ok(utils::ExitCode(0))
}

pub fn delete_rustup_and_cargo_home() -> Result<()> {
    let cargo_home = utils::cargo_home()?;
    utils::remove_dir("cargo_home", &cargo_home, &|_: Notification<'_>| ())?;

    Ok(())
}

pub fn complete_windows_uninstall() -> Result<utils::ExitCode> {
    panic!("stop doing that")
}

pub fn do_remove_from_path() -> Result<()> {
    for sh in shell::get_available_shells() {
        let source_bytes = format!("\n{}\n", sh.source_string()?).into_bytes();

        // Check more files for cleanup than normally are updated.
        for rc in sh.rcfiles().iter().filter(|rc| rc.is_file()) {
            let file = utils::read_file("rcfile", &rc)?;
            let file_bytes = file.into_bytes();
            // FIXME: This is whitespace sensitive where it should not be.
            if let Some(idx) = file_bytes
                .windows(source_bytes.len())
                .position(|w| w == source_bytes.as_slice())
            {
                // Here we rewrite the file without the offending line.
                let mut new_bytes = file_bytes[..idx].to_vec();
                new_bytes.extend(&file_bytes[idx + source_bytes.len()..]);
                let new_file = String::from_utf8(new_bytes).unwrap();
                utils::write_file("rcfile", &rc, &new_file)?;
            }
        }
    }

    Ok(())
}

pub fn do_add_to_path() -> Result<()> {
    let mut scripts = vec![];

    for sh in shell::get_available_shells() {
        let source_cmd = format!("\n{}", sh.source_string()?);
        for rc in sh.update_rcs() {
            if !rc.is_file() || !utils::read_file("rcfile", &rc)?.contains(&source_cmd) {
                utils::append_file("rcfile", &rc, &source_cmd).chain_err(|| {
                    ErrorKind::WritingShellProfile {
                        path: rc.to_path_buf(),
                    }
                })?;
                let script = sh.env_script();
                // Only write scripts once.
                if !scripts.contains(&script) {
                    script.write()?;
                    scripts.push(script);
                }
            }
        }
    }

    Ok(())
}

/// Tell the upgrader to replace the rustup bins, then delete
/// itself. Like with uninstallation, on Windows we're going to
/// have to jump through hoops to make everything work right.
///
/// On windows we're not going to wait for it to finish before exiting
/// successfully, so it should not do much, and it should try
/// really hard to succeed, because at this point the upgrade is
/// considered successful.
pub fn run_update(setup_path: &Path) -> Result<utils::ExitCode> {
    let status = Command::new(setup_path)
        .arg("--self-replace")
        .status()
        .chain_err(|| "unable to run updater")?;

    if !status.success() {
        return Err("self-updated failed to replace rustup executable".into());
    }

    Ok(utils::ExitCode(0))
}

/// This function is as the final step of a self-upgrade. It replaces
/// `CARGO_HOME`/bin/rustup with the running exe, and updates the the
/// links to it. On windows this will run *after* the original
/// rustup process exits.
pub fn self_replace() -> Result<utils::ExitCode> {
    install_bins()?;

    Ok(utils::ExitCode(0))
}
