//! Test cases of the multirust command that do not depend on the
//! dist server, mostly derived from multirust/test-v2.sh

extern crate rust_install;
extern crate multirust_mock;

use multirust_mock::clitools::{self, Config, Scenario,
                               expect_stdout_ok, expect_stderr_ok,
                               expect_ok, expect_err, run,
                               this_host_triple};
use rust_install::utils;

pub fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, f);
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
fn install_toolchain_linking_from_path() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
    });
}

#[test]
fn install_toolchain_from_path() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
    });
}

#[test]
fn install_toolchain_linking_from_path_again() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn install_toolchain_from_path_again() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn install_toolchain_change_from_copy_to_link() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn install_toolchain_change_from_link_to_copy() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn install_toolchain_from_custom() {
    setup(&|config| {
        let trip = this_host_triple();
        let custom_installer = config.distdir.path().join(
            format!("dist/rust-nightly-{}.tar.gz", trip));
        let custom_installer = custom_installer.to_string_lossy();
        expect_ok(config, &["multirust", "default", "custom",
                            "--installer", &custom_installer]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn install_toolchain_from_custom_wrong_extension() {
    setup(&|config| {
        let trip = this_host_triple();
        let custom_installer = config.distdir.path().join(
            format!("dist/rust-nightly-{}.msi", trip));
        let custom_installer = custom_installer.to_string_lossy();
        expect_err(config, &["multirust", "default", "custom",
                             "--installer", &custom_installer],
                   "invalid extension for installer: 'msi'");
    });
}

#[test]
fn install_override_toolchain_linking_from_path() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "override", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
    });
}

#[test]
fn install_override_toolchain_from_path() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "override", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
    });
}

