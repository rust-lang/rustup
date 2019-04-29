//! Test cases of the rustup command, using v2 manifests, mostly
//! derived from multirust/test-v2.sh

pub mod mock;

use crate::mock::clitools::{
    self, expect_err, expect_not_stdout_ok, expect_ok, expect_stderr_ok, expect_stdout_ok,
    set_current_dist_date, this_host_triple, Config, Scenario,
};
use std::fs;
use std::io::Write;
use tempdir::TempDir;

use rustup::dist::dist::TargetTriple;

macro_rules! for_host {
    ($s: expr) => {
        &format!($s, this_host_triple())
    };
}

pub fn setup(f: &dyn Fn(&mut Config)) {
    clitools::setup(Scenario::SimpleV2, f);
}

#[test]
fn rustc_no_default_toolchain() {
    setup(&|config| {
        expect_err(config, &["rustc"], "no default toolchain configured");
    });
}

#[test]
fn expected_bins_exist() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "1.3.0");
    });
}

#[test]
fn install_toolchain_from_channel() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        expect_ok(config, &["rustup", "default", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
    });
}

#[test]
fn install_toolchain_from_archive() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        expect_ok(config, &["rustup", "default", "nightly-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");
        expect_ok(config, &["rustup", "default", "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-1");
        expect_ok(config, &["rustup", "default", "stable-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-1");
    });
}

#[test]
fn install_toolchain_from_version() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "1.1.0"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
    });
}

#[test]
fn default_existing_toolchain() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stderr_ok(
            config,
            &["rustup", "default", "nightly"],
            for_host!("using existing install for 'nightly-{0}'"),
        );
    });
}

#[test]
fn update_channel() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
    });
}

#[test]
fn list_toolchains() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok(
            config,
            &["rustup", "update", "beta-2015-01-01", "--no-self-update"],
        );
        expect_stdout_ok(config, &["rustup", "toolchain", "list"], "nightly");
        expect_stdout_ok(config, &["rustup", "toolchain", "list"], "beta-2015-01-01");
    });
}

#[test]
fn list_toolchains_with_bogus_file() {
    // #520
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);

        let name = "bogus_regular_file.txt";
        let path = config.rustupdir.join("toolchains").join(name);
        rustup::utils::utils::write_file(name, &path, "").unwrap();
        expect_stdout_ok(config, &["rustup", "toolchain", "list"], "nightly");
        expect_not_stdout_ok(config, &["rustup", "toolchain", "list"], name);
    });
}

#[test]
fn list_toolchains_with_none() {
    setup(&|config| {
        expect_stdout_ok(
            config,
            &["rustup", "toolchain", "list"],
            "no installed toolchains",
        );
    });
}

#[test]
fn remove_toolchain() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "list"]);
        expect_stdout_ok(
            config,
            &["rustup", "toolchain", "list"],
            "no installed toolchains",
        );
    });
}

#[test]
fn add_remove_multiple_toolchains() {
    fn go(add: &str, rm: &str) {
        setup(&|config| {
            let tch1 = "beta";
            let tch2 = "nightly";

            expect_ok(
                config,
                &["rustup", "toolchain", add, tch1, tch2, "--no-self-update"],
            );
            expect_ok(config, &["rustup", "toolchain", "list"]);
            expect_stdout_ok(config, &["rustup", "toolchain", "list"], tch1);
            expect_stdout_ok(config, &["rustup", "toolchain", "list"], tch2);

            expect_ok(config, &["rustup", "toolchain", rm, tch1, tch2]);
            expect_ok(config, &["rustup", "toolchain", "list"]);
            expect_not_stdout_ok(config, &["rustup", "toolchain", "list"], tch1);
            expect_not_stdout_ok(config, &["rustup", "toolchain", "list"], tch2);
        });
    }

    for add in &["add", "update", "install"] {
        for rm in &["remove", "uninstall"] {
            go(add, rm);
        }
    }
}

#[test]
fn remove_default_toolchain_err_handling() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "nightly"]);
        expect_err(
            config,
            &["rustc"],
            for_host!("toolchain 'nightly-{0}' is not installed"),
        );
    });
}

