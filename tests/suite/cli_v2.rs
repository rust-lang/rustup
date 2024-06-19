//! Test cases of the rustup command, using v2 manifests, mostly
//! derived from multirust/test-v2.sh

use std::fs;
use std::io::Write;

use rustup::dist::TargetTriple;
use rustup::for_host;
use rustup::test::mock::clitools::{self, set_current_dist_date, CliTestContext, Config, Scenario};
use rustup::test::this_host_triple;

#[test]
fn rustc_no_default_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_err(
        &["rustc"],
        "rustup could not choose a version of rustc to run",
    );
}

#[test]
fn expected_bins_exist() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_stdout_ok(&["rustc", "--version"], "1.3.0");
}

#[test]
fn install_toolchain_from_channel() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
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
    let mut cx = CliTestContext::from(Scenario::ArchivesV2);
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
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "1.1.0"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
}

#[test]
fn install_with_profile() {
    let mut cx = CliTestContext::from(Scenario::UnavailableRls);
    // Start with a config that uses the "complete" profile
    set_current_dist_date(&cx.config, "2015-01-01");
    cx.config
        .expect_ok(&["rustup", "set", "profile", "complete"]);

    // Installing with minimal profile should only install rustc
    cx.config.expect_ok(&[
        "rustup",
        "toolchain",
        "install",
        "--profile",
        "minimal",
        "nightly",
    ]);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);

    cx.config.expect_component_executable("rustup");
    cx.config.expect_component_executable("rustc");
    cx.config.expect_component_not_executable("cargo");

    // After an update, we should _still_ only have the profile-dictated components
    set_current_dist_date(&cx.config, "2015-01-02");
    cx.config.expect_ok(&["rustup", "update", "nightly"]);

    cx.config.expect_component_executable("rustup");
    cx.config.expect_component_executable("rustc");
    cx.config.expect_component_not_executable("cargo");
}

#[test]
fn default_existing_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config.expect_stderr_ok(
        &["rustup", "default", "nightly"],
        for_host!("using existing install for 'nightly-{0}'"),
    );
}

#[test]
fn update_channel() {
    let mut cx = CliTestContext::from(Scenario::ArchivesV2);
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
    let mut cx = CliTestContext::from(Scenario::ArchivesV2);
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
fn list_toolchains_with_bogus_file() {
    // #520
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);

    let name = "bogus_regular_file.txt";
    let path = cx.config.rustupdir.join("toolchains").join(name);
    rustup::utils::utils::write_file(name, &path, "").unwrap();
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "nightly");
    cx.config
        .expect_not_stdout_ok(&["rustup", "toolchain", "list"], name);
}

#[test]
fn list_toolchains_with_none() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "no installed toolchains");
}

#[test]
fn remove_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "nightly"]);
    cx.config.expect_ok(&["rustup", "toolchain", "list"]);
    cx.config
        .expect_stdout_ok(&["rustup", "toolchain", "list"], "no installed toolchains");
}

// Issue #2873
#[test]
fn remove_toolchain_ignore_trailing_slash() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    // custom toolchain name with trailing slash
    let path = cx.config.customdir.join("custom-1");
    let path_str = path.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "dev", &path_str]);
    cx.config.expect_stderr_ok(
        &["rustup", "toolchain", "remove", "dev/"],
        "toolchain 'dev' uninstalled",
    );
    // check if custom toolchain directory contents are not removed
    let toolchain_dir_is_non_empty = fs::read_dir(&path).unwrap().next().is_some();
    assert!(toolchain_dir_is_non_empty);
    // distributable toolchain name with trailing slash
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config.expect_stderr_ok(
        &["rustup", "toolchain", "remove", for_host!("nightly-{}/")],
        for_host!("toolchain 'nightly-{}' uninstalled"),
    );
}

