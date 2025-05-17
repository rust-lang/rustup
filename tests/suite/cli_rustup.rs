//! Test cases for new rustup UI

#![allow(deprecated)]

use std::fs;
use std::path::{MAIN_SEPARATOR, PathBuf};
use std::{env::consts::EXE_SUFFIX, path::Path};

use rustup::for_host;
use rustup::test::{
    CROSS_ARCH1, CROSS_ARCH2, CliTestContext, MULTI_ARCH1, Scenario, this_host_triple,
    topical_doc_data,
};
use rustup::utils::raw;

macro_rules! for_host_and_home {
    ($config:expr, $s:tt $($arg:tt)*) => {
        &format!($s, this_host_triple(), $config.rustupdir $($arg)*)
    };
}

#[tokio::test]
async fn rustup_stable() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect_ok(&["rustup", "toolchain", "add", "stable"])
            .await;
    }

    let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect_ok_ex(
            &["rustup", "update"],
            for_host!(
                r"
  stable-{0} updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: cleaning up downloads & tmp directories
"
            ),
        )
        .await;
}

#[tokio::test]
async fn rustup_stable_quiet() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect_ok(&["rustup", "--quiet", "update", "stable"])
            .await;
    }

    let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect_ok_ex(
            &["rustup", "--quiet", "update"],
            for_host!(
                r"
  stable-{0} updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))

"
            ),
            "",
        )
        .await;
}

#[tokio::test]
async fn rustup_stable_no_change() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2_2015_01_01).await;
    cx.config.expect_ok(&["rustup", "update", "stable"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "update"],
            for_host!(
                r"
  stable-{0} unchanged - 1.0.0 (hash-stable-1.0.0)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: cleaning up downloads & tmp directories
"
            ),
        )
        .await;
}

#[tokio::test]
async fn rustup_all_channels() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"])
            .await;
    }

    let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect_ok_ex(
            &["rustup", "update"],
            for_host!(
                r"
   stable-{0} updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))
     beta-{0} updated - 1.2.0 (hash-beta-1.2.0) (from 1.1.0 (hash-beta-1.1.0))
  nightly-{0} updated - 1.3.0 (hash-nightly-2) (from 1.2.0 (hash-nightly-1))

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: syncing channel updates for 'beta-{0}'
info: latest update on 2015-01-02, rust version 1.2.0 (hash-beta-1.2.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: cleaning up downloads & tmp directories
"
            ),
        )
        .await;
}

#[tokio::test]
async fn rustup_some_channels_up_to_date() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
        cx.config
            .expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"])
            .await;
    }

    let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "update", "beta"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "update"],
            for_host!(
                r"
   stable-{0} updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))
   beta-{0} unchanged - 1.2.0 (hash-beta-1.2.0)
  nightly-{0} updated - 1.3.0 (hash-nightly-2) (from 1.2.0 (hash-nightly-1))

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: syncing channel updates for 'beta-{0}'
info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: cleaning up downloads & tmp directories
"
            ),
        )
        .await;
}

#[tokio::test]
async fn rustup_no_channels() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "update"],
            r"",
            r"info: no updatable toolchains installed
info: cleaning up downloads & tmp directories
",
        )
        .await;
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
async fn default_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "add", "nightly"])
        .await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "override", "set", "nightly"])
        .await;
    cx.config
        .expect_stderr_ok(
            &["rustup", "default", "stable"],
            for_host!(
                r"info: using existing install for 'stable-{0}'
info: default toolchain set to 'stable-{0}'
info: note that the toolchain 'nightly-{0}' is currently in use (directory override for"
            ),
        )
        .await;
}

#[tokio::test]
async fn rustup_zstd() {
    let cx = CliTestContext::new(Scenario::ArchivesV2_2015_01_01).await;
    cx.config
        .expect_stderr_ok(
            &["rustup", "--verbose", "toolchain", "add", "nightly"],
            for_host!(r"dist/2015-01-01/rust-std-nightly-{0}.tar.zst"),
        )
        .await;
}

#[tokio::test]
async fn add_target() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH1])
        .await;
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn remove_target() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH1])
        .await;
    assert!(cx.config.rustupdir.has(&path));
    cx.config
        .expect_ok(&["rustup", "target", "remove", CROSS_ARCH1])
        .await;
    assert!(!cx.config.rustupdir.has(&path));
}

#[tokio::test]
async fn add_remove_multiple_targets() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH1, CROSS_ARCH2])
        .await;
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
        .expect_ok(&["rustup", "target", "remove", CROSS_ARCH1, CROSS_ARCH2])
        .await;
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
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustup", "target", "list"], CROSS_ARCH1)
        .await;
}

#[tokio::test]
async fn list_installed_targets() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();

    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustup", "target", "list", "--installed"], &trip)
        .await;
}

#[tokio::test]
async fn add_target_explicit() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    cx.config
        .expect_ok(&["rustup", "toolchain", "add", "nightly"])
        .await;
    cx.config
        .expect_ok(&[
            "rustup",
            "target",
            "add",
            "--toolchain",
            "nightly",
            CROSS_ARCH1,
        ])
        .await;
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn remove_target_explicit() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        &this_host_triple(),
        CROSS_ARCH1
    );
    cx.config
        .expect_ok(&["rustup", "toolchain", "add", "nightly"])
        .await;
    cx.config
        .expect_ok(&[
            "rustup",
            "target",
            "add",
            "--toolchain",
            "nightly",
            CROSS_ARCH1,
        ])
        .await;
    assert!(cx.config.rustupdir.has(&path));
    cx.config
        .expect_ok(&[
            "rustup",
            "target",
            "remove",
            "--toolchain",
            "nightly",
            CROSS_ARCH1,
        ])
        .await;
    assert!(!cx.config.rustupdir.has(&path));
}