#[test]
fn remove_override_toolchain_err_handling() {
    setup(&|config| {
        let tempdir = TempDir::new("rustup").unwrap();
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "default", "nightly"]);
            expect_ok(config, &["rustup", "override", "add", "beta"]);
            expect_ok(config, &["rustup", "toolchain", "remove", "beta"]);
            expect_stderr_ok(
                config,
                &["rustc", "--version"],
                "info: installing component",
            );
        });
    });
}

#[test]
fn file_override_toolchain_err_handling() {
    setup(&|config| {
        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        rustup::utils::raw::write_file(&toolchain_file, "beta").unwrap();
        expect_stderr_ok(
            config,
            &["rustc", "--version"],
            "info: installing component",
        );
    });
}

#[test]
fn plus_override_toolchain_err_handling() {
    setup(&|config| {
        expect_err(
            config,
            &["rustc", "+beta"],
            for_host!("toolchain 'beta-{0}' is not installed"),
        );
    });
}

#[test]
fn bad_sha_on_manifest() {
    setup(&|config| {
        // Corrupt the sha
        let sha_file = config.distdir.join("dist/channel-rust-nightly.toml.sha256");
        let sha_str = rustup::utils::raw::read_file(&sha_file).unwrap();
        let mut sha_bytes = sha_str.into_bytes();
        sha_bytes[..10].clone_from_slice(b"aaaaaaaaaa");
        let sha_str = String::from_utf8(sha_bytes).unwrap();
        rustup::utils::raw::write_file(&sha_file, &sha_str).unwrap();
        expect_ok(config, &["rustup", "default", "nightly"]);
    });
}

#[test]
fn bad_sha_on_installer() {
    setup(&|config| {
        // Since the v2 sha's are contained in the manifest, corrupt the installer
        let dir = config.distdir.join("dist/2015-01-02");
        for file in fs::read_dir(&dir).unwrap() {
            let file = file.unwrap();
            let path = file.path();
            let filename = path.to_string_lossy();
            if filename.ends_with(".tar.gz") || filename.ends_with(".tar.xz") {
                rustup::utils::raw::write_file(&path, "xxx").unwrap();
            }
        }
        expect_err(config, &["rustup", "default", "nightly"], "checksum failed");
    });
}

#[test]
fn install_override_toolchain_from_channel() {
    setup(&|config| {
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        expect_ok(config, &["rustup", "override", "add", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        expect_ok(config, &["rustup", "override", "add", "stable"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
    });
}

#[test]
fn install_override_toolchain_from_archive() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        expect_ok(config, &["rustup", "override", "add", "nightly-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");
        expect_ok(config, &["rustup", "override", "add", "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-1");
        expect_ok(config, &["rustup", "override", "add", "stable-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-1");
    });
}

#[test]
fn install_override_toolchain_from_version() {
    setup(&|config| {
        expect_ok(config, &["rustup", "override", "add", "1.1.0"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
    });
}

#[test]
fn override_overrides_default() {
    setup(&|config| {
        let tempdir = TempDir::new("rustup").unwrap();
        expect_ok(config, &["rustup", "default", "nightly"]);
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "beta"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        });
    });
}

#[test]
fn multiple_overrides() {
    setup(&|config| {
        let tempdir1 = TempDir::new("rustup").unwrap();
        let tempdir2 = TempDir::new("rustup").unwrap();

        expect_ok(config, &["rustup", "default", "nightly"]);
        config.change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "beta"]);
        });
        config.change_dir(tempdir2.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "stable"]);
        });

        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");

        config.change_dir(tempdir1.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        });
        config.change_dir(tempdir2.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
        });
    });
}