#[test]
fn add_remove_multiple_toolchains() {
    fn go(add: &str, rm: &str) {
        let mut cx = CliTestContext::from(Scenario::SimpleV2);
        let tch1 = "beta";
        let tch2 = "nightly";

        cx.config
            .expect_ok(&["rustup", "toolchain", add, tch1, tch2]);
        cx.config.expect_ok(&["rustup", "toolchain", "list"]);
        cx.config
            .expect_stdout_ok(&["rustup", "toolchain", "list"], tch1);
        cx.config
            .expect_stdout_ok(&["rustup", "toolchain", "list"], tch2);

        cx.config
            .expect_ok(&["rustup", "toolchain", rm, tch1, tch2]);
        cx.config.expect_ok(&["rustup", "toolchain", "list"]);
        cx.config
            .expect_not_stdout_ok(&["rustup", "toolchain", "list"], tch1);
        cx.config
            .expect_not_stdout_ok(&["rustup", "toolchain", "list"], tch2);
    }

    for add in &["add", "update", "install"] {
        for rm in &["remove", "uninstall"] {
            go(add, rm);
        }
    }
}

#[test]
fn remove_default_toolchain_autoinstalls() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "nightly"]);
    cx.config
        .expect_stderr_ok(&["rustc", "--version"], "info: installing component");
}

#[test]
fn remove_override_toolchain_err_handling() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mut cx = cx.change_dir(tempdir.path());
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_ok(&["rustup", "override", "add", "beta"]);
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "beta"]);
    cx.config
        .expect_stderr_ok(&["rustc", "--version"], "info: installing component");
}

#[test]
fn file_override_toolchain_err_handling() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    rustup::utils::raw::write_file(&toolchain_file, "beta").unwrap();
    cx.config
        .expect_stderr_ok(&["rustc", "--version"], "info: installing component");
}

#[test]
fn plus_override_toolchain_err_handling() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_err(
        &["rustc", "+beta"],
        for_host!("toolchain 'beta-{0}' is not installed"),
    );
}

#[test]
fn bad_sha_on_manifest() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    // Corrupt the sha
    let sha_file = cx
        .config
        .distdir
        .as_ref()
        .unwrap()
        .join("dist/channel-rust-nightly.toml.sha256");
    let sha_str = fs::read_to_string(&sha_file).unwrap();
    let mut sha_bytes = sha_str.into_bytes();
    sha_bytes[..10].clone_from_slice(b"aaaaaaaaaa");
    let sha_str = String::from_utf8(sha_bytes).unwrap();
    rustup::utils::raw::write_file(&sha_file, &sha_str).unwrap();
    // We fail because the sha is bad, but we should emit the special message to that effect.
    cx.config.expect_err(
        &["rustup", "default", "nightly"],
        "update not yet available",
    );
}

#[test]
fn bad_manifest() {
    // issue #3851
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    // install some toolchain
    cx.config.expect_ok(&["rustup", "update", "nightly"]);

    #[cfg(not(target_os = "windows"))]
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/multirust-channel-manifest.toml",
        this_host_triple(),
    );

    #[cfg(target_os = "windows")]
    let path = format!(
        r"toolchains\nightly-{}\lib/rustlib\multirust-channel-manifest.toml",
        this_host_triple(),
    );

    assert!(cx.config.rustupdir.has(&path));
    let path = cx.config.rustupdir.join(&path);

    // corrupt the manifest file by inserting a NUL byte at some position
    let old = fs::read_to_string(&path).unwrap();
    let pattern = "[[pkg.rust.targ";
    let (prefix, suffix) = old.split_once(pattern).unwrap();
    let new = format!("{prefix}{pattern}\u{0}{suffix}");
    fs::write(&path, new).unwrap();

    // run some commands that try to load the manifest and
    // check that the manifest parsing error includes the manifest file path
    cx.config.expect_err(
        &["rustup", "check"],
        &format!("error: could not parse manifest file: '{}'", path.display()),
    );
    cx.config.expect_err(
        &["cargo", "--help"],
        &format!("error: could not parse manifest file: '{}'", path.display()),
    );
}

#[test]
fn bad_sha_on_installer() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    // Since the v2 sha's are contained in the manifest, corrupt the installer
    let dir = cx.config.distdir.as_ref().unwrap().join("dist/2015-01-02");
    for file in fs::read_dir(dir).unwrap() {
        let file = file.unwrap();
        let path = file.path();
        let filename = path.to_string_lossy();
        if filename.ends_with(".tar.gz")
            || filename.ends_with(".tar.xz")
            || filename.ends_with(".tar.zst")
        {
            rustup::utils::raw::write_file(&path, "xxx").unwrap();
        }
    }
    cx.config
        .expect_err(&["rustup", "default", "nightly"], "checksum failed");
}