#[tokio::test]
async fn list_targets_explicit() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "add", "nightly"])
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "target", "list", "--toolchain", "nightly"],
            CROSS_ARCH1,
        )
        .await;
}

#[tokio::test]
async fn link() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "custom", &path])
        .await;
    cx.config.expect_ok(&["rustup", "default", "custom"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-c-1")
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "show"], "custom (active, default)")
        .await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustup", "show"], "custom")
        .await;
}

// Issue #809. When we call the fallback cargo, when it in turn invokes
// "rustc", that rustc should actually be the rustup proxy, not the toolchain rustc.
// That way the proxy can pick the correct toolchain.
#[tokio::test]
async fn fallback_cargo_calls_correct_rustc() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
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
        .expect_ok(&["rustup", "toolchain", "link", "custom", &path])
        .await;
    cx.config.expect_ok(&["rustup", "default", "custom"]).await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-c-1")
        .await;
    cx.config
        .expect_stdout_ok(&["cargo", "--version"], "hash-nightly-2")
        .await;

    assert!(rustc_path.exists());

    // Here --call-rustc tells the mock cargo bin to exec `rustc --version`.
    // We should be ultimately calling the custom rustc, according to the
    // RUSTUP_TOOLCHAIN variable set by the original "cargo" proxy, and
    // interpreted by the nested "rustc" proxy.
    cx.config
        .expect_stdout_ok(&["cargo", "--call-rustc"], "hash-c-1")
        .await;
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
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;

    // We need an intermediary to run cargo itself.
    // The "mock" cargo can't do that because on Windows it will check
    // for a `cargo.exe` in the current directory before checking PATH.
    //
    // The solution here is to copy from the "mock" `cargo.exe` into
    // `~/.cargo/bin/cargo-foo`. This is just for convenience to avoid
    // needing to build another executable just for this test.
    let output = cx.config.run("rustup", ["which", "cargo"], &[]).await;
    let real_mock_cargo = output.stdout.trim();
    let cargo_bin_path = cx.config.cargodir.join("bin");
    let cargo_subcommand = cargo_bin_path.join(format!("cargo-foo{EXE_SUFFIX}"));
    fs::create_dir_all(&cargo_bin_path).unwrap();
    fs::copy(real_mock_cargo, cargo_subcommand).unwrap();

    cx.config
        .expect_stdout_ok(&["cargo", "--recursive-cargo-subcommand"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn show_home() {
    let mut cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show", "home"],
            &format!(
                r"{}
",
                cx.config.rustupdir
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_toolchain_none() {
    let mut cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show"],
            for_host_and_home!(
                &cx.config,
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------

active toolchain
----------------
no active toolchain
"
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_toolchain_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show"],
            for_host_and_home!(
                cx.config,
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
active because: it's the default toolchain
installed targets:
  {0}
"
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_no_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "install", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "default", "none"]).await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "show"],
            for_host!(
                "\
installed toolchains
--------------------
nightly-{0}

active toolchain
"
            ),
        )
        .await;
}

#[tokio::test]
async fn show_no_default_active() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "install", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "default", "none"]).await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "+nightly", "show"],
            for_host!(
                "\
installed toolchains
--------------------
nightly-{0} (active)

active toolchain
"
            ),
        )
        .await;
}

#[tokio::test]
async fn show_multiple_toolchains() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "update", "stable"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show"],
            for_host_and_home!(
                cx.config,
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
stable-{0}
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
active because: it's the default toolchain
installed targets:
  {0}
"
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_multiple_targets() {
    let mut cx = CliTestContext::new(Scenario::MultiHost).await;
    cx.config
        .expect_ok(&[
            "rustup",
            "default",
            &format!("nightly-{MULTI_ARCH1}"),
            "--force-non-host",
        ])
        .await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH2])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show"],
            &format!(
                r"Default host: {2}
rustup home:  {3}

installed toolchains
--------------------
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
active because: it's the default toolchain
installed targets:
  {1}
  {0}
",
                MULTI_ARCH1,
                CROSS_ARCH2,
                this_host_triple(),
                cx.config.rustupdir
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_multiple_toolchains_and_targets() {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86") {
        return;
    }

    let mut cx = CliTestContext::new(Scenario::MultiHost).await;
    cx.config
        .expect_ok(&[
            "rustup",
            "default",
            &format!("nightly-{MULTI_ARCH1}"),
            "--force-non-host",
        ])
        .await;
    cx.config
        .expect_ok(&["rustup", "target", "add", CROSS_ARCH2])
        .await;
    cx.config
        .expect_ok(&[
            "rustup",
            "update",
            "--force-non-host",
            &format!("stable-{MULTI_ARCH1}"),
        ])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show"],
            &format!(
                r"Default host: {2}
rustup home:  {3}

installed toolchains
--------------------
stable-{0}
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
active because: it's the default toolchain
installed targets:
  {1}
  {0}
",
                MULTI_ARCH1,
                CROSS_ARCH2,
                this_host_triple(),
                cx.config.rustupdir
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn list_default_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "toolchain", "list"],
            for_host!("nightly-{0} (active, default)\n"),
            r"",
        )
        .await;
}

#[tokio::test]
async fn list_default_toolchain_quiet() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "toolchain", "list", "--quiet"],
            for_host!("nightly-{0}\n"),
            r"",
        )
        .await;
}

#[tokio::test]
async fn list_no_default_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "install", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "default", "none"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "toolchain", "list"],
            for_host!("nightly-{0}\n"),
            r"",
        )
        .await;
}

#[tokio::test]
async fn list_no_default_override_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "override", "set", "nightly"])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "toolchain", "list"],
            for_host!("nightly-{0} (active)\n"),
            r"",
        )
        .await;
}

#[tokio::test]
async fn list_default_and_override_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "override", "set", "nightly"])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "toolchain", "list"],
            for_host!("nightly-{0} (active, default)\n"),
            r"",
        )
        .await;
}

