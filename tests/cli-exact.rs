//! Yet more cli test cases. These are testing that the output
//! is exactly as expected.

pub mod mock;

use crate::mock::clitools::{
    self, expect_err_ex, expect_ok, expect_ok_ex, expect_stdout_ok, set_current_dist_date,
    this_host_triple, Config, Scenario,
};

macro_rules! for_host {
    ($s: expr) => {
        &format!($s, this_host_triple())
    };
}

fn setup(f: &dyn Fn(&mut Config)) {
    clitools::setup(Scenario::ArchivesV2, f);
}

#[test]
fn update() {
    setup(&|config| {
        expect_ok_ex(
            config,
            &["rustup", "update", "nightly", "--no-self-update"],
            for_host!(
                r"
  nightly-{0} installed - 1.3.0 (hash-nightly-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: Defaulting to 500.0 MiB unpack ram
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'nightly-{0}'
"
            ),
        );
    });
}

#[test]
fn update_again() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_ok_ex(
            config,
            &["rustup", "update", "nightly", "--no-self-update"],
            for_host!(
                r"
  nightly-{0} unchanged - 1.3.0 (hash-nightly-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
"
            ),
        );
    });
}

#[test]
fn check_updates_none() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "check"],
            for_host!(
                r"stable-{0} - Up to date : 1.0.0 (hash-stable-1.0.0)
beta-{0} - Up to date : 1.1.0 (hash-beta-1.1.0)
nightly-{0} - Up to date : 1.2.0 (hash-nightly-1)
"
            ),
        );
    })
}

#[test]
fn check_updates_some() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        set_current_dist_date(config, "2015-01-02");
        expect_stdout_ok(
            config,
            &["rustup", "check"],
            for_host!(
                r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Update available : 1.1.0 (hash-beta-1.1.0) -> 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
            ),
        );
    })
}

#[test]
fn check_updates_with_update() {
    setup(&|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "check"],
            for_host!(
                r"stable-{0} - Up to date : 1.0.0 (hash-stable-1.0.0)
beta-{0} - Up to date : 1.1.0 (hash-beta-1.1.0)
nightly-{0} - Up to date : 1.2.0 (hash-nightly-1)
"
            ),
        );
        set_current_dist_date(config, "2015-01-02");
        expect_stdout_ok(
            config,
            &["rustup", "check"],
            for_host!(
                r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Update available : 1.1.0 (hash-beta-1.1.0) -> 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
            ),
        );
        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "check"],
            for_host!(
                r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Up to date : 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
            ),
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
  nightly-{0} installed - 1.3.0 (hash-nightly-2)

"
            ),
            for_host!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: Defaulting to 500.0 MiB unpack ram
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'nightly-{0}'
"
            ),
        );
    });
}

#[test]
fn override_again() {
    setup(&|config| {
        let cwd = config.current_dir();
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_ok_ex(
            config,
            &["rustup", "override", "add", "nightly"],
            for_host!(
                r"
  nightly-{} unchanged - 1.3.0 (hash-nightly-2)

"
            ),
            &format!(
                r"info: using existing install for 'nightly-{1}'
info: override toolchain for '{}' set to 'nightly-{1}'
",
                cwd.display(),
                &this_host_triple()
            ),
        );
    });
}

#[test]
fn remove_override() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let cwd = config.current_dir();
            expect_ok(config, &["rustup", "override", "add", "nightly"]);
            expect_ok_ex(
                config,
                &["rustup", "override", keyword],
                r"",
                &format!("info: override toolchain for '{}' removed\n", cwd.display()),
            );
        });
    }
}

#[test]
fn remove_override_none() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let cwd = config.current_dir();
            expect_ok_ex(
                config,
                &["rustup", "override", keyword],
                r"",
                &format!(
                    "info: no override toolchain for '{}'
info: you may use `--path <path>` option to remove override toolchain for a specific path\n",
                    cwd.display()
                ),
            );
        });
    }
}

#[test]
fn remove_override_with_path() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let dir = tempfile::Builder::new()
                .prefix("rustup-test")
                .tempdir()
                .unwrap();
            config.change_dir(dir.path(), || {
                expect_ok(config, &["rustup", "override", "add", "nightly"]);
            });
            expect_ok_ex(
                config,
                &[
                    "rustup",
                    "override",
                    keyword,
                    "--path",
                    dir.path().to_str().unwrap(),
                ],
                r"",
                &format!(
                    "info: override toolchain for '{}' removed\n",
                    dir.path().display()
                ),
            );
        });
    }
}

#[test]
fn remove_override_with_path_deleted() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let path = {
                let dir = tempfile::Builder::new()
                    .prefix("rustup-test")
                    .tempdir()
                    .unwrap();
                let path = std::fs::canonicalize(dir.path()).unwrap();
                config.change_dir(&path, || {
                    expect_ok(config, &["rustup", "override", "add", "nightly"]);
                });
                path
            };
            expect_ok_ex(
                config,
                &[
                    "rustup",
                    "override",
                    keyword,
                    "--path",
                    path.to_str().unwrap(),
                ],
                r"",
                &format!(
                    "info: override toolchain for '{}' removed\n",
                    path.display()
                ),
            );
        });
    }
}

