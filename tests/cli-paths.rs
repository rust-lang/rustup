//! This file contains tests relevant to Rustup's handling of updating PATHs.
//! It depends on self-update working, so if absolutely everything here breaks,
//! check those tests as well.
pub mod mock;

// Prefer omitting actually unpacking content while just testing paths.
const INIT_NONE: [&str; 4] = ["rustup-init", "-y", "--default-toolchain", "none"];

#[cfg(unix)]
mod unix {
    use std::fmt::Display;
    use std::fs;
    use std::path::PathBuf;

    use rustup::utils::raw;

    use super::INIT_NONE;
    use crate::mock::clitools::{self, expect_err, expect_ok, Scenario};

    // Let's write a fake .rc which looks vaguely like a real script.
    const FAKE_RC: &str = r#"
# Sources fruity punch.
. ~/fruit/punch

# Adds apples to PATH.
export PATH="$HOME/apple/bin"
"#;
    const DEFAULT_EXPORT: &str = "export PATH=\"$HOME/.cargo/bin:$PATH\"\n";
    const POSIX_SH: &str = "env";

    fn source(dir: impl Display, sh: impl Display) -> String {
        format!(". \"{dir}/{sh}\"\n", dir = dir, sh = sh)
    }

    #[test]
    fn install_creates_necessary_scripts() {
        clitools::setup(Scenario::Empty, &|config| {
            // Override the test harness so that cargo home looks like
            // $HOME/.cargo by removing CARGO_HOME from the environment,
            // otherwise the literal path will be written to the file.

            let mut cmd = clitools::cmd(config, "rustup-init", &INIT_NONE[1..]);
            let files: Vec<PathBuf> = [".cargo/env", ".profile", ".zshenv"]
                .iter()
                .map(|file| config.homedir.join(file))
                .collect();
            for file in &files {
                assert!(!file.exists());
            }
            cmd.env_remove("CARGO_HOME");
            cmd.env("SHELL", "zsh");
            assert!(cmd.output().unwrap().status.success());
            let mut rcs = files.iter();
            let env = rcs.next().unwrap();
            let envfile = fs::read_to_string(&env).unwrap();
            let (_, envfile_export) = envfile.split_at(envfile.find("export PATH").unwrap_or(0));
            assert_eq!(&envfile_export[..DEFAULT_EXPORT.len()], DEFAULT_EXPORT);

            for rc in rcs {
                let expected = source("$HOME/.cargo", POSIX_SH);
                let new_profile = fs::read_to_string(&rc).unwrap();
                assert_eq!(new_profile, expected);
            }
        });
    }

    #[test]
    fn install_updates_bash_rcs() {
        clitools::setup(Scenario::Empty, &|config| {
            let rcs: Vec<PathBuf> = [".bashrc", ".bash_profile", ".bash_login", ".profile"]
                .iter()
                .map(|rc| config.homedir.join(rc))
                .collect();
            for rc in &rcs {
                raw::write_file(&rc, FAKE_RC).unwrap();
            }

            expect_ok(config, &INIT_NONE);

            let expected = FAKE_RC.to_owned() + &source(config.cargodir.display(), POSIX_SH);
            for rc in &rcs {
                let new_rc = fs::read_to_string(&rc).unwrap();
                assert_eq!(new_rc, expected);
            }
        })
    }

    #[test]
    fn install_does_not_create_bash_rcs() {
        clitools::setup(Scenario::Empty, &|config| {
            let rcs: Vec<PathBuf> = [".bashrc", ".bash_profile", ".bash_login"]
                .iter()
                .map(|rc| config.homedir.join(rc))
                .collect();
            let rcs_before = rcs.iter().map(|rc| rc.exists());
            expect_ok(config, &INIT_NONE);

            for (before, after) in rcs_before.zip(rcs.iter().map(|rc| rc.exists())) {
                assert!(!before);
                assert_eq!(before, after);
            }
        });
    }

