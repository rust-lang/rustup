//! Test cases of the multirust command, using v2 manifests, mostly
//! derived from multirust/test-v2.sh

extern crate multirust_dist;
extern crate multirust_utils;
extern crate multirust_mock;
extern crate tempdir;

use std::fs;
use tempdir::TempDir;
use multirust_mock::clitools::{self, Config, Scenario,
                               this_host_triple,
                               expect_ok, expect_stdout_ok, expect_err,
                               expect_stderr_ok, set_current_dist_date,
                               change_dir, run};

pub fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, f);
}

#[test]
fn rustc_no_default_toolchain() {
    setup(&|config| {
        expect_err(config, &["rustc"],
                           "no default toolchain configured");
    });
}

#[test]
fn show_default_no_default_toolchain() {
    setup(&|config| {
        expect_stdout_ok(config, &["multirust", "show-default"],
                         "no default toolchain configured");
    });
}

#[test]
fn default_toolchain() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_stdout_ok(config, &["multirust", "show-default"],
                         "default toolchain: nightly");
    });
}

#[test]
fn expected_bins_exist() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "1.3.0");
    });
}

#[test]
fn install_toolchain_from_channel() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        expect_ok(config, &["multirust", "default", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        expect_ok(config, &["multirust", "default", "stable"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
    });
}

#[test]
fn install_toolchain_from_archive() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        expect_ok(config, &["multirust", "default" , "nightly-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");
        expect_ok(config, &["multirust", "default" , "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-1");
        expect_ok(config, &["multirust", "default" , "stable-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-1");
    });
}

#[test]
fn install_toolchain_from_version() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default" , "1.1.0"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
    });
}

#[test]
fn default_existing_toolchain() {
    setup(&|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_stderr_ok(config, &["multirust", "default", "nightly"],
                         "using existing install for 'nightly'");
    });
}

#[test]
fn update_channel() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn list_toolchains() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_ok(config, &["multirust", "update", "beta-2015-01-01"]);
        expect_stdout_ok(config, &["multirust", "list-toolchains"],
                         "nightly");
        expect_stdout_ok(config, &["multirust", "list-toolchains"],
                         "beta-2015-01-01");
    });
}

#[test]
fn list_toolchains_with_none() {
    setup(&|config| {
        expect_stdout_ok(config, &["multirust", "list-toolchains"],
                         "no installed toolchains");
    });
}

#[test]
fn remove_toolchain() {
    setup(&|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_ok(config, &["multirust", "remove-toolchain", "nightly"]);
        expect_ok(config, &["multirust", "list-toolchains"]);
        expect_stdout_ok(config, &["multirust", "list-toolchains"],
                         "no installed toolchains");
    });
}

#[test]
fn remove_default_toolchain_error_handling() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_ok(config, &["multirust", "remove-toolchain", "nightly"]);
        expect_err(config, &["rustc"],
                           "toolchain 'nightly' is not installed");
    });
}

#[test]
fn remove_override_toolchain_error_handling() {
    setup(&|config| {
        let tempdir = TempDir::new("multirust").unwrap();
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["multirust", "default", "nightly"]);
            expect_ok(config, &["multirust", "override", "beta"]);
            expect_ok(config, &["multirust", "remove-toolchain", "beta"]);
            expect_err(config, &["rustc"],
                               "toolchain 'beta' is not installed");
        });
    });
}

#[test]
fn bad_sha_on_manifest() {
    setup(&|config| {
        // Corrupt the sha
        let sha_file = config.distdir.path().join("dist/channel-rust-nightly.toml.sha256");
        let sha_str = multirust_utils::raw::read_file(&sha_file).unwrap();
        let mut sha_bytes = sha_str.into_bytes();
        &mut sha_bytes[..10].clone_from_slice(b"aaaaaaaaaa");
        let sha_str = String::from_utf8(sha_bytes).unwrap();
        multirust_utils::raw::write_file(&sha_file, &sha_str).unwrap();
        expect_err(config, &["multirust", "default", "nightly"],
                   "checksum failed");
    });
}

#[test]
fn bad_sha_on_installer() {
    setup(&|config| {
        // Since the v2 sha's are contained in the manifest, corrupt the installer
        let dir = config.distdir.path().join("dist/2015-01-02");
        for file in fs::read_dir(&dir).unwrap() {
            let file = file.unwrap();
            if file.path().to_string_lossy().ends_with(".tar.gz") {
                multirust_utils::raw::write_file(&file.path(), "xxx").unwrap();
            }
        }
        expect_err(config, &["multirust", "default", "nightly"],
                   "checksum failed");
    });
}

