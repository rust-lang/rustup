//! Test cases of the rustup command that do not depend on the
//! dist server, mostly derived from multirust/test-v2.sh

#![allow(deprecated)]

use std::fs;
use std::str;
use std::{env::consts::EXE_SUFFIX, path::Path};

use rustup::for_host;
use rustup::test::{
    CliTestContext, Config, MULTI_ARCH1, Scenario, print_command, print_indented, this_host_triple,
};
use rustup::utils;
use rustup::utils::raw::symlink_dir;

#[tokio::test]
async fn smoke_test() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "--version"]).await;
}

#[tokio::test]
async fn version_mentions_rustc_version_confusion() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;

    cx.config
        .expect_stderr_ok(
            &["rustup", "--version"],
            "This is the version for the rustup toolchain manager",
        )
        .await;

    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    cx.config
        .expect_stderr_ok(
            &["rustup", "+nightly", "--version"],
            "The currently active `rustc` version is `1.3.0",
        )
        .await;
}

#[tokio::test]
async fn no_colors_in_piped_error_output() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let args: Vec<&str> = vec![];
    let out = cx.config.run("rustc", args, &[]).await;
    assert!(!out.ok);
    assert!(!out.stderr.contains('\x1b'));
}

#[tokio::test]
async fn rustc_with_bad_rustup_toolchain_env_var() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let args: Vec<&str> = vec![];
    let out = cx
        .config
        .run("rustc", args, &[("RUSTUP_TOOLCHAIN", "bogus")])
        .await;
    assert!(!out.ok);
    assert!(out.stderr.contains("toolchain 'bogus' is not installed"));
}

#[tokio::test]
async fn custom_invalid_names() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err(
            &["rustup", "toolchain", "link", "nightly", "foo"],
            "invalid custom toolchain name 'nightly'",
        )
        .await;
    cx.config
        .expect_err(
            &["rustup", "toolchain", "link", "beta", "foo"],
            "invalid custom toolchain name 'beta'",
        )
        .await;
    cx.config
        .expect_err(
            &["rustup", "toolchain", "link", "stable", "foo"],
            "invalid custom toolchain name 'stable'",
        )
        .await;
}

#[tokio::test]
async fn custom_invalid_names_with_archive_dates() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err(
            &["rustup", "toolchain", "link", "nightly-2015-01-01", "foo"],
            "invalid custom toolchain name 'nightly-2015-01-01'",
        )
        .await;
    cx.config
        .expect_err(
            &["rustup", "toolchain", "link", "beta-2015-01-01", "foo"],
            "invalid custom toolchain name 'beta-2015-01-01'",
        )
        .await;
    cx.config
        .expect_err(
            &["rustup", "toolchain", "link", "stable-2015-01-01", "foo"],
            "invalid custom toolchain name 'stable-2015-01-01'",
        )
        .await;
}

// Regression test for newline placement
#[tokio::test]
async fn update_all_no_update_whitespace() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] installed - 1.3.0 (hash-nightly-2)


"#]]);
}

// Issue #145
#[tokio::test]
async fn update_works_without_term() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let mut cmd = cx.config.cmd("rustup", ["update", "nightly"]);
    cx.config.env(&mut cmd);
    cmd.env_remove("TERM");

    let out = cmd.output().unwrap();
    assert!(out.status.success());
}

// Issue #1738
#[tokio::test]
async fn show_works_with_dumb_term() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let mut cmd = cx.config.cmd("rustup", ["show"]);
    cx.config.env(&mut cmd);
    cmd.env("TERM", "dumb");
    assert!(cmd.spawn().unwrap().wait().unwrap().success());
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[tokio::test]
async fn subcommand_required_for_target() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let mut cmd = cx.config.cmd("rustup", ["target"]);
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code().unwrap(), 1);
    assert!(str::from_utf8(&out.stdout).unwrap().contains("Usage"));
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[tokio::test]
async fn subcommand_required_for_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let mut cmd = cx.config.cmd("rustup", ["toolchain"]);
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code().unwrap(), 1);
    assert!(str::from_utf8(&out.stdout).unwrap().contains("Usage"));
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[tokio::test]
async fn subcommand_required_for_override() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let mut cmd = cx.config.cmd("rustup", ["override"]);
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code().unwrap(), 1);
    assert!(str::from_utf8(&out.stdout).unwrap().contains("Usage"));
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[tokio::test]
async fn subcommand_required_for_self() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let mut cmd = cx.config.cmd("rustup", ["self"]);
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code().unwrap(), 1);
    assert!(str::from_utf8(&out.stdout).unwrap().contains("Usage"));
}

