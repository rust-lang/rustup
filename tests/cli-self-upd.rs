//! Testing self install, uninstall and update

pub mod mock;

use crate::mock::clitools::{
    self, expect_err, expect_err_ex, expect_ok, expect_ok_contains, expect_ok_ex, expect_stderr_ok,
    expect_stdout_ok, this_host_triple, Config, Scenario,
};
use crate::mock::dist::calc_hash;
use crate::mock::{get_path, restore_path};
use lazy_static::lazy_static;
use remove_dir_all::remove_dir_all;
use rustup::utils::{raw, utils};
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use tempdir::TempDir;

macro_rules! for_host {
    ($s: expr) => {
        &format!($s, this_host_triple())
    };
}

const TEST_VERSION: &str = "1.1.1";

pub fn setup(f: &dyn Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, &|config| {
        // Lock protects environment variables
        lazy_static! {
            static ref LOCK: Mutex<()> = Mutex::new(());
        }
        let _g = LOCK.lock();

        // On windows these tests mess with the user's PATH. Save
        // and restore them here to keep from trashing things.
        let saved_path = get_path();
        let _g = scopeguard::guard(saved_path, restore_path);

        f(config);
    });
}

pub fn update_setup(f: &dyn Fn(&Config, &Path)) {
    setup(&|config| {
        // Create a mock self-update server
        let self_dist_tmp = TempDir::new("self_dist").unwrap();
        let self_dist = self_dist_tmp.path();

        let trip = this_host_triple();
        let dist_dir = self_dist.join(&format!("archive/{}/{}", TEST_VERSION, trip));
        let dist_exe = dist_dir.join(&format!("rustup-init{}", EXE_SUFFIX));
        let rustup_bin = config.exedir.join(&format!("rustup-init{}", EXE_SUFFIX));

        fs::create_dir_all(dist_dir).unwrap();
        output_release_file(self_dist, "1", TEST_VERSION);
        fs::copy(&rustup_bin, &dist_exe).unwrap();
        // Modify the exe so it hashes different
        raw::append_file(&dist_exe, "").unwrap();

        let root_url = format!("file://{}", self_dist.display());
        env::set_var("RUSTUP_UPDATE_ROOT", root_url);

        f(config, self_dist);
    });
}

fn output_release_file(dist_dir: &Path, schema: &str, version: &str) {
    let contents = format!(
        r#"
schema-version = "{}"
version = "{}"
"#,
        schema, version
    );
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
        let rust_lldb = config
            .cargodir
            .join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
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
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let rustc = config.cargodir.join(&format!("bin/rustc{}", EXE_SUFFIX));
        let rustdoc = config.cargodir.join(&format!("bin/rustdoc{}", EXE_SUFFIX));
        let cargo = config.cargodir.join(&format!("bin/cargo{}", EXE_SUFFIX));
        let rust_lldb = config
            .cargodir
            .join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
        let rust_gdb = config.cargodir.join(&format!("bin/rust-gdb{}", EXE_SUFFIX));
        assert!(is_exe(&rustup));
        assert!(is_exe(&rustc));
        assert!(is_exe(&rustdoc));
        assert!(is_exe(&cargo));
        assert!(is_exe(&rust_lldb));
        assert!(is_exe(&rust_gdb));
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
        remove_dir_all(&config.cargodir).unwrap();
        remove_dir_all(&config.rustupdir).unwrap();
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
        let rust_lldb = config
            .cargodir
            .join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
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
        let rust_lldb = config
            .cargodir
            .join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
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
        expect_err(
            config,
            &["rustup", "self", "uninstall", "-y"],
            "rustup is not installed",
        );
    });
}

// The other tests here just run rustup from a temp directory. This
// does the uninstall by actually invoking the installed binary in
// order to test that it can successfully delete itself.
#[test]
#[cfg_attr(target_os = "macos", ignore)] // FIXME #1515
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
        let rust_lldb = config
            .cargodir
            .join(&format!("bin/rust-lldb{}", EXE_SUFFIX));
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

        let parent = config.cargodir.parent().unwrap();
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
fn uninstall_stress_test() {}

