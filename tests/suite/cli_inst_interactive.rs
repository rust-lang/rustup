//! Tests of the interactive console installer

use std::env::consts::EXE_SUFFIX;
use std::io::Write;
use std::process::Stdio;

use rustup::test::{Assert, CliTestContext, Config, SanitizedOutput, Scenario, this_host_triple};
#[cfg(windows)]
use rustup::test::{RegistryGuard, USER_PATH};
use rustup::utils::raw;

fn run_input(config: &Config, args: &[&str], input: &str) -> Assert {
    run_input_with_env(config, args, input, &[])
}

fn run_input_with_env(config: &Config, args: &[&str], input: &str, env: &[(&str, &str)]) -> Assert {
    let mut cmd = config.cmd(args[0], &args[1..]);
    config.env(&mut cmd);

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

    Assert::new(SanitizedOutput {
        ok: out.status.success(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    })
}

#[tokio::test]
async fn update() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    #[cfg(windows)]
    let _path_guard = RegistryGuard::new(&USER_PATH).unwrap();

    run_input(&cx.config, &["rustup-init"], "\n\n");
    run_input(&cx.config, &["rustup-init"], "\n\n").is_ok();
}

// Testing that the right number of blank lines are printed after the
// 'pre-install' message and before the 'post-install' message - overall smoke
// test for the install case.
#[tokio::test]
async fn smoke_case_install_no_modify_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    // During an interactive session, after "Press the Enter
    // key..."  the UI emits a blank line, then there is a blank
    // line that comes from the user pressing enter, then log
    // output on stderr, then an explicit blank line on stdout
    // before printing $toolchain installed
    run_input(&cx.config, &["rustup-init", "--no-modify-path"], "\n\n")
        .with_stdout(snapbox::str![[r#"
...
This path needs to be in your PATH environment variable,
but will not be added automatically.

You can uninstall at any time with rustup self uninstall and
these changes will be reverted.

Current installation options:


   default host triple: [HOST_TRIPLE]
     default toolchain: stable (default)
               profile: default
  modify PATH variable: no

1) Proceed with standard installation (default - just press enter)
2) Customize installation
3) Cancel installation
>

  stable-[HOST_TRIPLE] installed - 1.1.0 (hash-stable-1.1.0)


Rust is installed now. Great!
...
"#]])
        .is_ok();
    if cfg!(unix) {
        assert!(!cx.config.homedir.join(".profile").exists());
        assert!(cx.config.cargodir.join("env").exists());
    }
}

#[tokio::test]
async fn smoke_case_install_with_path_install() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    #[cfg(windows)]
    let _path_guard = RegistryGuard::new(&USER_PATH).unwrap();

    run_input(&cx.config, &["rustup-init"], "\n\n")
        .is_ok()
        .without_stdout("This path needs to be in your PATH environment variable");
}