#[tokio::test]
#[ignore = "FIXME: Windows shows UNC paths"]
async fn show_toolchain_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show"],
            &format!(
                r"Default host: {0}
rustup home:  {1}

nightly-{0} (directory override for '{2}')
1.3.0 (hash-nightly-2)
",
                this_host_triple(),
                cx.config.rustupdir,
                cwd.display(),
            ),
            r"",
        )
        .await;
}

#[tokio::test]
#[ignore = "FIXME: Windows shows UNC paths"]
async fn show_toolchain_toolchain_file_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");

    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect_ok_ex(
            &["rustup", "show"],
            &format!(
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------

stable-{0} (default)
nightly-{0}

active toolchain
----------------

nightly-{0} (overridden by '{2}')
1.3.0 (hash-nightly-2)

",
                this_host_triple(),
                cx.config.rustupdir,
                toolchain_file.display()
            ),
            r"",
        )
        .await;
}

#[tokio::test]
#[ignore = "FIXME: Windows shows UNC paths"]
async fn show_toolchain_version_nested_file_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"])
        .await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");

    raw::write_file(&toolchain_file, "nightly").unwrap();

    let subdir = cwd.join("foo");

    fs::create_dir_all(&subdir).unwrap();
    let mut cx = cx.change_dir(&subdir);
    cx.config
        .expect_ok_ex(
            &["rustup", "show"],
            &format!(
                r"Default host: {0}

installed toolchains
--------------------

stable-{0} (default)
nightly-{0}

active toolchain
----------------

nightly-{0} (overridden by '{1}')
1.3.0 (hash-nightly-2)

",
                this_host_triple(),
                toolchain_file.display()
            ),
            r"",
        )
        .await;
}

#[tokio::test]
#[ignore = "FIXME: Windows shows UNC paths"]
async fn show_toolchain_toolchain_file_override_not_installed() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");

    raw::write_file(&toolchain_file, "nightly").unwrap();

    // I'm not sure this should really be erroring when the toolchain
    // is not installed; just capturing the behavior.
    let mut cmd = cx.config.cmd("rustup", ["show"]);
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.starts_with("error: override toolchain 'nightly' is not installed"));
    assert!(stderr.contains(&format!(
        "the toolchain file at '{}' specifies an uninstalled toolchain",
        toolchain_file.display()
    )));
}

#[tokio::test]
async fn show_toolchain_override_not_installed() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"])
        .await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "nightly"])
        .await;
    let out = cx.config.run("rustup", ["show"], &[]).await;
    assert!(out.ok);
    assert!(
        !out.stderr
            .contains("is not installed: the directory override for")
    );
    assert!(out.stderr.contains("info: installing component 'rustc'"));
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
        let mut cx = cx.change_dir(emptydir.path());
        cx.config
            .expect_ok(&["rustup", "override", "set", "nightly", "--path", cwd_str])
            .await;
    }

    cx.config
        .expect_ok_ex(
            &["rustup", "override", "list"],
            &format!("{}\tnightly-{}\n", cwd_str, this_host_triple()),
            r"",
        )
        .await;

    {
        let mut cx = cx.change_dir(emptydir.path());
        cx.config
            .expect_ok(&["rustup", "override", "unset", "--path", cwd_str])
            .await;
    }

    cx.config
        .expect_ok_ex(&["rustup", "override", "list"], "no overrides\n", r"")
        .await;
}

#[tokio::test]
async fn show_toolchain_env() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    let out = cx
        .config
        .run("rustup", ["show"], &[("RUSTUP_TOOLCHAIN", "nightly")])
        .await;
    assert!(out.ok);
    assert_eq!(
        &out.stdout,
        for_host_and_home!(
            cx.config,
            r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
active because: overridden by environment variable RUSTUP_TOOLCHAIN
installed targets:
  {0}
"
        )
    );
}

#[tokio::test]
async fn show_toolchain_env_not_installed() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let out = cx
        .config
        .run("rustup", ["show"], &[("RUSTUP_TOOLCHAIN", "nightly")])
        .await;

    assert!(out.ok);

    let expected_out = for_host_and_home!(
        cx.config,
        r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------

active toolchain
----------------
name: nightly-{0}
active because: overridden by environment variable RUSTUP_TOOLCHAIN
installed targets:
  {0}
"
    );
    assert!(&out.stdout == expected_out);
}

#[tokio::test]
async fn show_active_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show", "active-toolchain"],
            for_host!("nightly-{0} (default)\n"),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_with_verbose() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    }

    let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    let config = &mut cx.config;
    config
        .expect_ok(&["rustup", "update", "nightly-2015-01-01"])
        .await;
    config
        .expect_ok_ex(
            &["rustup", "show", "--verbose"],
            for_host_and_home!(
                config,
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
nightly-{0} (active, default)
  1.3.0 (hash-nightly-2)
  path: {2}

nightly-2015-01-01-{0}
  1.2.0 (hash-nightly-1)
  path: {3}

active toolchain
----------------
name: nightly-{0}
active because: it's the default toolchain
compiler: 1.3.0 (hash-nightly-2)
path: {2}
installed targets:
  {0}
",
                config
                    .rustupdir
                    .join("toolchains")
                    .join(for_host!("nightly-{0}"))
                    .display(),
                config
                    .rustupdir
                    .join("toolchains")
                    .join(for_host!("nightly-2015-01-01-{0}"))
                    .display()
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_active_toolchain_with_verbose() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show", "active-toolchain", "--verbose"],
            for_host!(
                r"nightly-{0}
active because: it's the default toolchain
compiler: 1.3.0 (hash-nightly-2)
path: {1}
",
                cx.config
                    .rustupdir
                    .join("toolchains")
                    .join(for_host!("nightly-{0}"))
                    .display()
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_active_toolchain_with_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "override", "set", "stable"])
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "show", "active-toolchain"],
            for_host!("stable-{0}"),
        )
        .await;
}