// #316
#[test]
#[cfg(windows)]
fn override_windows_root() {
    setup(&|config| {
        use std::path::{Component, PathBuf};

        let cwd = config.current_dir();
        let prefix = cwd.components().next().unwrap();
        let prefix = match prefix {
            Component::Prefix(p) => p,
            _ => panic!(),
        };

        // This value is probably "C:"
        // Really sketchy to be messing with C:\ in a test...
        let prefix = prefix.as_os_str().to_str().unwrap();
        let prefix = format!("{}\\", prefix);
        config.change_dir(&PathBuf::from(&prefix), &|| {
            expect_ok(config, &["rustup", "default", "stable"]);
            expect_ok(config, &["rustup", "override", "add", "nightly"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
            expect_ok(config, &["rustup", "override", "remove"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
        });
    });
}

#[test]
fn change_override() {
    setup(&|config| {
        let tempdir = TempDir::new("rustup").unwrap();
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "nightly"]);
            expect_ok(config, &["rustup", "override", "add", "beta"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
        });
    });
}

#[test]
fn remove_override_no_default() {
    setup(&|config| {
        let tempdir = TempDir::new("rustup").unwrap();
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "nightly"]);
            expect_ok(config, &["rustup", "override", "remove"]);
            expect_err(config, &["rustc"], "no default toolchain configured");
        });
    });
}

#[test]
fn remove_override_with_default() {
    setup(&|config| {
        let tempdir = TempDir::new("rustup").unwrap();
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "default", "nightly"]);
            expect_ok(config, &["rustup", "override", "add", "beta"]);
            expect_ok(config, &["rustup", "override", "remove"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        });
    });
}

#[test]
fn remove_override_with_multiple_overrides() {
    setup(&|config| {
        let tempdir1 = TempDir::new("rustup").unwrap();
        let tempdir2 = TempDir::new("rustup").unwrap();
        expect_ok(config, &["rustup", "default", "nightly"]);
        config.change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "beta"]);
        });
        config.change_dir(tempdir2.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "stable"]);
        });
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        config.change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "remove"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        });
        config.change_dir(tempdir2.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
        });
    });
}

#[test]
fn no_update_on_channel_when_date_has_not_changed() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "update", "nightly", "--no-self-update"],
            "unchanged",
        );
    });
}

#[test]
fn update_on_channel_when_date_has_changed() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
    });
}

#[test]
fn run_command() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok(config, &["rustup", "default", "beta"]);
        expect_stdout_ok(
            config,
            &["rustup", "run", "nightly", "rustc", "--version"],
            "hash-n-2",
        );
    });
}

#[test]
fn remove_toolchain_then_add_again() {
    // Issue brson/multirust #53
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "beta"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "beta"]);
        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_ok(config, &["rustc", "--version"]);
    });
}

#[test]
fn upgrade_v1_to_v2() {
    clitools::setup(Scenario::Full, &|config| {
        set_current_dist_date(config, "2015-01-01");
        // Delete the v2 manifest so the first day we install from the v1s
        fs::remove_file(config.distdir.join("dist/channel-rust-nightly.toml.sha256")).unwrap();
        expect_ok(config, &["rustup", "default", "nightly"]);
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
    });
}

#[test]
fn upgrade_v2_to_v1() {
    clitools::setup(Scenario::Full, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        set_current_dist_date(config, "2015-01-02");
        fs::remove_file(config.distdir.join("dist/channel-rust-nightly.toml.sha256")).unwrap();
        expect_err(
            config,
            &["rustup", "update", "nightly"],
            "the server unexpectedly provided an obsolete version of the distribution manifest",
        );
    });
}

#[test]
fn list_targets_no_toolchain() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "target", "list", "--toolchain=nightly"],
            for_host!("toolchain 'nightly-{0}' is not installed"),
        );
    });
}

#[test]
fn list_targets_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_err(
            config,
            &["rustup", "target", "list", "--toolchain=nightly"],
            for_host!("toolchain 'nightly-{0}' does not support components"),
        );
    });
}

#[test]
fn list_targets_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(
            config,
            &["rustup", "toolchain", "link", "default-from-path", &path],
        );
        expect_ok(config, &["rustup", "default", "default-from-path"]);
        expect_err(
            config,
            &["rustup", "target", "list"],
            "toolchain 'default-from-path' does not support components",
        );
    });
}

#[test]
fn list_targets() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustup", "target", "list"], clitools::CROSS_ARCH1);
        expect_stdout_ok(config, &["rustup", "target", "list"], clitools::CROSS_ARCH2);
    });
}

