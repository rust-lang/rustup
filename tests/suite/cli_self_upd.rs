//! Testing self install, uninstall and update

#![allow(deprecated)]

use std::env;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::Path;
use std::process::Command;

use remove_dir_all::remove_dir_all;

use retry::{
    delay::{Fibonacci, jitter},
    retry,
};
use rustup::test::{
    CROSS_ARCH1, CliTestContext, Scenario, SelfUpdateTestContext, calc_hash, output_release_file,
    this_host_triple,
};
#[cfg(windows)]
use rustup::test::{RegistryGuard, RegistryValueId, USER_PATH};
use rustup::utils::{self, raw};
use rustup::{DUP_TOOLS, TOOLS, for_host};
#[cfg(windows)]
use windows_registry::Value;

const TEST_VERSION: &str = "1.1.1";

/// Empty dist server, rustup installed with no toolchain
async fn setup_empty_installed() -> CliTestContext {
    let mut cx = CliTestContext::new(Scenario::Empty).await;
    cx.config
        .expect_ok(&[
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ])
        .await;
    cx
}

/// SimpleV3 dist server, rustup installed with default toolchain
async fn setup_installed() -> CliTestContext {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx
}

/// This is the primary smoke test testing the full end to end behavior of the
/// installation code path: everything that is output, the proxy installation,
/// status of the proxies.
#[tokio::test]
async fn install_bins_to_cargo_home() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    #[cfg(windows)]
    let _path_guard = RegistryGuard::new(&USER_PATH).unwrap();

    cx.config
        .expect_ok_contains(
            &["rustup-init", "-y"],
            for_host!(
                r"
  stable-{0} installed - 1.1.0 (hash-stable-1.1.0)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'stable-{0}'
"
            ),
        )
        .await;
    #[cfg(windows)]
    fn check(path: &Path) {
        assert!(path.exists());
    }
    #[cfg(not(windows))]
    fn check(path: &Path) {
        fn is_exe(path: &Path) -> bool {
            use std::os::unix::fs::MetadataExt;
            let mode = path.metadata().unwrap().mode();
            mode & 0o777 == 0o755
        }
        assert!(is_exe(path));
    }

    for tool in TOOLS.iter().chain(DUP_TOOLS.iter()) {
        let path = &cx.config.cargodir.join(format!("bin/{tool}{EXE_SUFFIX}"));
        check(path);
    }
}

/// Ensure that proxies are relative symlinks.
#[tokio::test]
async fn proxies_are_relative_symlinks() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    #[cfg(windows)]
    let _path_guard = RegistryGuard::new(&USER_PATH).unwrap();

    cx.config
        .expect_ok_contains(
            &["rustup-init", "-y"],
            for_host!(
                r"
  stable-{0} installed - 1.1.0 (hash-stable-1.1.0)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'stable-{0}'
"
            ),
        )
        .await;

    let rustup = format!("rustup{EXE_SUFFIX}");
    for tool in TOOLS.iter().chain(DUP_TOOLS.iter()) {
        let path = &cx.config.cargodir.join(format!("bin/{tool}{EXE_SUFFIX}"));
        // If it's a normal file then it means that hardlinks are being used
        // for proxies instead of symlinks.
        if std::fs::symlink_metadata(path).unwrap().is_file() {
            continue;
        }
        let is_rustup_symlink = match std::fs::read_link(path) {
            Ok(p) => p.as_os_str() == rustup.as_str(),
            _ => false,
        };
        assert!(is_rustup_symlink, "{}", path.display());
    }
}

#[tokio::test]
async fn install_twice() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    #[cfg(windows)]
    let _path_guard = RegistryGuard::new(&USER_PATH).unwrap();

    cx.config.expect_ok(&["rustup-init", "-y"]).await;
    cx.config.expect_ok(&["rustup-init", "-y"]).await;
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    assert!(rustup.exists());
}

/// Smoke test for the entire install process when dirs need to be made :
/// depending just on unit tests here could miss subtle dependencies being added
/// earlier in the code, so a black-box test is needed.
#[tokio::test]
async fn install_creates_cargo_home() {
    let mut cx = CliTestContext::new(Scenario::Empty).await;
    remove_dir_all(&cx.config.cargodir).unwrap();
    cx.config.rustupdir.remove().unwrap();
    cx.config
        .expect_ok(&[
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ])
        .await;
    assert!(cx.config.cargodir.exists());
}

