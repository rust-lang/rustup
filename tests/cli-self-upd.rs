//! Testing self install, uninstall and update

// Disable these tests for MSI-based installation.
// The `self update` and `self uninstall` commands just call `msiexec`.
#![cfg(not(feature = "msi-installed"))]

extern crate rustup_mock;
extern crate rustup_utils;
#[macro_use]
extern crate lazy_static;
extern crate tempdir;
extern crate scopeguard;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;

use tempdir::TempDir;
use std::sync::Mutex;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::path::Path;
use std::fs;
use std::process::Command;
use rustup_mock::clitools::{self, Config, Scenario,
                               expect_ok, expect_ok_ex,
                               expect_stdout_ok,
                               expect_stderr_ok,
                               expect_err, expect_err_ex,
                               this_host_triple};
use rustup_mock::dist::{calc_hash};
use rustup_mock::{get_path, restore_path};
use rustup_utils::{utils, raw};

macro_rules! for_host { ($s: expr) => (&format!($s, this_host_triple())) }

const TEST_VERSION: &'static str = "1.1.1";

pub fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, &|config| {
        // Lock protects environment variables
        lazy_static! {
            static ref LOCK: Mutex<()> = Mutex::new(());
        }
        let _g = LOCK.lock();

        // An windows these tests mess with the user's PATH. Save
        // and restore them here to keep from trashing things.
        let saved_path = get_path();
        let _g = scopeguard::guard(saved_path, |p| restore_path(p));

        f(config);
    });
}

pub fn update_setup(f: &Fn(&Config, &Path)) {
    setup(&|config| {

        // Create a mock self-update server
        let ref self_dist_tmp = TempDir::new("self_dist").unwrap();
        let ref self_dist = self_dist_tmp.path();

        let ref trip = this_host_triple();
        let ref dist_dir = self_dist.join(&format!("archive/{}/{}", TEST_VERSION, trip));
        let ref dist_exe = dist_dir.join(&format!("rustup-init{}", EXE_SUFFIX));
        let ref rustup_bin = config.exedir.join(&format!("rustup-init{}", EXE_SUFFIX));

        fs::create_dir_all(dist_dir).unwrap();
        output_release_file(self_dist, "1", TEST_VERSION);
        fs::copy(rustup_bin, dist_exe).unwrap();
        // Modify the exe so it hashes different
        raw::append_file(dist_exe, "").unwrap();

        let ref root_url = format!("file://{}", self_dist.display());
        env::set_var("RUSTUP_UPDATE_ROOT", root_url);

        f(config, self_dist);
    });
}

fn output_release_file(dist_dir: &Path, schema: &str, version: &str) {
    let contents = format!(r#"
schema-version = "{}"
version = "{}"
"#, schema, version);
    let file = dist_dir.join("release-stable.toml");
    utils::write_file("release", &file, &contents).unwrap();
}

#[test]
fn install_bins_to_cargo_home() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let rustc = config.cargodir.join(&format!("bin/rustc{}", EXE_SUFFIX));
        let rustdoc = config.cargodir.join(&format!("bin/rustdoc{}", EXE_SUFFIX));
        let cargo = config.cargodir.join(&format!("bin/cargo{}", EXE_SUFFIX));
        let rust_lldb = config.cargodir.join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
        let rust_gdb = config.cargodir.join(&format!("bin/rust-gdb{}", EXE_SUFFIX));
        assert!(rustup.exists());
        assert!(rustc.exists());
        assert!(rustdoc.exists());
        assert!(cargo.exists());
        assert!(rust_lldb.exists());
        assert!(rust_gdb.exists());
    });
}

#[test]
fn install_twice() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup-init", "-y"]);
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        assert!(rustup.exists());
    });
}

