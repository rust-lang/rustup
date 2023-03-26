//! Yet more cli test cases. These are testing that the output
//! is exactly as expected.

use rustup::for_host;
use rustup::test::{
    mock::clitools::{self, set_current_dist_date, with_update_server, Config, Scenario},
    this_host_triple,
};
use rustup_macros::integration_test as test;

/// Start a test with Scenario::None
fn test(f: &dyn Fn(&mut Config)) {
    clitools::test(Scenario::None, f);
}

#[test]
fn update_once() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok_ex(
                &["rustup", "update", "nightly"],
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
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'nightly-{0}'
"
                ),
            );
        })
    });
}

#[test]
fn update_once_and_check_self_update() {
    let test_version = "2.0.0";
    test(&|config| {
        with_update_server(config, test_version, &|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
                config.expect_ok(&["rustup", "set", "auto-self-update", "check-only"]);
                let current_version = env!("CARGO_PKG_VERSION");

                config.expect_ok_ex(
                    &["rustup", "update", "nightly"],
                    &format!(
                        r"
  nightly-{} installed - 1.3.0 (hash-nightly-2)

rustup - Update available : {} -> {}
",
                        &this_host_triple(),
                        current_version,
                        test_version
                    ),
                    for_host!(
                        r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
"
                    ),
                );
            })
        })
    })
}

#[test]
fn update_once_and_self_update() {
    let test_version = "2.0.0";

    test(&|config| {
        with_update_server(config, test_version, &|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                config.expect_ok(&["rustup-init", "-y", "--no-modify-path"]);
                config.expect_ok(&["rustup", "set", "auto-self-update", "enable"]);
                config.expect_ok_ex(
                    &["rustup", "update", "nightly"],
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
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: checking for self-update
info: downloading self-update
"
                    ),
                );
            })
        })
    })
}

#[test]
fn update_again() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "update", "nightly"]);
            config.expect_ok(&["rustup", "upgrade", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "update", "nightly"],
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
            config.expect_ok_ex(
                &["rustup", "upgrade", "nightly"],
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
        })
    });
}

#[test]
fn check_updates_none() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"]);
            config.expect_stdout_ok(
                &["rustup", "check"],
                for_host!(
                    r"stable-{0} - Up to date : 1.1.0 (hash-stable-1.1.0)
beta-{0} - Up to date : 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Up to date : 1.3.0 (hash-nightly-2)
"
                ),
            );
        })
    })
}

#[test]
fn check_updates_some() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"]);
        });
        config.with_scenario(Scenario::SimpleV2, &|config| {
        config.expect_stdout_ok(
            &["rustup", "check"],
            for_host!(
                r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Update available : 1.1.0 (hash-beta-1.1.0) -> 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
            ),
        );
            })
    })
}

#[test]
fn check_updates_self() {
    let test_version = "2.0.0";

    test(&|config| {
        with_update_server(config, test_version, &|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                let current_version = env!("CARGO_PKG_VERSION");

                config.expect_stdout_ok(
                    &["rustup", "check"],
                    &format!(
                        r"rustup - Update available : {current_version} -> {test_version}
"
                    ),
                );
            })
        })
    })
}

#[test]
fn check_updates_self_no_change() {
    let current_version = env!("CARGO_PKG_VERSION");

    test(&|config| {
        with_update_server(config, current_version, &|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                config.expect_stdout_ok(
                    &["rustup", "check"],
                    &format!(
                        r"rustup - Up to date : {current_version}
"
                    ),
                );
            })
        })
    })
}

#[test]
fn check_updates_with_update() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"]);
            config.expect_stdout_ok(
                &["rustup", "check"],
                for_host!(
                    r"stable-{0} - Up to date : 1.0.0 (hash-stable-1.0.0)
beta-{0} - Up to date : 1.1.0 (hash-beta-1.1.0)
nightly-{0} - Up to date : 1.2.0 (hash-nightly-1)
"
                ),
            );
        });
        config.with_scenario(Scenario::SimpleV2, &|config | {
        config.expect_stdout_ok(
            &["rustup", "check"],
            for_host!(
                r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Update available : 1.1.0 (hash-beta-1.1.0) -> 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
            ),
        );
        config.expect_ok(&["rustup", "update", "beta"]);
        config.expect_stdout_ok(
            &["rustup", "check"],
            for_host!(
                r"stable-{0} - Update available : 1.0.0 (hash-stable-1.0.0) -> 1.1.0 (hash-stable-1.1.0)
beta-{0} - Up to date : 1.2.0 (hash-beta-1.2.0)
nightly-{0} - Update available : 1.2.0 (hash-nightly-1) -> 1.3.0 (hash-nightly-2)
"
            ),
        );
    })
    });
}

#[test]
fn default() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok_ex(
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
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: default toolchain set to 'nightly-{0}'
"
                ),
            );
        })
    });
}