/// Functional test needed here - we need to do the full dance where we start
/// with rustup.exe and end up deleting that exe itself.
#[tokio::test]
async fn uninstall_deletes_bins() {
    let mut cx = setup_empty_installed().await;
    // no-modify-path isn't needed here, as the test-dir-path isn't present
    // in the registry, so the no-change code path will be triggered.
    cx.config
        .expect_ok(&["rustup", "self", "uninstall", "-y"])
        .await;
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let rustc = cx.config.cargodir.join(format!("bin/rustc{EXE_SUFFIX}"));
    let rustdoc = cx.config.cargodir.join(format!("bin/rustdoc{EXE_SUFFIX}"));
    let cargo = cx.config.cargodir.join(format!("bin/cargo{EXE_SUFFIX}"));
    let rust_lldb = cx
        .config
        .cargodir
        .join(format!("bin/rust-lldb{EXE_SUFFIX}"));
    let rust_gdb = cx.config.cargodir.join(format!("bin/rust-gdb{EXE_SUFFIX}"));
    let rust_gdbgui = cx
        .config
        .cargodir
        .join(format!("bin/rust-gdbgui{EXE_SUFFIX}"));
    assert!(!rustup.exists());
    assert!(!rustc.exists());
    assert!(!rustdoc.exists());
    assert!(!cargo.exists());
    assert!(!rust_lldb.exists());
    assert!(!rust_gdb.exists());
    assert!(!rust_gdbgui.exists());
}

#[tokio::test]
async fn uninstall_works_if_some_bins_dont_exist() {
    let mut cx = setup_empty_installed().await;
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let rustc = cx.config.cargodir.join(format!("bin/rustc{EXE_SUFFIX}"));
    let rustdoc = cx.config.cargodir.join(format!("bin/rustdoc{EXE_SUFFIX}"));
    let cargo = cx.config.cargodir.join(format!("bin/cargo{EXE_SUFFIX}"));
    let rust_lldb = cx
        .config
        .cargodir
        .join(format!("bin/rust-lldb{EXE_SUFFIX}"));
    let rust_gdb = cx.config.cargodir.join(format!("bin/rust-gdb{EXE_SUFFIX}"));
    let rust_gdbgui = cx
        .config
        .cargodir
        .join(format!("bin/rust-gdbgui{EXE_SUFFIX}"));

    fs::remove_file(&rustc).unwrap();
    fs::remove_file(&cargo).unwrap();

    cx.config
        .expect_ok(&["rustup", "self", "uninstall", "-y"])
        .await;

    assert!(!rustup.exists());
    assert!(!rustc.exists());
    assert!(!rustdoc.exists());
    assert!(!cargo.exists());
    assert!(!rust_lldb.exists());
    assert!(!rust_gdb.exists());
    assert!(!rust_gdbgui.exists());
}

#[tokio::test]
async fn uninstall_deletes_rustup_home() {
    let mut cx = setup_empty_installed().await;
    cx.config
        .expect_ok(&["rustup", "self", "uninstall", "-y"])
        .await;
    assert!(!cx.config.rustupdir.has("."));
}

#[tokio::test]
async fn uninstall_works_if_rustup_home_doesnt_exist() {
    let mut cx = setup_empty_installed().await;
    cx.config.rustupdir.remove().unwrap();
    cx.config
        .expect_ok(&["rustup", "self", "uninstall", "-y"])
        .await;
}

#[tokio::test]
async fn uninstall_deletes_cargo_home() {
    let mut cx = setup_empty_installed().await;
    cx.config
        .expect_ok(&["rustup", "self", "uninstall", "-y"])
        .await;
    assert!(!cx.config.cargodir.exists());
}

#[tokio::test]
async fn uninstall_fails_if_not_installed() {
    let cx = setup_empty_installed().await;
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    fs::remove_file(rustup).unwrap();
    cx.config
        .expect_err(
            &["rustup", "self", "uninstall", "-y"],
            "rustup is not installed",
        )
        .await;
}