#[test]
#[cfg(unix)]
fn bins_are_executable() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        let ref rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let ref rustc = config.cargodir.join(&format!("bin/rustc{}", EXE_SUFFIX));
        let ref rustdoc = config.cargodir.join(&format!("bin/rustdoc{}", EXE_SUFFIX));
        let ref cargo = config.cargodir.join(&format!("bin/cargo{}", EXE_SUFFIX));
        let ref rust_lldb = config.cargodir.join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
        let ref rust_gdb = config.cargodir.join(&format!("bin/rust-gdb{}", EXE_SUFFIX));
        assert!(is_exe(rustup));
        assert!(is_exe(rustc));
        assert!(is_exe(rustdoc));
        assert!(is_exe(cargo));
        assert!(is_exe(rust_lldb));
        assert!(is_exe(rust_gdb));
    });

    fn is_exe(path: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        let mode = path.metadata().unwrap().mode();

        mode & 0o777 == 0o755
    }
}

#[test]
fn install_creates_cargo_home() {
    setup(&|config| {
        fs::remove_dir_all(&config.cargodir).unwrap();
        fs::remove_dir_all(&config.rustupdir).unwrap();
        expect_ok(config, &["rustup-init", "-y"]);
        assert!(config.cargodir.exists());
    });
}

#[test]
fn uninstall_deletes_bins() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let rustc = config.cargodir.join(&format!("bin/rustc{}", EXE_SUFFIX));
        let rustdoc = config.cargodir.join(&format!("bin/rustdoc{}", EXE_SUFFIX));
        let cargo = config.cargodir.join(&format!("bin/cargo{}", EXE_SUFFIX));
        let rust_lldb = config.cargodir.join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
        let rust_gdb = config.cargodir.join(&format!("bin/rust-gdb{}", EXE_SUFFIX));
        assert!(!rustup.exists());
        assert!(!rustc.exists());
        assert!(!rustdoc.exists());
        assert!(!cargo.exists());
        assert!(!rust_lldb.exists());
        assert!(!rust_gdb.exists());
    });
}

#[test]
fn uninstall_works_if_some_bins_dont_exist() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let rustc = config.cargodir.join(&format!("bin/rustc{}", EXE_SUFFIX));
        let rustdoc = config.cargodir.join(&format!("bin/rustdoc{}", EXE_SUFFIX));
        let cargo = config.cargodir.join(&format!("bin/cargo{}", EXE_SUFFIX));
        let rust_lldb = config.cargodir.join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
        let rust_gdb = config.cargodir.join(&format!("bin/rust-gdb{}", EXE_SUFFIX));

        fs::remove_file(&rustc).unwrap();
        fs::remove_file(&cargo).unwrap();

        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        assert!(!rustup.exists());
        assert!(!rustc.exists());
        assert!(!rustdoc.exists());
        assert!(!cargo.exists());
        assert!(!rust_lldb.exists());
        assert!(!rust_gdb.exists());
    });
}

#[test]
fn uninstall_deletes_rustup_home() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);
        assert!(!config.rustupdir.exists());
    });
}

#[test]
fn uninstall_works_if_rustup_home_doesnt_exist() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        raw::remove_dir(&config.rustupdir).unwrap();
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);
    });
}

#[test]
fn uninstall_deletes_cargo_home() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);
        assert!(!config.cargodir.exists());
    });
}

#[test]
fn uninstall_fails_if_not_installed() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        fs::remove_file(&rustup).unwrap();
        expect_err(config, &["rustup", "self", "uninstall", "-y"],
                   "rustup is not installed");
    });
}

// The other tests here just run rustup from a temp directory. This
// does the uninstall by actually invoking the installed binary in
// order to test that it can successfully delete itself.
#[test]
fn uninstall_self_delete_works() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let mut cmd = Command::new(rustup.clone());
        cmd.args(&["self", "uninstall", "-y"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        println!("out: {}", String::from_utf8(out.stdout).unwrap());
        println!("err: {}", String::from_utf8(out.stderr).unwrap());

        assert!(out.status.success());
        assert!(!rustup.exists());
        assert!(!config.cargodir.exists());

        let rustc = config.cargodir.join(&format!("bin/rustc{}", EXE_SUFFIX));
        let rustdoc = config.cargodir.join(&format!("bin/rustdoc{}", EXE_SUFFIX));
        let cargo = config.cargodir.join(&format!("bin/cargo{}", EXE_SUFFIX));
        let rust_lldb = config.cargodir.join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
        let rust_gdb = config.cargodir.join(&format!("bin/rust-gdb{}", EXE_SUFFIX));
        assert!(!rustc.exists());
        assert!(!rustdoc.exists());
        assert!(!cargo.exists());
        assert!(!rust_lldb.exists());
        assert!(!rust_gdb.exists());
    });
}