#[test]
fn install_override_toolchain_from_channel() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
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
    let mut cx = CliTestContext::from(Scenario::ArchivesV2);
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
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "override", "add", "1.1.0"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
}

#[test]
fn override_overrides_default() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.expect_ok(&["rustup", "default", "nightly"]);

    let mut cx = cx.change_dir(tempdir.path());
    cx.config.expect_ok(&["rustup", "override", "add", "beta"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
}

#[test]
fn multiple_overrides() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    cx.config.expect_ok(&["rustup", "default", "nightly"]);

    {
        let mut cx = cx.change_dir(tempdir1.path());
        cx.config.expect_ok(&["rustup", "override", "add", "beta"]);
    }

    {
        let mut cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect_ok(&["rustup", "override", "add", "stable"]);
    }

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
    }
}

// #316
#[test]
#[cfg(windows)]
fn override_windows_root() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    use std::path::{Component, PathBuf};

    let cwd = cx.config.current_dir();
    let prefix = cwd.components().next().unwrap();
    let prefix = match prefix {
        Component::Prefix(p) => p,
        _ => panic!(),
    };

    // This value is probably "C:"
    // Really sketchy to be messing with C:\ in a test...
    let prefix = prefix.as_os_str().to_str().unwrap();
    let prefix = format!("{prefix}\\");
    let mut cx = cx.change_dir(&PathBuf::from(&prefix));
    cx.config.expect_ok(&["rustup", "default", "stable"]);
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    cx.config.expect_ok(&["rustup", "override", "remove"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
}

#[test]
fn change_override() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mut cx = cx.change_dir(tempdir.path());
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"]);
    cx.config.expect_ok(&["rustup", "override", "add", "beta"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
}

#[test]
fn remove_override_no_default() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mut cx = cx.change_dir(tempdir.path());
    cx.config
        .expect_ok(&["rustup", "override", "add", "nightly"]);
    cx.config.expect_ok(&["rustup", "override", "remove"]);
    cx.config.expect_err(
        &["rustc"],
        "rustup could not choose a version of rustc to run",
    );
}

#[test]
fn remove_override_with_default() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mut cx = cx.change_dir(tempdir.path());
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_ok(&["rustup", "override", "add", "beta"]);
    cx.config.expect_ok(&["rustup", "override", "remove"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
}

#[test]
fn remove_override_with_multiple_overrides() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config.expect_ok(&["rustup", "default", "nightly"]);

    {
        let mut cx = cx.change_dir(tempdir1.path());
        cx.config.expect_ok(&["rustup", "override", "add", "beta"]);
    }

    {
        let mut cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect_ok(&["rustup", "override", "add", "stable"]);
    }

    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");

    {
        let mut cx = cx.change_dir(tempdir1.path());
        cx.config.expect_ok(&["rustup", "override", "remove"]);
        cx.config
            .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
    }
}

#[test]
fn no_update_on_channel_when_date_has_not_changed() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustup", "update", "nightly"], "unchanged");
}

#[test]
fn update_on_channel_when_date_has_changed() {
    let mut cx = CliTestContext::from(Scenario::ArchivesV2);
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
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
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
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "beta"]);
    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "beta"]);
    cx.config.expect_ok(&["rustup", "update", "beta"]);
    cx.config.expect_ok(&["rustc", "--version"]);
}

#[test]
fn upgrade_v1_to_v2() {
    let mut cx = CliTestContext::from(Scenario::Full);
    set_current_dist_date(&cx.config, "2015-01-01");
    // Delete the v2 manifest so the first day we install from the v1s
    fs::remove_file(
        cx.config
            .distdir
            .as_ref()
            .unwrap()
            .join("dist/channel-rust-nightly.toml.sha256"),
    )
    .unwrap();
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
    set_current_dist_date(&cx.config, "2015-01-02");
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
}

#[test]
fn upgrade_v2_to_v1() {
    let mut cx = CliTestContext::from(Scenario::Full);
    set_current_dist_date(&cx.config, "2015-01-01");
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    set_current_dist_date(&cx.config, "2015-01-02");
    fs::remove_file(
        cx.config
            .distdir
            .as_ref()
            .unwrap()
            .join("dist/channel-rust-nightly.toml.sha256"),
    )
    .unwrap();
    cx.config.expect_err(
        &["rustup", "update", "nightly"],
        "the server unexpectedly provided an obsolete version of the distribution manifest",
    );
}

