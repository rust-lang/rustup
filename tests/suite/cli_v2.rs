//! Test cases of the rustup command, using v2 manifests, mostly
//! derived from multirust/test-v2.sh

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use rustup::dist::TargetTriple;
use rustup::dist::manifest::Manifest;
use rustup::test::{
    CROSS_ARCH1, CROSS_ARCH2, CliTestContext, Config, Scenario, create_hash, this_host_triple,
};

#[tokio::test]
async fn rustc_no_default_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustc"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: rustup could not choose a version of rustc to run, because one wasn't specified explicitly, and no default is configured.
...
"#]])
        .is_err();
}

#[tokio::test]
async fn expected_bins_exist() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_toolchain_from_channel() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_toolchain_from_archive() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config
        .expect(["rustup", "default", "nightly-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default", "beta-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-beta-1.1.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "default", "stable-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.0.0 (hash-stable-1.0.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_toolchain_from_version() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "1.1.0"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_with_profile() {
    let cx = CliTestContext::new(Scenario::UnavailableRls).await;
    // Start with a config that uses the "complete" profile
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "set", "profile", "complete"])
        .await
        .is_ok();

    // Installing with minimal profile should only install rustc
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "install",
            "--profile",
            "minimal",
            "nightly",
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    cx.config.expect_component_executable("rustup").await;
    cx.config.expect_component_executable("rustc").await;
    cx.config.expect_component_not_executable("cargo").await;

    // After an update, we should _still_ only have the profile-dictated components
    cx.config.set_current_dist_date("2015-01-02");
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();

    cx.config.expect_component_executable("rustup").await;
    cx.config.expect_component_executable("rustc").await;
    cx.config.expect_component_not_executable("cargo").await;
}

#[tokio::test]
async fn default_existing_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: using existing install for 'nightly-[HOST_TRIPLE]'
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn update_channel() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
    cx.config.set_current_dist_date("2015-01-02");
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
}

#[tokio::test]
async fn list_toolchains() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "beta-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
beta-2015-01-01-[HOST_TRIPLE]
nightly-[HOST_TRIPLE] (active, default)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list", "-v"])
        .await
        .with_stdout(snapbox::str![[r#"
beta-2015-01-01-[HOST_TRIPLE] [..]/toolchains/beta-2015-01-01-[HOST_TRIPLE]
nightly-[HOST_TRIPLE] (active, default) [..]/toolchains/nightly-[HOST_TRIPLE]

"#]])
        .is_ok();
}

#[tokio::test]
async fn list_toolchains_with_bogus_file() {
    // #520
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();

    let name = "bogus_regular_file.txt";
    let path = cx.config.rustupdir.join("toolchains").join(name);
    rustup::utils::write_file(name, &path, "").unwrap();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE] (active, default)

"#]])
        .is_ok();
}

#[tokio::test]
async fn list_toolchains_with_none() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
no installed toolchains

"#]])
        .is_ok();
}

#[tokio::test]
async fn remove_toolchain_default() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: removing the default toolchain; proc-macros and build scripts might no longer build
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
no installed toolchains

"#]])
        .is_ok();
}

#[tokio::test]
async fn remove_toolchain_active() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "set", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "stable"])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: removing the active toolchain; a toolchain override will be required for running Rust tools
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
nightly-[HOST_TRIPLE] (default)

"#]])
        .is_ok();
}

// Issue #2873
#[tokio::test]
async fn remove_toolchain_ignore_trailing_slash() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    // custom toolchain name with trailing slash
    let path = cx.config.customdir.join("custom-1");
    let path_str = path.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "dev", &path_str])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "dev/"])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: toolchain 'dev' uninstalled
