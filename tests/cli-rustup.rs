//! Test cases for new rustup UI

extern crate rustup_dist;
extern crate rustup_mock;
extern crate rustup_utils;
extern crate tempdir;

use rustup_mock::clitools::{
    self, expect_err, expect_ok, expect_ok_ex, expect_stderr_ok, expect_stdout_ok,
    set_current_dist_date, this_host_triple, Config, Scenario,
};
use rustup_utils::raw;
use std::env::consts::EXE_SUFFIX;
use std::fs;
use std::path::MAIN_SEPARATOR;
use std::process;

macro_rules! for_host {
    ($s: expr) => {
        &format!($s, this_host_triple())
    };
}

pub fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        f(config);
    });
}

#[test]
fn rustup_stable() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        set_current_dist_date(config, "2015-01-02");
        expect_ok_ex(
            config,
            &["rustup", "update", "--no-self-update"],
            for_host!(
                r"
  stable-{0} updated - 1.1.0 (hash-s-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: removing component 'rust-std'
info: removing component 'rustc'
info: removing component 'cargo'
info: removing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
"
            ),
        );
    });
}

#[test]
fn rustup_stable_no_change() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_ok_ex(
            config,
            &["rustup", "update", "--no-self-update"],
            for_host!(
                r"
  stable-{0} unchanged - 1.0.0 (hash-s-1)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
"
            ),
        );
    });
}

#[test]
fn rustup_all_channels() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        set_current_dist_date(config, "2015-01-02");
        expect_ok_ex(
            config,
            &["rustup", "update", "--no-self-update"],
            for_host!(
                r"
   stable-{0} updated - 1.1.0 (hash-s-2)
     beta-{0} updated - 1.2.0 (hash-b-2)
  nightly-{0} updated - 1.3.0 (hash-n-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: removing component 'rust-std'
info: removing component 'rustc'
info: removing component 'cargo'
info: removing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: syncing channel updates for 'beta-{0}'
info: latest update on 2015-01-02, rust version 1.2.0
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: removing component 'rust-std'
info: removing component 'rustc'
info: removing component 'cargo'
info: removing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: removing component 'rust-std'
info: removing component 'rustc'
info: removing component 'cargo'
info: removing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
"
            ),
        );
    })
}

#[test]
fn rustup_some_channels_up_to_date() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_ok_ex(
            config,
            &["rustup", "update", "--no-self-update"],
            for_host!(
                r"
   stable-{0} updated - 1.1.0 (hash-s-2)
   beta-{0} unchanged - 1.2.0 (hash-b-2)
  nightly-{0} updated - 1.3.0 (hash-n-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: removing component 'rust-std'
info: removing component 'rustc'
info: removing component 'cargo'
info: removing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: syncing channel updates for 'beta-{0}'
info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: removing component 'rust-std'
info: removing component 'rustc'
info: removing component 'cargo'
info: removing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
"
            ),
        );
    })
}

#[test]
fn rustup_no_channels() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "stable"]);
        expect_ok_ex(
            config,
            &["rustup", "update", "--no-self-update"],
            r"",
            r"info: no updatable toolchains installed
",
        );
    })
}

#[test]
fn default() {
    setup(&|config| {
        expect_ok_ex(
            config,
            &["rustup", "default", "nightly"],
            for_host!(
                r"
  nightly-{0} installed - 1.3.0 (hash-n-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: default toolchain set to 'nightly-{0}'
"
            ),
        );
    });
}

#[test]
fn rustup_xz() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_stderr_ok(
            config,
            &[
                "rustup",
                "--verbose",
                "update",
                "nightly",
                "--no-self-update",
            ],
            for_host!(r"dist/2015-01-01/rust-std-nightly-{0}.tar.xz"),
        );
    });
}

#[test]
fn add_target() {
    setup(&|config| {
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            &this_host_triple(),
            clitools::CROSS_ARCH1
        );
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        assert!(config.rustupdir.join(path).exists());
    });
}

#[test]
fn remove_target() {
    setup(&|config| {
        let ref path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            &this_host_triple(),
            clitools::CROSS_ARCH1
        );
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        assert!(config.rustupdir.join(path).exists());
        expect_ok(
            config,
            &["rustup", "target", "remove", clitools::CROSS_ARCH1],
        );
        assert!(!config.rustupdir.join(path).exists());
    });
}

