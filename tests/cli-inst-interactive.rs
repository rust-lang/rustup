//! Tests of the interactive console installer

pub mod mock;

use crate::mock::clitools::{
    self, expect_ok, expect_stderr_ok, expect_stdout_ok, set_current_dist_date, this_host_triple,
    Config, SanitizedOutput, Scenario,
};
use crate::mock::{get_path, restore_path};
use lazy_static::lazy_static;
use rustup::utils::raw;
use std::fs;
use std::io::Write;
use std::process::Stdio;
use std::sync::Mutex;

macro_rules! for_host {
    ($s: expr) => {
        &format!($s, this_host_triple())
    };
}

pub fn setup_(complex: bool, f: &dyn Fn(&Config)) {
    let scenario = if complex {
        Scenario::UnavailableRls
    } else {
        Scenario::SimpleV2
    };
    clitools::setup(scenario, &|config| {
        // Lock protects environment variables
        lazy_static! {
            static ref LOCK: Mutex<()> = Mutex::new(());
        }
        let _g = LOCK.lock();

        // An windows these tests mess with the user's PATH. Save
        // and restore them here to keep from trashing things.
        let saved_path = get_path();
        let _g = scopeguard::guard(saved_path, restore_path);

        f(config);
    });
}

pub fn setup(f: &dyn Fn(&Config)) {
    setup_(false, f)
}

fn run_input(config: &Config, args: &[&str], input: &str) -> SanitizedOutput {
    run_input_with_env(config, args, input, &[])
}

fn run_input_with_env(
    config: &Config,
    args: &[&str],
    input: &str,
    env: &[(&str, &str)],
) -> SanitizedOutput {
    let mut cmd = clitools::cmd(config, args[0], &args[1..]);
    clitools::env(config, &mut cmd);

    for (key, value) in env.iter() {
        cmd.env(key, value);
    }

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
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
// 'pre-install' message and before the 'post-install' message.
#[test]
fn blank_lines_around_stderr_log_output_install() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"], "\n\n");

        // During an interactive session, after "Press the Enter
        // key..."  the UI emits a blank line, then there is a blank
        // line that comes from the user pressing enter, then log
        // output on stderr, then an explicit blank line on stdout
        // before printing $toolchain installed
        assert!(out.stdout.contains(
            r"
3) Cancel installation
>

  stable installed - 1.1.0 (hash-stable-1.1.0)


Rust is installed now. Great!
"
        ));
    });
}

#[test]
fn blank_lines_around_stderr_log_output_update() {
    setup(&|config| {
        run_input(config, &["rustup-init"], "\n\n");
        let out = run_input(
            config,
            &["rustup-init", "--no-update-default-toolchain"],
            "\n\n",
        );
        println!("-- stdout --\n {}", out.stdout);
        println!("-- stderr --\n {}", out.stderr);

        assert!(out.stdout.contains(
            r"
3) Cancel installation
>


Rust is installed now. Great!
"
        ));
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
        assert!(out
            .stdout
            .contains("This path needs to be in your PATH environment variable"));

        if cfg!(unix) {
            assert!(!config.homedir.join(".profile").exists());
        }
    });
}

#[test]
fn with_no_toolchain() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init", "--default-toolchain=none"], "\n\n");
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "no active toolchain");
    });
}

#[test]
fn with_non_default_toolchain() {
    setup(&|config| {
        let out = run_input(
            config,
            &["rustup-init", "--default-toolchain=nightly"],
            "\n\n",
        );
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "nightly");
    });
}

#[test]
fn with_non_release_channel_non_default_toolchain() {
    setup(&|config| {
        let out = run_input(
            config,
            &["rustup-init", "--default-toolchain=nightly-2015-01-02"],
            "\n\n",
        );
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "nightly");
        expect_stdout_ok(config, &["rustup", "show"], "2015-01-02");
    });
}

#[test]
fn set_nightly_toolchain() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"], "2\n\nnightly\n\n\n\n\n");
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "nightly");
    });
}

#[test]
fn set_no_modify_path() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"], "2\n\n\n\nno\n\n\n");
        assert!(out.ok);

        if cfg!(unix) {
            assert!(!config.homedir.join(".profile").exists());
        }
    });
}

#[test]
fn set_nightly_toolchain_and_unset() {
    setup(&|config| {
        let out = run_input(
            config,
            &["rustup-init"],
            "2\n\nnightly\n\n\n2\n\nbeta\n\n\n\n\n",
        );
        assert!(out.ok);

        expect_stdout_ok(config, &["rustup", "show"], "beta");
    });
}

#[test]
fn user_says_nope_after_advanced_install() {
    setup(&|config| {
        let out = run_input(config, &["rustup-init"], "2\n\n\n\n\nn\n\n\n");
        assert!(out.ok);
        assert!(!config.cargodir.join("bin").exists());
    });
}