#[test]
fn list_installed_targets() {
    setup(&|config| {
        let trip = this_host_triple();

        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustup", "target", "list", "--installed"], &trip);
    });
}

#[test]
fn add_target() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            this_host_triple(),
            clitools::CROSS_ARCH1
        );
        assert!(config.rustupdir.join(path).exists());
    });
}

#[test]
fn add_target_no_toolchain() {
    setup(&|config| {
        expect_err(
            config,
            &[
                "rustup",
                "target",
                "add",
                clitools::CROSS_ARCH1,
                "--toolchain=nightly",
            ],
            for_host!("toolchain 'nightly-{0}' is not installed"),
        );
    });
}
#[test]
fn add_target_bogus() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(
            config,
            &["rustup", "target", "add", "bogus"],
            "does not contain component 'rust-std' for target 'bogus'",
        );
    });
}

#[test]
fn add_target_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_err(
            config,
            &[
                "rustup",
                "target",
                "add",
                clitools::CROSS_ARCH1,
                "--toolchain=nightly",
            ],
            for_host!("toolchain 'nightly-{0}' does not support components"),
        );
    });
}

#[test]
fn add_target_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(
            config,
            &["rustup", "toolchain", "link", "default-from-path", &path],
        );
        expect_ok(config, &["rustup", "default", "default-from-path"]);
        expect_err(
            config,
            &["rustup", "target", "add", clitools::CROSS_ARCH1],
            "toolchain 'default-from-path' does not support components",
        );
    });
}

#[test]
fn add_target_again() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        expect_stderr_ok(
            config,
            &["rustup", "target", "add", clitools::CROSS_ARCH1],
            &format!(
                "component 'rust-std' for target '{}' is up to date",
                clitools::CROSS_ARCH1
            ),
        );
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            this_host_triple(),
            clitools::CROSS_ARCH1
        );
        assert!(config.rustupdir.join(path).exists());
    });
}

#[test]
fn add_target_host() {
    setup(&|config| {
        let trip = TargetTriple::from_build();
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stderr_ok(config, &["rustup", "target", "add", &trip.to_string()],
                   for_host!("component 'rust-std' for target '{0}' was automatically added because it is required for toolchain 'nightly-{0}'"));
    });
}

#[test]
fn remove_target() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        expect_ok(
            config,
            &["rustup", "target", "remove", clitools::CROSS_ARCH1],
        );
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            this_host_triple(),
            clitools::CROSS_ARCH1
        );
        assert!(!config.rustupdir.join(path).exists());
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib",
            this_host_triple(),
            clitools::CROSS_ARCH1
        );
        assert!(!config.rustupdir.join(path).exists());
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}",
            this_host_triple(),
            clitools::CROSS_ARCH1
        );
        assert!(!config.rustupdir.join(path).exists());
    });
}

#[test]
fn remove_target_not_installed() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(
            config,
            &["rustup", "target", "remove", clitools::CROSS_ARCH1],
            &format!(
                "toolchain 'nightly-{}' does not contain component 'rust-std' for target '{}'",
                this_host_triple(),
                clitools::CROSS_ARCH1
            ),
        );
    });
}

#[test]
fn remove_target_no_toolchain() {
    setup(&|config| {
        expect_err(
            config,
            &[
                "rustup",
                "target",
                "remove",
                clitools::CROSS_ARCH1,
                "--toolchain=nightly",
            ],
            for_host!("toolchain 'nightly-{0}' is not installed"),
        );
    });
}

#[test]
fn remove_target_bogus() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(
            config,
            &["rustup", "target", "remove", "bogus"],
            "does not contain component 'rust-std' for target 'bogus'",
        );
    });
}

#[test]
fn remove_target_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(
            config,
            &[
                "rustup",
                "target",
                "remove",
                clitools::CROSS_ARCH1,
                "--toolchain=nightly",
            ],
            for_host!("toolchain 'nightly-{0}' does not support components"),
        );
    });
}

