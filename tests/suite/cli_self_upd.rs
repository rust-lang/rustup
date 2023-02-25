//! Testing self install, uninstall and update

use std::env;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::Path;
use std::process::Command;

use remove_dir_all::remove_dir_all;

use rustup::test::{this_host_triple, with_saved_path};
use rustup::utils::{raw, utils};
use rustup::{for_host, Notification, DUP_TOOLS, TOOLS};

use crate::mock::{
    clitools::{self, output_release_file, self_update_setup, Config, Scenario},
    dist::calc_hash,
};

const TEST_VERSION: &str = "1.1.1";

pub fn update_setup(f: &dyn Fn(&mut Config, &Path)) {
    self_update_setup(f, TEST_VERSION)
}

/// Empty dist server, rustup installed with no toolchain
fn setup_empty_installed(f: &dyn Fn(&mut Config)) {
    clitools::test(Scenario::Empty, &|config| {
        config.expect_ok(&[
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ]);
        f(config);
    })
}

/// SimpleV3 dist server, rustup installed with default toolchain
fn setup_installed(f: &dyn Fn(&mut Config)) {
    clitools::test(Scenario::SimpleV2, &|config| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        f(config);
    })
}

#[test]
/// This is the primary smoke test testing the full end to end behavior of the
/// installation code path: everything that is output, the proxy installation,
/// status of the proxies.
fn install_bins_to_cargo_home() {
    clitools::test(Scenario::SimpleV2, &|config| {
        with_saved_path(&mut || {
            config.expect_ok_contains(
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
            );
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
                let path = &config.cargodir.join(&format!("bin/{tool}{EXE_SUFFIX}"));
                check(path);
            }
        })
    });
}

#[test]
fn install_twice() {
    clitools::test(Scenario::SimpleV2, &|config| {
        with_saved_path(&mut || {
            config.expect_ok(&["rustup-init", "-y"]);
            config.expect_ok(&["rustup-init", "-y"]);
            let rustup = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
            assert!(rustup.exists());
        })
    });
}

#[test]
/// Smoke test for the entire install process when dirs need to be made :
/// depending just on unit tests here could miss subtle dependencies being added
/// earlier in the code, so a black-box test is needed.
fn install_creates_cargo_home() {
    clitools::test(Scenario::Empty, &|config| {
        remove_dir_all(&config.cargodir).unwrap();
        config.rustupdir.remove().unwrap();
        config.expect_ok(&[
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ]);
        assert!(config.cargodir.exists());
    });
}

#[test]
/// Functional test needed here - we need to do the full dance where we start
/// with rustup.exe and end up deleting that exe itself.
fn uninstall_deletes_bins() {
    setup_empty_installed(&|config| {
        // no-modify-path isn't needed here, as the test-dir-path isn't present
        // in the registry, so the no-change code path will be triggered.
        config.expect_ok(&["rustup", "self", "uninstall", "-y"]);
        let rustup = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        let rustc = config.cargodir.join(format!("bin/rustc{EXE_SUFFIX}"));
        let rustdoc = config.cargodir.join(format!("bin/rustdoc{EXE_SUFFIX}"));
        let cargo = config.cargodir.join(format!("bin/cargo{EXE_SUFFIX}"));
        let rust_lldb = config.cargodir.join(format!("bin/rust-lldb{EXE_SUFFIX}"));
        let rust_gdb = config.cargodir.join(format!("bin/rust-gdb{EXE_SUFFIX}"));
        let rust_gdbgui = config.cargodir.join(format!("bin/rust-gdbgui{EXE_SUFFIX}"));
        assert!(!rustup.exists());
        assert!(!rustc.exists());
        assert!(!rustdoc.exists());
        assert!(!cargo.exists());
        assert!(!rust_lldb.exists());
        assert!(!rust_gdb.exists());
        assert!(!rust_gdbgui.exists());
    });
}

#[test]
fn uninstall_deletes_installed_toolchains() {
    setup_installed(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        config.expect_ok(&["rustup", "toolchain", "link", "custom", &path]);
        config.expect_ok_contains(
            &["rustup", "self", "uninstall", "-y"],
            "",
            r"
info: uninstalling toolchain 'custom'
info: toolchain 'custom' uninstalled
",
        );
        assert!(!&config.rustupdir.join("toolchains").exists())
    });
}