#[tokio::test]
async fn multi_host_smoke_test() {
    // We cannot run this test if the current host triple is equal to the
    // multi-arch triple, but this should never be the case.  Check that just
    // to be sure.
    assert_ne!(this_host_triple(), MULTI_ARCH1);

    let mut cx = CliTestContext::new(Scenario::MultiHost).await;
    let toolchain = format!("nightly-{MULTI_ARCH1}");
    cx.config
        .expect_ok(&["rustup", "default", &toolchain, "--force-non-host"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "xxxx-nightly-2")
        .await; // cross-host mocks have their own versions
}

#[tokio::test]
async fn custom_toolchain_cargo_fallback_proxy() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");

    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "mytoolchain",
            &path.to_string_lossy(),
        ])
        .await;
    cx.config
        .expect_ok(&["rustup", "default", "mytoolchain"])
        .await;

    cx.config.expect_ok(&["rustup", "update", "stable"]).await;
    cx.config
        .expect_stdout_ok(&["cargo", "--version"], "hash-stable-1.1.0")
        .await;

    cx.config.expect_ok(&["rustup", "update", "beta"]).await;
    cx.config
        .expect_stdout_ok(&["cargo", "--version"], "hash-beta-1.2.0")
        .await;

    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["cargo", "--version"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn custom_toolchain_cargo_fallback_run() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");

    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "mytoolchain",
            &path.to_string_lossy(),
        ])
        .await;
    cx.config
        .expect_ok(&["rustup", "default", "mytoolchain"])
        .await;

    cx.config.expect_ok(&["rustup", "update", "stable"]).await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-stable-1.1.0",
        )
        .await;

    cx.config.expect_ok(&["rustup", "update", "beta"]).await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-beta-1.2.0",
        )
        .await;

    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-nightly-2",
        )
        .await;
}

#[tokio::test]
async fn rustup_run_searches_path() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    #[cfg(windows)]
    let hello_cmd = &["rustup", "run", "nightly", "cmd", "/C", "echo hello"];
    #[cfg(not(windows))]
    let hello_cmd = &["rustup", "run", "nightly", "sh", "-c", "echo hello"];

    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config.expect_stdout_ok(hello_cmd, "hello").await;
}

#[tokio::test]
async fn rustup_doesnt_prepend_path_unnecessarily() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    async fn expect_stderr_ok_env_first_then(
        config: &Config,
        args: &[&str],
        env: &[(&str, &str)],
        first: &Path,
        second: Option<&Path>,
    ) {
        let out = config.run(args[0], &args[1..], env).await;
        let first_then_second = |list: &str| -> bool {
            let mut saw_first = false;
            let mut saw_second = false;
            for path in std::env::split_paths(list) {
                if path == first {
                    if saw_second {
                        return false;
                    }
                    saw_first = true;
                }
                if Some(&*path) == second {
                    if !saw_first {
                        return false;
                    }
                    saw_second = true;
                }
            }
            true
        };
        if !out.ok || !first_then_second(&out.stderr) {
            print_command(args, &out);
            println!("expected.ok: true");
            print_indented(
                "expected.stderr.first_then",
                &format!("{} comes before {:?}", first.display(), second),
            );
            panic!();
        }
    }

    // For all of these, CARGO_HOME/bin will be auto-prepended.
    let cargo_home_bin = cx.config.cargodir.join("bin");
    expect_stderr_ok_env_first_then(
        &cx.config,
        &["cargo", "--echo-path"],
        &[],
        &cargo_home_bin,
        None,
    )
    .await;
    expect_stderr_ok_env_first_then(
        &cx.config,
        &["cargo", "--echo-path"],
        &[("PATH", "")],
        &cargo_home_bin,
        None,
    )
    .await;

    // Check that CARGO_HOME/bin is prepended to path.
    let config = &mut cx.config;
    expect_stderr_ok_env_first_then(
        config,
        &["cargo", "--echo-path"],
        &[("PATH", &format!("{}", config.exedir.display()))],
        &cargo_home_bin,
        Some(&config.exedir),
    )
    .await;

    // But if CARGO_HOME/bin is already on PATH, it will not be prepended again,
    // so exedir will take precedence.
    expect_stderr_ok_env_first_then(
        config,
        &["cargo", "--echo-path"],
        &[(
            "PATH",
            std::env::join_paths([&config.exedir, &cargo_home_bin])
                .unwrap()
                .to_str()
                .unwrap(),
        )],
        &config.exedir,
        Some(&cargo_home_bin),
    )
    .await;
}

