use std::fs::create_dir_all;
use std::path::Path;

use rustup::test::{CliTestContext, Scenario};
use snapbox::Data;
use snapbox::cmd::{Command, cargo_bin};

#[track_caller]
fn test_help(name: &str, args: &[&str]) {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("home-ro");
    create_dir_all(&home).unwrap();

    let rustup_init = cargo_bin!("rustup-init");
    Command::new(rustup_init)
        .env("RUSTUP_FORCE_ARG0", "rustup")
        .env("RUSTUP_TERM_COLOR", "always")
        // once installed rustup asserts the presence of ~/.rustup/settings.toml if
        // Config is instantiated.
        .env("HOME", &home)
        .args(args)
        .assert()
        .success()
        .stdout_eq(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stdout.term.svg")),
            None,
        ))
        .stderr_eq("");
}

#[track_caller]
fn test_error(name: &str, args: &[&str]) {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("home-ro");
    create_dir_all(&home).unwrap();

    let rustup_init = cargo_bin!("rustup-init");
    Command::new(rustup_init)
        .env("RUSTUP_FORCE_ARG0", "rustup")
        .env("RUSTUP_TERM_COLOR", "always")
        // once installed rustup asserts the presence of ~/.rustup/settings.toml if
        // Config is instantiated.
        .env("HOME", &home)
        .args(args)
        .assert()
        .failure()
        .stdout_eq("")
        .stderr_eq(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stderr.term.svg")),
            None,
        ));
}

#[test]
#[cfg(not(windows))] // On windows, we don't have the `man` command
fn rustup_help_cmd() {
    test_help("rustup_help_cmd", &["help"]);
}

#[test]
#[cfg(not(windows))] // On windows, we don't have the `man` command
fn rustup_help_flag() {
    test_help("rustup_help_flag", &["--help"]);
}

#[test]
#[cfg(not(windows))] // On windows, we don't have the `man` command
fn rustup_only_options() {
    test_error("rustup_only_options", &["-q"]);
}

#[test]
fn rustup_check_cmd_help_flag() {
    test_help("rustup_check_cmd_help_flag", &["check", "--help"]);
}

#[test]
fn rustup_completions_cmd_help_flag() {
    test_help(
        "rustup_completions_cmd_help_flag",
        &["completions", "--help"],
    );
}

#[test]
fn rustup_component_cmd_help_flag() {
    test_help("rustup_component_cmd_help_flag", &["component", "--help"]);
}

#[test]
fn rustup_component_cmd_add_cmd_help_flag() {
    test_help(
        "rustup_component_cmd_add_cmd_help_flag",
        &["component", "add", "--help"],
    );
}

#[test]
fn rustup_component_cmd_list_cmd_help_flag() {
    test_help(
        "rustup_component_cmd_list_cmd_help_flag",
        &["component", "list", "--help"],
    );
}

#[test]
fn rustup_component_cmd_remove_cmd_help_flag() {
    test_help(
        "rustup_component_cmd_remove_cmd_help_flag",
        &["component", "remove", "--help"],
    );
}

#[tokio::test]
async fn rustup_default() {
    let name = "rustup_default";
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            ["rustup", "default", "nightly"],
            [("RUSTUP_TERM_COLOR", "always")],
        )
        .await
        .with_stdout(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stdout.term.svg")),
            None,
        ))
        .with_stderr(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stderr.term.svg")),
            None,
        ))
        .is_ok();
}

#[test]
fn rustup_default_cmd_help_flag() {
    test_help("rustup_default_cmd_help_flag", &["default", "--help"]);
}

#[test]
fn rustup_doc_cmd_help_flag() {
    test_help("rustup_doc_cmd_help_flag", &["doc", "--help"]);
}

#[test]
#[cfg(not(target_os = "windows"))] // On windows, we don't have man command
fn rustup_man_cmd_help_flag() {
    test_help("rustup_man_cmd_help_flag", &["man", "--help"]);
}