#[tokio::test]
async fn show_active_toolchain_with_override_verbose() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "override", "set", "stable"])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "show", "active-toolchain", "--verbose"],
            for_host!(
                r"stable-{0}
active because: directory override for '{1}'
compiler: 1.1.0 (hash-stable-1.1.0)
path: {2}
",
                cx.config.current_dir().display(),
                cx.config
                    .rustupdir
                    .join("toolchains")
                    .join(for_host!("stable-{0}"))
                    .display(),
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_active_toolchain_none() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect_err_ex(
            &["rustup", "show", "active-toolchain"],
            "",
            "error: no active toolchain\n",
        )
        .await;
}

#[tokio::test]
async fn show_profile() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_stdout_ok(&["rustup", "show", "profile"], "default")
        .await;

    // Check we get the same thing after we add or remove a component.
    cx.config
        .expect_ok(&["rustup", "component", "add", "rust-src"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "show", "profile"], "default")
        .await;
    cx.config
        .expect_ok(&["rustup", "component", "remove", "rustc"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "show", "profile"], "default")
        .await;
}

// #846
#[tokio::test]
async fn set_default_host() {
    let mut cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect_ok(&["rustup", "set", "default-host", &this_host_triple()])
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "show"], for_host!("Default host: {0}"))
        .await;
}

// #846
#[tokio::test]
async fn set_default_host_invalid_triple() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect_err(
            &["rustup", "set", "default-host", "foo"],
            "error: Provided host 'foo' couldn't be converted to partial triple",
        )
        .await;
}

// #745
#[tokio::test]
async fn set_default_host_invalid_triple_valid_partial() {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect_err(
            &["rustup", "set", "default-host", "x86_64-msvc"],
            "error: Provided host 'x86_64-msvc' did not specify an operating system",
        )
        .await;
}

// #422
#[tokio::test]
async fn update_doesnt_update_non_tracking_channels() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    }

    let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    cx.config
        .expect_ok(&["rustup", "update", "nightly-2015-01-01"])
        .await;
    let mut cmd = cx.config.cmd("rustup", ["update"]);
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(!stderr.contains(for_host!(
        "syncing channel updates for 'nightly-2015-01-01-{}'"
    )));
}

#[tokio::test]
async fn toolchain_install_is_like_update() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "run", "nightly", "rustc", "--version"],
            "hash-nightly-2",
        )
        .await;
}

#[tokio::test]
async fn toolchain_install_is_like_update_quiet() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "--quiet", "toolchain", "install", "nightly"])
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "run", "nightly", "rustc", "--version"],
            "hash-nightly-2",
        )
        .await;
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
        .expect_stderr_ok(
            &["rustup", "toolchain", "install"],
            &format!(
                "\
info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'rustc'
info: installing component 'rustc'
info: the active toolchain `nightly-{0}` has been installed
info: it's active because: overridden by '{1}'",
                this_host_triple(),
                toolchain_file.display(),
            ),
        )
        .await;

    cx.config
        .expect_stderr_ok(
            &["rustup", "toolchain", "install"],
            &format!(
                "\
info: using existing install for 'nightly-{0}'
info: the active toolchain `nightly-{0}` has been installed
info: it's active because: overridden by '{1}'",
                this_host_triple(),
                toolchain_file.display(),
            ),
        )
        .await;
}

#[tokio::test]
async fn toolchain_update_is_like_update() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "update", "nightly"])
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "run", "nightly", "rustc", "--version"],
            "hash-nightly-2",
        )
        .await;
}

#[tokio::test]
async fn toolchain_uninstall_is_like_uninstall() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect_ok(&["rustup", "toolchain", "install", "nightly"])
            .await;
    }

    cx.config.expect_ok(&["rustup", "default", "none"]).await;
    cx.config
        .expect_ok(&["rustup", "uninstall", "nightly"])
        .await;
    cx.config
        .expect_not_stdout_ok(&["rustup", "show"], for_host!("'nightly-{}'"))
        .await;
}

#[tokio::test]
async fn proxy_toolchain_shorthand() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "update", "nightly"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "+stable", "--version"], "hash-stable-1.1.0")
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "+nightly", "--version"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn add_component() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rust-src"])
        .await;
    let path = format!(
        "toolchains/stable-{}/lib/rustlib/src/rust-src/foo.rs",
        this_host_triple()
    );
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn add_component_by_target_triple() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&[
            "rustup",
            "component",
            "add",
            &format!("rust-std-{CROSS_ARCH1}"),
        ])
        .await;
    let path = format!(
        "toolchains/stable-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        CROSS_ARCH1
    );
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn add_component_by_target_triple_renamed_from() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", for_host!("rls-{}")])
        .await;
    cx.config
        .expect_ok_contains(
            &["rustup", "component", "list", "--installed"],
            for_host!("rls-{}"),
            "",
        )
        .await;
}

#[tokio::test]
async fn add_component_by_target_triple_renamed_to() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", for_host!("rls-preview-{}")])
        .await;
    cx.config
        .expect_ok_contains(
            &["rustup", "component", "list", "--installed"],
            for_host!("rls-{}"),
            "",
        )
        .await;
}

#[tokio::test]
async fn fail_invalid_component_name() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_err(
            &[
                "rustup",
                "component",
                "add",
                &format!("dummy-{CROSS_ARCH1}"),
            ],
            &format!(
            "error: toolchain 'stable-{}' does not contain component 'dummy-{}' for target '{}'",
            this_host_triple(),
            CROSS_ARCH1,
            this_host_triple()
        ),
        )
        .await;
}

#[tokio::test]
async fn fail_invalid_component_target() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config.expect_err(
        &[
            "rustup",
            "component",
            "add",
            "rust-std-invalid-target",
        ],
        &format!("error: toolchain 'stable-{}' does not contain component 'rust-std-invalid-target' for target '{}'",this_host_triple(),  this_host_triple()),
    ).await;
}

