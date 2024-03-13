use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::install_bins;
use super::shell;
use crate::currentprocess::varsource::VarSource;
use crate::process;
use crate::utils::utils;
use crate::utils::Notification;

// If the user is trying to install with sudo, on some systems this will
// result in writing root-owned files to the user's home directory, because
// sudo is configured not to change $HOME. Don't let that bogosity happen.
pub(crate) fn do_anti_sudo_check(no_prompt: bool) -> Result<utils::ExitCode> {
    pub(crate) fn home_mismatch() -> (bool, PathBuf, PathBuf) {
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

pub(crate) fn delete_rustup_and_cargo_home() -> Result<()> {
    let cargo_home = utils::cargo_home()?;
    utils::remove_dir("cargo_home", &cargo_home, &|_: Notification<'_>| ())
}

pub(crate) fn do_remove_from_path() -> Result<()> {
    for sh in shell::get_available_shells() {
        let source_bytes = format!("{}\n", sh.source_string()?).into_bytes();

        // Check more files for cleanup than normally are updated.
        for rc in sh.rcfiles().iter().filter(|rc| rc.is_file()) {
            let file = utils::read_file("rcfile", rc)?;
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
                utils::write_file("rcfile", rc, &new_file)?;
            }
        }
    }

    remove_legacy_paths()?;

    Ok(())
}

pub(crate) fn do_add_to_path() -> Result<()> {
    for sh in shell::get_available_shells() {
        let source_cmd = sh.source_string()?;
        let source_cmd_with_newline = format!("\n{}", &source_cmd);

        for rc in sh.update_rcs() {
            let cmd_to_write = match utils::read_file("rcfile", &rc) {
                Ok(contents) if contents.contains(&source_cmd) => continue,
                Ok(contents) if !contents.ends_with('\n') => &source_cmd_with_newline,
                _ => &source_cmd,
            };

            let rc_dir = rc.parent().with_context(|| {
                format!(
                    "parent directory doesn't exist for rcfile path: `{}`",
                    rc.display()
                )
            })?;
            utils::ensure_dir_exists("rcfile dir", rc_dir, &|_: Notification<'_>| ())?;
            utils::append_file("rcfile", &rc, cmd_to_write)
                .with_context(|| format!("could not amend shell profile: '{}'", rc.display()))?;
        }
    }

    remove_legacy_paths()?;

    Ok(())
}

pub(crate) fn do_write_env_files() -> Result<()> {
    let mut written = vec![];

    for sh in shell::get_available_shells() {
        let script = sh.env_script();
        // Only write each possible script once.
        if !written.contains(&script) {
            script.write()?;
            written.push(script);
        }
    }

    Ok(())
}

pub(crate) fn do_add_to_programs() -> Result<()> {
    Ok(())
}

pub(crate) fn do_remove_from_programs() -> Result<()> {
    Ok(())
}

/// Tell the upgrader to replace the rustup bins, then delete
/// itself.
pub(crate) fn run_update(setup_path: &Path) -> Result<utils::ExitCode> {
    let status = Command::new(setup_path)
        .arg("--self-replace")
        .status()
        .context("unable to run updater")?;

    if !status.success() {
        bail!("self-updated failed to replace rustup executable");
    }

    Ok(utils::ExitCode(0))
}

/// This function is as the final step of a self-upgrade. It replaces
/// `CARGO_HOME`/bin/rustup with the running exe, and updates the
/// links to it.
pub(crate) fn self_replace() -> Result<utils::ExitCode> {
    install_bins()?;

    Ok(utils::ExitCode(0))
}

fn remove_legacy_source_command(source_cmd: String) -> Result<()> {
    let cmd_bytes = source_cmd.into_bytes();
    for rc in shell::legacy_paths().filter(|rc| rc.is_file()) {
        let file = utils::read_file("rcfile", &rc)?;
        let file_bytes = file.into_bytes();
        // FIXME: This is whitespace sensitive where it should not be.
        if let Some(idx) = file_bytes
            .windows(cmd_bytes.len())
            .position(|w| w == cmd_bytes.as_slice())
        {
            // Here we rewrite the file without the offending line.
            let mut new_bytes = file_bytes[..idx].to_vec();
            new_bytes.extend(&file_bytes[idx + cmd_bytes.len()..]);
            let new_file = String::from_utf8(new_bytes).unwrap();
            utils::write_file("rcfile", &rc, &new_file)?;
        }
    }
    Ok(())
}

fn remove_legacy_paths() -> Result<()> {
    // Before the work to support more kinds of shells, which was released in
    // version 1.23.0 of Rustup, we always inserted this line instead, which is
    // now considered legacy
    remove_legacy_source_command(format!(
        "export PATH=\"{}/bin:$PATH\"\n",
        shell::cargo_home_str()?
    ))?;
    // Unfortunately in 1.23, we accidentally used `source` rather than `.`
    // which, while widely supported, isn't actually POSIX, so we also
    // clean that up here.  This issue was filed as #2623.
    remove_legacy_source_command(format!("source \"{}/env\"\n", shell::cargo_home_str()?))?;

    Ok(())
}