#[test]
fn install_override_toolchain_from_channel() {
    setup(&|config| {
        expect_ok(config, &["multirust", "override", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
        expect_ok(config, &["multirust", "override", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-b-2");
        expect_ok(config, &["multirust", "override", "stable"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-2");
    });
}

#[test]
fn install_override_toolchain_from_archive() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        expect_ok(config, &["multirust", "override", "nightly-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        expect_ok(config, &["multirust", "override", "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-b-1");
        expect_ok(config, &["multirust", "override", "stable-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-1");
    });
}

#[test]
fn install_override_toolchain_from_version() {
    setup(&|config| {
        expect_ok(config, &["multirust", "override", "1.1.0"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-2");
    });
}

#[test]
fn override_overrides_default() {
    setup(&|config| {
        let tempdir = TempDir::new("multirust").unwrap();
        expect_ok(config, &["multirust", "default" , "nightly"]);
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["multirust", "override" , "beta"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        });
    });
}

#[test]
fn multiple_overrides() {
    setup(&|config| {
        let tempdir1 = TempDir::new("multirust").unwrap();
        let tempdir2 = TempDir::new("multirust").unwrap();

        expect_ok(config, &["multirust", "default", "nightly"]);
        change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["multirust", "override", "beta"]);
        });
        change_dir(tempdir2.path(), &|| {
            expect_ok(config, &["multirust", "override", "stable"]);
        });

        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");

        change_dir(tempdir1.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        });
        change_dir(tempdir2.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
        });
    });
}

#[test]
fn change_override() {
    setup(&|config| {
        let tempdir = TempDir::new("multirust").unwrap();
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["multirust", "override", "nightly"]);
            expect_ok(config, &["multirust", "override", "beta"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        });
    });
}

#[test]
fn show_override() {
    setup(&|config| {
        let tempdir = TempDir::new("multirust").unwrap();
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["multirust", "override", "nightly"]);

            let expected_override_dir = fs::canonicalize(tempdir.path()).unwrap();;
            let expected_toolchain_dir = config.homedir.path().join("toolchains").join("nightly");

            expect_stdout_ok(config, &["multirust", "show-override"],
                             "override toolchain: nightly");
            expect_stdout_ok(config, &["multirust", "show-override"],
                             &format!("override reason: directory override for '{}'",
                                      expected_override_dir.to_string_lossy()));
            expect_stdout_ok(config, &["multirust", "show-override"],
                             &format!("override location: {}",
                                      expected_toolchain_dir.to_string_lossy()));
            expect_stdout_ok(config, &["multirust", "show-override"],
                             "hash-n-2");
        });
    });
}

#[test]
fn show_override_no_default() {
    setup(&|config| {
        expect_stdout_ok(config, &["multirust", "show-override"],
                         "no override");
    });
}

#[test]
fn show_override_show_default() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_stdout_ok(config, &["multirust", "show-override"],
                         "no override");
        expect_stdout_ok(config, &["multirust", "show-override"],
                         "default toolchain: nightly");
    });
}

#[test]
fn show_override_from_multirust_toolchain_env_var() {
    setup(&|config| {
        let tempdir = TempDir::new("multirusT").unwrap();
        change_dir(tempdir.path(), &|| {

            let expected_toolchain_dir = config.homedir.path().join("toolchains").join("beta");

            expect_ok(config, &["multirust", "update", "beta"]);
            expect_ok(config, &["multirust", "override", "nightly"]);
            // change_dir has a lock so it's ok to futz the environment
            let out = run(config, "multirust", &["show-override"],
                          &[("MULTIRUST_TOOLCHAIN", "beta")]);
            assert!(out.ok);
            assert!(out.stdout.contains("override toolchain: beta"));
            assert!(out.stdout.contains("override reason: environment override"));
            assert!(out.stdout.contains(&format!("override location: {}",
                                                 expected_toolchain_dir.to_string_lossy())));
            assert!(out.stdout.contains("override toolchain: beta"));
        });
    });
}

#[test]
fn remove_override_no_default() {
    setup(&|config| {
        let tempdir = TempDir::new("multirust").unwrap();
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["multirust", "override", "nightly"]);
            expect_ok(config, &["multirust", "remove-override"]);
            expect_err(config, &["rustc"],
                               "no default toolchain configured");
        });
    });
}

#[test]
fn remove_override_with_default() {
    setup(&|config| {
        let tempdir = TempDir::new("multirust").unwrap();
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["multirust", "default", "nightly"]);
            expect_ok(config, &["multirust", "override", "beta"]);
            expect_ok(config, &["multirust", "remove-override"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        });
    });
}

#[test]
fn remove_override_with_multiple_overrides() {
    setup(&|config| {
        let tempdir1 = TempDir::new("multirust").unwrap();
        let tempdir2 = TempDir::new("multirust").unwrap();
        expect_ok(config, &["multirust", "default", "nightly"]);
        change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["multirust", "override", "beta"]);
        });
        change_dir(tempdir2.path(), &|| {
            expect_ok(config, &["multirust", "override", "stable"]);
        });
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["multirust", "remove-override"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        });
        change_dir(tempdir2.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
        });
    });
}