#[test]
fn list_targets_no_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_err(
        &["rustup", "target", "list", "--toolchain=nightly"],
        for_host!("toolchain 'nightly-{0}' is not installed"),
    );
}

#[test]
fn list_targets_v1_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config.expect_err(
        &["rustup", "target", "list", "--toolchain=nightly"],
        for_host!("toolchain 'nightly-{0}' does not support components"),
    );
}

#[test]
fn list_targets_custom_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "default-from-path", &path]);
    cx.config
        .expect_ok(&["rustup", "default", "default-from-path"]);
    cx.config.expect_err(
        &["rustup", "target", "list"],
        "toolchain 'default-from-path' does not support components",
    );
}

#[test]
fn list_targets() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustup", "target", "list"], clitools::CROSS_ARCH1);
    cx.config
        .expect_stdout_ok(&["rustup", "target", "list"], clitools::CROSS_ARCH2);
}

#[test]
fn list_installed_targets() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let trip = this_host_triple();

    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustup", "target", "list", "--installed"], &trip);
}

#[test]
fn add_target1() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        clitools::CROSS_ARCH1
    );
    assert!(cx.config.rustupdir.has(path));
}

#[test]
fn add_target2() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH2]);
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        clitools::CROSS_ARCH2
    );
    assert!(cx.config.rustupdir.has(path));
}

#[test]
fn add_all_targets() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_ok(&["rustup", "target", "add", "all"]);
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        clitools::CROSS_ARCH1
    );
    assert!(cx.config.rustupdir.has(path));
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        clitools::CROSS_ARCH2
    );
    assert!(cx.config.rustupdir.has(path));
}

#[test]
fn add_all_targets_fail() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_err(
        &[
            "rustup",
            "target",
            "add",
            clitools::CROSS_ARCH1,
            "all",
            clitools::CROSS_ARCH2,
        ],
        &format!(
            "`rustup target add {} all {}` includes `all`",
            clitools::CROSS_ARCH1,
            clitools::CROSS_ARCH2
        ),
    );
}

#[test]
fn add_target_by_component_add() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_not_stdout_ok(
        &["rustup", "target", "list"],
        &format!("{} (installed)", clitools::CROSS_ARCH1),
    );
    cx.config.expect_ok(&[
        "rustup",
        "component",
        "add",
        &format!("rust-std-{}", clitools::CROSS_ARCH1),
    ]);
    cx.config.expect_stdout_ok(
        &["rustup", "target", "list"],
        &format!("{} (installed)", clitools::CROSS_ARCH1),
    );
}

#[test]
fn remove_target_by_component_remove() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
    cx.config.expect_stdout_ok(
        &["rustup", "target", "list"],
        &format!("{} (installed)", clitools::CROSS_ARCH1),
    );
    cx.config.expect_ok(&[
        "rustup",
        "component",
        "remove",
        &format!("rust-std-{}", clitools::CROSS_ARCH1),
    ]);
    cx.config.expect_not_stdout_ok(
        &["rustup", "target", "list"],
        &format!("{} (installed)", clitools::CROSS_ARCH1),
    );
}

#[test]
fn add_target_no_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_err(
        &[
            "rustup",
            "target",
            "add",
            clitools::CROSS_ARCH1,
            "--toolchain=nightly",
        ],
        for_host!("toolchain 'nightly-{0}' is not installed"),
    );
}
#[test]
fn add_target_bogus() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_err(
            &["rustup", "target", "add", "bogus"],
            "does not support target 'bogus'\n\
            note: you can see a list of supported targets with `rustc --print=target-list`\n\
            note: if you are adding support for a new target to rustc itself, see https://rustc-dev-guide.rust-lang.org/building/new-target.html",
        );
}

#[test]
fn add_target_v1_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config.expect_err(
        &[
            "rustup",
            "target",
            "add",
            clitools::CROSS_ARCH1,
            "--toolchain=nightly",
        ],
        for_host!("toolchain 'nightly-{0}' does not support components (v1 manifest)"),
    );
}

