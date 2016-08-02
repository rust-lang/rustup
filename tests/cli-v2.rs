//! Test cases of the multirust command, using v2 manifests, mostly
//! derived from multirust/test-v2.sh

extern crate rustup_dist;
extern crate rustup_utils;
extern crate rustup_mock;
extern crate tempdir;

use std::fs;
use tempdir::TempDir;
use rustup_mock::clitools::{self, Config, Scenario,
                               expect_ok, expect_stdout_ok, expect_err,
                               expect_stderr_ok, expect_not_stdout_ok,
                               set_current_dist_date, change_dir,
                               this_host_triple};

use rustup_dist::dist::TargetTriple;

macro_rules! for_host { ($s: expr) => (&format!($s, this_host_triple())) }

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
        expect_ok(config, &["rustup", "default" , "nightly-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");
        expect_ok(config, &["rustup", "default" , "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-1");
        expect_ok(config, &["rustup", "default" , "stable-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-1");
    });
}

#[test]
fn install_toolchain_from_version() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default" , "1.1.0"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
    });
}

#[test]
fn default_existing_toolchain() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_stderr_ok(config, &["rustup", "default", "nightly"],
                         for_host!("using existing install for 'nightly-{0}'"));
    });
}

#[test]
fn update_channel() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn list_toolchains() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_ok(config, &["rustup", "update", "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustup", "toolchain", "list"],
                         "nightly");
        expect_stdout_ok(config, &["rustup", "toolchain", "list"],
                         "beta-2015-01-01");
    });
}

#[test]
fn list_toolchains_with_bogus_file() {
    // #520
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly"]);

        let name = "bogus_regular_file.txt";
        let path = config.rustupdir.join("toolchains").join(name);
        rustup_utils::utils::write_file(name, &path, "").unwrap();
        expect_stdout_ok(config, &["rustup", "toolchain", "list"], "nightly");
        expect_not_stdout_ok(config, &["rustup", "toolchain", "list"], name);
    });
}

#[test]
fn list_toolchains_with_none() {
    setup(&|config| {
        expect_stdout_ok(config, &["rustup", "toolchain", "list"],
                         "no installed toolchains");
    });
}

#[test]
fn remove_toolchain() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "list"]);
        expect_stdout_ok(config, &["rustup", "toolchain", "list"],
                         "no installed toolchains");
    });
}

#[test]
fn remove_default_toolchain_err_handling() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "nightly"]);
        expect_err(config, &["rustc"],
                           for_host!("toolchain 'nightly-{0}' is not installed"));
    });
}

#[test]
fn remove_override_toolchain_err_handling() {
    setup(&|config| {
        let tempdir = TempDir::new("rustup").unwrap();
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "default", "nightly"]);
            expect_ok(config, &["rustup", "override", "add", "beta"]);
            expect_ok(config, &["rustup", "toolchain", "remove", "beta"]);
            expect_err(config, &["rustc"],
                               for_host!("toolchain 'beta-{0}' is not installed"));
        });
    });
}

#[test]
fn bad_sha_on_manifest() {
    setup(&|config| {
        // Corrupt the sha
        let sha_file = config.distdir.join("dist/channel-rust-nightly.toml.sha256");
        let sha_str = rustup_utils::raw::read_file(&sha_file).unwrap();
        let mut sha_bytes = sha_str.into_bytes();
        &mut sha_bytes[..10].clone_from_slice(b"aaaaaaaaaa");
        let sha_str = String::from_utf8(sha_bytes).unwrap();
        rustup_utils::raw::write_file(&sha_file, &sha_str).unwrap();
        expect_err(config, &["rustup", "default", "nightly"],
                   "checksum failed");
    });
}

#[test]
fn bad_sha_on_installer() {
    setup(&|config| {
        // Since the v2 sha's are contained in the manifest, corrupt the installer
        let dir = config.distdir.join("dist/2015-01-02");
        for file in fs::read_dir(&dir).unwrap() {
            let file = file.unwrap();
            if file.path().to_string_lossy().ends_with(".tar.gz") {
                rustup_utils::raw::write_file(&file.path(), "xxx").unwrap();
            }
        }
        expect_err(config, &["rustup", "default", "nightly"],
                   "checksum failed");
    });
}