#[test]
fn override_again() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let cwd = config.current_dir();
            config.expect_ok(&["rustup", "override", "add", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "override", "add", "nightly"],
                "",
                &format!(
                    r"info: override toolchain for '{}' set to 'nightly-{1}'
",
                    cwd.display(),
                    &this_host_triple()
                ),
            );
        })
    });
}

#[test]
fn remove_override() {
    for keyword in &["remove", "unset"] {
        test(&|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                let cwd = config.current_dir();
                config.expect_ok(&["rustup", "override", "add", "nightly"]);
                config.expect_ok_ex(
                    &["rustup", "override", keyword],
                    r"",
                    &format!("info: override toolchain for '{}' removed\n", cwd.display()),
                );
            })
        });
    }
}

#[test]
fn remove_override_none() {
    for keyword in &["remove", "unset"] {
        test(&|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                let cwd = config.current_dir();
                config.expect_ok_ex(
                    &["rustup", "override", keyword],
                    r"",
                    &format!(
                        "info: no override toolchain for '{}'
info: you may use `--path <path>` option to remove override toolchain for a specific path\n",
                        cwd.display()
                    ),
                );
            })
        });
    }
}

#[test]
fn remove_override_with_path() {
    for keyword in &["remove", "unset"] {
        test(&|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                let dir = tempfile::Builder::new()
                    .prefix("rustup-test")
                    .tempdir()
                    .unwrap();
                config.change_dir(dir.path(), &|config| {
                    config.expect_ok(&["rustup", "override", "add", "nightly"]);
                });
                config.expect_ok_ex(
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
            })
        });
    }
}

#[test]
fn remove_override_with_path_deleted() {
    for keyword in &["remove", "unset"] {
        test(&|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                let path = {
                    let dir = tempfile::Builder::new()
                        .prefix("rustup-test")
                        .tempdir()
                        .unwrap();
                    let path = std::fs::canonicalize(dir.path()).unwrap();
                    config.change_dir(&path, &|config| {
                        config.expect_ok(&["rustup", "override", "add", "nightly"]);
                    });
                    path
                };
                config.expect_ok_ex(
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
            })
        });
    }
}

#[test]
#[cfg_attr(target_os = "windows", ignore)] // FIXME #1103
fn remove_override_nonexistent() {
    for keyword in &["remove", "unset"] {
        test(&|config| {
            config.with_scenario(Scenario::SimpleV2, &|config| {
                let path = {
                    let dir = tempfile::Builder::new()
                        .prefix("rustup-test")
                        .tempdir()
                        .unwrap();
                    let path = std::fs::canonicalize(dir.path()).unwrap();
                    config.change_dir(&path, &|config| {
                        config.expect_ok(&["rustup", "override", "add", "nightly"]);
                    });
                    path
                };
                // FIXME TempDir seems to succumb to difficulties removing dirs on windows
                let _ = rustup::utils::raw::remove_dir(&path);
                assert!(!path.exists());
                config.expect_ok_ex(
                    &["rustup", "override", keyword, "--nonexistent"],
                    r"",
                    &format!(
                        "info: override toolchain for '{}' removed\n",
                        path.display()
                    ),
                );
            })
        });
    }
}

#[test]
fn list_overrides() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let cwd = std::fs::canonicalize(config.current_dir()).unwrap();
            let mut cwd_formatted = format!("{}", cwd.display());

            if cfg!(windows) {
                cwd_formatted = cwd_formatted[4..].to_owned();
            }

            let trip = this_host_triple();
            config.expect_ok(&["rustup", "override", "add", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "override", "list"],
                &format!(
                    "{:<40}\t{:<20}\n",
                    cwd_formatted,
                    &format!("nightly-{trip}")
                ),
                r"",
            );
        })
    });
}

#[test]
fn list_overrides_with_nonexistent() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let trip = this_host_triple();

            let nonexistent_path = {
                let dir = tempfile::Builder::new()
                    .prefix("rustup-test")
                    .tempdir()
                    .unwrap();
                config.change_dir(dir.path(), &|config| {
                    config.expect_ok(&["rustup", "override", "add", "nightly"]);
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

            config.expect_ok_ex(
                &["rustup", "override", "list"],
                &format!(
                    "{:<40}\t{:<20}\n\n",
                    path_formatted + " (not a directory)",
                    &format!("nightly-{trip}")
                ),
                "info: you may remove overrides for non-existent directories with
`rustup override unset --nonexistent`\n",
            );
        })
    });
}

#[test]
fn update_no_manifest() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_err_ex(
                &["rustup", "update", "nightly-2016-01-01"],
                r"",
                for_host!(
                    r"info: syncing channel updates for 'nightly-2016-01-01-{0}'
error: no release found for 'nightly-2016-01-01'
"
                ),
            );
        })
    });
}

