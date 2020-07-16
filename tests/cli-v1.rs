//! Test cases of the rustup command, using v1 manifests, mostly
//! derived from multirust/test-v2.sh

pub mod mock;

use std::fs;

use rustup::for_host;

use crate::mock::clitools::{
    self, expect_err, expect_ok, expect_stderr_ok, expect_stdout_ok, set_current_dist_date, Config,
    Scenario,
};

pub fn setup(f: &dyn Fn(&mut Config)) {
    clitools::setup(Scenario::SimpleV1, f);
}

#[test]
fn rustc_no_default_toolchain() {
    setup(&|config| {
        expect_err(
            config,
            &["rustc"],
            "no override and no default toolchain set",
        );
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
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");
        expect_ok(config, &["rustup", "default", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-beta-1.2.0");
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-stable-1.1.0");
    });
}

#[test]
fn install_toolchain_from_archive() {
    clitools::setup(Scenario::ArchivesV1, &|config| {
        expect_ok(config, &["rustup", "default", "nightly-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");
        expect_ok(config, &["rustup", "default", "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-beta-1.1.0");
        expect_ok(config, &["rustup", "default", "stable-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-stable-1.0.0");
    });
}

#[test]
fn install_toolchain_from_version() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "1.1.0"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-stable-1.1.0");
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
    clitools::setup(Scenario::ArchivesV1, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");
    });
}

#[test]
fn list_toolchains() {
    clitools::setup(Scenario::ArchivesV1, &|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok(
            config,
            &["rustup", "update", "beta-2015-01-01", "--no-self-update"],
        );
        expect_stdout_ok(config, &["rustup", "toolchain", "list"], "nightly");
        expect_stdout_ok(
            config,
            &["rustup", "toolchain", "list", "-v"],
            "(default)\t",
        );
        #[cfg(windows)]
        expect_stdout_ok(
            config,
            &["rustup", "toolchain", "list", "-v"],
            for_host!("\\toolchains\\nightly-{}"),
        );
        #[cfg(not(windows))]
        expect_stdout_ok(
            config,
            &["rustup", "toolchain", "list", "-v"],
            for_host!("/toolchains/nightly-{}"),
        );
        expect_stdout_ok(config, &["rustup", "toolchain", "list"], "beta-2015-01-01");
        #[cfg(windows)]
        expect_stdout_ok(
            config,
            &["rustup", "toolchain", "list", "-v"],
            "\\toolchains\\beta-2015-01-01",
        );
        #[cfg(not(windows))]
        expect_stdout_ok(
            config,
            &["rustup", "toolchain", "list", "-v"],
            "/toolchains/beta-2015-01-01",
        );
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
fn remove_default_toolchain_autoinstalls() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "nightly"]);
        expect_stderr_ok(
            config,
            &["rustc", "--version"],
            "info: installing component",
        );
    });
}

#[test]
fn remove_override_toolchain_err_handling() {
    setup(&|config| {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
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
fn bad_sha_on_manifest() {
    setup(&|config| {
        let sha_file = config.distdir.join("dist/channel-rust-nightly.sha256");
        let sha_str = fs::read_to_string(&sha_file).unwrap();
        let mut sha_bytes = sha_str.into_bytes();
        sha_bytes[..10].clone_from_slice(b"aaaaaaaaaa");
        let sha_str = String::from_utf8(sha_bytes).unwrap();
        rustup::utils::raw::write_file(&sha_file, &sha_str).unwrap();
        expect_err(config, &["rustup", "default", "nightly"], "checksum failed");
    });
}

#[test]
fn bad_sha_on_installer() {
    setup(&|config| {
        let dir = config.distdir.join("dist");
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
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");
        expect_ok(config, &["rustup", "override", "add", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-beta-1.2.0");
        expect_ok(config, &["rustup", "override", "add", "stable"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-stable-1.1.0");
    });
}

#[test]
fn install_override_toolchain_from_archive() {
    clitools::setup(Scenario::ArchivesV1, &|config| {
        expect_ok(config, &["rustup", "override", "add", "nightly-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");
        expect_ok(config, &["rustup", "override", "add", "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-beta-1.1.0");
        expect_ok(config, &["rustup", "override", "add", "stable-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-stable-1.0.0");
    });
}

#[test]
fn install_override_toolchain_from_version() {
    setup(&|config| {
        expect_ok(config, &["rustup", "override", "add", "1.1.0"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-stable-1.1.0");
    });
}

#[test]
fn override_overrides_default() {
    setup(&|config| {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        expect_ok(config, &["rustup", "default", "nightly"]);
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "beta"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-beta-1.2.0");
        });
    });
}

#[test]
fn multiple_overrides() {
    setup(&|config| {
        let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

        expect_ok(config, &["rustup", "default", "nightly"]);
        config.change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "beta"]);
        });
        config.change_dir(tempdir2.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "stable"]);
        });

        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");

        config.change_dir(tempdir1.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-beta-1.2.0");
        });
        config.change_dir(tempdir2.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-stable-1.1.0");
        });
    });
}

#[test]
fn change_override() {
    setup(&|config| {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "nightly"]);
            expect_ok(config, &["rustup", "override", "add", "beta"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-beta-1.2.0");
        });
    });
}

#[test]
fn remove_override_no_default() {
    setup(&|config| {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "nightly"]);
            expect_ok(config, &["rustup", "override", "remove"]);
            expect_err(
                config,
                &["rustc"],
                "no override and no default toolchain set",
            );
        });
    });
}

#[test]
fn remove_override_with_default() {
    setup(&|config| {
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        config.change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "default", "nightly"]);
            expect_ok(config, &["rustup", "override", "add", "beta"]);
            expect_ok(config, &["rustup", "override", "remove"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");
        });
    });
}

#[test]
fn remove_override_with_multiple_overrides() {
    setup(&|config| {
        let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        expect_ok(config, &["rustup", "default", "nightly"]);
        config.change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "beta"]);
        });
        config.change_dir(tempdir2.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "stable"]);
        });
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");
        config.change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "remove"]);
            expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");
        });
        config.change_dir(tempdir2.path(), &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-stable-1.1.0");
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
    clitools::setup(Scenario::ArchivesV1, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");
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
            "hash-nightly-2",
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