...
"#]])
        .is_ok();
    // check if custom toolchain directory contents are not removed
    let toolchain_dir_is_non_empty = fs::read_dir(&path).unwrap().next().is_some();
    assert!(toolchain_dir_is_non_empty);
    // distributable toolchain name with trailing slash
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "remove",
            &format!("nightly-{}/", this_host_triple()),
        ])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: toolchain 'nightly-[HOST_TRIPLE]' uninstalled
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn add_remove_multiple_toolchains() {
    async fn go(add: &str, rm: &str) {
        let cx = CliTestContext::new(Scenario::SimpleV2).await;
        let tch1 = "beta";
        let tch2 = "nightly";

        cx.config
            .expect(["rustup", "toolchain", add, tch1, tch2])
            .await
            .is_ok();
        cx.config
            .expect(["rustup", "toolchain", "list"])
            .await
            .is_ok();
        cx.config
            .expect(["rustup", "toolchain", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
beta-[HOST_TRIPLE] (active, default)
nightly-[HOST_TRIPLE]

"#]])
            .is_ok();
        cx.config
            .expect(["rustup", "toolchain", rm, tch1, tch2])
            .await
            .is_ok();
        cx.config
            .expect(["rustup", "toolchain", "list"])
            .await
            .is_ok();
        cx.config
            .expect(["rustup", "toolchain", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
no installed toolchains

"#]])
            .is_ok();
        cx.config
            .expect(["rustup", "toolchain", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
no installed toolchains

"#]])
            .is_ok();
    }

    for add in &["add", "update", "install"] {
        for rm in &["remove", "uninstall"] {
            go(add, rm).await;
        }
    }
}

#[tokio::test]
async fn remove_override_toolchain_err_handling() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'beta-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.2.0 (hash-beta-1.2.0)
info: downloading component[..]
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn file_override_toolchain_err_handling() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let cwd = cx.config.current_dir();
    let toolchain_file = cwd.join("rust-toolchain");
    rustup::utils::raw::write_file(&toolchain_file, "beta").unwrap();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'beta-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.2.0 (hash-beta-1.2.0)
info: downloading component[..]
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rust-std-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn plus_override_toolchain_err_handling() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            ["rustc", "+beta", "--version"],
            &[("RUSTUP_AUTO_INSTALL", "0")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'beta-[HOST_TRIPLE]' is not installed
...
"#]])
        .is_err();
    cx.config
        .expect(["rustc", "+beta", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn bad_sha_on_manifest() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
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
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
info: this might indicate an issue with the third-party release server '[..]'
...
"#]])
        .is_err();
}

#[tokio::test]
async fn bad_manifest() {
    // issue #3851
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    // install some toolchain
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();

    let path = [
        "toolchains",
        &format!("nightly-{}", this_host_triple()),
        "lib",
        "rustlib",
        "multirust-channel-manifest.toml",
    ]
    .into_iter()
    .collect::<PathBuf>();

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
    cx.config
        .expect(["rustup", "check"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: could not parse manifest file: '[..]': [..]
...
"#]])
        .is_err();
}

#[tokio::test]
async fn bad_sha_on_installer() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
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
        .expect(["rustup", "default", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: component download failed for cargo-[HOST_TRIPLE]: checksum failed for '[..]', expected: '[..]', calculated: '[..]'
...
"#]])
        .is_err();
}

#[tokio::test]
async fn install_override_toolchain_from_channel() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_override_toolchain_from_archive() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config
        .expect(["rustup", "override", "add", "nightly-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-beta-1.1.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "stable-2015-01-01"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.0.0 (hash-stable-1.0.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn install_override_toolchain_from_version() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "override", "add", "1.1.0"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn override_overrides_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn multiple_overrides() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect(["rustup", "override", "add", "beta"])
            .await
            .is_ok();
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect(["rustup", "override", "add", "stable"])
            .await
            .is_ok();
    }

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect(["rustc", "--version"])
            .await
            .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
            .is_ok();
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect(["rustc", "--version"])
            .await
            .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
            .is_ok();
    }
}

// #316
#[cfg(windows)]
#[tokio::test]
async fn override_windows_root() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
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
    let cx = cx.change_dir(&PathBuf::from(&prefix));
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "override", "remove"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn change_override() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-beta-1.2.0)

"#]])
        .is_ok();
}

#[tokio::test]
async fn remove_override_no_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "override", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "remove"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: rustup could not choose a version of rustc to run, because one wasn't specified explicitly, and no default is configured.
...
"#]])
        .is_err();
}

#[tokio::test]
async fn remove_override_with_default() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let cx = cx.change_dir(tempdir.path());
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "add", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "override", "remove"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]]);
}

#[tokio::test]
async fn remove_override_with_multiple_overrides() {
    let mut cx = CliTestContext::new(Scenario::SimpleV2).await;
    let tempdir1 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tempdir2 = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect(["rustup", "override", "add", "beta"])
            .await
            .is_ok();
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect(["rustup", "override", "add", "stable"])
            .await
            .is_ok();
    }

    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .is_ok();

    {
        let cx = cx.change_dir(tempdir1.path());
        cx.config
            .expect(["rustup", "override", "remove"])
            .await
            .is_ok();
        cx.config
            .expect(["rustc", "--version"])
            .await
            .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
            .is_ok();
    }

    {
        let cx = cx.change_dir(tempdir2.path());
        cx.config
            .expect(["rustc", "--version"])
            .await
            .with_stdout(snapbox::str![[r#"
1.1.0 (hash-stable-1.1.0)

"#]])
            .is_ok();
    }
}

#[tokio::test]
async fn no_update_on_channel_when_date_has_not_changed() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"

  nightly-[HOST_TRIPLE] unchanged - 1.3.0 (hash-nightly-2)


"#]]);
}

#[tokio::test]
async fn update_on_channel_when_date_has_changed() {
    let cx = CliTestContext::new(Scenario::ArchivesV2).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]]);
    cx.config.set_current_dist_date("2015-01-02");
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]]);
}