#[tokio::test]
async fn rustup_failed_path_search() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    use std::env::consts::EXE_SUFFIX;

    let rustup_path = cx.config.exedir.join(format!("rustup{EXE_SUFFIX}"));
    let tool_path = cx.config.exedir.join(format!("fake_proxy{EXE_SUFFIX}"));
    utils::hardlink_file(&rustup_path, &tool_path).expect("Failed to create fake proxy for test");

    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "custom",
            &cx.config.customdir.join("custom-1").to_string_lossy(),
        ])
        .await;

    cx.config.expect_ok(&["rustup", "default", "custom"]).await;

    let broken = &["rustup", "run", "custom", "fake_proxy"];
    cx.config
        .expect_err(
            broken,
            "unknown proxy name: 'fake_proxy'; valid proxy names are \
             'rustc', 'rustdoc', 'cargo', 'rust-lldb', 'rust-gdb', 'rust-gdbgui', \
             'rls', 'cargo-clippy', 'clippy-driver', 'cargo-miri', \
             'rust-analyzer', 'rustfmt', 'cargo-fmt'",
        )
        .await;

    // Hardlink will be automatically cleaned up by test setup code
}

#[tokio::test]
async fn rustup_failed_path_search_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    use std::env::consts::EXE_SUFFIX;

    let rustup_path = cx.config.exedir.join(format!("rustup{EXE_SUFFIX}"));
    let tool_path = cx.config.exedir.join(format!("cargo-miri{EXE_SUFFIX}"));
    utils::hardlink_file(&rustup_path, &tool_path)
        .expect("Failed to create fake cargo-miri for test");

    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "custom-1",
            &cx.config.customdir.join("custom-1").to_string_lossy(),
        ])
        .await;

    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "custom-2",
            &cx.config.customdir.join("custom-2").to_string_lossy(),
        ])
        .await;

    cx.config
        .expect_ok(&["rustup", "default", "custom-2"])
        .await;

    let broken = &["rustup", "run", "custom-1", "cargo-miri"];
    cx.config
        .expect_err(broken, "cannot use `rustup component add`")
        .await;

    let broken = &["rustup", "run", "custom-2", "cargo-miri"];
    cx.config
        .expect_err(broken, "cannot use `rustup component add`")
        .await;

    // Hardlink will be automatically cleaned up by test setup code
}

#[tokio::test]
async fn rustup_run_not_installed() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "install", "stable"]).await;
    cx.config
        .expect_err(
            &["rustup", "run", "nightly", "rustc", "--version"],
            for_host!("toolchain 'nightly-{0}' is not installed"),
        )
        .await;
}

#[tokio::test]
async fn rustup_run_install() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "install", "stable"]).await;
    cx.config
        .expect_stderr_ok(
            &[
                "rustup",
                "run",
                "--install",
                "nightly",
                "cargo",
                "--version",
            ],
            "info: installing component 'rustc'",
        )
        .await;
}

#[tokio::test]
async fn toolchains_are_resolved_early() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    let full_toolchain = format!("nightly-{}", this_host_triple());
    cx.config
        .expect_stderr_ok(
            &["rustup", "default", &full_toolchain],
            &format!("info: using existing install for '{full_toolchain}'"),
        )
        .await;
}

// #190
#[tokio::test]
async fn proxies_pass_empty_args() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "run", "nightly", "rustc", "--empty-arg-test", ""])
        .await;
}

#[tokio::test]
async fn rls_exists_in_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;

    assert!(cx.config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
    cx.config.expect_ok(&["rls", "--version"]).await;
}

#[tokio::test]
async fn run_rls_when_not_available_in_toolchain() {
    let mut cx = CliTestContext::new(Scenario::UnavailableRls).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config.expect_err(
        &["rls", "--version"],
        &format!(
            "the 'rls' component which provides the command 'rls{}' is not available for the 'nightly-{}' toolchain",
            EXE_SUFFIX,
            this_host_triple(),
        ),
    ).await;

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;

    cx.config.expect_ok(&["rls", "--version"]).await;
}