    #[test]
    fn install_errors_when_rc_cannot_be_updated() {
        clitools::setup(Scenario::Empty, &|config| {
            let rc = config.homedir.join(".profile");
            fs::File::create(&rc).unwrap();
            let mut perms = fs::metadata(&rc).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(&rc, perms).unwrap();

            expect_err(config, &INIT_NONE, "amend shell");
        });
    }

    #[test]
    fn install_with_zdotdir() {
        clitools::setup(Scenario::Empty, &|config| {
            let zdotdir = tempfile::Builder::new()
                .prefix("zdotdir")
                .tempdir()
                .unwrap();
            let rc = zdotdir.path().join(".zshenv");
            raw::write_file(&rc, FAKE_RC).unwrap();

            let mut cmd = clitools::cmd(config, "rustup-init", &INIT_NONE[1..]);
            cmd.env("SHELL", "zsh");
            cmd.env("ZDOTDIR", zdotdir.path());
            assert!(cmd.output().unwrap().status.success());

            let new_rc = fs::read_to_string(&rc).unwrap();
            let expected = FAKE_RC.to_owned() + &source(config.cargodir.display(), POSIX_SH);
            assert_eq!(new_rc, expected);
        });
    }

    #[test]
    fn install_adds_path_to_rc_just_once() {
        clitools::setup(Scenario::Empty, &|config| {
            let profile = config.homedir.join(".profile");
            raw::write_file(&profile, FAKE_RC).unwrap();
            expect_ok(config, &INIT_NONE);
            expect_ok(config, &INIT_NONE);

            let new_profile = fs::read_to_string(&profile).unwrap();
            let expected = FAKE_RC.to_owned() + &source(config.cargodir.display(), POSIX_SH);
            assert_eq!(new_profile, expected);
        });
    }

    #[test]
    fn uninstall_removes_source_from_rcs() {
        clitools::setup(Scenario::Empty, &|config| {
            let rcs: Vec<PathBuf> = [
                ".bashrc",
                ".bash_profile",
                ".bash_login",
                ".profile",
                ".zshenv",
            ]
            .iter()
            .map(|rc| config.homedir.join(rc))
            .collect();

            for rc in &rcs {
                raw::write_file(&rc, FAKE_RC).unwrap();
            }

            expect_ok(config, &INIT_NONE);
            expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

            for rc in &rcs {
                let new_rc = fs::read_to_string(&rc).unwrap();
                assert_eq!(new_rc, FAKE_RC);
            }
        })
    }

    #[test]
    fn install_adds_sources_while_removing_legacy_paths() {
        clitools::setup(Scenario::Empty, &|config| {
            let zdotdir = tempfile::Builder::new()
                .prefix("zdotdir")
                .tempdir()
                .unwrap();
            let rcs: Vec<PathBuf> = [".bash_profile", ".profile"]
                .iter()
                .map(|rc| config.homedir.join(rc))
                .collect();
            let zprofiles = vec![
                config.homedir.join(".zprofile"),
                zdotdir.path().join(".zprofile"),
            ];
            let old_rc = FAKE_RC.to_owned() + DEFAULT_EXPORT;
            for rc in rcs.iter().chain(zprofiles.iter()) {
                raw::write_file(&rc, &old_rc).unwrap();
            }

            let mut cmd = clitools::cmd(config, "rustup-init", &INIT_NONE[1..]);
            cmd.env("SHELL", "zsh");
            cmd.env("ZDOTDIR", zdotdir.path());
            cmd.env_remove("CARGO_HOME");
            assert!(cmd.output().unwrap().status.success());
            let fixed_rc = FAKE_RC.to_owned() + &source("$HOME/.cargo", POSIX_SH);
            for rc in &rcs {
                let new_rc = fs::read_to_string(&rc).unwrap();
                assert_eq!(new_rc, fixed_rc);
            }
            for rc in &zprofiles {
                let new_rc = fs::read_to_string(&rc).unwrap();
                assert_eq!(new_rc, FAKE_RC);
            }
        })
    }

