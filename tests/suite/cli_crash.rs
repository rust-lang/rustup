use rustup::{
    dist::{
        download::CHECKPOINT_DOWNLOAD_BEFORE_RENAME,
        manifestation::CHECKPOINT_UPDATE_BEFORE_METADATA,
    },
    test::{CliTestContext, Scenario},
};

#[tokio::test]
async fn interrupted_install_can_be_retried() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "toolchain", "install", "nightly"],
    )
    .is_err();

    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "+nightly", "--version"])
        .await
        .is_ok();
}

/// TODO: Two processes installing the same toolchain may download the same file and thus share the
/// same `.partial` path. Whichever finishes first renames it to its final name, and the other one
/// then fails to find it.
///
/// See: <https://github.com/rust-lang/rustup/issues/4607>
#[tokio::test]
async fn concurrent_install_conflicts_on_partial_download() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let parked = cx.spawn_at(
        CHECKPOINT_DOWNLOAD_BEFORE_RENAME,
        ["rustup", "toolchain", "install", "nightly"],
    );
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    parked
        .resume()
        .with_stderr(snapbox::str![[r#"
...
error: component download failed for [..]: could not rename 'downloaded' file from '[..].partial' to '[..]':[..]
...
"#]])
        .is_err();
}

/// TODO: A proxy launched while a modification is in flight. During this time, it sees a toolchain
/// whose files and metadata disagree.
///
/// In this test, the `cargo` files are already gone, but the toolchain is still advertised as
/// having the `cargo` component, hence the unhelpful message.
///
/// See: <https://github.com/rust-lang/rust-clippy/issues/12763>
#[tokio::test]
async fn proxy_during_component_removal_sees_inconsistent_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "remove", "cargo"],
    );
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: the 'cargo[EXE]' binary, normally provided by the 'cargo' component, is not applicable to the 'nightly-[HOST_TUPLE]' toolchain

"#]])
        .is_err();

    parked.resume().is_ok();
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: 'cargo[EXE]' is not installed for the toolchain 'nightly-[HOST_TUPLE]'.
...
"#]])
        .is_err();
}

/// TODO: When uninstalling a toolchain while it is being installed, the process installing the
/// toolchain is not aware of the race, so it happily considers a toolchain directory that lacks
/// most of its files as fully installed.
///
/// See: <https://github.com/rust-lang/rustup/issues/988#issuecomment-593632709>
#[tokio::test]
async fn uninstall_during_install_leaves_broken_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "toolchain", "install", "nightly"],
    );
    cx.config
        .expect(["rustup", "toolchain", "uninstall", "nightly"])
        .await
        .is_ok();
    parked.resume().is_ok();

    // The toolchain shows up as installed...
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TUPLE] (active, default)

"#]])
        .is_ok();
    // ... but it is not usable.
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: the 'rustc[EXE]' binary, normally provided by the 'rustc' component, is not applicable to the 'nightly-[HOST_TUPLE]' toolchain

"#]])
        .is_err();
}
/// TODO: A modification killed in the middle leaves files behind that the next attempt trips over,
/// since the metadata that would have accounted for them was never written.
///
/// Unlike [`interrupted_install_can_be_retried()`], there is no "partially installed toolchain"
/// recovery path for components, so the toolchain stays stuck.
///
/// See: <https://github.com/rust-lang/rustup/issues/4400>
#[tokio::test]
async fn interrupted_component_add_cannot_be_retried() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "rust-src"],
    )
    .is_err();

    cx.config
        .expect(["rustup", "component", "add", "rust-src"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: failed to install component: 'rust-src', detected conflict: 'lib/rustlib/src/rust-src/foo.rs'
...
"#]])
        .is_err();
}

/// TODO: `rustup component add` races against another `rustup component add` of the same component.
///
/// The second process cannot tell that the first one is already installing the component, so it
/// tries to install it again on top of the first one's files.
///
/// See: <https://github.com/rust-lang/rustup/issues/3716>
#[tokio::test]
async fn concurrent_component_add_same_component_conflicts() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "rust-src"],
    );
    cx.config
        .expect(["rustup", "component", "add", "rust-src"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: failed to install component: 'rust-src', detected conflict: 'lib/rustlib/src/rust-src/foo.rs'
...
"#]])
        .is_err();

    parked.resume().is_ok();
}

/// TODO: This failure is similar to [`concurrent_component_add_same_component_conflicts()`], but on
/// a component that provides a binary in `/bin`.
///
/// See: <https://github.com/rust-lang/rustup/issues/4465>
#[tokio::test]
async fn concurrent_component_add_same_component_conflicts_with_bin() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "remove", "cargo"])
        .await
        .is_ok();

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "cargo"],
    );
    cx.config
        .expect(["rustup", "component", "add", "cargo"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: failed to install component: 'cargo-[HOST_TUPLE]', detected conflict: 'bin/cargo[..]'
...
"#]])
        .is_err();

    parked.resume().is_ok();
}

/// TODO: Two `rustup component add` of *different* components lose one of the writes to the
/// toolchain's component list.
///
/// Each process reads the list at the start, then writes back its own view of it at the end. The
/// first process to finish has its component forgotten by the second one, although its files are on
/// disk. Adding it again then trips over its own files.
///
/// See: <https://github.com/rust-lang/rustup/issues/3530>
#[tokio::test]
async fn concurrent_component_add_different_components_loses_config_write() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let parked = cx.spawn_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "add", "rust-src"],
    );
    cx.config
        .expect(["rustup", "component", "add", "rust-analysis"])
        .await
        .is_ok();
    parked.resume().is_ok();

    // Both components should be listed here, but `rust-analysis` is gone.
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TUPLE]
rust-docs-[HOST_TUPLE]
rust-src
rust-std-[HOST_TUPLE]
rustc-[HOST_TUPLE]

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rust-analysis"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: failed to install component: 'rust-analysis-[HOST_TUPLE]', detected conflict: 'lib/rustlib/[HOST_TUPLE]/analysis/libfoo.json'
...
"#]])
        .is_err();
}

/// TODO: A component removal killed in the middle leaves a toolchain that still claims to have the
/// component whose files are already gone.
///
/// `rustup component add` then considers the component up to date, so the only way out is to remove
/// it first and then add it again.
///
/// See: <https://github.com/rust-lang/rustup/issues/2417>
#[tokio::test]
async fn interrupted_component_remove_leaves_stale_component() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.kill_at(
        CHECKPOINT_UPDATE_BEFORE_METADATA,
        ["rustup", "component", "remove", "cargo"],
    )
    .is_err();

    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: the 'cargo[EXE]' binary, normally provided by the 'cargo' component, is not applicable to the 'nightly-[HOST_TUPLE]' toolchain

"#]])
        .is_err();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TUPLE]
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "cargo"])
        .await
        .with_stderr(snapbox::str![[r#"
info: component cargo is up to date

"#]])
        .is_ok();
    cx.config.expect(["cargo", "--version"]).await.is_err();

    cx.config
        .expect(["rustup", "component", "remove", "cargo"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "cargo"])
        .await
        .is_ok();
    cx.config.expect(["cargo", "--version"]).await.is_ok();
}
