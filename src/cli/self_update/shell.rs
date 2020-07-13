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
//! 2) sourcing this script in any known and appropriate rc file

use super::canonical_cargo_home;
use super::*;
use crate::process;
use error_chain::bail;
use std::path::PathBuf;

pub type Shell = Box<dyn UnixShell>;

#[derive(Debug, PartialEq)]
pub struct ShellScript {
    content: &'static str,
    name: &'static str,
}

impl ShellScript {
    pub fn write(&self) -> Result<()> {
        let home = utils::cargo_home()?;
        let cargo_bin = match home.to_str() {
            Some(s) => format!("{}/bin", s),
            None => bail!("Non-Unicode path found."),
        };
        let env_name = home.join(self.name);
        let env_file = self.content.replace("{cargo_bin}", &cargo_bin);
        utils::write_file(self.name, &env_name, &env_file)?;
        Ok(())
    }
}

#[allow(dead_code)] // For some reason.
const POSIX_ENV: &str = include_str!("env");

macro_rules! support_shells {
    ( $($shell:ident,)* ) => {
        fn enumerate_shells() -> Vec<Shell> {
            vec![$( Box::new($shell), )*]
        }
    }
}

// TODO: Tcsh (BSD)
// TODO?: Make a decision on Ion Shell, Power Shell, Nushell
// Cross-platform non-POSIX shells have not been assessed for integration yet
support_shells! {
    Posix,
    Bash,
    Zsh,
}

pub fn get_available_shells() -> impl Iterator<Item = Shell> {
    enumerate_shells().into_iter().filter(|sh| sh.does_exist())
}

pub trait UnixShell {
    // Detects if a shell "exists". Users have multiple shells, so an "eager"
    // heuristic should be used, assuming shells exist if any traces do.
    fn does_exist(&self) -> bool;

    // Gives all rcfiles of a given shell that rustup is concerned with.
    // Used primarily in checking rcfiles for cleanup.
    fn rcfiles(&self) -> Vec<PathBuf>;

    // Gives rcs that should be written to.
    fn update_rcs(&self) -> Vec<PathBuf>;

    // Writes the relevant env file.
    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env",
            content: POSIX_ENV,
        }
    }

    fn source_string(&self) -> Result<String> {
        Ok(format!(r#"source "{}/env""#, canonical_cargo_home()?))
    }
}

struct Posix;
impl UnixShell for Posix {
    fn does_exist(&self) -> bool {
        true
    }

    fn rcfiles(&self) -> Vec<PathBuf> {
        match utils::home_dir() {
            Some(dir) => vec![dir.join(".profile")],
            _ => vec![],
        }
    }

    fn update_rcs(&self) -> Vec<PathBuf> {
        // Write to .profile even if it doesn't exist.
        self.rcfiles()
    }
}

struct Bash;

impl Bash {
    fn profiles() -> impl Iterator<Item = PathBuf> {
        [".bash_profile", ".bash_login", ".profile", ".bashrc"]
            .iter()
            .filter_map(|rc| utils::home_dir().map(|dir| dir.join(rc)))
    }
}

impl UnixShell for Bash {
    fn does_exist(&self) -> bool {
        // Checking to see if we can find any traces of Bash in $SHELL
        // or a file or binary on disk.
        matches!(process().var("SHELL"), Ok(sh) if sh.contains("bash"))
            || self.rcfiles().iter().any(|rc| rc.is_file())
            || matches!(utils::find_cmd(&["bash"]), Some(_))
    }

    fn rcfiles(&self) -> Vec<PathBuf> {
        Bash::profiles().collect()
    }

    fn update_rcs(&self) -> Vec<PathBuf> {
        Bash::profiles().filter(|rc| rc.is_file()).collect()
    }
}

struct Zsh;
impl UnixShell for Zsh {
    fn does_exist(&self) -> bool {
        // Checking to see if we can find any traces of zsh, either in env vars,
        // an rc of concern, or the binary.
        matches!(process().var("SHELL"), Ok(sh) if sh.contains("zsh"))
            || matches!(process().var("ZDOTDIR"), Ok(dir) if dir.len() > 0)
            || self.rcfiles().iter().any(|rc| rc.is_file())
            || matches!(utils::find_cmd(&["zsh"]), Some(_))
    }

    fn rcfiles(&self) -> Vec<PathBuf> {
        // FIXME: if zsh exists but is not in the process tree of the shell
        // on install, $ZDOTDIR may not be loaded and give the wrong result.
        let zdotdir = match process().var("ZDOTDIR") {
            Ok(dir) => Some(PathBuf::from(dir)),
            _ => utils::home_dir(),
        };

        // .zshenv is always sourced
        match zdotdir.map(|dir| dir.join(".zshenv")) {
            Some(zshenv) => vec![zshenv],
            _ => vec![],
        }
    }

    fn update_rcs(&self) -> Vec<PathBuf> {
        // .zshenv is preferred for path mods, always write it.
        self.rcfiles()
    }
}
