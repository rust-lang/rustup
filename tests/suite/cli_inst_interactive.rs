//! Tests of the interactive console installer

use std::env::consts::EXE_SUFFIX;
use std::io::Write;
use std::process::Stdio;

use rustup::for_host;
use rustup::test::mock::clitools::CliTestContext;
use rustup::test::{
    mock::clitools::{self, set_current_dist_date, Config, SanitizedOutput, Scenario},
    this_host_triple, with_saved_path,
};
use rustup::utils::raw;

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
fn update() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    with_saved_path(&mut || {
        run_input(&cx.config, &["rustup-init"], "\n\n");
        let out = run_input(&cx.config, &["rustup-init"], "\n\n");
        assert!(out.ok, "stdout:\n{}\nstderr:\n{}", out.stdout, out.stderr);
    })
}

// Testing that the right number of blank lines are printed after the
// 'pre-install' message and before the 'post-install' message - overall smoke
// test for the install case.
#[test]
fn smoke_case_install_no_modify_path() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(&cx.config, &["rustup-init", "--no-modify-path"], "\n\n");
    assert!(out.ok);
    // During an interactive session, after "Press the Enter
    // key..."  the UI emits a blank line, then there is a blank
    // line that comes from the user pressing enter, then log
    // output on stderr, then an explicit blank line on stdout
    // before printing $toolchain installed
    assert!(
        out.stdout.contains(for_host!(
            r"
This path needs to be in your PATH environment variable,
but will not be added automatically.

You can uninstall at any time with rustup self uninstall and
these changes will be reverted.

Current installation options:


   default host triple: {0}
     default toolchain: stable (default)
               profile: default
  modify PATH variable: no

1) Proceed with standard installation (default - just press enter)
2) Customize installation
3) Cancel installation
>

  stable-{0} installed - 1.1.0 (hash-stable-1.1.0)


Rust is installed now. Great!

"
        )),
        "pattern not found in \"\"\"{}\"\"\"",
        out.stdout
    );
    if cfg!(unix) {
        assert!(!cx.config.homedir.join(".profile").exists());
        assert!(cx.config.cargodir.join("env").exists());
    }
}

#[test]
fn smoke_case_install_with_path_install() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    with_saved_path(&mut || {
        let out = run_input(&cx.config, &["rustup-init"], "\n\n");
        assert!(out.ok);
        assert!(!out
            .stdout
            .contains("This path needs to be in your PATH environment variable"));
    });
}

#[test]
fn blank_lines_around_stderr_log_output_update() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
    let out = run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-update-default-toolchain",
            "--no-modify-path",
        ],
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
}

#[test]
fn installer_shows_default_host_triple() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(&cx.config, &["rustup-init", "--no-modify-path"], "2\n");

    println!("-- stdout --\n {}", out.stdout);
    println!("-- stderr --\n {}", out.stderr);
    assert!(out.stdout.contains(for_host!(
        r"
Default host triple? [{0}]
"
    )));
}

#[test]
fn installer_shows_default_toolchain_as_stable() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(&cx.config, &["rustup-init", "--no-modify-path"], "2\n\n");

    println!("-- stdout --\n {}", out.stdout);
    println!("-- stderr --\n {}", out.stderr);
    assert!(out.stdout.contains(
        r"
Default toolchain? (stable/beta/nightly/none) [stable]
"
    ));
}

#[test]
fn installer_shows_default_toolchain_when_set_in_args() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-modify-path",
            "--default-toolchain=nightly",
        ],
        "2\n\n",
    );

    println!("-- stdout --\n {}", out.stdout);
    println!("-- stderr --\n {}", out.stderr);
    assert!(out.stdout.contains(
        r"
Default toolchain? (stable/beta/nightly/none) [nightly]
"
    ));
}

#[test]
fn installer_shows_default_profile() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(&cx.config, &["rustup-init", "--no-modify-path"], "2\n\n\n");

    println!("-- stdout --\n {}", out.stdout);
    println!("-- stderr --\n {}", out.stderr);
    assert!(out.stdout.contains(
        r"
Profile (which tools and data to install)? (minimal/default/complete) [default]
"
    ));
}

#[test]
fn installer_shows_default_profile_when_set_in_args() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path", "--profile=minimal"],
        "2\n\n\n",
    );

    println!("-- stdout --\n {}", out.stdout);
    println!("-- stderr --\n {}", out.stderr);
    assert!(out.stdout.contains(
        r"
Profile (which tools and data to install)? (minimal/default/complete) [minimal]
"
    ));
}