#[test]
fn install_override_toolchain_from_channel() {
    setup(&|config| {
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
        expect_ok(config, &["rustup", "override", "add", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-b-2");
        expect_ok(config, &["rustup", "override", "add", "stable"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-2");
    });
}

#[test]
fn install_override_toolchain_from_archive() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        expect_ok(config, &["rustup", "override", "add", "nightly-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        expect_ok(config, &["rustup", "override", "add", "beta-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-b-1");
        expect_ok(config, &["rustup", "override", "add", "stable-2015-01-01"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-1");
    });
}

#[test]
fn install_override_toolchain_from_version() {
    setup(&|config| {
        expect_ok(config, &["rustup", "override", "add", "1.1.0"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-s-2");
    });
}

#[test]
fn override_overrides_default() {
    setup(&|config| {
        let tempdir = TempDir::new("rustup").unwrap();
        expect_ok(config, &["rustup", "default" , "nightly"]);
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "override" , "add", "beta"]);
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
        change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "beta"]);
        });
        change_dir(tempdir2.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "stable"]);
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

// #316
#[test]
#[cfg(windows)]
fn override_windows_root() {
    setup(&|config| {
        use std::path::{PathBuf, Component};

        let cwd = ::std::env::current_dir().unwrap();
        let prefix = cwd.components().next().unwrap();
        let prefix = match prefix {
            Component::Prefix(p) => p,
            _ => panic!()
        };

        // This value is probably "C:"
        // Really sketchy to be messing with C:\ in a test...
        let prefix = prefix.as_os_str().to_str().unwrap();
        let prefix = format!("{}\\", prefix);
        change_dir(&PathBuf::from(&prefix), &|| {
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
        change_dir(tempdir.path(), &|| {
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
        change_dir(tempdir.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "nightly"]);
            expect_ok(config, &["rustup", "override", "remove"]);
            expect_err(config, &["rustc"],
                               "no default toolchain configured");
        });
    });
}

#[test]
fn remove_override_with_default() {
    setup(&|config| {
        let tempdir = TempDir::new("rustup").unwrap();
        change_dir(tempdir.path(), &|| {
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
        change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "beta"]);
        });
        change_dir(tempdir2.path(), &|| {
            expect_ok(config, &["rustup", "override", "add", "stable"]);
        });
        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        change_dir(tempdir1.path(), &|| {
            expect_ok(config, &["rustup", "override", "remove"]);
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
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_stdout_ok(config, &["rustup", "update", "nightly"],
                         "unchanged");
    });
}

#[test]
fn update_on_channel_when_date_has_changed() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-1");
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn run_command() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_ok(config, &["rustup", "default", "beta"]);
        expect_stdout_ok(config, &["rustup", "run", "nightly", "rustc" , "--version"],
                         "hash-n-2");
    });
}

#[test]
fn remove_toolchain_then_add_again() {
    // Issue brson/multirust #53
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "beta"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "beta"]);
        expect_ok(config, &["rustup", "update", "beta"]);
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
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-n-2");
    });
}

#[test]
fn upgrade_v2_to_v1() {
    clitools::setup(Scenario::Full, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        set_current_dist_date(config, "2015-01-02");
        fs::remove_file(config.distdir.join("dist/channel-rust-nightly.toml.sha256")).unwrap();
        expect_err(config, &["rustup", "update", "nightly"],
                           "the server unexpectedly provided an obsolete version of the distribution manifest");
    });
}

#[test]
fn list_targets_no_toolchain() {
    setup(&|config| {
        expect_err(config, &["rustup", "target", "list", "--toolchain=nightly"],
                   for_host!("toolchain 'nightly-{0}' is not installed"));
    });
}

#[test]
fn list_targets_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_err(config, &["rustup", "target", "list", "--toolchain=nightly"],
                   for_host!("toolchain 'nightly-{0}' does not support components"));
    });
}

#[test]
fn list_targets_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["rustup", "toolchain", "link", "default-from-path",
                            &path]);
        expect_ok(config, &["rustup", "default", "default-from-path"]);
        expect_err(config, &["rustup", "target", "list"],
                   "toolchain 'default-from-path' does not support components");
    });
}

#[test]
fn list_targets() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustup", "target", "list"],
                         clitools::CROSS_ARCH1);
        expect_stdout_ok(config, &["rustup", "target", "list"],
                         clitools::CROSS_ARCH2);
    });
}

#[test]
fn add_target() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        let path = format!("toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                           this_host_triple(), clitools::CROSS_ARCH1);
        assert!(config.rustupdir.join(path).exists());
    });
}

#[test]
fn add_target_no_toolchain() {
    setup(&|config| {
        expect_err(config, &["rustup", "target", "add", clitools::CROSS_ARCH1, "--toolchain=nightly"],
                   for_host!("toolchain 'nightly-{0}' is not installed"));
    });
}
#[test]
fn add_target_bogus() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(config, &["rustup", "target", "add", "bogus"],
                   "does not contain component 'rust-std' for target 'bogus'");
    });
}

#[test]
fn add_target_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_err(config, &["rustup", "target", "add", clitools::CROSS_ARCH1, "--toolchain=nightly"],
                   for_host!("toolchain 'nightly-{0}' does not support components"));
    });
}