#[tokio::test]
async fn run_rls_when_not_installed() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config.expect_err(
        &["rls", "--version"],
        &format!(
            "'rls{}' is not installed for the toolchain 'stable-{}'.\nTo install, run `rustup component add rls`",
            EXE_SUFFIX,
            this_host_triple(),
        ),
    ).await;
}

#[tokio::test]
async fn run_rls_when_not_installed_for_nightly() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;
    cx.config.expect_err(
        &["rls", "+nightly", "--version"],
        &format!(
            "'rls{}' is not installed for the toolchain 'nightly-{}'.\nTo install, run `rustup component add --toolchain nightly-{1} rls`",
            EXE_SUFFIX,
            this_host_triple(),
        ),
    ).await;
}

#[tokio::test]
async fn run_rust_lldb_when_not_in_toolchain() {
    let mut cx = CliTestContext::new(Scenario::UnavailableRls).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config.expect_err(
        &["rust-lldb", "--version"],
        &format!(
            "the 'rust-lldb{}' binary, normally provided by the 'rustc' component, is not applicable to the 'nightly-{}' toolchain",
            EXE_SUFFIX,
            this_host_triple(),
        ),
    ).await;
}

#[tokio::test]
async fn rename_rls_before() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update"]).await;

    assert!(cx.config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
    cx.config.expect_ok(&["rls", "--version"]).await;
}

#[tokio::test]
async fn rename_rls_after() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls-preview"])
        .await;

    assert!(cx.config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
    cx.config.expect_ok(&["rls", "--version"]).await;
}

#[tokio::test]
async fn rename_rls_add_old_name() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;

    assert!(cx.config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
    cx.config.expect_ok(&["rls", "--version"]).await;
}

#[tokio::test]
async fn rename_rls_list() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;

    let out = cx.config.run("rustup", ["component", "list"], &[]).await;
    assert!(out.ok);
    assert!(out.stdout.contains(&format!("rls-{}", this_host_triple())));
}

#[tokio::test]
async fn rename_rls_preview_list() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls-preview"])
        .await;

    let out = cx.config.run("rustup", ["component", "list"], &[]).await;
    assert!(out.ok);
    assert!(out.stdout.contains(&format!("rls-{}", this_host_triple())));
}

#[tokio::test]
async fn rename_rls_remove() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update"]).await;

    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;
    cx.config.expect_ok(&["rls", "--version"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "remove", "rls"])
        .await;
    cx.config
        .expect_err(
            &["rls", "--version"],
            &format!("'rls{EXE_SUFFIX}' is not installed"),
        )
        .await;

    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;
    cx.config.expect_ok(&["rls", "--version"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "remove", "rls-preview"])
        .await;
    cx.config
        .expect_err(
            &["rls", "--version"],
            &format!("'rls{EXE_SUFFIX}' is not installed"),
        )
        .await;
}

// issue #3737
/// `~/.rustup/toolchains` is permitted to be a symlink.
#[tokio::test]
#[cfg(any(unix, windows))]
async fn toolchains_symlink() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    let test_toolchains = cwd.join("toolchains-test");
    fs::create_dir(&test_toolchains).unwrap();
    symlink_dir(&test_toolchains, &cx.config.rustupdir.join("toolchains")).unwrap();

    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok_contains(&["rustup", "toolchain", "list"], "nightly", "")
        .await;
    cx.config
        .expect_ok_contains(&["rustc", "--version"], "hash-nightly-2", "")
        .await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "uninstall", "nightly"])
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "toolchain", "list"],
            "no installed toolchains\n",
        )
        .await;
}

// issue #3344
/// `~/.rustup/tmp` and `~/.rustup/downloads` are permitted to be symlinks.
#[tokio::test]
#[cfg(any(unix, windows))]
async fn tmp_downloads_symlink() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    let cwd = cx.config.current_dir();

    let test_tmp = cwd.join("tmp-test");
    fs::create_dir(&test_tmp).unwrap();
    symlink_dir(&test_tmp, &cx.config.rustupdir.join("tmp")).unwrap();

    let test_downloads = cwd.join("tmp-downloads");
    fs::create_dir(&test_downloads).unwrap();
    symlink_dir(&test_downloads, &cx.config.rustupdir.join("downloads")).unwrap();

    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update"]).await;

    assert!(cx.config.rustupdir.join("tmp").exists());
    assert!(cx.config.rustupdir.join("downloads").exists());
}