#[tokio::test]
async fn remove_component() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rust-src"])
        .await;
    let path = PathBuf::from(format!(
        "toolchains/stable-{}/lib/rustlib/src/rust-src/foo.rs",
        this_host_triple()
    ));
    assert!(cx.config.rustupdir.has(&path));
    cx.config
        .expect_ok(&["rustup", "component", "remove", "rust-src"])
        .await;
    assert!(!cx.config.rustupdir.has(path.parent().unwrap()));
}

#[tokio::test]
async fn remove_component_by_target_triple() {
    let component_with_triple = format!("rust-std-{CROSS_ARCH1}");
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "component", "add", &component_with_triple])
        .await;
    let path = PathBuf::from(format!(
        "toolchains/stable-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        CROSS_ARCH1
    ));
    assert!(cx.config.rustupdir.has(&path));
    cx.config
        .expect_ok(&["rustup", "component", "remove", &component_with_triple])
        .await;
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

    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&[
            "rustup",
            "component",
            "add",
            "rust-src",
            "rust-analysis",
            &component_with_triple1,
            &component_with_triple2,
        ])
        .await;
    for file in &files {
        let path = format!("toolchains/nightly-{}/{}", this_host_triple(), file);
        assert!(cx.config.rustupdir.has(&path));
    }
    cx.config
        .expect_ok(&[
            "rustup",
            "component",
            "remove",
            "rust-src",
            "rust-analysis",
            &component_with_triple1,
            &component_with_triple2,
        ])
        .await;
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
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn env_override_path() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));

    let out = cx
        .config
        .run(
            "rustc",
            ["--version"],
            &[("RUSTUP_TOOLCHAIN", toolchain_path.to_str().unwrap())],
        )
        .await;
    assert!(out.ok);
    assert!(out.stdout.contains("hash-nightly-2"));
}

#[tokio::test]
async fn plus_override_relpath_is_not_supported() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let toolchain_path = Path::new("..")
        .join(cx.config.rustupdir.rustupdir.file_name().unwrap())
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));
    cx.config
        .expect_err(
            &[
                "rustc",
                format!("+{}", toolchain_path.to_str().unwrap()).as_str(),
                "--version",
            ],
            "error: relative path toolchain",
        )
        .await;
}

#[tokio::test]
async fn run_with_relpath_is_not_supported() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let toolchain_path = Path::new("..")
        .join(cx.config.rustupdir.rustupdir.file_name().unwrap())
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()));
    cx.config
        .expect_err(
            &[
                "rustup",
                "run",
                toolchain_path.to_str().unwrap(),
                "rustc",
                "--version",
            ],
            "relative path toolchain",
        )
        .await;
}

#[tokio::test]
async fn plus_override_abspath_is_supported() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()))
        .canonicalize()
        .unwrap();
    cx.config
        .expect_ok(&[
            "rustc",
            format!("+{}", toolchain_path.to_str().unwrap()).as_str(),
            "--version",
        ])
        .await;
}

#[tokio::test]
async fn run_with_abspath_is_supported() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let toolchain_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("nightly-{}", this_host_triple()))
        .canonicalize()
        .unwrap();
    cx.config
        .expect_ok(&[
            "rustup",
            "run",
            toolchain_path.to_str().unwrap(),
            "rustc",
            "--version",
        ])
        .await;
}

#[tokio::test]
async fn file_override_path() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

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
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;

    // Check that the toolchain has the right name
    cx.config
        .expect_stdout_ok(
            &["rustup", "show", "active-toolchain"],
            &format!("nightly-{}", this_host_triple()),
        )
        .await;
}

#[tokio::test]
async fn proxy_override_path() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

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
        .expect_stdout_ok(&["cargo", "--call-rustc"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn file_override_path_relative_not_supported() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

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
        .expect_err(&["rustc", "--version"], "relative path toolchain")
        .await;
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
        .expect_err(
            &["rustc", "--version"],
            "toolchain options are ignored for path toolchain (ephemeral)",
        )
        .await;

    raw::write_file(
        &toolchain_file,
        "[toolchain]\npath=\"ephemeral\"\ncomponents=[\"dummy\"]",
    )
    .unwrap();

    cx.config
        .expect_err(
            &["rustc", "--version"],
            "toolchain options are ignored for path toolchain (ephemeral)",
        )
        .await;

    raw::write_file(
        &toolchain_file,
        "[toolchain]\npath=\"ephemeral\"\nprofile=\"minimal\"",
    )
    .unwrap();

    cx.config
        .expect_err(
            &["rustc", "--version"],
            "toolchain options are ignored for path toolchain (ephemeral)",
        )
        .await;
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
        .expect_err(
            &["rustc", "--version"],
            "cannot specify both channel (nightly) and path (ephemeral) simultaneously",
        )
        .await;
}

#[tokio::test]
async fn file_override_subdir() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    let subdir = cwd.join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    let cx = cx.change_dir(&subdir);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn file_override_with_archive() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    }

    let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly-2015-01-01"])
        .await;

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly-2015-01-01").unwrap();

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
}

#[tokio::test]
async fn file_override_toml_format_select_installed_toolchain() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    }

    let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly-2015-01-01"])
        .await;

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;

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
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
}

#[tokio::test]
async fn file_override_toml_format_install_both_toolchain_and_components() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    }

    let mut cx = cx.with_dist_dir(Scenario::ArchivesV2_2015_01_01);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0")
        .await;
    cx.config
        .expect_not_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)")
        .await;

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
        .expect_ok(&["rustup", "toolchain", "install"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1")
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)")
        .await;
}

#[tokio::test]
async fn file_override_toml_format_add_missing_components() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_not_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)")
        .await;

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
        .expect_stderr_ok(
            &["rustup", "toolchain", "install"],
            "info: installing component 'rust-src'",
        )
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)")
        .await;
}