#[tokio::test]
async fn run_command() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "run", "nightly", "rustc", "--version"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]]);
}

#[tokio::test]
async fn remove_toolchain_then_add_again() {
    // Issue brson/multirust #53
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "beta"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "toolchain", "remove", "beta"])
        .await
        .is_ok();
    cx.config.expect(["rustup", "update", "beta"]).await.is_ok();
    cx.config.expect(["rustc", "--version"]).await.is_ok();
}

#[tokio::test]
async fn upgrade_v1_to_v2() {
    let cx = CliTestContext::new(Scenario::Full).await;
    cx.config.set_current_dist_date("2015-01-01");
    // Delete the v2 manifest so the first day we install from the v1s
    fs::remove_file(
        cx.config
            .distdir
            .as_ref()
            .unwrap()
            .join("dist/channel-rust-nightly.toml.sha256"),
    )
    .unwrap();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
1.2.0 (hash-nightly-1)

"#]]);
    cx.config.set_current_dist_date("2015-01-02");
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]]);
}

#[tokio::test]
async fn upgrade_v2_to_v1() {
    let cx = CliTestContext::new(Scenario::Full).await;
    cx.config.set_current_dist_date("2015-01-01");
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config.set_current_dist_date("2015-01-02");
    fs::remove_file(
        cx.config
            .distdir
            .as_ref()
            .unwrap()
            .join("dist/channel-rust-nightly.toml.sha256"),
    )
    .unwrap();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: the server unexpectedly provided an obsolete version of the distribution manifest
...
"#]])
        .is_err();
}

#[tokio::test]
async fn list_targets_no_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            ["rustup", "target", "list", "--toolchain=nightly"],
            &[("RUSTUP_AUTO_INSTALL", "0")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' is not installed
...
"#]])
        .is_err();
}

#[tokio::test]
async fn set_auto_install_disable() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "set", "auto-install", "disable"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list", "--toolchain=nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' is not installed
...
"#]])
        .is_err();
    cx.config
        .expect_with_env(
            ["rustup", "target", "list", "--toolchain=nightly"],
            &[("RUSTUP_AUTO_INSTALL", "0")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' is not installed
...
"#]])
        .is_err();
    // The environment variable takes precedence over the setting.
    cx.config
        .expect_with_env(
            ["rustup", "target", "list", "--toolchain=nightly"],
            &[("RUSTUP_AUTO_INSTALL", "1")],
        )
        .await
        .is_ok();
}

#[tokio::test]
async fn list_targets_v1_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list", "--toolchain=nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not support components (v1 manifest)

"#]])
        .is_err();
}

#[tokio::test]
async fn list_targets_custom_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();
    let stable_path = cx
        .config
        .rustupdir
        .join("toolchains")
        .join(format!("stable-{}", this_host_triple()));
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "stuff",
            &stable_path.to_string_lossy(),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "+stuff", "target", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