// On windows rustup self uninstall temporarily puts a rustup-gc-$randomnumber.exe
// file in CONFIG.CARGODIR/.. ; check that it doesn't exist.
#[test]
fn uninstall_doesnt_leave_gc_file() {
    use std::thread;
    use std::time::Duration;

    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        // The gc removal happens after rustup terminates. Give it a moment.
        thread::sleep(Duration::from_millis(100));

        let ref parent = config.cargodir.parent().unwrap();
        // Actually, there just shouldn't be any files here
        for dirent in fs::read_dir(parent).unwrap() {
            let dirent = dirent.unwrap();
            println!("{}", dirent.path().display());
            panic!();
        }
    })
}

#[test]
#[ignore]
fn uninstall_stress_test() {
}

#[cfg(unix)]
fn install_adds_path_to_rc(rcfile: &str) {
    setup(&|config| {
        let my_rc = "foo\nbar\nbaz";
        let ref rc = config.homedir.join(rcfile);
        raw::write_file(rc, my_rc).unwrap();
        expect_ok(config, &["rustup-init", "-y"]);

        let new_rc = raw::read_file(rc).unwrap();
        let addition = format!(r#"export PATH="{}/bin:$PATH""#,
                               config.cargodir.display());
        let expected = format!("{}\n{}\n", my_rc, addition);
        assert_eq!(new_rc, expected);
    });
}

#[test]
#[cfg(unix)]
fn install_adds_path_to_profile() {
    install_adds_path_to_rc(".profile");
}

#[test]
#[cfg(unix)]
fn install_adds_path_to_rcfile_just_once() {
    setup(&|config| {
        let my_profile = "foo\nbar\nbaz";
        let ref profile = config.homedir.join(".profile");
        raw::write_file(profile, my_profile).unwrap();
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup-init", "-y"]);

        let new_profile = raw::read_file(profile).unwrap();
        let addition = format!(r#"export PATH="{}/bin:$PATH""#,
                               config.cargodir.display());
        let expected = format!("{}\n{}\n", my_profile, addition);
        assert_eq!(new_profile, expected);
    });
}

#[cfg(unix)]
fn uninstall_removes_path_from_rc(rcfile: &str) {
    setup(&|config| {
        let my_rc = "foo\nbar\nbaz";
        let ref rc = config.homedir.join(rcfile);
        raw::write_file(rc, my_rc).unwrap();
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let new_rc = raw::read_file(rc).unwrap();
        assert_eq!(new_rc, my_rc);
    });
}

#[test]
#[cfg(unix)]
fn uninstall_removes_path_from_bashrc() {
    uninstall_removes_path_from_rc(".profile");
}

#[test]
#[cfg(unix)]
fn uninstall_doesnt_touch_rc_files_that_dont_contain_cargo_home() {
    setup(&|config| {
        let my_rc = "foo\nbar\nbaz";
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let ref profile = config.homedir.join(".profile");
        raw::write_file(profile, my_rc).unwrap();

        let profile = raw::read_file(profile).unwrap();

        assert_eq!(profile, my_rc);
    });
}