#[test]
fn add_remove_multiple_targets() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(
            config,
            &[
                "rustup",
                "target",
                "add",
                clitools::CROSS_ARCH1,
                clitools::CROSS_ARCH2,
            ],
        );
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            &this_host_triple(),
            clitools::CROSS_ARCH1
        );
        assert!(config.rustupdir.join(path).exists());
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            &this_host_triple(),
            clitools::CROSS_ARCH2
        );
        assert!(config.rustupdir.join(path).exists());

        expect_ok(
            config,
            &[
                "rustup",
                "target",
                "remove",
                clitools::CROSS_ARCH1,
                clitools::CROSS_ARCH2,
            ],
        );
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            &this_host_triple(),
            clitools::CROSS_ARCH1
        );
        assert!(!config.rustupdir.join(path).exists());
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            &this_host_triple(),
            clitools::CROSS_ARCH2
        );
        assert!(!config.rustupdir.join(path).exists());
    });
}

#[test]
fn list_targets() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustup", "target", "list"], clitools::CROSS_ARCH1);
    });
}

#[test]
fn add_target_explicit() {
    setup(&|config| {
        let path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            &this_host_triple(),
            clitools::CROSS_ARCH1
        );
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok(
            config,
            &[
                "rustup",
                "target",
                "add",
                "--toolchain",
                "nightly",
                clitools::CROSS_ARCH1,
            ],
        );
        assert!(config.rustupdir.join(path).exists());
    });
}

#[test]
fn remove_target_explicit() {
    setup(&|config| {
        let ref path = format!(
            "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
            &this_host_triple(),
            clitools::CROSS_ARCH1
        );
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok(
            config,
            &[
                "rustup",
                "target",
                "add",
                "--toolchain",
                "nightly",
                clitools::CROSS_ARCH1,
            ],
        );
        assert!(config.rustupdir.join(path).exists());
        expect_ok(
            config,
            &[
                "rustup",
                "target",
                "remove",
                "--toolchain",
                "nightly",
                clitools::CROSS_ARCH1,
            ],
        );
        assert!(!config.rustupdir.join(path).exists());
    });
}

#[test]
fn list_targets_explicit() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "target", "list", "--toolchain", "nightly"],
            clitools::CROSS_ARCH1,
        );
    });
}

#[test]
fn link() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["rustup", "toolchain", "link", "custom", &path]);
        expect_ok(config, &["rustup", "default", "custom"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-c-1");
        expect_stdout_ok(config, &["rustup", "show"], "custom (default)");
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, &["rustup", "show"], "custom");
    });
}

// Issue #809. When we call the fallback cargo, when it in turn invokes
// "rustc", that rustc should actually be the rustup proxy, not the toolchain rustc.
// That way the proxy can pick the correct toolchain.
#[test]
fn fallback_cargo_calls_correct_rustc() {
    setup(&|config| {
        // Hm, this is the _only_ test that assumes that toolchain proxies
        // exist in CARGO_HOME. Adding that proxy here.
        let ref rustup_path = config.exedir.join(format!("rustup{}", EXE_SUFFIX));
        let ref cargo_bin_path = config.cargodir.join("bin");
        fs::create_dir_all(cargo_bin_path).unwrap();
        let ref rustc_path = cargo_bin_path.join(format!("rustc{}", EXE_SUFFIX));
        fs::hard_link(rustup_path, rustc_path).unwrap();

        // Install a custom toolchain and a nightly toolchain for the cargo fallback
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        expect_ok(config, &["rustup", "toolchain", "link", "custom", &path]);
        expect_ok(config, &["rustup", "default", "custom"]);
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-c-1");
        expect_stdout_ok(config, &["cargo", "--version"], "hash-n-2");

        assert!(rustc_path.exists());

        // Here --call-rustc tells the mock cargo bin to exec `rustc --version`.
        // We should be ultimately calling the custom rustc, according to the
        // RUSTUP_TOOLCHAIN variable set by the original "cargo" proxy, and
        // interpreted by the nested "rustc" proxy.
        expect_stdout_ok(config, &["cargo", "--call-rustc"], "hash-c-1");
    });
}

#[test]
fn show_toolchain_none() {
    setup(&|config| {
        expect_ok_ex(
            config,
            &["rustup", "show"],
            for_host!(
                r"Default host: {0}

no active toolchain
"
            ),
            r"",
        );
    });
}

