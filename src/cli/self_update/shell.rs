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

use std::path::{Path, PathBuf};

use anyhow::bail;
use itertools::Itertools;

use super::utils;
use crate::process::Process;

#[derive(Debug, PartialEq)]
pub(crate) struct ShellScript {
    content: &'static str,
    name: &'static str,
}

// TODO: Update into a bytestring.
fn path_str_with_home(
    home: &str,
    path: &Path,
    home_dir: Option<&Path>,
    default_suffix: &str,
) -> anyhow::Result<String> {
    let default_path = home_dir
        .unwrap_or_else(|| Path::new("."))
        .join(default_suffix);
    Ok(if default_path == path {
        format!("{home}/{default_suffix}")
    } else {
        match path.to_str() {
            Some(p) => p.to_owned(),
            None => bail!("Non-Unicode path!"),
        }
    })
}

/// Builds the shell source lines for the post-install message, showing only
/// shells that are available on the current system. Shells sharing the same
/// env file are grouped onto one line (e.g. sh/bash/zsh all use `env`).
pub(crate) fn build_source_env_lines(
    process: &Process,
    env_dir: &Path,
    home_dir: Option<&Path>,
) -> String {
    let mut groups = Vec::<(_, Vec<_>)>::new();
    for shell in get_available_shells(process) {
        let Ok(src) = shell.source_string(env_dir, home_dir) else {
            continue;
        };
        if let Some(names) = groups
            .iter_mut()
            .find_map(|(s, names)| (*s == src).then_some(names))
        {
            names.push(shell.name());
        } else {
            groups.push((src, vec![shell.name()]));
        }
    }
    let src_width = groups.iter().map(|(src, _)| src.len()).max().unwrap_or(0);
    groups
        .into_iter()
        .map(|(src, names)| format!("{:<src_width$} # For {}\n", src, names.join("/")))
        .collect()
}

// TODO: Tcsh (BSD)
// TODO?: Make a decision on Ion Shell
// Cross-platform non-POSIX shells have not been assessed for integration yet
pub(crate) fn get_available_shells(
    process: &Process,
) -> impl Iterator<Item = &'static dyn UnixShell> + '_ {
    [
        &Posix as &dyn UnixShell,
        &Bash,
        &Zsh,
        &Fish,
        &Nu,
        &Tcsh,
        &Pwsh,
        &Xonsh,
    ]
    .into_iter()
    .filter(move |sh| sh.does_exist(process))
}

pub(crate) trait UnixShell {
    // Detects if a shell "exists". Users have multiple shells, so an "eager"
    // heuristic should be used, assuming shells exist if any traces do.
    fn does_exist(&self, process: &Process) -> bool;

    // Returns the display name of the shell, used in post-install messages.
    fn name(&self) -> &'static str;

    // Gives candidate rcfile paths, which may not exist, in preference order.
    // Used to select installation targets and check all candidates for cleanup.
    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>>;

    // Returns rcfile paths where installation should add the source command.
    // May return multiple paths, including files that do not yet exist.
    // Does not modify the files.
    fn rcs(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        Box::new(self.rc_candidates(process).take(1))
    }

    // Writes the relevant env file.
    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env",
            content: include_str!("env.sh"),
        }
    }

    fn home_var(&self) -> &'static str {
        #[cfg(windows)]
        let home = "%USERPROFILE%";
        #[cfg(not(windows))]
        let home = "$HOME";
        home
    }

    fn env_dir_str(&self, env_dir: &Path, home_dir: Option<&Path>) -> anyhow::Result<String> {
        path_str_with_home(self.home_var(), env_dir, home_dir, ".cargo")
    }

    fn source_string(&self, env_dir: &Path, home_dir: Option<&Path>) -> anyhow::Result<String> {
        Ok(format!(
            r#". "{}/env""#,
            self.env_dir_str(env_dir, home_dir)?
        ))
    }

    fn write_script(
        &self,
        script: &ShellScript,
        env_dir: &Path,
        bin_dir: &Path,
        home_dir: Option<&Path>,
    ) -> anyhow::Result<()> {
        let cargo_bin = path_str_with_home(self.home_var(), bin_dir, home_dir, ".cargo/bin")?;
        let env_name = env_dir.join(script.name);
        let env_file = script.content.replace("{cargo_bin}", &cargo_bin);
        utils::write_file(script.name, &env_name, &env_file)?;
        Ok(())
    }
}

pub(super) struct Posix;

impl UnixShell for Posix {
    fn does_exist(&self, _: &Process) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "sh/ash/dash/pdksh"
    }

    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        let home_dir = process.home_dir();
        // Write to .profile even if it doesn't exist. It's the only rc in the
        // POSIX spec so it should always be set up.
        Box::new(home_dir.into_iter().map(|dir| dir.join(".profile")))
    }
}