// The other tests here just run rustup from a temp directory. This
// does the uninstall by actually invoking the installed binary in
// order to test that it can successfully delete itself.
#[tokio::test]
#[cfg_attr(target_os = "macos", ignore)] // FIXME #1515
async fn uninstall_self_delete_works() {
    let cx = setup_empty_installed().await;
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let mut cmd = Command::new(rustup.clone());
    cmd.args(["self", "uninstall", "-y"]);
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();
    println!("out: {}", String::from_utf8(out.stdout).unwrap());
    println!("err: {}", String::from_utf8(out.stderr).unwrap());

    assert!(out.status.success());
    assert!(!rustup.exists());
    assert!(!cx.config.cargodir.exists());

    let rustc = cx.config.cargodir.join(format!("bin/rustc{EXE_SUFFIX}"));
    let rustdoc = cx.config.cargodir.join(format!("bin/rustdoc{EXE_SUFFIX}"));
    let cargo = cx.config.cargodir.join(format!("bin/cargo{EXE_SUFFIX}"));
    let rust_lldb = cx
        .config
        .cargodir
        .join(format!("bin/rust-lldb{EXE_SUFFIX}"));
    let rust_gdb = cx.config.cargodir.join(format!("bin/rust-gdb{EXE_SUFFIX}"));
    let rust_gdbgui = cx
        .config
        .cargodir
        .join(format!("bin/rust-gdbgui{EXE_SUFFIX}"));
    assert!(!rustc.exists());
    assert!(!rustdoc.exists());
    assert!(!cargo.exists());
    assert!(!rust_lldb.exists());
    assert!(!rust_gdb.exists());
    assert!(!rust_gdbgui.exists());
}

// On windows rustup self uninstall temporarily puts a rustup-gc-$randomnumber.exe
// file in CONFIG.CARGODIR/.. ; check that it doesn't exist.
#[tokio::test]
async fn uninstall_doesnt_leave_gc_file() {
    let mut cx = setup_empty_installed().await;
    cx.config
        .expect_ok(&["rustup", "self", "uninstall", "-y"])
        .await;
    let parent = cx.config.cargodir.parent().unwrap();

    // The gc removal happens after rustup terminates. Typically under
    // 100ms, but during the contention of test suites can be substantially
    // longer while still succeeding.

    let check = || ensure_empty(parent);
    match retry(Fibonacci::from_millis(1).map(jitter).take(23), check) {
        Ok(_) => (),
        Err(e) => panic!("{e}"),
    }
}

fn ensure_empty(dir: &Path) -> Result<(), GcErr> {
    let garbage = fs::read_dir(dir)
        .unwrap()
        .map(|d| d.unwrap().path().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    match garbage.len() {
        0 => Ok(()),
        _ => Err(GcErr(garbage)),
    }
}

#[derive(thiserror::Error, Debug)]
#[error("garbage remaining: {:?}", .0)]
struct GcErr(Vec<String>);

#[tokio::test]
async fn update_exact() {
    let version = env!("CARGO_PKG_VERSION");
    let expected_output = format!(
        "info: checking for self-update (current version: {version})
info: downloading self-update (new version: {TEST_VERSION})
"
    );

    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config
        .expect_ok_ex(
            &["rustup", "self", "update"],
            &format!("  rustup updated - {version} (from {version})\n\n",),
            &expected_output,
        )
        .await;
}

#[tokio::test]
async fn update_precise() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect_with_env(
            ["rustup", "self", "update"],
            [("RUSTUP_VERSION", TEST_VERSION)],
        )
        .await
        .extend_redactions([
            ("[TEST_VERSION]", TEST_VERSION),
            ("[VERSION]", env!("CARGO_PKG_VERSION")),
        ])
        .with_stdout(snapbox::str![[r#"
  rustup updated - [VERSION] (from [VERSION])


"#]])
        .with_stderr(snapbox::str![[r#"
info: checking for self-update (current version: [VERSION])
info: `RUSTUP_VERSION` has been set to `[TEST_VERSION]`
info: downloading self-update (new version: [TEST_VERSION])

"#]]);
}

#[cfg(windows)]
#[tokio::test]
async fn update_overwrites_programs_display_version() {
    const PLACEHOLDER_VERSION: &str = "9.999.99";
    let version = env!("CARGO_PKG_VERSION");

    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    let _guard = RegistryGuard::new(&USER_RUSTUP_VERSION).unwrap();
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;

    USER_RUSTUP_VERSION
        .set(Some(&Value::from(PLACEHOLDER_VERSION)))
        .unwrap();
    cx.config.expect_ok(&["rustup", "self", "update"]).await;
    assert_eq!(
        USER_RUSTUP_VERSION.get().unwrap().unwrap(),
        Value::from(version)
    );
}

#[cfg(windows)]
const USER_RUSTUP_VERSION: RegistryValueId = RegistryValueId {
    sub_key: r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Rustup",
    value_name: "DisplayVersion",
};

#[tokio::test]
async fn update_but_not_installed() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_err_ex(
            &["rustup", "self", "update"],
            r"",
            &format!(
                r"error: rustup is not installed at '{}'
",
                cx.config.cargodir.display()
            ),
        )
        .await;
}

#[tokio::test]
async fn update_but_delete_existing_updater_first() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    // The updater is stored in a known location
    let setup = cx
        .config
        .cargodir
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));

    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;

    // If it happens to already exist for some reason it
    // should just be deleted.
    raw::write_file(&setup, "").unwrap();
    cx.config.expect_ok(&["rustup", "self", "update"]).await;

    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    assert!(rustup.exists());
}

#[tokio::test]
async fn update_download_404() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;

    let trip = this_host_triple();
    let dist_dir = cx.path().join(format!("archive/{TEST_VERSION}/{trip}"));
    let dist_exe = dist_dir.join(format!("rustup-init{EXE_SUFFIX}"));

    fs::remove_file(dist_exe).unwrap();

    cx.config
        .expect_err(&["rustup", "self", "update"], "could not download file")
        .await;
}

#[tokio::test]
async fn update_bogus_version() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config.expect_err(
        &["rustup", "update", "1.0.0-alpha"],
        "invalid value '1.0.0-alpha' for '[TOOLCHAIN]...': invalid toolchain name: '1.0.0-alpha'",
    ).await;
}