#[tokio::test]
async fn blank_lines_around_stderr_log_output_update() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-update-default-toolchain",
            "--no-modify-path",
        ],
        "\n\n",
    )
    .with_stdout(snapbox::str![[r#"
...
3) Cancel installation
>


Rust is installed now. Great!
...
"#]]);
}

#[tokio::test]
async fn installer_shows_default_host_triple() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(&cx.config, &["rustup-init", "--no-modify-path"], "2\n").with_stdout(snapbox::str![
        [r#"
...
Default host triple? [[HOST_TRIPLE]]
...
"#]
    ]);
}

#[tokio::test]
async fn installer_shows_default_toolchain_as_stable() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(&cx.config, &["rustup-init", "--no-modify-path"], "2\n\n").with_stdout(
        snapbox::str![[r#"
...
Default toolchain? (stable/beta/nightly/none) [stable]
...
"#]],
    );
}

#[tokio::test]
async fn installer_shows_default_toolchain_when_set_in_args() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-modify-path",
            "--default-toolchain=nightly",
        ],
        "2\n\n",
    )
    .with_stdout(snapbox::str![[r#"
...
Default toolchain? (stable/beta/nightly/none) [nightly]
...
"#]]);
}

#[tokio::test]
async fn installer_shows_default_profile() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(&cx.config, &["rustup-init", "--no-modify-path"], "2\n\n\n").with_stdout(
        snapbox::str![[r#"
...
Profile (which tools and data to install)? (minimal/default/complete) [default]
...
"#]],
    );
}

#[tokio::test]
async fn installer_shows_default_profile_when_set_in_args() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path", "--profile=minimal"],
        "2\n\n\n",
    )
    .with_stdout(snapbox::str![[r#"
...
Profile (which tools and data to install)? (minimal/default/complete) [minimal]
...
"#]]);
}

#[tokio::test]
async fn installer_shows_default_for_modify_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(&cx.config, &["rustup-init"], "2\n\n\n\n").with_stdout(snapbox::str![[r#"
...
Modify PATH variable? (Y/n)
...
"#]]);
}

#[tokio::test]
async fn installer_shows_default_for_modify_path_when_set_with_args() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\n\n\n",
    )
    .with_stdout(snapbox::str![[r#"
...
Modify PATH variable? (y/N)
...
"#]]);
}

#[tokio::test]
async fn user_says_nope() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(&cx.config, &["rustup-init", "--no-modify-path"], "n\n\n").is_ok();
    assert!(!cx.config.cargodir.join("bin").exists());
}

#[tokio::test]
async fn with_no_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-modify-path",
            "--default-toolchain=none",
        ],
        "\n\n",
    )
    .is_ok();

    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
no active toolchain
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn with_non_default_toolchain_still_prompts() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-modify-path",
            "--default-toolchain=nightly",
        ],
        "\n\n",
    )
    .is_ok();

    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
installed toolchains
--------------------
nightly-[HOST_TRIPLE] (active, default)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn with_non_release_channel_non_default_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &[
            "rustup-init",
            "--no-modify-path",
            "--default-toolchain=nightly-2015-01-02",
        ],
        "\n\n",
    )
    .is_ok();

    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
installed toolchains
--------------------
nightly-2015-01-02-[HOST_TRIPLE] (active, default)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn set_nightly_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\nnightly\n\n\n\n\n",
    )
    .is_ok();

    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
installed toolchains
--------------------
nightly-[HOST_TRIPLE] (active, default)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn set_no_modify_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\n\n\nno\n\n\n",
    )
    .is_ok();

    if cfg!(unix) {
        assert!(!cx.config.homedir.join(".profile").exists());
    }
}

#[tokio::test]
async fn set_nightly_toolchain_and_unset() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\nnightly\n\n\n2\n\nbeta\n\n\n\n\n",
    )
    .is_ok();

    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
installed toolchains
--------------------
beta-[HOST_TRIPLE] (active, default)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn user_says_nope_after_advanced_install() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    run_input(
        &cx.config,
        &["rustup-init", "--no-modify-path"],
        "2\n\n\n\n\nn\n\n\n",
    )
    .is_ok();
    assert!(!cx.config.cargodir.join("bin").exists());
}

