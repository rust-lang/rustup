//! Test cases of the rustup command, using v1 manifests, mostly
//! derived from multirust/test-v2.sh

use std::fs;

use rustup::for_host;
use rustup::test::mock::clitools::{set_current_dist_date, CliTestContext, Scenario};

#[test]
fn rustc_no_default_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_err(
        &["rustc"],
        "rustup could not choose a version of rustc to run",
    );
}

#[test]
fn expected_bins_exist() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_stdout_ok(&["rustc", "--version"], "1.3.0");
}

#[test]
fn install_toolchain_from_channel() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    cx.config.expect_ok(&["rustup", "default", "beta"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
    cx.config.expect_ok(&["rustup", "default", "stable"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
}

#[test]
fn install_toolchain_from_archive() {
    let mut cx = CliTestContext::from(Scenario::ArchivesV1);
    cx.config
        .expect_ok(&["rustup", "default", "nightly-2015-01-01"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
    cx.config
        .expect_ok(&["rustup", "default", "beta-2015-01-01"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.1.0");
    cx.config
        .expect_ok(&["rustup", "default", "stable-2015-01-01"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.0.0");
}

#[test]
fn install_toolchain_from_version() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "default", "1.1.0"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
}

#[test]
fn default_existing_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config.expect_stderr_ok(
        &["rustup", "default", "nightly"],
        for_host!("using existing install for 'nightly-{0}'"),
    );
}

#[test]
fn update_channel() {
    let mut cx = CliTestContext::from(Scenario::ArchivesV1);
    set_current_dist_date(&cx.config, "2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
    set_current_dist_date(&cx.config, "2015-01-02");
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
}

#[test]
fn list_toolchains() {
    let mut cx = CliTestContext::from(Scenario::ArchivesV1);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "update", "beta-2015-01-01"]);
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "nightly");
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list", "-v"], "(active, default) ");
    #[cfg(windows)]
    cx.config.expect_stdout_ok(
        &["rustup", "toolchain", "list", "-v"],
        for_host!(r"\toolchains\nightly-{}"),
    );
    #[cfg(not(windows))]
    cx.config.expect_stdout_ok(
        &["rustup", "toolchain", "list", "-v"],
        for_host!("/toolchains/nightly-{}"),
    );
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "beta-2015-01-01");
    #[cfg(windows)]
    cx.config.expect_stdout_ok(
        &["rustup", "toolchain", "list", "-v"],
        r"\toolchains\beta-2015-01-01",
    );
    #[cfg(not(windows))]
    cx.config.expect_stdout_ok(
        &["rustup", "toolchain", "list", "-v"],
        "/toolchains/beta-2015-01-01",
    );
}

#[test]
fn list_toolchains_with_none() {
    let cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "no installed toolchains");
}

#[test]
fn remove_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "nightly"]);
    cx.config.expect_ok(&["rustup", "toolchain", "list"]);
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "no installed toolchains");
}

#[test]
fn remove_default_toolchain_autoinstalls() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "nightly"]);
    cx.config
        .expect_stderr_ok(&["rustc", "--version"], "info: installing component");
}

#[test]
fn remove_override_toolchain_err_handling() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.change_dir(tempdir.path(), &|config| {
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_ok(&["rustup", "override", "add", "beta"]);
        config.expect_ok(&["rustup", "toolchain", "remove", "beta"]);
        config.expect_stderr_ok(&["rustc", "--version"], "info: installing component");
    });
}

#[test]
fn bad_sha_on_manifest() {
    let cx = CliTestContext::from(Scenario::SimpleV1);
    let sha_file = cx
        .config
        .distdir
        .as_ref()
        .unwrap()
        .join("dist/channel-rust-nightly.sha256");
    let sha_str = fs::read_to_string(&sha_file).unwrap();
    let mut sha_bytes = sha_str.into_bytes();
    sha_bytes[..10].clone_from_slice(b"aaaaaaaaaa");
    let sha_str = String::from_utf8(sha_bytes).unwrap();
    rustup::utils::raw::write_file(&sha_file, &sha_str).unwrap();
    cx.config
        .expect_err(&["rustup", "default", "nightly"], "checksum failed");
}