#[test]
fn add_target_custom_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "default-from-path", &path]);
    cx.config
        .expect_ok(&["rustup", "default", "default-from-path"]);
    cx.config.expect_err(
        &["rustup", "target", "add", clitools::CROSS_ARCH1],
        "toolchain 'default-from-path' does not support components",
    );
}

#[test]
fn cannot_add_empty_named_custom_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config.expect_err(
        &["rustup", "toolchain", "link", "", &path],
        "invalid value '' for '<TOOLCHAIN>': invalid toolchain name ''",
    );
}

#[test]
fn add_target_again() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
    cx.config.expect_stderr_ok(
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
    assert!(cx.config.rustupdir.has(path));
}

#[test]
fn add_target_host() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let trip = this_host_triple();
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_ok(&["rustup", "target", "add", &trip]);
}

#[test]
fn remove_target() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
    cx.config
        .expect_ok(&["rustup", "target", "remove", clitools::CROSS_ARCH1]);
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        clitools::CROSS_ARCH1
    );
    assert!(!cx.config.rustupdir.has(path));
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib",
        this_host_triple(),
        clitools::CROSS_ARCH1
    );
    assert!(!cx.config.rustupdir.has(path));
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}",
        this_host_triple(),
        clitools::CROSS_ARCH1
    );
    assert!(!cx.config.rustupdir.has(path));
}

#[test]
fn remove_target_not_installed() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_err(
        &["rustup", "target", "remove", clitools::CROSS_ARCH1],
        &format!(
            "toolchain 'nightly-{}' does not have target '{}' installed",
            this_host_triple(),
            clitools::CROSS_ARCH1
        ),
    );
}

#[test]
fn remove_target_no_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_err(
        &[
            "rustup",
            "target",
            "remove",
            clitools::CROSS_ARCH1,
            "--toolchain=nightly",
        ],
        for_host!("toolchain 'nightly-{0}' is not installed"),
    );
}

#[test]
fn remove_target_bogus() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_err(
        &["rustup", "target", "remove", "bogus"],
        "does not have target 'bogus' installed",
    );
}

#[test]
fn remove_target_v1_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV1);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_err(
        &[
            "rustup",
            "target",
            "remove",
            clitools::CROSS_ARCH1,
            "--toolchain=nightly",
        ],
        for_host!("toolchain 'nightly-{0}' does not support components (v1 manifest)"),
    );
}

#[test]
fn remove_target_custom_toolchain() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect_ok(&["rustup", "toolchain", "link", "default-from-path", &path]);
    cx.config
        .expect_ok(&["rustup", "default", "default-from-path"]);
    cx.config.expect_err(
        &["rustup", "target", "remove", clitools::CROSS_ARCH1],
        "toolchain 'default-from-path' does not support components",
    );
}

#[test]
fn remove_target_again() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
    cx.config
        .expect_ok(&["rustup", "target", "remove", clitools::CROSS_ARCH1]);
    cx.config.expect_err(
        &["rustup", "target", "remove", clitools::CROSS_ARCH1],
        &format!(
            "toolchain 'nightly-{}' does not have target '{}' installed",
            this_host_triple(),
            clitools::CROSS_ARCH1
        ),
    );
}

#[test]
fn remove_target_host() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let host = this_host_triple();
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config
        .expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
    cx.config.expect_stderr_ok(
            &["rustup", "target", "remove", &host],
            "after removing the default host target, proc-macros and build scripts might no longer build",
        );
    let path = format!("toolchains/nightly-{host}/lib/rustlib/{host}/lib/libstd.rlib");
    assert!(!cx.config.rustupdir.has(path));
    let path = format!("toolchains/nightly-{host}/lib/rustlib/{host}/lib");
    assert!(!cx.config.rustupdir.has(path));
    let path = format!("toolchains/nightly-{host}/lib/rustlib/{host}");
    assert!(!cx.config.rustupdir.has(path));
}

#[test]
fn remove_target_last() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let host = this_host_triple();
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_stderr_ok(
        &["rustup", "target", "remove", &host],
        "after removing the last target, no build targets will be available",
    );
}

#[test]
// Issue #304
fn remove_target_missing_update_hash() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "update", "nightly"]);

    let file_name = format!("nightly-{}", this_host_triple());
    fs::remove_file(cx.config.rustupdir.join("update-hashes").join(file_name)).unwrap();

    cx.config
        .expect_ok(&["rustup", "toolchain", "remove", "nightly"]);
}