// issue #1169
/// A toolchain that is a stale symlink should be correctly uninstalled.
#[tokio::test]
#[cfg(any(unix, windows))]
async fn toolchain_broken_symlink() {
    let mut cx = CliTestContext::new(Scenario::None).await;
    // We artificially create a broken symlink toolchain -- but this can also happen "legitimately"
    // by having a proper toolchain there, using "toolchain link", and later removing the directory.
    fs::create_dir(cx.config.rustupdir.join("toolchains")).unwrap();
    fs::create_dir(cx.config.rustupdir.join("this-directory-does-not-exist")).unwrap();
    symlink_dir(
        &cx.config.rustupdir.join("this-directory-does-not-exist"),
        &cx.config.rustupdir.join("toolchains").join("test"),
    )
    .unwrap();
    fs::remove_dir(cx.config.rustupdir.join("this-directory-does-not-exist")).unwrap();

    // Make sure this "fake install" actually worked
    cx.config
        .expect_ok_ex(&["rustup", "toolchain", "list"], "test\n", "")
        .await;
    // Now try to uninstall it.  That should work only once.
    cx.config
        .expect_ok_ex(
            &["rustup", "toolchain", "uninstall", "test"],
            "",
            r"info: uninstalling toolchain 'test'
info: toolchain 'test' uninstalled
",
        )
        .await;
    cx.config
        .expect_stderr_ok(
            &["rustup", "toolchain", "uninstall", "test"],
            "no toolchain installed for 'test'",
        )
        .await;
}

// issue #1297
#[tokio::test]
async fn update_unavailable_rustc() {
    let mut cx = CliTestContext::new(Scenario::Unavailable).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;

    // latest nightly is unavailable
    cx.config.set_current_dist_date("2015-01-02");
    // update should do nothing
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
}

// issue 2562
#[tokio::test]
async fn install_unavailable_platform() {
    let cx = CliTestContext::new(Scenario::Unavailable).await;
    cx.config.set_current_dist_date("2015-01-02");
    // explicit attempt to install should fail
    cx.config
        .expect_err(
            &["rustup", "toolchain", "install", "nightly"],
            "is not installable",
        )
        .await;
    // implicit attempt to install should fail
    cx.config
        .expect_err(&["rustup", "default", "nightly"], "is not installable")
        .await;
}

// issue #1329
#[tokio::test]
async fn install_beta_with_tag() {
    let mut cx = CliTestContext::new(Scenario::BetaTag).await;
    cx.config
        .expect_ok(&["rustup", "default", "1.78.0-beta"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "1.78.0-beta")
        .await;

    cx.config
        .expect_ok(&["rustup", "default", "1.79.0-beta.2"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "1.79.0-beta.2")
        .await;
}

#[tokio::test]
async fn update_nightly_even_with_incompat() {
    let mut cx = CliTestContext::new(Scenario::MissingComponent).await;
    cx.config.set_current_dist_date("2019-09-12");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;
    cx.config.expect_component_executable("rls").await;

    // latest nightly is now one that does not have RLS
    cx.config.set_current_dist_date("2019-09-14");

    cx.config.expect_component_executable("rls").await;
    // update should bring us to latest nightly that does
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
    cx.config.expect_component_executable("rls").await;
}

#[tokio::test]
async fn nightly_backtrack_skips_missing() {
    let mut cx = CliTestContext::new(Scenario::MissingNightly).await;
    cx.config.set_current_dist_date("2019-09-16");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;
    cx.config.expect_component_executable("rls").await;

    // rls is missing on latest, nightly is missing on second-to-latest
    cx.config.set_current_dist_date("2019-09-18");

    // update should not change nightly, and should not error
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
}

#[tokio::test]
async fn completion_rustup() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "completions", "bash", "rustup"])
        .await;
}

#[tokio::test]
async fn completion_cargo() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "completions", "bash", "cargo"])
        .await;
}

#[tokio::test]
async fn completion_default() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok_eq(
            &["rustup", "completions", "bash"],
            &["rustup", "completions", "bash", "rustup"],
        )
        .await;
}

#[tokio::test]
async fn completion_bad_shell() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err(
            &["rustup", "completions", "fake"],
            r#"error: invalid value 'fake' for '<SHELL>'"#,
        )
        .await;
    cx.config
        .expect_err(
            &["rustup", "completions", "fake", "cargo"],
            r#"error: invalid value 'fake' for '<SHELL>'"#,
        )
        .await;
}

