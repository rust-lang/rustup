//! Test cases for new rustup UI

use std::fs;
use std::path::PathBuf;
use std::{env::consts::EXE_SUFFIX, path::Path};

use rustup::test::{
    CROSS_ARCH1, CROSS_ARCH2, CliTestContext, MULTI_ARCH1, Scenario, this_host_triple,
    topical_doc_data,
};
use rustup::utils::raw;

#[tokio::test]
async fn rustup_stable() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect(["rustup", "toolchain", "add", "stable"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect(["rustup", "update"])
        .await
        .with_stdout(snapbox::str![[r#"

  stable-[HOST_TRIPLE] updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'stable-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component[..]
...
info: cleaning up downloads & tmp directories

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn rustup_stable_quiet() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect(["rustup", "--quiet", "update", "stable"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect(["rustup", "--quiet", "update"])
        .await
        .with_stdout(snapbox::str![[r#"

  stable-[HOST_TRIPLE] updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))


"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn rustup_stable_no_change() {
    let cx = CliTestContext::new(Scenario::ArchivesV2_2015_01_01).await;
    cx.config
        .expect(["rustup", "update", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update"])
        .await
        .with_stdout(snapbox::str![[r#"

  stable-[HOST_TRIPLE] unchanged - 1.0.0 (hash-stable-1.0.0)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'stable-[HOST_TRIPLE]'
info: cleaning up downloads & tmp directories

"#]])
        .is_ok();
}

#[tokio::test]
async fn rustup_all_channels() {
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
        .expect(["rustup", "update"])
        .await
        .with_stdout(snapbox::str![[r#"

   stable-[HOST_TRIPLE] updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))
     beta-[HOST_TRIPLE] updated - 1.2.0 (hash-beta-1.2.0) (from 1.1.0 (hash-beta-1.1.0))
  nightly-[HOST_TRIPLE] updated - 1.3.0 (hash-nightly-2) (from 1.2.0 (hash-nightly-1))


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'stable-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component[..]
...
info: syncing channel updates for 'beta-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.2.0 (hash-beta-1.2.0)
info: downloading component[..]
...
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component[..]
...
info: cleaning up downloads & tmp directories

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+stable", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
    cx.config
        .expect(["rustup", "+beta", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
    cx.config
        .expect(["rustup", "+nightly", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn rustup_some_channels_up_to_date() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect(["rustup", "toolchain", "add", "stable", "beta", "nightly"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config.expect(["rustup", "update", "beta"]).await.is_ok();
    cx.config
        .expect(["rustup", "update"])
        .await
        .with_stdout(snapbox::str![[r#"

   stable-[HOST_TRIPLE] updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))
   beta-[HOST_TRIPLE] unchanged - 1.2.0 (hash-beta-1.2.0)
  nightly-[HOST_TRIPLE] updated - 1.3.0 (hash-nightly-2) (from 1.2.0 (hash-nightly-1))


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'stable-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component[..]
...
info: syncing channel updates for 'beta-[HOST_TRIPLE]'
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component[..]
...
info: cleaning up downloads & tmp directories

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+stable", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
    cx.config
        .expect(["rustup", "+beta", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
    cx.config
        .expect(["rustup", "+nightly", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn rustup_no_channels() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update"])
        .await
        .with_stdout(snapbox::str![""])
        .with_stderr(snapbox::str![[r#"
info: no updatable toolchains installed
info: cleaning up downloads & tmp directories

"#]])
        .is_ok();
}

#[tokio::test]
async fn default() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] installed - 1.3.0 (hash-nightly-2)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component[..]
...
info: default toolchain set to 'nightly-[HOST_TRIPLE]'

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn default_override() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "set", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .with_stderr(snapbox::str![[r#"
info: using existing install for 'stable-[HOST_TRIPLE]'
info: default toolchain set to 'stable-[HOST_TRIPLE]'
info: note that the toolchain 'nightly-[HOST_TRIPLE]' is currently in use (directory override for '[..]')

"#]])
        .is_ok();
}

#[tokio::test]
async fn rustup_zstd() {
    let cx = CliTestContext::new(Scenario::ArchivesV2_2015_01_01).await;
    cx.config
        .expect(["rustup", "--verbose", "toolchain", "add", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
[..]dist/2015-01-01/rust-std-nightly-[HOST_TRIPLE].tar.zst[..]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn add_target() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn remove_target() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    assert!(cx.config.rustupdir.has(&path));
    cx.config
        .expect(["rustup", "target", "remove", CROSS_ARCH1])
        .await
        .is_ok();
    assert!(!cx.config.rustupdir.has(&path));
}

#[tokio::test]
async fn add_remove_multiple_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1, CROSS_ARCH2])
        .await
        .is_ok();
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    assert!(cx.config.rustupdir.has(path));
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH2
    );
    assert!(cx.config.rustupdir.has(path));

    cx.config
        .expect(["rustup", "target", "remove", CROSS_ARCH1, CROSS_ARCH2])
        .await
        .is_ok();
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    assert!(!cx.config.rustupdir.has(path));
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH2
    );
    assert!(!cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn list_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn list_installed_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
[HOST_TRIPLE]

"#]])
        .is_ok();
}

#[tokio::test]
async fn add_target_explicit() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    cx.config
        .expect(["rustup", "toolchain", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "target",
            "add",
            "--toolchain",
            "nightly",
            CROSS_ARCH1,
        ])
        .await
        .is_ok();
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn remove_target_explicit() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    cx.config
        .expect(["rustup", "toolchain", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "target",
            "add",
            "--toolchain",
            "nightly",
            CROSS_ARCH1,
        ])
        .await
        .is_ok();
    assert!(cx.config.rustupdir.has(&path));
    cx.config
        .expect([
            "rustup",
            "target",
            "remove",
            "--toolchain",
            "nightly",
            CROSS_ARCH1,
        ])
        .await
        .is_ok();
    assert!(!cx.config.rustupdir.has(&path));
}

#[tokio::test]
async fn list_targets_explicit() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list", "--toolchain", "nightly"])
        .await
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn link() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "custom", &path])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "custom"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.0.0 (hash-c-1)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
custom (active, default)
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
custom
...
"#]])
        .is_ok();
}

// Issue #809. When we call the fallback cargo, when it in turn invokes
// "rustc", that rustc should actually be the rustup proxy, not the toolchain rustc.
// That way the proxy can pick the correct toolchain.
#[tokio::test]
async fn fallback_cargo_calls_correct_rustc() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    // Hm, this is the _only_ test that assumes that toolchain proxies
    // exist in CARGO_HOME. Adding that proxy here.
    let rustup_path = cx.config.exedir.join(format!("rustup{EXE_SUFFIX}"));
    let cargo_bin_path = cx.config.cargodir.join("bin");
    fs::create_dir_all(&cargo_bin_path).unwrap();
    let rustc_path = cargo_bin_path.join(format!("rustc{EXE_SUFFIX}"));
    fs::hard_link(rustup_path, &rustc_path).unwrap();

    // Install a custom toolchain and a nightly toolchain for the cargo fallback
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "custom", &path])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "custom"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.0.0 (hash-c-1)

"#]])
        .is_ok();
    cx.config
        .expect(["cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();

    assert!(rustc_path.exists());

    // Here --call-rustc tells the mock cargo bin to exec `rustc --version`.
    // We should be ultimately calling the custom rustc, according to the
    // RUSTUP_TOOLCHAIN variable set by the original "cargo" proxy, and
    // interpreted by the nested "rustc" proxy.
    cx.config
        .expect(["cargo", "--call-rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
1.0.0 (hash-c-1)

"#]])
        .is_ok();
}

// Checks that cargo can recursively invoke itself with rustup shorthand (via
// the proxy).
//
// This involves a series of chained commands:
//
// 1. Calls `cargo --recursive-cargo-subcommand`
// 2. The rustup `cargo` proxy launches, and launches the "mock" nightly cargo exe.
// 3. The nightly "mock" cargo sees --recursive-cargo-subcommand, and launches
//    `cargo-foo --recursive-cargo`
// 4. `cargo-foo` sees `--recursive-cargo` and launches `cargo +nightly --version`
// 5. The rustup `cargo` proxy launches, and launches the "mock" nightly cargo exe.
// 6. The nightly "mock" cargo sees `--version` and prints the version.
//
// Previously, rustup would place the toolchain's `bin` directory in PATH for
// Windows due to some DLL issues. However, those aren't necessary anymore.
// If the toolchain `bin` directory is in PATH, then this test would fail in
// step 5 because the `cargo` executable would be the "mock" nightly cargo,
// and the first argument would be `+nightly` which would be an error.
#[tokio::test]
async fn recursive_cargo() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    // We need an intermediary to run cargo itself.
    // The "mock" cargo can't do that because on Windows it will check
    // for a `cargo.exe` in the current directory before checking PATH.
    //
    // The solution here is to copy from the "mock" `cargo.exe` into
    // `~/.cargo/bin/cargo-foo`. This is just for convenience to avoid
    // needing to build another executable just for this test.
    let which_cargo = cx.config.expect(["rustup", "which", "cargo"]).await;
    let real_mock_cargo = which_cargo.output.stdout.trim();
    let cargo_bin_path = cx.config.cargodir.join("bin");
    let cargo_subcommand = cargo_bin_path.join(format!("cargo-foo{EXE_SUFFIX}"));
    fs::create_dir_all(&cargo_bin_path).unwrap();
    fs::copy(real_mock_cargo, cargo_subcommand).unwrap();

    cx.config
        .expect(["cargo", "--recursive-cargo-subcommand"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn show_home() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect(["rustup", "show", "home"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
[RUSTUP_DIR]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_toolchain_none() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------

active toolchain
----------------
no active toolchain

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_toolchain_default() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
nightly-[HOST_TRIPLE] (active, default)

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: it's the default toolchain
installed targets:
  [HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_no_default() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "none"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
installed toolchains
--------------------
nightly-[HOST_TRIPLE]

active toolchain
----------------
no active toolchain

"#]])
        .is_ok();
}

#[tokio::test]
async fn show_no_default_active() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "none"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
installed toolchains
--------------------
nightly-[HOST_TRIPLE] (active)

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: overridden by +toolchain on the command line
installed targets:
  [HOST_TRIPLE]

"#]])
        .is_ok();
}

#[tokio::test]
async fn show_multiple_toolchains() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
stable-[HOST_TRIPLE]
nightly-[HOST_TRIPLE] (active, default)

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: it's the default toolchain
installed targets:
  [HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_multiple_targets() {
    let cx = CliTestContext::new(Scenario::MultiHost).await;
    cx.config
        .expect([
            "rustup",
            "default",
            &format!("nightly-{MULTI_ARCH1}"),
            "--force-non-host",
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH2])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
nightly-[MULTI_ARCH_I] (active, default)

active toolchain
----------------
name: nightly-[MULTI_ARCH_I]
active because: it's the default toolchain
installed targets:
  [CROSS_ARCH_II]
  [MULTI_ARCH_I]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_multiple_toolchains_and_targets() {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86") {
        return;
    }

    let cx = CliTestContext::new(Scenario::MultiHost).await;
    cx.config
        .expect([
            "rustup",
            "default",
            &format!("nightly-{MULTI_ARCH1}"),
            "--force-non-host",
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH2])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "update",
            "--force-non-host",
            &format!("stable-{MULTI_ARCH1}"),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
stable-[MULTI_ARCH_I]
nightly-[MULTI_ARCH_I] (active, default)

active toolchain
----------------
name: nightly-[MULTI_ARCH_I]
active because: it's the default toolchain
installed targets:
  [CROSS_ARCH_II]
  [MULTI_ARCH_I]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn list_default_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE] (active, default)

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn list_default_toolchain_quiet() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list", "--quiet"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn list_no_default_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "none"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn list_no_default_override_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "override", "set", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE] (active)

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn list_default_and_override_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "override", "set", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "toolchain", "list"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE] (active, default)

"#]])
        .with_stderr(snapbox::str![[""]]);
}

#[tokio::test]
async fn show_toolchain_override() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
nightly-[HOST_TRIPLE] (active)

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: directory override for '[..]'
installed targets:
  [HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_toolchain_toolchain_file_override() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");

    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
stable-[HOST_TRIPLE] (default)
nightly-[HOST_TRIPLE] (active)

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: overridden by '[..]'
installed targets:
  [HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_toolchain_version_nested_file_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "add", "stable", "beta", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");

    raw::write_file(&toolchain_file, "nightly").unwrap();

    let subdir = cwd.join("foo");

    fs::create_dir_all(&subdir).unwrap();
    let cx = cx.change_dir(&subdir);
    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
stable-[HOST_TRIPLE] (default)
beta-[HOST_TRIPLE]
nightly-[HOST_TRIPLE] (active)

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: overridden by '[..]'
installed targets:
  [HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_toolchain_toolchain_file_override_not_installed() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");

    raw::write_file(&toolchain_file, "nightly").unwrap();

    // I'm not sure this should really be erroring when the toolchain
    // is not installed; just capturing the behavior.
    cx.config
        .expect_with_env(["rustup", "show"], [("RUSTUP_AUTO_INSTALL", "0")])
        .await
        .extend_redactions([
            ("[RUSTUP_DIR]", &cx.config.rustupdir.rustupdir),
            ("[TOOLCHAIN_FILE]", &toolchain_file),
        ])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]


"#]])
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' is not installed
help: run `rustup toolchain install` to install it

"#]])
        .is_err();
}

#[tokio::test]
async fn show_toolchain_override_not_installed() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: directory override for '[..]'
installed targets:
  [HOST_TRIPLE]

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn override_set_unset_with_path() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = fs::canonicalize(cx.config.current_dir()).unwrap();
    let mut cwd_str = cwd.to_str().unwrap();

    if cfg!(windows) {
        cwd_str = &cwd_str[4..];
    }

    let emptydir = tempfile::tempdir().unwrap();
    {
        let cx = cx.change_dir(emptydir.path());
        cx.config
            .expect(["rustup", "override", "set", "nightly", "--path", cwd_str])
            .await
            .is_ok();
    }

    cx.config
        .expect(["rustup", "override", "list"])
        .await
        .extend_redactions([("[CWD]", cwd_str.to_string())])
        .with_stdout(snapbox::str![[r#"
[CWD]	nightly-[HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();

    {
        let cx = cx.change_dir(emptydir.path());
        cx.config
            .expect(["rustup", "override", "unset", "--path", cwd_str])
            .await
            .is_ok();
    }

    cx.config
        .expect(["rustup", "override", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
no overrides

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_toolchain_env() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect_with_env(["rustup", "show"], &[("RUSTUP_TOOLCHAIN", "nightly")])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .is_ok()
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
nightly-[HOST_TRIPLE] (active, default)

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: overridden by environment variable RUSTUP_TOOLCHAIN
installed targets:
  [HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn show_toolchain_env_not_installed() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(["rustup", "show"], &[("RUSTUP_TOOLCHAIN", "nightly")])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .is_ok()
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: overridden by environment variable RUSTUP_TOOLCHAIN
installed targets:
  [HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn show_active_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "show", "active-toolchain"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE] (default)

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_with_verbose() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect(["rustup", "default", "nightly"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    let config = &cx.config;
    config
        .expect(["rustup", "update", "nightly-2015-01-01"])
        .await
        .is_ok();
    config
        .expect(["rustup", "show", "--verbose"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
nightly-[HOST_TRIPLE] (active, default)
  1.3.0 (hash-nightly-2)
  path: [RUSTUP_DIR]/toolchains/nightly-[HOST_TRIPLE]

nightly-2015-01-01-[HOST_TRIPLE]
  1.2.0 (hash-nightly-1)
  path: [RUSTUP_DIR]/toolchains/nightly-2015-01-01-[HOST_TRIPLE]

active toolchain
----------------
name: nightly-[HOST_TRIPLE]
active because: it's the default toolchain
compiler: 1.3.0 (hash-nightly-2)
path: [RUSTUP_DIR]/toolchains/nightly-[HOST_TRIPLE]
installed targets:
  [HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_active_toolchain_with_verbose() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "active-toolchain", "--verbose"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE]
active because: it's the default toolchain
compiler: 1.3.0 (hash-nightly-2)
path: [RUSTUP_DIR]/toolchains/nightly-[HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_active_toolchain_with_override() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "set", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .with_stdout(snapbox::str![[r#"
stable-[HOST_TRIPLE] (directory override for '[..]')

"#]])
        .is_ok();
}

#[tokio::test]
async fn show_active_toolchain_with_override_verbose() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "set", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "active-toolchain", "--verbose"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", &cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
stable-[HOST_TRIPLE]
active because: directory override for '[..]'
compiler: 1.1.0 (hash-stable-1.1.0)
path: [RUSTUP_DIR]/toolchains/stable-[HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_active_toolchain_none() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
error: no active toolchain

"#]])
        .is_err();
}

#[tokio::test]
async fn show_profile() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "profile"])
        .await
        .with_stdout(snapbox::str![[r#"
default

"#]])
        .is_ok();

    // Check we get the same thing after we add or remove a component.
    cx.config
        .expect(["rustup", "component", "add", "rust-src"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "profile"])
        .await
        .with_stdout(snapbox::str![[r#"
default

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "remove", "rustc"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "profile"])
        .await
        .with_stdout(snapbox::str![[r#"
default

"#]])
        .is_ok();
}

// #846
#[tokio::test]
async fn set_default_host() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect(["rustup", "set", "default-host", &this_host_triple()])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
Default host: [HOST_TRIPLE]
...
"#]])
        .is_ok();
}

// #846
#[tokio::test]
async fn set_default_host_invalid_triple() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect(["rustup", "set", "default-host", "foo"])
        .await
        .with_stderr(snapbox::str![[r#"
error: Provided host 'foo' couldn't be converted to partial triple

"#]])
        .is_err();
}

// #745
#[tokio::test]
async fn set_default_host_invalid_triple_valid_partial() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect(["rustup", "set", "default-host", "x86_64-msvc"])
        .await
        .with_stderr(snapbox::str![[r#"
error: Provided host 'x86_64-msvc' did not specify an operating system

"#]])
        .is_err();
}

// #422
#[tokio::test]
async fn update_doesnt_update_non_tracking_channels() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect(["rustup", "default", "nightly"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    cx.config
        .expect(["rustup", "update", "nightly-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update"])
        .await
        .is_ok()
        .without_stderr(&format!(
            "syncing channel updates for 'nightly-2015-01-01-{}'",
            this_host_triple(),
        ));
}

#[tokio::test]
async fn toolchain_install_is_like_update() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
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
async fn toolchain_install_is_like_update_quiet() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "--quiet", "toolchain", "install", "nightly"])
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
async fn toolchain_install_without_args_installs_active() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
profile = "minimal"
channel = "nightly"
"#,
    )
    .unwrap();

    cx.config
        .expect(["rustup", "toolchain", "install"])
        .await
        .extend_redactions([("[TOOLCHAIN_FILE]", &toolchain_file)])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component[..]
...
info: the active toolchain `nightly-[HOST_TRIPLE]` has been installed
info: it's active because: overridden by '[TOOLCHAIN_FILE]'

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
rustc-[HOST_TRIPLE]

"#]]);

    cx.config
        .expect(["rustup", "toolchain", "install"])
        .await
        .extend_redactions([("[TOOLCHAIN_FILE]", &toolchain_file)])
        .with_stderr(snapbox::str![[r#"
info: using existing install for 'nightly-[HOST_TRIPLE]'
info: the active toolchain `nightly-[HOST_TRIPLE]` has been installed
info: it's active because: overridden by '[TOOLCHAIN_FILE]'

"#]])
        .is_ok();
}

#[tokio::test]
async fn toolchain_update_is_like_update() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "update", "nightly"])
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
async fn toolchain_uninstall_is_like_uninstall() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect(["rustup", "toolchain", "install", "nightly"])
            .await
            .is_ok();
    }

    cx.config
        .expect(["rustup", "default", "none"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "uninstall", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show"])
        .await
        .with_stdout(snapbox::str![[r#"
...
installed toolchains
--------------------

active toolchain
----------------
no active toolchain

"#]])
        .is_ok();
}

#[tokio::test]
async fn proxy_toolchain_shorthand() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustc", "+stable", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

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

#[tokio::test]
async fn add_component() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rust-src"])
        .await
        .is_ok();
    let path = format!(
        "toolchains/stable-{}/lib/rustlib/src/rust-src/foo.rs",
        this_host_triple()
    );
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn add_component_by_target_triple() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "component",
            "add",
            &format!("rust-std-{CROSS_ARCH1}"),
        ])
        .await
        .is_ok();
    let path = format!(
        "toolchains/stable-{}/lib/rustlib/{CROSS_ARCH1}/lib/libstd.rlib",
        this_host_triple()
    );
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn add_component_by_target_triple_renamed_from() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "component",
            "add",
            &format!("rls-{}", this_host_triple()),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rls-[HOST_TRIPLE]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn add_component_by_target_triple_renamed_to() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "component",
            "add",
            &format!("rls-preview-{}", this_host_triple()),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rls-[HOST_TRIPLE]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn fail_invalid_component_name() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(&[
            "rustup",
            "component",
            "add",
            &format!("dummy-{CROSS_ARCH1}"),
        ])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'stable-[HOST_TRIPLE]' does not contain component 'dummy-[CROSS_ARCH_I]' for target '[HOST_TRIPLE]'

"#]])
        .is_err();
}

#[tokio::test]
async fn fail_invalid_component_target() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config.expect(&[
        "rustup",
        "component",
        "add",
        "rust-std-invalid-target",
    ])
    .await
    .with_stderr(snapbox::str![[r#"
...
error: toolchain 'stable-[HOST_TRIPLE]' does not contain component 'rust-std-invalid-target' for target '[HOST_TRIPLE]'
...
"#]])
    .is_err();
}

#[tokio::test]
async fn remove_component() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "component", "add", "rust-src"])
        .await
        .is_ok();
    let path = PathBuf::from(format!(
        "toolchains/stable-{}/lib/rustlib/src/rust-src/foo.rs",
        this_host_triple(),
    ));
    assert!(cx.config.rustupdir.has(&path));
    cx.config
        .expect(&["rustup", "component", "remove", "rust-src"])
        .await
        .is_ok();
    assert!(!cx.config.rustupdir.has(path.parent().unwrap()));
}

#[tokio::test]
async fn remove_component_by_target_triple() {
    let component_with_triple = format!("rust-std-{CROSS_ARCH1}");
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "component", "add", &component_with_triple])
        .await
        .is_ok();
    let path = PathBuf::from(format!(
        "toolchains/stable-{}/lib/rustlib/{CROSS_ARCH1}/lib/libstd.rlib",
        this_host_triple()
    ));
    assert!(cx.config.rustupdir.has(&path));
    cx.config
        .expect(&["rustup", "component", "remove", &component_with_triple])
        .await
        .is_ok();
    assert!(!cx.config.rustupdir.has(path.parent().unwrap()));
}

#[tokio::test]
async fn add_remove_multiple_components() {
    let files = [
        "lib/rustlib/src/rust-src/foo.rs".to_owned(),
        format!("lib/rustlib/{}/analysis/libfoo.json", this_host_triple()),
        format!("lib/rustlib/{CROSS_ARCH1}/lib/libstd.rlib"),
        format!("lib/rustlib/{CROSS_ARCH2}/lib/libstd.rlib"),
    ];
    let component_with_triple1 = format!("rust-std-{CROSS_ARCH1}");
    let component_with_triple2 = format!("rust-std-{CROSS_ARCH2}");

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(&[
            "rustup",
            "component",
            "add",
            "rust-src",
            "rust-analysis",
            &component_with_triple1,
            &component_with_triple2,
        ])
        .await
        .is_ok();
    for file in &files {
        let path = format!("toolchains/nightly-{}/{}", this_host_triple(), file);
        assert!(cx.config.rustupdir.has(&path));
    }
    cx.config
        .expect(&[
            "rustup",
            "component",
            "remove",
            "rust-src",
            "rust-analysis",
            &component_with_triple1,
            &component_with_triple2,
        ])
        .await
        .is_ok();
    for file in &files {
        let path = PathBuf::from(format!(
            "toolchains/nightly-{}/{}",
            this_host_triple(),
            file
        ));
        assert!(!cx.config.rustupdir.has(path.parent().unwrap()));
    }
}

#[tokio::test]
async fn file_override() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(&["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect(&["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn env_override_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));

    cx.config
        .expect_with_env(
            ["rustc", "--version"],
            [("RUSTUP_TOOLCHAIN", toolchain_path.to_str().unwrap())],
        )
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn plus_override_relpath_is_not_supported() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let toolchain_path = Path::new("..")
        .join(cx.config.rustupdir.rustupdir.file_name().unwrap())
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));
    cx.config
        .expect([
            "rustc",
            format!("+{}", toolchain_path.to_str().unwrap()).as_str(),
            "--version",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
error: relative path toolchain '[..]/toolchains/nightly-[HOST_TRIPLE]'

"#]])
        .is_err();
}

#[tokio::test]
async fn run_with_relpath_is_not_supported() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let toolchain_path = Path::new("..")
        .join(cx.config.rustupdir.rustupdir.file_name().unwrap())
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));
    cx.config
        .expect([
            "rustup",
            "run",
            toolchain_path.to_str().unwrap(),
            "rustc",
            "--version",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
...
error:[..] relative path toolchain '[..]/toolchains/nightly-[HOST_TRIPLE]'
...
"#]])
        .is_err();
}

#[tokio::test]
async fn plus_override_abspath_is_supported() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()))
        .canonicalize()
        .unwrap();
    cx.config
        .expect([
            "rustc",
            format!("+{}", toolchain_path.to_str().unwrap()).as_str(),
            "--version",
        ])
        .await
        .is_ok();
}

#[tokio::test]
async fn run_with_abspath_is_supported() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()))
        .canonicalize()
        .unwrap();
    cx.config
        .expect([
            "rustup",
            "run",
            toolchain_path.to_str().unwrap(),
            "rustc",
            "--version",
        ])
        .await
        .is_ok();
}

#[tokio::test]
async fn file_override_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));
    let toolchain_file = cx.config.current_dir().join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        &format!("[toolchain]\npath='{}'", toolchain_path.to_str().unwrap()),
    )
    .unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();

    // Check that the toolchain has the right name
    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/nightly-[HOST_TRIPLE] (overridden by '[..]/rust-toolchain.toml')

"#]])
        .is_ok();
}

#[tokio::test]
async fn proxy_override_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));
    let toolchain_file = cx.config.current_dir().join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        &format!("[toolchain]\npath='{}'", toolchain_path.to_str().unwrap()),
    )
    .unwrap();

    cx.config
        .expect(["cargo", "--call-rustc"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_path_relative_not_supported() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));
    let toolchain_file = cx.config.current_dir().join("rust-toolchain.toml");

    // Find shared prefix so we can determine a relative path
    let mut p1 = toolchain_path.components().peekable();
    let mut p2 = toolchain_file.components().peekable();
    while let (Some(p1p), Some(p2p)) = (p1.peek(), p2.peek()) {
        if p1p == p2p {
            let _ = p1.next();
            let _ = p2.next();
        } else {
            // The two paths diverge here
            break;
        }
    }
    let mut relative_path = PathBuf::new();
    // NOTE: We skip 1 since we don't need to .. across the .toml file at the end of the path
    for _ in p2.skip(1) {
        relative_path.push("..");
    }
    for p in p1 {
        relative_path.push(p);
    }
    assert!(relative_path.is_relative());

    raw::write_file(
        &toolchain_file,
        &format!("[toolchain]\npath='{}'", relative_path.to_str().unwrap()),
    )
    .unwrap();

    // Change into an ephemeral dir so that we test that the path is relative to the override
    let ephemeral = cx.config.current_dir().join("ephemeral");
    fs::create_dir_all(&ephemeral).unwrap();

    let cx = cx.change_dir(&ephemeral);
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: relative path toolchain '[..]/toolchains/nightly-[HOST_TRIPLE]'

"#]])
        .is_err();
}

#[tokio::test]
async fn file_override_path_no_options() {
    let cx = CliTestContext::new(Scenario::None).await;
    // Make a plausible-looking toolchain
    let cwd = cx.config.current_dir();
    let toolchain_path = cwd.join("ephemeral");
    let toolchain_bin = toolchain_path.join("bin");
    fs::create_dir_all(toolchain_bin).unwrap();

    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        "[toolchain]\npath=\"ephemeral\"\ntargets=[\"dummy\"]",
    )
    .unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain options are ignored for path toolchain (ephemeral)

"#]])
        .is_err();

    raw::write_file(
        &toolchain_file,
        "[toolchain]\npath=\"ephemeral\"\ncomponents=[\"dummy\"]",
    )
    .unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain options are ignored for path toolchain (ephemeral)

"#]])
        .is_err();

    raw::write_file(
        &toolchain_file,
        "[toolchain]\npath=\"ephemeral\"\nprofile=\"minimal\"",
    )
    .unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain options are ignored for path toolchain (ephemeral)

"#]])
        .is_err();
}

#[tokio::test]
async fn file_override_path_xor_channel() {
    let cx = CliTestContext::new(Scenario::None).await;
    // Make a plausible-looking toolchain
    let cwd = cx.config.current_dir();
    let toolchain_path = cwd.join("ephemeral");
    let toolchain_bin = toolchain_path.join("bin");
    fs::create_dir_all(toolchain_bin).unwrap();

    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        "[toolchain]\npath=\"ephemeral\"\nchannel=\"nightly\"",
    )
    .unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
[..]cannot specify both channel (nightly) and path (ephemeral) simultaneously[..]
"#]])
        .is_err();
}

#[tokio::test]
async fn file_override_subdir() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    let subdir = cwd.join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    let cx = cx.change_dir(&subdir);
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_with_archive() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect(["rustup", "default", "stable"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly-2015-01-01"])
        .await
        .is_ok();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly-2015-01-01").unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_toml_format_select_installed_toolchain() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect(["rustup", "default", "stable"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly-2015-01-01"])
        .await
        .is_ok();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
channel = "nightly-2015-01-01"
"#,
    )
    .unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_toml_format_install_both_toolchain_and_components() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect(["rustup", "default", "stable"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .is_ok()
        .without_stdout("rust-src (installed)");

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
channel = "nightly-2015-01-01"
components = [ "rust-src" ]
"#,
    )
    .unwrap();

    cx.config
        .expect(["rustup", "toolchain", "install"])
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
        .expect(["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rust-src (installed)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_toml_format_add_missing_components() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .is_ok()
        .without_stdout("rust-src (installed)");

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
components = [ "rust-src" ]
"#,
    )
    .unwrap();

    cx.config
        .expect(["rustup", "toolchain", "install"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rust-src (installed)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_toml_format_add_missing_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .is_ok()
        .without_stdout(&format!("{CROSS_ARCH2} (installed)"));

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(
        &toolchain_file,
        &format!(
            r#"
[toolchain]
targets = [ "{CROSS_ARCH2}" ]
"#,
        ),
    )
    .unwrap();

    cx.config
        .expect(["rustup", "toolchain", "install"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rust-std-[CROSS_ARCH_II] (installed)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_toml_format_skip_invalid_component() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
components = [ "rust-bongo" ]
"#,
    )
    .unwrap();

    cx.config
        .expect(&["rustup", "toolchain", "install"])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: Force-skipping unavailable component 'rust-bongo-[HOST_TRIPLE]'
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_toml_format_specify_profile() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "set", "profile", "default"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "component", "list"])
        .await
        // The `rust-docs-[HOST_TRIPLE]` component is installed.
        .with_stdout(snapbox::str![[r#"
...
rust-docs-[HOST_TRIPLE] (installed)
...
"#]])
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
profile = "minimal"
channel = "nightly"
"#,
    )
    .unwrap();
    cx.config
        .expect(&["rustup", "toolchain", "install"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "component", "list"])
        .await
        // The `rust-docs-[HOST_TRIPLE]` component is not installed.
        .with_stdout(snapbox::str![[r#"
...
rust-docs-[HOST_TRIPLE]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn default_profile_is_respected_with_rust_toolchain_file() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "set", "profile", "minimal"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
channel = "nightly"
"#,
    )
    .unwrap();
    cx.config
        .expect(&["rustup", "toolchain", "install"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rust-docs-[HOST_TRIPLE]
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn close_file_override_beats_far_directory_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "toolchain", "install", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "override", "set", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();

    let cwd = cx.config.current_dir();

    let subdir = cwd.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    let toolchain_file = subdir.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    let cx = cx.change_dir(&subdir);
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

// Check that toolchain overrides have the correct priority.
#[tokio::test]
async fn override_order() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    let host = this_host_triple();
    // give each override type a different toolchain
    let default_tc = &format!("beta-2015-01-01-{host}");
    let env_tc = &format!("stable-2015-01-01-{host}");
    let dir_tc = &format!("beta-2015-01-02-{host}");
    let file_tc = &format!("stable-2015-01-02-{host}");
    let command_tc = &format!("nightly-2015-01-01-{host}");
    cx.config
        .expect(["rustup", "install", default_tc])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "install", env_tc])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "install", dir_tc])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "install", file_tc])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "install", command_tc])
        .await
        .is_ok();

    // No default
    cx.config
        .expect(["rustup", "default", "none"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
error: no active toolchain

"#]])
        .is_err();

    // Default
    cx.config
        .expect(["rustup", "default", default_tc])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .extend_redactions([("[DEFAULT_TC]", default_tc)])
        .with_stdout(snapbox::str![[r#"
[DEFAULT_TC] (default)

"#]])
        .is_ok();

    // file > default
    let toolchain_file = cx.config.current_dir().join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        &format!("[toolchain]\nchannel='{file_tc}'"),
    )
    .unwrap();
    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .extend_redactions([("[FILE_TC]", file_tc)])
        .with_stdout(snapbox::str![[r#"
[FILE_TC] (overridden by '[..]/rust-toolchain.toml')

"#]])
        .is_ok();

    // dir override > file > default
    cx.config
        .expect(["rustup", "override", "set", dir_tc])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .extend_redactions([("[DIR_TC]", dir_tc)])
        .with_stdout(snapbox::str![[r#"
[DIR_TC] (directory override for '[..]')

"#]])
        .is_ok();

    // env > dir override > file > default
    cx.config
        .expect_with_env(
            ["rustup", "show", "active-toolchain"],
            [("RUSTUP_TOOLCHAIN", &**env_tc)],
        )
        .await
        .extend_redactions([("[ENV_TC]", env_tc)])
        .with_stdout(snapbox::str![[r#"
[ENV_TC] (overridden by environment variable RUSTUP_TOOLCHAIN)

"#]])
        .is_ok();

    // +toolchain > env > dir override > file > default
    cx.config
        .expect_with_env(
            [
                "rustup",
                &format!("+{command_tc}"),
                "show",
                "active-toolchain",
            ],
            &[("RUSTUP_TOOLCHAIN", &**env_tc)],
        )
        .await
        .extend_redactions([("[COMMAND_TC]", command_tc)])
        .with_stdout(snapbox::str![[r#"
[COMMAND_TC] (overridden by +toolchain on the command line)

"#]])
        .is_ok();
}

#[tokio::test]
async fn directory_override_doesnt_need_to_exist_unless_it_is_selected() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "beta"])
        .await
        .is_ok();
    // not installing nightly

    cx.config
        .expect(["rustup", "override", "set", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn env_override_beats_file_override() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect_with_env(["rustc", "--version"], [("RUSTUP_TOOLCHAIN", "beta")])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn plus_override_beats_file_override() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect(["rustc", "+beta", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn file_override_not_installed_custom() {
    let cx = CliTestContext::new(Scenario::None).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "gumbo").unwrap();

    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: custom toolchain 'gumbo' specified in override file '[..]/rust-toolchain' is not installed
...
"#]])
        .is_err();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: custom toolchain 'gumbo' specified in override file '[..]/rust-toolchain' is not installed
...
"#]])
        .is_err();
}

#[tokio::test]
async fn file_override_not_installed_custom_toml() {
    let cx = CliTestContext::new(Scenario::None).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(&toolchain_file, r#"toolchain.channel = "i-am-the-walrus""#).unwrap();

    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: custom toolchain 'i-am-the-walrus' specified in override file '[..]/rust-toolchain.toml' is not installed
...
"#]])
        .is_err();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: custom toolchain 'i-am-the-walrus' specified in override file '[..]/rust-toolchain.toml' is not installed
...
"#]])
        .is_err();
}

#[tokio::test]
async fn bad_file_override() {
    let cx = CliTestContext::new(Scenario::None).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    // invalid name - cannot specify no toolchain in a toolchain file
    raw::write_file(&toolchain_file, "none").unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: invalid toolchain name detected in override file '[..]/rust-toolchain'
...
"#]])
        .is_err();
}

// https://github.com/rust-lang/rustup/issues/4053
#[tokio::test]
async fn bad_file_override_with_manip() {
    let cx = CliTestContext::new(Scenario::None).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        r#"toolchain.channel = "nightly', please install with 'curl --proto '=https' --tlsv1.2 -sSf https://sh.rust-toolchain.rs/ | sh -s -- --default-toolchain nightly -y""#,
    ).unwrap();

    cx.config
        .expect(["rustup", "show", "active-toolchain"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: invalid toolchain name detected in override file '[..]/rust-toolchain.toml'
...
"#]])
        .is_err();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: invalid toolchain name detected in override file '[..]/rust-toolchain.toml'
...
"#]])
        .is_err();
}

#[tokio::test]
async fn valid_override_settings() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    cx.config
        .expect(&["rustup", "default", "nightly"])
        .await
        .is_ok();
    let nightly_with_host = format!("nightly-{}", this_host_triple());
    raw::write_file(&toolchain_file, "nightly").unwrap();
    cx.config.expect(&["rustc", "--version"]).await.is_ok();
    // Special case: same version as is installed is permitted.
    raw::write_file(&toolchain_file, &nightly_with_host).unwrap();
    cx.config.expect(&["rustc", "--version"]).await.is_ok();
    let fullpath = cx
        .config
        .rustupdir
        .clone()
        .join("toolchains")
        .join(&nightly_with_host);
    cx.config
        .expect(&[
            "rustup",
            "toolchain",
            "link",
            "system",
            &format!("{}", fullpath.display()),
        ])
        .await
        .is_ok();
    raw::write_file(&toolchain_file, "system").unwrap();
    cx.config.expect(&["rustc", "--version"]).await.is_ok();
}

#[tokio::test]
async fn file_override_with_target_info() {
    // Target info is not portable between machines, so we reject toolchain
    // files that include it.
    let cx = CliTestContext::new(Scenario::None).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly-x86_64-unknown-linux-gnu").unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .remove_redactions(["[HOST_TRIPLE]"])
        .with_stderr(snapbox::str![[r#"
...
error: target triple in channel name 'nightly-x86_64-unknown-linux-gnu'
...
"#]])
        .is_err();
}

#[tokio::test]
async fn docs_with_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(&["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "doc", "--path"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/stable-[HOST_TRIPLE]/share/doc/rust/html/index.html

"#]])
        .is_ok();

    cx.config
        .expect(["rustup", "doc", "--path", "--toolchain", "nightly"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/nightly-[HOST_TRIPLE]/share/doc/rust/html/index.html

"#]])
        .is_ok();
}

#[tokio::test]
async fn docs_topical_with_path() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();

    for (args, path) in topical_doc_data::test_cases() {
        let mut cmd = vec!["rustup", "doc", "--path"];
        cmd.extend(args);
        cx.config
            .expect(cmd)
            .await
            .extend_redactions([("[PATH]", path)])
            .is_ok()
            .with_stdout(snapbox::str![[r#"
[..]/toolchains/stable-[HOST_TRIPLE]/[PATH]

"#]])
            .with_stderr(snapbox::str![""]);
    }
}

#[tokio::test]
async fn docs_missing() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "set", "profile", "minimal"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "doc"])
        .await
        .with_stderr(snapbox::str![[r#"
info: `rust-docs` not installed in toolchain `nightly-[HOST_TRIPLE]`
info: To install, try `rustup component add --toolchain nightly-[HOST_TRIPLE] rust-docs`
error: unable to view documentation which is not installed

"#]])
        .is_err();
}

#[tokio::test]
async fn docs_custom() {
    let cx = CliTestContext::new(Scenario::None).await;
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "custom", &path])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "custom"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "doc", "--path"])
        .await
        .with_stdout(snapbox::str![[r#"
[..]/toolchains/custom/share/doc/rust/html/index.html

"#]])
        .is_ok();
}

#[cfg(unix)]
#[tokio::test]
async fn non_utf8_arg() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect_with_env(
            [
                OsStr::new("rustc"),
                OsStr::new("--echo-args"),
                OsStr::new("echoed non-utf8 arg:"),
                OsStr::from_bytes(b"\xc3\x28"),
            ],
            [("RUST_BACKTRACE", "1")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
echoed non-utf8 arg:
...
"#]]);
}

#[cfg(windows)]
#[tokio::test]
async fn non_utf8_arg() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect_with_env(
            [
                OsString::from("rustc"),
                OsString::from("--echo-args"),
                OsString::from("echoed non-utf8 arg:"),
                OsString::from_wide(&[0xd801, 0xd801]),
            ],
            [("RUST_BACKTRACE", "1")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
echoed non-utf8 arg:
...
"#]]);
}

#[cfg(unix)]
#[tokio::test]
async fn non_utf8_toolchain() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect_with_env(
            [OsStr::new("rustc"), OsStr::from_bytes(b"+\xc3\x28")],
            [("RUST_BACKTRACE", "1")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain '�(' is not installed
...
"#]]);
}

#[cfg(windows)]
#[tokio::test]
async fn non_utf8_toolchain() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect_with_env(
            [
                OsString::from("rustc"),
                OsString::from_wide(&[u16::from(b'+'), 0xd801, 0xd801]),
            ],
            [("RUST_BACKTRACE", "1")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain '��' is not installed
...
"#]]);
}

#[tokio::test]
async fn check_host_goes_away() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::HostGoesMissingBefore);
        cx.config
            .expect(["rustup", "default", "nightly"])
            .await
            .is_ok();
    }

    let cx = cx.with_dist_dir(Scenario::HostGoesMissingAfter);
    cx.config
        .expect(&["rustup", "update", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: target '[HOST_TRIPLE]' not found in channel[..]
...
"#]])
        .is_err();
}

#[cfg(unix)]
#[tokio::test]
async fn check_unix_settings_fallback() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    // No default toolchain specified yet
    cx.config
        .expect(["rustup", "default"])
        .await
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
error: no default toolchain is configured

"#]])
        .is_err();

    // Default toolchain specified in fallback settings file
    let mock_settings_file = cx.config.current_dir().join("mock_fallback_settings.toml");
    raw::write_file(
        &mock_settings_file,
        &format!("default_toolchain = 'nightly-{}'", this_host_triple()),
    )
    .unwrap();

    cx.config
        .expect_with_env(
            ["rustup", "default"],
            [(
                "RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS",
                &*mock_settings_file.display().to_string(),
            )],
        )
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE] (default)

"#]])
        .is_ok();
}

#[tokio::test]
async fn deny_incompatible_toolchain_install() {
    let cx = CliTestContext::new(Scenario::MultiHost).await;
    let arch = MULTI_ARCH1;
    cx.config
        .expect(["rustup", "toolchain", "install", &format!("nightly-{arch}")])
        .await
        .extend_redactions([("[ARCH]", arch)])
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[ARCH]' may not be able to run on this system
note: to build software for that platform, try `rustup target add [ARCH]` instead
note: add the `--force-non-host` flag to install the toolchain anyway

"#]])
        .is_err();
}

#[tokio::test]
async fn deny_incompatible_toolchain_default() {
    let cx = CliTestContext::new(Scenario::MultiHost).await;
    let arch = MULTI_ARCH1;
    cx.config
        .expect(["rustup", "default", &format!("nightly-{arch}")])
        .await
        .extend_redactions([("[ARCH]", arch)])
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[ARCH]' may not be able to run on this system
note: to build software for that platform, try `rustup target add [ARCH]` instead
note: add the `--force-non-host` flag to install the toolchain anyway

"#]])
        .is_err();
}

#[tokio::test]
async fn dont_warn_on_partial_build() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let arch = this_host_triple().split_once('-').unwrap().0.to_owned();
    cx.config
        .expect(["rustup", "toolchain", "install", &format!("nightly-{arch}")])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
...
"#]])
        .is_ok()
        .without_stderr(&format!(
            "toolchain 'nightly-{arch}' may not be able to run on this system."
        ));
}

/// Checks that `rust-toolchain.toml` files are considered
#[tokio::test]
async fn rust_toolchain_toml() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: rustup could not choose a version of rustc to run, because one wasn't specified explicitly, and no default is configured.
...
"#]])
        .is_err();

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(&toolchain_file, "[toolchain]\nchannel = \"nightly\"").unwrap();
    cx.config
        .expect(["rustup", "toolchain", "install"])
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

/// Ensures that `rust-toolchain.toml` files (with `.toml` extension) only allow TOML contents
#[tokio::test]
async fn only_toml_in_rust_toolchain_toml() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect(["rustc", "--version"])
        .await
        .extend_redactions([("[CWD]", &cwd)])
        .with_stderr(snapbox::str![[r#"
...
error: could not parse override file: '[CWD]/rust-toolchain.toml'[..]
...
"#]])
        .is_err();
}

/// Checks that a warning occurs if both `rust-toolchain` and `rust-toolchain.toml` files exist
#[tokio::test]
async fn warn_on_duplicate_rust_toolchain_file() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    let toolchain_file_1 = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file_1, "stable").unwrap();
    let toolchain_file_2 = cwd.join("rust-toolchain.toml");
    raw::write_file(&toolchain_file_2, "[toolchain]").unwrap();

    cx.config
        .expect(&["rustup", "toolchain", "install"])
        .await
        .extend_redactions([("[CWD]", &cwd.canonicalize().unwrap())])
        .with_stderr(snapbox::str![[r#"
...
warn: both `[CWD]/rust-toolchain` and `[CWD]/rust-toolchain.toml` exist. Using `[CWD]/rust-toolchain`
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn custom_toolchain_with_components_toolchains_profile_does_not_err() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");

    // install a toolchain so we can make a custom toolchain that links to it
    cx.config
        .expect(&[
            "rustup",
            "toolchain",
            "install",
            "nightly",
            "--profile=minimal",
            "--component=cargo",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component[..]
...
info: default toolchain set to 'nightly-[HOST_TRIPLE]'

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);

    // link the toolchain
    let toolchains = cx.config.rustupdir.join("toolchains");
    raw::symlink_dir(
        &toolchains.join(format!("nightly-{}", this_host_triple())),
        &toolchains.join("my-custom"),
    )
    .expect("failed to symlink");

    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
channel = "my-custom"
components = ["rustc-dev"]
targets = ["x86_64-unknown-linux-gnu"]
profile = "minimal"
"#,
    )
    .unwrap();

    cx.config
        .expect(&["rustup", "show", "active-toolchain"])
        .await
        .extend_redactions([("[CWD]", &cwd)])
        .with_stdout(snapbox::str![[r#"
my-custom (overridden by '[CWD]/rust-toolchain.toml')

"#]])
        .is_ok();

    cx.config
        .expect(&["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();

    cx.config
        .expect(&["cargo", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

// Issue #4251
#[tokio::test]
async fn show_custom_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(&["rustup", "default", "stable"])
        .await
        .is_ok();
    let stable_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("stable-{}", this_host_triple()));
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "stuff",
            &stable_path.to_string_lossy(),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "+stuff", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
stable-[HOST_TRIPLE] (default)
stuff (active)

active toolchain
----------------
name: stuff
active because: overridden by +toolchain on the command line
installed targets:
  [HOST_TRIPLE]

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}

#[tokio::test]
async fn show_custom_toolchain_without_components_file() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    let stable_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("stable-{}", this_host_triple()));
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "stuff",
            &stable_path.to_string_lossy(),
        ])
        .await
        .is_ok();

    let components_file = stable_path.join("lib").join("rustlib").join("components");
    fs::remove_file(&components_file).unwrap();
    cx.config
        .expect(["rustup", "+stuff", "show"])
        .await
        .extend_redactions([("[RUSTUP_DIR]", cx.config.rustupdir.to_string())])
        .with_stdout(snapbox::str![[r#"
Default host: [HOST_TRIPLE]
rustup home:  [RUSTUP_DIR]

installed toolchains
--------------------
stable-[HOST_TRIPLE] (default)
stuff (active)

active toolchain
----------------
name: stuff
active because: overridden by +toolchain on the command line
installed targets:

"#]])
        .with_stderr(snapbox::str![[""]])
        .is_ok();
}