#[test]
fn remove_target_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(
            config,
            &["rustup", "toolchain", "link", "default-from-path", &path],
        );
        expect_ok(config, &["rustup", "default", "default-from-path"]);
        expect_err(
            config,
            &["rustup", "target", "remove", clitools::CROSS_ARCH1],
            "toolchain 'default-from-path' does not support components",
        );
    });
}

#[test]
fn remove_target_again() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        expect_ok(
            config,
            &["rustup", "target", "remove", clitools::CROSS_ARCH1],
        );
        expect_err(
            config,
            &["rustup", "target", "remove", clitools::CROSS_ARCH1],
            &format!(
                "toolchain 'nightly-{}' does not contain component 'rust-std' for target '{}'",
                this_host_triple(),
                clitools::CROSS_ARCH1
            ),
        );
    });
}

#[test]
fn remove_target_host() {
    setup(&|config| {
        let trip = TargetTriple::from_build();
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(config, &["rustup", "target", "remove", &trip.to_string()],
                   for_host!("component 'rust-std' for target '{0}' is required for toolchain 'nightly-{0}' and cannot be removed"));
    });
}

#[test]
// Issue #304
fn remove_target_missing_update_hash() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);

        let file_name = format!("nightly-{}", this_host_triple());
        fs::remove_file(config.rustupdir.join("update-hashes").join(file_name)).unwrap();

        expect_ok(config, &["rustup", "toolchain", "remove", "nightly"]);
    });
}

// Issue #1777
#[test]
fn warn_about_and_remove_stray_hash() {
    setup(&|config| {
        let mut hash_path = config.rustupdir.join("update-hashes");
        fs::create_dir_all(&hash_path).expect("Unable to make the update-hashes directory");

        hash_path.push(for_host!("nightly-{}"));

        let mut file = fs::File::create(&hash_path).expect("Unable to open update-hash file");
        file.write_all(b"LEGITHASH")
            .expect("Unable to write update-hash");
        drop(file);

        expect_stderr_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
            &format!(
                "removing stray hash found at '{}' in order to continue",
                hash_path.display()
            ),
        );
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "1.3.0");
    });
}

fn make_component_unavailable(config: &Config, name: &str, target: &TargetTriple) {
    use crate::mock::dist::create_hash;
    use rustup::dist::manifest::Manifest;

    let manifest_path = config.distdir.join("dist/channel-rust-nightly.toml");
    let manifest_str = rustup::utils::raw::read_file(&manifest_path).unwrap();
    let mut manifest = Manifest::parse(&manifest_str).unwrap();
    {
        let std_pkg = manifest.packages.get_mut(name).unwrap();
        let target_pkg = std_pkg.targets.get_mut(target).unwrap();
        target_pkg.bins = None;
    }
    let manifest_str = manifest.stringify();
    rustup::utils::raw::write_file(&manifest_path, &manifest_str).unwrap();

    // Have to update the hash too
    let hash_path = manifest_path.with_extension("toml.sha256");
    println!("{}", hash_path.display());
    create_hash(&manifest_path, &hash_path);
}

#[test]
fn update_unavailable_std() {
    setup(&|config| {
        let trip = TargetTriple::from_build();
        make_component_unavailable(config, "rust-std", &trip);
        expect_err(
            config,
            &["rustup", "update", "nightly", "--no-self-update"],
            &format!(
                "component 'rust-std' for target '{}' is unavailable for download for channel 'nightly'",
                trip,
            ),
        );
    });
}

#[test]
fn update_unavailable_force() {
    setup(&|config| {
        let trip = TargetTriple::from_build();
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok(
            config,
            &[
                "rustup",
                "component",
                "add",
                "rls",
                "--toolchain",
                "nightly",
            ],
        );
        make_component_unavailable(config, "rls-preview", &trip);
        expect_err(
            config,
            &["rustup", "update", "nightly", "--no-self-update"],
            &format!(
                "component 'rls' for target '{}' is unavailable for download for channel 'nightly'",
                trip,
            ),
        );
        expect_ok(
            config,
            &["rustup", "update", "nightly", "--force", "--no-self-update"],
        );
    });
}
