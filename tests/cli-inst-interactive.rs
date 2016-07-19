//! Tests of the interactive console installer

extern crate rustup_mock;
extern crate rustup_utils;
#[macro_use]
extern crate lazy_static;
extern crate scopeguard;

use std::sync::Mutex;
use std::process::Stdio;
use std::io::Write;
use rustup_mock::clitools::{self, Config, Scenario,
                            SanitizedOutput,
                            expect_stdout_ok};
use rustup_mock::{get_path, restore_path};

pub fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, &|config| {
        // Lock protects environment variables
        lazy_static! {
            static ref LOCK: Mutex<()> = Mutex::new(());
        }
        let _g = LOCK.lock();

        // An windows these tests mess with the user's PATH. Save
        // and restore them here to keep from trashing things.
        let saved_path = get_path();
        let _g = scopeguard::guard(saved_path, |p| restore_path(p));

        f(config);
    });
}

fn run_input(config: &Config, args: &[&str], input: &str) -> SanitizedOutput {
    let mut cmd = clitools::cmd(config, &args[0], &args[1..]);
    clitools::env(config, &mut cmd);

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();

    child.stdin.as_mut().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();

    SanitizedOutput {
        ok: out.status.success(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

#[test]
fn smoke_test() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"], "\n\n");
        assert!(out.ok);
    });
}

#[test]
fn update() {
    setup(&|config| {
        run_input(config, &["rustup-init"], "\n\n");
        let out = run_input(config, &["rustup-init"], "\n\n");
        assert!(out.ok, "stdout:\n{}\nstderr:\n{}", out.stdout, out.stderr);
    });
}

// Testing that the right number of blank lines are printed after the
// 'pre-install' message and before the 'post-install' messag.
#[test]
fn blank_lines_around_stderr_log_output_install() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"], "\n\n");

        // During an interactive session, after "Press the Enter
        // key..."  the UI emits a blank line, then there is a blank
        // line that comes from the user pressing enter, then log
        // output on stderr, then an explicit blank line on stdout
        // before printing $toolchain installed
        assert!(out.stdout.contains(r"
3) Cancel installation


  stable installed - 1.1.0 (hash-s-2)


Rust is installed now. Great!
"));
    });
}

#[test]
fn blank_lines_around_stderr_log_output_update() {
    setup(&|config| {
        run_input(config, &["rustup-init"], "\n\n");
        let out = run_input(config, &["rustup-init"], "\n\n");

        assert!(out.stdout.contains(r"
3) Cancel installation



Rust is installed now. Great!
"));
    });
}

#[test]
fn user_says_nope() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"], "n\n\n");
        assert!(out.ok);
        assert!(!config.cargodir.join("bin").exists());
    });
}

#[test]
fn with_no_modify_path() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init", "--no-modify-path"], "\n\n");
        assert!(out.ok);
        assert!(out.stdout.contains("This path needs to be in your PATH environment variable"));

        if cfg!(unix) {
            assert!(!config.homedir.join(".profile").exists());
        }
    });
}

#[test]
fn with_non_default_toolchain() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init", "--default-toolchain=nightly"], "\n\n");
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "nightly");
    });
}

#[test]
fn with_non_release_channel_non_default_toolchain() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init", "--default-toolchain=nightly-2015-01-02"],
                            "\n\n");
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "nightly");
        expect_stdout_ok(config, &["rustup", "show"], "2015-01-02");
    });
}

#[test]
fn set_nightly_toolchain() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"],
                            "2\n\nnightly\n\n\n\n");
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "nightly");
    });
}

#[test]
fn set_no_modify_path() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"],
                            "2\n\n\nno\n\n\n");
        assert!(out.ok);

        if cfg!(unix) {
            assert!(!config.homedir.join(".profile").exists());
        }
    });
}

#[test]
fn set_nightly_toolchain_and_unset() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"],
                            "2\n\nnightly\n\n2\n\nbeta\n\n\n\n");
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "beta");
    });
}

#[test]
fn user_says_nope_after_advanced_install() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"],
                            "2\n\n\n\nn\n\n");
        assert!(out.ok);
        assert!(!config.cargodir.join("bin").exists());
    });
}