#[test]
fn uninstall_works_if_some_bins_dont_exist() {
    setup_empty_installed(&|config| {
        let rustup = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        let rustc = config.cargodir.join(format!("bin/rustc{EXE_SUFFIX}"));
        let rustdoc = config.cargodir.join(format!("bin/rustdoc{EXE_SUFFIX}"));
        let cargo = config.cargodir.join(format!("bin/cargo{EXE_SUFFIX}"));
        let rust_lldb = config.cargodir.join(format!("bin/rust-lldb{EXE_SUFFIX}"));
        let rust_gdb = config.cargodir.join(format!("bin/rust-gdb{EXE_SUFFIX}"));
        let rust_gdbgui = config.cargodir.join(format!("bin/rust-gdbgui{EXE_SUFFIX}"));

        fs::remove_file(&rustc).unwrap();
        fs::remove_file(&cargo).unwrap();

        config.expect_ok(&["rustup", "self", "uninstall", "-y"]);

        assert!(!rustup.exists());
        assert!(!rustc.exists());
        assert!(!rustdoc.exists());
        assert!(!cargo.exists());
        assert!(!rust_lldb.exists());
        assert!(!rust_gdb.exists());
        assert!(!rust_gdbgui.exists());
    });
}

#[test]
fn uninstall_deletes_rustup_home() {
    setup_empty_installed(&|config| {
        config.expect_ok(&["rustup", "self", "uninstall", "-y"]);
        assert!(!config.rustupdir.has("."));
    });
}

#[test]
fn uninstall_works_if_rustup_home_doesnt_exist() {
    setup_empty_installed(&|config| {
        config.rustupdir.remove().unwrap();
        config.expect_ok(&["rustup", "self", "uninstall", "-y"]);
    });
}

#[test]
fn uninstall_deletes_cargo_home() {
    setup_empty_installed(&|config| {
        config.expect_ok(&["rustup", "self", "uninstall", "-y"]);
        assert!(!config.cargodir.exists());
    });
}

#[test]
fn uninstall_fails_if_not_installed() {
    setup_empty_installed(&|config| {
        let rustup = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        fs::remove_file(rustup).unwrap();
        config.expect_err(
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
    setup_empty_installed(&|config| {
        let rustup = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        let mut cmd = Command::new(rustup.clone());
        cmd.args(["self", "uninstall", "-y"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        println!("out: {}", String::from_utf8(out.stdout).unwrap());
        println!("err: {}", String::from_utf8(out.stderr).unwrap());

        assert!(out.status.success());
        assert!(!rustup.exists());
        assert!(!config.cargodir.exists());

        let rustc = config.cargodir.join(format!("bin/rustc{EXE_SUFFIX}"));
        let rustdoc = config.cargodir.join(format!("bin/rustdoc{EXE_SUFFIX}"));
        let cargo = config.cargodir.join(format!("bin/cargo{EXE_SUFFIX}"));
        let rust_lldb = config.cargodir.join(format!("bin/rust-lldb{EXE_SUFFIX}"));
        let rust_gdb = config.cargodir.join(format!("bin/rust-gdb{EXE_SUFFIX}"));
        let rust_gdbgui = config.cargodir.join(format!("bin/rust-gdbgui{EXE_SUFFIX}"));
        assert!(!rustc.exists());
        assert!(!rustdoc.exists());
        assert!(!cargo.exists());
        assert!(!rust_lldb.exists());
        assert!(!rust_gdb.exists());
        assert!(!rust_gdbgui.exists());
    });
}

// On windows rustup self uninstall temporarily puts a rustup-gc-$randomnumber.exe
// file in CONFIG.CARGODIR/.. ; check that it doesn't exist.
#[test]
fn uninstall_doesnt_leave_gc_file() {
    use std::thread;
    use std::time::Duration;

    setup_empty_installed(&|config| {
        config.expect_ok(&["rustup", "self", "uninstall", "-y"]);

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
fn update_exact() {
    let version = env!("CARGO_PKG_VERSION");
    let expected_output = "info: checking for self-update
info: downloading self-update
"
    .to_string();

    update_setup(&|config, _| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        config.expect_ok_ex(
            &["rustup", "self", "update"],
            &format!("  rustup updated - {version} (from {version})\n\n",),
            &expected_output,
        )
    });
}

#[test]
fn update_but_not_installed() {
    update_setup(&|config, _| {
        config.expect_err_ex(
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
        let setup = config.cargodir.join(format!("bin/rustup-init{EXE_SUFFIX}"));

        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);

        // If it happens to already exist for some reason it
        // should just be deleted.
        raw::write_file(&setup, "").unwrap();
        config.expect_ok(&["rustup", "self", "update"]);

        let rustup = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        assert!(rustup.exists());
    });
}

#[test]
fn update_download_404() {
    update_setup(&|config, self_dist| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);

        let trip = this_host_triple();
        let dist_dir = self_dist.join(format!("archive/{TEST_VERSION}/{trip}"));
        let dist_exe = dist_dir.join(format!("rustup-init{EXE_SUFFIX}"));

        fs::remove_file(dist_exe).unwrap();

        config.expect_err(&["rustup", "self", "update"], "could not download file");
    });
}

#[test]
fn update_bogus_version() {
    update_setup(&|config, _| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        config.expect_err(
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
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);

        let bin = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        let before_hash = calc_hash(&bin);

        // Running the self update command on the installed binary,
        // so that the running binary must be replaced.
        let mut cmd = Command::new(&bin);
        cmd.args(["self", "update"]);
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
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        output_release_file(self_dist, "17", "1.1.1");
        config.expect_err(&["rustup", "self", "update"], "unknown schema version");
    });
}

#[test]
fn update_no_change() {
    let version = env!("CARGO_PKG_VERSION");
    update_setup(&|config, self_dist| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        output_release_file(self_dist, "1", version);
        config.expect_ok_ex(
            &["rustup", "self", "update"],
            &format!(
                r"  rustup unchanged - {version}

"
            ),
            r"info: checking for self-update
",
        );
    });
}

#[test]
fn rustup_self_updates_trivial() {
    update_setup(&|config, _| {
        config.expect_ok(&["rustup", "set", "auto-self-update", "enable"]);
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);

        let bin = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        let before_hash = calc_hash(&bin);

        config.expect_ok(&["rustup", "update"]);

        let after_hash = calc_hash(&bin);

        assert_ne!(before_hash, after_hash);
    })
}

#[test]
fn rustup_self_updates_with_specified_toolchain() {
    update_setup(&|config, _| {
        config.expect_ok(&["rustup", "set", "auto-self-update", "enable"]);
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);

        let bin = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        let before_hash = calc_hash(&bin);

        config.expect_ok(&["rustup", "update", "stable"]);

        let after_hash = calc_hash(&bin);

        assert_ne!(before_hash, after_hash);
    })
}

#[test]
fn rustup_no_self_update_with_specified_toolchain() {
    update_setup(&|config, _| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);

        let bin = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        let before_hash = calc_hash(&bin);

        config.expect_ok(&["rustup", "update", "stable"]);

        let after_hash = calc_hash(&bin);

        assert_eq!(before_hash, after_hash);
    })
}

