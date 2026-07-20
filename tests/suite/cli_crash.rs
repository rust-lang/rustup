use rustup::test::{CliTestContext, Scenario};

#[tokio::test]
async fn interrupted_install_can_be_retried() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let status = cx.kill_at(
        "manifestation-update-before-metadata",
        ["rustup", "toolchain", "install", "nightly"],
    );
    assert!(!status.success());

    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "+nightly", "--version"])
        .await
        .is_ok();
}
