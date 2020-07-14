//! This file contains tests relevant to Rustup's handling of updating PATHs.
//! It depends on self-update working, so if absolutely everything here breaks,
//! check those tests as well.
#![allow(unused)]
pub mod mock;

use crate::mock::clitools::{
    self, expect_err, expect_err_ex, expect_ok, expect_ok_contains, expect_ok_ex, expect_stderr_ok,
    expect_stdout_ok, this_host_triple, Config, Scenario,
};
use crate::mock::dist::calc_hash;
use crate::mock::{get_path, restore_path};
use lazy_static::lazy_static;
use remove_dir_all::remove_dir_all;
use rustup::utils::{raw, utils};
use rustup::Notification;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

macro_rules! for_host {
    ($s: expr) => {
        &format!($s, this_host_triple())
    };
}

const TEST_VERSION: &str = "1.1.1";

pub fn setup(f: &dyn Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, &|config| {
        // Lock protects environment variables
        lazy_static! {
            static ref LOCK: Mutex<()> = Mutex::new(());
        }
        let _g = LOCK.lock();

        // On windows these tests mess with the user's PATH. Save
        // and restore them here to keep from trashing things.
        let saved_path = get_path();
        let _g = scopeguard::guard(saved_path, restore_path);

        f(config);
    });
}

#[cfg(unix)]
mod unix {
    use super::*;

    #[test]
    fn produces_env_file_on_unix() {
        setup(&|config| {
            // Override the test harness so that cargo home looks like
            // $HOME/.cargo by removing CARGO_HOME from the environment,
            // otherwise the literal path will be written to the file.

            let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
            cmd.env_remove("CARGO_HOME");
            assert!(cmd.output().unwrap().status.success());
            let envfile = config.homedir.join(".cargo/env.sh");
            let envfile = fs::read_to_string(&envfile).unwrap();
            let path_string = "export PATH=\"$HOME/.cargo/bin:${PATH}\"";
            let (_, envfile_export) = envfile.split_at(match envfile.find("export PATH") {
                Some(idx) => idx,
                None => 0,
            });
            assert_eq!(&envfile_export[..path_string.len()], path_string);
        });
    }

    fn install_adds_path_to_rc(rcfile: &str) {
        setup(&|config| {
            let my_rc = "foo\nbar\nbaz";
            let rc = config.homedir.join(rcfile);
            raw::write_file(&rc, my_rc).unwrap();
            expect_ok(config, &["rustup-init", "-y"]);

            let new_rc = fs::read_to_string(&rc).unwrap();
            let addition = format!("source \"{}/env.sh\"", config.cargodir.display());
            let expected = format!("{}\n{}\n", my_rc, addition);
            assert_eq!(new_rc, expected);
        });
    }

    #[test]
    fn install_adds_path_to_profile() {
        install_adds_path_to_rc(".profile");
    }

    #[test]
    fn install_adds_path_to_bashrc() {
        install_adds_path_to_rc(".bashrc");
    }

    #[test]
    fn install_adds_path_to_bash_profile() {
        install_adds_path_to_rc(".bash_profile");
    }

    #[test]
    fn install_does_not_add_path_to_bash_profile_that_doesnt_exist() {
        setup(&|config| {
            let rc = config.homedir.join(".bash_profile");
            expect_ok(config, &["rustup-init", "-y"]);

            assert!(!rc.exists());
        });
    }

    #[test]
    fn install_errors_when_rc_file_cannot_be_updated() {
        setup(&|config| {
            let rc = config.homedir.join(".bashrc");
            fs::File::create(&rc).unwrap();
            let mut perms = fs::metadata(&rc).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(&rc, perms).unwrap();

            expect_err(config, &["rustup-init", "-y"], "amend shell");
        });
    }