// Issue #1777
#[test]
fn warn_about_and_remove_stray_hash() {
    let mut cx = CliTestContext::from(Scenario::None);
    let mut hash_path = cx.config.rustupdir.join("update-hashes");
    fs::create_dir_all(&hash_path).expect("Unable to make the update-hashes directory");
    hash_path.push(for_host!("nightly-{}"));
    let mut file = fs::File::create(&hash_path).expect("Unable to open update-hash file");
    file.write_all(b"LEGITHASH")
        .expect("Unable to write update-hash");
    drop(file);

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config.expect_stderr_ok(
        &["rustup", "toolchain", "install", "nightly"],
        &format!(
            "removing stray hash found at '{}' in order to continue",
            hash_path.display()
        ),
    );
}

fn make_component_unavailable(config: &Config, name: &str, target: String) {
    use rustup::dist::manifest::Manifest;
    use rustup::test::mock::dist::create_hash;

    let manifest_path = config
        .distdir
        .as_ref()
        .unwrap()
        .join("dist/channel-rust-nightly.toml");
    let manifest_str = fs::read_to_string(&manifest_path).unwrap();
    let mut manifest = Manifest::parse(&manifest_str).unwrap();
    {
        let std_pkg = manifest.packages.get_mut(name).unwrap();
        let target = TargetTriple::new(target);
        let target_pkg = std_pkg.targets.get_mut(&target).unwrap();
        target_pkg.bins = Vec::new();
    }
    let manifest_str = manifest.stringify().unwrap();
    rustup::utils::raw::write_file(&manifest_path, &manifest_str).unwrap();

    // Have to update the hash too
    let hash_path = manifest_path.with_extension("toml.sha256");
    println!("{}", hash_path.display());
    create_hash(&manifest_path, &hash_path);
}

#[test]
fn update_unavailable_std() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    make_component_unavailable(&cx.config, "rust-std", this_host_triple());
    cx.config.expect_err(
            &["rustup", "update", "nightly", ],
            for_host!(
                "component 'rust-std' for target '{0}' is unavailable for download for channel 'nightly'"
            ),
        );
}

#[test]
fn add_missing_component() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    make_component_unavailable(&cx.config, "rls-preview", this_host_triple());
    cx.config
        .expect_ok(&["rustup", "toolchain", "add", "nightly"]);
    cx.config.expect_err(
        &["rustup", "component", "add", "rls-preview"],
        for_host!(
            "component 'rls' for target '{0}' is unavailable for download for channel 'nightly'\n\
                Sometimes not all components are available in any given nightly."
        ),
    );
    // Make sure the following pattern does not match,
    // thus addressing https://github.com/rust-lang/rustup/issues/3418.
    cx.config.expect_not_stderr_err(
        &["rustup", "component", "add", "rls-preview"],
        "If you don't need the component, you can remove it with:",
    );
}

#[test]
fn add_missing_component_toolchain() {
    let cx = CliTestContext::from(Scenario::SimpleV2);
    make_component_unavailable(&cx.config, "rust-std", this_host_triple());
    cx.config.expect_err(
            &["rustup", "toolchain", "add", "nightly"],
            for_host!(
                r"component 'rust-std' for target '{0}' is unavailable for download for channel 'nightly'
Sometimes not all components are available in any given nightly. If you don't need the component, you could try a minimal installation with:

    rustup toolchain add nightly --profile minimal

If you require these components, please install and use the latest successful build version,
which you can find at <https://rust-lang.github.io/rustup-components-history>.

After determining the correct date, install it with a command such as:

    rustup toolchain install nightly-2018-12-27

Then you can use the toolchain with commands such as:

    cargo +nightly-2018-12-27 build"
            ),
        );
}

#[test]
fn update_unavailable_force() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let trip = this_host_triple();
    cx.config.expect_ok(&["rustup", "update", "nightly"]);
    cx.config.expect_ok(&[
        "rustup",
        "component",
        "add",
        "rls",
        "--toolchain",
        "nightly",
    ]);
    make_component_unavailable(&cx.config, "rls-preview", trip);
    cx.config.expect_err(
        &["rustup", "update", "nightly"],
        for_host!(
            "component 'rls' for target '{0}' is unavailable for download for channel 'nightly'"
        ),
    );
    cx.config
        .expect_ok(&["rustup", "update", "nightly", "--force"]);
}