#[test]
fn installer_shows_default_for_modify_path() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(&cx.config, &["rustup-init"], "2\n\n\n\n");

    println!("-- stdout --\n {}", out.stdout);
    println!("-- stderr --\n {}", out.stderr);
    assert!(out.stdout.contains(
        r"
Modify PATH variable? (Y/n)
"
    ));
}

#[test]
fn installer_shows_default_for_modify_path_when_set_with_args() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\n\n\n",
    );

    println!("-- stdout --\n {}", out.stdout);
    println!("-- stderr --\n {}", out.stderr);
    assert!(out.stdout.contains(
        r"
Modify PATH variable? (y/N)
"
    ));
}

#[test]
fn user_says_nope() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(&cx.config, &["rustup-init", "--no-modify-path"], "n\n\n");
    assert!(out.ok);
    assert!(!cx.config.cargodir.join("bin").exists());
}

#[test]
fn with_no_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-modify-path",
            "--default-toolchain=none",
        ],
        "\n\n",
    );
    assert!(out.ok);

    cx.config
        .expect_stdout_ok(&["rustup", "show"], "no active toolchain");
}

#[test]
fn with_non_default_toolchain_still_prompts() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-modify-path",
            "--default-toolchain=nightly",
        ],
        "\n\n",
    );
    assert!(out.ok);

    cx.config.expect_stdout_ok(&["rustup", "show"], "nightly");
}

#[test]
fn with_non_release_channel_non_default_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-modify-path",
            "--default-toolchain=nightly-2015-01-02",
        ],
        "\n\n",
    );
    assert!(out.ok);

    cx.config.expect_stdout_ok(&["rustup", "show"], "nightly");
    cx.config
        .expect_stdout_ok(&["rustup", "show"], "2015-01-02");
}

#[test]
fn set_nightly_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\nnightly\n\n\n\n\n",
    );
    assert!(out.ok);

    cx.config.expect_stdout_ok(&["rustup", "show"], "nightly");
}

#[test]
fn set_no_modify_path() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\n\n\nno\n\n\n",
    );
    assert!(out.ok);

    if cfg!(unix) {
        assert!(!cx.config.homedir.join(".profile").exists());
    }
}

#[test]
fn set_nightly_toolchain_and_unset() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\nnightly\n\n\n2\n\nbeta\n\n\n\n\n",
    );
    println!("{:?}", out.stderr);
    println!("{:?}", out.stdout);
    assert!(out.ok);

    cx.config.expect_stdout_ok(&["rustup", "show"], "beta");
}

#[test]
fn user_says_nope_after_advanced_install() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\n\n\n\nn\n\n\n",
    );
    assert!(out.ok);
    assert!(!cx.config.cargodir.join("bin").exists());
}

#[test]
fn install_with_components() {
    fn go(comp_args: &[&str]) {
        let mut args = vec!["rustup-init", "-y", "--no-modify-path"];
        args.extend_from_slice(comp_args);

        let mut cx = CliTestContext::from(Scenario::SimpleV2);
        cx.config.expect_ok(&args);
        cx.config
            .expect_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)");
        cx.config.expect_stdout_ok(
            &["rustup", "component", "list"],
            &format!("rust-analysis-{} (installed)", this_host_triple()),
        );
    }

    go(&["-c", "rust-src", "-c", "rust-analysis"]);
    go(&["-c", "rust-src,rust-analysis"]);
}

#[test]
fn install_forces_and_skips_rls() {
    let cx = CliTestContext::from(Scenario::UnavailableRls);
    set_current_dist_date(&cx.config, "2015-01-01");

    let out = run_input(
        &cx.config,
        &[
            "rustup-init",
            "--profile",
            "complete",
            "--default-toolchain",
            "nightly",
            "--no-modify-path",
        ],
        "\n\n",
    );
    assert!(out.ok);
    assert!(out
        .stderr
        .contains("warn: Force-skipping unavailable component"));
}

#[test]
fn test_warn_if_complete_profile_is_used() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_stderr_ok(
        &[
            "rustup-init",
            "-y",
            "--profile",
            "complete",
            "--no-modify-path",
        ],
        "warn: downloading with complete profile",
    );
}