#[cfg(unix)]
fn install_adds_path_to_rc(rcfile: &str) {
    setup(&|config| {
        let my_rc = "foo\nbar\nbaz";
        let rc = config.homedir.join(rcfile);
        raw::write_file(&rc, my_rc).unwrap();
        expect_ok(config, &["rustup-init", "-y"]);

        let new_rc = raw::read_file(&rc).unwrap();
        let addition = format!(r#"export PATH="{}/bin:$PATH""#, config.cargodir.display());
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
fn install_adds_path_to_bash_profile() {
    install_adds_path_to_rc(".bash_profile");
}

#[test]
#[cfg(unix)]
fn install_does_not_add_path_to_bash_profile_that_doesnt_exist() {
    setup(&|config| {
        let rc = config.homedir.join(".bash_profile");
        expect_ok(config, &["rustup-init", "-y"]);

        assert!(!rc.exists());
    });
}

#[test]
#[cfg(unix)]
fn install_with_zsh_adds_path_to_zprofile() {
    setup(&|config| {
        let my_rc = "foo\nbar\nbaz";
        let rc = config.homedir.join(".zprofile");
        raw::write_file(&rc, my_rc).unwrap();

        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        cmd.env("SHELL", "zsh");
        assert!(cmd.output().unwrap().status.success());

        let new_rc = raw::read_file(&rc).unwrap();
        let addition = format!(r#"export PATH="{}/bin:$PATH""#, config.cargodir.display());
        let expected = format!("{}\n{}\n", my_rc, addition);
        assert_eq!(new_rc, expected);
    });
}

#[test]
#[cfg(unix)]
fn install_with_zsh_adds_path_to_zdotdir_zprofile() {
    setup(&|config| {
        let zdotdir = TempDir::new("zdotdir").unwrap();
        let my_rc = "foo\nbar\nbaz";
        let rc = zdotdir.path().join(".zprofile");
        raw::write_file(&rc, my_rc).unwrap();

        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        cmd.env("SHELL", "zsh");
        cmd.env("ZDOTDIR", zdotdir.path());
        assert!(cmd.output().unwrap().status.success());

        let new_rc = raw::read_file(&rc).unwrap();
        let addition = format!(r#"export PATH="{}/bin:$PATH""#, config.cargodir.display());
        let expected = format!("{}\n{}\n", my_rc, addition);
        assert_eq!(new_rc, expected);
    });
}

#[test]
#[cfg(unix)]
fn install_adds_path_to_rcfile_just_once() {
    setup(&|config| {
        let my_profile = "foo\nbar\nbaz";
        let profile = config.homedir.join(".profile");
        raw::write_file(&profile, my_profile).unwrap();
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup-init", "-y"]);

        let new_profile = raw::read_file(&profile).unwrap();
        let addition = format!(r#"export PATH="{}/bin:$PATH""#, config.cargodir.display());
        let expected = format!("{}\n{}\n", my_profile, addition);
        assert_eq!(new_profile, expected);
    });
}

#[cfg(unix)]
fn uninstall_removes_path_from_rc(rcfile: &str) {
    setup(&|config| {
        let my_rc = "foo\nbar\nbaz";
        let rc = config.homedir.join(rcfile);
        raw::write_file(&rc, my_rc).unwrap();
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let new_rc = raw::read_file(&rc).unwrap();
        assert_eq!(new_rc, my_rc);
    });
}

#[test]
#[cfg(unix)]
fn uninstall_removes_path_from_profile() {
    uninstall_removes_path_from_rc(".profile");
}

#[test]
#[cfg(unix)]
fn uninstall_removes_path_from_bash_profile() {
    uninstall_removes_path_from_rc(".bash_profile");
}

#[test]
#[cfg(unix)]
fn uninstall_doesnt_touch_rc_files_that_dont_contain_cargo_home() {
    setup(&|config| {
        let my_rc = "foo\nbar\nbaz";
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let profile = config.homedir.join(".profile");
        raw::write_file(&profile, my_rc).unwrap();

        let profile = raw::read_file(&profile).unwrap();

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
        let profile = config.homedir.join(".profile");
        raw::write_file(&profile, my_profile).unwrap();
        let mut cmd = clitools::cmd(config, "rustup-init", &["-y"]);
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());

        let new_profile = raw::read_file(&profile).unwrap();
        let expected = format!("{}\nexport PATH=\"$HOME/.cargo/bin:$PATH\"\n", my_profile);
        assert_eq!(new_profile, expected);

        let mut cmd = clitools::cmd(config, "rustup", &["self", "uninstall", "-y"]);
        cmd.env_remove("CARGO_HOME");
        assert!(cmd.output().unwrap().status.success());

        let new_profile = raw::read_file(&profile).unwrap();
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
        let profile = config.homedir.join(".profile");
        expect_ok(config, &["rustup-init", "-y", "--no-modify-path"]);
        assert!(!profile.exists());
    });
}

#[test]
#[cfg(windows)]
fn install_doesnt_modify_path_if_passed_no_modify_path() {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    setup(&|config| {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let old_path = environment.get_raw_value("PATH").unwrap();

        expect_ok(config, &["rustup-init", "-y", "--no-modify-path"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let new_path = environment.get_raw_value("PATH").unwrap();

        assert!(old_path == new_path);
    });
}

#[test]
fn update_exact() {
    let version = env!("CARGO_PKG_VERSION");
    let expected_output = &(r"info: checking for self-updates
info: downloading self-update
info: rustup updated successfully to "
        .to_owned()
        + version
        + "
");

    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok_ex(config, &["rustup", "self", "update"], r"", expected_output)
    });
}

#[test]
fn update_but_not_installed() {
    update_setup(&|config, _| {
        expect_err_ex(
            config,
            &["rustup", "self", "update"],
            r"",
            &format!(
                r"error: rustup is not installed at '{}'
",
                config.cargodir.display()
            ),
        );
    });
}

#[test]
fn update_but_delete_existing_updater_first() {
    update_setup(&|config, _| {
        // The updater is stored in a known location
        let setup = config
            .cargodir
            .join(&format!("bin/rustup-init{}", EXE_SUFFIX));

        expect_ok(config, &["rustup-init", "-y"]);

        // If it happens to already exist for some reason it
        // should just be deleted.
        raw::write_file(&setup, "").unwrap();
        expect_ok(config, &["rustup", "self", "update"]);

        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        assert!(rustup.exists());
    });
}

#[test]
fn update_download_404() {
    update_setup(&|config, self_dist| {
        expect_ok(config, &["rustup-init", "-y"]);

        let trip = this_host_triple();
        let dist_dir = self_dist.join(&format!("archive/{}/{}", TEST_VERSION, trip));
        let dist_exe = dist_dir.join(&format!("rustup-init{}", EXE_SUFFIX));

        fs::remove_file(dist_exe).unwrap();

        expect_err(
            config,
            &["rustup", "self", "update"],
            "could not download file",
        );
    });
}

#[test]
fn update_bogus_version() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_err(
            config,
            &["rustup", "update", "1.0.0-alpha"],
            "could not download nonexistent rust version `1.0.0-alpha`",
        );
    });
}

// Check that rustup.exe has changed after the update. This
// is hard for windows because the running process needs to exit
// before the new updater can delete it.
#[test]
fn update_updates_rustup_bin() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);

        let bin = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let before_hash = calc_hash(&bin);

        // Running the self update command on the installed binary,
        // so that the running binary must be replaced.
        let mut cmd = Command::new(&bin);
        cmd.args(&["self", "update"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();

        println!("out: {}", String::from_utf8(out.stdout).unwrap());
        println!("err: {}", String::from_utf8(out.stderr).unwrap());

        assert!(out.status.success());

        let after_hash = calc_hash(&bin);

        assert_ne!(before_hash, after_hash);
    });
}

#[test]
fn update_bad_schema() {
    update_setup(&|config, self_dist| {
        expect_ok(config, &["rustup-init", "-y"]);
        output_release_file(self_dist, "17", "1.1.1");
        expect_err(
            config,
            &["rustup", "self", "update"],
            "unknown schema version",
        );
    });
}

#[test]
fn update_no_change() {
    let version = env!("CARGO_PKG_VERSION");
    update_setup(&|config, self_dist| {
        expect_ok(config, &["rustup-init", "-y"]);
        output_release_file(self_dist, "1", version);
        expect_ok_ex(
            config,
            &["rustup", "self", "update"],
            r"",
            r"info: checking for self-updates
",
        );
    });
}

#[test]
fn rustup_self_updates() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);

        let bin = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let before_hash = calc_hash(&bin);

        expect_ok(config, &["rustup", "update"]);

        let after_hash = calc_hash(&bin);

        assert_ne!(before_hash, after_hash);
    })
}