// In the default case we want to write $HOME/.cargo/bin as the path,
// not the full path.
#[test]
#[cfg(unix)]
fn when_cargo_home_is_the_default_write_path_specially() {
    setup(&|config| {
        // Override the test harness so that cargo home looks like
        // $HOME/.cargo by removing CARGO_HOME from the environment,
        // otherwise the literal path will be written to the file.

        let my_profile = "foo\nbar\nbaz";
        let ref profile = config.homedir.join(".profile");
        raw::write_file(profile, my_profile).unwrap();
        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());

        let new_profile = raw::read_file(profile).unwrap();
        let addition = format!(r#"export PATH="$HOME/.cargo/bin:$PATH""#);
        let expected = format!("{}\n{}\n", my_profile, addition);
        assert_eq!(new_profile, expected);

        let mut cmd = clitools::cmd(config, "rustup", &["self", "uninstall", "-y"]);
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());

        let new_profile = raw::read_file(profile).unwrap();
        assert_eq!(new_profile, my_profile);
    });
}

#[test]
#[cfg(windows)]
fn install_adds_path() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);

        let path = config.cargodir.join("bin").to_string_lossy().to_string();
        assert!(get_path().unwrap().contains(&path));
    });
}

#[test]
#[cfg(windows)]
fn install_does_not_add_path_twice() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup-init", "-y"]);

        let path = config.cargodir.join("bin").to_string_lossy().to_string();
        assert_eq!(get_path().unwrap().matches(&path).count(), 1);
    });
}

#[test]
#[cfg(windows)]
fn uninstall_removes_path() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let path = config.cargodir.join("bin").to_string_lossy().to_string();
        assert!(!get_path().unwrap().contains(&path));
    });
}


#[test]
#[cfg(unix)]
fn install_doesnt_modify_path_if_passed_no_modify_path() {
    setup(&|config| {
        let ref profile = config.homedir.join(".profile");
        expect_ok(config, &["rustup-init", "-y", "--no-modify-path"]);
        assert!(!profile.exists());
    });
}

#[test]
#[cfg(windows)]
fn install_doesnt_modify_path_if_passed_no_modify_path() {
    use winreg::RegKey;
    use winapi::*;

    setup(&|config| {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let old_path = environment.get_raw_value("PATH").unwrap();

        expect_ok(config, &["rustup-init", "-y", "--no-modify-path"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let new_path = environment.get_raw_value("PATH").unwrap();

        assert!(old_path == new_path);
    });
}

#[test]
fn update_exact() {
    let version = env!("CARGO_PKG_VERSION");
    let expected_output = &(
r"info: checking for self-updates
info: downloading self-update
info: rustup updated successfully to ".to_owned() + version + "
");

    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok_ex(config, &["rustup", "self", "update"],
                     r"", expected_output)
    });
}

#[test]
fn update_but_not_installed() {
    update_setup(&|config, _| {
        expect_err_ex(config, &["rustup", "self", "update"],
r"",
&format!(
r"error: rustup is not installed at '{}'
", config.cargodir.display()));
    });
}

#[test]
fn update_but_delete_existing_updater_first() {
    update_setup(&|config, _| {
        // The updater is stored in a known location
        let ref setup = config.cargodir.join(&format!("bin/rustup-init{}", EXE_SUFFIX));

        expect_ok(config, &["rustup-init", "-y"]);

        // If it happens to already exist for some reason it
        // should just be deleted.
        raw::write_file(setup, "").unwrap();
        expect_ok(config, &["rustup", "self", "update"]);

        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        assert!(rustup.exists());
    });
}

#[test]
fn update_download_404() {
    update_setup(&|config, self_dist| {
        expect_ok(config, &["rustup-init", "-y"]);

        let ref trip = this_host_triple();
        let ref dist_dir = self_dist.join(&format!("archive/{}/{}", TEST_VERSION, trip));
        let ref dist_exe = dist_dir.join(&format!("rustup-init{}", EXE_SUFFIX));

        fs::remove_file(dist_exe).unwrap();

        expect_err(config, &["rustup", "self", "update"],
                   "could not download file");
    });
}

#[test]
fn update_bogus_version() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_err(config, &["rustup", "update", "1.0.0-alpha"],
            "could not download nonexistent rust version `1.0.0-alpha`");
    });
}