#[test]
fn show_toolchain_default() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok_ex(
            config,
            &["rustup", "show"],
            for_host!(
                r"Default host: {0}

nightly-{0} (default)
1.3.0 (hash-n-2)
"
            ),
            r"",
        );
    });
}

#[test]
fn show_multiple_toolchains() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_ok_ex(
            config,
            &["rustup", "show"],
            for_host!(
                r"Default host: {0}

installed toolchains
--------------------

stable-{0}
nightly-{0} (default)

active toolchain
----------------

nightly-{0} (default)
1.3.0 (hash-n-2)

"
            ),
            r"",
        );
    });
}

#[test]
fn show_multiple_targets() {
    // Using the MULTI_ARCH1 target doesn't work on i686 linux
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86") {
        return;
    }

    clitools::setup(Scenario::MultiHost, &|config| {
        expect_ok(
            config,
            &[
                "rustup",
                "default",
                &format!("nightly-{}", clitools::MULTI_ARCH1),
            ],
        );
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH2]);
        expect_ok_ex(
            config,
            &["rustup", "show"],
            &format!(
                r"Default host: {2}

installed targets for active toolchain
--------------------------------------

{1}
{0}

active toolchain
----------------

nightly-{0} (default)
1.3.0 (xxxx-n-2)

",
                clitools::MULTI_ARCH1,
                clitools::CROSS_ARCH2,
                this_host_triple()
            ),
            r"",
        );
    });
}

#[test]
fn show_multiple_toolchains_and_targets() {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86") {
        return;
    }

    clitools::setup(Scenario::MultiHost, &|config| {
        expect_ok(
            config,
            &[
                "rustup",
                "default",
                &format!("nightly-{}", clitools::MULTI_ARCH1),
            ],
        );
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH2]);
        expect_ok(
            config,
            &[
                "rustup",
                "update",
                &format!("stable-{}", clitools::MULTI_ARCH1),
                "--no-self-update",
            ],
        );
        expect_ok_ex(
            config,
            &["rustup", "show"],
            &format!(
                r"Default host: {2}

installed toolchains
--------------------

stable-{0}
nightly-{0} (default)

installed targets for active toolchain
--------------------------------------

{1}
{0}

active toolchain
----------------

nightly-{0} (default)
1.3.0 (xxxx-n-2)

",
                clitools::MULTI_ARCH1,
                clitools::CROSS_ARCH2,
                this_host_triple()
            ),
            r"",
        );
    });
}

#[test]
fn list_default_toolchain() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok_ex(
            config,
            &["rustup", "toolchain", "list"],
            for_host!(
                r"nightly-{0} (default)
"
            ),
            r"",
        );
    });
}

#[test]
#[ignore(windows)] // FIXME Windows shows UNC paths
fn show_toolchain_override() {
    setup(&|config| {
        let cwd = config.current_dir();
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_ok_ex(
            config,
            &["rustup", "show"],
            &format!(
                r"Default host: {0}

nightly-{0} (directory override for '{1}')
1.3.0 (hash-n-2)
",
                this_host_triple(),
                cwd.display()
            ),
            r"",
        );
    });
}

#[test]
#[ignore(windows)] // FIXME Windows shows UNC paths
fn show_toolchain_toolchain_file_override() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(config, &["rustup", "toolchain", "install", "nightly"]);

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");

        raw::write_file(&toolchain_file, "nightly").unwrap();

        expect_ok_ex(
            config,
            &["rustup", "show"],
            &format!(
                r"Default host: {0}

installed toolchains
--------------------

stable-{0} (default)
nightly-{0}

active toolchain
----------------

nightly-{0} (overridden by '{1}')
1.3.0 (hash-n-2)

",
                this_host_triple(),
                toolchain_file.display()
            ),
            r"",
        );
    });
}

#[test]
#[ignore(windows)] // FIXME Windows shows UNC paths
fn show_toolchain_version_nested_file_override() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(config, &["rustup", "toolchain", "install", "nightly"]);

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");

        raw::write_file(&toolchain_file, "nightly").unwrap();

        let subdir = cwd.join("foo");

        fs::create_dir_all(&subdir).unwrap();
        config.change_dir(&subdir, &|| {
            expect_ok_ex(
                config,
                &["rustup", "show"],
                &format!(
                    r"Default host: {0}

installed toolchains
--------------------

stable-{0} (default)
nightly-{0}

active toolchain
----------------

nightly-{0} (overridden by '{1}')
1.3.0 (hash-n-2)

",
                    this_host_triple(),
                    toolchain_file.display()
                ),
                r"",
            );
        });
    });
}

