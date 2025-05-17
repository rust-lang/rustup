//! This file contains tests relevant to Rustup's handling of updating PATHs.
//! It depends on self-update working, so if absolutely everything here breaks,
//! check those tests as well.

#![allow(deprecated)]

// Prefer omitting actually unpacking content while just testing paths.
const INIT_NONE: [&str; 4] = ["rustup-init", "-y", "--default-toolchain", "none"];

#[cfg(unix)]
mod unix {
    use std::fmt::Display;
    use std::fs;
    use std::path::PathBuf;

    use super::INIT_NONE;
    use rustup::test::{CliTestContext, Scenario};
    use rustup::utils::raw;

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
        format!(". \"{dir}/{sh}\"\n")
    }

    // In 1.23 we used `source` instead of `.` by accident.  This is not POSIX
    // so we want to ensure that if we put this into someone's dot files, then
    // with newer rustups we will revert that.
    fn non_posix_source(dir: impl Display, sh: impl Display) -> String {
        format!("source \"{dir}/{sh}\"\n")
    }

    #[tokio::test]
    async fn install_creates_necessary_scripts() {
        let cx = CliTestContext::new(Scenario::Empty).await;
        // Override the test harness so that cargo home looks like
        // $HOME/.cargo by removing CARGO_HOME from the environment,
        // otherwise the literal path will be written to the file.

        let mut cmd = cx.config.cmd("rustup-init", &INIT_NONE[1..]);
        let files: Vec<PathBuf> = [".cargo/env", ".profile", ".zshenv"]
            .iter()
            .map(|file| cx.config.homedir.join(file))
            .collect();
        for file in &files {
            assert!(!file.exists());
        }
        cmd.env_remove("CARGO_HOME");
        cmd.env("SHELL", "zsh");
        assert!(cmd.output().unwrap().status.success());
        let mut rcs = files.iter();
        let env = rcs.next().unwrap();
        let envfile = fs::read_to_string(env).unwrap();
        let (_, envfile_export) = envfile.split_at(envfile.find("export PATH").unwrap_or(0));
        assert_eq!(&envfile_export[..DEFAULT_EXPORT.len()], DEFAULT_EXPORT);

        for rc in rcs {
            let expected = source("$HOME/.cargo", POSIX_SH);
            let new_profile = fs::read_to_string(rc).unwrap();
            assert_eq!(new_profile, expected);
        }
    }

    #[tokio::test]
    async fn install_updates_bash_rcs() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        let rcs: Vec<PathBuf> = [".bashrc", ".bash_profile", ".bash_login", ".profile"]
            .iter()
            .map(|rc| cx.config.homedir.join(rc))
            .collect();
        for rc in &rcs {
            raw::write_file(rc, FAKE_RC).unwrap();
        }

        cx.config.expect_ok(&INIT_NONE).await;

        let expected = FAKE_RC.to_owned() + &source(cx.config.cargodir.display(), POSIX_SH);
        for rc in &rcs {
            let new_rc = fs::read_to_string(rc).unwrap();
            assert_eq!(new_rc, expected);
        }
    }

    #[tokio::test]
    async fn install_does_not_create_bash_rcs() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        let rcs: Vec<PathBuf> = [".bashrc", ".bash_profile", ".bash_login"]
            .iter()
            .map(|rc| cx.config.homedir.join(rc))
            .collect();
        let rcs_before = rcs.iter().map(|rc| rc.exists());
        cx.config.expect_ok(&INIT_NONE).await;

        for (before, after) in rcs_before.zip(rcs.iter().map(|rc| rc.exists())) {
            assert!(!before);
            assert_eq!(before, after);
        }
    }

    // This test should NOT be run as root!
    #[tokio::test]
    async fn install_errors_when_rc_cannot_be_updated() {
        let cx = CliTestContext::new(Scenario::Empty).await;
        let rc = cx.config.homedir.join(".profile");
        fs::File::create(&rc).unwrap();
        let mut perms = fs::metadata(&rc).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&rc, perms).unwrap();

        cx.config.expect_err(&INIT_NONE, "amend shell").await;
    }

    #[tokio::test]
    async fn install_with_zdotdir() {
        let cx = CliTestContext::new(Scenario::Empty).await;
        let zdotdir = tempfile::Builder::new()
            .prefix("zdotdir")
            .tempdir()
            .unwrap();
        let rc = zdotdir.path().join(".zshenv");
        raw::write_file(&rc, FAKE_RC).unwrap();

        let mut cmd = cx.config.cmd("rustup-init", &INIT_NONE[1..]);
        cmd.env("SHELL", "zsh");
        cmd.env("ZDOTDIR", zdotdir.path());
        assert!(cmd.output().unwrap().status.success());

        let new_rc = fs::read_to_string(&rc).unwrap();
        let expected = FAKE_RC.to_owned() + &source(cx.config.cargodir.display(), POSIX_SH);
        assert_eq!(new_rc, expected);
    }

    #[tokio::test]
    async fn install_with_zdotdir_from_calling_zsh() {
        // This test requires that zsh is callable.
        if std::process::Command::new("zsh")
            .arg("-c")
            .arg("true")
            .status()
            .is_err()
        {
            return;
        }

        let cx = CliTestContext::new(Scenario::Empty).await;
        let zdotdir = tempfile::Builder::new()
            .prefix("zdotdir")
            .tempdir()
            .unwrap();
        let rc = zdotdir.path().join(".zshenv");
        raw::write_file(&rc, FAKE_RC).unwrap();

        // If $SHELL doesn't include "zsh", Zsh::zdotdir() will call zsh to obtain $ZDOTDIR.
        // ZDOTDIR could be set directly in the environment, but having ~/.zshenv set
        // ZDOTDIR is a normal setup, and ensures that the value came from calling zsh.
        let home_zshenv = cx.config.homedir.join(".zshenv");
        let export_zdotdir = format!(
            "export ZDOTDIR=\"{}\"\n",
            zdotdir.path().as_os_str().to_str().unwrap()
        );
        raw::write_file(&home_zshenv, &export_zdotdir).unwrap();

        let mut cmd = cx.config.cmd("rustup-init", &INIT_NONE[1..]);
        cmd.env("SHELL", "/bin/sh");
        assert!(cmd.output().unwrap().status.success());

        let new_rc = fs::read_to_string(&rc).unwrap();
        let expected = FAKE_RC.to_owned() + &source(cx.config.cargodir.display(), POSIX_SH);
        assert_eq!(new_rc, expected);
    }

    #[tokio::test]
    async fn install_adds_path_to_rc_just_once() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        let profile = cx.config.homedir.join(".profile");
        raw::write_file(&profile, FAKE_RC).unwrap();
        cx.config.expect_ok(&INIT_NONE).await;
        cx.config.expect_ok(&INIT_NONE).await;

        let new_profile = fs::read_to_string(&profile).unwrap();
        let expected = FAKE_RC.to_owned() + &source(cx.config.cargodir.display(), POSIX_SH);
        assert_eq!(new_profile, expected);
    }

    #[tokio::test]
    async fn install_adds_path_to_rc_handling_no_newline() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        let profile = cx.config.homedir.join(".profile");
        let fake_rc_modified = FAKE_RC.strip_suffix('\n').expect("Should end in a newline");
        raw::write_file(&profile, fake_rc_modified).unwrap();
        // Run once to add the configuration
        cx.config.expect_ok(&INIT_NONE).await;
        // Run twice to test that the process is idempotent
        cx.config.expect_ok(&INIT_NONE).await;

        let new_profile = fs::read_to_string(&profile).unwrap();
        let expected = FAKE_RC.to_owned() + &source(cx.config.cargodir.display(), POSIX_SH);
        assert_eq!(new_profile, expected);
    }

    #[tokio::test]
    async fn install_adds_path_to_multiple_rc_files() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        // Two RC files that are both from the same shell
        let bash_profile = cx.config.homedir.join(".bash_profile");
        let bashrc = cx.config.homedir.join(".bashrc");

        let expected = FAKE_RC.to_owned() + &source(cx.config.cargodir.display(), POSIX_SH);

        // The order that the two files are processed isn't known, so test both orders
        for [path1, path2] in &[[&bash_profile, &bashrc], [&bashrc, &bash_profile]] {
            raw::write_file(path1, &expected).unwrap();
            raw::write_file(path2, FAKE_RC).unwrap();

            cx.config.expect_ok(&INIT_NONE).await;

            let new1 = fs::read_to_string(path1).unwrap();
            assert_eq!(new1, expected);
            let new2 = fs::read_to_string(path2).unwrap();
            assert_eq!(new2, expected);
        }
    }

    #[tokio::test]
    async fn uninstall_removes_source_from_rcs() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        let rcs: Vec<PathBuf> = [
            ".bashrc",
            ".bash_profile",
            ".bash_login",
            ".profile",
            ".zshenv",
        ]
        .iter()
        .map(|rc| cx.config.homedir.join(rc))
        .collect();

        for rc in &rcs {
            raw::write_file(rc, FAKE_RC).unwrap();
        }

        cx.config.expect_ok(&INIT_NONE).await;
        cx.config
            .expect_ok(&["rustup", "self", "uninstall", "-y"])
            .await;

        for rc in &rcs {
            let new_rc = fs::read_to_string(rc).unwrap();
            assert_eq!(new_rc, FAKE_RC);
        }
    }

    #[tokio::test]
    async fn install_adds_sources_while_removing_legacy_paths() {
        let cx = CliTestContext::new(Scenario::Empty).await;
        let zdotdir = tempfile::Builder::new()
            .prefix("zdotdir")
            .tempdir()
            .unwrap();
        let rcs: Vec<PathBuf> = [".bash_profile", ".profile"]
            .iter()
            .map(|rc| cx.config.homedir.join(rc))
            .collect();
        let zprofiles = vec![
            cx.config.homedir.join(".zprofile"),
            zdotdir.path().join(".zprofile"),
        ];
        let old_rc =
            FAKE_RC.to_owned() + DEFAULT_EXPORT + &non_posix_source("$HOME/.cargo", POSIX_SH);
        for rc in rcs.iter().chain(zprofiles.iter()) {
            raw::write_file(rc, &old_rc).unwrap();
        }

        let mut cmd = cx.config.cmd("rustup-init", &INIT_NONE[1..]);
        cmd.env("SHELL", "zsh");
        cmd.env("ZDOTDIR", zdotdir.path());
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());
        let fixed_rc = FAKE_RC.to_owned() + &source("$HOME/.cargo", POSIX_SH);
        for rc in &rcs {
            let new_rc = fs::read_to_string(rc).unwrap();
            assert_eq!(new_rc, fixed_rc);
        }
        for rc in &zprofiles {
            let new_rc = fs::read_to_string(rc).unwrap();
            assert_eq!(new_rc, FAKE_RC);
        }
    }

    #[tokio::test]
    async fn uninstall_cleans_up_legacy_paths() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        // Install first, then overwrite.
        cx.config.expect_ok(&INIT_NONE).await;

        let zdotdir = tempfile::Builder::new()
            .prefix("zdotdir")
            .tempdir()
            .unwrap();
        let mut cmd = cx.config.cmd("rustup-init", &INIT_NONE[1..]);
        cmd.env("SHELL", "zsh");
        cmd.env("ZDOTDIR", zdotdir.path());
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());
        let mut rcs: Vec<PathBuf> = [".bash_profile", ".profile", ".zprofile"]
            .iter()
            .map(|rc| cx.config.homedir.join(rc))
            .collect();
        rcs.push(zdotdir.path().join(".zprofile"));
        let old_rc =
            FAKE_RC.to_owned() + DEFAULT_EXPORT + &non_posix_source("$HOME/.cargo", POSIX_SH);
        for rc in &rcs {
            raw::write_file(rc, &old_rc).unwrap();
        }

        let mut cmd = cx.config.cmd("rustup", ["self", "uninstall", "-y"]);
        cmd.env("SHELL", "zsh");
        cmd.env("ZDOTDIR", zdotdir.path());
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());

        for rc in &rcs {
            let new_rc = fs::read_to_string(rc).unwrap();
            // It's not ideal, but it's OK, if we leave whitespace.
            assert_eq!(new_rc, FAKE_RC);
        }
    }

    // In the default case we want to write $HOME/.cargo/bin as the path,
    // not the full path.
    #[tokio::test]
    async fn when_cargo_home_is_the_default_write_path_specially() {
        let cx = CliTestContext::new(Scenario::Empty).await;
        // Override the test harness so that cargo home looks like
        // $HOME/.cargo by removing CARGO_HOME from the environment,
        // otherwise the literal path will be written to the file.

        let profile = cx.config.homedir.join(".profile");
        raw::write_file(&profile, FAKE_RC).unwrap();
        let mut cmd = cx.config.cmd("rustup-init", &INIT_NONE[1..]);
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());

        let new_profile = fs::read_to_string(&profile).unwrap();
        let expected = format!("{FAKE_RC}. \"$HOME/.cargo/env\"\n");
        assert_eq!(new_profile, expected);

        let mut cmd = cx.config.cmd("rustup", ["self", "uninstall", "-y"]);
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());

        let new_profile = fs::read_to_string(&profile).unwrap();
        assert_eq!(new_profile, FAKE_RC);
    }

    #[tokio::test]
    async fn install_doesnt_modify_path_if_passed_no_modify_path() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        let profile = cx.config.homedir.join(".profile");
        cx.config
            .expect_ok(&[
                "rustup-init",
                "-y",
                "--no-modify-path",
                "--default-toolchain",
                "none",
            ])
            .await;
        assert!(!profile.exists());
    }
}