#[test]
fn install_with_components() {
    fn go(comp_args: &[&str]) {
        let mut args = vec!["rustup-init", "-y"];
        args.extend_from_slice(comp_args);

        setup(&|config| {
            expect_ok(config, &args);
            expect_stdout_ok(
                config,
                &["rustup", "component", "list"],
                "rust-src (installed)",
            );
            expect_stdout_ok(
                config,
                &["rustup", "component", "list"],
                &format!("rust-analysis-{} (installed)", this_host_triple()),
            );
        })
    }

    go(&["-c", "rust-src", "-c", "rust-analysis"]);
    go(&["-c", "rust-src,rust-analysis"]);
}

#[test]
fn install_forces_and_skips_rls() {
    setup_(true, &|config| {
        set_current_dist_date(config, "2015-01-01");

        let out = run_input(
            config,
            &[
                "rustup-init",
                "--profile",
                "complete",
                "--default-toolchain",
                "nightly",
            ],
            "\n\n",
        );
        assert!(out.ok);
        assert!(out
            .stderr
            .contains("warning: Force-skipping unavailable component"));
    });
}

#[test]
fn test_warn_if_complete_profile_is_used() {
    setup(&|config| {
        expect_stderr_ok(
            config,
            &["rustup-init", "-y", "--profile", "complete"],
            "warning: downloading with complete profile",
        );
    });
}

fn create_rustup_sh_metadata(config: &Config) {
    let rustup_dir = config.homedir.join(".rustup");
    fs::create_dir_all(&rustup_dir).unwrap();
    let version_file = rustup_dir.join("rustup-version");
    raw::write_file(&version_file, "").unwrap();
}

#[test]
fn test_prompt_fail_if_rustup_sh_already_installed_reply_nothing() {
    setup(&|config| {
        create_rustup_sh_metadata(&config);
        let out = run_input(config, &["rustup-init"], "\n");
        assert!(!out.ok);
        assert!(out
            .stderr
            .contains("warning: it looks like you have existing rustup.sh metadata"));
        assert!(out
            .stderr
            .contains("error: cannot install while rustup.sh is installed"));
        assert!(out.stdout.contains("Continue? (y/N)"));
    })
}

#[test]
fn test_prompt_fail_if_rustup_sh_already_installed_reply_no() {
    setup(&|config| {
        create_rustup_sh_metadata(&config);
        let out = run_input(config, &["rustup-init"], "no\n");
        assert!(!out.ok);
        assert!(out
            .stderr
            .contains("warning: it looks like you have existing rustup.sh metadata"));
        assert!(out
            .stderr
            .contains("error: cannot install while rustup.sh is installed"));
        assert!(out.stdout.contains("Continue? (y/N)"));
    })
}

#[test]
fn test_prompt_succeed_if_rustup_sh_already_installed_reply_yes() {
    setup(&|config| {
        create_rustup_sh_metadata(&config);
        let out = run_input(config, &["rustup-init"], "yes\n\n\n");
        assert!(out.ok);
        assert!(out
            .stderr
            .contains("warning: it looks like you have existing rustup.sh metadata"));
        assert!(out
            .stderr
            .contains("error: cannot install while rustup.sh is installed"));
        assert!(out.stdout.contains("Continue? (y/N)"));
        assert!(!out.stdout.contains(
            "warning: continuing (because the -y flag is set and the error is ignorable)"
        ))
    })
}

#[test]
fn test_warn_succeed_if_rustup_sh_already_installed_y_flag() {
    setup(&|config| {
        create_rustup_sh_metadata(&config);
        let out = run_input(config, &["rustup-init", "-y"], "");
        assert!(out.ok);
        assert!(out
            .stderr
            .contains("warning: it looks like you have existing rustup.sh metadata"));
        assert!(out
            .stderr
            .contains("error: cannot install while rustup.sh is installed"));
        assert!(out.stderr.contains(
            "warning: continuing (because the -y flag is set and the error is ignorable)"
        ));
        assert!(!out.stdout.contains("Continue? (y/N)"));
    })
}

#[test]
fn test_succeed_if_rustup_sh_already_installed_env_var_set() {
    setup(&|config| {
        create_rustup_sh_metadata(&config);
        let out = run_input_with_env(
            config,
            &["rustup-init", "-y"],
            "",
            &[("RUSTUP_INIT_SKIP_EXISTENCE_CHECKS", "yes")],
        );
        assert!(out.ok);
        assert!(!out
            .stderr
            .contains("warning: it looks like you have existing rustup.sh metadata"));
        assert!(!out
            .stderr
            .contains("error: cannot install while rustup.sh is installed"));
        assert!(!out.stderr.contains(
            "warning: continuing (because the -y flag is set and the error is ignorable)"
        ));
        assert!(!out.stdout.contains("Continue? (y/N)"));
    })
}

#[test]
fn installing_when_already_installed_updates_toolchain() {
    setup(&|config| {
        run_input(config, &["rustup-init"], "\n\n");
        let out = run_input(config, &["rustup-init"], "\n\n");
        println!("stdout:\n{}\n...\n", out.stdout);
        assert!(out
            .stdout
            .contains(for_host!("stable-{} unchanged - 1.1.0 (hash-stable-1.1.0)")));
    })
}