#[test]
fn no_update_on_channel_when_date_has_not_changed() {
    setup(&|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_stderr_ok(config, &["multirust", "update", "nightly"],
                         "already up to date");
    });
}

#[test]
fn update_on_channel_when_date_has_changed() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn update_no_toolchain_means_update_all_toolchains() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["multirust", "update"]);

        expect_stderr_ok(config, &["multirust", "default", "nightly"],
                         "using existing");
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        expect_stderr_ok(config, &["multirust", "default", "beta"],
                         "using existing");
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-b-1");
        expect_stderr_ok(config, &["multirust", "default", "stable"],
                         "using existing");
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-1");

        set_current_dist_date(config, "2015-01-02");
        expect_stderr_ok(config, &["multirust", "update", "nightly"],
                         "updating existing");
        expect_ok(config, &["multirust", "update"]);

        expect_stderr_ok(config, &["multirust", "default", "nightly"],
                         "using existing");
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
        expect_stderr_ok(config, &["multirust", "default", "beta"],
                         "using existing");
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-b-2");
        expect_stderr_ok(config, &["multirust", "default", "stable"],
                         "using existing");
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-2");
    });
}

#[test]
fn run_command() {
    setup(&|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_ok(config, &["multirust", "default", "beta"]);
        expect_stdout_ok(config, &["multirust", "run", "nightly", "rustc" , "--version"],
                         "hash-n-2");
    });
}

#[test]
fn remove_toolchain_then_add_again() {
    // Issue brson/multirust #53
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "beta"]);
        expect_ok(config, &["multirust", "remove-toolchain", "beta"]);
        expect_ok(config, &["multirust", "update", "beta"]);
        expect_ok(config, &["rustc", "--version"]);
    });
}

#[test]
fn upgrade_v1_to_v2() {
    clitools::setup(Scenario::Full, &|config| {
        set_current_dist_date(config, "2015-01-01");
        // Delete the v2 manifest so the first day we install from the v1s
        fs::remove_file(config.distdir.path().join("dist/channel-rust-nightly.toml.sha256")).unwrap();
        expect_ok(config, &["multirust", "default", "nightly"]);
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn upgrade_v2_to_v1() {
    clitools::setup(Scenario::Full, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["multirust", "default", "nightly"]);
        set_current_dist_date(config, "2015-01-02");
        fs::remove_file(config.distdir.path().join("dist/channel-rust-nightly.toml.sha256")).unwrap();
        expect_err(config, &["multirust", "update", "nightly"],
                           "the server unexpectedly provided an obsolete version of the distribution manifest");
    });
}

#[test]
fn list_targets_no_toolchain() {
    setup(&|config| {
        expect_err(config, &["multirust", "list-targets", "nightly"],
                   "toolchain 'nightly' is not installed");
    });
}

#[test]
fn list_targets_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_err(config, &["multirust", "list-targets", "nightly"],
                   "toolchain 'nightly' does not support components");
    });
}

#[test]
fn list_targets_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--copy-local", &path]);
        expect_err(config, &["multirust", "list-targets", "default-from-path"],
                   "invalid custom toolchain name: 'default-from-path'");
    });
}

#[test]
fn list_targets() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_stdout_ok(config, &["multirust", "list-targets", "nightly"],
                         clitools::CROSS_ARCH1);
        expect_stdout_ok(config, &["multirust", "list-targets", "nightly"],
                         clitools::CROSS_ARCH2);
    });
}

#[test]
fn add_target() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_ok(config, &["multirust", "add-target", "nightly", clitools::CROSS_ARCH1]);
        let path = format!("toolchains/nightly/lib/rustlib/{}/lib/libstd.rlib",
                           clitools::CROSS_ARCH1);
        assert!(config.homedir.path().join(path).exists());
    });
}

#[test]
fn add_target_no_toolchain() {
    setup(&|config| {
        expect_err(config, &["multirust", "add-target", "nightly", clitools::CROSS_ARCH1],
                   "toolchain 'nightly' is not installed");
    });
}
#[test]
fn add_target_bogus() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_err(config, &["multirust", "add-target", "nightly", "bogus"],
                   "toolchain 'nightly' does not contain component 'rust-std' for target 'bogus'");
    });
}

#[test]
fn add_target_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_err(config, &["multirust", "add-target", "nightly", clitools::CROSS_ARCH1],
                   "toolchain 'nightly' does not support components");
    });
}

