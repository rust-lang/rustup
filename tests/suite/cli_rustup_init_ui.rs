use std::fs::create_dir_all;
use std::path::Path;

use snapbox::Data;
use snapbox::cmd::{Command, cargo_bin};

#[track_caller]
fn test_help(name: &str, args: &[&str]) {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("home-ro");
    create_dir_all(&home).unwrap();

    let rustup_init = cargo_bin!("rustup-init");
    Command::new(rustup_init)
        .env("RUSTUP_TERM_COLOR", "always")
        // once installed rustup asserts the presence of ~/.rustup/settings.toml if
        // Config is instantiated.
        .env("HOME", &home)
        .args(args)
        .assert()
        .success()
        .stdout_eq(Data::read_from(
            Path::new(&format!(
                "tests/suite/cli_rustup_init_ui/{name}.stdout.term.svg"
            )),
            None,
        ))
        .stderr_eq("");
}

#[track_caller]
#[cfg(not(target_os = "windows"))] // On windows, we don't use rustup-init.sh
fn test_sh_help(name: &str, args: &[&str]) {
    let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("home-ro");
    create_dir_all(&home).unwrap();

    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rustup_init_sh = project_root.join("rustup-init.sh");
    Command::new(rustup_init_sh)
        .env("RUSTUP_TERM_COLOR", "always")
        // once installed rustup asserts the presence of ~/.rustup/settings.toml if
        // Config is instantiated.
        .env("HOME", &home)
        .args(args)
        .assert()
        .success()
        .stdout_eq(Data::read_from(
            Path::new(&format!(
                "tests/suite/cli_rustup_init_ui/{name}.stdout.term.svg"
            )),
            None,
        ))
        .stderr_eq("");
}

#[test]
fn rustup_init_help_flag() {
    test_help("rustup_init_help_flag", &["--help"]);
}

#[test]
#[cfg(not(target_os = "windows"))] // On windows, we don't use rustup-init.sh
fn rustup_init_sh_help_flag() {
    test_sh_help("rustup_init_sh_help_flag", &["--help"]);
}