struct Bash;

impl UnixShell for Bash {
    fn does_exist(&self, process: &Process) -> bool {
        self.rc_candidates(process).any(|rc| rc.is_file())
    }

    fn name(&self) -> &'static str {
        "bash"
    }

    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        let home_dir = process.home_dir();
        // Bash also may read .profile, however Rustup already includes handling
        // .profile as part of POSIX and always does setup for POSIX shells.
        Box::new(
            [".bash_profile", ".bash_login", ".bashrc"]
                .into_iter()
                .filter_map(move |rc| home_dir.as_ref().map(|dir| dir.join(rc))),
        )
    }

    fn rcs(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        Box::new(self.rc_candidates(process).filter(|rc| rc.is_file()))
    }
}

struct Zsh;

impl Zsh {
    fn zdotdir(process: &Process) -> anyhow::Result<PathBuf> {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

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

    fn name(&self) -> &'static str {
        "zsh"
    }

    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        let home_dir = process.home_dir();
        Box::new(
            Self::zdotdir(process)
                .into_iter()
                .map(|dir| dir.join(".zshenv"))
                .chain(home_dir.into_iter().map(|dir| dir.join(".zshenv"))),
        )
    }

    fn rcs(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        // zsh can change $ZDOTDIR both _before_ AND _during_ reading .zshenv,
        // so we: write to $ZDOTDIR/.zshenv if-exists ($ZDOTDIR changes before)
        // OR write to $HOME/.zshenv if it exists (change-during)
        // if neither exist, we create it ourselves, but using the same logic,
        // because we must still respond to whether $ZDOTDIR is set or unset.
        // In any case we only write once.
        Box::new(
            self.rc_candidates(process)
                .find_or_first(|rc| rc.is_file())
                .into_iter(),
        )
    }
}

struct Fish;

impl UnixShell for Fish {
    fn does_exist(&self, process: &Process) -> bool {
        // fish has to either be the shell or be callable for fish setup.
        matches!(process.var("SHELL"), Ok(sh) if sh.contains("fish"))
            || utils::find_cmd(&["fish"], process).is_some()
    }

    fn name(&self) -> &'static str {
        "fish"
    }

    // > "$XDG_CONFIG_HOME/fish/conf.d" (or "~/.config/fish/conf.d" if that variable is unset) for the user
    // from <https://github.com/fish-shell/fish-shell/issues/3170#issuecomment-228311857>
    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        let home_dir = process.home_dir();
        // The first rcfile takes precedence.
        Box::new(
            process
                .var("XDG_CONFIG_HOME")
                .into_iter()
                .map(|dir| Path::new(&dir).join("fish/conf.d/rustup.fish"))
                .chain(
                    home_dir
                        .into_iter()
                        .map(|home| home.join(".config/fish/conf.d/rustup.fish")),
                ),
        )
    }

    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env.fish",
            content: include_str!("env.fish"),
        }
    }

    fn source_string(&self, env_dir: &Path, home_dir: Option<&Path>) -> anyhow::Result<String> {
        Ok(format!(
            r#"source "{}/env.fish""#,
            self.env_dir_str(env_dir, home_dir)?
        ))
    }
}

pub(super) struct Nu;

impl UnixShell for Nu {
    fn does_exist(&self, process: &Process) -> bool {
        // nu has to either be the shell or be callable for nu setup.
        matches!(process.var("SHELL"), Ok(sh) if sh.contains("nu"))
            || utils::find_cmd(&["nu"], process).is_some()
    }

    fn name(&self) -> &'static str {
        "nushell"
    }

    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        let home_dir = process.home_dir();
        // The first rcfile in XDG_CONFIG_HOME takes precedence.
        Box::new(
            process
                .var("XDG_CONFIG_HOME")
                .into_iter()
                .map(|dir| Path::new(&dir).join("nushell/config.nu"))
                .chain(
                    home_dir
                        .into_iter()
                        .map(|home| home.join(".config/nushell/config.nu")),
                ),
        )
    }

    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env.nu",
            content: include_str!("env.nu"),
        }
    }

    fn source_string(&self, env_dir: &Path, home_dir: Option<&Path>) -> anyhow::Result<String> {
        Ok(format!(
            r#"source "{}/env.nu""#,
            self.env_dir_str(env_dir, home_dir)?
        ))
    }

    fn home_var(&self) -> &'static str {
        "~"
    }
}

struct Tcsh;

impl UnixShell for Tcsh {
    fn does_exist(&self, process: &Process) -> bool {
        matches!(process.var("SHELL"), Ok(sh) if sh.contains("tcsh"))
            || utils::find_cmd(&["tcsh"], process).is_some()
    }

