use std::{fs, path::PathBuf, process::Command, time::Duration};

use rustup::install::{
    CHECKPOINT_INSTALL_AFTER_PUBLISH, CHECKPOINT_INSTALL_BEFORE_PUBLISH, STAGING_DIR_PREFIX,
};
use rustup::test::{CliTestContext, Scenario, this_host_tuple};
use wait_timeout::ChildExt;

fn assert_completes_successfully(mut command: Command) {
    let mut child = command.spawn().expect("failed to start command");
    let Some(status) = child
        .wait_timeout(Duration::from_secs(10))
        .expect("failed to wait for command")
    else {
        let _ = child.kill();
        let _ = child.wait();
        panic!("command did not complete within 10 seconds");
    };
    assert!(status.success(), "command failed with status {status}");
}

fn nightly_path(cx: &CliTestContext) -> PathBuf {
    cx.config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_tuple()))
}

fn nightly_update_hash_path(cx: &CliTestContext) -> PathBuf {
    cx.config
        .rustupdir
        .join("update-hashes")
        .join(format!("nightly-{}", this_host_tuple()))
}

fn nightly_staging_root(cx: &CliTestContext) -> PathBuf {
    cx.config
        .rustupdir
        .join("toolchains")
        .join(format!("{STAGING_DIR_PREFIX}nightly-{}", this_host_tuple()))
}

fn staging_paths(cx: &CliTestContext) -> Vec<PathBuf> {
    fs::read_dir(cx.config.rustupdir.join("toolchains"))
        .expect("failed to read toolchains directory")
        .map(|entry| entry.expect("failed to read toolchains entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(STAGING_DIR_PREFIX))
        })
        .collect()
}

async fn assert_nightly_is_complete(cx: &CliTestContext) {
    cx.config
        .expect(["rustup", "+nightly", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TUPLE]
rust-docs-[HOST_TUPLE]
rust-std-[HOST_TUPLE]
rustc-[HOST_TUPLE]

"#]])
        .is_ok();
    cx.config
        .expect(["rustc", "+nightly", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

async fn assert_unpublished_install_can_be_retried(checkpoint: &str) {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let command = cx.config.cmd("rustup", ["toolchain", "install", "nightly"]);

    let status = cx.kill_at_checkpoint(command, checkpoint);
    assert!(!status.success());
    assert!(
        !nightly_path(&cx).exists(),
        "an interrupted staging operation published the toolchain"
    );
    assert_eq!(staging_paths(&cx), vec![nightly_staging_root(&cx)]);
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .without_stdout("rustup-staging")
        .without_stdout("nightly")
        .is_ok();

    assert_completes_successfully(cx.config.cmd("rustup", ["toolchain", "install", "nightly"]));
    assert!(nightly_path(&cx).is_dir());
    assert_nightly_is_complete(&cx).await;
}

#[tokio::test]
async fn interrupted_install_can_be_retried() {
    assert_unpublished_install_can_be_retried(BEFORE_METADATA).await;
}

#[tokio::test]
async fn interrupted_install_before_publication_can_be_retried() {
    assert_unpublished_install_can_be_retried(CHECKPOINT_INSTALL_BEFORE_PUBLISH).await;
}

#[tokio::test]
async fn interrupted_installs_do_not_accumulate_stages() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    for _ in 0..2 {
        let command = cx.config.cmd("rustup", ["toolchain", "install", "nightly"]);
        let status = cx.kill_at_checkpoint(command, CHECKPOINT_INSTALL_BEFORE_PUBLISH);
        assert!(!status.success());
        assert_eq!(staging_paths(&cx), vec![nightly_staging_root(&cx)]);
    }

    assert_completes_successfully(cx.config.cmd("rustup", ["toolchain", "install", "nightly"]));
    assert!(staging_paths(&cx).is_empty());
    assert_nightly_is_complete(&cx).await;
}

#[tokio::test]
async fn stale_stage_is_reclaimed_by_the_next_install() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let junk = nightly_staging_root(&cx)
        .join("toolchain")
        .join("bin")
        .join("stale-junk");
    fs::create_dir_all(junk.parent().unwrap()).unwrap();
    fs::write(&junk, "junk").unwrap();
    fs::write(nightly_staging_root(&cx).join("update-hash"), "stale-hash").unwrap();

    assert_completes_successfully(cx.config.cmd("rustup", ["toolchain", "install", "nightly"]));
    assert!(staging_paths(&cx).is_empty());
    assert!(
        !nightly_path(&cx).join("bin").join("stale-junk").exists(),
        "contents of a stale stage leaked into the published toolchain"
    );
    assert_nightly_is_complete(&cx).await;
}

#[tokio::test]
async fn unpublished_install_does_not_change_update_hash() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let update_hash = nightly_update_hash_path(&cx);
    fs::create_dir_all(update_hash.parent().unwrap()).unwrap();
    fs::write(&update_hash, "stale-hash").unwrap();

    let command = cx.config.cmd("rustup", ["toolchain", "install", "nightly"]);
    let status = cx.kill_at_checkpoint(command, CHECKPOINT_INSTALL_BEFORE_PUBLISH);

    assert!(!status.success());
    assert_eq!(fs::read_to_string(update_hash).unwrap(), "stale-hash");
    assert!(!nightly_path(&cx).exists());
}

#[tokio::test]
async fn interrupted_install_after_publication_is_complete() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let command = cx.config.cmd("rustup", ["toolchain", "install", "nightly"]);

    let status = cx.kill_at_checkpoint(command, CHECKPOINT_INSTALL_AFTER_PUBLISH);
    assert!(!status.success());
    assert!(nightly_path(&cx).is_dir());
    assert!(staging_paths(&cx).is_empty());
    assert_nightly_is_complete(&cx).await;

    assert_completes_successfully(cx.config.cmd("rustup", ["toolchain", "install", "nightly"]));
    assert_nightly_is_complete(&cx).await;
}

#[tokio::test]
async fn failed_install_removes_staging_directory() {
    let cx = CliTestContext::new(Scenario::UnavailableRls).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "set", "profile", "complete"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_err();

    assert!(!nightly_path(&cx).exists());
    assert!(staging_paths(&cx).is_empty());
}

#[tokio::test]
async fn interrupted_update_can_be_retried() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "+nightly", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    let command = cx.config.cmd("rustup", ["update", "nightly"]);
    let status = cx.kill_at_checkpoint(command, BEFORE_METADATA);
    assert!(!status.success());

    assert_completes_successfully(cx.config.cmd("rustup", ["update", "nightly"]));

    assert_nightly_is_complete(&cx).await;
}

// TODO(rust-lang/rustup#4966): import this from `manifestation` once the
// checkpoint constants are exported next to their checkpoint sites.
const BEFORE_METADATA: &str = "manifestation-update-before-metadata";
