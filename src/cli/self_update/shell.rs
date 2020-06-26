//! Paths and Unix shells
//!
//! MacOS, Linux, FreeBSD, and many other OS model their design on Unix,
//! so handling them is relatively consistent. But only relatively.
//! POSIX postdates Unix by 20 years, and each "Unix-like" shell develops
//! unique quirks over time.

// TODO: Nushell, PowerShell
// Cross-platform non-POSIX shells were not assessed for integration yet.

use super::*;
use crate::process;
use std::path::PathBuf;

pub type Shell = Box<dyn UnixShell>;

macro_rules! support_shells {
    ( $($shell:ident,)* ) => {
        fn enumerate_shells() -> Vec<Shell> {
            vec![$( Box::new($shell), )*]
        }
    }
}

support_shells! {
    Posix,
    Bash,
    Zsh,
}

pub fn get_available_shells() -> impl Iterator<Item = Shell> {
    enumerate_shells().into_iter().filter(|sh| sh.does_exist())
}

pub trait UnixShell {
    fn does_exist(&self) -> bool;

    fn rcfile(&self) -> Option<PathBuf>;

    fn export_string(&self) -> Result<String> {
        // The path is *prepended* in case there are system-installed
        // rustc's that need to be overridden.
        Ok(format!(
            r#"export PATH="{}/bin:$PATH""#,
            canonical_cargo_home()?
        ))
    }
}

struct Posix;
impl UnixShell for Posix {
    fn does_exist(&self) -> bool {
        true
    }

    fn rcfile(&self) -> Option<PathBuf> {
        utils::home_dir().map(|dir| dir.join(".profile"))
    }
}

struct Bash;
impl UnixShell for Bash {
    fn does_exist(&self) -> bool {
        self.rcfile().map_or(false, |rc| rc.is_file())
            || matches!(utils::find_cmd(&["bash"]), Some(_))
    }

    fn rcfile(&self) -> Option<PathBuf> {
        // .bashrc is normative, in spite of a few weird Mac versions.
        utils::home_dir().map(|dir| dir.join(".bashrc"))
    }
}

struct Zsh;
impl UnixShell for Zsh {
    fn does_exist(&self) -> bool {
        self.rcfile().map_or(false, |rc| rc.is_file())
            || matches!(utils::find_cmd(&["zsh"]), Some(_))
    }

    fn rcfile(&self) -> Option<PathBuf> {
        let zdotdir = match process().var("ZDOTDIR") {
            Ok(dir) => Some(PathBuf::from(dir)),
            _ => utils::home_dir(),
        };

        // .zshenv is preferred for path mods but not all zshers use it,
        // zsh always loads .zshrc on interactive, unlike bash's weirdness.
        zdotdir.map(|dir| match dir.join(".zshenv") {
            rc if rc.is_file() => rc,
            _ => dir.join(".zshrc"),
        })
    }
}