#[tokio::test]
async fn file_override_toml_format_add_missing_targets() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_not_stdout_ok(
            &["rustup", "component", "list"],
            "arm-linux-androideabi (installed)",
        )
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(
        &toolchain_file,
        r#"
[toolchain]
targets = [ "arm-linux-androideabi" ]
"#,
    )
    .unwrap();

    cx.config
        .expect_stderr_ok(
            &["rustup", "toolchain", "install"],
            "info: installing component 'rust-std' for 'arm-linux-androideabi'",
        )
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "component", "list"],
            "arm-linux-androideabi (installed)",
        )
        .await;
}

#[tokio::test]
async fn file_override_toml_format_skip_invalid_component() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;

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
        .expect_stderr_ok(
            &["rustup", "toolchain", "install"],
            "warn: Force-skipping unavailable component 'rust-bongo",
        )
        .await;
}

#[tokio::test]
async fn file_override_toml_format_specify_profile() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "set", "profile", "default"])
        .await;
    cx.config
        .expect_stderr_ok(
            &["rustup", "default", "stable"],
            "downloading component 'rust-docs'",
        )
        .await;

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
        .expect_ok(&["rustup", "toolchain", "install"])
        .await;
    cx.config
        .expect_not_stdout_ok(
            &["rustup", "component", "list"],
            for_host!("rust-docs-{} (installed)"),
        )
        .await;
}

#[tokio::test]
async fn default_profile_is_respected_with_rust_toolchain_file() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "set", "profile", "minimal"])
        .await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;

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
        .expect_ok(&["rustup", "toolchain", "install"])
        .await;
    cx.config
        .expect_not_stdout_ok(
            &["rustup", "component", "list"],
            for_host!("rust-docs-{} (installed)"),
        )
        .await;
}

#[tokio::test]
async fn close_file_override_beats_far_directory_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "beta"])
        .await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    cx.config
        .expect_ok(&["rustup", "override", "set", "beta"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
        .await;

    let cwd = cx.config.current_dir();

    let subdir = cwd.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    let toolchain_file = subdir.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    let cx = cx.change_dir(&subdir);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
}

// Check that toolchain overrides have the correct priority.
#[tokio::test]
async fn override_order() {
    let mut cx = CliTestContext::new(Scenario::ArchivesV2).await;
    let host = this_host_triple();
    // give each override type a different toolchain
    let default_tc = &format!("beta-2015-01-01-{host}");
    let env_tc = &format!("stable-2015-01-01-{host}");
    let dir_tc = &format!("beta-2015-01-02-{host}");
    let file_tc = &format!("stable-2015-01-02-{host}");
    let command_tc = &format!("nightly-2015-01-01-{host}");
    cx.config
        .expect_ok(&["rustup", "install", default_tc])
        .await;
    cx.config.expect_ok(&["rustup", "install", env_tc]).await;
    cx.config.expect_ok(&["rustup", "install", dir_tc]).await;
    cx.config.expect_ok(&["rustup", "install", file_tc]).await;
    cx.config
        .expect_ok(&["rustup", "install", command_tc])
        .await;

    // No default
    cx.config.expect_ok(&["rustup", "default", "none"]).await;
    cx.config
        .expect_err_ex(
            &["rustup", "show", "active-toolchain"],
            "",
            "error: no active toolchain\n",
        )
        .await;

    // Default
    cx.config
        .expect_ok(&["rustup", "default", default_tc])
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "show", "active-toolchain"], default_tc)
        .await;

    // file > default
    let toolchain_file = cx.config.current_dir().join("rust-toolchain.toml");
    raw::write_file(
        &toolchain_file,
        &format!("[toolchain]\nchannel='{file_tc}'"),
    )
    .unwrap();
    cx.config
        .expect_stdout_ok(&["rustup", "show", "active-toolchain"], file_tc)
        .await;

    // dir override > file > default
    cx.config
        .expect_ok(&["rustup", "override", "set", dir_tc])
        .await;
    cx.config
        .expect_stdout_ok(&["rustup", "show", "active-toolchain"], dir_tc)
        .await;

    // env > dir override > file > default
    let out = cx
        .config
        .run(
            "rustup",
            ["show", "active-toolchain"],
            &[("RUSTUP_TOOLCHAIN", env_tc)],
        )
        .await;
    assert!(out.ok);
    assert!(out.stdout.contains(env_tc));

    // +toolchain > env > dir override > file > default
    let out = cx
        .config
        .run(
            "rustup",
            [&format!("+{command_tc}"), "show", "active-toolchain"],
            &[("RUSTUP_TOOLCHAIN", env_tc)],
        )
        .await;
    assert!(out.ok);
    assert!(out.stdout.contains(command_tc));
}

#[tokio::test]
async fn directory_override_doesnt_need_to_exist_unless_it_is_selected() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "beta"])
        .await;
    // not installing nightly

    cx.config
        .expect_ok(&["rustup", "override", "set", "beta"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
        .await;
}

#[tokio::test]
async fn env_override_beats_file_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "beta"])
        .await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    let mut cmd = cx.config.cmd("rustc", ["--version"]);
    cx.config.env(&mut cmd);
    cmd.env("RUSTUP_TOOLCHAIN", "beta");

    let out = cmd.output().unwrap();
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("hash-beta-1.2.0")
    );
}

#[tokio::test]
async fn plus_override_beats_file_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "beta"])
        .await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect_stdout_ok(&["rustc", "+beta", "--version"], "hash-beta-1.2.0")
        .await;
}

#[tokio::test]
async fn file_override_not_installed_custom() {
    let cx = CliTestContext::new(Scenario::None).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    raw::write_file(&toolchain_file, "gumbo").unwrap();

    cx.config
        .expect_err(
            &["rustup", "show", "active-toolchain"],
            "custom toolchain 'gumbo' specified in override file",
        )
        .await;
    cx.config
        .expect_err(
            &["rustc", "--version"],
            "custom toolchain 'gumbo' specified in override file",
        )
        .await;
}