#[test]
fn add_target_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--copy-local", &path]);
        expect_err(config, &["multirust", "add-target", "default-from-path", clitools::CROSS_ARCH1],
                   "invalid custom toolchain name: 'default-from-path'");
    });
}

#[test]
fn add_target_again() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_ok(config, &["multirust", "add-target", "nightly", clitools::CROSS_ARCH1]);
        expect_stderr_ok(config, &["multirust", "add-target", "nightly", clitools::CROSS_ARCH1],
                         &format!("component 'rust-std' for target '{}' is up to date",
                                 clitools::CROSS_ARCH1));
        let path = format!("toolchains/nightly/lib/rustlib/{}/lib/libstd.rlib",
                           clitools::CROSS_ARCH1);
        assert!(config.homedir.path().join(path).exists());
    });
}

#[test]
fn add_target_host() {
    setup(&|config| {
        let trip = this_host_triple();
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_err(config, &["multirust", "add-target", "nightly", &trip],
                   &format!("component 'rust-std' for target '{}' is required for toolchain 'nightly' and cannot be re-added", trip));
    });
}

#[test]
fn remove_target() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_ok(config, &["multirust", "add-target", "nightly", clitools::CROSS_ARCH1]);
        expect_ok(config, &["multirust", "remove-target", "nightly", clitools::CROSS_ARCH1]);
        let path = format!("toolchains/nightly/lib/rustlib/{}/lib/libstd.rlib",
                           clitools::CROSS_ARCH1);
        assert!(!config.homedir.path().join(path).exists());
    });
}

#[test]
fn remove_target_not_installed() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_err(config, &["multirust", "remove-target", "nightly", clitools::CROSS_ARCH1],
                   &format!("toolchain 'nightly' does not contain component 'rust-std' for target '{}'",
                            clitools::CROSS_ARCH1));
    });
}

#[test]
fn remove_target_no_toolchain() {
    setup(&|config| {
        expect_err(config, &["multirust", "remove-target", "nightly", clitools::CROSS_ARCH1],
                   "toolchain 'nightly' is not installed");
    });
}

#[test]
fn remove_target_bogus() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_err(config, &["multirust", "remove-target", "nightly", "bogus"],
                   "toolchain 'nightly' does not contain component 'rust-std' for target 'bogus'");
    });
}

#[test]
fn remove_target_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_err(config, &["multirust", "remove-target", "nightly", clitools::CROSS_ARCH1],
                   "toolchain 'nightly' does not support components");
    });
}

#[test]
fn remove_target_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--copy-local", &path]);
        expect_err(config, &["multirust", "remove-target", "default-from-path", clitools::CROSS_ARCH1],
                   "invalid custom toolchain name: 'default-from-path'");
    });
}

#[test]
fn remove_target_again() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_ok(config, &["multirust", "add-target", "nightly", clitools::CROSS_ARCH1]);
        expect_ok(config, &["multirust", "remove-target", "nightly", clitools::CROSS_ARCH1]);
        expect_err(config, &["multirust", "remove-target", "nightly", clitools::CROSS_ARCH1],
                   &format!("toolchain 'nightly' does not contain component 'rust-std' for target '{}'",
                            clitools::CROSS_ARCH1));
    });
}

#[test]
fn remove_target_host() {
    setup(&|config| {
        let trip = this_host_triple();
        expect_ok(config, &["multirust", "default", "nightly"]);
        expect_err(config, &["multirust", "remove-target", "nightly", &trip],
                   &format!("component 'rust-std' for target '{}' is required for toolchain 'nightly' and cannot be removed", trip));
    });
}

fn make_component_unavailable(config: &Config, name: &str, target: &str) {
    use multirust_dist::manifest::Manifest;
    use multirust_mock::dist::create_hash;

    let ref manifest_path = config.distdir.path().join("dist/channel-rust-nightly.toml");
    let ref manifest_str = multirust_utils::raw::read_file(manifest_path).unwrap();
    let mut manifest = Manifest::parse(manifest_str).unwrap();
    {
        let mut std_pkg = manifest.packages.get_mut(name).unwrap();
        let mut target_pkg = std_pkg.targets.get_mut(target).unwrap();
        target_pkg.available = false;
    }
    let ref manifest_str = manifest.stringify();
    multirust_utils::raw::write_file(manifest_path, manifest_str).unwrap();

    // Have to update the hash too
    let ref hash_path = manifest_path.with_extension("toml.sha256");
    println!("{}", hash_path.display());
    create_hash(manifest_path, hash_path);
}

#[test]
fn update_unavailable_std() {
    setup(&|config| {
        let ref trip = this_host_triple();
        make_component_unavailable(config, "rust-std", trip);
        expect_err(config, &["multirust", "update", "nightly"],
                   &format!("component 'rust-std' for '{}' is unavailable for download", trip));
    });
}