#[test]
fn rustup_override_cmd_help_flag() {
    test_help("rustup_override_cmd_help_flag", &["override", "--help"]);
}

#[test]
fn rustup_override_cmd_add_cmd_help_flag() {
    test_help(
        "rustup_override_cmd_add_cmd_help_flag",
        &["override", "add", "--help"],
    );
}

#[test]
fn rustup_override_cmd_list_cmd_help_flag() {
    test_help(
        "rustup_override_cmd_list_cmd_help_flag",
        &["override", "list", "--help"],
    );
}

#[test]
fn rustup_override_cmd_remove_cmd_help_flag() {
    test_help(
        "rustup_override_cmd_remove_cmd_help_flag",
        &["override", "remove", "--help"],
    );
}

#[test]
fn rustup_override_cmd_set_cmd_help_flag() {
    test_help(
        "rustup_override_cmd_set_cmd_help_flag",
        &["override", "set", "--help"],
    );
}

#[test]
fn rustup_override_cmd_unset_cmd_help_flag() {
    test_help(
        "rustup_override_cmd_unset_cmd_help_flag",
        &["override", "unset", "--help"],
    );
}

#[test]
fn rustup_run_cmd_help_flag() {
    test_help("rustup_run_cmd_help_flag", &["run", "--help"]);
}

#[test]
fn rustup_self_cmd_help_flag() {
    test_help("rustup_self_cmd_help_flag", &["self", "--help"]);
}

#[test]
fn rustup_self_cmd_uninstall_cmd_help_flag() {
    test_help(
        "rustup_self_cmd_uninstall_cmd_help_flag",
        &["self", "uninstall", "--help"],
    );
}

#[test]
fn rustup_self_cmd_update_cmd_help_flag() {
    test_help(
        "rustup_self_cmd_update_cmd_help_flag",
        &["self", "update", "--help"],
    );
}

#[test]
fn rustup_self_cmd_upgrade_data_cmd_help_flag() {
    test_help(
        "rustup_self_cmd_upgrade_data_cmd_help_flag",
        &["self", "upgrade-data", "--help"],
    );
}

#[test]
fn rustup_set_cmd_help_flag() {
    test_help("rustup_set_cmd_help_flag", &["set", "--help"]);
}

#[test]
fn rustup_set_cmd_auto_install_cmd_help_flag() {
    test_help(
        "rustup_set_cmd_auto_install_cmd_help_flag",
        &["set", "auto-install", "--help"],
    );
}

#[test]
fn rustup_set_cmd_auto_self_update_cmd_help_flag() {
    test_help(
        "rustup_set_cmd_auto_self_update_cmd_help_flag",
        &["set", "auto-self-update", "--help"],
    );
}

#[test]
fn rustup_set_cmd_default_host_cmd_help_flag() {
    test_help(
        "rustup_set_cmd_default_host_cmd_help_flag",
        &["set", "default-host", "--help"],
    );
}

#[test]
fn rustup_set_cmd_profile_cmd_help_flag() {
    test_help(
        "rustup_set_cmd_profile_cmd_help_flag",
        &["set", "profile", "--help"],
    );
}

#[tokio::test]
async fn rustup_show_toolchain() {
    let name = "rustup_show_toolchain";
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect_with_env(["rustup", "show"], [("RUSTUP_TERM_COLOR", "always")])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stdout.term.svg")),
            None,
        ))
        .with_stderr("")
        .is_ok();
}

#[test]
fn rustup_show_cmd_help_flag() {
    test_help("rustup_show_cmd_help_flag", &["show", "--help"]);
}

#[test]
fn rustup_show_cmd_active_toolchain_cmd_help_flag() {
    test_help(
        "rustup_show_cmd_active_toolchain_cmd_help_flag",
        &["show", "active-toolchain", "--help"],
    );
}

#[test]
fn rustup_show_cmd_home_cmd_help_flag() {
    test_help(
        "rustup_show_cmd_home_cmd_help_flag",
        &["show", "home", "--help"],
    );
}

