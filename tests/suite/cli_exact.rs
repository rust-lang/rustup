//! Yet more cli test cases. These are testing that the output
//! is exactly as expected.

use rustup::test::{
    CROSS_ARCH1, CROSS_ARCH2, CliTestContext, MULTI_ARCH1, Scenario, this_host_triple,
};
use rustup::utils::raw;

#[tokio::test]
async fn update_once() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] installed - 1.3.0 (hash-nightly-2)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'nightly-[HOST_TRIPLE]'

"#]]);
}

#[tokio::test]
async fn update_once_and_check_self_update() {
    let test_version = "2.0.0";
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let _dist_guard = cx.with_update_server(test_version);
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "set", "auto-self-update", "check-only"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .extend_redactions([("[TEST_VERSION]", test_version)])
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] installed - 1.3.0 (hash-nightly-2)

rustup - Update available : [CURRENT_VERSION] -> [TEST_VERSION]

"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'

"#]]);
}

#[tokio::test]
async fn update_once_and_self_update() {
    let test_version = "2.0.0";
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let _dist_guard = cx.with_update_server(test_version);
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "set", "auto-self-update", "enable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .extend_redactions([("[TEST_VERSION]", test_version)])
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] installed - 1.3.0 (hash-nightly-2)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: checking for self-update (current version: [CURRENT_VERSION])
info: downloading self-update (new version: [TEST_VERSION])

"#]]);
}

#[tokio::test]
async fn update_again() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "upgrade", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] unchanged - 1.3.0 (hash-nightly-2)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'

"#]]);
    cx.config
        .expect(["rustup", "upgrade", "nightly"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] unchanged - 1.3.0 (hash-nightly-2)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'

"#]]);
}