#[test]
fn install_override_toolchain_linking_from_path_again() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "override", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "override", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn install_override_toolchain_from_path_again() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "override", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "override", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn install_override_toolchain_change_from_copy_to_link() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn install_override_toolchain_change_from_link_to_copy() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "default", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn install_override_toolchain_from_custom() {
    setup(&|config| {
        let trip = this_host_triple();
        let custom_installer = config.distdir.path().join(
            format!("dist/rust-nightly-{}.tar.gz", trip));
        let custom_installer = custom_installer.to_string_lossy();
        expect_ok(config, &["multirust", "override", "custom",
                            "--installer", &custom_installer]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn update_toolchain_linking_from_path() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--link-local", &path]);
        expect_ok(config, &["multirust", "default", "default-from-path"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
    });
}

#[test]
fn update_toolchain_from_path() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--copy-local", &path]);
        expect_ok(config, &["multirust", "default", "default-from-path"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
    });
}

#[test]
fn update_toolchain_linking_from_path_again() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--link-local", &path]);
        expect_ok(config, &["multirust", "default", "default-from-path"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn update_toolchain_from_path_again() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--copy-local", &path]);
        expect_ok(config, &["multirust", "default", "default-from-path"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn update_toolchain_change_from_copy_to_link() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--copy-local", &path]);
        expect_ok(config, &["multirust", "default", "default-from-path"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--link-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn update_toolchain_change_from_link_to_copy() {
    setup(&|config| {
        let path = config.customdir.path().join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--link-local", &path]);
        expect_ok(config, &["multirust", "default", "default-from-path"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
        let path = config.customdir.path().join("custom-2");
        let path = path.to_string_lossy();
        expect_ok(config, &["multirust", "update", "default-from-path",
                            "--copy-local", &path]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-2");
    });
}

#[test]
fn custom_invalid_names() {
    setup(&|config| {
        expect_err(config, &["multirust", "update", "nightly",
                             "--installer", "foo"],
                   "invalid custom toolchain name: 'nightly'");
        expect_err(config, &["multirust", "update", "beta",
                             "--installer", "foo"],
                   "invalid custom toolchain name: 'beta'");
        expect_err(config, &["multirust", "update", "stable",
                             "--installer", "foo"],
                   "invalid custom toolchain name: 'stable'");
    });
}

#[test]
fn custom_invalid_names_with_archive_dates() {
    setup(&|config| {
        expect_err(config, &["multirust", "update", "nightly-2015-01-01",
                             "--installer", "foo"],
                   "invalid custom toolchain name: 'nightly-2015-01-01'");
        expect_err(config, &["multirust", "update", "beta-2015-01-01",
                             "--installer", "foo"],
                   "invalid custom toolchain name: 'beta-2015-01-01'");
        expect_err(config, &["multirust", "update", "stable-2015-01-01",
                             "--installer", "foo"],
                   "invalid custom toolchain name: 'stable-2015-01-01'");
    });
}

#[test]
fn invalid_names_with_link_local() {
    setup(&|config| {
        expect_err(config, &["multirust", "update", "nightly",
                             "--link-local", "foo"],
                   "invalid custom toolchain name: 'nightly'");
        expect_err(config, &["multirust", "update", "nightly-2015-01-01",
                             "--link-local", "foo"],
                   "invalid custom toolchain name: 'nightly-2015-01-01'");
    });
}

#[test]
fn invalid_names_with_copy_local() {
    setup(&|config| {
        expect_err(config, &["multirust", "update", "nightly",
                             "--copy-local", "foo"],
                   "invalid custom toolchain name: 'nightly'");
        expect_err(config, &["multirust", "update", "nightly-2015-01-01",
                             "--copy-local", "foo"],
                   "invalid custom toolchain name: 'nightly-2015-01-01'");
    });
}

#[test]
fn custom_remote_url() {
    setup(&|config| {
        let trip = this_host_triple();
        let custom_installer = config.distdir.path().join(
            format!("dist/rust-nightly-{}.tar.gz", trip));
        let custom_installer = custom_installer.to_string_lossy();
        let custom_installer = format!("file://{}", custom_installer);
        expect_ok(config, &["multirust", "default", "custom",
                            "--installer", &custom_installer]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn custom_multiple_local_path() {
    clitools::setup(Scenario::Full, &|config| {
        let trip = this_host_triple();
        let custom_installer1 = config.distdir.path().join(
            format!("dist/2015-01-01/rustc-nightly-{}.tar.gz", trip));
        let custom_installer1 = custom_installer1.to_string_lossy();
        let custom_installer2 = config.distdir.path().join(
            format!("dist/2015-01-01/cargo-nightly-{}.tar.gz", trip));
        let custom_installer2 = custom_installer2.to_string_lossy();
        expect_ok(config, &["multirust", "default", "custom",
                            "--installer", &custom_installer1,
                            "--installer", &custom_installer2]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        expect_stdout_ok(config, &["cargo", "--version"],
                         "hash-n-1");
    });
}

#[test]
fn custom_multiple_remote_url() {
    clitools::setup(Scenario::Full, &|config| {
        let trip = this_host_triple();
        let custom_installer1 = config.distdir.path().join(
            format!("dist/2015-01-01/rustc-nightly-{}.tar.gz", trip));
        let custom_installer1 = custom_installer1.to_string_lossy();
        let custom_installer1 = format!("file://{}", custom_installer1);
        let custom_installer2 = config.distdir.path().join(
            format!("dist/2015-01-01/cargo-nightly-{}.tar.gz", trip));
        let custom_installer2 = custom_installer2.to_string_lossy();
        let custom_installer2 = format!("file://{}", custom_installer2);
        expect_ok(config, &["multirust", "default", "custom",
                            "--installer", &custom_installer1,
                            "--installer", &custom_installer2]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        expect_stdout_ok(config, &["cargo", "--version"],
                         "hash-n-1");
    });
}

#[test]
#[ignore]
fn delete_data() {
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
        expect_stderr_ok(config, &["multirust", "upgrade-data"],
                         "warning: this upgrade will remove all existing toolchains. you will need to reinstall them");
        expect_err(config, &["multirust", "show-default"],
                   "toolchain 'nightly' is not installed");
        expect_err(config, &["rustc", "--version"],
                   "toolchain 'nightly' is not installed");
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