#[tokio::test]
async fn completion_bad_tool() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err(
            &["rustup", "completions", "bash", "fake"],
            r#"error: invalid value 'fake' for '[COMMAND]'"#,
        )
        .await;
}

#[tokio::test]
async fn completion_cargo_unsupported_shell() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err(
            &["rustup", "completions", "fish", "cargo"],
            "error: cargo does not currently support completions for ",
        )
        .await;
}

#[tokio::test]
async fn add_remove_component() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config.expect_component_executable("rustc").await;
    cx.config
        .expect_ok(&["rustup", "component", "remove", "rustc"])
        .await;
    cx.config.expect_component_not_executable("rustc").await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rustc"])
        .await;
    cx.config.expect_component_executable("rustc").await;
}

#[tokio::test]
async fn which() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path_1 = cx.config.customdir.join("custom-1");
    let path_1 = path_1.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "custom-1", &path_1])
        .await;
    cx.config
        .expect_ok(&["rustup", "default", "custom-1"])
        .await;
    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(
            &["rustup", "which", "rustc"],
            "\\toolchains\\custom-1\\bin\\rustc",
        )
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(
            &["rustup", "which", "rustc"],
            "/toolchains/custom-1/bin/rustc",
        )
        .await;
    let path_2 = cx.config.customdir.join("custom-2");
    let path_2 = path_2.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "custom-2", &path_2])
        .await;
    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(
            &["rustup", "which", "--toolchain=custom-2", "rustc"],
            "\\toolchains\\custom-2\\bin\\rustc",
        )
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(
            &["rustup", "which", "--toolchain=custom-2", "rustc"],
            "/toolchains/custom-2/bin/rustc",
        )
        .await;
}

#[tokio::test]
async fn which_asking_uninstalled_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path_1 = cx.config.customdir.join("custom-1");
    let path_1 = path_1.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "custom-1", &path_1])
        .await;
    cx.config
        .expect_ok(&["rustup", "default", "custom-1"])
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "which", "rustc"],
            &["", "toolchains", "custom-1", "bin", "rustc"].join(std::path::MAIN_SEPARATOR_STR),
        )
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "which", "--toolchain=nightly", "rustc"],
            &["", "toolchains", for_host!("nightly-{0}"), "bin", "rustc"]
                .join(std::path::MAIN_SEPARATOR_STR),
        )
        .await;
}

#[tokio::test]
async fn override_by_toolchain_on_the_command_line() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "stable", "nightly"])
        .await;

    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(
            &["rustup", "+stable", "which", "rustc"],
            for_host!("\\toolchains\\stable-{}"),
        )
        .await;
    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(&["rustup", "+stable", "which", "rustc"], "\\bin\\rustc")
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(
            &["rustup", "+stable", "which", "rustc"],
            for_host!("/toolchains/stable-{}"),
        )
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(&["rustup", "+stable", "which", "rustc"], "/bin/rustc")
        .await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(
            &["rustup", "+nightly", "which", "rustc"],
            for_host!("\\toolchains\\nightly-{}"),
        )
        .await;
    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(&["rustup", "+nightly", "which", "rustc"], "\\bin\\rustc")
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(
            &["rustup", "+nightly", "which", "rustc"],
            for_host!("/toolchains/nightly-{}"),
        )
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(&["rustup", "+nightly", "which", "rustc"], "/bin/rustc")
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "+nightly", "show"],
            "active because: overridden by +toolchain on the command line",
        )
        .await;
    cx.config
        .expect_err(
            &["rustup", "+foo", "which", "rustc"],
            "toolchain 'foo' is not installed",
        )
        .await;
    cx.config
        .expect_stderr_ok(
            &["rustup", "+stable", "set", "profile", "minimal"],
            "profile set to 'minimal'",
        )
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "default"], for_host!("nightly-{}"))
        .await;
}

#[tokio::test]
async fn toolchain_link_then_list_verbose() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path_1 = cx.config.customdir.join("custom-1");
    let path_1 = path_1.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "custom-1", &path_1])
        .await;
    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list", "-v"], "\\custom-1")
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list", "-v"], "/custom-1")
        .await;
}

#[tokio::test]
async fn update_self_smart_guess() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let out = cx.config.run("rustup", &["update", "self"], &[]).await;
    let invalid_toolchain = out.stderr.contains("invalid toolchain name");
    if !out.ok && invalid_toolchain {
        assert!(
            out.stderr
                .contains("if you meant to update rustup itself, use `rustup self update`")
        )
    }
}