#[tokio::test]
async fn install_with_components() {
    async fn go(comp_args: &[&str]) {
        let mut args = vec!["rustup-init", "-y", "--no-modify-path"];
        args.extend_from_slice(comp_args);

        let cx = CliTestContext::new(Scenario::SimpleV2).await;
        cx.config.expect(&args).await.is_ok();
        cx.config
            .expect(["rustup", "component", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
...
rust-src (installed)
...
"#]])
            .is_ok();
        cx.config
            .expect(["rustup", "component", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
...
rust-analysis-[HOST_TRIPLE] (installed)
...
"#]])
            .is_ok();
    }

    go(&["-c", "rust-src", "-c", "rust-analysis"]).await;
    go(&["-c", "rust-src,rust-analysis"]).await;
}

#[tokio::test]
async fn install_forces_and_skips_rls() {
    let cx = CliTestContext::new(Scenario::UnavailableRls).await;
    cx.config.set_current_dist_date("2015-01-01");

    run_input(
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
    )
    .is_ok()
    .with_stderr(snapbox::str![[r#"
...
warn: Force-skipping unavailable component 'rls-[HOST_TRIPLE]'
...
"#]]);
}

#[tokio::test]
async fn test_warn_if_complete_profile_is_used() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--profile",
            "complete",
            "--no-modify-path",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: downloading with complete profile isn't recommended unless you are a developer of the rust language
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn installing_when_already_installed_updates_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    run_input(&cx.config, &["rustup-init", "--no-modify-path"], "\n\n").with_stdout(snapbox::str![
        [r#"
...
[..]stable-[HOST_TRIPLE] unchanged - 1.1.0 (hash-stable-1.1.0)
...
"#]
    ]);
}

#[tokio::test]
async fn install_stops_if_rustc_exists() {
    let temp_dir = tempfile::Builder::new()
        .prefix("fakebin")
        .tempdir()
        .unwrap();
    // Create fake executable
    let fake_exe = temp_dir.path().join(format!("{}{}", "rustc", EXE_SUFFIX));
    raw::append_file(&fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            ["rustup-init", "--no-modify-path"],
            [
                ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
                ("PATH", temp_dir_path),
            ],
        )
        .await
        .is_err()
        .with_stderr(snapbox::str![[r#"
...
warn: It looks like you have an existing installation of Rust at:
...
warn: If you are sure that you want both rustup and your already installed Rust
...
"#]]);
}

#[tokio::test]
async fn install_stops_if_cargo_exists() {
    let temp_dir = tempfile::Builder::new()
        .prefix("fakebin")
        .tempdir()
        .unwrap();
    // Create fake executable
    let fake_exe = temp_dir.path().join(format!("{}{}", "cargo", EXE_SUFFIX));
    raw::append_file(&fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            ["rustup-init", "--no-modify-path"],
            [
                ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
                ("PATH", temp_dir_path),
            ],
        )
        .await
        .is_err()
        .with_stderr(snapbox::str![[r#"
...
warn: It looks like you have an existing installation of Rust at:
...
warn: If you are sure that you want both rustup and your already installed Rust
...
"#]]);
}

#[tokio::test]
async fn with_no_prompt_install_succeeds_if_rustc_exists() {
    let temp_dir = tempfile::Builder::new()
        .prefix("fakebin")
        .tempdir()
        .unwrap();
    // Create fake executable
    let fake_exe = temp_dir.path().join(format!("{}{}", "rustc", EXE_SUFFIX));
    raw::append_file(&fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            ["rustup-init", "-y", "--no-modify-path"],
            [
                ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
                ("PATH", temp_dir_path),
            ],
        )
        .await
        .is_ok();
}

// Issue 2547
#[tokio::test]
async fn install_non_installable_toolchain() {
    let cx = CliTestContext::new(Scenario::Unavailable).await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "nightly",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' is not installable
...
"#]])
        .is_err();
}

#[tokio::test]
async fn install_warns_about_existing_settings_file() {
    let temp_dir = tempfile::Builder::new()
        .prefix("fakehome")
        .tempdir()
        .unwrap();
    // Create `settings.toml`
    let settings_file = temp_dir.path().join("settings.toml");
    raw::write_file(
        &settings_file,
        &format!(
            r#"default_toolchain = "{}"
profile = "default"
version = "12""#,
            this_host_triple()
        ),
    )
    .unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            ["rustup-init", "-y", "--no-modify-path"],
            [
                ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
                ("RUSTUP_HOME", temp_dir_path),
            ],
        )
        .await
        .is_ok()
        .with_stderr(snapbox::str![[r#"
...
warn: It looks like you have an existing rustup settings file at:
...
"#]]);
}