#[tokio::test]
async fn file_override_not_installed_custom_toml() {
    let cx = CliTestContext::new(Scenario::None).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(&toolchain_file, r#"toolchain.channel = "i-am-the-walrus""#).unwrap();

    cx.config
        .expect_err(
            &["rustup", "show", "active-toolchain"],
            "custom toolchain 'i-am-the-walrus' specified in override file",
        )
        .await;
    cx.config
        .expect_err(
            &["rustc", "--version"],
            "custom toolchain 'i-am-the-walrus' specified in override file",
        )
        .await;
}

#[tokio::test]
async fn bad_file_override() {
    let cx = CliTestContext::new(Scenario::None).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    // invalid name - cannot specify no toolchain in a toolchain file
    raw::write_file(&toolchain_file, "none").unwrap();

    cx.config
        .expect_err(
            &["rustc", "--version"],
            "invalid toolchain name detected in override file",
        )
        .await;
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
        .expect_err(
            &["rustup", "show", "active-toolchain"],
            "invalid toolchain name detected in override file",
        )
        .await;
    cx.config
        .expect_err(
            &["rustc", "--version"],
            "invalid toolchain name detected in override file",
        )
        .await;
}

#[tokio::test]
async fn valid_override_settings() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    raw::write_file(&toolchain_file, "nightly").unwrap();
    cx.config.expect_ok(&["rustc", "--version"]).await;
    // Special case: same version as is installed is permitted.
    raw::write_file(&toolchain_file, for_host!("nightly-{}")).unwrap();
    cx.config.expect_ok(&["rustc", "--version"]).await;
    let fullpath = cx
        .config
        .rustupdir
        .clone()
        .join("toolchains")
        .join(for_host!("nightly-{}"));
    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "system",
            &format!("{}", fullpath.display()),
        ])
        .await;
    raw::write_file(&toolchain_file, "system").unwrap();
    cx.config.expect_ok(&["rustc", "--version"]).await;
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
        .expect_err(
            &["rustc", "--version"],
            "target triple in channel name 'nightly-x86_64-unknown-linux-gnu'",
        )
        .await;
}

#[tokio::test]
async fn docs_with_path() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    let mut cmd = cx.config.cmd("rustup", ["doc", "--path"]);
    cx.config.env(&mut cmd);

    let out = cmd.output().unwrap();
    let path = format!("share{MAIN_SEPARATOR}doc{MAIN_SEPARATOR}rust{MAIN_SEPARATOR}html");
    assert!(String::from_utf8(out.stdout).unwrap().contains(&path));

    cx.config
        .expect_stdout_ok(
            &["rustup", "doc", "--path", "--toolchain", "nightly"],
            "nightly",
        )
        .await;
}

#[tokio::test]
async fn docs_topical_with_path() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"])
        .await;

    for (args, path) in topical_doc_data::test_cases() {
        let mut cmd = cx
            .config
            .cmd("rustup", ["doc", "--path"].iter().chain(args.iter()));
        cx.config.env(&mut cmd);

        let out = cmd.output().unwrap();
        eprintln!("{:?}", String::from_utf8(out.stderr).unwrap());
        let out_str = String::from_utf8(out.stdout).unwrap();
        assert!(
            out_str.contains(&path),
            "comparing path\nargs: '{args:?}'\nexpected path: '{path}'\noutput: {out_str}\n\n\n",
        );
    }
}

#[tokio::test]
async fn docs_missing() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup", "set", "profile", "minimal"])
        .await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_err(
            &["rustup", "doc"],
            "error: unable to view documentation which is not installed",
        )
        .await;
}

#[tokio::test]
async fn docs_custom() {
    let mut cx = CliTestContext::new(Scenario::None).await;
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "custom", &path])
        .await;
    cx.config.expect_ok(&["rustup", "default", "custom"]).await;
    cx.config
        .expect_stdout_ok(&["rustup", "doc", "--path"], "custom")
        .await;
}

#[cfg(unix)]
#[tokio::test]
async fn non_utf8_arg() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    let out = cx
        .config
        .run(
            "rustc",
            [
                OsStr::new("--echo-args"),
                OsStr::new("echoed non-utf8 arg:"),
                OsStr::from_bytes(b"\xc3\x28"),
            ],
            &[("RUST_BACKTRACE", "1")],
        )
        .await;
    assert!(out.stderr.contains("echoed non-utf8 arg"));
}

#[cfg(windows)]
#[tokio::test]
async fn non_utf8_arg() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    let out = cx
        .config
        .run(
            "rustc",
            [
                OsString::from("--echo-args".to_string()),
                OsString::from("echoed non-utf8 arg:".to_string()),
                OsString::from_wide(&[0xd801, 0xd801]),
            ],
            &[("RUST_BACKTRACE", "1")],
        )
        .await;
    assert!(out.stderr.contains("echoed non-utf8 arg"));
}

#[cfg(unix)]
#[tokio::test]
async fn non_utf8_toolchain() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    let out = cx
        .config
        .run(
            "rustc",
            [OsStr::from_bytes(b"+\xc3\x28")],
            &[("RUST_BACKTRACE", "1")],
        )
        .await;
    assert!(out.stderr.contains("toolchain '�(' is not installed"));
}

#[cfg(windows)]
#[tokio::test]
async fn non_utf8_toolchain() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    let out = cx
        .config
        .run(
            "rustc",
            [OsString::from_wide(&[u16::from(b'+'), 0xd801, 0xd801])],
            &[("RUST_BACKTRACE", "1")],
        )
        .await;
    assert!(out.stderr.contains("toolchain '��' is not installed"));
}