#[test]
#[cfg_attr(target_os = "windows", ignore)] // FIXME #1103
fn remove_override_nonexistent() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let path = {
                let dir = tempfile::Builder::new()
                    .prefix("rustup-test")
                    .tempdir()
                    .unwrap();
                let path = std::fs::canonicalize(dir.path()).unwrap();
                config.change_dir(&path, || {
                    expect_ok(config, &["rustup", "override", "add", "nightly"]);
                });
                path
            };
            // FIXME TempDir seems to succumb to difficulties removing dirs on windows
            let _ = rustup::utils::raw::remove_dir(&path);
            assert!(!path.exists());
            expect_ok_ex(
                config,
                &["rustup", "override", keyword, "--nonexistent"],
                r"",
                &format!(
                    "info: override toolchain for '{}' removed\n",
                    path.display()
                ),
            );
        });
    }
}

#[test]
fn list_overrides() {
    setup(&|config| {
        let cwd = std::fs::canonicalize(config.current_dir()).unwrap();
        let mut cwd_formatted = format!("{}", cwd.display());

        if cfg!(windows) {
            cwd_formatted = cwd_formatted[4..].to_owned();
        }

        let trip = this_host_triple();
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_ok_ex(
            config,
            &["rustup", "override", "list"],
            &format!(
                "{:<40}\t{:<20}\n",
                cwd_formatted,
                &format!("nightly-{}", trip)
            ),
            r"",
        );
    });
}

#[test]
fn list_overrides_with_nonexistent() {
    setup(&|config| {
        let trip = this_host_triple();

        let nonexistent_path = {
            let dir = tempfile::Builder::new()
                .prefix("rustup-test")
                .tempdir()
                .unwrap();
            config.change_dir(dir.path(), || {
                expect_ok(config, &["rustup", "override", "add", "nightly"]);
            });
            std::fs::canonicalize(dir.path()).unwrap()
        };
        // FIXME TempDir seems to succumb to difficulties removing dirs on windows
        let _ = rustup::utils::raw::remove_dir(&nonexistent_path);
        assert!(!nonexistent_path.exists());
        let mut path_formatted = format!("{}", nonexistent_path.display());

        if cfg!(windows) {
            path_formatted = path_formatted[4..].to_owned();
        }

        expect_ok_ex(
            config,
            &["rustup", "override", "list"],
            &format!(
                "{:<40}\t{:<20}\n\n",
                path_formatted + " (not a directory)",
                &format!("nightly-{}", trip)
            ),
            "info: you may remove overrides for non-existent directories with
`rustup override unset --nonexistent`\n",
        );
    });
}

// Issue #111
#[test]
fn update_invalid_toolchain() {
    setup(&|config| {
        expect_err_ex(
            config,
            &["rustup", "update", "nightly-2016-03-1"],
            r"",
            r"info: syncing channel updates for 'nightly-2016-03-1'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
error: target '2016-03-1' not found in channel.  Perhaps check https://forge.rust-lang.org/release/platform-support.html for available targets
",
        );
    });
}

#[test]
fn default_invalid_toolchain() {
    setup(&|config| {
        expect_err_ex(
            config,
            &["rustup", "default", "nightly-2016-03-1"],
            r"",
            r"info: syncing channel updates for 'nightly-2016-03-1'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
error: target '2016-03-1' not found in channel.  Perhaps check https://forge.rust-lang.org/release/platform-support.html for available targets
",
        );
    });
}

#[test]
fn list_targets() {
    setup(&|config| {
        let trip = this_host_triple();
        let mut sorted = vec![
            format!("{} (installed)", &*trip),
            format!("{} (installed)", clitools::CROSS_ARCH1),
            clitools::CROSS_ARCH2.to_string(),
        ];
        sorted.sort();

        let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        expect_ok_ex(config, &["rustup", "target", "list"], &expected, r"");
    });
}

#[test]
fn list_installed_targets() {
    setup(&|config| {
        let trip = this_host_triple();
        let mut sorted = vec![
            trip,
            clitools::CROSS_ARCH1.to_string(),
            clitools::CROSS_ARCH2.to_string(),
        ];
        sorted.sort();

        let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH1]);
        expect_ok(config, &["rustup", "target", "add", clitools::CROSS_ARCH2]);
        expect_ok_ex(
            config,
            &["rustup", "target", "list", "--installed"],
            &expected,
            r"",
        );
    });
}

#[test]
fn cross_install_indicates_target() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        // TODO error 'nightly-x86_64-apple-darwin' is not installed
        expect_ok_ex(
            config,
            &["rustup", "target", "add", clitools::CROSS_ARCH1],
            r"",
            &format!(
                r"info: downloading component 'rust-std' for '{0}'
info: installing component 'rust-std' for '{0}'
info: Defaulting to 500.0 MiB unpack ram
",
                clitools::CROSS_ARCH1
            ),
        );
    });
}

// issue #927
#[test]
fn undefined_linked_toolchain() {
    setup(&|config| {
        expect_err_ex(
            config,
            &["cargo", "+bogus", "test"],
            r"",
            "error: toolchain 'bogus' is not installed\n",
        );
    });
}

#[test]
fn install_by_version_number() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "0.100.99"]);
    })
}
