//! Paths and Unix shells
//!
//! MacOS, Linux, FreeBSD, and many other OS model their design on Unix,
//! so handling them is relatively consistent. But only relatively.
//! POSIX postdates Unix by 20 years, and each "Unix-like" shell develops
//! unique quirks over time.
//!
//!
//! Windowing Managers, Desktop Environments, GUI Terminals, and PATHs
//!
//! Duplicating paths in PATH can cause performance issues when the OS searches
//! the same place multiple times. Traditionally, Unix configurations have
//! resolved this by setting up PATHs in the shell's login profile.
//!
//! This has its own issues. Login profiles are only intended to run once, but
//! changing the PATH is common enough that people may run it twice. Desktop
//! environments often choose to NOT start login shells in GUI terminals. Thus,
//! a trend has emerged to place PATH updates in other run-commands (rc) files,
//! leaving Rustup with few assumptions to build on for fulfilling its promise
//! to set up PATH appropriately.
//!
//! Rustup addresses this by:
//! 1) using a shell script that updates PATH if the path is not in PATH
//! 2) sourcing this script (`. /path/to/script`) in any appropriate rc file

use std::borrow::Cow;
use std::path::PathBuf;

use anyhow::{bail, Result};

use super::utils;
use crate::process::Process;

pub(crate) type Shell = Box<dyn UnixShell>;

#[derive(Debug, PartialEq)]
pub(crate) struct ShellScript {
    content: &'static str,
    name: &'static str,
}

impl ShellScript {
    pub(crate) fn write(&self, process: &Process) -> Result<()> {
        let home = process.cargo_home()?;
        let cargo_bin = format!("{}/bin", cargo_home_str(process)?);
        let env_name = home.join(self.name);
        let env_file = self.content.replace("{cargo_bin}", &cargo_bin);
        utils::write_file(self.name, &env_name, &env_file)?;
        Ok(())
    }
}

// TODO: Update into a bytestring.
pub(crate) fn cargo_home_str(process: &Process) -> Result<Cow<'static, str>> {
    let path = process.cargo_home()?;

    let default_cargo_home = process
        .home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cargo");
    Ok(if default_cargo_home == path {
        "$HOME/.cargo".into()
    } else {
        match path.to_str() {
            Some(p) => p.to_owned().into(),
            None => bail!("Non-Unicode path!"),
        }
    })
}

// TODO: Tcsh (BSD)
// TODO?: Make a decision on Ion Shell, Power Shell, Nushell
// Cross-platform non-POSIX shells have not been assessed for integration yet
fn enumerate_shells() -> Vec<Shell> {
    vec![
        Box::new(Posix),
        Box::new(Bash),
        Box::new(Zsh),
        Box::new(Fish),
    ]
}

pub(crate) fn get_available_shells(process: &Process) -> impl Iterator<Item = Shell> + '_ {
    enumerate_shells()
        .into_iter()
        .filter(|sh| sh.does_exist(process))
}

pub(crate) trait UnixShell {
    // Detects if a shell "exists". Users have multiple shells, so an "eager"
    // heuristic should be used, assuming shells exist if any traces do.
    fn does_exist(&self, process: &Process) -> bool;

    // Gives all rcfiles of a given shell that Rustup is concerned with.
    // Used primarily in checking rcfiles for cleanup.
    fn rcfiles(&self, process: &Process) -> Vec<PathBuf>;

    // Gives rcs that should be written to.
    fn update_rcs(&self, process: &Process) -> Vec<PathBuf>;

