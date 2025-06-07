//! Test cases of the rustup command that do not depend on the
//! dist server, mostly derived from multirust/test-v2.sh

use std::fs;
use std::str;
use std::{env::consts::EXE_SUFFIX, path::Path};

use itertools::Itertools;
use rustup::test::Assert;
use rustup::test::{CliTestContext, MULTI_ARCH1, Scenario, this_host_triple};
use rustup::utils;
use rustup::utils::raw::symlink_dir;

#[tokio::test]
async fn smoke_test() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect(["rustup", "--version"]).await.is_ok();
}

#[tokio::test]
async fn version_mentions_rustc_version_confusion() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    cx.config
        .expect(["rustup", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: This is the version for the rustup toolchain manager, not the rustc compiler.
...
"#]])
        .is_ok();

    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "+nightly", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: The currently active `rustc` version is `1.3.0 (hash-nightly-2)`
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn no_colors_in_piped_error_output() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustc"])
        .await
        .is_err()
        .without_stderr("\u{1b}");
}

#[tokio::test]
async fn rustc_with_bad_rustup_toolchain_env_var() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(["rustc"], [("RUSTUP_TOOLCHAIN", "bogus")])
        .await
        .with_stderr(snapbox::str![[r#"
error: override toolchain 'bogus' is not installed[..]

"#]])
        .is_err();
}

#[tokio::test]
async fn custom_invalid_names() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "link", "nightly", "foo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error:[..] invalid custom toolchain name 'nightly'
...
"#]])
        .is_err();
    cx.config
        .expect(["rustup", "toolchain", "link", "beta", "foo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error:[..] invalid custom toolchain name 'beta'
...
"#]])
        .is_err();
    cx.config
        .expect(["rustup", "toolchain", "link", "stable", "foo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error:[..] invalid custom toolchain name 'stable'
...
"#]])
        .is_err();
}

#[tokio::test]
async fn custom_invalid_names_with_archive_dates() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "link", "nightly-2015-01-01", "foo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error:[..] invalid custom toolchain name 'nightly-2015-01-01'
...
"#]])
        .is_err();
    cx.config
        .expect(["rustup", "toolchain", "link", "beta-2015-01-01", "foo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error:[..] invalid custom toolchain name 'beta-2015-01-01'
...
"#]])
        .is_err();
    cx.config
        .expect(["rustup", "toolchain", "link", "stable-2015-01-01", "foo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error:[..] invalid custom toolchain name 'stable-2015-01-01'
...
"#]])
        .is_err();
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

    let cx = CliTestContext::new(Scenario::MultiHost).await;
    let toolchain = format!("nightly-{MULTI_ARCH1}");
    cx.config
        .expect(["rustup", "default", &toolchain, "--force-non-host"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (xxxx-nightly-2)

"#]])
        .is_ok(); // cross-host mocks have their own versions
}

#[tokio::test]
async fn custom_toolchain_cargo_fallback_proxy() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");

    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "mytoolchain",
            &path.to_string_lossy(),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "mytoolchain"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "update", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();

    cx.config.expect(["rustup", "update", "beta"]).await.is_ok();
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();

    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn custom_toolchain_cargo_fallback_run() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");

    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "mytoolchain",
            &path.to_string_lossy(),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "mytoolchain"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "update", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "run", "mytoolchain", "cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();

    cx.config.expect(["rustup", "update", "beta"]).await.is_ok();
    cx.config
        .expect(["rustup", "run", "mytoolchain", "cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();

    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "run", "mytoolchain", "cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn rustup_run_searches_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    #[cfg(windows)]
    let hello_cmd = &["rustup", "run", "nightly", "cmd", "/C", "echo hello"];
    #[cfg(not(windows))]
    let hello_cmd = &["rustup", "run", "nightly", "sh", "-c", "echo hello"];

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(hello_cmd)
        .await
        .with_stdout(snapbox::str![[r#"
hello

"#]])
        .is_ok();
}

#[tokio::test]
async fn rustup_doesnt_prepend_path_unnecessarily() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    let assert_ok_with_paths = |assert: &Assert, data| {
        assert.is_ok();
        let stderr = std::env::split_paths(&assert.output.stderr)
            .format_with("\n", |p, f| f(&p.display()))
            .to_string();
        let stderr = assert.redact(&stderr);
        snapbox::assert_data_eq!(stderr, data);
    };

    // For all of these, CARGO_HOME/bin will be auto-prepended.
    let cargo_home_bin = cx.config.cargodir.join("bin");
    assert_ok_with_paths(
        cx.config
            .expect(["cargo", "--echo-path"])
            .await
            .extend_redactions([("[CARGO_HOME_BIN]", &cargo_home_bin)]),
        snapbox::str![[r#"
[CARGO_HOME_BIN]
...
"#]],
    );

    assert_ok_with_paths(
        cx.config
            .expect_with_env(["cargo", "--echo-path"], [("PATH", "")])
            .await
            .extend_redactions([("[CARGO_HOME_BIN]", &cargo_home_bin)]),
        snapbox::str![[r#"
[CARGO_HOME_BIN]
...
"#]],
    );

    // Check that CARGO_HOME/bin is prepended to path.
    assert_ok_with_paths(
        cx.config
            .expect_with_env(
                ["cargo", "--echo-path"],
                [("PATH", &*cx.config.exedir.display().to_string())],
            )
            .await
            .extend_redactions([
                ("[CARGO_HOME_BIN]", &cargo_home_bin),
                ("[EXEDIR]", &cx.config.exedir),
            ]),
        snapbox::str![[r#"
[CARGO_HOME_BIN]
[EXEDIR]
...
"#]],
    );

    // But if CARGO_HOME/bin is already on PATH, it will not be prepended again,
    // so exedir will take precedence.
    assert_ok_with_paths(
        cx.config
            .expect_with_env(
                ["cargo", "--echo-path"],
                [(
                    "PATH",
                    std::env::join_paths([&cx.config.exedir, &cargo_home_bin])
                        .unwrap()
                        .to_str()
                        .unwrap(),
                )],
            )
            .await
            .extend_redactions([
                ("[CARGO_HOME_BIN]", &cargo_home_bin),
                ("[EXEDIR]", &cx.config.exedir),
            ]),
        snapbox::str![[r#"
[EXEDIR]
[CARGO_HOME_BIN]
...
"#]],
    );
}

#[tokio::test]
async fn rustup_failed_path_search() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    use std::env::consts::EXE_SUFFIX;

    let rustup_path = cx.config.exedir.join(format!("rustup{EXE_SUFFIX}"));
    let tool_path = cx.config.exedir.join(format!("fake_proxy{EXE_SUFFIX}"));
    utils::hardlink_file(&rustup_path, &tool_path).expect("Failed to create fake proxy for test");

    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "custom",
            &cx.config.customdir.join("custom-1").to_string_lossy(),
        ])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "default", "custom"])
        .await
        .is_ok();

    let broken = &["rustup", "run", "custom", "fake_proxy"];
    cx.config
        .expect(broken)
        .await
        .with_stderr(snapbox::str![[r#"
...
error: unknown proxy name: 'fake_proxy'; valid proxy names are 'rustc', 'rustdoc', 'cargo', 'rust-lldb', 'rust-gdb', 'rust-gdbgui', 'rls', 'cargo-clippy', 'clippy-driver', 'cargo-miri', 'rust-analyzer', 'rustfmt', 'cargo-fmt'
...
"#]])
        .is_err();

    // Hardlink will be automatically cleaned up by test setup code
}

#[tokio::test]
async fn rustup_failed_path_search_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    use std::env::consts::EXE_SUFFIX;

    let rustup_path = cx.config.exedir.join(format!("rustup{EXE_SUFFIX}"));
    let tool_path = cx.config.exedir.join(format!("cargo-miri{EXE_SUFFIX}"));
    utils::hardlink_file(&rustup_path, &tool_path)
        .expect("Failed to create fake cargo-miri for test");

    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "custom-1",
            &cx.config.customdir.join("custom-1").to_string_lossy(),
        ])
        .await
        .is_ok();

    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "custom-2",
            &cx.config.customdir.join("custom-2").to_string_lossy(),
        ])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "default", "custom-2"])
        .await
        .is_ok();

    let broken = &["rustup", "run", "custom-1", "cargo-miri"];
    cx.config
        .expect(broken)
        .await
        .with_stderr(snapbox::str![[r#"
...
note: this is a custom toolchain, which cannot use `rustup component add`
help: if you built this toolchain from source, and used `rustup toolchain link`, then you may be able to build the component with `x.py`
...
"#]])
        .is_err();

    let broken = &["rustup", "run", "custom-2", "cargo-miri"];
    cx.config
        .expect(broken)
        .await
        .with_stderr(snapbox::str![[r#"
...
note: this is a custom toolchain, which cannot use `rustup component add`
help: if you built this toolchain from source, and used `rustup toolchain link`, then you may be able to build the component with `x.py`
...
"#]])
        .is_err();

    // Hardlink will be automatically cleaned up by test setup code
}

#[tokio::test]
async fn rustup_run_not_installed() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "install", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "run", "nightly", "rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' is not installed
help: run `rustup toolchain install nightly-[HOST_TRIPLE]` to install it

"#]])
        .is_err();
}

#[tokio::test]
async fn rustup_run_install() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "install", "stable"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "run",
            "--install",
            "nightly",
            "cargo",
            "--version",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: installing component 'rustc'
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn toolchains_are_resolved_early() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    let full_toolchain = format!("nightly-{}", this_host_triple());
    cx.config
        .expect(["rustup", "default", &full_toolchain])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: using existing install for 'nightly-[HOST_TRIPLE]'
...
"#]])
        .is_ok();
}

// #190
#[tokio::test]
async fn proxies_pass_empty_args() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "run", "nightly", "rustc", "--empty-arg-test", ""])
        .await
        .is_ok();
}

#[tokio::test]
async fn rls_exists_in_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();

    assert!(cx.config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
    cx.config.expect(["rls", "--version"]).await.is_ok();
}

#[tokio::test]
async fn run_rls_when_not_available_in_toolchain() {
    let cx = CliTestContext::new(Scenario::UnavailableRls).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rls", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: the 'rls' component which provides the command 'rls[EXE]' is not available for the 'nightly-[HOST_TRIPLE]' toolchain

"#]])
        .is_err();

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect(["rustup", "update"]).await.is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();

    cx.config.expect(["rls", "--version"]).await.is_ok();
}

#[tokio::test]
async fn run_rls_when_not_installed() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rls", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: 'rls[EXE]' is not installed for the toolchain 'stable-[HOST_TRIPLE]'.
To install, run `rustup component add rls`

"#]])
        .is_err();
}

#[tokio::test]
async fn run_rls_when_not_installed_for_nightly() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rls", "+nightly", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: 'rls[EXE]' is not installed for the toolchain 'nightly-[HOST_TRIPLE]'.
To install, run `rustup component add --toolchain nightly-[HOST_TRIPLE] rls`

"#]])
        .is_err();
}

#[tokio::test]
async fn run_rust_lldb_when_not_in_toolchain() {
    let cx = CliTestContext::new(Scenario::UnavailableRls).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rust-lldb", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: the 'rust-lldb[EXE]' binary, normally provided by the 'rustc' component, is not applicable to the 'nightly-[HOST_TRIPLE]' toolchain

"#]])
        .is_err();
}

#[tokio::test]
async fn rename_rls_before() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect(["rustup", "update"]).await.is_ok();

    assert!(cx.config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
    cx.config.expect(["rls", "--version"]).await.is_ok();
}

#[tokio::test]
async fn rename_rls_after() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect(["rustup", "update"]).await.is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls-preview"])
        .await
        .is_ok();

    assert!(cx.config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
    cx.config.expect(["rls", "--version"]).await.is_ok();
}

#[tokio::test]
async fn rename_rls_add_old_name() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect(["rustup", "update"]).await.is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();

    assert!(cx.config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
    cx.config.expect(["rls", "--version"]).await.is_ok();
}

#[tokio::test]
async fn rename_rls_list() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect(["rustup", "update"]).await.is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rls-[HOST_TRIPLE] (installed)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn rename_rls_preview_list() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect(["rustup", "update"]).await.is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls-preview"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rls-[HOST_TRIPLE] (installed)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn rename_rls_remove() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect(["rustup", "update"]).await.is_ok();

    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config.expect(["rls", "--version"]).await.is_ok();
    cx.config
        .expect(["rustup", "component", "remove", "rls"])
        .await
        .is_ok();
    cx.config
        .expect(["rls", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: 'rls[EXE]' is not installed for the toolchain 'nightly-[HOST_TRIPLE]'.
...
"#]])
        .is_err();

    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config.expect(["rls", "--version"]).await.is_ok();
    cx.config
        .expect(["rustup", "component", "remove", "rls-preview"])
        .await
        .is_ok();
    cx.config
        .expect(["rls", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: 'rls[EXE]' is not installed for the toolchain 'nightly-[HOST_TRIPLE]'.
...
"#]])
        .is_err();
}

// issue #3737
/// `~/.rustup/toolchains` is permitted to be a symlink.
#[tokio::test]
#[cfg(any(unix, windows))]
async fn toolchains_symlink() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    let test_toolchains = cwd.join("toolchains-test");
    fs::create_dir(&test_toolchains).unwrap();
    symlink_dir(&test_toolchains, &cx.config.rustupdir.join("toolchains")).unwrap();

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
nightly[..]
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "uninstall", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
no installed toolchains

"#]])
        .is_ok();
}

// issue #3344
/// `~/.rustup/tmp` and `~/.rustup/downloads` are permitted to be symlinks.
#[tokio::test]
#[cfg(any(unix, windows))]
async fn tmp_downloads_symlink() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    let cwd = cx.config.current_dir();

    let test_tmp = cwd.join("tmp-test");
    fs::create_dir(&test_tmp).unwrap();
    symlink_dir(&test_tmp, &cx.config.rustupdir.join("tmp")).unwrap();

    let test_downloads = cwd.join("tmp-downloads");
    fs::create_dir(&test_downloads).unwrap();
    symlink_dir(&test_downloads, &cx.config.rustupdir.join("downloads")).unwrap();

    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect(["rustup", "update"]).await.is_ok();

    assert!(cx.config.rustupdir.join("tmp").exists());
    assert!(cx.config.rustupdir.join("downloads").exists());
}

// issue #1169
/// A toolchain that is a stale symlink should be correctly uninstalled.
#[tokio::test]
#[cfg(any(unix, windows))]
async fn toolchain_broken_symlink() {
    let cx = CliTestContext::new(Scenario::None).await;
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
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
test

"#]])
        .is_ok();
    // Now try to uninstall it.  That should work only once.
    cx.config
        .expect(["rustup", "toolchain", "uninstall", "test"])
        .await
        .with_stderr(snapbox::str![[r#"
info: uninstalling toolchain 'test'
info: toolchain 'test' uninstalled

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "uninstall", "test"])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: no toolchain installed for 'test'
...
"#]])
        .is_ok();
}

// issue #1297
#[tokio::test]
async fn update_unavailable_rustc() {
    let cx = CliTestContext::new(Scenario::Unavailable).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();

    // latest nightly is unavailable
    cx.config.set_current_dist_date("2015-01-02");
    // update should do nothing
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
}

// issue 2562
#[tokio::test]
async fn install_unavailable_platform() {
    let cx = CliTestContext::new(Scenario::Unavailable).await;
    cx.config.set_current_dist_date("2015-01-02");
    // explicit attempt to install should fail
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' is not installable
...
"#]])
        .is_err();
    // implicit attempt to install should fail
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' is not installable
...
"#]])
        .is_err();
}

// issue #1329
#[tokio::test]
async fn install_beta_with_tag() {
    let cx = CliTestContext::new(Scenario::BetaTag).await;
    cx.config
        .expect(["rustup", "default", "1.78.0-beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.78.0 (hash-1.78.0-beta-1.78.0)

"#]])
        .is_ok();

    cx.config
        .expect(["rustup", "default", "1.79.0-beta.2"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.79.0 (hash-1.79.0-beta.2-1.79.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn update_nightly_even_with_incompat() {
    let cx = CliTestContext::new(Scenario::MissingComponent).await;
    cx.config.set_current_dist_date("2019-09-12");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.37.0 (hash-nightly-1)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config.expect_component_executable("rls").await;

    // latest nightly is now one that does not have RLS
    cx.config.set_current_dist_date("2019-09-14");

    cx.config.expect_component_executable("rls").await;
    // update should bring us to latest nightly that does
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.37.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config.expect_component_executable("rls").await;
}

#[tokio::test]
async fn nightly_backtrack_skips_missing() {
    let cx = CliTestContext::new(Scenario::MissingNightly).await;
    cx.config.set_current_dist_date("2019-09-16");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.37.0 (hash-nightly-1)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config.expect_component_executable("rls").await;

    // rls is missing on latest, nightly is missing on second-to-latest
    cx.config.set_current_dist_date("2019-09-18");

    // update should not change nightly, and should not error
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.37.0 (hash-nightly-1)

"#]])
        .is_ok();
}

#[tokio::test]
async fn completion_rustup() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "completions", "bash", "rustup"])
        .await
        .is_ok();
}

#[tokio::test]
async fn completion_cargo() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "completions", "bash", "cargo"])
        .await
        .is_ok();
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
        .expect(["rustup", "completions", "fake"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: invalid value 'fake' for '<SHELL>'[..]
...
"#]])
        .is_err();
    cx.config
        .expect(["rustup", "completions", "fake", "cargo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: invalid value 'fake' for '<SHELL>'[..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn completion_bad_tool() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "completions", "bash", "fake"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: invalid value 'fake' for '[COMMAND]'[..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn completion_cargo_unsupported_shell() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "completions", "fish", "cargo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: cargo does not currently support completions for[..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn add_remove_component() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config.expect_component_executable("rustc").await;
    cx.config
        .expect(["rustup", "component", "remove", "rustc"])
        .await
        .is_ok();
    cx.config.expect_component_not_executable("rustc").await;
    cx.config
        .expect(["rustup", "component", "add", "rustc"])
        .await
        .is_ok();
    cx.config.expect_component_executable("rustc").await;
}

#[tokio::test]
async fn which() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path_1 = cx.config.customdir.join("custom-1");
    let path_1 = path_1.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "custom-1", &path_1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "custom-1"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "which", "rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/custom-1/bin/rustc[EXE]

"#]])
        .is_ok();
    let path_2 = cx.config.customdir.join("custom-2");
    let path_2 = path_2.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "custom-2", &path_2])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "which", "--toolchain=custom-2", "rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/custom-2/bin/rustc[EXE]

"#]])
        .is_ok();
}

#[tokio::test]
async fn which_asking_uninstalled_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path_1 = cx.config.customdir.join("custom-1");
    let path_1 = path_1.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "custom-1", &path_1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "custom-1"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "which", "rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/custom-1/bin/rustc[EXE]

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "which", "--toolchain=nightly", "rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/nightly-[HOST_TRIPLE]/bin/rustc[EXE]

"#]])
        .is_ok();
}

#[tokio::test]
async fn override_by_toolchain_on_the_command_line() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "stable", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "+stable", "which", "rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/stable-[HOST_TRIPLE]/bin/rustc[EXE]

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "which", "rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/nightly-[HOST_TRIPLE]/bin/rustc[EXE]

"#]])
        .is_ok();

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "+stable", "which", "rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/stable-[HOST_TRIPLE]/bin/rustc[EXE]

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "which", "rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/nightly-[HOST_TRIPLE]/bin/rustc[EXE]

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
active because: overridden by +toolchain on the command line
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+foo", "which", "rustc"])
        .await
        .with_stderr(snapbox::str![[r#"
error: override toolchain 'foo' is not installed: the +toolchain on the command line specifies an uninstalled toolchain

"#]])
        .is_err();
    cx.config
        .expect(["rustup", "+stable", "set", "profile", "minimal"])
        .await
        .with_stderr(snapbox::str![[r#"
info: profile set to 'minimal'

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE][..]

"#]])
        .is_ok();
}

#[tokio::test]
async fn toolchain_link_then_list_verbose() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path_1 = cx.config.customdir.join("custom-1");
    let path_1 = path_1.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "custom-1", &path_1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list", "-v"])
        .await
        .with_stdout(snapbox::str![[r#"
custom-1 [..]/custom-1

"#]])
        .is_ok();
}

#[tokio::test]
async fn update_self_smart_guess() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "self"])
        .await
        .is_err()
        .with_stderr(snapbox::str![[r#"
...
info: if you meant to update rustup itself, use `rustup self update`
...
"#]]);
}

#[tokio::test]
async fn uninstall_self_smart_guess() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "uninstall", "self"])
        .await
        .is_ok()
        .with_stderr(snapbox::str![[r#"
...
info: if you meant to uninstall rustup itself, use `rustup self uninstall`
...
"#]]);
}

// https://github.com/rust-lang/rustup/issues/4073
#[tokio::test]
async fn toolchain_install_multi_components_comma() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let components = ["rls", "rust-docs"];
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "install",
            "--profile=minimal",
            "--component",
            &components.join(","),
            "nightly",
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rls-[HOST_TRIPLE][..]
...
rust-docs-[HOST_TRIPLE][..]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn rustup_updates_cargo_env_if_proxy() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();

    use std::env::consts::EXE_SUFFIX;
    let proxy_path = cx
        .config
        .workdir
        .borrow()
        .join("bin")
        .join(format!("cargo{EXE_SUFFIX}"));
    let real_path = cx.config.expect(["rustup", "which", "cargo"]).await;
    real_path.is_ok();
    let real_path = &real_path.output.stdout;

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
        .expect(["cargo", "--echo-cargo-env"])
        .await
        .with_stderr(snapbox::str![[r#"
...
CARGO environment variable not set[..]
...
"#]])
        .is_err();

    // If CARGO is set to a proxy then change it to the real CARGO path
    cx.config
        .expect_with_env(
            ["cargo", "--echo-cargo-env"],
            [("CARGO", &*proxy_path.display().to_string())],
        )
        .await
        .extend_redactions([("[REAL_PATH]", real_path)])
        .with_stderr(snapbox::str![[r#"
[REAL_PATH]
"#]])
        .is_ok();
}

#[tokio::test]
async fn rust_analyzer_proxy_falls_back_external() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "install",
            "stable",
            "--profile=minimal",
            "--component=rls",
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();

    // We pretend to have a `rust-analyzer` installation by reusing the `rls`
    // proxy and mock binary.
    let rls = format!("rls{EXE_SUFFIX}");
    let ra = format!("rust-analyzer{EXE_SUFFIX}");
    let exedir = &cx.config.exedir;
    let bindir = &cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("stable-{}", this_host_triple()))
        .join("bin");
    for dir in [exedir, bindir] {
        fs::rename(dir.join(&rls), dir.join(&ra)).unwrap();
    }

    // Base case: rustup-hosted RA installed, external RA unavailable,
    // use the former.
    let real_path = cx
        .config
        .expect(["rust-analyzer", "--echo-current-exe"])
        .await;
    real_path.is_ok();
    let real_path = Path::new(real_path.output.stderr.trim());

    assert!(real_path.is_file());

    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let extern_dir = tempdir.path();
    let extern_path = &extern_dir.join("rust-analyzer");
    fs::copy(real_path, extern_path).unwrap();

    // First case: rustup-hosted and external RA both installed,
    // prioritize the former.
    cx.config
        .expect_with_env(
            ["rust-analyzer", "--echo-current-exe"],
            [("PATH", &*extern_dir.display().to_string())],
        )
        .await
        .extend_redactions([("[REAL_PATH]", real_path.to_owned())])
        .with_stderr(snapbox::str![[r#"
[REAL_PATH]

"#]])
        .is_ok();

    // Second case: rustup-hosted RA unavailable, fallback on the external RA.
    fs::remove_file(bindir.join(&ra)).unwrap();
    cx.config
        .expect_with_env(
            ["rust-analyzer", "--echo-current-exe"],
            [("PATH", &*extern_dir.display().to_string())],
        )
        .await
        .extend_redactions([("[EXTERN_PATH]", extern_path)])
        .with_stderr(snapbox::str![[r#"
info: `rust-analyzer` is unavailable for the active toolchain
info: falling back to "[EXTERN_PATH]"
[EXTERN_PATH]

"#]])
        .is_ok();
}