#[test]
#[ignore(windows)] // FIXME Windows shows UNC paths
fn show_toolchain_toolchain_file_override_not_installed() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");

        raw::write_file(&toolchain_file, "nightly").unwrap();

        // I'm not sure this should really be erroring when the toolchain
        // is not installed; just capturing the behavior.
        let mut cmd = clitools::cmd(config, "rustup", &["show"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(stderr.starts_with("error: override toolchain 'nightly' is not installed"));
        assert!(stderr.contains(&format!(
            "the toolchain file at '{}' specifies an uninstalled toolchain",
            toolchain_file.display()
        )));
    });
}

#[test]
fn show_toolchain_override_not_installed() {
    setup(&|config| {
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_ok(config, &["rustup", "toolchain", "remove", "nightly"]);
        let mut cmd = clitools::cmd(config, "rustup", &["show"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8(out.stdout).unwrap();
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(!stdout.contains("not a directory"));
        assert!(!stdout.contains("is not installed"));
        assert!(stderr.contains("info: installing component 'rustc'"));
    });
}

#[test]
fn show_toolchain_env() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        let mut cmd = clitools::cmd(config, "rustup", &["show"]);
        clitools::env(config, &mut cmd);
        cmd.env("RUSTUP_TOOLCHAIN", "nightly");
        let out = cmd.output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert_eq!(
            &stdout,
            for_host!(
                r"Default host: {0}

nightly-{0} (environment override by RUSTUP_TOOLCHAIN)
1.3.0 (hash-n-2)
"
            )
        );
    });
}

#[test]
fn show_toolchain_env_not_installed() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["show"]);
        clitools::env(config, &mut cmd);
        cmd.env("RUSTUP_TOOLCHAIN", "nightly");
        let out = cmd.output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8(out.stdout).unwrap();
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(!stdout.contains("is not installed"));
        assert!(stderr.contains("info: installing component 'rustc'"));
    });
}

#[test]
fn show_active_toolchain() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok_ex(
            config,
            &["rustup", "show", "active-toolchain"],
            for_host!(
                r"nightly-{0}
"
            ),
            r"",
        );
    });
}

#[test]
fn show_active_toolchain_none() {
    setup(&|config| {
        expect_ok_ex(config, &["rustup", "show", "active-toolchain"], r"", r"");
    });
}

// #846
#[test]
fn set_default_host() {
    setup(&|config| {
        expect_ok(
            config,
            &["rustup", "set", "default-host", &this_host_triple()],
        );
        expect_stdout_ok(config, &["rustup", "show"], for_host!("Default host: {0}"));
    });
}

// #846
#[test]
fn set_default_host_invalid_triple() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "set", "default-host", "foo"],
            "Invalid host triple",
        );
    });
}

// #422
#[test]
fn update_doesnt_update_non_tracking_channels() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(
            config,
            &["rustup", "update", "nightly-2015-01-01", "--no-self-update"],
        );
        let mut cmd = clitools::cmd(config, "rustup", &["update"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(!stderr.contains(for_host!(
            "syncing channel updates for 'nightly-2015-01-01-{}'"
        )));
    });
}

#[test]
fn toolchain_install_is_like_update() {
    setup(&|config| {
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
        );
        expect_stdout_ok(
            config,
            &["rustup", "run", "nightly", "rustc", "--version"],
            "hash-n-2",
        );
    });
}

#[test]
fn toolchain_install_is_like_update_except_that_bare_install_is_an_error() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "toolchain", "install", "--no-self-update"],
            "arguments were not provided",
        );
    });
}

#[test]
fn toolchain_update_is_like_update() {
    setup(&|config| {
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "update",
                "nightly",
                "--no-self-update",
            ],
        );
        expect_stdout_ok(
            config,
            &["rustup", "run", "nightly", "rustc", "--version"],
            "hash-n-2",
        );
    });
}

