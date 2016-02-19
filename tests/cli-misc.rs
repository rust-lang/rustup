//! Test cases of the multirust command that do not depend on the
//! dist server, mostly derived from multirust/test-v2.sh

extern crate rust_install;

use rust_install::mock::dist::ManifestVersion;
use rust_install::mock::clitools::{self, Config,
                                   expect_ok, run};

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