// Check that rustup.exe has changed after the update. This
// is hard for windows because the running process needs to exit
// before the new updater can delete it.
#[test]
fn update_updates_rustup_bin() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);

        let ref bin = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let before_hash = calc_hash(bin);

        // Running the self update command on the installed binary,
        // so that the running binary must be replaced.
        let mut cmd = Command::new(bin.clone());
        cmd.args(&["self", "update"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();

        println!("out: {}", String::from_utf8(out.stdout).unwrap());
        println!("err: {}", String::from_utf8(out.stderr).unwrap());

        assert!(out.status.success());

        let after_hash = calc_hash(bin);

        assert!(before_hash != after_hash);
    });
}

#[test]
fn update_bad_schema() {
    update_setup(&|config, self_dist| {
        expect_ok(config, &["rustup-init", "-y"]);
        output_release_file(self_dist, "17", "1.1.1");
        expect_err(config, &["rustup", "self", "update"],
                     "unknown schema version");
    });
}

#[test]
fn update_no_change() {
    let version = env!("CARGO_PKG_VERSION");
    update_setup(&|config, self_dist| {
        expect_ok(config, &["rustup-init", "-y"]);
        output_release_file(self_dist, "1", version);
        expect_ok_ex(config, &["rustup", "self", "update"],
r"",
r"info: checking for self-updates
");
    });
}

#[test]
fn rustup_self_updates() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);

        let ref bin = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let before_hash = calc_hash(bin);

        expect_ok(config, &["rustup", "update"]);

        let after_hash = calc_hash(bin);

        assert!(before_hash != after_hash);
    })
}

#[test]
fn rustup_self_update_exact() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);

        expect_ok_ex(config, &["rustup", "update"],
for_host!(r"
  stable-{0} unchanged - 1.1.0 (hash-s-2)

"),
for_host!(r"info: syncing channel updates for 'stable-{0}'
info: checking for self-updates
info: downloading self-update
"));
    })
}

// Because self-delete on windows is hard, rustup-init doesn't
// do it. It instead leaves itself installed for cleanup by later
// invocations of rustup.
#[test]
fn updater_leaves_itself_for_later_deletion() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_ok(config, &["rustup", "self", "update"]);

        let setup = config.cargodir.join(&format!("bin/rustup-init{}", EXE_SUFFIX));
        assert!(setup.exists());
    });
}

#[test]
fn updater_is_deleted_after_running_rustup() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_ok(config, &["rustup", "self", "update"]);

        expect_ok(config, &["rustup", "update", "nightly"]);

        let setup = config.cargodir.join(&format!("bin/rustup-init{}", EXE_SUFFIX));
        assert!(!setup.exists());
    });
}

#[test]
fn updater_is_deleted_after_running_rustc() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "self", "update"]);

        expect_ok(config, &["rustc", "--version"]);

        let setup = config.cargodir.join(&format!("bin/rustup-init{}", EXE_SUFFIX));
        assert!(!setup.exists());
    });
}

#[test]
fn rustup_still_works_after_update() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "self", "update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        expect_ok(config, &["rustup", "default", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
    });
}

// There's a race condition between the updater replacing
// the rustup binary and tool hardlinks and subsequent
// invocations of rustup and rustc (on windows).
#[test]
#[ignore]
fn update_stress_test() {
}

// The installer used to be called rustup-setup. For compatibility it
// still needs to work in that mode.
#[test]
#[cfg(not(windows))]
fn as_rustup_setup() {
    update_setup(&|config, _| {
        let init = config.exedir.join(format!("rustup-init{}", EXE_SUFFIX));
        let setup = config.exedir.join(format!("rustup-setup{}", EXE_SUFFIX));
        fs::copy(&init, &setup).unwrap();
        expect_ok(config, &["rustup-setup", "-y"]);
    });
}

#[test]
fn first_install_exact() {
    setup(&|config| {
        expect_ok_ex(config, &["rustup-init", "-y"],
r"
  stable installed - 1.1.0 (hash-s-2)

",
for_host!(r"info: syncing channel updates for 'stable-{0}'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: default toolchain set to 'stable'
")
                  );
    });
}

#[test]
fn reinstall_exact() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok_ex(config, &["rustup-init", "-y"],
r"
",
r"info: updating existing rustup installation
"
                  );
    });
}

