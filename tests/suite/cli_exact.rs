//! Yet more cli test cases. These are testing that the output
//! is exactly as expected.

#![allow(deprecated)]

use rustup::for_host;
use rustup::test::{
    CROSS_ARCH1, CROSS_ARCH2, CliTestContext, MULTI_ARCH1, Scenario, this_host_triple,
};
use rustup::utils::raw;

#[tokio::test]
async fn update_once() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "update", "nightly"],
            for_host!(
                r"
  nightly-{0} installed - 1.3.0 (hash-nightly-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'nightly-{0}'
"
            ),
        )
        .await;
}

#[tokio::test]
async fn update_once_and_check_self_update() {
    let test_version = "2.0.0";
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let _dist_guard = cx.with_update_server(test_version);
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config
        .expect_ok(&["rustup", "set", "auto-self-update", "check-only"])
        .await;
    let current_version = env!("CARGO_PKG_VERSION");

    cx.config
        .expect_ok_ex(
            &["rustup", "update", "nightly"],
            &format!(
                r"
  nightly-{} installed - 1.3.0 (hash-nightly-2)

rustup - Update available : {} -> {}
",
                &this_host_triple(),
                current_version,
                test_version
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
"
            ),
        )
        .await;
}

#[tokio::test]
async fn update_once_and_self_update() {
    let test_version = "2.0.0";
    let current = env!("CARGO_PKG_VERSION");
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let _dist_guard = cx.with_update_server(test_version);
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config
        .expect_ok(&["rustup", "set", "auto-self-update", "enable"])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "update", "nightly"],
            for_host!(
                r"
  nightly-{0} installed - 1.3.0 (hash-nightly-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: checking for self-update (current version: {current})
info: downloading self-update (new version: 2.0.0)
"
            ),
        )
        .await;
}

#[tokio::test]
async fn update_again() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "upgrade", "nightly"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "update", "nightly"],
            for_host!(
                r"
  nightly-{0} unchanged - 1.3.0 (hash-nightly-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
"
            ),
        )
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "upgrade", "nightly"],
            for_host!(
                r"
  nightly-{0} unchanged - 1.3.0 (hash-nightly-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
"
            ),
        )
        .await;
}

#[tokio::test]
async fn check_updates_none() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"])
        .await;
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
        let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"])
            .await;
    }

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config.expect_stdout_ok(
        &["rustup", "check"],
        for_host!(
            r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Update available : 1.1.0 (hash-beta-1.1.0) -> 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
        ),
    ).await;
}

#[tokio::test]
async fn check_updates_self() {
    let test_version = "2.0.0";
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let _dist_guard = cx.with_update_server(test_version);
    let current_version = env!("CARGO_PKG_VERSION");

    // We are checking an update to rustup itself in this test.
    cx.config
        .run("rustup", ["set", "auto-self-update", "enable"], &[])
        .await;

    cx.config
        .expect_stdout_ok(
            &["rustup", "check"],
            &format!(
                r"rustup - Update available : {current_version} -> {test_version}
"
            ),
        )
        .await;
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
        .extend_redactions([("[VERSION]", current_version)])
        .is_err()
        .with_stdout(snapbox::str![[r#"
rustup - Up to date : [VERSION]

"#]]);
}

#[tokio::test]
async fn check_updates_with_update() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"])
            .await;
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

    let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config.expect_stdout_ok(
        &["rustup", "check"],
        for_host!(
            r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Update available : 1.1.0 (hash-beta-1.1.0) -> 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
        ),
    ).await;
    cx.config.expect_ok(&["rustup", "update", "beta"]).await;
    cx.config.expect_stdout_ok(
        &["rustup", "check"],
        for_host!(
            r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Up to date : 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
        ),
    ).await;
}

#[tokio::test]
async fn default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "default", "nightly"],
            for_host!(
                r"
  nightly-{0} installed - 1.3.0 (hash-nightly-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'nightly-{0}'
"
            ),
        )
        .await;
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
        .with_stdout("")
        .with_stderr(snapbox::str![[r#"
info: override toolchain for '[CWD]' set to 'nightly-[HOST_TRIPLE]'

"#]]);
}

#[tokio::test]
async fn remove_override() {
    for keyword in &["remove", "unset"] {
        let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
        let cwd = cx.config.current_dir();
        cx.config
            .expect_ok(&["rustup", "override", "add", "nightly"])
            .await;
        cx.config
            .expect_ok_ex(
                &["rustup", "override", keyword],
                r"",
                &format!("info: override toolchain for '{}' removed\n", cwd.display()),
            )
            .await;
    }
}

#[tokio::test]
async fn remove_override_none() {
    for keyword in &["remove", "unset"] {
        let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
        let cwd = cx.config.current_dir();
        cx.config
            .expect_ok_ex(
                &["rustup", "override", keyword],
                r"",
                &format!(
                    "info: no override toolchain for '{}'
info: you may use `--path <path>` option to remove override toolchain for a specific path\n",
                    cwd.display()
                ),
            )
            .await;
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
            let mut cx = cx.change_dir(dir.path());
            cx.config
                .expect_ok(&["rustup", "override", "add", "nightly"])
                .await;
        }

        cx.config
            .expect_ok_ex(
                &[
                    "rustup",
                    "override",
                    keyword,
                    "--path",
                    dir.path().to_str().unwrap(),
                ],
                r"",
                &format!(
                    "info: override toolchain for '{}' removed\n",
                    dir.path().display()
                ),
            )
            .await;
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
            let mut cx = cx.change_dir(&path);
            cx.config
                .expect_ok(&["rustup", "override", "add", "nightly"])
                .await;
            path
        };
        cx.config
            .expect_ok_ex(
                &[
                    "rustup",
                    "override",
                    keyword,
                    "--path",
                    path.to_str().unwrap(),
                ],
                r"",
                &format!(
                    "info: override toolchain for '{}' removed\n",
                    path.display()
                ),
            )
            .await;
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
            let mut cx = cx.change_dir(&path);
            cx.config
                .expect_ok(&["rustup", "override", "add", "nightly"])
                .await;
            path
        };
        // FIXME TempDir seems to succumb to difficulties removing dirs on windows
        let _ = rustup::utils::raw::remove_dir(&path);
        assert!(!path.exists());
        cx.config
            .expect_ok_ex(
                &["rustup", "override", keyword, "--nonexistent"],
                r"",
                &format!(
                    "info: override toolchain for '{}' removed\n",
                    path.display()
                ),
            )
            .await;
    }
}