#[test]
fn test_prompt_fail_if_rustup_sh_already_installed_reply_nothing() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.create_rustup_sh_metadata();
    let out = run_input(&cx.config, &["rustup-init", "--no-modify-path"], "\n");
    assert!(!out.ok);
    assert!(out
        .stderr
        .contains("warn: it looks like you have existing rustup.sh metadata"));
    assert!(out
        .stderr
        .contains("error: cannot install while rustup.sh is installed"));
    assert!(out.stdout.contains("Continue? (y/N)"));
}

#[test]
fn test_prompt_fail_if_rustup_sh_already_installed_reply_no() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.create_rustup_sh_metadata();
    let out = run_input(&cx.config, &["rustup-init", "--no-modify-path"], "no\n");
    assert!(!out.ok);
    assert!(out
        .stderr
        .contains("warn: it looks like you have existing rustup.sh metadata"));
    assert!(out
        .stderr
        .contains("error: cannot install while rustup.sh is installed"));
    assert!(out.stdout.contains("Continue? (y/N)"));
}

#[test]
fn test_prompt_succeed_if_rustup_sh_already_installed_reply_yes() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.create_rustup_sh_metadata();
    let out = run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "yes\n\n\n",
    );
    assert!(out
        .stderr
        .contains("warn: it looks like you have existing rustup.sh metadata"));
    assert!(out
        .stderr
        .contains("error: cannot install while rustup.sh is installed"));
    assert!(out.stdout.contains("Continue? (y/N)"));
    assert!(!out
        .stdout
        .contains("warn: continuing (because the -y flag is set and the error is ignorable)"));
    assert!(out.ok);
}

#[test]
fn installing_when_already_installed_updates_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
    let out = run_input(&cx.config, &["rustup-init", "--no-modify-path"], "\n\n");
    println!("stdout:\n{}\n...\n", out.stdout);
    assert!(out
        .stdout
        .contains(for_host!("stable-{} unchanged - 1.1.0 (hash-stable-1.1.0)")));
}

#[test]
fn install_stops_if_rustc_exists() {
    let temp_dir = tempfile::Builder::new()
        .prefix("fakebin")
        .tempdir()
        .unwrap();
    // Create fake executable
    let fake_exe = temp_dir.path().join(format!("{}{}", "rustc", EXE_SUFFIX));
    raw::append_file(&fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = cx.config.run(
        "rustup-init",
        ["--no-modify-path"],
        &[
            ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
            ("PATH", temp_dir_path),
        ],
    );
    assert!(!out.ok);
    assert!(out
        .stderr
        .contains("it looks like you have an existing installation of Rust at:"));
    assert!(out
        .stderr
        .contains("If you are sure that you want both rustup and your already installed Rust"));
}

#[test]
fn install_stops_if_cargo_exists() {
    let temp_dir = tempfile::Builder::new()
        .prefix("fakebin")
        .tempdir()
        .unwrap();
    // Create fake executable
    let fake_exe = temp_dir.path().join(format!("{}{}", "cargo", EXE_SUFFIX));
    raw::append_file(&fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = cx.config.run(
        "rustup-init",
        ["--no-modify-path"],
        &[
            ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
            ("PATH", temp_dir_path),
        ],
    );
    assert!(!out.ok);
    assert!(out
        .stderr
        .contains("it looks like you have an existing installation of Rust at:"));
    assert!(out
        .stderr
        .contains("If you are sure that you want both rustup and your already installed Rust"));
}

#[test]
fn with_no_prompt_install_succeeds_if_rustc_exists() {
    let temp_dir = tempfile::Builder::new()
        .prefix("fakebin")
        .tempdir()
        .unwrap();
    // Create fake executable
    let fake_exe = temp_dir.path().join(format!("{}{}", "rustc", EXE_SUFFIX));
    raw::append_file(&fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let cx = CliTestContext::from(Scenario::SimpleV2);
    let out = cx.config.run(
        "rustup-init",
        ["-y", "--no-modify-path"],
        &[
            ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
            ("PATH", temp_dir_path),
        ],
    );
    assert!(out.ok);
}

// Issue 2547
#[test]
fn install_non_installable_toolchain() {
    let cx = CliTestContext::from(Scenario::Unavailable);
    cx.config.expect_err(
        &[
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "nightly",
        ],
        "is not installable",
    );
}