// Issue #111
#[test]
fn update_custom_toolchain() {
    test(&|config| {
        // installable toolchains require 2 digits in the DD and MM fields, so this is
        // treated as a custom toolchain, which can't be used with update.
        config.expect_err(
            &["rustup", "update", "nightly-2016-03-1"],
            "invalid toolchain name: 'nightly-2016-03-1'",
        );
    });
}

#[test]
fn default_custom_not_installed_toolchain() {
    test(&|config| {
        // installable toolchains require 2 digits in the DD and MM fields, so this is
        // treated as a custom toolchain, which isn't installed.
        config.expect_err(
            &["rustup", "default", "nightly-2016-03-1"],
            "toolchain 'nightly-2016-03-1' is not installed",
        );
    });
}

#[test]
fn default_none() {
    test(&|config| {
        config.expect_stderr_ok(
            &["rustup", "default", "none"],
            "info: default toolchain unset",
        );
        config.expect_err_ex(
            &["rustc", "--version"],
            "",
            "error: rustup could not choose a version of rustc to run, because one wasn't specified explicitly, and no default is configured.
help: run 'rustup default stable' to download the latest stable release of Rust and set it as your default toolchain.
",
        );
    })
}

#[test]
fn list_targets() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let trip = this_host_triple();
            let mut sorted = vec![
                format!("{} (installed)", &*trip),
                format!("{} (installed)", clitools::CROSS_ARCH1),
                clitools::CROSS_ARCH2.to_string(),
            ];
            sorted.sort();

            let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
            config.expect_ok_ex(&["rustup", "target", "list"], &expected, r"");
        })
    });
}

#[test]
fn list_installed_targets() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let trip = this_host_triple();
            let mut sorted = vec![
                trip,
                clitools::CROSS_ARCH1.to_string(),
                clitools::CROSS_ARCH2.to_string(),
            ];
            sorted.sort();

            let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
            config.expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH2]);
            config.expect_ok_ex(&["rustup", "target", "list", "--installed"], &expected, r"");
        })
    });
}

#[test]
fn cross_install_indicates_target() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            // TODO error 'nightly-x86_64-apple-darwin' is not installed
            config.expect_ok_ex(
                &["rustup", "target", "add", clitools::CROSS_ARCH1],
                r"",
                &format!(
                    r"info: downloading component 'rust-std' for '{0}'
info: installing component 'rust-std' for '{0}'
",
                    clitools::CROSS_ARCH1
                ),
            );
        })
    });
}

// issue #927
#[test]
fn undefined_linked_toolchain() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_err_ex(
                &["cargo", "+bogus", "test"],
                r"",
                "error: toolchain 'bogus' is not installable\n",
            );
        })
    });
}

#[test]
fn install_by_version_number() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2TwoVersions, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "0.100.99"]);
        })
    })
}

// issue #2191
#[test]
fn install_unreleased_component() {
    clitools::test(Scenario::MissingComponentMulti, &|config| {
        // Initial channel content is host + rls + multiarch-std
        set_current_dist_date(config, "2019-09-12");
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_ok(&["rustup", "component", "add", "rls"]);
        config.expect_ok(&["rustup", "target", "add", clitools::MULTI_ARCH1]);

        // Next channel variant should have host + rls but not multiarch-std
        set_current_dist_date(config, "2019-09-13");
        config.expect_ok_ex(
            &["rustup", "update", "nightly"],
            for_host!(
                r"
  nightly-{} unchanged - 1.37.0 (hash-nightly-1)

"
            ),
            &format!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2019-09-13, rust version 1.37.0 (hash-nightly-2)
info: skipping nightly which is missing installed component 'rust-std-{1}'
info: syncing channel updates for 'nightly-2019-09-12-{0}'
",
                this_host_triple(),
                clitools::MULTI_ARCH1
            ),
        );

        // Next channel variant should have host + multiarch-std but have rls missing
        set_current_dist_date(config, "2019-09-14");
        config.expect_ok_ex(
            &["rustup", "update", "nightly"],
            for_host!(
                r"
  nightly-{} unchanged - 1.37.0 (hash-nightly-1)

"
            ),
            &format!(
                r"info: syncing channel updates for 'nightly-{0}'
info: latest update on 2019-09-14, rust version 1.37.0 (hash-nightly-3)
info: skipping nightly which is missing installed component 'rls'
info: syncing channel updates for 'nightly-2019-09-13-{0}'
info: latest update on 2019-09-13, rust version 1.37.0 (hash-nightly-2)
info: skipping nightly which is missing installed component 'rust-std-{1}'
info: syncing channel updates for 'nightly-2019-09-12-{0}'
",
                this_host_triple(),
                clitools::MULTI_ARCH1,
            ),
        );
    })
}