    fn name(&self) -> &'static str {
        "tcsh"
    }

    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        let home_dir = process.home_dir();
        Box::new(
            [".tcshrc", ".cshrc"]
                .into_iter()
                .filter_map(move |rc| home_dir.as_ref().map(|home| home.join(rc))),
        )
    }

    fn rcs(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        // Prefer .tcshrc over .cshrc.
        // If neither exists, default to ~/.tcshrc
        Box::new(
            self.rc_candidates(process)
                .find_or_first(|rc| rc.is_file())
                .into_iter(),
        )
    }

    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env.tcsh",
            content: include_str!("env.tcsh"),
        }
    }

    fn source_string(&self, env_dir: &Path, home_dir: Option<&Path>) -> anyhow::Result<String> {
        Ok(format!(
            r#"source "{}/env.tcsh""#,
            self.env_dir_str(env_dir, home_dir)?
        ))
    }
}

struct Pwsh;

impl UnixShell for Pwsh {
    fn does_exist(&self, process: &Process) -> bool {
        matches!(process.var("SHELL"), Ok(sh) if sh.contains("pwsh"))
            || utils::find_cmd(&["pwsh"], process).is_some()
    }

    fn name(&self) -> &'static str {
        "pwsh"
    }

    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        let home_dir = process.home_dir();
        // PowerShell provides many kinds of user-specific and host-specific
        // profile files. When the system has multiple profiles, PowerShell
        // executes them in a defined order.
        //
        // For more details, please refer to:
        // https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_profiles
        let config_dir = home_dir.map(|home| home.join(".config/powershell"));

        // `~/.config/powershell/profile.ps1` is the "Current User, All Hosts"
        // profile file. It affects all PowerShell hosts of the current user.
        // Always modify the "Current User, All Hosts" profile.
        let profile = config_dir.as_ref().map(|dir| dir.join("profile.ps1"));

        // Some editors like Visual Studio Code or PowerShell ISE use their
        // own dedicated profile files, whose file names are
        // `<Host Profile ID>_profile.ps1`. Such customization may use
        // PowerShell Editor Services for IDE integration.
        // https://github.com/PowerShell/PowerShellEditorServices
        let host_profiles = config_dir
            .into_iter()
            .flat_map(|dir| dir.read_dir().into_iter().flatten())
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.ends_with("_profile.ps1"));
        Box::new(profile.into_iter().chain(host_profiles))
    }

    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env.ps1",
            content: include_str!("env.ps1"),
        }
    }

    fn source_string(&self, env_dir: &Path, home_dir: Option<&Path>) -> anyhow::Result<String> {
        Ok(format!(
            r#". "{}/env.ps1""#,
            self.env_dir_str(env_dir, home_dir)?
        ))
    }
}

struct Xonsh;

impl UnixShell for Xonsh {
    fn does_exist(&self, process: &Process) -> bool {
        process.var("XONSHRC").is_ok() || utils::find_cmd(&["xonsh"], process).is_some()
    }

    fn name(&self) -> &'static str {
        "xonsh"
    }

    fn rc_candidates(&self, process: &Process) -> Box<dyn Iterator<Item = PathBuf>> {
        let home_dir = process.home_dir();
        // The first rcfile in XDG_CONFIG_HOME takes precedence.
        let home_rcs = [".config/xonsh/rc.xsh", ".xonshrc"]
            .into_iter()
            .filter_map(move |rc| home_dir.as_ref().map(|home| home.join(rc)));
        Box::new(
            process
                .var("XDG_CONFIG_HOME")
                .into_iter()
                .map(|dir| Path::new(&dir).join("xonsh/rc.xsh"))
                .chain(home_rcs),
        )
    }

    fn env_script(&self) -> ShellScript {
        ShellScript {
            name: "env.xsh",
            content: include_str!("env.xsh"),
        }
    }

    fn source_string(&self, env_dir: &Path, home_dir: Option<&Path>) -> anyhow::Result<String> {
        Ok(format!(
            r#"source "{}/env.xsh""#,
            self.env_dir_str(env_dir, home_dir)?
        ))
    }

    fn home_var(&self) -> &'static str {
        "$HOME"
    }
}

pub(crate) fn legacy_paths<'a>(
    process: &Process,
    home_dir: Option<&'a Path>,
) -> impl Iterator<Item = PathBuf> + 'a {
    let zprofiles = Zsh::zdotdir(process)
        .into_iter()
        .map(|dir| dir.join(".zprofile"))
        .chain(home_dir.map(|dir| dir.join(".zprofile")));
    let profiles = [".bash_profile", ".profile"]
        .into_iter()
        .filter_map(move |rc| home_dir.map(|dir| dir.join(rc)));

    profiles.chain(zprofiles)
}
