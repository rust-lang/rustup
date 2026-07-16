use rustup::test::{CliTestContext, Scenario};

#[tokio::test]
async fn interrupted_install_can_be_retried() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let command = cx.config.cmd("rustup", ["toolchain", "install", "nightly"]);

    let status = cx.kill_at_checkpoint(command, "manifestation-update-before-metadata");
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