#[test]
fn toolchain_uninstall_is_like_uninstall() {
    setup(&|config| {
        expect_ok(config, &["rustup", "uninstall", "nightly"]);
        let mut cmd = clitools::cmd(config, "rustup", &["show"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(out.status.success());
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert!(!stdout.contains(for_host!("'nightly-2015-01-01-{}'")));
    });
}

#[test]
fn toolchain_update_is_like_update_except_that_bare_install_is_an_error() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "toolchain", "update"],
            "arguments were not provided",
        );
    });
}

#[test]
fn proxy_toolchain_shorthand() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "update",
                "nightly",
                "--no-self-update",
            ],
        );
        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");
        expect_stdout_ok(config, &["rustc", "+stable", "--version"], "hash-s-2");
        expect_stdout_ok(config, &["rustc", "+nightly", "--version"], "hash-n-2");
    });
}

#[test]
fn add_component() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(config, &["rustup", "component", "add", "rust-src"]);
        let path = format!(
            "toolchains/stable-{}/lib/rustlib/src/rust-src/foo.rs",
            this_host_triple()
        );
        let path = config.rustupdir.join(path);
        assert!(path.exists());
    });
}

#[test]
fn remove_component() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(config, &["rustup", "component", "add", "rust-src"]);
        let path = format!(
            "toolchains/stable-{}/lib/rustlib/src/rust-src/foo.rs",
            this_host_triple()
        );
        let path = config.rustupdir.join(path);
        assert!(path.exists());
        expect_ok(config, &["rustup", "component", "remove", "rust-src"]);
        assert!(!path.parent().unwrap().exists());
    });
}

#[test]
fn add_remove_multiple_components() {
    let files = [
        "lib/rustlib/src/rust-src/foo.rs".to_owned(),
        format!("lib/rustlib/{}/analysis/libfoo.json", this_host_triple()),
    ];

    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(
            config,
            &["rustup", "component", "add", "rust-src", "rust-analysis"],
        );
        for file in &files {
            let path = format!("toolchains/nightly-{}/{}", this_host_triple(), file);
            let path = config.rustupdir.join(path);
            assert!(path.exists());
        }
        expect_ok(
            config,
            &["rustup", "component", "remove", "rust-src", "rust-analysis"],
        );
        for file in &files {
            let path = format!("toolchains/nightly-{}/{}", this_host_triple(), file);
            let path = config.rustupdir.join(path);
            assert!(!path.parent().unwrap().exists());
        }
    });
}

// Run without setting RUSTUP_HOME, with setting HOME and USERPROFILE
fn run_no_home(config: &Config, args: &[&str], env: &[(&str, &str)]) -> process::Output {
    let home_dir_str = &format!("{}", config.homedir.display());
    let mut cmd = clitools::cmd(config, args[0], &args[1..]);
    clitools::env(config, &mut cmd);
    cmd.env_remove("RUSTUP_HOME");
    cmd.env("HOME", home_dir_str);
    cmd.env("USERPROFILE", home_dir_str);
    for &(name, val) in env {
        cmd.env(name, val);
    }
    let out = cmd.output().unwrap();
    assert!(out.status.success());

    out
}

