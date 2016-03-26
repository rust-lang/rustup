//! Test cases for new rustup UI

extern crate multirust_dist;
extern crate multirust_utils;
extern crate multirust_mock;
extern crate tempdir;

use multirust_mock::clitools::{self, Config, Scenario,
                               expect_ok, expect_ok_ex,
                               expect_stdout_ok,
                               expect_err_ex,
                               set_current_dist_date};

pub fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        f(config);
    });
}

#[test]
fn rustup_stable() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable"]);
        set_current_dist_date(config, "2015-01-02");
        expect_ok_ex(config, &["rustup", "--no-self-update"],
r"
  stable updated: 1.1.0 (hash-s-2)

",
r"info: syncing channel updates for 'stable'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
");
    });
}

#[test]
fn rustup_stable_no_change() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable"]);
        expect_ok_ex(config, &["rustup", "--no-self-update"],
r"
  stable unchanged: 1.0.0 (hash-s-1)

",
r"info: syncing channel updates for 'stable'
");
    });
}

#[test]
fn rustup_all_channels() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable"]);
        expect_ok(config, &["multirust", "update", "beta"]);
        expect_ok(config, &["multirust", "update", "nightly"]);
        set_current_dist_date(config, "2015-01-02");
        expect_ok_ex(config, &["rustup", "--no-self-update"],
r"
   stable updated: 1.1.0 (hash-s-2)
     beta updated: 1.2.0 (hash-b-2)
  nightly updated: 1.3.0 (hash-n-2)

",
r"info: syncing channel updates for 'stable'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: syncing channel updates for 'beta'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: syncing channel updates for 'nightly'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
");
    })
}

#[test]
fn rustup_some_channels_up_to_date() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable"]);
        expect_ok(config, &["multirust", "update", "beta"]);
        expect_ok(config, &["multirust", "update", "nightly"]);
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["multirust", "update", "beta"]);
        expect_ok_ex(config, &["rustup", "--no-self-update"],
r"
   stable updated: 1.1.0 (hash-s-2)
   beta unchanged: 1.2.0 (hash-b-2)
  nightly updated: 1.3.0 (hash-n-2)

",
r"info: syncing channel updates for 'stable'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: syncing channel updates for 'beta'
info: syncing channel updates for 'nightly'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
");
    })
}

#[test]
fn rustup_no_channels() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "stable"]);
        expect_ok(config, &["multirust", "remove-toolchain", "stable"]);
        expect_ok_ex(config, &["rustup", "--no-self-update"],
r"",
r"info: no updatable toolchains installed
");
    })
}

#[test]
fn default() {
    setup(&|config| {
        expect_ok_ex(config, &["rustup", "default", "nightly"],
r"
  nightly installed: 1.3.0 (hash-n-2)

",
r"info: syncing channel updates for 'nightly'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: default toolchain set to 'nightly'
");
    });
}

#[test]
fn add_target() {
    setup(&|config| {
        let path = format!("toolchains/nightly/lib/rustlib/{}/lib/libstd.rlib",
                           clitools::CROSS_ARCH1);
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add",
                            clitools::CROSS_ARCH1]);
        assert!(config.rustupdir.join(path).exists());
    });
}

#[test]
fn remove_target() {
    setup(&|config| {
        let ref path = format!("toolchains/nightly/lib/rustlib/{}/lib/libstd.rlib",
                           clitools::CROSS_ARCH1);
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add",
                            clitools::CROSS_ARCH1]);
        assert!(config.rustupdir.join(path).exists());
        expect_ok(config, &["rustup", "target", "remove",
                            clitools::CROSS_ARCH1]);
        assert!(!config.rustupdir.join(path).exists());
    });
}

#[test]
fn list_targets() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustup", "target", "list"],
                         clitools::CROSS_ARCH1);
    });
}

#[test]
fn link() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["rustup", "toolchain", "link", "custom",
                            &path]);
        expect_ok(config, &["rustup", "default", "custom"]);
        expect_stdout_ok(config, &["rustc", "--version"],
                         "hash-c-1");
    });
}

#[test]
fn show_toolchain_none() {
    setup(&|config| {
        expect_ok_ex(config, &["rustup", "show"],
r"no active toolchain
",
r"");
    });
}

#[test]
fn show_toolchain_default() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok_ex(config, &["rustup", "show"],
r"nightly (default toolchain)
",
r"");
    });
}

#[test]
fn show_toolchain_override() {
    setup(&|config| {
        let cwd = ::std::env::current_dir().unwrap();
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_ok_ex(config, &["rustup", "show"],
&format!(r"nightly (directory override for '{}')
", cwd.display()),
r"");
    });
}

#[test]
fn show_toolchain_override_not_installed() {
    setup(&|config| {
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "nightly"]);
        // I'm not sure this should really be erroring when the toolchain
        // is not installed; just capturing the behavior.
        expect_err_ex(config, &["rustup", "show"],
r"",
r"error: toolchain 'nightly' is not installed
");
    });
}

#[test]
fn show_toolchain_env() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        let mut cmd = clitools::cmd(config, "rustup", &["show"]);
        clitools::env(config, &mut cmd);
        cmd.env("MULTIRUST_TOOLCHAIN", "nightly");
        let out = cmd.output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert!(stdout == "nightly (environment override by MULTIRUST_TOOLCHAIN)\n");
    });
}

#[test]
fn show_toolchain_env_not_installed() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["show"]);
        clitools::env(config, &mut cmd);
        cmd.env("MULTIRUST_TOOLCHAIN", "nightly");
        let out = cmd.output().unwrap();
        // I'm not sure this should really be erroring when the toolchain
        // is not installed; just capturing the behavior.
        assert!(!out.status.success());
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(stderr == "error: toolchain 'nightly' is not installed\n");
    });
}