    #[test]
    fn install_with_zsh_adds_path_to_zshenv() {
        setup(&|config| {
            let my_rc = "foo\nbar\nbaz";
            let rc = config.homedir.join(".zshenv");
            raw::write_file(&rc, my_rc).unwrap();

            let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
            cmd.env("SHELL", "zsh");
            assert!(cmd.output().unwrap().status.success());

            let new_rc = fs::read_to_string(&rc).unwrap();
            let addition = format!(r#"source "{}/env.sh""#, config.cargodir.display());
            let expected = format!("{}\n{}\n", my_rc, addition);
            assert_eq!(new_rc, expected);
        });
    }

    #[test]
    fn install_with_zdotdir() {
        // New strategy: move all zdotdir tests into one pile
        // Move all zsh-without-zdotdir tests into the pile with bash and posix profiles
        setup(&|config| {
            let zdotdir = tempfile::Builder::new()
                .prefix("zdotdir")
                .tempdir()
                .unwrap();
            let my_rc = "foo\nbar\nbaz";
            let rc = zdotdir.path().join(".zshenv");
            let profile = zdotdir.path().join(".zprofile");
            raw::write_file(&rc, my_rc).unwrap();

            let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
            cmd.env("SHELL", "zsh");
            cmd.env("ZDOTDIR", zdotdir.path());
            assert!(cmd.output().unwrap().status.success());

            let new_rc = fs::read_to_string(&rc).unwrap();
            let addition = format!(r#"source "{}/env.sh""#, config.cargodir.display());
            let expected = format!("{}\n{}\n", my_rc, addition);
            assert_eq!(new_rc, expected);
        });
    }

    #[test]
    fn install_adds_path_to_rcfile_just_once() {
        setup(&|config| {
            let my_profile = "foo\nbar\nbaz";
            let profile = config.homedir.join(".profile");
            raw::write_file(&profile, my_profile).unwrap();
            expect_ok(config, &["rustup-init", "-y"]);
            expect_ok(config, &["rustup-init", "-y"]);

            let new_profile = fs::read_to_string(&profile).unwrap();
            let addition = format!(r#"source "{}/env.sh""#, config.cargodir.display());
            let expected = format!("{}\n{}\n", my_profile, addition);
            assert_eq!(new_profile, expected);
        });
    }

    fn uninstall_removes_path_from_rc(rcfile: &str) {
        setup(&|config| {
            let my_rc = "foo\nbar\nbaz";
            let rc = config.homedir.join(rcfile);
            raw::write_file(&rc, my_rc).unwrap();
            expect_ok(config, &["rustup-init", "-y"]);
            expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

            let new_rc = fs::read_to_string(&rc).unwrap();
            assert_eq!(new_rc, my_rc);
        });
    }

    #[test]
    fn uninstall_removes_path_from_profile() {
        uninstall_removes_path_from_rc(".profile");
    }

    #[test]
    fn uninstall_removes_path_from_bashrc() {
        uninstall_removes_path_from_rc(".bashrc");
    }

    #[test]
    fn uninstall_removes_path_from_bash_profile() {
        uninstall_removes_path_from_rc(".bash_profile");
    }

    #[test]
    fn uninstall_removes_path_from_zshenv() {
        uninstall_removes_path_from_rc(".zshenv");
    }

    #[test]
    fn uninstall_doesnt_touch_rc_files_that_dont_contain_cargo_home() {
        setup(&|config| {
            let my_rc = "foo\nbar\nbaz";
            expect_ok(config, &["rustup-init", "-y"]);
            expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

            let profile = config.homedir.join(".profile");
            raw::write_file(&profile, my_rc).unwrap();

            let profile = fs::read_to_string(&profile).unwrap();

            assert_eq!(profile, my_rc);
        });
    }

    // In the default case we want to write $HOME/.cargo/bin as the path,
    // not the full path.
    #[test]
    fn when_cargo_home_is_the_default_write_path_specially() {
        setup(&|config| {
            // Override the test harness so that cargo home looks like
            // $HOME/.cargo by removing CARGO_HOME from the environment,
            // otherwise the literal path will be written to the file.

            let my_profile = "foo\nbar\nbaz";
            let profile = config.homedir.join(".profile");
            raw::write_file(&profile, my_profile).unwrap();
            let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
            cmd.env_remove("CARGO_HOME");
            assert!(cmd.output().unwrap().status.success());

            let new_profile = fs::read_to_string(&profile).unwrap();
            let expected = format!("{}\nsource \"$HOME/.cargo/env.sh\"\n", my_profile);
            assert_eq!(new_profile, expected);

            let mut cmd = clitools::cmd(config, "rustup", &["self", "uninstall", "-y"]);
            cmd.env_remove("CARGO_HOME");
            assert!(cmd.output().unwrap().status.success());

            let new_profile = fs::read_to_string(&profile).unwrap();
            assert_eq!(new_profile, my_profile);
        });
    }

    #[test]
    fn install_doesnt_modify_path_if_passed_no_modify_path() {
        setup(&|config| {
            let profile = config.homedir.join(".profile");
            expect_ok(config, &["rustup-init", "-y", "--no-modify-path"]);
            assert!(!profile.exists());
        });
    }
}

#[cfg(windows)]
mod windows {

    use super::*;

    #[test]
    fn install_doesnt_modify_path_if_passed_no_modify_path() {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
        use winreg::RegKey;

        setup(&|config| {
            let root = RegKey::predef(HKEY_CURRENT_USER);
            let environment = root
                .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                .unwrap();
            let old_path = environment.get_raw_value("PATH").unwrap();

            expect_ok(config, &["rustup-init", "-y", "--no-modify-path"]);

            let root = RegKey::predef(HKEY_CURRENT_USER);
            let environment = root
                .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                .unwrap();
            let new_path = environment.get_raw_value("PATH").unwrap();

            assert_eq!(old_path, new_path);
        });
    }

    // HKCU\Environment\PATH may not exist during install, and it may need to be
    // deleted during uninstall if we remove the last path from it
    #[test]
    fn windows_handle_empty_path_registry_key() {
        use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
        use winreg::RegKey;

        setup(&|config| {
            let root = RegKey::predef(HKEY_CURRENT_USER);
            let environment = root
                .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                .unwrap();
            let _ = environment.delete_value("PATH");

            expect_ok(config, &["rustup-init", "-y"]);

            let root = RegKey::predef(HKEY_CURRENT_USER);
            let environment = root
                .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                .unwrap();
            let path = environment.get_raw_value("PATH").unwrap();
            assert!(path.vtype == RegType::REG_EXPAND_SZ);

            expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

            let root = RegKey::predef(HKEY_CURRENT_USER);
            let environment = root
                .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
                .unwrap();
            let path = environment.get_raw_value("PATH");

            assert!(path.is_err());
        });
    }

    #[test]
    fn uninstall_removes_path() {
        setup(&|config| {
            expect_ok(config, &["rustup-init", "-y"]);
            expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

            let path = config.cargodir.join("bin").to_string_lossy().to_string();
            assert!(!get_path().unwrap().contains(&path));
        });
    }

    #[test]
    fn install_adds_path() {
        setup(&|config| {
            expect_ok(config, &["rustup-init", "-y"]);

            let path = config.cargodir.join("bin").to_string_lossy().to_string();
            assert!(
                get_path().unwrap().contains(&path),
                format!("`{}` not in `{}`", get_path().unwrap(), &path)
            );
        });
    }
}