[HOST_TRIPLE]

"#]]);
    cx.config
        .expect(["rustup", "+stuff", "target", "list"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
[HOST_TRIPLE] (installed)

"#]]);
}

#[tokio::test]
async fn list_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I]
...
"#]]);
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_II]
...
"#]]);
}

#[tokio::test]
async fn list_installed_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;

    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn add_target1() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        CROSS_ARCH1
    );
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn add_target2() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH2])
        .await
        .is_ok();
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        CROSS_ARCH2
    );
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn add_all_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", "all"])
        .await
        .is_ok();
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        CROSS_ARCH1
    );
    assert!(cx.config.rustupdir.has(path));
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        CROSS_ARCH2
    );
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn add_all_targets_fail() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1, "all", CROSS_ARCH2])
        .await
        .with_stderr(snapbox::str![[r#"
error: `rustup target add [CROSS_ARCH_I] all [CROSS_ARCH_II]` includes `all`

"#]])
        .is_err();
}

#[tokio::test]
async fn add_target_by_component_add() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .is_ok()
        .without_stdout(&format!("{CROSS_ARCH1} (installed)"));
    cx.config
        .expect([
            "rustup",
            "component",
            "add",
            &format!("rust-std-{CROSS_ARCH1}"),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I] (installed)
...
"#]]);
}

#[tokio::test]
async fn remove_target_by_component_remove() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I] (installed)
...
"#]]);
    cx.config
        .expect([
            "rustup",
            "component",
            "remove",
            &format!("rust-std-{CROSS_ARCH1}"),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .is_ok()
        .without_stdout(&format!("{CROSS_ARCH1} (installed)"));
}

#[tokio::test]
async fn add_target_no_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            [
                "rustup",
                "target",
                "add",
                CROSS_ARCH1,
                "--toolchain=nightly",
            ],
            &[("RUSTUP_AUTO_INSTALL", "0")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' is not installed
...
"#]])
        .is_err();
}

#[tokio::test]
async fn add_target_bogus() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", "bogus"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' does not support target 'bogus'
note: you can see a list of supported targets with `rustc --print=target-list`
note: if you are adding support for a new target to rustc itself, see https://rustc-dev-guide.rust-lang.org/building/new-target.html

"#]])
        .is_err();
}

#[tokio::test]
async fn add_target_unavailable() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", "mipsel-sony-psp"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' has no prebuilt artifacts available for target 'mipsel-sony-psp'
note: this may happen to a low-tier target as per https://doc.rust-lang.org/nightly/rustc/platform-support.html
note: you can find instructions on that page to build the target support from source

"#]])
        .is_err();
}

#[tokio::test]
async fn add_target_v1_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "target",
            "add",
            CROSS_ARCH1,
            "--toolchain=nightly",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not support components (v1 manifest)

"#]])
        .is_err();
}

#[tokio::test]
async fn add_target_custom_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "default-from-path", &path])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "default-from-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'default-from-path' does not support components

"#]])
        .is_err();
}

#[tokio::test]
async fn cannot_add_empty_named_custom_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "", &path])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: invalid value '' for '<TOOLCHAIN>': invalid toolchain name ''
...
"#]])
        .is_err();
}

#[tokio::test]
async fn add_target_again() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .with_stderr(snapbox::str![[r#"
info: component 'rust-std' for target '[CROSS_ARCH_I]' is up to date

"#]])
        .is_ok();
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        CROSS_ARCH1
    );
    assert!(cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn add_target_host() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", &trip])
        .await
        .is_ok();
}

#[tokio::test]
async fn remove_target() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", CROSS_ARCH1])
        .await
        .is_ok();
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
        this_host_triple(),
        CROSS_ARCH1
    );
    assert!(!cx.config.rustupdir.has(path));
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}/lib",
        this_host_triple(),
        CROSS_ARCH1
    );
    assert!(!cx.config.rustupdir.has(path));
    let path = format!(
        "toolchains/nightly-{}/lib/rustlib/{}",
        this_host_triple(),
        CROSS_ARCH1
    );
    assert!(!cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn remove_target_not_installed() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", CROSS_ARCH1])
        .await
        .extend_redactions([
            ("[HOST_TRIPLE]", this_host_triple().to_string()),
            ("[CROSS_ARCH_I]", CROSS_ARCH1.to_string()),
        ])
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' does not have target '[CROSS_ARCH_I]' installed
...
"#]])
        .is_err();
}