#[cfg(windows)]
mod windows {
    use super::INIT_NONE;
    use rustup::test::{CliTestContext, Scenario};
    use rustup::test::{RegistryGuard, USER_PATH, get_path};

    use windows_registry::{HSTRING, Value};

    #[tokio::test]
    /// Smoke test for end-to-end code connectivity of the installer path mgmt on windows.
    async fn install_uninstall_affect_path() {
        let mut cx = CliTestContext::new(Scenario::Empty).await;
        let _guard = RegistryGuard::new(&USER_PATH).unwrap();
        let cfg_path = cx.config.cargodir.join("bin").display().to_string();
        let get_path_ = || {
            HSTRING::try_from(get_path().unwrap().unwrap())
                .unwrap()
                .to_string()
        };

        cx.config.expect_ok(&INIT_NONE).await;
        assert!(
            get_path_().contains(cfg_path.trim_matches('"')),
            "`{}` not in `{}`",
            cfg_path,
            get_path_()
        );

        cx.config
            .expect_ok(&["rustup", "self", "uninstall", "-y"])
            .await;
        assert!(!get_path_().contains(&cfg_path));
    }

    #[tokio::test]
    /// Smoke test for end-to-end code connectivity of the installer path mgmt on windows.
    async fn install_uninstall_affect_path_with_non_unicode() {
        use std::os::windows::ffi::OsStrExt;

        use windows_registry::{CURRENT_USER, Type};

        let mut cx = CliTestContext::new(Scenario::Empty).await;
        let _guard = RegistryGuard::new(&USER_PATH).unwrap();
        // Set up a non unicode PATH
        let mut reg_value = Value::from([
            0x00, 0xD8, // leading surrogate
            0x01, 0x01, // bogus trailing surrogate
            0x00, 0x00, // null
        ]);
        reg_value.set_ty(Type::ExpandString);
        CURRENT_USER
            .create("Environment")
            .unwrap()
            .set_value("PATH", &reg_value)
            .unwrap();

        // compute expected path after installation
        let mut expected = Value::from(
            cx.config
                .cargodir
                .join("bin")
                .as_os_str()
                .encode_wide()
                .flat_map(|v| vec![v as u8, (v >> 8) as u8])
                .chain(vec![b';', 0])
                .chain(reg_value.iter().copied())
                .collect::<Vec<u8>>()
                .as_slice(),
        );
        expected.set_ty(Type::ExpandString);

        cx.config.expect_ok(&INIT_NONE).await;
        assert_eq!(get_path().unwrap().unwrap(), expected);

        cx.config
            .expect_ok(&["rustup", "self", "uninstall", "-y"])
            .await;
        assert_eq!(get_path().unwrap().unwrap(), reg_value);
    }
}