// Check that rustup.exe has changed after the update. This
// is hard for windows because the running process needs to exit
// before the new updater can delete it.
#[tokio::test]
async fn update_updates_rustup_bin() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;

    let bin = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let before_hash = calc_hash(&bin);

    // Running the self update command on the installed binary,
    // so that the running binary must be replaced.
    let mut cmd = Command::new(&bin);
    cmd.args(["self", "update"]);
    cx.config.env(&mut cmd);
    let out = cmd.output().unwrap();

    println!("out: {}", String::from_utf8(out.stdout).unwrap());
    println!("err: {}", String::from_utf8(out.stderr).unwrap());

    assert!(out.status.success());

    let after_hash = calc_hash(&bin);

    assert_ne!(before_hash, after_hash);
}

#[tokio::test]
async fn update_bad_schema() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    output_release_file(cx.path(), "17", "1.1.1");
    cx.config
        .expect_err(&["rustup", "self", "update"], "unknown variant")
        .await;
}

#[tokio::test]
async fn update_no_change() {
    let version = env!("CARGO_PKG_VERSION");
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    output_release_file(cx.path(), "1", version);
    cx.config
        .expect_ok_ex(
            &["rustup", "self", "update"],
            &format!(
                r"  rustup unchanged - {version}

"
            ),
            &format!(
                r"info: checking for self-update (current version: {version})
"
            ),
        )
        .await;
}

#[tokio::test]
async fn rustup_self_updates_trivial() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup", "set", "auto-self-update", "enable"])
        .await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;

    let bin = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let before_hash = calc_hash(&bin);

    cx.config.expect_ok(&["rustup", "update"]).await;

    let after_hash = calc_hash(&bin);

    assert_ne!(before_hash, after_hash);
}

#[tokio::test]
async fn rustup_self_updates_with_specified_toolchain() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup", "set", "auto-self-update", "enable"])
        .await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;

    let bin = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let before_hash = calc_hash(&bin);

    cx.config.expect_ok(&["rustup", "update", "stable"]).await;

    let after_hash = calc_hash(&bin);

    assert_ne!(before_hash, after_hash);
}

#[tokio::test]
async fn rustup_no_self_update_with_specified_toolchain() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;

    let bin = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let before_hash = calc_hash(&bin);

    cx.config.expect_ok(&["rustup", "update", "stable"]).await;

    let after_hash = calc_hash(&bin);

    assert_eq!(before_hash, after_hash);
}