#[tokio::test]
async fn remove_target_no_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect_with_env(
            [
                "rustup",
                "target",
                "remove",
                CROSS_ARCH1,
                "--toolchain=nightly",
            ],
            &[("RUSTUP_AUTO_INSTALL", "0")],
        )
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' is not installed
...
"#]])
        .is_err();
}

#[tokio::test]
async fn remove_target_bogus() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", "bogus"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' does not have target 'bogus' installed
...
"#]])
        .is_err();
}

#[tokio::test]
async fn remove_target_v1_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV1).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "target",
            "remove",
            CROSS_ARCH1,
            "--toolchain=nightly",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not support components (v1 manifest)

"#]])
        .is_err();
}

#[tokio::test]
async fn remove_target_custom_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let path = cx.config.customdir.join("custom-1");
    let path = path.to_string_lossy();
    cx.config
        .expect(["rustup", "toolchain", "link", "default-from-path", &path])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "default-from-path"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", CROSS_ARCH1])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'default-from-path' does not support components

"#]])
        .is_err();
}

#[tokio::test]
async fn remove_target_again() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", CROSS_ARCH1])
        .await
        .extend_redactions([
            ("[HOST_TRIPLE]", this_host_triple().to_string()),
            ("[CROSS_ARCH_I]", CROSS_ARCH1.to_string()),
        ])
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' does not have target '[CROSS_ARCH_I]' installed
...
"#]])
        .is_err();
}

#[tokio::test]
async fn remove_target_host() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let host = this_host_triple();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", &host])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: removing the default host target; proc-macros and build scripts might no longer build
...
"#]])
        .is_ok();
    let path = format!("toolchains/nightly-{host}/lib/rustlib/{host}/lib/libstd.rlib");
    assert!(!cx.config.rustupdir.has(path));
    let path = format!("toolchains/nightly-{host}/lib/rustlib/{host}/lib");
    assert!(!cx.config.rustupdir.has(path));
    let path = format!("toolchains/nightly-{host}/lib/rustlib/{host}");
    assert!(!cx.config.rustupdir.has(path));
}

#[tokio::test]
async fn remove_target_last() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let host = this_host_triple();
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", &host])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: removing the last target; no build targets will be available
...
"#]])
        .is_ok();
}

#[tokio::test]
// Issue #304
async fn remove_target_missing_update_hash() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();

    let file_name = format!("nightly-{}", this_host_triple());
    fs::remove_file(cx.config.rustupdir.join("update-hashes").join(file_name)).unwrap();

    cx.config
        .expect(["rustup", "toolchain", "remove", "nightly"])
        .await
        .is_ok();
}

// Issue #1777
#[tokio::test]
async fn warn_about_and_remove_stray_hash() {
    let mut cx = CliTestContext::new(Scenario::None).await;
    let mut hash_path = cx.config.rustupdir.join("update-hashes");
    fs::create_dir_all(&hash_path).expect("Unable to make the update-hashes directory");
    hash_path.push(format!("nightly-{}", this_host_triple()));
    let mut file = fs::File::create(&hash_path).expect("Unable to open update-hash file");
    file.write_all(b"LEGITHASH")
        .expect("Unable to write update-hash");
    drop(file);

    let cx = cx.with_dist_dir(Scenario::SimpleV2);
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: removing stray hash found at '[..]/update-hashes/nightly-[HOST_TRIPLE]' in order to continue
...
"#]])
        .is_ok();
}

fn make_component_unavailable(config: &Config, name: &str, target: String) {
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

#[tokio::test]
async fn update_unavailable_std() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    make_component_unavailable(&cx.config, "rust-std", this_host_triple());
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: component 'rust-std' for target '[HOST_TRIPLE]' is unavailable for download for channel 'nightly'
...
"#]])
        .is_err();
}

