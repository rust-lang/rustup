//! Test cases of the multirust command that do not depend on the
//! dist server, mostly derived from multirust/test-v2.sh

extern crate rust_install;

use rust_install::mock::dist::ManifestVersion;
use rust_install::mock::clitools::{self, Config, expect_stdout_ok,
                                   expect_ok, expect_err, run};
use rust_install::utils;

pub fn setup(f: &Fn(&Config)) {
    clitools::setup(&[ManifestVersion::V2], f);
}

#[test]
fn smoke_test() {
    setup(&|config| {
        expect_ok(config, &["multirust", "--version"]);
    });
}

#[test]
fn no_colors_in_piped_error_output() {
    setup(&|config| {
        let out = run(config, "rustc", &[], &[]);
        assert!(!out.ok);
        assert!(!out.stderr.contains("\u{1b}"));
    });
}

#[test]
fn rustc_with_bad_multirust_toolchain_env_var() {
    setup(&|config| {
        let out = run(config, "rustc", &[], &[("MULTIRUST_TOOLCHAIN", "bogus")]);
        assert!(!out.ok);
        assert!(out.stderr.contains("toolchain 'bogus' is not installed"));
    });
}

#[test]
#[ignore]
fn install_toolchain_linking_from_path() {
}

#[test]
#[ignore]
fn install_toolchain_from_path() {
}

#[test]
#[ignore]
fn install_toolchain_linking_from_path_again() {
}

#[test]
#[ignore]
fn install_toolchain_from_path_again() {
}

#[test]
#[ignore]
fn install_toolchain_change_from_copy_to_link() {
}

#[test]
#[ignore]
fn install_toolchain_change_from_link_to_copy() {
}

#[test]
#[ignore]
fn install_toolchain_from_custom() {
}

#[test]
#[ignore]
fn install_override_toolchain_linking_from_path() {
}

#[test]
#[ignore]
fn install_override_toolchain_from_path() {
}

#[test]
#[ignore]
fn install_override_toolchain_linking_from_path_again() {
}

#[test]
#[ignore]
fn install_override_toolchain_from_path_again() {
}

#[test]
#[ignore]
fn install_override_toolchain_change_from_copy_to_link() {
}

#[test]
#[ignore]
fn install_override_toolchain_change_from_link_to_copy() {
}

#[test]
#[ignore]
fn install_override_toolchain_from_custom() {
}

#[test]
#[ignore]
fn custom_no_installer_specified() {
}

#[test]
#[ignore]
fn custom_invalid_names() {
}

#[test]
#[ignore]
fn custom_invalid_names_with_archive_dates() {
}

#[test]
#[ignore]
fn custom_local() {
}

#[test]
#[ignore]
fn custom_remote() {
}

#[test]
#[ignore]
fn custom_multiple_local() {
}

#[test]
#[ignore]
fn custom_multiple_remote() {
}

#[test]
#[ignore]
fn update_toolchain_linking_path() {
}

#[test]
#[ignore]
fn update_toolchain_from_path() {
}

#[test]
#[ignore]
fn update_toolchain_change_from_copy_to_link() {
}

#[test]
#[ignore]
fn update_toolchain_change_from_link_to_copy() {
}

#[test]
#[ignore]
fn custom_dir_invalid_name() {
}

#[test]
#[ignore]
fn custom_without_rustc() {
}

#[test]
fn running_with_v2_metadata() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        // Replace the metadata version
        utils::raw::write_file(&config.homedir.path().join("version"),
                               "2").unwrap();
        expect_err(config, &["multirust", "default", "nightly"],
                   "multirust's metadata is out of date. run multirust upgrade-data");
        expect_err(config, &["rustc", "--version"],
                   "multirust's metadata is out of date. run multirust upgrade-data");
    });
}

// The thing that changed in the version bump from 2 -> 12 was the
// toolchain format. Check that on the upgrade all the toolchains.
// are deleted.
#[test]
fn upgrade_v2_metadata_to_v12() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        // Replace the metadata version
        utils::raw::write_file(&config.homedir.path().join("version"),
                               "2").unwrap();
        expect_ok(config, &["multirust", "upgrade-data"]);
        expect_err(config, &["multirust", "show-default"],
                   "toolchain 'nightly' is not installed");
        expect_err(config, &["rustc", "--version"],
                   "toolchain 'nightly' is not installed");
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