#[tokio::test]
async fn rustup_self_update_exact() {
    let version = env!("CARGO_PKG_VERSION");
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup", "set", "auto-self-update", "enable"])
        .await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;

    cx.config
        .expect_ok_ex(
            &["rustup", "update"],
            for_host!(
                r"
  stable-{0} unchanged - 1.1.0 (hash-stable-1.1.0)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: checking for self-update (current version: {version})
info: downloading self-update (new version: {TEST_VERSION})
info: cleaning up downloads & tmp directories
"
            ),
        )
        .await;
}

// Because self-delete on windows is hard, rustup-init doesn't
// do it. It instead leaves itself installed for cleanup by later
// invocations of rustup.
#[tokio::test]
async fn updater_leaves_itself_for_later_deletion() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "self", "update"]).await;

    let setup = cx
        .config
        .cargodir
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));
    assert!(setup.exists());
}

#[tokio::test]
async fn updater_is_deleted_after_running_rustup() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "self", "update"]).await;

    cx.config.expect_ok(&["rustup", "update", "nightly"]).await;

    let setup = cx
        .config
        .cargodir
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));
    assert!(!setup.exists());
}

#[tokio::test]
async fn updater_is_deleted_after_running_rustc() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "self", "update"]).await;

    cx.config.expect_ok(&["rustc", "--version"]).await;

    let setup = cx
        .config
        .cargodir
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));
    assert!(!setup.exists());
}

#[tokio::test]
async fn rustup_still_works_after_update() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config.expect_ok(&["rustup", "self", "update"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
    cx.config.expect_ok(&["rustup", "default", "beta"]).await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0")
        .await;
}

// The installer used to be called rustup-setup. For compatibility it
// still needs to work in that mode.
#[tokio::test]
async fn as_rustup_setup() {
    let mut cx = CliTestContext::new(Scenario::Empty).await;
    let init = cx.config.exedir.join(format!("rustup-init{EXE_SUFFIX}"));
    let setup = cx.config.exedir.join(format!("rustup-setup{EXE_SUFFIX}"));
    fs::copy(init, setup).unwrap();
    cx.config
        .expect_ok(&[
            "rustup-setup",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ])
        .await;
}

#[tokio::test]
async fn reinstall_exact() {
    let cx = setup_empty_installed().await;
    cx.config
        .expect_stderr_ok(
            &[
                "rustup-init",
                "-y",
                "--no-update-default-toolchain",
                "--no-modify-path",
            ],
            r"info: updating existing rustup installation - leaving toolchains alone",
        )
        .await;
}

#[tokio::test]
async fn reinstall_specifying_toolchain() {
    let cx = setup_installed().await;
    cx.config
        .expect_stdout_ok(
            &[
                "rustup-init",
                "-y",
                "--default-toolchain=stable",
                "--no-modify-path",
            ],
            for_host!(r"stable-{0} unchanged - 1.1.0"),
        )
        .await;
}

#[tokio::test]
async fn reinstall_specifying_component() {
    let mut cx = setup_installed().await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;
    cx.config
        .expect_stdout_ok(
            &[
                "rustup-init",
                "-y",
                "--default-toolchain=stable",
                "--no-modify-path",
            ],
            for_host!(r"stable-{0} unchanged - 1.1.0"),
        )
        .await;
}

#[tokio::test]
async fn reinstall_specifying_different_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_stderr_ok(
            &[
                "rustup-init",
                "-y",
                "--default-toolchain=nightly",
                "--no-modify-path",
            ],
            for_host!(r"info: default toolchain set to 'nightly-{0}'"),
        )
        .await;
}

#[tokio::test]
async fn install_sets_up_stable_unless_a_different_default_is_requested() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&[
            "rustup-init",
            "-y",
            "--default-toolchain",
            "nightly",
            "--no-modify-path",
        ])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
}

#[tokio::test]
async fn install_sets_up_stable_unless_there_is_already_a_default() {
    let mut cx = setup_installed().await;
    cx.config.expect_ok(&["rustup", "default", "nightly"]).await;
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "stable"])
        .await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2")
        .await;
    cx.config
        .expect_err(
            &["rustup", "run", "stable", "rustc", "--version"],
            for_host!("toolchain 'stable-{0}' is not installed"),
        )
        .await;
}

#[tokio::test]
async fn readline_no_stdin() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_err(
            &["rustup-init", "--no-modify-path"],
            "unable to read from stdin for confirmation",
        )
        .await;
}