// Rename ~/.multirust to ~/.rustup
#[test]
fn multirust_dir_upgrade_rename_multirust_dir_to_rustup() {
    setup(&|config| {
        let multirust_dir = config.homedir.join(".multirust");
        let rustup_dir = config.homedir.join(".rustup");
        let multirust_dir_str = &format!("{}", multirust_dir.display());

        // First write data into ~/.multirust
        run_no_home(
            config,
            &["rustup", "default", "stable"],
            &[("RUSTUP_HOME", multirust_dir_str)],
        );
        let out = run_no_home(
            config,
            &["rustup", "toolchain", "list"],
            &[("RUSTUP_HOME", multirust_dir_str)],
        );
        assert!(String::from_utf8(out.stdout).unwrap().contains("stable"));

        assert!(multirust_dir.exists());
        assert!(!rustup_dir.exists());

        // Next run without RUSTUP_DIR, but with HOME/USERPROFILE set so rustup
        // can infer RUSTUP_DIR. It will silently move ~/.multirust to
        // ~/.rustup.
        let out = run_no_home(config, &["rustup", "toolchain", "list"], &[]);
        assert!(String::from_utf8(out.stdout).unwrap().contains("stable"));

        assert!(multirust_dir.exists());
        assert!(fs::symlink_metadata(&multirust_dir)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(rustup_dir.exists());
    });
}

// Renaming ~/.multirust to ~/.rustup but ~/.rustup/rustup-version (rustup.sh) exists
#[test]
fn multirust_dir_upgrade_old_rustup_exists() {
    setup(&|config| {
        let multirust_dir = config.homedir.join(".multirust");
        let rustup_dir = config.homedir.join(".rustup");
        let rustup_sh_dir = config.homedir.join(".rustup.sh");

        let multirust_dir_str = &format!("{}", multirust_dir.display());
        let old_rustup_sh_version_file = rustup_dir.join("rustup-version");
        let new_rustup_sh_version_file = rustup_sh_dir.join("rustup-version");

        // First write data into ~/.multirust
        run_no_home(
            config,
            &["rustup", "default", "stable"],
            &[("RUSTUP_HOME", multirust_dir_str)],
        );
        let out = run_no_home(
            config,
            &["rustup", "toolchain", "list"],
            &[("RUSTUP_HOME", multirust_dir_str)],
        );
        assert!(String::from_utf8(out.stdout).unwrap().contains("stable"));

        assert!(multirust_dir.exists());
        assert!(!rustup_dir.exists());

        // Now add rustup.sh data to ~/.rustup
        fs::create_dir_all(&rustup_dir).unwrap();
        raw::write_file(&old_rustup_sh_version_file, "1").unwrap();
        assert!(old_rustup_sh_version_file.exists());

        // Now do the upgrade, and ~/.rustup will be moved to ~/.rustup.sh
        let out = run_no_home(config, &["rustup", "toolchain", "list"], &[]);
        assert!(String::from_utf8(out.stdout).unwrap().contains("stable"));

        assert!(multirust_dir.exists());
        assert!(fs::symlink_metadata(&multirust_dir)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(rustup_dir.exists());
        assert!(!old_rustup_sh_version_file.exists());
        assert!(new_rustup_sh_version_file.exists());
    });
}

// Renaming ~/.multirust to ~/.rustup but ~/.rustup/rustup-version (rustup.sh) exists,
// oh and alse ~/.rustup.sh exists
#[test]
fn multirust_dir_upgrade_old_rustup_existsand_new_rustup_sh_exists() {
    setup(&|config| {
        let multirust_dir = config.homedir.join(".multirust");
        let rustup_dir = config.homedir.join(".rustup");
        let rustup_sh_dir = config.homedir.join(".rustup.sh");

        let multirust_dir_str = &format!("{}", multirust_dir.display());
        let old_rustup_sh_version_file = rustup_dir.join("rustup-version");
        let new_rustup_sh_version_file = rustup_sh_dir.join("rustup-version");

        // First write data into ~/.multirust
        run_no_home(
            config,
            &["rustup", "default", "stable"],
            &[("RUSTUP_HOME", multirust_dir_str)],
        );
        let out = run_no_home(
            config,
            &["rustup", "toolchain", "list"],
            &[("RUSTUP_HOME", multirust_dir_str)],
        );
        assert!(String::from_utf8(out.stdout).unwrap().contains("stable"));

        assert!(multirust_dir.exists());
        assert!(!rustup_dir.exists());

        // This time there are two things that look like rustup.sh.
        // Only one can win. It doesn't matter much which.

        // Now add rustup.sh data to ~/.rustup
        fs::create_dir_all(&rustup_dir).unwrap();
        raw::write_file(&old_rustup_sh_version_file, "1").unwrap();

        // Also to ~/.rustup.sh
        fs::create_dir_all(&rustup_sh_dir).unwrap();
        raw::write_file(&new_rustup_sh_version_file, "1").unwrap();

        assert!(old_rustup_sh_version_file.exists());
        assert!(new_rustup_sh_version_file.exists());

        // Now do the upgrade, and ~/.rustup will be moved to ~/.rustup.sh
        let out = run_no_home(config, &["rustup", "toolchain", "list"], &[]);
        assert!(String::from_utf8(out.stdout).unwrap().contains("stable"));

        // .multirust is now a symlink to .rustup
        assert!(multirust_dir.exists());
        assert!(fs::symlink_metadata(&multirust_dir)
            .unwrap()
            .file_type()
            .is_symlink());

        assert!(rustup_dir.exists());
        assert!(!old_rustup_sh_version_file.exists());
        assert!(new_rustup_sh_version_file.exists());
    });
}

#[test]
fn multirust_upgrade_works_with_proxy() {
    setup(&|config| {
        let multirust_dir = config.homedir.join(".multirust");
        let rustup_dir = config.homedir.join(".rustup");

        // Put data in ~/.multirust
        run_no_home(
            config,
            &["rustup", "default", "stable"],
            &[("RUSTUP_HOME", &format!("{}", multirust_dir.display()))],
        );

        run_no_home(config, &["rustc", "--version"], &[]);

        assert!(multirust_dir.exists());
        assert!(fs::symlink_metadata(&multirust_dir)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(rustup_dir.exists());
    });
}

#[test]
fn file_override() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
        );

        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly").unwrap();

        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
    });
}