#[test]
fn rustup_self_update_exact() {
    update_setup(&|config, _| {
        config.expect_ok(&["rustup", "set", "auto-self-update", "enable"]);
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);

        config.expect_ok_ex(
            &["rustup", "update"],
            for_host!(
                r"
  stable-{0} unchanged - 1.1.0 (hash-stable-1.1.0)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: checking for self-update
info: downloading self-update
info: cleaning up downloads & tmp directories
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
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        config.expect_ok(&["rustup", "update", "nightly"]);
        config.expect_ok(&["rustup", "self", "update"]);

        let setup = config.cargodir.join(format!("bin/rustup-init{EXE_SUFFIX}"));
        assert!(setup.exists());
    });
}

#[test]
fn updater_is_deleted_after_running_rustup() {
    update_setup(&|config, _| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        config.expect_ok(&["rustup", "update", "nightly"]);
        config.expect_ok(&["rustup", "self", "update"]);

        config.expect_ok(&["rustup", "update", "nightly"]);

        let setup = config.cargodir.join(format!("bin/rustup-init{EXE_SUFFIX}"));
        assert!(!setup.exists());
    });
}

#[test]
fn updater_is_deleted_after_running_rustc() {
    update_setup(&|config, _| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_ok(&["rustup", "self", "update"]);

        config.expect_ok(&["rustc", "--version"]);

        let setup = config.cargodir.join(format!("bin/rustup-init{EXE_SUFFIX}"));
        assert!(!setup.exists());
    });
}