#[tokio::test]
async fn rustup_init_works_with_weird_names() {
    // Browsers often rename bins to e.g. rustup-init(2).exe.
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let old = cx.config.exedir.join(format!("rustup-init{EXE_SUFFIX}"));
    let new = cx.config.exedir.join(format!("rustup-init(2){EXE_SUFFIX}"));
    fs::rename(old, new).unwrap();
    cx.config
        .expect_ok(&["rustup-init(2)", "-y", "--no-modify-path"])
        .await;
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    assert!(rustup.exists());
}

#[tokio::test]
async fn rls_proxy_set_up_after_install() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let mut cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
            .await;
    }

    cx.config
        .expect_err(
            &["rls", "--version"],
            &format!(
                "'rls{}' is not installed for the toolchain 'stable-{}'",
                EXE_SUFFIX,
                this_host_triple(),
            ),
        )
        .await;
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls"])
        .await;
    cx.config.expect_ok(&["rls", "--version"]).await;
}

#[tokio::test]
async fn rls_proxy_set_up_after_update() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    let rls_path = cx.config.cargodir.join(format!("bin/rls{EXE_SUFFIX}"));
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    fs::remove_file(&rls_path).unwrap();
    cx.config.expect_ok(&["rustup", "self", "update"]).await;
    assert!(rls_path.exists());
}

#[tokio::test]
async fn update_does_not_overwrite_rustfmt() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    let version = env!("CARGO_PKG_VERSION");
    output_release_file(cx.path(), "1", version);

    // Since we just did a fresh install rustfmt will exist. Let's emulate
    // it not existing in this test though by removing it just after our
    // installation.
    let rustfmt_path = cx.config.cargodir.join(format!("bin/rustfmt{EXE_SUFFIX}"));
    assert!(rustfmt_path.exists());
    fs::remove_file(&rustfmt_path).unwrap();
    raw::write_file(&rustfmt_path, "").unwrap();
    assert_eq!(utils::file_size(&rustfmt_path).unwrap(), 0);

    // Ok, now a self-update should complain about `rustfmt` not looking
    // like rustup and the user should take some action.
    cx.config
        .expect_stderr_ok(
            &["rustup", "self", "update"],
            "`rustfmt` is already installed",
        )
        .await;
    assert!(rustfmt_path.exists());
    assert_eq!(utils::file_size(&rustfmt_path).unwrap(), 0);

    // Now simulate us removing the rustfmt executable and rerunning a self
    // update, this should install the rustup shim. Note that we don't run
    // `rustup` here but rather the rustup we've actually installed, this'll
    // help reproduce bugs related to having that file being opened by the
    // current process.
    fs::remove_file(&rustfmt_path).unwrap();
    let installed_rustup = cx.config.cargodir.join("bin/rustup");
    cx.config
        .expect_ok(&[installed_rustup.to_str().unwrap(), "self", "update"])
        .await;
    assert!(rustfmt_path.exists());
    assert!(utils::file_size(&rustfmt_path).unwrap() > 0);
}

#[tokio::test]
async fn update_installs_clippy_cargo_and() {
    let mut cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect_ok(&["rustup-init", "-y", "--no-modify-path"])
        .await;
    let version = env!("CARGO_PKG_VERSION");
    output_release_file(cx.path(), "1", version);

    let cargo_clippy_path = cx
        .config
        .cargodir
        .join(format!("bin/cargo-clippy{EXE_SUFFIX}"));
    assert!(cargo_clippy_path.exists());
}

#[tokio::test]
async fn install_with_components_and_targets() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&[
            "rustup-init",
            "--default-toolchain",
            "nightly",
            "-y",
            "-c",
            "rls",
            "-t",
            CROSS_ARCH1,
            "--no-modify-path",
        ])
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "target", "list"],
            &format!("{CROSS_ARCH1} (installed)"),
        )
        .await;
    cx.config
        .expect_stdout_ok(
            &["rustup", "component", "list"],
            &format!("rls-{} (installed)", this_host_triple()),
        )
        .await;
}

#[tokio::test]
async fn install_minimal_profile() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_ok(&[
            "rustup-init",
            "-y",
            "--profile",
            "minimal",
            "--no-modify-path",
        ])
        .await;

    cx.config.expect_component_executable("rustup").await;
    cx.config.expect_component_executable("rustc").await;
    cx.config.expect_component_not_executable("cargo").await;
}