#[test]
fn file_override_subdir() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
        );

        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly").unwrap();

        let subdir = cwd.join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        config.change_dir(&subdir, &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        });
    });
}

#[test]
fn file_override_with_archive() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly-2015-01-01",
                "--no-self-update",
            ],
        );

        expect_stdout_ok(config, &["rustc", "--version"], "hash-s-2");

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly-2015-01-01").unwrap();

        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");
    });
}

#[test]
fn directory_override_beats_file_override() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &["rustup", "toolchain", "install", "beta", "--no-self-update"],
        );
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
        );

        expect_ok(config, &["rustup", "override", "set", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly").unwrap();

        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
    });
}

#[test]
fn close_file_override_beats_far_directory_override() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &["rustup", "toolchain", "install", "beta", "--no-self-update"],
        );
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
        );

        expect_ok(config, &["rustup", "override", "set", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");

        let cwd = config.current_dir();

        let subdir = cwd.join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        let toolchain_file = subdir.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly").unwrap();

        config.change_dir(&subdir, &|| {
            expect_stdout_ok(config, &["rustc", "--version"], "hash-n-2");
        });
    });
}

#[test]
fn directory_override_doesnt_need_to_exist_unless_it_is_selected() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &["rustup", "toolchain", "install", "beta", "--no-self-update"],
        );
        // not installing nightly

        expect_ok(config, &["rustup", "override", "set", "beta"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly").unwrap();

        expect_stdout_ok(config, &["rustc", "--version"], "hash-b-2");
    });
}

#[test]
fn env_override_beats_file_override() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &["rustup", "toolchain", "install", "beta", "--no-self-update"],
        );
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
        );

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly").unwrap();

        let mut cmd = clitools::cmd(config, "rustc", &["--version"]);
        clitools::env(config, &mut cmd);
        cmd.env("RUSTUP_TOOLCHAIN", "beta");

        let out = cmd.output().unwrap();
        assert!(String::from_utf8(out.stdout).unwrap().contains("hash-b-2"));
    });
}

#[test]
fn plus_override_beats_file_override() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &["rustup", "toolchain", "install", "beta", "--no-self-update"],
        );
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
        );

        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly").unwrap();

        expect_stdout_ok(config, &["rustc", "+beta", "--version"], "hash-b-2");
    });
}

#[test]
fn bad_file_override() {
    setup(&|config| {
        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "gumbo").unwrap();

        expect_err(
            config,
            &["rustc", "--version"],
            "invalid channel name 'gumbo' in",
        );
    });
}

#[test]
fn file_override_with_target_info() {
    setup(&|config| {
        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly-x86_64-unknown-linux-gnu").unwrap();

        expect_err(
            config,
            &["rustc", "--version"],
            "target triple in channel name 'nightly-x86_64-unknown-linux-gnu'",
        );
    });
}

#[test]
fn docs_with_path() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "install",
                "nightly",
                "--no-self-update",
            ],
        );

        let mut cmd = clitools::cmd(config, "rustup", &["doc", "--path"]);
        clitools::env(config, &mut cmd);

        let out = cmd.output().unwrap();
        let path = format!("share{0}doc{0}rust{0}html", MAIN_SEPARATOR);
        assert!(String::from_utf8(out.stdout).unwrap().contains(&path));

        let mut cmd = clitools::cmd(
            config,
            "rustup",
            &["doc", "--path", "--toolchain", "nightly"],
        );
        clitools::env(config, &mut cmd);

        let out = cmd.output().unwrap();
        assert!(String::from_utf8(out.stdout).unwrap().contains("nightly"));
    });
}