#[tokio::test]
async fn check_updates_none() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "add", "stable", "beta", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "check"])
        .await
        .is_err()
        .with_stdout(snapbox::str![[r#"
stable-[HOST_TRIPLE] - Up to date : 1.1.0 (hash-stable-1.1.0)
beta-[HOST_TRIPLE] - Up to date : 1.2.0 (hash-beta-1.2.0)
nightly-[HOST_TRIPLE] - Up to date : 1.3.0 (hash-nightly-2)

"#]]);
}

#[tokio::test]
async fn check_updates_some() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect(["rustup", "toolchain", "add", "stable", "beta", "nightly"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect(["rustup", "check"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
stable-[HOST_TRIPLE] - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-[HOST_TRIPLE] - Update available : 1.1.0 (hash-beta-1.1.0) -> 1.2.0 (hash-beta-1.2.0)
nightly-[HOST_TRIPLE] - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)

"#]]);
}

#[tokio::test]
async fn check_updates_self() {
    let test_version = "2.0.0";
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let _dist_guard = cx.with_update_server(test_version);

    // We are checking an update to rustup itself in this test.
    cx.config
        .run("rustup", ["set", "auto-self-update", "enable"], &[])
        .await;

    cx.config
        .expect(["rustup", "check"])
        .await
        .extend_redactions([("[TEST_VERSION]", test_version)])
        .is_ok()
        .with_stdout(snapbox::str![[r#"
rustup - Update available : [CURRENT_VERSION] -> [TEST_VERSION]

"#]]);
}

#[tokio::test]
async fn check_updates_self_no_change() {
    let current_version = env!("CARGO_PKG_VERSION");
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let _dist_guard = cx.with_update_server(current_version);

    // We are checking an update to rustup itself in this test.
    cx.config
        .run("rustup", ["set", "auto-self-update", "enable"], &[])
        .await;

    cx.config
        .expect(["rustup", "check"])
        .await
        .is_err()
        .with_stdout(snapbox::str![[r#"
rustup - Up to date : [CURRENT_VERSION]

"#]]);
}

#[tokio::test]
async fn check_updates_with_update() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect(["rustup", "toolchain", "add", "stable", "beta", "nightly"])
            .await
            .is_ok();
        cx.config
            .expect(["rustup", "check"])
            .await
            .is_err()
            .with_stdout(snapbox::str![[r#"
stable-[HOST_TRIPLE] - Up to date : 1.0.0 (hash-stable-1.0.0)
beta-[HOST_TRIPLE] - Up to date : 1.1.0 (hash-beta-1.1.0)
nightly-[HOST_TRIPLE] - Up to date : 1.2.0 (hash-nightly-1)

"#]]);
    }

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect(["rustup", "check"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
stable-[HOST_TRIPLE] - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-[HOST_TRIPLE] - Update available : 1.1.0 (hash-beta-1.1.0) -> 1.2.0 (hash-beta-1.2.0)
nightly-[HOST_TRIPLE] - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)

"#]]);
    cx.config.expect(["rustup", "update", "beta"]).await.is_ok();
    cx.config
        .expect(["rustup", "check"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
stable-[HOST_TRIPLE] - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-[HOST_TRIPLE] - Up to date : 1.2.0 (hash-beta-1.2.0)
nightly-[HOST_TRIPLE] - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)

"#]]);
}

#[tokio::test]
async fn default() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] installed - 1.3.0 (hash-nightly-2)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'nightly-[HOST_TRIPLE]'

"#]]);
}

#[tokio::test]
async fn override_again() {
    let cx = &CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .extend_redactions([("[CWD]", cx.config.current_dir().display().to_string())])
        .is_ok()
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
info: override toolchain for '[CWD]' set to 'nightly-[HOST_TRIPLE]'

"#]]);
}

#[tokio::test]
async fn remove_override() {
    for keyword in &["remove", "unset"] {
        let cx = CliTestContext::new(Scenario::SimpleV2).await;
        let cwd = cx.config.current_dir();
        cx.config
            .expect(["rustup", "override", "add", "nightly"])
            .await
            .is_ok();
        cx.config
            .expect(["rustup", "override", keyword])
            .await
            .extend_redactions([("[CWD]", cwd.display().to_string())])
            .is_ok()
            .with_stdout(snapbox::str![[""]])
            .with_stderr(snapbox::str![[r#"
info: override toolchain for '[CWD]' removed

"#]]);
    }
}

#[tokio::test]
async fn remove_override_none() {
    for keyword in &["remove", "unset"] {
        let cx = CliTestContext::new(Scenario::SimpleV2).await;
        let cwd = cx.config.current_dir();
        cx.config
            .expect(["rustup", "override", keyword])
            .await
            .extend_redactions([("[CWD]", cwd.display().to_string())])
            .is_ok()
            .with_stdout(snapbox::str![[""]])
            .with_stderr(snapbox::str![[r#"
info: no override toolchain for '[CWD]'
info: you may use `--path <path>` option to remove override toolchain for a specific path

"#]]);
    }
}

#[tokio::test]
async fn remove_override_with_path() {
    for keyword in &["remove", "unset"] {
        let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
        let dir = tempfile::Builder::new()
            .prefix("rustup-test")
            .tempdir()
            .unwrap();

        {
            let cx = cx.change_dir(dir.path());
            cx.config
                .expect(["rustup", "override", "add", "nightly"])
                .await
                .is_ok();
        }

        cx.config
            .expect([
                "rustup",
                "override",
                keyword,
                "--path",
                dir.path().to_str().unwrap(),
            ])
            .await
            .extend_redactions([("[PATH]", dir.path().display().to_string())])
            .is_ok()
            .with_stdout(snapbox::str![[""]])
            .with_stderr(snapbox::str![[r#"
info: override toolchain for '[PATH]' removed

"#]]);
    }
}

#[tokio::test]
async fn remove_override_with_path_deleted() {
    for keyword in &["remove", "unset"] {
        let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
        let path = {
            let dir = tempfile::Builder::new()
                .prefix("rustup-test")
                .tempdir()
                .unwrap();
            let path = std::fs::canonicalize(dir.path()).unwrap();
            let cx = cx.change_dir(&path);
            cx.config
                .expect(["rustup", "override", "add", "nightly"])
                .await
                .is_ok();
            path
        };
        cx.config
            .expect([
                "rustup",
                "override",
                keyword,
                "--path",
                path.to_str().unwrap(),
            ])
            .await
            .extend_redactions([("[PATH]", path.display().to_string())])
            .is_ok()
            .with_stdout(snapbox::str![[""]])
            .with_stderr(snapbox::str![[r#"
info: override toolchain for '[PATH]' removed

"#]]);
    }
}

#[tokio::test]
#[cfg_attr(target_os = "windows", ignore)] // FIXME #1103
async fn remove_override_nonexistent() {
    for keyword in &["remove", "unset"] {
        let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
        let path = {
            let dir = tempfile::Builder::new()
                .prefix("rustup-test")
                .tempdir()
                .unwrap();
            let path = std::fs::canonicalize(dir.path()).unwrap();
            let cx = cx.change_dir(&path);
            cx.config
                .expect(["rustup", "override", "add", "nightly"])
                .await
                .is_ok();
            path
        };
        // FIXME TempDir seems to succumb to difficulties removing dirs on windows
        let _ = rustup::utils::raw::remove_dir(&path);
        assert!(!path.exists());
        cx.config
            .expect(["rustup", "override", keyword, "--nonexistent"])
            .await
            .extend_redactions([("[PATH]", path.display().to_string())])
            .is_ok()
            .with_stdout(snapbox::str![[""]])
            .with_stderr(snapbox::str![[r#"
info: override toolchain for '[PATH]' removed

"#]]);
    }
}

#[tokio::test]
async fn list_overrides() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = std::fs::canonicalize(cx.config.current_dir()).unwrap();
    let mut cwd_formatted = format!("{}", cwd.display());

    if cfg!(windows) {
        cwd_formatted.drain(..4);
    }

    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "list"])
        .await
        .extend_redactions([("[CWD]", cwd_formatted)])
        .is_ok()
        .with_stdout(snapbox::str![[r#"
[CWD]	nightly-[HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]]);
}

#[tokio::test]
async fn list_overrides_with_nonexistent() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;

    let nonexistent_path = {
        let dir = tempfile::Builder::new()
            .prefix("rustup-test")
            .tempdir()
            .unwrap();
        let cx = cx.change_dir(dir.path());
        cx.config
            .expect(["rustup", "override", "add", "nightly"])
            .await
            .is_ok();
        std::fs::canonicalize(dir.path()).unwrap()
    };
    // FIXME TempDir seems to succumb to difficulties removing dirs on windows
    let _ = rustup::utils::raw::remove_dir(&nonexistent_path);
    assert!(!nonexistent_path.exists());
    let mut path_formatted = format!("{}", nonexistent_path.display());

    if cfg!(windows) {
        path_formatted.drain(..4);
    }

    cx.config
        .expect(["rustup", "override", "list"])
        .await
        .extend_redactions([("[PATH]", path_formatted + " (not a directory)")])
        .is_ok()
        .with_stdout(snapbox::str![[r#"
[PATH]	nightly-[HOST_TRIPLE]


"#]])
        .with_stderr(snapbox::str![[r#"
info: you may remove overrides for non-existent directories with
`rustup override unset --nonexistent`

"#]]);
}

#[tokio::test]
async fn update_no_manifest() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly-2016-01-01"])
        .await
        .is_err()
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-2016-01-01-[HOST_TRIPLE]'
error: no release found for 'nightly-2016-01-01'

"#]]);
}

// Issue #111
#[tokio::test]
async fn update_custom_toolchain() {
    let cx = CliTestContext::new(Scenario::None).await;
    // installable toolchains require 2 digits in the DD and MM fields, so this is
    // treated as a custom toolchain, which can't be used with update.
    cx.config
        .expect(["rustup", "update", "nightly-2016-03-1"])
        .await
        .is_err()
        .with_stderr(snapbox::str![[r#"
...
error: [..]invalid toolchain name: 'nightly-2016-03-1'
...
"#]]);
}

#[tokio::test]
async fn default_custom_not_installed_toolchain() {
    let cx = CliTestContext::new(Scenario::None).await;
    // installable toolchains require 2 digits in the DD and MM fields, so this is
    // treated as a custom toolchain, which isn't installed.
    cx.config
        .expect(["rustup", "default", "nightly-2016-03-1"])
        .await
        .is_err()
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-2016-03-1' is not installed

"#]]);
}

#[tokio::test]
async fn default_none() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect(["rustup", "default", "none"])
        .await
        .is_ok()
        .with_stderr(snapbox::str![[r#"
info: default toolchain unset

"#]]);

    cx.config
        .expect(["rustup", "default"])
        .await
        .is_err()
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
error: no default toolchain is configured

"#]]);

    cx.config
        .expect(["rustc", "--version"])
        .await
        .is_err()
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
error: rustup could not choose a version of rustc to run, because one wasn't specified explicitly, and no default is configured.
help: run 'rustup default stable' to download the latest stable release of Rust and set it as your default toolchain.

"#]]);
}

#[tokio::test]
async fn list_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    let mut sorted = [
        format!("{} (installed)", &*trip),
        format!("{CROSS_ARCH1} (installed)"),
        CROSS_ARCH2.to_string(),
    ];
    sorted.sort();

    let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .extend_redactions([("[EXPECTED]", expected)])
        .is_ok()
        .with_stdout(snapbox::str![[r#"
[EXPECTED]"#]])
        .with_stderr(snapbox::str![[""]]);
}

#[tokio::test]
async fn list_targets_quiet() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    let mut sorted = [trip, CROSS_ARCH1.to_string(), CROSS_ARCH2.to_string()];
    sorted.sort();

    let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list", "--quiet"])
        .await
        .extend_redactions([("[EXPECTED]", expected)])
        .is_ok()
        .with_stdout(snapbox::str![[r#"
[EXPECTED]"#]])
        .with_stderr(snapbox::str![[""]]);
}

#[tokio::test]
async fn list_installed_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    let mut sorted = [trip, CROSS_ARCH1.to_string(), CROSS_ARCH2.to_string()];
    sorted.sort();

    let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH2])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list", "--installed"])
        .await
        .extend_redactions([("[EXPECTED]", expected)])
        .is_ok()
        .with_stdout(snapbox::str![[r#"
[EXPECTED]"#]])
        .with_stderr(snapbox::str![[""]]);
}

#[tokio::test]
async fn cross_install_indicates_target() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    // TODO error 'nightly-x86_64-apple-darwin' is not installed
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
info: downloading component 'rust-std' for '[CROSS_ARCH_I]'
info: installing component 'rust-std' for '[CROSS_ARCH_I]'

"#]]);
}

// issue #3573
#[tokio::test]
async fn show_suggestion_for_missing_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(["cargo", "+nightly", "fmt"], [("RUSTUP_AUTO_INSTALL", "0")])
        .await
        .is_err()
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' is not installed
help: run `rustup toolchain install nightly-[HOST_TRIPLE]` to install it

"#]]);
}

// issue #4212
#[tokio::test]
async fn show_suggestion_for_missing_toolchain_with_components() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
channel = "stable"
components = [ "rust-src" ]
"#,
    )
    .unwrap();
    cx.config
        .expect_with_env(["cargo", "fmt"], [("RUSTUP_AUTO_INSTALL", "0")])
        .await
        .is_err()
        .with_stderr(snapbox::str![[r#"
error: toolchain 'stable-[HOST_TRIPLE]' is not installed
help: run `rustup toolchain install` to install it

"#]]);
}

// issue #927
#[tokio::test]
async fn undefined_linked_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["cargo", "+bogus", "test"])
        .await
        .is_err()
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
error: toolchain 'bogus' is not installed

"#]]);
}

#[tokio::test]
async fn install_by_version_number() {
    let cx = CliTestContext::new(Scenario::ArchivesV2TwoVersions).await;
    cx.config
        .expect(["rustup", "toolchain", "add", "0.100.99"])
        .await
        .is_ok();
}

// issue #2191
#[tokio::test]
async fn install_unreleased_component() {
    let cx = CliTestContext::new(Scenario::MissingComponentMulti).await;
    // Initial channel content is host + rls + multiarch-std
    cx.config.set_current_dist_date("2019-09-12");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", MULTI_ARCH1])
        .await
        .is_ok();

    // Next channel variant should have host + rls but not multiarch-std
    cx.config.set_current_dist_date("2019-09-13");
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] unchanged - 1.37.0 (hash-nightly-1)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2019-09-13, rust version 1.37.0 (hash-nightly-2)
info: skipping nightly which is missing installed component 'rust-std-[MULTI_ARCH_I]'
info: syncing channel updates for 'nightly-2019-09-12-[HOST_TRIPLE]'

"#]]);

    // Next channel variant should have host + multiarch-std but have rls missing
    cx.config.set_current_dist_date("2019-09-14");
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] unchanged - 1.37.0 (hash-nightly-1)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2019-09-14, rust version 1.37.0 (hash-nightly-3)
info: skipping nightly which is missing installed component 'rls'
info: syncing channel updates for 'nightly-2019-09-13-[HOST_TRIPLE]'
info: latest update on 2019-09-13, rust version 1.37.0 (hash-nightly-2)
info: skipping nightly which is missing installed component 'rust-std-[MULTI_ARCH_I]'
info: syncing channel updates for 'nightly-2019-09-12-[HOST_TRIPLE]'

"#]]);
}