    #[test]
    fn uninstall_cleans_up_legacy_paths() {
        clitools::setup(Scenario::Empty, &|config| {
            // Install first, then overwrite.
            expect_ok(config, &INIT_NONE);

            let zdotdir = tempfile::Builder::new()
                .prefix("zdotdir")
                .tempdir()
                .unwrap();
            let mut cmd = clitools::cmd(config, "rustup-init", &INIT_NONE[1..]);
            cmd.env("SHELL", "zsh");
            cmd.env("ZDOTDIR", zdotdir.path());
            cmd.env_remove("CARGO_HOME");
            assert!(cmd.output().unwrap().status.success());
            let mut rcs: Vec<PathBuf> = [".bash_profile", ".profile", ".zprofile"]
                .iter()
                .map(|rc| config.homedir.join(rc))
                .collect();
            rcs.push(zdotdir.path().join(".zprofile"));
            let old_rc = FAKE_RC.to_owned() + DEFAULT_EXPORT;
            for rc in &rcs {
                raw::write_file(&rc, &old_rc).unwrap();
            }

            let mut cmd = clitools::cmd(config, "rustup", &["self", "uninstall", "-y"]);
            cmd.env("SHELL", "zsh");
            cmd.env("ZDOTDIR", zdotdir.path());
            cmd.env_remove("CARGO_HOME");
            assert!(cmd.output().unwrap().status.success());

            for rc in &rcs {
                let new_rc = fs::read_to_string(&rc).unwrap();
                // It's not ideal, but it's OK, if we leave whitespace.
                assert_eq!(new_rc, FAKE_RC);
            }
        })
    }

    // In the default case we want to write $HOME/.cargo/bin as the path,
    // not the full path.
    #[test]
    fn when_cargo_home_is_the_default_write_path_specially() {
        clitools::setup(Scenario::Empty, &|config| {
            // Override the test harness so that cargo home looks like
            // $HOME/.cargo by removing CARGO_HOME from the environment,
            // otherwise the literal path will be written to the file.

            let profile = config.homedir.join(".profile");
            raw::write_file(&profile, FAKE_RC).unwrap();
            let mut cmd = clitools::cmd(config, "rustup-init", &INIT_NONE[1..]);
            cmd.env_remove("CARGO_HOME");
            assert!(cmd.output().unwrap().status.success());

            let new_profile = fs::read_to_string(&profile).unwrap();
            let expected = format!("{}. \"$HOME/.cargo/env\"\n", FAKE_RC);
            assert_eq!(new_profile, expected);

            let mut cmd = clitools::cmd(config, "rustup", &["self", "uninstall", "-y"]);
            cmd.env_remove("CARGO_HOME");
            assert!(cmd.output().unwrap().status.success());

            let new_profile = fs::read_to_string(&profile).unwrap();
            assert_eq!(new_profile, FAKE_RC);
        });
    }

    #[test]
    fn install_doesnt_modify_path_if_passed_no_modify_path() {
        clitools::setup(Scenario::Empty, &|config| {
            let profile = config.homedir.join(".profile");
            expect_ok(
                config,
                &[
                    "rustup-init",
                    "-y",
                    "--no-modify-path",
                    "--default-toolchain",
                    "none",
                ],
            );
            assert!(!profile.exists());
        });
    }
}

#[cfg(windows)]
mod windows {
    use rustup::test::{get_path, with_saved_path};

    use super::INIT_NONE;
    use crate::mock::clitools::{self, expect_ok, Scenario};

    #[test]
    #[cfg(windows)]
    /// Smoke test for end-to-end code connectivity of the installer path mgmt on windows.
    fn install_uninstall_affect_path() {
        clitools::setup(Scenario::Empty, &|config| {
            with_saved_path(&|| {
                let path = format!("{:?}", config.cargodir.join("bin").to_string_lossy());

                expect_ok(config, &INIT_NONE);
                assert!(
                    get_path()
                        .unwrap()
                        .to_string()
                        .contains(path.trim_matches('"')),
                    format!("`{}` not in `{}`", path, get_path().unwrap())
                );

                expect_ok(config, &["rustup", "self", "uninstall", "-y"]);
                assert!(!get_path().unwrap().to_string().contains(&path));
            })
        });
    }
}