#[tokio::test]
async fn add_missing_component() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    make_component_unavailable(&cx.config, "rls-preview", this_host_triple());
    cx.config
        .expect(["rustup", "toolchain", "add", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls-preview"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: component 'rls' for target '[HOST_TRIPLE]' is unavailable for download for channel 'nightly'
Sometimes not all components are available in any given nightly. 
...
"#]])
        .is_err();
    // Make sure the following pattern does not match,
    // thus addressing https://github.com/rust-lang/rustup/issues/3418.
    cx.config
        .expect(["rustup", "component", "add", "rls-preview"])
        .await
        .is_err()
        .without_stderr("If you don't need the component, you can remove it with:");
}

#[tokio::test]
async fn add_missing_component_toolchain() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    make_component_unavailable(&cx.config, "rust-std", this_host_triple());
    cx.config
        .expect(["rustup", "toolchain", "add", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: component 'rust-std' for target '[HOST_TRIPLE]' is unavailable for download for channel 'nightly'
Sometimes not all components are available in any given nightly.
If you don't need these components, you could try a minimal installation with:

    rustup toolchain add nightly --profile minimal

If you require these components, please install and use the latest successfully built version,
which you can find at <https://rust-lang.github.io/rustup-components-history>.

After determining the correct date, install it with a command such as:

    rustup toolchain install nightly-2018-12-27

Then you can use the toolchain with commands such as:

    cargo +nightly-2018-12-27 build
...
"#]])
        .is_err();
}

#[tokio::test]
async fn update_removed_component_toolchain() {
    let cx = CliTestContext::new(Scenario::RemovedRls).await;
    cx.config.set_current_dist_date("2024-05-01");
    cx.config
        .expect(["rustup", "default", "stable"])
        .await
        .is_ok();

    // Install `rls` on the first day.
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.78.0 (hash-stable-1.78.0)

"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config.expect_component_executable("rls").await;

    // `rls` is missing on the second day.
    cx.config.set_current_dist_date("2024-06-15");

    // An update at this time should inform the user of an unavailable component.
    cx.config
        .expect(["rustup", "update", "stable"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: component 'rls' for target '[HOST_TRIPLE]' is unavailable for download for channel 'stable'
One or many components listed above might have been permanently removed from newer versions
of the official Rust distribution due to deprecation.
...
"#]])
        .is_err();

    // We're still stuck with the old version.
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.78.0 (hash-stable-1.78.0)

"#]])
        .is_ok();
    cx.config.expect_component_executable("rls").await;
}

#[tokio::test]
async fn update_unavailable_force() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "component",
            "add",
            "rls",
            "--toolchain",
            "nightly",
        ])
        .await
        .is_ok();
    make_component_unavailable(&cx.config, "rls-preview", trip);
    cx.config
        .expect(["rustup", "update", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: component 'rls' for target '[HOST_TRIPLE]' is unavailable for download for channel 'nightly'
...
"#]])
        .is_err();
    cx.config
        .expect(["rustup", "update", "nightly", "--force"])
        .await
        .is_ok();
}

#[tokio::test]
async fn add_component_suggest_best_match() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rsl"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not contain component 'rsl' for target '[HOST_TRIPLE]'; did you mean 'rls'?

"#]])
        .is_err();
    cx.config
        .expect(["rustup", "component", "add", "rsl-preview"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not contain component 'rsl-preview' for target '[HOST_TRIPLE]'; did you mean 'rls-preview'?

"#]])
        .is_err();
    cx.config
        .expect(["rustup", "component", "add", "rustd"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not contain component 'rustd' for target '[HOST_TRIPLE]'; did you mean 'rustc'?

"#]])
        .is_err();
    cx.config
        .expect(["rustup", "component", "add", "potato"])
        .await
        .is_err()
        .without_stderr("did you mean");
}

#[tokio::test]
async fn remove_component_suggest_best_match() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "remove", "rsl"])
        .await
        .is_err()
        .without_stderr("did you mean 'rls'?");
    cx.config
        .expect(["rustup", "component", "add", "rls"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "remove", "rsl"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not contain component 'rsl' for target '[HOST_TRIPLE]'; did you mean 'rls'?

"#]])
        .is_err();
    cx.config
        .expect(["rustup", "component", "add", "rls-preview"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "add", "rsl-preview"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not contain component 'rsl-preview' for target '[HOST_TRIPLE]'; did you mean 'rls-preview'?

"#]])
        .is_err();
    cx.config
        .expect(["rustup", "component", "remove", "rustd"])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not contain component 'rustd' for target '[HOST_TRIPLE]'; did you mean 'rustc'?

"#]])
        .is_err();
}

#[tokio::test]
async fn add_target_suggest_best_match() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "add", &format!("{CROSS_ARCH1}a")[..]])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: toolchain 'nightly-[HOST_TRIPLE]' does not support target '[CROSS_ARCH_I]a'; did you mean '[CROSS_ARCH_I]'?
...
"#]])
        .is_err();
    cx.config
        .expect(["rustup", "target", "add", "potato"])
        .await
        .is_err()
        .without_stderr("did you mean");
}

#[tokio::test]
async fn remove_target_suggest_best_match() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", &format!("{CROSS_ARCH1}a")[..]])
        .await
        .is_err()
        .without_stderr(&format!("did you mean '{CROSS_ARCH1}'"));
    cx.config
        .expect(["rustup", "target", "add", CROSS_ARCH1])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "target", "remove", &format!("{CROSS_ARCH1}a")[..]])
        .await
        .with_stderr(snapbox::str![[r#"
error: toolchain 'nightly-[HOST_TRIPLE]' does not have target '[CROSS_ARCH_I]a' installed; did you mean '[CROSS_ARCH_I]'?


"#]])
        .is_err();
}

#[tokio::test]
async fn target_list_ignores_unavailable_targets() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    let target_list = &["rustup", "target", "list"];
    cx.config
        .expect(target_list)
        .await
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I]
...
"#]])
        .is_ok();
    make_component_unavailable(&cx.config, "rust-std", CROSS_ARCH1.to_owned());
    cx.config
        .expect(["rustup", "update", "nightly", "--force"])
        .await
        .is_ok();
    cx.config
        .expect(target_list)
        .await
        .is_ok()
        .without_stdout(CROSS_ARCH1);
}

#[tokio::test]
async fn install_with_components() {
    async fn go(comp_args: &[&str]) {
        let mut args = vec!["rustup", "toolchain", "install", "nightly"];
        args.extend_from_slice(comp_args);

        let cx = CliTestContext::new(Scenario::SimpleV2).await;
        cx.config.expect(&args).await.is_ok();
        cx.config
            .expect(["rustup", "component", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
...
rust-src (installed)
...
"#]])
            .is_ok();
        cx.config
            .expect(["rustup", "component", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
...
rust-analysis-[HOST_TRIPLE] (installed)
...
"#]])
            .is_ok();
    }

    go(&["-c", "rust-src", "-c", "rust-analysis"]).await;
    go(&["-c", "rust-src,rust-analysis"]).await;
}

#[tokio::test]
async fn install_with_targets() {
    async fn go(comp_args: &[&str]) {
        let mut args = vec!["rustup", "toolchain", "install", "nightly"];
        args.extend_from_slice(comp_args);

        let cx = CliTestContext::new(Scenario::SimpleV2).await;
        cx.config.expect(&args).await.is_ok();
        cx.config
            .expect(["rustup", "target", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I] (installed)
...
"#]])
            .is_ok();
        cx.config
            .expect(["rustup", "target", "list"])
            .await
            .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_II] (installed)
...
"#]])
            .is_ok();
    }

    go(&["-t", CROSS_ARCH1, "-t", CROSS_ARCH2]).await;
    go(&["-t", &format!("{CROSS_ARCH1},{CROSS_ARCH2}")]).await;
}

#[tokio::test]
async fn install_with_component_and_target() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "default", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "install",
            "nightly",
            "-c",
            "rls",
            "-t",
            CROSS_ARCH1,
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
rls-[HOST_TRIPLE] (installed)
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "target", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
[CROSS_ARCH_I] (installed)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn test_warn_if_complete_profile_is_used() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect(["rustup", "set", "auto-self-update", "enable"])
        .await
        .is_ok();
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "install",
            "--profile",
            "complete",
            "stable",
        ])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: downloading with complete profile isn't recommended unless you are a developer of the rust language
...
"#]])
        .is_err();
}

#[tokio::test]
async fn test_complete_profile_skips_missing_when_forced() {
    let cx = CliTestContext::new(Scenario::UnavailableRls).await;
    cx.config.set_current_dist_date("2015-01-01");

    cx.config
        .expect(["rustup", "set", "profile", "complete"])
        .await
        .is_ok();
    // First try and install without force
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: component 'rls' for target '[HOST_TRIPLE]' is unavailable for download for channel 'nightly'
...
"#]])
        .is_err();
    // Now try and force
    cx.config
        .expect(["rustup", "toolchain", "install", "--force", "nightly"])
        .await
        .with_stderr(snapbox::str![[r#"
...
warn: Force-skipping unavailable component 'rls-[HOST_TRIPLE]'
...
"#]])
        .is_ok();

    // Ensure that the skipped component (rls) is not installed
    cx.config
        .expect(["rustup", "component", "list"])
        .await
        .with_stdout(snapbox::str![[r#"
...
cargo-[HOST_TRIPLE] (installed)
...
rust-docs-[HOST_TRIPLE] (installed)
...
"#]])
        .is_ok();
}

#[tokio::test]
async fn run_with_install_flag_against_unavailable_component() {
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    let trip = this_host_triple();
    make_component_unavailable(&cx.config, "rust-std", trip);
    cx.config
        .expect([
            "rustup",
            "run",
            "--install",
            "nightly",
            "rustc",
            "--version",
        ])
        .await
        .with_stdout(snapbox::str![[r#"
1.3.0 (hash-nightly-2)

"#]])
        .with_stderr(snapbox::str![[r#"
info: syncing channel updates for 'nightly-[HOST_TRIPLE]'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
warn: Force-skipping unavailable component 'rust-std-[HOST_TRIPLE]'
info: downloading component[..]
...
"#]])
        .is_ok();
    cx.config
        .expect(["rustup", "+nightly", "component", "list", "--installed"])
        .await
        .is_ok()
        .with_stdout(snapbox::str![[r#"
cargo-[HOST_TRIPLE]
rust-docs-[HOST_TRIPLE]
rustc-[HOST_TRIPLE]

"#]]);
}

#[tokio::test]
async fn install_allow_downgrade() {
    let cx = CliTestContext::new(Scenario::MissingComponent).await;

    // this dist has no rls and there is no newer one
    cx.config.set_current_dist_date("2019-09-14");
    cx.config
        .expect(["rustup", "toolchain", "install", "nightly"])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.37.0 (hash-nightly-3)

"#]])
        .is_ok();
    cx.config.expect_component_not_executable("rls").await;

    cx.config
        .expect(["rustup", "toolchain", "install", "nightly", "-c", "rls"])
        .await
        .with_stderr(snapbox::str![[r#"
...
error: component 'rls' for target '[HOST_TRIPLE]' is unavailable for download for channel 'nightly'
...
"#]])
        .is_err();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.37.0 (hash-nightly-3)

"#]])
        .is_ok();
    cx.config.expect_component_not_executable("rls").await;

    cx.config
        .expect([
            "rustup",
            "toolchain",
            "install",
            "nightly",
            "-c",
            "rls",
            "--allow-downgrade",
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustc", "--version"])
        .await
        .with_stdout(snapbox::str![[r#"
1.37.0 (hash-nightly-2)

"#]])
        .is_ok();
    cx.config.expect_component_executable("rls").await;
}

#[tokio::test]
async fn regression_2601() {
    // We're checking that we don't regress per #2601
    let cx = CliTestContext::new(Scenario::SimpleV2).await;
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "install",
            "--profile",
            "minimal",
            "nightly",
            "--component",
            "rust-src",
        ])
        .await
        .is_ok();
    // The bug exposed in #2601 was that the above would end up installing
    // rust-src-$ARCH which would then have to be healed on the following
    // command, resulting in a reinstallation.
    cx.config
        .expect(["rustup", "component", "add", "rust-src"])
        .await
        .with_stderr(snapbox::str![[r#"
info: component 'rust-src' is up to date

"#]])
        .is_ok();
}
