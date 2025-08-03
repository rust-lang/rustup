//! Testing self install, uninstall and update

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
use rustup::{DUP_TOOLS, TOOLS};
#[cfg(windows)]
use windows_registry::Value;

const TEST_VERSION: &str = "1.1.1";

/// Empty dist server, rustup installed with no toolchain
async fn setup_empty_installed() -> CliTestContext {
    let cx = CliTestContext::new(Scenario::Empty).await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ])
        .await
        .is_ok();
    cx
}

/// SimpleV3 dist server, rustup installed with default toolchain
async fn setup_installed() -> CliTestContext {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
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
        .expect(["rustup-init", "-y"])
        .await
        .with_stdout(snapbox::str![[r#"
...
  stable-[HOST_TRIPLE] installed - 1.1.0 (hash-stable-1.1.0)
...
"#]])
        .with_stderr(snapbox::str![[r#"
...
info: syncing channel updates for 'stable-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component[..]
...
info: default toolchain set to 'stable-[HOST_TRIPLE]'

"#]])
        .is_ok();
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
        .expect(["rustup-init", "-y"])
        .await
        .with_stdout(snapbox::str![[r#"
...
  stable-[HOST_TRIPLE] installed - 1.1.0 (hash-stable-1.1.0)
...
"#]])
        .with_stderr(snapbox::str![[r#"
...
info: syncing channel updates for 'stable-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component[..]
...
info: default toolchain set to 'stable-[HOST_TRIPLE]'
...
"#]])
        .is_ok();

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
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    #[cfg(windows)]
    let _path_guard = RegistryGuard::new(&USER_PATH).unwrap();

    cx.config.expect(["rustup-init", "-y"]).await.is_ok();
    cx.config.expect(["rustup-init", "-y"]).await.is_ok();
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    assert!(rustup.exists());
}

/// Smoke test for the entire install process when dirs need to be made :
/// depending just on unit tests here could miss subtle dependencies being added
/// earlier in the code, so a black-box test is needed.
#[tokio::test]
async fn install_creates_cargo_home() {
    let cx = CliTestContext::new(Scenario::Empty).await;
    remove_dir_all(&cx.config.cargodir).unwrap();
    cx.config.rustupdir.remove().unwrap();
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ])
        .await
        .is_ok();
    assert!(cx.config.cargodir.exists());
}

/// Functional test needed here - we need to do the full dance where we start
/// with rustup.exe and end up deleting that exe itself.
#[tokio::test]
async fn uninstall_deletes_bins() {
    let cx = setup_empty_installed().await;
    // no-modify-path isn't needed here, as the test-dir-path isn't present
    // in the registry, so the no-change code path will be triggered.
    cx.config
        .expect(["rustup", "self", "uninstall", "-y"])
        .await
        .is_ok();
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
    let cx = setup_empty_installed().await;
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
        .expect(["rustup", "self", "uninstall", "-y"])
        .await
        .is_ok();

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
    let cx = setup_empty_installed().await;
    cx.config
        .expect(["rustup", "self", "uninstall", "-y"])
        .await
        .is_ok();
    assert!(!cx.config.rustupdir.has("."));
}

#[tokio::test]
async fn uninstall_works_if_rustup_home_doesnt_exist() {
    let cx = setup_empty_installed().await;
    cx.config.rustupdir.remove().unwrap();
    cx.config
        .expect(["rustup", "self", "uninstall", "-y"])
        .await
        .is_ok();
}

#[tokio::test]
async fn uninstall_deletes_cargo_home() {
    let cx = setup_empty_installed().await;
    cx.config
        .expect(["rustup", "self", "uninstall", "-y"])
        .await
        .is_ok();
    assert!(!cx.config.cargodir.exists());
}

#[tokio::test]
async fn uninstall_fails_if_not_installed() {
    let cx = setup_empty_installed().await;
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    fs::remove_file(rustup).unwrap();
    cx.config
        .expect(["rustup", "self", "uninstall", "-y"])
        .await
        .with_stderr(snapbox::str![[r#"
error: rustup is not installed at '[..]'

"#]])
        .is_err();
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
    let cx = setup_empty_installed().await;
    cx.config
        .expect(["rustup", "self", "uninstall", "-y"])
        .await
        .is_ok();
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
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "self", "update"])
        .await
        .extend_redactions([("[TEST_VERSION]", TEST_VERSION)])
        .with_stdout(snapbox::str![[r#"
  rustup updated - [CURRENT_VERSION] (from [CURRENT_VERSION])


"#]])
        .with_stderr(snapbox::str![[r#"
info: checking for self-update (current version: [CURRENT_VERSION])
info: downloading self-update (new version: [TEST_VERSION])

"#]])
        .is_ok();
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
        .extend_redactions([("[TEST_VERSION]", TEST_VERSION)])
        .with_stdout(snapbox::str![[r#"
  rustup updated - [CURRENT_VERSION] (from [CURRENT_VERSION])


"#]])
        .with_stderr(snapbox::str![[r#"
info: checking for self-update (current version: [CURRENT_VERSION])
info: `RUSTUP_VERSION` has been set to `[TEST_VERSION]`
info: downloading self-update (new version: [TEST_VERSION])

"#]]);
}

#[cfg(windows)]
#[tokio::test]
async fn update_overwrites_programs_display_version() {
    const PLACEHOLDER_VERSION: &str = "9.999.99";
    let version = env!("CARGO_PKG_VERSION");

    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    let _guard = RegistryGuard::new(&USER_RUSTUP_VERSION).unwrap();
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();

    USER_RUSTUP_VERSION
        .set(Some(&Value::from(PLACEHOLDER_VERSION)))
        .unwrap();
    cx.config.expect(["rustup", "self", "update"]).await.is_ok();
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
        .expect(["rustup", "self", "update"])
        .await
        .extend_redactions([("[CARGO_DIR]", cx.config.cargodir)])
        .is_err()
        .with_stdout(snapbox::str![[""]])
        .with_stderr(snapbox::str![[r#"
error: rustup is not installed at '[CARGO_DIR]'

"#]]);
}

#[tokio::test]
async fn update_but_delete_existing_updater_first() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    // The updater is stored in a known location
    let setup = cx
        .config
        .cargodir
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));

    cx.config
        .expect(&["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();

    // If it happens to already exist for some reason it
    // should just be deleted.
    raw::write_file(&setup, "").unwrap();
    cx.config
        .expect(&["rustup", "self", "update"])
        .await
        .is_ok();

    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    assert!(rustup.exists());
}

#[tokio::test]
async fn update_download_404() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();

    let trip = this_host_triple();
    let dist_dir = cx.path().join(format!("archive/{TEST_VERSION}/{trip}"));
    let dist_exe = dist_dir.join(format!("rustup-init{EXE_SUFFIX}"));

    fs::remove_file(dist_exe).unwrap();

    cx.config
        .expect(["rustup", "self", "update"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: could not download file from '[..]' to '[..]': file not found
...
"#]])
        .is_err();
}

#[tokio::test]
async fn update_bogus_version() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "1.0.0-alpha"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: invalid value '1.0.0-alpha' for '[TOOLCHAIN]...': invalid toolchain name: '1.0.0-alpha'
...
"#]])
        .is_err();
}

// Check that rustup.exe has changed after the update. This
// is hard for windows because the running process needs to exit
// before the new updater can delete it.
#[tokio::test]
async fn update_updates_rustup_bin() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(&["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();

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
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    output_release_file(cx.path(), "17", "1.1.1");
    cx.config
        .expect(["rustup", "self", "update"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: unable to parse rustup release file[..]
...
unknown variant [..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn update_no_change() {
    let version = env!("CARGO_PKG_VERSION");
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    output_release_file(cx.path(), "1", version);
    cx.config
        .expect(["rustup", "self", "update"])
        .await
        .with_stdout(snapbox::str![[r#"
  rustup unchanged - [CURRENT_VERSION]


"#]])
        .with_stderr(snapbox::str![[r#"
info: checking for self-update (current version: [CURRENT_VERSION])

"#]])
        .is_ok();
}

#[tokio::test]
async fn rustup_self_updates_trivial() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup", "set", "auto-self-update", "enable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();

    let bin = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let before_hash = calc_hash(&bin);

    cx.config.expect(["rustup", "update"]).await.is_ok();

    let after_hash = calc_hash(&bin);

    assert_ne!(before_hash, after_hash);
}

#[tokio::test]
async fn rustup_self_updates_with_specified_toolchain() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup", "set", "auto-self-update", "enable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();

    let bin = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let before_hash = calc_hash(&bin);

    cx.config
        .expect(["rustup", "update", "stable"])
        .await
        .is_ok();

    let after_hash = calc_hash(&bin);

    assert_ne!(before_hash, after_hash);
}

#[tokio::test]
async fn rustup_no_self_update_with_specified_toolchain() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();

    let bin = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    let before_hash = calc_hash(&bin);

    cx.config
        .expect(["rustup", "update", "stable"])
        .await
        .is_ok();

    let after_hash = calc_hash(&bin);

    assert_eq!(before_hash, after_hash);
}

#[tokio::test]
async fn rustup_self_update_exact() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup", "set", "auto-self-update", "enable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();

    cx.config
        .expect(["rustup", "update"])
        .await
        .extend_redactions([("[TEST_VERSION]", TEST_VERSION)])
        .with_stdout(snapbox::str![[r#"

  stable-[HOST_TRIPLE] unchanged - 1.1.0 (hash-stable-1.1.0)


"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'stable-[HOST_TRIPLE]'
info: checking for self-update (current version: [CURRENT_VERSION])
info: downloading self-update (new version: [TEST_VERSION])
info: cleaning up downloads & tmp directories

"#]])
        .is_ok();
}

// Because self-delete on windows is hard, rustup-init doesn't
// do it. It instead leaves itself installed for cleanup by later
// invocations of rustup.
#[tokio::test]
async fn updater_leaves_itself_for_later_deletion() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config.expect(["rustup", "self", "update"]).await.is_ok();

    let setup = cx
        .config
        .cargodir
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));
    assert!(setup.exists());
}

#[tokio::test]
async fn updater_is_deleted_after_running_rustup() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config.expect(["rustup", "self", "update"]).await.is_ok();

    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();

    let setup = cx
        .config
        .cargodir
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));
    assert!(!setup.exists());
}

#[tokio::test]
async fn updater_is_deleted_after_running_rustc() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config.expect(["rustup", "self", "update"]).await.is_ok();

    cx.config.expect(["rustc", "--version"]).await.is_ok();

    let setup = cx
        .config
        .cargodir
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));
    assert!(!setup.exists());
}

#[tokio::test]
async fn rustup_still_works_after_update() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config.expect(["rustup", "self", "update"]).await.is_ok();
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
}

// The installer used to be called rustup-setup. For compatibility it
// still needs to work in that mode.
#[tokio::test]
async fn as_rustup_setup() {
    let cx = CliTestContext::new(Scenario::Empty).await;
    let init = cx.config.exedir.join(format!("rustup-init{EXE_SUFFIX}"));
    let setup = cx.config.exedir.join(format!("rustup-setup{EXE_SUFFIX}"));
    fs::copy(init, setup).unwrap();
    cx.config
        .expect([
            "rustup-setup",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ])
        .await
        .is_ok();
}

#[tokio::test]
async fn reinstall_exact() {
    let cx = setup_empty_installed().await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--no-update-default-toolchain",
            "--no-modify-path",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: updating existing rustup installation - leaving toolchains alone
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn reinstall_specifying_toolchain() {
    let cx = setup_installed().await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--default-toolchain=stable",
            "--no-modify-path",
        ])
        .await
        .with_stdout(snapbox::str![[r#"
...
  stable-[HOST_TRIPLE] unchanged - 1.1.0 (hash-stable-1.1.0)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn reinstall_specifying_component() {
    let cx = setup_installed().await;
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--default-toolchain=stable",
            "--no-modify-path",
        ])
        .await
        .with_stdout(snapbox::str![[r#"
...
  stable-[HOST_TRIPLE] unchanged - 1.1.0 (hash-stable-1.1.0)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn reinstall_specifying_different_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--default-toolchain=nightly",
            "--no-modify-path",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: default toolchain set to 'nightly-[HOST_TRIPLE]'
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn install_sets_up_stable_unless_a_different_default_is_requested() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--default-toolchain",
            "nightly",
            "--no-modify-path",
        ])
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
async fn install_sets_up_stable_unless_there_is_already_a_default() {
    let cx = setup_installed().await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
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
        .expect(["rustup", "run", "stable", "rustc", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'stable-[HOST_TRIPLE]' is not installed
help: run `rustup toolchain install stable-[HOST_TRIPLE]` to install it

"#]])
        .is_err();
}

#[tokio::test]
async fn readline_no_stdin() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup-init", "--no-modify-path"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: unable to read from stdin for confirmation[..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn rustup_init_works_with_weird_names() {
    // Browsers often rename bins to e.g. rustup-init(2).exe.
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let old = cx.config.exedir.join(format!("rustup-init{EXE_SUFFIX}"));
    let new = cx.config.exedir.join(format!("rustup-init(2){EXE_SUFFIX}"));
    fs::rename(old, new).unwrap();
    cx.config
        .expect(&["rustup-init(2)", "-y", "--no-modify-path"])
        .await
        .is_ok();
    let rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
    assert!(rustup.exists());
}

#[tokio::test]
async fn rls_proxy_set_up_after_install() {
    let mut cx = CliTestContext::new(Scenario::None).await;

    {
        let cx = cx.with_dist_dir(Scenario::SimpleV2);
        cx.config
            .expect(["rustup-init", "-y", "--no-modify-path"])
            .await
            .is_ok();
    }

    cx.config
        .expect(["rls", "--version"])
        .await
        .with_stderr(snapbox::str![[r#"
error: 'rls[EXE]' is not installed for the toolchain 'stable-[HOST_TRIPLE]'.
To install, run `rustup component add rls`

"#]])
        .is_err();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config.expect(["rls", "--version"]).await.is_ok();
}

#[tokio::test]
async fn rls_proxy_set_up_after_update() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    let rls_path = cx.config.cargodir.join(format!("bin/rls{EXE_SUFFIX}"));
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    fs::remove_file(&rls_path).unwrap();
    cx.config.expect(["rustup", "self", "update"]).await.is_ok();
    assert!(rls_path.exists());
}

#[tokio::test]
async fn update_does_not_overwrite_rustfmt() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
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
        .expect(["rustup", "self", "update"])
        .await
        .with_stderr(snapbox::str![[r#"
info: checking for self-update (current version: [CURRENT_VERSION])
warn: tool `rustfmt` is already installed, remove it from `[..]`, then run `rustup update` to have rustup manage this tool.

"#]])
        .is_ok();
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
        .expect([installed_rustup.to_str().unwrap(), "self", "update"])
        .await
        .is_ok();
    assert!(rustfmt_path.exists());
    assert!(utils::file_size(&rustfmt_path).unwrap() > 0);
}

#[tokio::test]
async fn update_installs_clippy_cargo_and() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
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
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect([
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
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I] (installed)
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rls-[HOST_TRIPLE] (installed)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn install_minimal_profile() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--profile",
            "minimal",
            "--no-modify-path",
        ])
        .await
        .is_ok();

    cx.config.expect_component_executable("rustup").await;
    cx.config.expect_component_executable("rustc").await;
    cx.config.expect_component_not_executable("cargo").await;
}