#[tokio::test]
async fn list_overrides() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = std::fs::canonicalize(cx.config.current_dir()).unwrap();
    let mut cwd_formatted = format!("{}", cwd.display());

    if cfg!(windows) {
        cwd_formatted.drain(..4);
    }

    let trip = this_host_triple();
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "override", "list"],
            &format!(
                "{:<40}\t{:<20}\n",
                cwd_formatted,
                &format!("nightly-{trip}")
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn list_overrides_with_nonexistent() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();

    let nonexistent_path = {
        let dir = tempfile::Builder::new()
            .prefix("rustup-test")
            .tempdir()
            .unwrap();
        let mut cx = cx.change_dir(dir.path());
        cx.config
            .expect_ok(&["rustup", "override", "add", "nightly"])
            .await;
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
        .expect_ok_ex(
            &["rustup", "override", "list"],
            &format!(
                "{:<40}\t{:<20}\n\n",
                path_formatted + " (not a directory)",
                &format!("nightly-{trip}")
            ),
            "info: you may remove overrides for non-existent directories with
`rustup override unset --nonexistent`\n",
        )
        .await;
}

#[tokio::test]
async fn update_no_manifest() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err_ex(
            &["rustup", "update", "nightly-2016-01-01"],
            r"",
            for_host!(
                r"info: syncing channel updates for 'nightly-2016-01-01-{0}'
error: no release found for 'nightly-2016-01-01'
"
            ),
        )
        .await;
}

// Issue #111
#[tokio::test]
async fn update_custom_toolchain() {
    let cx = CliTestContext::new(Scenario::None).await;
    // installable toolchains require 2 digits in the DD and MM fields, so this is
    // treated as a custom toolchain, which can't be used with update.
    cx.config
        .expect_err(
            &["rustup", "update", "nightly-2016-03-1"],
            "invalid toolchain name: 'nightly-2016-03-1'",
        )
        .await;
}

#[tokio::test]
async fn default_custom_not_installed_toolchain() {
    let cx = CliTestContext::new(Scenario::None).await;
    // installable toolchains require 2 digits in the DD and MM fields, so this is
    // treated as a custom toolchain, which isn't installed.
    cx.config
        .expect_err(
            &["rustup", "default", "nightly-2016-03-1"],
            "toolchain 'nightly-2016-03-1' is not installed",
        )
        .await;
}

#[tokio::test]
async fn default_none() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect_stderr_ok(
            &["rustup", "default", "none"],
            "info: default toolchain unset",
        )
        .await;

    cx.config
        .expect_err_ex(
            &["rustup", "default"],
            "",
            "error: no default toolchain is configured\n",
        )
        .await;

    cx.config.expect_err_ex(
            &["rustc", "--version"],
            "",
            "error: rustup could not choose a version of rustc to run, because one wasn't specified explicitly, and no default is configured.
help: run 'rustup default stable' to download the latest stable release of Rust and set it as your default toolchain.
",
        ).await;
}