#[test]
#[cfg(unix)]
fn produces_env_file_on_unix() {
    setup(&|config| {
        // Override the test harness so that cargo home looks like
        // $HOME/.cargo by removing CARGO_HOME from the environment,
        // otherwise the literal path will be written to the file.

        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());
        let ref envfile = config.homedir.join(".cargo/env");
        let envfile = raw::read_file(envfile).unwrap();
        assert!(envfile.contains(r#"export PATH="$HOME/.cargo/bin:$PATH""#));
    });
}

#[test]
#[cfg(windows)]
fn doesnt_produce_env_file_on_windows() {
}

#[test]
fn install_sets_up_stable() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-2");
    });
}

#[test]
fn install_sets_up_stable_unless_a_different_default_is_requested() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y", "--default-toolchain", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn install_sets_up_stable_unless_there_is_already_a_default() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "stable"]);
        expect_ok(config, &["rustup-init", "-y"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
        expect_err(config, &["rustup", "run", "stable", "rustc", "--version"],
                   for_host!("toolchain 'stable-{0}' is not installed"));
    });
}

// Installation used to be to ~/.multirust/bin instead of
// ~/.cargo/bin. If those bins exist during installation they
// should be deleted to avoid confusion.
#[test]
#[cfg(unix)]
fn install_deletes_legacy_multirust_bins() {
    setup(&|config| {
        let ref multirust_bin_dir = config.homedir.join(".multirust/bin");
        fs::create_dir_all(multirust_bin_dir).unwrap();
        let ref multirust_bin = multirust_bin_dir.join("multirust");
        let ref rustc_bin = multirust_bin_dir.join("rustc");
        raw::write_file(multirust_bin, "").unwrap();
        raw::write_file(rustc_bin, "").unwrap();
        assert!(multirust_bin.exists());
        assert!(rustc_bin.exists());
        expect_ok(config, &["rustup-init", "-y"]);
        assert!(!multirust_bin.exists());
        assert!(!rustc_bin.exists());
    });
}

// Installation used to be to
// C:\Users\brian\AppData\Local\.multirust\bin
// instead of C:\Users\brian\.cargo\bin, etc.
#[test]
#[cfg(windows)]
#[ignore]
fn install_deletes_legacy_multirust_bins() {
    // This is untestable on Windows because the definiton multirust-rs
    // used for home couldn't be overridden and isn't on the same
    // code path as std::env::home.

    // This is where windows is considering $HOME:
    // windows::get_special_folder(&windows::FOLDERID_Profile).unwrap();
}

// rustup-init obeys CONFIG.CARGODIR, which multirust-rs *used* to set
// before installation moved from ~/.multirust/bin to ~/.cargo/bin.
// If installation running under the old multirust via `cargo run`,
// then CONFIG.CARGODIR will be set during installation, causing the
// install to go to the wrong place. Detect this scenario specifically
// and avoid it.
#[test]
#[cfg(unix)] // Can't test on windows without clobbering the home dir
fn legacy_upgrade_installs_to_correct_location() {
    setup(&|config| {
        let fake_cargo = config.rustupdir.join(".multirust/cargo");
        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        cmd.env("CARGO_HOME", format!("{}", fake_cargo.display()));
        assert!(cmd.output().unwrap().status.success());

        let rustup = config.homedir.join(&format!(".cargo/bin/rustup{}", EXE_SUFFIX));
        assert!(rustup.exists());
    });
}

#[test]
fn readline_no_stdin() {
    setup(&|config| {
        expect_err(config, &["rustup-init"],
                   "unable to read from stdin for confirmation");
    });
}

#[test]
fn rustup_init_works_with_weird_names() {
    // Browsers often rename bins to e.g. rustup-init(2).exe.

    setup(&|config| {
        let ref old = config.exedir.join(
            &format!("rustup-init{}", EXE_SUFFIX));
        let ref new = config.exedir.join(
            &format!("rustup-init(2){}", EXE_SUFFIX));
        fs::rename(old, new).unwrap();
        expect_ok(config, &["rustup-init(2)", "-y"]);
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        assert!(rustup.exists());
    });
}