#[test]
fn bad_sha_on_installer() {
    let cx = CliTestContext::from(Scenario::SimpleV1);
    let dir = cx.config.distdir.as_ref().unwrap().join("dist");
    for file in fs::read_dir(dir).unwrap() {
        let file = file.unwrap();
        let path = file.path();
        let filename = path.to_string_lossy();
        if filename.ends_with(".tar.gz") || filename.ends_with(".tar.xz") {
            rustup::utils::raw::write_file(&path, "xxx").unwrap();
        }
    }
    cx.config
        .expect_err(&["rustup", "default", "nightly"], "checksum failed");
}

#[test]
fn install_override_toolchain_from_channel() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    cx.config.expect_ok(&["rustup", "override", "add", "beta"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
    cx.config
        .expect_ok(&["rustup", "override", "add", "stable"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
}

#[test]
fn install_override_toolchain_from_archive() {
    let mut cx = CliTestContext::from(Scenario::ArchivesV1);
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly-2015-01-01"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
    cx.config
        .expect_ok(&["rustup", "override", "add", "beta-2015-01-01"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.1.0");
    cx.config
        .expect_ok(&["rustup", "override", "add", "stable-2015-01-01"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.0.0");
}

#[test]
fn install_override_toolchain_from_version() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "override", "add", "1.1.0"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
}

#[test]
fn override_overrides_default() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.change_dir(tempdir.path(), &|config| {
        config.expect_ok(&["rustup", "override", "add", "beta"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
    });
}

#[test]
fn multiple_overrides() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.change_dir(tempdir1.path(), &|config| {
        config.expect_ok(&["rustup", "override", "add", "beta"]);
    });
    cx.config.change_dir(tempdir2.path(), &|config| {
        config.expect_ok(&["rustup", "override", "add", "stable"]);
    });

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");

    cx.config.change_dir(tempdir1.path(), &|config| {
        config.expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
    });
    cx.config.change_dir(tempdir2.path(), &|config| {
        config.expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
    });
}

#[test]
fn change_override() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.change_dir(tempdir.path(), &|config| {
        config.expect_ok(&["rustup", "override", "add", "nightly"]);
        config.expect_ok(&["rustup", "override", "add", "beta"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
    });
}

#[test]
fn remove_override_no_default() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.change_dir(tempdir.path(), &|config| {
        config.expect_ok(&["rustup", "override", "add", "nightly"]);
        config.expect_ok(&["rustup", "override", "remove"]);
        config.expect_err(
            &["rustc"],
            "rustup could not choose a version of rustc to run",
        );
    });
}

#[test]
fn remove_override_with_default() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.change_dir(tempdir.path(), &|config| {
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_ok(&["rustup", "override", "add", "beta"]);
        config.expect_ok(&["rustup", "override", "remove"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    });
}

#[test]
fn remove_override_with_multiple_overrides() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.change_dir(tempdir1.path(), &|config| {
        config.expect_ok(&["rustup", "override", "add", "beta"]);
    });
    cx.config.change_dir(tempdir2.path(), &|config| {
        config.expect_ok(&["rustup", "override", "add", "stable"]);
    });
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    cx.config.change_dir(tempdir1.path(), &|config| {
        config.expect_ok(&["rustup", "override", "remove"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    });
    cx.config.change_dir(tempdir2.path(), &|config| {
        config.expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
    });
}

#[test]
fn no_update_on_channel_when_date_has_not_changed() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustup", "update", "nightly"], "unchanged");
}

#[test]
fn update_on_channel_when_date_has_changed() {
    let mut cx = CliTestContext::from(Scenario::ArchivesV1);
    set_current_dist_date(&cx.config, "2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
    set_current_dist_date(&cx.config, "2015-01-02");
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
}

#[test]
fn run_command() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config.expect_ok(&["rustup", "default", "beta"]);
    cx.config.expect_stdout_ok(
        &["rustup", "run", "nightly", "rustc", "--version"],
        "hash-nightly-2",
    );
}

#[test]
fn remove_toolchain_then_add_again() {
    // Issue brson/multirust #53
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "default", "beta"]);
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "beta"]);
    cx.config.expect_ok(&["rustup", "update", "beta"]);
    cx.config.expect_ok(&["rustc", "--version"]);
}