#[test]
fn rustup_still_works_after_update() {
    update_setup(&|config, _| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_ok(&["rustup", "self", "update"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
        config.expect_ok(&["rustup", "default", "beta"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
    });
}

// The installer used to be called rustup-setup. For compatibility it
// still needs to work in that mode.
#[test]
fn as_rustup_setup() {
    clitools::test(Scenario::Empty, &|config| {
        let init = config.exedir.join(format!("rustup-init{EXE_SUFFIX}"));
        let setup = config.exedir.join(format!("rustup-setup{EXE_SUFFIX}"));
        fs::copy(init, setup).unwrap();
        config.expect_ok(&[
            "rustup-setup",
            "-y",
            "--no-modify-path",
            "--default-toolchain",
            "none",
        ]);
    });
}

#[test]
fn reinstall_exact() {
    setup_empty_installed(&|config| {
        config.expect_stderr_ok(
            &[
                "rustup-init",
                "-y",
                "--no-update-default-toolchain",
                "--no-modify-path",
            ],
            r"info: updating existing rustup installation - leaving toolchains alone",
        );
    });
}

#[test]
fn reinstall_specifying_toolchain() {
    setup_installed(&|config| {
        config.expect_stdout_ok(
            &[
                "rustup-init",
                "-y",
                "--default-toolchain=stable",
                "--no-modify-path",
            ],
            for_host!(r"stable-{0} unchanged - 1.1.0"),
        );
    });
}

#[test]
fn reinstall_specifying_component() {
    setup_installed(&|config| {
        config.expect_ok(&["rustup", "component", "add", "rls"]);
        config.expect_stdout_ok(
            &[
                "rustup-init",
                "-y",
                "--default-toolchain=stable",
                "--no-modify-path",
            ],
            for_host!(r"stable-{0} unchanged - 1.1.0"),
        );
    });
}

#[test]
fn reinstall_specifying_different_toolchain() {
    clitools::test(Scenario::SimpleV2, &|config| {
        config.expect_stderr_ok(
            &[
                "rustup-init",
                "-y",
                "--default-toolchain=nightly",
                "--no-modify-path",
            ],
            for_host!(r"info: default toolchain set to 'nightly-{0}'"),
        );
    });
}

#[test]
fn install_sets_up_stable_unless_a_different_default_is_requested() {
    clitools::test(Scenario::SimpleV2, &|config| {
        config.expect_ok(&[
            "rustup-init",
            "-y",
            "--default-toolchain",
            "nightly",
            "--no-modify-path",
        ]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    });
}

#[test]
fn install_sets_up_stable_unless_there_is_already_a_default() {
    setup_installed(&|config| {
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_ok(&["rustup", "toolchain", "remove", "stable"]);
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
        config.expect_err(
            &["rustup", "run", "stable", "rustc", "--version"],
            for_host!("toolchain 'stable-{0}' is not installed"),
        );
    });
}

#[test]
fn readline_no_stdin() {
    clitools::test(Scenario::SimpleV2, &|config| {
        config.expect_err(
            &["rustup-init", "--no-modify-path"],
            "unable to read from stdin for confirmation",
        );
    });
}

#[test]
fn rustup_init_works_with_weird_names() {
    // Browsers often rename bins to e.g. rustup-init(2).exe.
    clitools::test(Scenario::SimpleV2, &|config| {
        let old = config.exedir.join(format!("rustup-init{EXE_SUFFIX}"));
        let new = config.exedir.join(format!("rustup-init(2){EXE_SUFFIX}"));
        utils::rename_file("test", &old, &new, &|_: Notification<'_>| {}).unwrap();
        config.expect_ok(&["rustup-init(2)", "-y", "--no-modify-path"]);
        let rustup = config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        assert!(rustup.exists());
    });
}

#[test]
fn install_but_rustup_sh_is_installed() {
    clitools::test(Scenario::Empty, &|config| {
        config.create_rustup_sh_metadata();
        config.expect_stderr_ok(
            &[
                "rustup-init",
                "-y",
                "--default-toolchain",
                "none",
                "--no-modify-path",
            ],
            "cannot install while rustup.sh is installed",
        );
    });
}

#[test]
fn test_warn_succeed_if_rustup_sh_already_installed_y_flag() {
    clitools::test(Scenario::SimpleV2, &|config| {
        config.create_rustup_sh_metadata();
        let out = config.run("rustup-init", &["-y", "--no-modify-path"], &[]);
        assert!(out.ok);
        assert!(out
            .stderr
            .contains("warning: it looks like you have existing rustup.sh metadata"));
        assert!(out
            .stderr
            .contains("error: cannot install while rustup.sh is installed"));
        assert!(out.stderr.contains(
            "warning: continuing (because the -y flag is set and the error is ignorable)"
        ));
        assert!(!out.stdout.contains("Continue? (y/N)"));
    })
}

#[test]
fn test_succeed_if_rustup_sh_already_installed_env_var_set() {
    clitools::test(Scenario::SimpleV2, &|config| {
        config.create_rustup_sh_metadata();
        let out = config.run(
            "rustup-init",
            &["-y", "--no-modify-path"],
            &[("RUSTUP_INIT_SKIP_EXISTENCE_CHECKS", "yes")],
        );
        assert!(out.ok);
        assert!(!out
            .stderr
            .contains("warning: it looks like you have existing rustup.sh metadata"));
        assert!(!out
            .stderr
            .contains("error: cannot install while rustup.sh is installed"));
        assert!(!out.stderr.contains(
            "warning: continuing (because the -y flag is set and the error is ignorable)"
        ));
        assert!(!out.stdout.contains("Continue? (y/N)"));
    })
}

#[test]
fn rls_proxy_set_up_after_install() {
    setup_installed(&|config| {
        config.expect_err(
            &["rls", "--version"],
            &format!(
                "'rls{}' is not installed for the toolchain 'stable-{}'",
                EXE_SUFFIX,
                this_host_triple(),
            ),
        );
        config.expect_ok(&["rustup", "component", "add", "rls"]);
        config.expect_ok(&["rls", "--version"]);
    });
}

#[test]
fn rls_proxy_set_up_after_update() {
    update_setup(&|config, _| {
        let rls_path = config.cargodir.join(format!("bin/rls{EXE_SUFFIX}"));
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        fs::remove_file(&rls_path).unwrap();
        config.expect_ok(&["rustup", "self", "update"]);
        assert!(rls_path.exists());
    });
}

#[test]
fn update_does_not_overwrite_rustfmt() {
    update_setup(&|config, self_dist| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        let version = env!("CARGO_PKG_VERSION");
        output_release_file(self_dist, "1", version);

        // Since we just did a fresh install rustfmt will exist. Let's emulate
        // it not existing in this test though by removing it just after our
        // installation.
        let rustfmt_path = config.cargodir.join(format!("bin/rustfmt{EXE_SUFFIX}"));
        assert!(rustfmt_path.exists());
        fs::remove_file(&rustfmt_path).unwrap();
        raw::write_file(&rustfmt_path, "").unwrap();
        assert_eq!(utils::file_size(&rustfmt_path).unwrap(), 0);

        // Ok, now a self-update should complain about `rustfmt` not looking
        // like rustup and the user should take some action.
        config.expect_stderr_ok(
            &["rustup", "self", "update"],
            "`rustfmt` is already installed",
        );
        assert!(rustfmt_path.exists());
        assert_eq!(utils::file_size(&rustfmt_path).unwrap(), 0);

        // Now simulate us removing the rustfmt executable and rerunning a self
        // update, this should install the rustup shim. Note that we don't run
        // `rustup` here but rather the rustup we've actually installed, this'll
        // help reproduce bugs related to having that file being opened by the
        // current process.
        fs::remove_file(&rustfmt_path).unwrap();
        let installed_rustup = config.cargodir.join("bin/rustup");
        config.expect_ok(&[installed_rustup.to_str().unwrap(), "self", "update"]);
        assert!(rustfmt_path.exists());
        assert!(utils::file_size(&rustfmt_path).unwrap() > 0);
    });
}

#[test]
fn update_installs_clippy_cargo_and() {
    update_setup(&|config, self_dist| {
        config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
        let version = env!("CARGO_PKG_VERSION");
        output_release_file(self_dist, "1", version);

        let cargo_clippy_path = config
            .cargodir
            .join(format!("bin/cargo-clippy{EXE_SUFFIX}"));
        assert!(cargo_clippy_path.exists());
    });
}

#[test]
fn install_with_components_and_targets() {
    clitools::test(Scenario::SimpleV2, &|config| {
        config.expect_ok(&[
            "rustup-init",
            "--default-toolchain",
            "nightly",
            "-y",
            "-c",
            "rls",
            "-t",
            clitools::CROSS_ARCH1,
            "--no-modify-path",
        ]);
        config.expect_stdout_ok(
            &["rustup", "target", "list"],
            &format!("{} (installed)", clitools::CROSS_ARCH1),
        );
        config.expect_stdout_ok(
            &["rustup", "component", "list"],
            &format!("rls-{} (installed)", this_host_triple()),
        );
    })
}

#[test]
fn install_minimal_profile() {
    clitools::test(Scenario::SimpleV2, &|config| {
        config.expect_ok(&[
            "rustup-init",
            "-y",
            "--profile",
            "minimal",
            "--no-modify-path",
        ]);

        config.expect_component_executable("rustup");
        config.expect_component_executable("rustc");
        config.expect_component_not_executable("cargo");
    });
}
