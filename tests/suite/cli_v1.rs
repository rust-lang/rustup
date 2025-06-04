//! Test cases of the rustup command, using v1 manifests, mostly
//! derived from multirust/test-v2.sh

use std::fs;

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
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_toolchain_from_channel() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_toolchain_from_archive() {
    let cx = CliTestContext::new(Scenario::ArchivesV1).await;
    cx.config
        .expect(["rustup", "default", "nightly-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default", "beta-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-beta-1.1.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default", "stable-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.0.0 (hash-stable-1.0.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_toolchain_from_version() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "default", "1.1.0"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn default_existing_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: using existing install for 'nightly-[HOST_TRIPLE]'
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn update_channel() {
    let cx = CliTestContext::new(Scenario::ArchivesV1).await;
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
    cx.config.set_current_dist_date("2015-01-02");
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn list_toolchains() {
    let cx = CliTestContext::new(Scenario::ArchivesV1).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "beta-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
beta-2015-01-01-[HOST_TRIPLE]
nightly-[HOST_TRIPLE] (active, default)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list", "-v"])
        .await
        .with_stdout(snapbox::str![[r#"
beta-2015-01-01-[HOST_TRIPLE] [..]/toolchains/beta-2015-01-01-[HOST_TRIPLE]
nightly-[HOST_TRIPLE] (active, default) [..]/toolchains/nightly-[HOST_TRIPLE]

"#]])
        .is_ok();
}

#[tokio::test]
async fn list_toolchains_with_none() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
no installed toolchains

"#]])
        .is_ok();
}

#[tokio::test]
async fn remove_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
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

#[tokio::test]
async fn remove_override_toolchain_err_handling() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .with_stderr(snapbox::str![[r#"
...
info: downloading component 'rust'
...
"#]])
        .is_ok();
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
        .expect(["rustup", "default", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: checksum failed[..]
...
"#]])
        .is_err();
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
        .expect(["rustup", "default", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: checksum failed[..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn install_override_toolchain_from_channel() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_override_toolchain_from_archive() {
    let cx = CliTestContext::new(Scenario::ArchivesV1).await;
    cx.config
        .expect(["rustup", "override", "add", "nightly-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-beta-1.1.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "stable-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.0.0 (hash-stable-1.0.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_override_toolchain_from_version() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "override", "add", "1.1.0"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn override_overrides_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn multiple_overrides() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect(["rustup", "override", "add", "beta"])
            .await
            .is_ok();
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect(["rustup", "override", "add", "stable"])
            .await
            .is_ok();
    }

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect(["rustc", "--version"])
            .await
            .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
            .is_ok();
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect(["rustc", "--version"])
            .await
            .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
            .is_ok();
    }
}

#[tokio::test]
async fn change_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn remove_override_no_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "remove"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc"])
        .await
        .with_stderr(snapbox::str![[r#"
error: rustup could not choose a version of rustc to run[..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn remove_override_with_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "remove"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn remove_override_with_multiple_overrides() {
    let mut cx = CliTestContext::new(Scenario::SimpleV1).await;
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect(["rustup", "override", "add", "beta"])
            .await
            .is_ok();
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect(["rustup", "override", "add", "stable"])
            .await
            .is_ok();
    }

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect(["rustup", "override", "remove"])
            .await
            .is_ok();
        cx.config
            .expect(["rustc", "--version"])
            .await
            .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
            .is_ok();
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect(["rustc", "--version"])
            .await
            .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
            .is_ok();
    }
}

#[tokio::test]
async fn no_update_on_channel_when_date_has_not_changed() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] unchanged - 1.3.0 (hash-nightly-2)


"#]])
        .is_ok();
}

#[tokio::test]
async fn update_on_channel_when_date_has_changed() {
    let cx = CliTestContext::new(Scenario::ArchivesV1).await;
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
    cx.config.set_current_dist_date("2015-01-02");
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn run_command() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "run", "nightly", "rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn remove_toolchain_then_add_again() {
    // Issue brson/multirust #53
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "default", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "beta"])
        .await
        .is_ok();
    cx.config.expect(["rustup", "update", "beta"]).await.is_ok();
    cx.config.expect(["rustc", "--version"]).await.is_ok();
}