    // Writes the relevant env file.
    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env",
            content: include_str!("env.sh"),
        }
    }

    fn source_string(&self, process: &Process) -> Result<String> {
        Ok(format!(r#". "{}/env""#, cargo_home_str(process)?))
    }
}

struct Posix;
impl UnixShell for Posix {
    fn does_exist(&self, _: &Process) -> bool {
        true
    }

    fn rcfiles(&self, process: &Process) -> Vec<PathBuf> {
        match process.home_dir() {
            Some(dir) => vec![dir.join(".profile")],
            _ => vec![],
        }
    }

    fn update_rcs(&self, process: &Process) -> Vec<PathBuf> {
        // Write to .profile even if it doesn't exist. It's the only rc in the
        // POSIX spec so it should always be set up.
        self.rcfiles(process)
    }
}

struct Bash;

impl UnixShell for Bash {
    fn does_exist(&self, process: &Process) -> bool {
        !self.update_rcs(process).is_empty()
    }

    fn rcfiles(&self, process: &Process) -> Vec<PathBuf> {
        // Bash also may read .profile, however Rustup already includes handling
        // .profile as part of POSIX and always does setup for POSIX shells.
        [".bash_profile", ".bash_login", ".bashrc"]
            .iter()
            .filter_map(|rc| process.home_dir().map(|dir| dir.join(rc)))
            .collect()
    }

    fn update_rcs(&self, process: &Process) -> Vec<PathBuf> {
        self.rcfiles(process)
            .into_iter()
            .filter(|rc| rc.is_file())
            .collect()
    }
}

struct Zsh;

impl Zsh {
    fn zdotdir(process: &Process) -> Result<PathBuf> {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        if matches!(process.var("SHELL"), Ok(sh) if sh.contains("zsh")) {
            match process.var("ZDOTDIR") {
                Ok(dir) if !dir.is_empty() => Ok(PathBuf::from(dir)),
                _ => bail!("Zsh setup failed."),
            }
        } else {
            match std::process::Command::new("zsh")
                .args(["-c", "echo -n $ZDOTDIR"])
                .output()
            {
                Ok(io) if !io.stdout.is_empty() => Ok(PathBuf::from(OsStr::from_bytes(&io.stdout))),
                _ => bail!("Zsh setup failed."),
            }
        }
    }
}

impl UnixShell for Zsh {
    fn does_exist(&self, process: &Process) -> bool {
        // zsh has to either be the shell or be callable for zsh setup.
        matches!(process.var("SHELL"), Ok(sh) if sh.contains("zsh"))
            || utils::find_cmd(&["zsh"], process).is_some()
    }

    fn rcfiles(&self, process: &Process) -> Vec<PathBuf> {
        [Zsh::zdotdir(process).ok(), process.home_dir()]
            .iter()
            .filter_map(|dir| dir.as_ref().map(|p| p.join(".zshenv")))
            .collect()
    }

    fn update_rcs(&self, process: &Process) -> Vec<PathBuf> {
        // zsh can change $ZDOTDIR both _before_ AND _during_ reading .zshenv,
        // so we: write to $ZDOTDIR/.zshenv if-exists ($ZDOTDIR changes before)
        // OR write to $HOME/.zshenv if it exists (change-during)
        // if neither exist, we create it ourselves, but using the same logic,
        // because we must still respond to whether $ZDOTDIR is set or unset.
        // In any case we only write once.
        self.rcfiles(process)
            .into_iter()
            .filter(|env| env.is_file())
            .chain(self.rcfiles(process))
            .take(1)
            .collect()
    }
}

struct Fish;

impl UnixShell for Fish {
    fn does_exist(&self, process: &Process) -> bool {
        // fish has to either be the shell or be callable for fish setup.
        matches!(process.var("SHELL"), Ok(sh) if sh.contains("fish"))
            || utils::find_cmd(&["fish"], process).is_some()
    }

    // > "$XDG_CONFIG_HOME/fish/conf.d" (or "~/.config/fish/conf.d" if that variable is unset) for the user
    // from <https://github.com/fish-shell/fish-shell/issues/3170#issuecomment-228311857>
    fn rcfiles(&self, process: &Process) -> Vec<PathBuf> {
        let p0 = process.var("XDG_CONFIG_HOME").ok().map(|p| {
            let mut path = PathBuf::from(p);
            path.push("fish/conf.d/rustup.fish");
            path
        });

        let p1 = process.home_dir().map(|mut path| {
            path.push(".config/fish/conf.d/rustup.fish");
            path
        });

        p0.into_iter().chain(p1).collect()
    }

    fn update_rcs(&self, process: &Process) -> Vec<PathBuf> {
        // The first rcfile takes precedence.
        match self.rcfiles(process).into_iter().next() {
            Some(path) => vec![path],
            None => vec![],
        }
    }

    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env.fish",
            content: include_str!("env.fish"),
        }
    }

    fn source_string(&self, process: &Process) -> Result<String> {
        Ok(format!(r#"source "{}/env.fish""#, cargo_home_str(process)?))
    }
}

pub(crate) fn legacy_paths(process: &Process) -> impl Iterator<Item = PathBuf> + '_ {
    let zprofiles = Zsh::zdotdir(process)
        .into_iter()
        .chain(process.home_dir())
        .map(|d| d.join(".zprofile"));
    let profiles = [".bash_profile", ".profile"]
        .iter()
        .filter_map(|rc| process.home_dir().map(|d| d.join(rc)));

    profiles.chain(zprofiles)
}