// # 261
#[test]
#[cfg(windows)]
fn doesnt_write_wrong_path_type_to_reg() {
    use winreg::RegKey;
    use winreg::enums::RegType;
    use winapi::*;

    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.vtype == RegType::REG_EXPAND_SZ);

        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.vtype == RegType::REG_EXPAND_SZ);
    });
}


// HKCU\Environment\PATH may not exist during install, and it may need to be
// deleted during uninstall if we remove the last path from it
#[test]
#[cfg(windows)]
fn windows_handle_empty_path_registry_key() {
    use winreg::RegKey;
    use winreg::enums::RegType;
    use winapi::*;

    setup(&|config| {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let _ = environment.delete_value("PATH");

        expect_ok(config, &["rustup-init", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.vtype == RegType::REG_EXPAND_SZ);

        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let path = environment.get_raw_value("PATH");

        assert!(path.is_err());
    });
}

#[test]
#[cfg(windows)]
fn windows_uninstall_removes_semicolon_from_path() {
    use winreg::RegKey;
    use winreg::enums::RegType;
    use winapi::*;

    setup(&|config| {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();

        // This time set the value of PATH and make sure it's restored exactly after uninstall,
        // not leaving behind any semi-colons
        environment.set_value("PATH", &"foo").unwrap();

        expect_ok(config, &["rustup-init", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.vtype == RegType::REG_EXPAND_SZ);

        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let path: String = environment.get_value("PATH").unwrap();
        assert!(path == "foo");
    });
}

#[test]
#[cfg(windows)]
fn install_doesnt_mess_with_a_non_unicode_path() {
    use winreg::{RegKey, RegValue};
    use winreg::enums::RegType;
    use winapi::*;

    setup(&|config| {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();

        let reg_value = RegValue {
            bytes: vec![0x00, 0xD8,  // leading surrogate
                        0x01, 0x01,  // bogus trailing surrogate
                        0x00, 0x00], // null
            vtype: RegType::REG_EXPAND_SZ
        };
        environment.set_raw_value("PATH", &reg_value).unwrap();

        expect_stderr_ok(config, &["rustup-init", "-y"],
                         "the registry key HKEY_CURRENT_USER\\Environment\\PATH does not contain valid Unicode. \
                          Not modifying the PATH variable");

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.bytes == reg_value.bytes);
    });
}

#[test]
#[cfg(windows)]
fn uninstall_doesnt_mess_with_a_non_unicode_path() {
    use winreg::{RegKey, RegValue};
    use winreg::enums::RegType;
    use winapi::*;

    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();

        let reg_value = RegValue {
            bytes: vec![0x00, 0xD8,  // leading surrogate
                        0x01, 0x01,  // bogus trailing surrogate
                        0x00, 0x00], // null
            vtype: RegType::REG_EXPAND_SZ
        };
        environment.set_raw_value("PATH", &reg_value).unwrap();

        expect_stderr_ok(config, &["rustup", "self", "uninstall", "-y"],
                         "the registry key HKEY_CURRENT_USER\\Environment\\PATH does not contain valid Unicode. \
                          Not modifying the PATH variable");

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE).unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.bytes == reg_value.bytes);
    });
}

#[test]
#[ignore] // untestable
fn install_but_rustup_is_installed() {
}

#[test]
#[ignore] // untestable
fn install_but_rustc_is_installed() {
}

#[test]
fn install_but_rustup_sh_is_installed() {
    setup(&|config| {
        let rustup_dir = config.homedir.join(".rustup");
        fs::create_dir_all(&rustup_dir).unwrap();
        let version_file = rustup_dir.join("rustup-version");
        raw::write_file(&version_file, "").unwrap();
        expect_err(config, &["rustup-init", "-y"],
                   "cannot install while rustup.sh is installed");
    });
}

