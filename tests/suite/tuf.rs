//! Downloads through TUF, driven by rustup commands against a repository
//! signed over the mock servers. The dist and update roots point at an
//! unreachable host, so every manifest and self-update file has to come
//! through the repository; component tarballs still come from the mock.

use rustup::test::{CROSS_ARCH1, CliTestContext, MockTufServer, Scenario, SelfUpdateTestContext};

const TEST_VERSION: &str = "1.1.1";

async fn tuf(cx: &CliTestContext) -> MockTufServer {
    MockTufServer::new(cx.config.distdir.as_ref().unwrap(), None).await
}

#[tokio::test]
async fn install_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tuf = tuf(&cx).await;
    cx.config
        .expect_with_env(["rustup", "default", "nightly"], tuf.env("on"))
        .await
        .is_ok();
    cx.config
        .expect_with_env(["rustc", "--version"], tuf.env("on"))
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_dated_toolchain() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    let tuf = tuf(&cx).await;
    cx.config
        .expect_with_env(["rustup", "default", "nightly-2015-01-01"], tuf.env("on"))
        .await
        .is_ok();
    cx.config
        .expect_with_env(["rustc", "--version"], tuf.env("on"))
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
}

#[tokio::test]
async fn update_after_channel_moves() {
    let cx = CliTestContext::new(Scenario::Full).await;
    cx.config.set_current_dist_date("2015-01-01");
    let mut tuf = tuf(&cx).await;
    cx.config
        .expect_with_env(["rustup", "default", "nightly"], tuf.env("on"))
        .await
        .is_ok();

    cx.config.set_current_dist_date("2015-01-02");
    tuf.publish().await;
    cx.config
        .expect_with_env(["rustup", "update", "nightly"], tuf.env("on"))
        .await
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TUPLE] updated - 1.3.0 (hash-nightly-2) (from 1.2.0 (hash-nightly-1))


"#]])
        .is_ok();
}

#[tokio::test]
async fn add_target() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tuf = tuf(&cx).await;
    cx.config
        .expect_with_env(["rustup", "default", "nightly"], tuf.env("on"))
        .await
        .is_ok();
    cx.config
        .expect_with_env(["rustup", "target", "add", CROSS_ARCH1], tuf.env("on"))
        .await
        .is_ok();
}

#[tokio::test]
async fn check_reads_manifests() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tuf = tuf(&cx).await;
    cx.config
        .expect_with_env(["rustup", "default", "nightly"], tuf.env("on"))
        .await
        .is_ok();
    cx.config
        .expect_with_env(["rustup", "check"], tuf.env("on"))
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TUPLE] - up to date: 1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn tampered_manifest_fails() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tuf = tuf(&cx).await;
    tuf.tamper("channels/current/nightly.toml");
    cx.config
        .expect_with_env(["rustup", "default", "nightly"], tuf.env("on"))
        .await
        .with_stderr(snapbox::str![[r#"
...
error: could not download file from '[..]' to '[..]': TUF verification failed: [..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn tampered_manifest_tolerated_in_warn_mode() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tuf = tuf(&cx).await;
    tuf.tamper("channels/current/nightly.toml");
    cx.config
        .expect_with_env(["rustup", "default", "nightly"], tuf.env("warn"))
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: TUF verification failed: [..]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn off_mode_ignores_repository() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tuf = tuf(&cx).await;
    tuf.tamper("channels/current/nightly.toml");
    // The mock dist server stays reachable, and the tampered repository is
    // never consulted.
    cx.config
        .expect_with_env(["rustup", "default", "nightly"], tuf.settings("off"))
        .await
        .is_ok();
}

#[tokio::test]
async fn self_update() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    let tuf = MockTufServer::new(cx.config.distdir.as_ref().unwrap(), Some(cx.path())).await;
    cx.config
        .expect_with_env(["rustup-init", "-y", "--no-modify-path"], tuf.env("on"))
        .await
        .is_ok();
    cx.config
        .expect_with_env(["rustup", "self", "update"], tuf.env("on"))
        .await
        .extend_redactions([("[TEST_VERSION]", TEST_VERSION)])
        .with_stdout(snapbox::str![[r#"
  rustup updated - [CURRENT_VERSION] (from [CURRENT_VERSION])


"#]])
        .with_stderr(snapbox::str![[r#"
info: checking for self-update (current version: [CURRENT_VERSION])
info: syncing TUF database from [..]
info: downloading self-update (new version: [TEST_VERSION])

"#]])
        .is_ok();
}

#[tokio::test]
async fn self_update_tampered_binary_fails() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    let tuf = MockTufServer::new(cx.config.distdir.as_ref().unwrap(), Some(cx.path())).await;
    cx.config
        .expect_with_env(["rustup-init", "-y", "--no-modify-path"], tuf.env("on"))
        .await
        .is_ok();
    tuf.tamper(&format!(
        "rustup/{TEST_VERSION}/{}/rustup-init{}",
        rustup::test::this_host_tuple(),
        std::env::consts::EXE_SUFFIX
    ));
    cx.config
        .expect_with_env(["rustup", "self", "update"], tuf.env("on"))
        .await
        .with_stderr(snapbox::str![[r#"
...
error: could not download file from '[..]' to '[..]': TUF verification failed: [..]
...
"#]])
        .is_err();
    // The installed rustup is untouched.
    cx.config
        .expect_with_env(["rustup", "--version"], tuf.env("on"))
        .await
        .is_ok();
}