#[test]
fn add_component_suggest_best_match() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_err(
        &["rustup", "component", "add", "rsl"],
        "did you mean 'rls'?",
    );
    cx.config.expect_err(
        &["rustup", "component", "add", "rsl-preview"],
        "did you mean 'rls-preview'?",
    );
    cx.config.expect_err(
        &["rustup", "component", "add", "rustd"],
        "did you mean 'rustc'?",
    );
    cx.config
        .expect_not_stderr_err(&["rustup", "component", "add", "potato"], "did you mean");
}

#[test]
fn remove_component_suggest_best_match() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_not_stderr_err(
        &["rustup", "component", "remove", "rsl"],
        "did you mean 'rls'?",
    );
    cx.config.expect_ok(&["rustup", "component", "add", "rls"]);
    cx.config.expect_err(
        &["rustup", "component", "remove", "rsl"],
        "did you mean 'rls'?",
    );
    cx.config
        .expect_ok(&["rustup", "component", "add", "rls-preview"]);
    cx.config.expect_err(
        &["rustup", "component", "add", "rsl-preview"],
        "did you mean 'rls-preview'?",
    );
    cx.config.expect_err(
        &["rustup", "component", "remove", "rustd"],
        "did you mean 'rustc'?",
    );
}

#[test]
fn add_target_suggest_best_match() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_err(
        &[
            "rustup",
            "target",
            "add",
            &format!("{}a", clitools::CROSS_ARCH1)[..],
        ],
        &format!("did you mean '{}'", clitools::CROSS_ARCH1),
    );
    cx.config
        .expect_not_stderr_err(&["rustup", "target", "add", "potato"], "did you mean");
}

#[test]
fn remove_target_suggest_best_match() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_not_stderr_err(
        &[
            "rustup",
            "target",
            "remove",
            &format!("{}a", clitools::CROSS_ARCH1)[..],
        ],
        &format!("did you mean '{}'", clitools::CROSS_ARCH1),
    );
    cx.config
        .expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
    cx.config.expect_err(
        &[
            "rustup",
            "target",
            "remove",
            &format!("{}a", clitools::CROSS_ARCH1)[..],
        ],
        &format!("did you mean '{}'", clitools::CROSS_ARCH1),
    );
}

#[test]
fn target_list_ignores_unavailable_targets() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    let target_list = &["rustup", "target", "list"];
    cx.config
        .expect_stdout_ok(target_list, clitools::CROSS_ARCH1);
    make_component_unavailable(&cx.config, "rust-std", clitools::CROSS_ARCH1.to_owned());
    cx.config
        .expect_ok(&["rustup", "update", "nightly", "--force"]);
    cx.config
        .expect_not_stdout_ok(target_list, clitools::CROSS_ARCH1);
}

#[test]
fn install_with_components() {
    fn go(comp_args: &[&str]) {
        let mut args = vec!["rustup", "toolchain", "install", "nightly"];
        args.extend_from_slice(comp_args);

        let mut cx = CliTestContext::from(Scenario::SimpleV2);
        cx.config.expect_ok(&args);
        cx.config
            .expect_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)");
        cx.config.expect_stdout_ok(
            &["rustup", "component", "list"],
            &format!("rust-analysis-{} (installed)", this_host_triple()),
        );
    }

    go(&["-c", "rust-src", "-c", "rust-analysis"]);
    go(&["-c", "rust-src,rust-analysis"]);
}

#[test]
fn install_with_targets() {
    fn go(comp_args: &[&str]) {
        let mut args = vec!["rustup", "toolchain", "install", "nightly"];
        args.extend_from_slice(comp_args);

        let mut cx = CliTestContext::from(Scenario::SimpleV2);
        cx.config.expect_ok(&args);
        cx.config.expect_stdout_ok(
            &["rustup", "target", "list"],
            &format!("{} (installed)", clitools::CROSS_ARCH1),
        );
        cx.config.expect_stdout_ok(
            &["rustup", "target", "list"],
            &format!("{} (installed)", clitools::CROSS_ARCH2),
        );
    }

    go(&["-t", clitools::CROSS_ARCH1, "-t", clitools::CROSS_ARCH2]);
    go(&[
        "-t",
        &format!("{},{}", clitools::CROSS_ARCH1, clitools::CROSS_ARCH2),
    ]);
}