#[test]
fn rustup_show_cmd_profile_cmd_help_flag() {
    test_help(
        "rustup_show_cmd_profile_cmd_help_flag",
        &["show", "profile", "--help"],
    );
}

#[test]
fn rustup_target_cmd_help_flag() {
    test_help("rustup_target_cmd_help_flag", &["target", "--help"]);
}

#[test]
fn rustup_target_cmd_add_cmd_help_flag() {
    test_help(
        "rustup_target_cmd_add_cmd_help_flag",
        &["target", "add", "--help"],
    );
}

#[test]
fn rustup_target_cmd_list_cmd_help_flag() {
    test_help(
        "rustup_target_cmd_list_cmd_help_flag",
        &["target", "list", "--help"],
    );
}

#[test]
fn rustup_target_cmd_remove_cmd_help_flag() {
    test_help(
        "rustup_target_cmd_remove_cmd_help_flag",
        &["target", "remove", "--help"],
    );
}

#[test]
fn rustup_toolchain_cmd_help_flag() {
    test_help("rustup_toolchain_cmd_help_flag", &["toolchain", "--help"]);
}

#[test]
fn rustup_toolchain_cmd_install_cmd_help_flag() {
    test_help(
        "rustup_toolchain_cmd_install_cmd_help_flag",
        &["toolchain", "install", "--help"],
    );
}

#[test]
fn rustup_toolchain_cmd_link_cmd_help_flag() {
    test_help(
        "rustup_toolchain_cmd_link_cmd_help_flag",
        &["toolchain", "link", "--help"],
    );
}

#[test]
fn rustup_toolchain_cmd_list_cmd_help_flag() {
    test_help(
        "rustup_toolchain_cmd_list_cmd_help_flag",
        &["toolchain", "list", "--help"],
    );
}

#[test]
fn rustup_toolchain_cmd_uninstall_cmd_help_flag() {
    test_help(
        "rustup_toolchain_cmd_uninstall_cmd_help_flag",
        &["toolchain", "uninstall", "--help"],
    );
}

#[test]
fn rustup_up_cmd_help_flag() {
    test_help("rustup_up_cmd_help_flag", &["up", "--help"]);
}

#[tokio::test]
async fn rustup_update_no_change() {
    let name = "rustup_update_no_change";
    let cx = CliTestContext::new(Scenario::ArchivesV2_2015_01_01).await;
    cx.config
        .expect(["rustup", "update", "stable"])
        .await
        .is_ok();
    cx.config
        .expect_with_env(["rustup", "update"], [("RUSTUP_TERM_COLOR", "always")])
        .await
        .with_stdout(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stdout.term.svg")),
            None,
        ))
        .with_stderr(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stderr.term.svg")),
            None,
        ))
        .is_ok();
}

#[tokio::test]
async fn rustup_update_updated() {
    let name = "rustup_update_updated";
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect(["rustup", "toolchain", "add", "stable"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect_with_env(["rustup", "update"], [("RUSTUP_TERM_COLOR", "always")])
        .await
        .with_stdout(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stdout.term.svg")),
            None,
        ))
        .with_stderr(Data::read_from(
            Path::new(&format!("tests/suite/cli_rustup_ui/{name}.stderr.term.svg")),
            None,
        ))
        .is_ok();
}

#[test]
fn rustup_update_cmd_help_flag() {
    test_help("rustup_update_cmd_help_flag", &["update", "--help"]);
}

#[test]
fn rustup_upgrade_cmd_help_flag() {
    test_help("rustup_upgrade_cmd_help_flag", &["upgrade", "--help"]);
}

#[test]
fn rustup_which_cmd_help_flag() {
    test_help("rustup_which_cmd_help_flag", &["which", "--help"]);
}

#[test]
fn rustup_unknown_arg() {
    test_error("rustup_unknown_arg", &["random"]);
}