#[tokio::test]
async fn check_host_goes_away() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::HostGoesMissingBefore);
        cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    }

    let cx = cx.with_dist_dir(Scenario::HostGoesMissingAfter);
    cx.config
        .expect_err(
            &["rustup", "update", "nightly"],
            for_host!("target '{}' not found in channel"),
        )
        .await;
}

#[cfg(unix)]
#[tokio::test]
async fn check_unix_settings_fallback() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    // No default toolchain specified yet
    cx.config
        .expect_err_ex(
            &["rustup", "default"],
            "",
            "error: no default toolchain is configured\n",
        )
        .await;

    // Default toolchain specified in fallback settings file
    let mock_settings_file = cx.config.current_dir().join("mock_fallback_settings.toml");
    raw::write_file(
        &mock_settings_file,
        for_host!(r"default_toolchain = 'nightly-{0}'"),
    )
    .unwrap();

    let mut cmd = cx.config.cmd("rustup", ["default"]);
    cx.config.env(&mut cmd);

    // Override the path to the fallback settings file to be the mock file
    cmd.env("RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS", mock_settings_file);

    let out = cmd.output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        &stdout,
        for_host!(
            r"nightly-{0} (default)
"
        )
    );
}

#[tokio::test]
async fn deny_incompatible_toolchain_install() {
    let cx = CliTestContext::new(Scenario::MultiHost).await;
    let arch = MULTI_ARCH1;
    cx.config
        .expect_err(
            &["rustup", "toolchain", "install", &format!("nightly-{arch}")],
            &format!(
                "error: toolchain 'nightly-{arch}' may not be able to run on this system
note: to build software for that platform, try `rustup target add {arch}` instead",
            ),
        )
        .await;
}

#[tokio::test]
async fn deny_incompatible_toolchain_default() {
    let cx = CliTestContext::new(Scenario::MultiHost).await;
    let arch = MULTI_ARCH1;
    cx.config
        .expect_err(
            &["rustup", "default", &format!("nightly-{arch}")],
            &format!(
                "error: toolchain 'nightly-{arch}' may not be able to run on this system
note: to build software for that platform, try `rustup target add {arch}` instead",
            ),
        )
        .await;
}

#[tokio::test]
async fn dont_warn_on_partial_build() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let triple = this_host_triple();
    let arch = triple.split('-').next().unwrap();
    let mut cmd = cx.config.cmd(
        "rustup",
        ["toolchain", "install", &format!("nightly-{arch}")],
    );
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains(&format!(
        r"info: syncing channel updates for 'nightly-{triple}'"
    )));
    assert!(!stderr.contains(&format!(
        r"warn: toolchain 'nightly-{arch}' may not be able to run on this system."
    )));
}

/// Checks that `rust-toolchain.toml` files are considered
#[tokio::test]
async fn rust_toolchain_toml() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err(
            &["rustc", "--version"],
            "rustup could not choose a version of rustc to run",
        )
        .await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(&toolchain_file, "[toolchain]\nchannel = \"nightly\"").unwrap();
    cx.config
        .expect_ok(&["rustup", "toolchain", "install"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
}

/// Ensures that `rust-toolchain.toml` files (with `.toml` extension) only allow TOML contents
#[tokio::test]
async fn only_toml_in_rust_toolchain_toml() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");
    raw::write_file(&toolchain_file, "nightly").unwrap();

    cx.config
        .expect_err(&["rustc", "--version"], "error parsing override file")
        .await;
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
        .expect_stderr_ok(
            &["rustup", "toolchain", "install"],
            &format!(
                "warn: both `{0}` and `{1}` exist. Using `{0}`",
                toolchain_file_1.canonicalize().unwrap().display(),
                toolchain_file_2.canonicalize().unwrap().display(),
            ),
        )
        .await;
}

#[tokio::test]
async fn custom_toolchain_with_components_toolchains_profile_does_not_err() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain.toml");

    // install a toolchain so we can make a custom toolchain that links to it
    cx.config
        .expect_stderr_ok(
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--profile=minimal",
                "--component=cargo",
            ],
            for_host!(
                "\
info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rustc'
info: default toolchain set to 'nightly-{0}'"
            ),
        )
        .await;

    // link the toolchain
    let toolchains = cx.config.rustupdir.join("toolchains");
    raw::symlink_dir(
        &toolchains.join(for_host!("nightly-{0}")),
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
        .expect_stdout_ok(
            &["rustup", "show", "active-toolchain"],
            &format!("my-custom (overridden by '{0}')", toolchain_file.display(),),
        )
        .await;

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "1.3.0 (hash-nightly-2)")
        .await;

    cx.config
        .expect_stdout_ok(&["cargo", "--version"], "1.3.0 (hash-nightly-2)")
        .await;
}

// Issue #4251
#[tokio::test]
async fn show_custom_toolchain() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    let stable_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("stable-{}", this_host_triple()));
    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "stuff",
            &stable_path.to_string_lossy(),
        ])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "+stuff", "show"],
            &format!(
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
stable-{0} (default)
stuff (active)

active toolchain
----------------
name: stuff
active because: overridden by +toolchain on the command line
installed targets:
  {0}
",
                this_host_triple(),
                cx.config.rustupdir,
            ),
            r"",
        )
        .await;
}

#[tokio::test]
async fn show_custom_toolchain_without_components_file() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config.expect_ok(&["rustup", "default", "stable"]).await;
    let stable_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("stable-{}", this_host_triple()));
    cx.config
        .expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "stuff",
            &stable_path.to_string_lossy(),
        ])
        .await;

    let components_file = stable_path.join("lib").join("rustlib").join("components");
    fs::remove_file(&components_file).unwrap();
    cx.config
        .expect_ok_ex(
            &["rustup", "+stuff", "show"],
            &format!(
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
stable-{0} (default)
stuff (active)

active toolchain
----------------
name: stuff
active because: overridden by +toolchain on the command line
installed targets:
",
                this_host_triple(),
                cx.config.rustupdir,
            ),
            r"",
        )
        .await;
}