#[test]
fn add_target_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["rustup", "toolchain", "link", "default-from-path",
                            &path]);
        expect_ok(config, &["rustup", "default", "default-from-path"]);
        expect_err(config, &["rustup", "target", "add", clitools::CROSS_ARCH1],
                   "toolchain 'default-from-path' does not support components");
    });
}

#[test]
fn add_target_again() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        expect_stderr_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1],
                         &format!("component 'rust-std' for target '{}' is up to date",
                                 clitools::CROSS_ARCH1));
        let path = format!("toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                           this_host_triple(), clitools::CROSS_ARCH1);
        assert!(config.rustupdir.join(path).exists());
    });
}

#[test]
fn add_target_host() {
    setup(&|config| {
        let trip = TargetTriple::from_build();
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(config, &["rustup", "target", "add", &trip.to_string()],
                   for_host!("component 'rust-std' for target '{0}' is required for toolchain 'nightly-{0}' and cannot be re-added"));
    });
}

#[test]
fn remove_target() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        expect_ok(config, &["rustup", "target", "remove", clitools::CROSS_ARCH1]);
        let path = format!("toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                           this_host_triple(), clitools::CROSS_ARCH1);
        assert!(!config.rustupdir.join(path).exists());
        let path = format!("toolchains/nightly-{}/lib/rustlib/{}/lib",
                           this_host_triple(), clitools::CROSS_ARCH1);
        assert!(!config.rustupdir.join(path).exists());
        let path = format!("toolchains/nightly-{}/lib/rustlib/{}",
                           this_host_triple(), clitools::CROSS_ARCH1);
        assert!(!config.rustupdir.join(path).exists());
    });
}

#[test]
fn remove_target_not_installed() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(config, &["rustup", "target", "remove", clitools::CROSS_ARCH1],
                   &format!("toolchain 'nightly-{}' does not contain component 'rust-std' for target '{}'",
                            this_host_triple(), clitools::CROSS_ARCH1));
    });
}

#[test]
fn remove_target_no_toolchain() {
    setup(&|config| {
        expect_err(config, &["rustup", "target", "remove", clitools::CROSS_ARCH1, "--toolchain=nightly"],
                   for_host!("toolchain 'nightly-{0}' is not installed"));
    });
}

#[test]
fn remove_target_bogus() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(config, &["rustup", "target", "remove", "bogus"],
                   "does not contain component 'rust-std' for target 'bogus'");
    });
}

#[test]
fn remove_target_v1_toolchain() {
    clitools::setup(Scenario::SimpleV1, &|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(config, &["rustup", "target", "remove", clitools::CROSS_ARCH1, "--toolchain=nightly"],
                   for_host!("toolchain 'nightly-{0}' does not support components"));
    });
}

#[test]
fn remove_target_custom_toolchain() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["rustup", "toolchain", "link", "default-from-path",
                            &path]);
        expect_ok(config, &["rustup", "default", "default-from-path"]);
        expect_err(config, &["rustup", "target", "remove", clitools::CROSS_ARCH1],
                   "toolchain 'default-from-path' does not support components");
    });
}

#[test]
fn remove_target_again() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        expect_ok(config, &["rustup", "target", "remove", clitools::CROSS_ARCH1]);
        expect_err(config, &["rustup", "target", "remove", clitools::CROSS_ARCH1],
                   &format!("toolchain 'nightly-{}' does not contain component 'rust-std' for target '{}'",
                            this_host_triple(), clitools::CROSS_ARCH1));
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

fn make_component_unavailable(config: &Config, name: &str, target: &TargetTriple) {
    use rustup_dist::manifest::Manifest;
    use rustup_mock::dist::create_hash;

    let ref manifest_path = config.distdir.join("dist/channel-rust-nightly.toml");
    let ref manifest_str = rustup_utils::raw::read_file(manifest_path).unwrap();
    let mut manifest = Manifest::parse(manifest_str).unwrap();
    {
        let mut std_pkg = manifest.packages.get_mut(name).unwrap();
        let mut target_pkg = std_pkg.targets.get_mut(target).unwrap();
        target_pkg.available = false;
    }
    let ref manifest_str = manifest.stringify();
    rustup_utils::raw::write_file(manifest_path, manifest_str).unwrap();

    // Have to update the hash too
    let ref hash_path = manifest_path.with_extension("toml.sha256");
    println!("{}", hash_path.display());
    create_hash(manifest_path, hash_path);
}

#[test]
fn update_unavailable_std() {
    setup(&|config| {
        let ref trip = TargetTriple::from_build();
        make_component_unavailable(config, "rust-std", trip);
        expect_err(config, &["rustup", "update", "nightly"],
                   &format!("component 'rust-std' for '{}' is unavailable for download", trip));
    });
}
