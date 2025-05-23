//! Test cases of the rustup command, using v1 manifests, mostly
//! derived from multirust/test-v2.sh

#![allow(deprecated)]

use std::fs;

use rustup::for_host;
use rustup::test::{CliTestContext, Scenario};

#[tokio::test]
async fn rustc_no_default_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustc"])
        .await
        .is_err()
        .with_stderr(snapbox::str![[r#"
error: rustup could not choose a version of rustc to run, because one wasn't specified explicitly, and no default is configured.
help: run 'rustup default stable' to download the latest stable release of Rust and set it as your default toolchain.

"#]]);
}

#[tokio::test]
async fn expected_bins_exist() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "1.3.0")
        .await;
}

#[tokio::test]
async fn install_toolchain_from_channel() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
    cx.config.expect_ok(&["rustup", "default", "beta"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
        .await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;
}

#[tokio::test]
async fn install_toolchain_from_archive() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV1).await;
    cx.config
        .expect_ok(&["rustup", "default", "nightly-2015-01-01"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
    cx.config
        .expect_ok(&["rustup", "default", "beta-2015-01-01"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.1.0")
        .await;
    cx.config
        .expect_ok(&["rustup", "default", "stable-2015-01-01"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.0.0")
        .await;
}

#[tokio::test]
async fn install_toolchain_from_version() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config.expect_ok(&["rustup", "default", "1.1.0"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;
}

#[tokio::test]
async fn default_existing_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stderr_ok(
            &["rustup", "default", "nightly"],
            for_host!("using existing install for 'nightly-{0}'"),
        )
        .await;
}

#[tokio::test]
async fn update_channel() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV1).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn list_toolchains() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV1).await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "update", "beta-2015-01-01"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "nightly")
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list", "-v"], "(active, default) ")
        .await;
    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(
            &["rustup", "toolchain", "list", "-v"],
            for_host!(r"\toolchains\nightly-{}"),
        )
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(
            &["rustup", "toolchain", "list", "-v"],
            for_host!("/toolchains/nightly-{}"),
        )
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "beta-2015-01-01")
        .await;
    #[cfg(windows)]
    cx.config
        .expect_stdout_ok(
            &["rustup", "toolchain", "list", "-v"],
            r"\toolchains\beta-2015-01-01",
        )
        .await;
    #[cfg(not(windows))]
    cx.config
        .expect_stdout_ok(
            &["rustup", "toolchain", "list", "-v"],
            "/toolchains/beta-2015-01-01",
        )
        .await;
}

#[tokio::test]
async fn list_toolchains_with_none() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "no installed toolchains")
        .await;
}

#[tokio::test]
async fn remove_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "nightly"])
        .await;
    cx.config.expect_ok(&["rustup", "toolchain", "list"]).await;
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "no installed toolchains")
        .await;
}

#[tokio::test]
async fn remove_override_toolchain_err_handling() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mut cx = cx.change_dir(tempdir.path());
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "beta"])
        .await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "beta"])
        .await;
    cx.config
        .expect_ok_contains(
            &["rustc", "--version"],
            "1.2.0 (hash-beta-1.2.0)",
            "info: downloading component 'rust'",
        )
        .await;
}

#[tokio::test]
async fn bad_sha_on_manifest() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    let sha_file = cx
        .config
        .distdir
        .as_ref()
        .unwrap()
        .join("dist/channel-rust-nightly.sha256");
    let sha_str = fs::read_to_string(&sha_file).unwrap();
    let mut sha_bytes = sha_str.into_bytes();
    sha_bytes[..10].clone_from_slice(b"aaaaaaaaaa");
    let sha_str = String::from_utf8(sha_bytes).unwrap();
    rustup::utils::raw::write_file(&sha_file, &sha_str).unwrap();
    cx.config
        .expect_err(&["rustup", "default", "nightly"], "checksum failed")
        .await;
}

#[tokio::test]
async fn bad_sha_on_installer() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    let dir = cx.config.distdir.as_ref().unwrap().join("dist");
    for file in fs::read_dir(dir).unwrap() {
        let file = file.unwrap();
        let path = file.path();
        let filename = path.to_string_lossy();
        if filename.ends_with(".tar.gz") || filename.ends_with(".tar.xz") {
            rustup::utils::raw::write_file(&path, "xxx").unwrap();
        }
    }
    cx.config
        .expect_err(&["rustup", "default", "nightly"], "checksum failed")
        .await;
}

#[tokio::test]
async fn install_override_toolchain_from_channel() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "beta"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
        .await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "stable"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;
}

#[tokio::test]
async fn install_override_toolchain_from_archive() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV1).await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly-2015-01-01"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "beta-2015-01-01"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.1.0")
        .await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "stable-2015-01-01"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.0.0")
        .await;
}

#[tokio::test]
async fn install_override_toolchain_from_version() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "1.1.0"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;
}

#[tokio::test]
async fn override_overrides_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    let mut cx = cx.change_dir(tempdir.path());
    cx.config
        .expect_ok(&["rustup", "override", "add", "beta"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
        .await;
}

#[tokio::test]
async fn multiple_overrides() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    {
        let mut cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect_ok(&["rustup", "override", "add", "beta"])
            .await;
    }

    {
        let mut cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect_ok(&["rustup", "override", "add", "stable"])
            .await;
    }

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
            .await;
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
            .await;
    }
}

#[tokio::test]
async fn change_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mut cx = cx.change_dir(tempdir.path());
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"])
        .await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "beta"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
        .await;
}

#[tokio::test]
async fn remove_override_no_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mut cx = cx.change_dir(tempdir.path());
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"])
        .await;
    cx.config.expect_ok(&["rustup", "override", "remove"]).await;
    cx.config
        .expect_err(
            &["rustc"],
            "rustup could not choose a version of rustc to run",
        )
        .await;
}

#[tokio::test]
async fn remove_override_with_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mut cx = cx.change_dir(tempdir.path());
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "beta"])
        .await;
    cx.config.expect_ok(&["rustup", "override", "remove"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn remove_override_with_multiple_overrides() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    {
        let mut cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect_ok(&["rustup", "override", "add", "beta"])
            .await;
    }

    {
        let mut cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect_ok(&["rustup", "override", "add", "stable"])
            .await;
    }

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;

    {
        let mut cx = cx.change_dir(tempdir1.path());
        cx.config.expect_ok(&["rustup", "override", "remove"]).await;
        cx.config
            .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
            .await;
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
            .await;
    }
}

#[tokio::test]
async fn no_update_on_channel_when_date_has_not_changed() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustup", "update", "nightly"], "unchanged")
        .await;
}

#[tokio::test]
async fn update_on_channel_when_date_has_changed() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV1).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
    cx.config.set_current_dist_date("2015-01-02");
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn run_command() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "default", "beta"]).await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "run", "nightly", "rustc", "--version"],
            "hash-nightly-2",
        )
        .await;
}

#[tokio::test]
async fn remove_toolchain_then_add_again() {
    // Issue brson/multirust #53
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config.expect_ok(&["rustup", "default", "beta"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "beta"])
        .await;
    cx.config.expect_ok(&["rustup", "update", "beta"]).await;
    cx.config.expect_ok(&["rustc", "--version"]).await;
}