#[tokio::test]
async fn uninstall_self_smart_guess() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let out = cx.config.run("rustup", &["uninstall", "self"], &[]).await;
    let no_toolchain_installed = out.stdout.contains("no toolchain installed");
    if out.ok && no_toolchain_installed {
        assert!(
            out.stdout
                .contains("if you meant to uninstall rustup itself, use `rustup self uninstall`")
        )
    }
}

// https://github.com/rust-lang/rustup/issues/4073
#[tokio::test]
async fn toolchain_install_multi_components_comma() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let components = ["rls", "rust-docs"];
    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "install",
            "--profile=minimal",
            "--component",
            &components.join(","),
            "nightly",
        ])
        .await;
    for component in components {
        cx.config
            .expect_ok_contains(
                &["rustup", "+nightly", "component", "list", "--installed"],
                for_host!("{component}-{}"),
                "",
            )
            .await;
    }
}

#[tokio::test]
async fn rustup_updates_cargo_env_if_proxy() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;

    use std::env::consts::EXE_SUFFIX;
    let proxy_path = cx
        .config
        .workdir
        .borrow()
        .join("bin")
        .join(format!("cargo{EXE_SUFFIX}"));
    let real_path = cx.config.run("rustup", &["which", "cargo"], &[]).await;
    assert!(real_path.ok);
    let real_path = real_path.stdout;

    fs::create_dir_all(proxy_path.parent().unwrap()).unwrap();
    #[cfg(windows)]
    if std::os::windows::fs::symlink_file("rustup", &proxy_path).is_err() {
        // skip this test on Windows if symlinking isn't enabled.
        return;
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink("rustup", &proxy_path).unwrap();

    // If CARGO isn't set then we should not set it.
    cx.config
        .expect_err(
            &["cargo", "--echo-cargo-env"],
            "CARGO environment variable not set",
        )
        .await;

    // If CARGO is set to a proxy then change it to the real CARGO path
    cx.config
        .expect_ok_ex_env(
            &["cargo", "--echo-cargo-env"],
            &[("CARGO", proxy_path.to_str().unwrap())],
            "",
            &real_path,
        )
        .await;
}

#[tokio::test]
async fn rust_analyzer_proxy_falls_back_external() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "install",
            "stable",
            "--profile=minimal",
            "--component=rls",
        ])
        .await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;

    // We pretend to have a `rust-analyzer` installation by reusing the `rls`
    // proxy and mock binary.
    let rls = format!("rls{EXE_SUFFIX}");
    let ra = format!("rust-analyzer{EXE_SUFFIX}");
    let exedir = &cx.config.exedir;
    let bindir = &cx
        .config
        .rustupdir
        .join("toolchains")
        .join(for_host!("stable-{0}"))
        .join("bin");
    for dir in [exedir, bindir] {
        fs::rename(dir.join(&rls), dir.join(&ra)).unwrap();
    }

    // Base case: rustup-hosted RA installed, external RA unavailable,
    // use the former.
    let real_path = cx
        .config
        .run("rust-analyzer", &["--echo-current-exe"], &[])
        .await;
    assert!(real_path.ok);
    let real_path_str = real_path.stderr.lines().next().unwrap();
    let real_path = Path::new(real_path_str);

    assert!(real_path.is_file());

    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let extern_dir = tempdir.path();
    let extern_path = &extern_dir.join("rust-analyzer");
    fs::copy(real_path, extern_path).unwrap();

    // First case: rustup-hosted and external RA both installed,
    // prioritize the former.
    cx.config
        .expect_ok_ex_env(
            &["rust-analyzer", "--echo-current-exe"],
            &[("PATH", &extern_dir.to_string_lossy())],
            "",
            &format!("{real_path_str}\n"),
        )
        .await;

    // Second case: rustup-hosted RA unavailable, fallback on the external RA.
    fs::remove_file(bindir.join(&ra)).unwrap();
    cx.config
        .expect_ok_ex_env(
            &["rust-analyzer", "--echo-current-exe"],
            &[("PATH", &extern_dir.to_string_lossy())],
            "",
            &format!(
                "info: `rust-analyzer` is unavailable for the active toolchain\ninfo: falling back to {:?}\n{}\n",
                extern_path.as_os_str(), extern_path.display()
            ),
        )
        .await;
}