#[test]
fn install_with_component_and_target() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&["rustup", "default", "nightly"]);
    cx.config.expect_ok(&[
        "rustup",
        "toolchain",
        "install",
        "nightly",
        "-c",
        "rls",
        "-t",
        clitools::CROSS_ARCH1,
    ]);
    cx.config.expect_stdout_ok(
        &["rustup", "component", "list"],
        &format!("rls-{} (installed)", this_host_triple()),
    );
    cx.config.expect_stdout_ok(
        &["rustup", "target", "list"],
        &format!("{} (installed)", clitools::CROSS_ARCH1),
    );
}

#[test]
fn test_warn_if_complete_profile_is_used() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config
        .expect_ok(&["rustup", "set", "auto-self-update", "enable"]);
    cx.config.expect_err(
        &[
            "rustup",
            "toolchain",
            "install",
            "--profile",
            "complete",
            "stable",
        ],
        "warn: downloading with complete profile",
    );
}

#[test]
fn test_complete_profile_skips_missing_when_forced() {
    let mut cx = CliTestContext::from(Scenario::UnavailableRls);
    set_current_dist_date(&cx.config, "2015-01-01");

    cx.config
        .expect_ok(&["rustup", "set", "profile", "complete"]);
    // First try and install without force
    cx.config.expect_err(
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
            ],
            for_host!("error: component 'rls' for target '{}' is unavailable for download for channel 'nightly'")
        );
    // Now try and force
    cx.config.expect_stderr_ok(
        &["rustup", "toolchain", "install", "--force", "nightly"],
        for_host!("warn: Force-skipping unavailable component 'rls-{}'"),
    );

    // Ensure that the skipped component (rls) is not installed
    cx.config.expect_not_stdout_ok(
        &["rustup", "component", "list"],
        for_host!("rls-{} (installed)"),
    );
}

#[test]
fn run_with_install_flag_against_unavailable_component() {
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    let trip = this_host_triple();
    make_component_unavailable(&cx.config, "rust-std", trip);
    cx.config.expect_ok_ex(
        &[
            "rustup",
            "run",
            "--install",
            "nightly",
            "rustc",
            "--version",
        ],
        "1.3.0 (hash-nightly-2)
",
        for_host!(
            r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
warn: Force-skipping unavailable component 'rust-std-{0}'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rustc'
"
        ),
    );
}

#[test]
fn install_allow_downgrade() {
    let mut cx = CliTestContext::from(Scenario::MissingComponent);
    let trip = this_host_triple();

    // this dist has no rls and there is no newer one
    set_current_dist_date(&cx.config, "2019-09-14");
    cx.config
        .expect_ok(&["rustup", "toolchain", "install", "nightly"]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-3");
    cx.config.expect_component_not_executable("rls");

    cx.config.expect_err(
        &["rustup", "toolchain", "install", "nightly", "-c", "rls"],
        &format!(
            "component 'rls' for target '{trip}' is unavailable for download for channel 'nightly'",
        ),
    );
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-3");
    cx.config.expect_component_not_executable("rls");

    cx.config.expect_ok(&[
        "rustup",
        "toolchain",
        "install",
        "nightly",
        "-c",
        "rls",
        "--allow-downgrade",
    ]);
    cx.config
        .expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
    cx.config.expect_component_executable("rls");
}

#[test]
fn regression_2601() {
    // We're checking that we don't regress per #2601
    let mut cx = CliTestContext::from(Scenario::SimpleV2);
    cx.config.expect_ok(&[
        "rustup",
        "toolchain",
        "install",
        "--profile",
        "minimal",
        "nightly",
        "--component",
        "rust-src",
    ]);
    // The bug exposed in #2601 was that the above would end up installing
    // rust-src-$ARCH which would then have to be healed on the following
    // command, resulting in a reinstallation.
    cx.config.expect_stderr_ok(
        &["rustup", "component", "add", "rust-src"],
        "info: component 'rust-src' is up to date",
    );
}