#[test]
fn install_but_rustup_metadata() {
    setup(&|config| {
        let multirust_dir = config.homedir.join(".multirust");
        fs::create_dir_all(&multirust_dir).unwrap();
        let version_file = multirust_dir.join("version");
        raw::write_file(&version_file, "2").unwrap();
        expect_err(config, &["rustup-init", "-y"],
                   "cannot install while multirust is installed");
    });
}

#[test]
fn legacy_upgrade_removes_multirust_bin() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        assert!(cmd.output().unwrap().status.success());

        let rustup_bin = config.cargodir.join(format!("bin/rustup{}", EXE_SUFFIX));
        let multirust_bin = config.cargodir.join(format!("bin/multirust{}", EXE_SUFFIX));
        fs::copy(&rustup_bin, &multirust_bin).unwrap();
        assert!(multirust_bin.exists());

        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        assert!(cmd.output().unwrap().status.success());

        assert!(!multirust_bin.exists());
    });
}

// Create a ~/.multirust symlink to ~/.rustup
#[test]
fn install_creates_legacy_home_symlink() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        // It'll only do this behavior when RUSTUP_HOME isn't set
        cmd.env_remove("RUSTUP_HOME");
        assert!(cmd.output().unwrap().status.success());

        let mut cmd = clitools::cmd(config, "rustc", &["--version"]);
        cmd.env_remove("RUSTUP_HOME");
        let out = String::from_utf8(cmd.output().unwrap().stdout).unwrap();
        assert!(out.contains("hash-s-2"));

        let rustup_dir = config.homedir.join(".rustup");
        assert!(rustup_dir.exists());
        let multirust_dir = config.homedir.join(".multirust");
        assert!(multirust_dir.exists());
        assert!(fs::symlink_metadata(&multirust_dir).unwrap().file_type().is_symlink());
    });
}

// Do upgrade over multirust. #848
#[test]
fn install_over_unupgraded_multirust_dir() {
    setup(&|config| {
        let rustup_dir = config.homedir.join(".rustup");
        let multirust_dir = config.homedir.join(".multirust");

        // Install rustup
        let mut cmd = clitools::cmd(config, "rustup-init", &["-y", "--default-toolchain=nightly"]);
        cmd.env_remove("RUSTUP_HOME");
        assert!(cmd.output().unwrap().status.success());

        let mut cmd = clitools::cmd(config, "rustc", &["--version"]);
        cmd.env_remove("RUSTUP_HOME");
        let out = String::from_utf8(cmd.output().unwrap().stdout).unwrap();
        assert!(out.contains("hash-n-2"));

        // Move .rustup to .multirust so the next rustup-init will be
        // an upgrade from ~/.multirust to ~/.rustup
        raw::remove_dir(&multirust_dir).unwrap();
        fs::rename(&rustup_dir, &multirust_dir).unwrap();
        assert!(!rustup_dir.exists());
        assert!(multirust_dir.exists());

        // Do the upgrade
        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        cmd.env_remove("RUSTUP_HOME");
        assert!(cmd.output().unwrap().status.success());

        // Directories should be set up correctly
        assert!(rustup_dir.exists());
        assert!(multirust_dir.exists());
        assert!(fs::symlink_metadata(&multirust_dir).unwrap().file_type().is_symlink());

        // We should still be on nightly
        let mut cmd = clitools::cmd(config, "rustc", &["--version"]);
        cmd.env_remove("RUSTUP_HOME");
        let out = String::from_utf8(cmd.output().unwrap().stdout).unwrap();
        assert!(out.contains("hash-n-2"));
    });
}

#[test]
fn uninstall_removes_legacy_home_symlink() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        // It'll only do this behavior when RUSTUP_HOME isn't set
        cmd.env_remove("RUSTUP_HOME");
        assert!(cmd.output().unwrap().status.success());

        let multirust_dir = config.homedir.join(".multirust");
        assert!(multirust_dir.exists());

        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);
        assert!(!multirust_dir.exists());
    });
}