#[test]
fn rustup_self_updates_with_specified_toolchain() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);

        let bin = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let before_hash = calc_hash(&bin);

        expect_ok(config, &["rustup", "update", "stable"]);

        let after_hash = calc_hash(&bin);

        assert_ne!(before_hash, after_hash);
    })
}

#[test]
fn rustup_no_self_update_with_specified_toolchain() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);

        let bin = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        let before_hash = calc_hash(&bin);

        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);

        let after_hash = calc_hash(&bin);

        assert_eq!(before_hash, after_hash);
    })
}

#[test]
fn rustup_self_update_exact() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);

        expect_ok_ex(
            config,
            &["rustup", "update"],
            for_host!(
                r"
  stable-{0} unchanged - 1.1.0 (hash-s-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: checking for self-updates
info: downloading self-update
"
            ),
        );
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

        let setup = config
            .cargodir
            .join(&format!("bin/rustup-init{}", EXE_SUFFIX));
        assert!(setup.exists());
    });
}

#[test]
fn updater_is_deleted_after_running_rustup() {
    update_setup(&|config, _| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_ok(config, &["rustup", "self", "update"]);

        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);

        let setup = config
            .cargodir
            .join(&format!("bin/rustup-init{}", EXE_SUFFIX));
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

        let setup = config
            .cargodir
            .join(&format!("bin/rustup-init{}", EXE_SUFFIX));
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
fn update_stress_test() {}

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
        expect_ok_contains(
            config,
            &["rustup-init", "-y"],
            r"
  stable installed - 1.1.0 (hash-s-2)

",
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: default toolchain set to 'stable'
"
            ),
        );
    });
}