#[tokio::test]
async fn list_targets() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    let mut sorted = [
        format!("{} (installed)", &*trip),
        format!("{CROSS_ARCH1} (installed)"),
        CROSS_ARCH2.to_string(),
    ];
    sorted.sort();

    let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH1])
        .await;
    cx.config
        .expect_ok_ex(&["rustup", "target", "list"], &expected, r"")
        .await;
}

#[tokio::test]
async fn list_targets_quiet() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    let mut sorted = [trip, CROSS_ARCH1.to_string(), CROSS_ARCH2.to_string()];
    sorted.sort();

    let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH1])
        .await;
    cx.config
        .expect_ok_ex(&["rustup", "target", "list", "--quiet"], &expected, r"")
        .await;
}

#[tokio::test]
async fn list_installed_targets() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    let mut sorted = [trip, CROSS_ARCH1.to_string(), CROSS_ARCH2.to_string()];
    sorted.sort();

    let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH1])
        .await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH2])
        .await;
    cx.config
        .expect_ok_ex(&["rustup", "target", "list", "--installed"], &expected, r"")
        .await;
}

#[tokio::test]
async fn cross_install_indicates_target() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    // TODO error 'nightly-x86_64-apple-darwin' is not installed
    cx.config
        .expect_ok_ex(
            &["rustup", "target", "add", CROSS_ARCH1],
            r"",
            &format!(
                r"info: downloading component 'rust-std' for '{CROSS_ARCH1}'
info: installing component 'rust-std' for '{CROSS_ARCH1}'
"
            ),
        )
        .await;
}

// issue #3573
#[tokio::test]
async fn show_suggestion_for_missing_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err_env(
            &["cargo", "+nightly", "fmt"],
            &[("RUSTUP_AUTO_INSTALL", "0")],
            for_host!(
                r"error: toolchain 'nightly-{0}' is not installed
help: run `rustup toolchain install nightly-{0}` to install it
"
            ),
        )
        .await;
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
        .expect_err_env(
            &["cargo", "fmt"],
            &[("RUSTUP_AUTO_INSTALL", "0")],
            for_host!(
                r"error: toolchain 'stable-{0}' is not installed
help: run `rustup toolchain install` to install it
"
            ),
        )
        .await;
}

// issue #927
#[tokio::test]
async fn undefined_linked_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err_ex(
            &["cargo", "+bogus", "test"],
            r"",
            "error: toolchain 'bogus' is not installed\n",
        )
        .await;
}

#[tokio::test]
async fn install_by_version_number() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2TwoVersions).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "add", "0.100.99"])
        .await;
}

// issue #2191
#[tokio::test]
async fn install_unreleased_component() {
    let mut cx = CliTestContext::new(Scenario::MissingComponentMulti).await;
    // Initial channel content is host + rls + multiarch-std
    cx.config.set_current_dist_date("2019-09-12");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;
    cx.config
        .expect_ok(&["rustup", "target", "add", MULTI_ARCH1])
        .await;

    // Next channel variant should have host + rls but not multiarch-std
    cx.config.set_current_dist_date("2019-09-13");
    cx.config
        .expect_ok_ex(
            &["rustup", "update", "nightly"],
            for_host!(
                r"
  nightly-{} unchanged - 1.37.0 (hash-nightly-1)

"
            ),
            &format!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2019-09-13, rust version 1.37.0 (hash-nightly-2)
info: skipping nightly which is missing installed component 'rust-std-{1}'
info: syncing channel updates for 'nightly-2019-09-12-{0}'
",
                this_host_triple(),
                MULTI_ARCH1
            ),
        )
        .await;

    // Next channel variant should have host + multiarch-std but have rls missing
    cx.config.set_current_dist_date("2019-09-14");
    cx.config
        .expect_ok_ex(
            &["rustup", "update", "nightly"],
            for_host!(
                r"
  nightly-{} unchanged - 1.37.0 (hash-nightly-1)

"
            ),
            &format!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2019-09-14, rust version 1.37.0 (hash-nightly-3)
info: skipping nightly which is missing installed component 'rls'
info: syncing channel updates for 'nightly-2019-09-13-{0}'
info: latest update on 2019-09-13, rust version 1.37.0 (hash-nightly-2)
info: skipping nightly which is missing installed component 'rust-std-{1}'
info: syncing channel updates for 'nightly-2019-09-12-{0}'
",
                this_host_triple(),
                MULTI_ARCH1,
            ),
        )
        .await;
}
