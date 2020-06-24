use std::path::{Path, PathBuf};
use std::process::Command;

use super::super::errors::*;
use super::path_update::PathUpdateMethod;
use super::{canonical_cargo_home, install_bins};
use crate::process;
use crate::utils::utils;
use crate::utils::Notification;

use super::shell;
use super::shell::Shell;

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
    for rcpath in get_remove_path_methods().filter_map(|sh| sh.rcfile()) {
        let file = utils::read_file("rcfile", &rcpath)?;
        let addition = format!("\n{}\n", shell_export_string()?);

        let file_bytes = file.into_bytes();
        let addition_bytes = addition.into_bytes();

        let idx = file_bytes
            .windows(addition_bytes.len())
            .position(|w| w == &*addition_bytes);
        if let Some(i) = idx {
            let mut new_file_bytes = file_bytes[..i].to_vec();
            new_file_bytes.extend(&file_bytes[i + addition_bytes.len()..]);
            let new_file = String::from_utf8(new_file_bytes).unwrap();
            utils::write_file("rcfile", &rcpath, &new_file)?;
        } else {
            // Weird case. rcfile no longer needs to be modified?
        }
    }

    Ok(())
}

pub fn write_env() -> Result<()> {
    let env_file = utils::cargo_home()?.join("env");
    let env_str = format!("{}\n", shell_export_string()?);
    utils::write_file("env", &env_file, &env_str)?;
    Ok(())
}

pub fn shell_export_string() -> Result<String> {
    let path = format!("{}/bin", canonical_cargo_home()?);
    // The path is *prepended* in case there are system-installed
    // rustc's that need to be overridden.
    Ok(format!(r#"export PATH="{}:$PATH""#, path))
}

pub fn do_add_to_path(methods: &[PathUpdateMethod]) -> Result<()> {
    for method in methods {
        if let PathUpdateMethod::RcFile(ref rcpath) = *method {
            let file = if rcpath.exists() {
                utils::read_file("rcfile", rcpath)?
            } else {
                String::new()
            };
            let addition = format!("\n{}", shell_export_string()?);
            if !file.contains(&addition) {
                utils::append_file("rcfile", rcpath, &addition).chain_err(|| {
                    ErrorKind::WritingShellProfile {
                        path: rcpath.to_path_buf(),
                    }
                })?;
            }
        } else {
            unreachable!()
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

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
pub fn get_add_path_methods() -> Vec<PathUpdateMethod> {
    shell::get_available_shells()
        .filter_map(|sh| sh.rcfile())
        .map(PathUpdateMethod::RcFile)
        .collect()
}

/// Decide which rcfiles we're going to update, so we
/// can tell the user before they confirm.
fn get_remove_path_methods() -> impl Iterator<Item = Shell> {
    shell::get_available_shells().filter(|sh| {
        if let (Some(rc), Ok(export)) = (sh.rcfile(), sh.export_string()) {
            rc.is_file()
                && utils::read_file("rcfile", &rc)
                    .unwrap_or_default()
                    .contains(&export)
        } else {
            false
        }
    })
}