#[test]
fn reinstall_exact() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_stderr_ok(
            config,
            &["rustup-init", "-y"],
            r"info: updating existing rustup installation
",
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
        let envfile = config.homedir.join(".cargo/env");
        let envfile = raw::read_file(&envfile).unwrap();
        assert!(envfile.contains(r#"export PATH="$HOME/.cargo/bin:$PATH""#));
    });
}

#[test]
#[cfg(windows)]
fn doesnt_produce_env_file_on_windows() {}

#[test]
fn install_sets_up_stable() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
    });
}

#[test]
fn install_sets_up_stable_unless_a_different_default_is_requested() {
    setup(&|config| {
        expect_ok(
            config,
            &["rustup-init", "-y", "--default-toolchain", "nightly"],
        );
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
    });
}

#[test]
fn install_sets_up_stable_unless_there_is_already_a_default() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "stable"]);
        expect_ok(config, &["rustup-init", "-y"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        expect_err(
            config,
            &["rustup", "run", "stable", "rustc", "--version"],
            for_host!("toolchain 'stable-{0}' is not installed"),
        );
    });
}

#[test]
fn readline_no_stdin() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup-init"],
            "unable to read from stdin for confirmation",
        );
    });
}

#[test]
fn rustup_init_works_with_weird_names() {
    // Browsers often rename bins to e.g. rustup-init(2).exe.

    setup(&|config| {
        let old = config.exedir.join(&format!("rustup-init{}", EXE_SUFFIX));
        let new = config.exedir.join(&format!("rustup-init(2){}", EXE_SUFFIX));
        utils::rename_file("test", &old, &new).unwrap();
        expect_ok(config, &["rustup-init(2)", "-y"]);
        let rustup = config.cargodir.join(&format!("bin/rustup{}", EXE_SUFFIX));
        assert!(rustup.exists());
    });
}

// # 261
#[test]
#[cfg(windows)]
fn doesnt_write_wrong_path_type_to_reg() {
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.vtype == RegType::REG_EXPAND_SZ);

        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.vtype == RegType::REG_EXPAND_SZ);
    });
}

// HKCU\Environment\PATH may not exist during install, and it may need to be
// deleted during uninstall if we remove the last path from it
#[test]
#[cfg(windows)]
fn windows_handle_empty_path_registry_key() {
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    setup(&|config| {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let _ = environment.delete_value("PATH");

        expect_ok(config, &["rustup-init", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.vtype == RegType::REG_EXPAND_SZ);

        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let path = environment.get_raw_value("PATH");

        assert!(path.is_err());
    });
}

#[test]
#[cfg(windows)]
fn windows_uninstall_removes_semicolon_from_path() {
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    setup(&|config| {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();

        // This time set the value of PATH and make sure it's restored exactly after uninstall,
        // not leaving behind any semi-colons
        environment.set_value("PATH", &"foo").unwrap();

        expect_ok(config, &["rustup-init", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.vtype == RegType::REG_EXPAND_SZ);

        expect_ok(config, &["rustup", "self", "uninstall", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let path: String = environment.get_value("PATH").unwrap();
        assert!(path == "foo");
    });
}

#[test]
#[cfg(windows)]
fn install_doesnt_mess_with_a_non_unicode_path() {
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::{RegKey, RegValue};

    setup(&|config| {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();

        let reg_value = RegValue {
            bytes: vec![
                0x00, 0xD8, // leading surrogate
                0x01, 0x01, // bogus trailing surrogate
                0x00, 0x00,
            ], // null
            vtype: RegType::REG_EXPAND_SZ,
        };
        environment.set_raw_value("PATH", &reg_value).unwrap();

        expect_stderr_ok(config, &["rustup-init", "-y"],
                         "the registry key HKEY_CURRENT_USER\\Environment\\PATH does not contain valid Unicode. \
                          Not modifying the PATH variable");

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.bytes == reg_value.bytes);
    });
}

#[test]
#[cfg(windows)]
fn uninstall_doesnt_mess_with_a_non_unicode_path() {
    use winreg::enums::{RegType, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::{RegKey, RegValue};

    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();

        let reg_value = RegValue {
            bytes: vec![
                0x00, 0xD8, // leading surrogate
                0x01, 0x01, // bogus trailing surrogate
                0x00, 0x00,
            ], // null
            vtype: RegType::REG_EXPAND_SZ,
        };
        environment.set_raw_value("PATH", &reg_value).unwrap();

        expect_stderr_ok(config, &["rustup", "self", "uninstall", "-y"],
                         "the registry key HKEY_CURRENT_USER\\Environment\\PATH does not contain valid Unicode. \
                          Not modifying the PATH variable");

        let root = RegKey::predef(HKEY_CURRENT_USER);
        let environment = root
            .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
            .unwrap();
        let path = environment.get_raw_value("PATH").unwrap();
        assert!(path.bytes == reg_value.bytes);
    });
}

#[test]
#[ignore] // untestable
fn install_but_rustup_is_installed() {}

#[test]
#[ignore] // untestable
fn install_but_rustc_is_installed() {}

#[test]
fn install_but_rustup_sh_is_installed() {
    setup(&|config| {
        let rustup_dir = config.homedir.join(".rustup");
        fs::create_dir_all(&rustup_dir).unwrap();
        let version_file = rustup_dir.join("rustup-version");
        raw::write_file(&version_file, "").unwrap();
        expect_err(
            config,
            &["rustup-init", "-y"],
            "cannot install while rustup.sh is installed",
        );
    });
}

#[test]
fn rls_proxy_set_up_after_install() {
    setup(&|config| {
        expect_ok(config, &["rustup-init", "-y"]);
        expect_err(
            config,
            &["rls", "--version"],
            &format!(
                "'rls{}' is not installed for the toolchain 'stable-{}'",
                EXE_SUFFIX,
                this_host_triple(),
            ),
        );
        expect_ok(config, &["rustup", "component", "add", "rls"]);
        expect_ok(config, &["rls", "--version"]);
    });
}

#[test]
fn rls_proxy_set_up_after_update() {
    update_setup(&|config, _| {
        let rls_path = config.cargodir.join(format!("bin/rls{}", EXE_SUFFIX));
        expect_ok(config, &["rustup-init", "-y"]);
        fs::remove_file(&rls_path).unwrap();
        expect_ok(config, &["rustup", "self", "update"]);
        assert!(rls_path.exists());
    });
}

#[test]
fn update_does_not_overwrite_rustfmt() {
    update_setup(&|config, self_dist| {
        expect_ok(config, &["rustup-init", "-y"]);
        let version = env!("CARGO_PKG_VERSION");
        output_release_file(self_dist, "1", version);

        // Since we just did a fresh install rustfmt will exist. Let's emulate
        // it not existing in this test though by removing it just after our
        // installation.
        let rustfmt_path = config.cargodir.join(format!("bin/rustfmt{}", EXE_SUFFIX));
        assert!(rustfmt_path.exists());
        fs::remove_file(&rustfmt_path).unwrap();
        raw::write_file(&rustfmt_path, "").unwrap();
        assert_eq!(utils::file_size(&rustfmt_path).unwrap(), 0);

        // Ok, now a self-update should complain about `rustfmt` not looking
        // like rustup and the user should take some action.
        expect_stderr_ok(
            config,
            &["rustup", "self", "update"],
            "`rustfmt` is already installed",
        );
        assert!(rustfmt_path.exists());
        assert_eq!(utils::file_size(&rustfmt_path).unwrap(), 0);

        // Now simluate us removing the rustfmt executable and rerunning a self
        // update, this should install the rustup shim. Note that we don't run
        // `rustup` here but rather the rustup we've actually installed, this'll
        // help reproduce bugs related to having that file being opened by the
        // current process.
        fs::remove_file(&rustfmt_path).unwrap();
        let installed_rustup = config.cargodir.join("bin/rustup");
        expect_ok(
            config,
            &[installed_rustup.to_str().unwrap(), "self", "update"],
        );
        assert!(rustfmt_path.exists());
        assert!(utils::file_size(&rustfmt_path).unwrap() > 0);
    });
}

#[test]
fn update_installs_clippy_cargo_and() {
    update_setup(&|config, self_dist| {
        expect_ok(config, &["rustup-init", "-y"]);
        let version = env!("CARGO_PKG_VERSION");
        output_release_file(self_dist, "1", version);

        let cargo_clippy_path = config
            .cargodir
            .join(format!("bin/cargo-clippy{}", EXE_SUFFIX));
        assert!(cargo_clippy_path.exists());
    });
}
