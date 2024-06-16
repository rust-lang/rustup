//! Test cases for new rustup UI

use std::fs;
use std::path::{PathBuf, MAIN_SEPARATOR};
use std::{env::consts::EXE_SUFFIX, path::Path};

use rustup::for_host;
use rustup::test::this_host_triple;
use rustup::utils::raw;
use rustup_macros::integration_test as test;

use rustup::test::mock::{
    self,
    clitools::{self, Config, Scenario},
};

macro_rules! for_host_and_home {
    ($config:ident, $s: expr) => {
        &format!($s, this_host_triple(), $config.rustupdir)
    };
}

fn test(f: &dyn Fn(&mut Config)) {
    clitools::test(Scenario::None, f);
}

#[test]
fn rustup_stable() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "stable"]);
        });
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok_ex(
                &["rustup", "update"],
                for_host!(
                    r"
  stable-{0} updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))

"
                ),
                for_host!(
                    r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: cleaning up downloads & tmp directories
"
                ),
            );
        })
    });
}

#[test]
fn rustup_stable_quiet() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "--quiet", "update", "stable"]);
        });
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok_ex(
                &["rustup", "--quiet", "update"],
                for_host!(
                    r"
  stable-{0} updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))

"
                ),
                for_host!(
                    r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: cleaning up downloads & tmp directories
"
                ),
            );
        })
    });
}

#[test]
fn rustup_stable_no_change() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "update", "stable"]);
            config.expect_ok_ex(
                &["rustup", "update"],
                for_host!(
                    r"
  stable-{0} unchanged - 1.0.0 (hash-stable-1.0.0)

"
                ),
                for_host!(
                    r"info: syncing channel updates for 'stable-{0}'
info: cleaning up downloads & tmp directories
"
                ),
            );
        })
    });
}

#[test]
fn rustup_all_channels() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"]);
        });
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok_ex(
                &["rustup", "update"],
                for_host!(
                    r"
   stable-{0} updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))
     beta-{0} updated - 1.2.0 (hash-beta-1.2.0) (from 1.1.0 (hash-beta-1.1.0))
  nightly-{0} updated - 1.3.0 (hash-nightly-2) (from 1.2.0 (hash-nightly-1))

"
                ),
                for_host!(
                    r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: syncing channel updates for 'beta-{0}'
info: latest update on 2015-01-02, rust version 1.2.0 (hash-beta-1.2.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: cleaning up downloads & tmp directories
"
                ),
            );
        })
    })
}

#[test]
fn rustup_some_channels_up_to_date() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"]);
        });
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "update", "beta"]);
            config.expect_ok_ex(
                &["rustup", "update"],
                for_host!(
                    r"
   stable-{0} updated - 1.1.0 (hash-stable-1.1.0) (from 1.0.0 (hash-stable-1.0.0))
   beta-{0} unchanged - 1.2.0 (hash-beta-1.2.0)
  nightly-{0} updated - 1.3.0 (hash-nightly-2) (from 1.2.0 (hash-nightly-1))

"
                ),
                for_host!(
                    r"info: syncing channel updates for 'stable-{0}'
info: latest update on 2015-01-02, rust version 1.1.0 (hash-stable-1.1.0)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: syncing channel updates for 'beta-{0}'
info: syncing channel updates for 'nightly-{0}'
info: latest update on 2015-01-02, rust version 1.3.0 (hash-nightly-2)
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: removing previous version of component 'cargo'
info: removing previous version of component 'rust-docs'
info: removing previous version of component 'rust-std'
info: removing previous version of component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: cleaning up downloads & tmp directories
"
                ),
            );
        })
    })
}

#[test]
fn rustup_no_channels() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok_ex(
                &["rustup", "update"],
                r"",
                r"info: no updatable toolchains installed
info: cleaning up downloads & tmp directories
",
            );
        })
    })
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
fn default_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "nightly"]);
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "override", "set", "nightly"]);
            config.expect_stderr_ok(
                &["rustup", "default", "stable"],
                for_host!(
                    r"info: using existing install for 'stable-{0}'
info: default toolchain set to 'stable-{0}'
info: note that the toolchain 'nightly-{0}' is currently in use (directory override for"
                ),
            );
        })
    });
}

#[test]
fn rustup_zstd() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_stderr_ok(
                &["rustup", "--verbose", "toolchain", "add", "nightly"],
                for_host!(r"dist/2015-01-01/rust-std-nightly-{0}.tar.zst"),
            );
        })
    });
}

#[test]
fn add_target() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let path = format!(
                "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                &this_host_triple(),
                clitools::CROSS_ARCH1
            );
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
            assert!(config.rustupdir.has(path));
        })
    });
}

#[test]
fn remove_target() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let path = format!(
                "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                &this_host_triple(),
                clitools::CROSS_ARCH1
            );
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH1]);
            assert!(config.rustupdir.has(&path));
            config.expect_ok(&["rustup", "target", "remove", clitools::CROSS_ARCH1]);
            assert!(!config.rustupdir.has(&path));
        })
    });
}

#[test]
fn add_remove_multiple_targets() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&[
                "rustup",
                "target",
                "add",
                clitools::CROSS_ARCH1,
                clitools::CROSS_ARCH2,
            ]);
            let path = format!(
                "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                &this_host_triple(),
                clitools::CROSS_ARCH1
            );
            assert!(config.rustupdir.has(path));
            let path = format!(
                "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                &this_host_triple(),
                clitools::CROSS_ARCH2
            );
            assert!(config.rustupdir.has(path));

            config.expect_ok(&[
                "rustup",
                "target",
                "remove",
                clitools::CROSS_ARCH1,
                clitools::CROSS_ARCH2,
            ]);
            let path = format!(
                "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                &this_host_triple(),
                clitools::CROSS_ARCH1
            );
            assert!(!config.rustupdir.has(path));
            let path = format!(
                "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                &this_host_triple(),
                clitools::CROSS_ARCH2
            );
            assert!(!config.rustupdir.has(path));
        })
    });
}

#[test]
fn list_targets() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_stdout_ok(&["rustup", "target", "list"], clitools::CROSS_ARCH1);
        })
    });
}

#[test]
fn list_installed_targets() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let trip = this_host_triple();

            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_stdout_ok(&["rustup", "target", "list", "--installed"], &trip);
        })
    });
}

#[test]
fn add_target_explicit() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let path = format!(
                "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                &this_host_triple(),
                clitools::CROSS_ARCH1
            );
            config.expect_ok(&["rustup", "toolchain", "add", "nightly"]);
            config.expect_ok(&[
                "rustup",
                "target",
                "add",
                "--toolchain",
                "nightly",
                clitools::CROSS_ARCH1,
            ]);
            assert!(config.rustupdir.has(path));
        })
    });
}

#[test]
fn remove_target_explicit() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let path = format!(
                "toolchains/nightly-{}/lib/rustlib/{}/lib/libstd.rlib",
                &this_host_triple(),
                clitools::CROSS_ARCH1
            );
            config.expect_ok(&["rustup", "toolchain", "add", "nightly"]);
            config.expect_ok(&[
                "rustup",
                "target",
                "add",
                "--toolchain",
                "nightly",
                clitools::CROSS_ARCH1,
            ]);
            assert!(config.rustupdir.has(&path));
            config.expect_ok(&[
                "rustup",
                "target",
                "remove",
                "--toolchain",
                "nightly",
                clitools::CROSS_ARCH1,
            ]);
            assert!(!config.rustupdir.has(&path));
        })
    });
}

#[test]
fn list_targets_explicit() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "nightly"]);
            config.expect_stdout_ok(
                &["rustup", "target", "list", "--toolchain", "nightly"],
                clitools::CROSS_ARCH1,
            );
        })
    });
}

#[test]
fn link() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let path = config.customdir.join("custom-1");
            let path = path.to_string_lossy();
            config.expect_ok(&["rustup", "toolchain", "link", "custom", &path]);
            config.expect_ok(&["rustup", "default", "custom"]);
            config.expect_stdout_ok(&["rustc", "--version"], "hash-c-1");
            config.expect_stdout_ok(&["rustup", "show"], "custom (active, default)");
            config.expect_ok(&["rustup", "update", "nightly"]);
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_stdout_ok(&["rustup", "show"], "custom");
        })
    });
}

// Issue #809. When we call the fallback cargo, when it in turn invokes
// "rustc", that rustc should actually be the rustup proxy, not the toolchain rustc.
// That way the proxy can pick the correct toolchain.
#[test]
fn fallback_cargo_calls_correct_rustc() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            // Hm, this is the _only_ test that assumes that toolchain proxies
            // exist in CARGO_HOME. Adding that proxy here.
            let rustup_path = config.exedir.join(format!("rustup{EXE_SUFFIX}"));
            let cargo_bin_path = config.cargodir.join("bin");
            fs::create_dir_all(&cargo_bin_path).unwrap();
            let rustc_path = cargo_bin_path.join(format!("rustc{EXE_SUFFIX}"));
            fs::hard_link(rustup_path, &rustc_path).unwrap();

            // Install a custom toolchain and a nightly toolchain for the cargo fallback
            let path = config.customdir.join("custom-1");
            let path = path.to_string_lossy();
            config.expect_ok(&["rustup", "toolchain", "link", "custom", &path]);
            config.expect_ok(&["rustup", "default", "custom"]);
            config.expect_ok(&["rustup", "update", "nightly"]);
            config.expect_stdout_ok(&["rustc", "--version"], "hash-c-1");
            config.expect_stdout_ok(&["cargo", "--version"], "hash-nightly-2");

            assert!(rustc_path.exists());

            // Here --call-rustc tells the mock cargo bin to exec `rustc --version`.
            // We should be ultimately calling the custom rustc, according to the
            // RUSTUP_TOOLCHAIN variable set by the original "cargo" proxy, and
            // interpreted by the nested "rustc" proxy.
            config.expect_stdout_ok(&["cargo", "--call-rustc"], "hash-c-1");
        })
    });
}

// Checks that cargo can recursively invoke itself with rustup shorthand (via
// the proxy).
//
// This involves a series of chained commands:
//
// 1. Calls `cargo --recursive-cargo-subcommand`
// 2. The rustup `cargo` proxy launches, and launches the "mock" nightly cargo exe.
// 3. The nightly "mock" cargo sees --recursive-cargo-subcommand, and launches
//    `cargo-foo --recursive-cargo`
// 4. `cargo-foo` sees `--recursive-cargo` and launches `cargo +nightly --version`
// 5. The rustup `cargo` proxy launches, and launches the "mock" nightly cargo exe.
// 6. The nightly "mock" cargo sees `--version` and prints the version.
//
// Previously, rustup would place the toolchain's `bin` directory in PATH for
// Windows due to some DLL issues. However, those aren't necessary anymore.
// If the toolchain `bin` directory is in PATH, then this test would fail in
// step 5 because the `cargo` executable would be the "mock" nightly cargo,
// and the first argument would be `+nightly` which would be an error.
#[test]
fn recursive_cargo() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);

            // We need an intermediary to run cargo itself.
            // The "mock" cargo can't do that because on Windows it will check
            // for a `cargo.exe` in the current directory before checking PATH.
            //
            // The solution here is to copy from the "mock" `cargo.exe` into
            // `~/.cargo/bin/cargo-foo`. This is just for convenience to avoid
            // needing to build another executable just for this test.
            let output = config.run("rustup", ["which", "cargo"], &[]);
            let real_mock_cargo = output.stdout.trim();
            let cargo_bin_path = config.cargodir.join("bin");
            let cargo_subcommand = cargo_bin_path.join(format!("cargo-foo{}", EXE_SUFFIX));
            fs::create_dir_all(&cargo_bin_path).unwrap();
            fs::copy(real_mock_cargo, cargo_subcommand).unwrap();

            config.expect_stdout_ok(&["cargo", "--recursive-cargo-subcommand"], "hash-nightly-2");
        });
    });
}

#[test]
fn show_home() {
    test(&|config| {
        config.expect_ok_ex(
            &["rustup", "show", "home"],
            &format!(
                r"{}
",
                config.rustupdir
            ),
            r"",
        );
    });
}

#[test]
fn show_toolchain_none() {
    test(&|config| {
        config.expect_ok_ex(
            &["rustup", "show"],
            for_host_and_home!(
                config,
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------

active toolchain
----------------
no active toolchain
"
            ),
            r"",
        );
    });
}

#[test]
fn show_toolchain_default() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "show"],
                for_host_and_home!(
                    config,
                    r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
compiler: 1.3.0 (hash-nightly-2)
active because: it's the default toolchain
installed targets:
  {0}
"
                ),
                r"",
            );
        })
    });
}

#[test]
fn show_no_default() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "install", "nightly"]);
            config.expect_ok(&["rustup", "default", "none"]);
            config.expect_stdout_ok(
                &["rustup", "show"],
                for_host!(
                    "\
installed toolchains
--------------------
nightly-{0}

active toolchain
"
                ),
            );
        })
    });
}

#[test]
fn show_no_default_active() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "install", "nightly"]);
            config.expect_ok(&["rustup", "default", "none"]);
            config.expect_stdout_ok(
                &["rustup", "+nightly", "show"],
                for_host!(
                    "\
installed toolchains
--------------------
nightly-{0} (active)

active toolchain
"
                ),
            );
        })
    });
}

#[test]
fn show_multiple_toolchains() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&["rustup", "update", "stable"]);
            config.expect_ok_ex(
                &["rustup", "show"],
                for_host_and_home!(
                    config,
                    r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
stable-{0}
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
compiler: 1.3.0 (hash-nightly-2)
active because: it's the default toolchain
installed targets:
  {0}
"
                ),
                r"",
            );
        })
    });
}

#[test]
fn show_multiple_targets() {
    test(&|config| {
        config.with_scenario(Scenario::MultiHost, &|config| {
            config.expect_ok(&[
                "rustup",
                "default",
                &format!("nightly-{}", clitools::MULTI_ARCH1),
            ]);
            config.expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH2]);
            config.expect_ok_ex(
                &["rustup", "show"],
                &format!(
                    r"Default host: {2}
rustup home:  {3}

installed toolchains
--------------------
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
compiler: 1.3.0 (xxxx-nightly-2)
active because: it's the default toolchain
installed targets:
  {1}
  {0}
",
                    clitools::MULTI_ARCH1,
                    clitools::CROSS_ARCH2,
                    this_host_triple(),
                    config.rustupdir
                ),
                r"",
            );
        })
    });
}

#[test]
fn show_multiple_toolchains_and_targets() {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86") {
        return;
    }

    test(&|config| {
        config.with_scenario(Scenario::MultiHost, &|config| {
            config.expect_ok(&[
                "rustup",
                "default",
                &format!("nightly-{}", clitools::MULTI_ARCH1),
            ]);
            config.expect_ok(&["rustup", "target", "add", clitools::CROSS_ARCH2]);
            config.expect_ok(&[
                "rustup",
                "update",
                &format!("stable-{}", clitools::MULTI_ARCH1),
            ]);
            config.expect_ok_ex(
                &["rustup", "show"],
                &format!(
                    r"Default host: {2}
rustup home:  {3}

installed toolchains
--------------------
stable-{0}
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
compiler: 1.3.0 (xxxx-nightly-2)
active because: it's the default toolchain
installed targets:
  {1}
  {0}
",
                    clitools::MULTI_ARCH1,
                    clitools::CROSS_ARCH2,
                    this_host_triple(),
                    config.rustupdir
                ),
                r"",
            );
        })
    });
}

#[test]
fn list_default_toolchain() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "toolchain", "list"],
                for_host!("nightly-{0} (active, default)\n"),
                r"",
            );
        })
    });
}

#[test]
fn list_default_toolchain_quiet() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "toolchain", "list", "--quiet"],
                for_host!("nightly-{0}\n"),
                r"",
            );
        })
    });
}

#[test]
fn list_no_default_toolchain() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "install", "nightly"]);
            config.expect_ok(&["rustup", "default", "none"]);
            config.expect_ok_ex(
                &["rustup", "toolchain", "list"],
                for_host!("nightly-{0}\n"),
                r"",
            );
        })
    });
}

#[test]
fn list_no_default_override_toolchain() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "override", "set", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "toolchain", "list"],
                for_host!("nightly-{0} (active)\n"),
                r"",
            );
        })
    });
}

#[test]
fn list_default_and_override_toolchain() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&["rustup", "override", "set", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "toolchain", "list"],
                for_host!("nightly-{0} (active, default)\n"),
                r"",
            );
        })
    });
}

#[test]
fn heal_damaged_toolchain() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_not_stderr_ok(&["rustup", "which", "rustc"], "syncing channel updates");
            let manifest_path = format!(
                "toolchains/nightly-{}/lib/rustlib/multirust-channel-manifest.toml",
                this_host_triple()
            );

            let mut rustc_path = config.rustupdir.join(
                [
                    "toolchains",
                    &format!("nightly-{}", this_host_triple()),
                    "bin",
                    "rustc",
                ]
                .iter()
                .collect::<PathBuf>(),
            );

            if cfg!(windows) {
                rustc_path.set_extension("exe");
            }

            fs::remove_file(config.rustupdir.join(manifest_path)).unwrap();
            config.expect_ok_ex(
                &["rustup", "which", "rustc"],
                &format!("{}\n", rustc_path.to_str().unwrap()),
                for_host!("info: syncing channel updates for 'nightly-{0}'\n"),
            );
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_stderr_ok(&["rustup", "which", "rustc"], "syncing channel updates");
        })
    });
}

#[test]
#[ignore = "FIXME: Windows shows UNC paths"]
fn show_toolchain_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let cwd = config.current_dir();
            config.expect_ok(&["rustup", "override", "add", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "show"],
                &format!(
                    r"Default host: {0}
rustup home:  {1}

nightly-{0} (directory override for '{2}')
1.3.0 (hash-nightly-2)
",
                    this_host_triple(),
                    config.rustupdir,
                    cwd.display(),
                ),
                r"",
            );
        })
    });
}

#[test]
#[ignore = "FIXME: Windows shows UNC paths"]
fn show_toolchain_toolchain_file_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");

            raw::write_file(&toolchain_file, "nightly").unwrap();

            config.expect_ok_ex(
                &["rustup", "show"],
                &format!(
                    r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------

stable-{0} (default)
nightly-{0}

active toolchain
----------------

nightly-{0} (overridden by '{2}')
1.3.0 (hash-nightly-2)

",
                    this_host_triple(),
                    config.rustupdir,
                    toolchain_file.display()
                ),
                r"",
            );
        })
    });
}

#[test]
#[ignore = "FIXME: Windows shows UNC paths"]
fn show_toolchain_version_nested_file_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "toolchain", "add", "stable", "beta", "nightly"]);
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");

            raw::write_file(&toolchain_file, "nightly").unwrap();

            let subdir = cwd.join("foo");

            fs::create_dir_all(&subdir).unwrap();
            config.change_dir(&subdir, &|config| {
                config.expect_ok_ex(
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
1.3.0 (hash-nightly-2)

",
                        this_host_triple(),
                        toolchain_file.display()
                    ),
                    r"",
                );
            });
        })
    });
}

#[test]
#[ignore = "FIXME: Windows shows UNC paths"]
fn show_toolchain_toolchain_file_override_not_installed() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");

            raw::write_file(&toolchain_file, "nightly").unwrap();

            // I'm not sure this should really be erroring when the toolchain
            // is not installed; just capturing the behavior.
            let mut cmd = clitools::cmd(config, "rustup", ["show"]);
            clitools::env(config, &mut cmd);
            let out = cmd.output().unwrap();
            assert!(!out.status.success());
            let stderr = String::from_utf8(out.stderr).unwrap();
            assert!(stderr.starts_with("error: override toolchain 'nightly' is not installed"));
            assert!(stderr.contains(&format!(
                "the toolchain file at '{}' specifies an uninstalled toolchain",
                toolchain_file.display()
            )));
        })
    });
}

#[test]
fn show_toolchain_override_not_installed() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "override", "add", "nightly"]);
            config.expect_ok(&["rustup", "toolchain", "remove", "nightly"]);
            let out = config.run("rustup", ["show"], &[]);
            assert!(!out.ok);
            assert!(out
                .stderr
                .contains("is not installed: the directory override for"));
            assert!(!out.stderr.contains("info: installing component 'rustc'"));
        })
    });
}

#[test]
fn override_set_unset_with_path() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let cwd = fs::canonicalize(config.current_dir()).unwrap();
            let mut cwd_str = cwd.to_str().unwrap();

            if cfg!(windows) {
                cwd_str = &cwd_str[4..];
            }

            let emptydir = tempfile::tempdir().unwrap();
            config.change_dir(emptydir.path(), &|config| {
                config.expect_ok(&["rustup", "override", "set", "nightly", "--path", cwd_str]);
            });
            config.expect_ok_ex(
                &["rustup", "override", "list"],
                &format!("{}\tnightly-{}\n", cwd_str, this_host_triple()),
                r"",
            );
            config.change_dir(emptydir.path(), &|config| {
                config.expect_ok(&["rustup", "override", "unset", "--path", cwd_str]);
            });
            config.expect_ok_ex(&["rustup", "override", "list"], "no overrides\n", r"");
        })
    });
}

#[test]
fn show_toolchain_env() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            let out = config.run("rustup", ["show"], &[("RUSTUP_TOOLCHAIN", "nightly")]);
            assert!(out.ok);
            assert_eq!(
                &out.stdout,
                for_host_and_home!(
                    config,
                    r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
nightly-{0} (active, default)

active toolchain
----------------
name: nightly-{0}
compiler: 1.3.0 (hash-nightly-2)
active because: overridden by environment variable RUSTUP_TOOLCHAIN
installed targets:
  {0}
"
                )
            );
        })
    });
}

#[test]
fn show_toolchain_env_not_installed() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let out = config.run("rustup", ["show"], &[("RUSTUP_TOOLCHAIN", "nightly")]);

            assert!(!out.ok);

            let expected_out = for_host_and_home!(
                config,
                r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------

active toolchain
----------------
"
            );
            assert!(&out.stdout == expected_out);
            assert!(
                out.stderr
                    == format!(
                        "error: override toolchain 'nightly-{}' is not installed: \
                the RUSTUP_TOOLCHAIN environment variable specifies an uninstalled toolchain\n",
                        this_host_triple()
                    )
            );
        })
    });
}

#[test]
fn show_active_toolchain() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "show", "active-toolchain"],
                for_host!("nightly-{0}\nactive because: it's the default toolchain\n"),
                r"",
            );
        })
    });
}

#[test]
fn show_with_verbose() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
        });
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "update", "nightly-2015-01-01"]);
            config.expect_ok_ex(
                &["rustup", "show", "--verbose"],
                for_host_and_home!(
                    config,
                    r"Default host: {0}
rustup home:  {1}

installed toolchains
--------------------
nightly-2015-01-01-{0}
  1.2.0 (hash-nightly-1)

nightly-{0} (active, default)
  1.3.0 (hash-nightly-2)

active toolchain
----------------
name: nightly-{0}
compiler: 1.3.0 (hash-nightly-2)
active because: it's the default toolchain
installed targets:
  {0}
"
                ),
                r"",
            );
        })
    });
}

#[test]
fn show_active_toolchain_with_verbose() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok_ex(
                &["rustup", "show", "active-toolchain", "--verbose"],
                for_host!(
                    r"nightly-{0}
active because: it's the default toolchain
compiler: 1.3.0 (hash-nightly-2)
"
                ),
                r"",
            );
        })
    });
}

#[test]
fn show_active_toolchain_with_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&["rustup", "override", "set", "stable"]);
            config.expect_stdout_ok(
                &["rustup", "show", "active-toolchain"],
                for_host!("stable-{0}\nactive because: directory override for"),
            );
        })
    });
}

#[test]
fn show_active_toolchain_none() {
    test(&|config| {
        config.expect_ok_ex(
            &["rustup", "show", "active-toolchain"],
            "There isn't an active toolchain\n",
            "",
        );
    });
}

#[test]
fn show_profile() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_stdout_ok(&["rustup", "show", "profile"], "default");

            // Check we get the same thing after we add or remove a component.
            config.expect_ok(&["rustup", "component", "add", "rust-src"]);
            config.expect_stdout_ok(&["rustup", "show", "profile"], "default");
            config.expect_ok(&["rustup", "component", "remove", "rustc"]);
            config.expect_stdout_ok(&["rustup", "show", "profile"], "default");
        })
    });
}

// #846
#[test]
fn set_default_host() {
    test(&|config| {
        config.expect_ok(&["rustup", "set", "default-host", &this_host_triple()]);
        config.expect_stdout_ok(&["rustup", "show"], for_host!("Default host: {0}"));
    });
}

// #846
#[test]
fn set_default_host_invalid_triple() {
    test(&|config| {
        config.expect_err(
            &["rustup", "set", "default-host", "foo"],
            "error: Provided host 'foo' couldn't be converted to partial triple",
        );
    });
}

// #745
#[test]
fn set_default_host_invalid_triple_valid_partial() {
    test(&|config| {
        config.expect_err(
            &["rustup", "set", "default-host", "x86_64-msvc"],
            "error: Provided host 'x86_64-msvc' did not specify an operating system",
        );
    });
}

// #422
#[test]
fn update_doesnt_update_non_tracking_channels() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
        });
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "update", "nightly-2015-01-01"]);
            let mut cmd = clitools::cmd(config, "rustup", ["update"]);
            clitools::env(config, &mut cmd);
            let out = cmd.output().unwrap();
            let stderr = String::from_utf8(out.stderr).unwrap();
            assert!(!stderr.contains(for_host!(
                "syncing channel updates for 'nightly-2015-01-01-{}'"
            )));
        })
    });
}

#[test]
fn toolchain_install_is_like_update() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);
            config.expect_stdout_ok(
                &["rustup", "run", "nightly", "rustc", "--version"],
                "hash-nightly-2",
            );
        })
    });
}

#[test]
fn toolchain_install_is_like_update_quiet() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "--quiet", "toolchain", "install", "nightly"]);
            config.expect_stdout_ok(
                &["rustup", "run", "nightly", "rustc", "--version"],
                "hash-nightly-2",
            );
        })
    });
}

#[test]
fn toolchain_install_is_like_update_except_that_bare_install_is_an_error() {
    test(&|config| {
        config.expect_err(
            &["rustup", "toolchain", "install"],
            "arguments were not provided",
        );
    });
}

#[test]
fn toolchain_update_is_like_update() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "toolchain", "update", "nightly"]);
            config.expect_stdout_ok(
                &["rustup", "run", "nightly", "rustc", "--version"],
                "hash-nightly-2",
            );
        })
    });
}

#[test]
fn toolchain_uninstall_is_like_uninstall() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);
        });
        config.expect_ok(&["rustup", "default", "none"]);
        config.expect_ok(&["rustup", "uninstall", "nightly"]);
        config.expect_not_stdout_ok(&["rustup", "show"], for_host!("'nightly-{}'"));
    });
}

#[test]
fn toolchain_update_is_like_update_except_that_bare_install_is_an_error() {
    test(&|config| {
        config.expect_err(
            &["rustup", "toolchain", "update"],
            "arguments were not provided",
        );
    });
}

#[test]
fn proxy_toolchain_shorthand() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "update", "nightly"]);
            config.expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
            config.expect_stdout_ok(&["rustc", "+stable", "--version"], "hash-stable-1.1.0");
            config.expect_stdout_ok(&["rustc", "+nightly", "--version"], "hash-nightly-2");
        })
    });
}

#[test]
fn add_component() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "component", "add", "rust-src"]);
            let path = format!(
                "toolchains/stable-{}/lib/rustlib/src/rust-src/foo.rs",
                this_host_triple()
            );
            assert!(config.rustupdir.has(path));
        })
    });
}

#[test]
fn add_component_by_target_triple() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&[
                "rustup",
                "component",
                "add",
                &format!("rust-std-{}", clitools::CROSS_ARCH1),
            ]);
            let path = format!(
                "toolchains/stable-{}/lib/rustlib/{}/lib/libstd.rlib",
                this_host_triple(),
                clitools::CROSS_ARCH1
            );
            assert!(config.rustupdir.has(path));
        })
    });
}

#[test]
fn fail_invalid_component_name() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_err(
                &[
                    "rustup",
                    "component",
                    "add",
                    &format!("dummy-{}", clitools::CROSS_ARCH1),
                ],
                &format!("error: toolchain 'stable-{}' does not contain component 'dummy-{}' for target '{}'",this_host_triple(), clitools::CROSS_ARCH1, this_host_triple()),
            );
        })
    });
}

#[test]
fn fail_invalid_component_target() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_err(
                &[
                    "rustup",
                    "component",
                    "add",
                    "rust-std-invalid-target",
                ],
                &format!("error: toolchain 'stable-{}' does not contain component 'rust-std-invalid-target' for target '{}'",this_host_triple(),  this_host_triple()),
            );
        })
    });
}

#[test]
fn remove_component() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "component", "add", "rust-src"]);
            let path = PathBuf::from(format!(
                "toolchains/stable-{}/lib/rustlib/src/rust-src/foo.rs",
                this_host_triple()
            ));
            assert!(config.rustupdir.has(&path));
            config.expect_ok(&["rustup", "component", "remove", "rust-src"]);
            assert!(!config.rustupdir.has(path.parent().unwrap()));
        })
    });
}

#[test]
fn remove_component_by_target_triple() {
    let component_with_triple = format!("rust-std-{}", clitools::CROSS_ARCH1);
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "component", "add", &component_with_triple]);
            let path = PathBuf::from(format!(
                "toolchains/stable-{}/lib/rustlib/{}/lib/libstd.rlib",
                this_host_triple(),
                clitools::CROSS_ARCH1
            ));
            assert!(config.rustupdir.has(&path));
            config.expect_ok(&["rustup", "component", "remove", &component_with_triple]);
            assert!(!config.rustupdir.has(path.parent().unwrap()));
        })
    });
}

#[test]
fn add_remove_multiple_components() {
    let files = [
        "lib/rustlib/src/rust-src/foo.rs".to_owned(),
        format!("lib/rustlib/{}/analysis/libfoo.json", this_host_triple()),
        format!("lib/rustlib/{}/lib/libstd.rlib", clitools::CROSS_ARCH1),
        format!("lib/rustlib/{}/lib/libstd.rlib", clitools::CROSS_ARCH2),
    ];
    let component_with_triple1 = format!("rust-std-{}", clitools::CROSS_ARCH1);
    let component_with_triple2 = format!("rust-std-{}", clitools::CROSS_ARCH2);

    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_ok(&[
                "rustup",
                "component",
                "add",
                "rust-src",
                "rust-analysis",
                &component_with_triple1,
                &component_with_triple2,
            ]);
            for file in &files {
                let path = format!("toolchains/nightly-{}/{}", this_host_triple(), file);
                assert!(config.rustupdir.has(&path));
            }
            config.expect_ok(&[
                "rustup",
                "component",
                "remove",
                "rust-src",
                "rust-analysis",
                &component_with_triple1,
                &component_with_triple2,
            ]);
            for file in &files {
                let path = PathBuf::from(format!(
                    "toolchains/nightly-{}/{}",
                    this_host_triple(),
                    file
                ));
                assert!(!config.rustupdir.has(path.parent().unwrap()));
            }
        })
    });
}

#[test]
fn file_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            config.expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(&toolchain_file, "nightly").unwrap();

            config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
        })
    });
}

#[test]
fn env_override_path() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let toolchain_path = config
                .rustupdir
                .join("toolchains")
                .join(format!("nightly-{}", this_host_triple()));

            let out = config.run(
                "rustc",
                ["--version"],
                &[("RUSTUP_TOOLCHAIN", toolchain_path.to_str().unwrap())],
            );
            assert!(out.ok);
            assert!(out.stdout.contains("hash-nightly-2"));
        })
    });
}

#[test]
fn plus_override_relpath_is_not_supported() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let toolchain_path = Path::new("..")
                .join(config.rustupdir.rustupdir.file_name().unwrap())
                .join("toolchains")
                .join(format!("nightly-{}", this_host_triple()));
            config.expect_err(
                &[
                    "rustc",
                    format!("+{}", toolchain_path.to_str().unwrap()).as_str(),
                    "--version",
                ],
                "error: relative path toolchain",
            );
        })
    });
}

#[test]
fn run_with_relpath_is_not_supported() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let toolchain_path = Path::new("..")
                .join(config.rustupdir.rustupdir.file_name().unwrap())
                .join("toolchains")
                .join(format!("nightly-{}", this_host_triple()));
            config.expect_err(
                &[
                    "rustup",
                    "run",
                    toolchain_path.to_str().unwrap(),
                    "rustc",
                    "--version",
                ],
                "relative path toolchain",
            );
        })
    });
}

#[test]
fn plus_override_abspath_is_supported() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let toolchain_path = config
                .rustupdir
                .join("toolchains")
                .join(format!("nightly-{}", this_host_triple()))
                .canonicalize()
                .unwrap();
            config.expect_ok(&[
                "rustc",
                format!("+{}", toolchain_path.to_str().unwrap()).as_str(),
                "--version",
            ]);
        })
    });
}

#[test]
fn run_with_abspath_is_supported() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let toolchain_path = config
                .rustupdir
                .join("toolchains")
                .join(format!("nightly-{}", this_host_triple()))
                .canonicalize()
                .unwrap();
            config.expect_ok(&[
                "rustup",
                "run",
                toolchain_path.to_str().unwrap(),
                "rustc",
                "--version",
            ]);
        })
    });
}

#[test]
fn file_override_path() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let toolchain_path = config
                .rustupdir
                .join("toolchains")
                .join(format!("nightly-{}", this_host_triple()));
            let toolchain_file = config.current_dir().join("rust-toolchain.toml");
            raw::write_file(
                &toolchain_file,
                &format!("[toolchain]\npath='{}'", toolchain_path.to_str().unwrap()),
            )
            .unwrap();

            config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");

            // Check that the toolchain has the right name
            config.expect_stdout_ok(
                &["rustup", "show", "active-toolchain"],
                &format!("nightly-{}", this_host_triple()),
            );
        })
    });
}

#[test]
fn proxy_override_path() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let toolchain_path = config
                .rustupdir
                .join("toolchains")
                .join(format!("nightly-{}", this_host_triple()));
            let toolchain_file = config.current_dir().join("rust-toolchain.toml");
            raw::write_file(
                &toolchain_file,
                &format!("[toolchain]\npath='{}'", toolchain_path.to_str().unwrap()),
            )
            .unwrap();

            config.expect_stdout_ok(&["cargo", "--call-rustc"], "hash-nightly-2");
        })
    });
}

#[test]
fn file_override_path_relative_not_supported() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let toolchain_path = config
                .rustupdir
                .join("toolchains")
                .join(format!("nightly-{}", this_host_triple()));
            let toolchain_file = config.current_dir().join("rust-toolchain.toml");

            // Find shared prefix so we can determine a relative path
            let mut p1 = toolchain_path.components().peekable();
            let mut p2 = toolchain_file.components().peekable();
            while let (Some(p1p), Some(p2p)) = (p1.peek(), p2.peek()) {
                if p1p == p2p {
                    let _ = p1.next();
                    let _ = p2.next();
                } else {
                    // The two paths diverge here
                    break;
                }
            }
            let mut relative_path = PathBuf::new();
            // NOTE: We skip 1 since we don't need to .. across the .toml file at the end of the path
            for _ in p2.skip(1) {
                relative_path.push("..");
            }
            for p in p1 {
                relative_path.push(p);
            }
            assert!(relative_path.is_relative());

            raw::write_file(
                &toolchain_file,
                &format!("[toolchain]\npath='{}'", relative_path.to_str().unwrap()),
            )
            .unwrap();

            // Change into an ephemeral dir so that we test that the path is relative to the override
            let ephemeral = config.current_dir().join("ephemeral");
            fs::create_dir_all(&ephemeral).unwrap();
            config.change_dir(&ephemeral, &|config| {
                config.expect_err(&["rustc", "--version"], "relative path toolchain");
            });
        })
    });
}

#[test]
fn file_override_path_no_options() {
    test(&|config| {
        // Make a plausible-looking toolchain
        let cwd = config.current_dir();
        let toolchain_path = cwd.join("ephemeral");
        let toolchain_bin = toolchain_path.join("bin");
        fs::create_dir_all(toolchain_bin).unwrap();

        let toolchain_file = cwd.join("rust-toolchain.toml");
        raw::write_file(
            &toolchain_file,
            "[toolchain]\npath=\"ephemeral\"\ntargets=[\"dummy\"]",
        )
        .unwrap();

        config.expect_err(
            &["rustc", "--version"],
            "toolchain options are ignored for path toolchain (ephemeral)",
        );

        raw::write_file(
            &toolchain_file,
            "[toolchain]\npath=\"ephemeral\"\ncomponents=[\"dummy\"]",
        )
        .unwrap();

        config.expect_err(
            &["rustc", "--version"],
            "toolchain options are ignored for path toolchain (ephemeral)",
        );

        raw::write_file(
            &toolchain_file,
            "[toolchain]\npath=\"ephemeral\"\nprofile=\"minimal\"",
        )
        .unwrap();

        config.expect_err(
            &["rustc", "--version"],
            "toolchain options are ignored for path toolchain (ephemeral)",
        );
    });
}

#[test]
fn file_override_path_xor_channel() {
    test(&|config| {
        // Make a plausible-looking toolchain
        let cwd = config.current_dir();
        let toolchain_path = cwd.join("ephemeral");
        let toolchain_bin = toolchain_path.join("bin");
        fs::create_dir_all(toolchain_bin).unwrap();

        let toolchain_file = cwd.join("rust-toolchain.toml");
        raw::write_file(
            &toolchain_file,
            "[toolchain]\npath=\"ephemeral\"\nchannel=\"nightly\"",
        )
        .unwrap();

        config.expect_err(
            &["rustc", "--version"],
            "cannot specify both channel (nightly) and path (ephemeral) simultaneously",
        );
    });
}

#[test]
fn file_override_subdir() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            config.expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(&toolchain_file, "nightly").unwrap();

            let subdir = cwd.join("subdir");
            fs::create_dir_all(&subdir).unwrap();
            config.change_dir(&subdir, &|config| {
                config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
            });
        })
    });
}

#[test]
fn file_override_with_archive() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
        });
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "toolchain", "install", "nightly-2015-01-01"]);

            config.expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(&toolchain_file, "nightly-2015-01-01").unwrap();

            config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
        })
    });
}

#[test]
fn file_override_toml_format_select_installed_toolchain() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
        });
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_ok(&["rustup", "toolchain", "install", "nightly-2015-01-01"]);

            config.expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(
                &toolchain_file,
                r#"
[toolchain]
channel = "nightly-2015-01-01"
"#,
            )
            .unwrap();

            config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
        })
    });
}

#[test]
fn file_override_toml_format_install_both_toolchain_and_components() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
        });
        config.with_scenario(Scenario::ArchivesV2_2015_01_01, &|config| {
            config.expect_stdout_ok(&["rustc", "--version"], "hash-stable-1.1.0");
            config.expect_not_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)");

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(
                &toolchain_file,
                r#"
[toolchain]
channel = "nightly-2015-01-01"
components = [ "rust-src" ]
"#,
            )
            .unwrap();

            config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
            config.expect_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)");
        })
    });
}

#[test]
fn file_override_toml_format_add_missing_components() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_not_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)");

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(
                &toolchain_file,
                r#"
[toolchain]
components = [ "rust-src" ]
"#,
            )
            .unwrap();

            config.expect_stdout_ok(&["rustup", "component", "list"], "rust-src (installed)");
        })
    });
}

#[test]
fn file_override_toml_format_add_missing_targets() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_not_stdout_ok(
                &["rustup", "component", "list"],
                "arm-linux-androideabi (installed)",
            );

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(
                &toolchain_file,
                r#"
[toolchain]
targets = [ "arm-linux-androideabi" ]
"#,
            )
            .unwrap();

            config.expect_stdout_ok(
                &["rustup", "component", "list"],
                "arm-linux-androideabi (installed)",
            );
        })
    });
}

#[test]
fn file_override_toml_format_skip_invalid_component() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(
                &toolchain_file,
                r#"
[toolchain]
components = [ "rust-bongo" ]
"#,
            )
            .unwrap();

            config.expect_stderr_ok(
                &["rustc", "--version"],
                "warning: Force-skipping unavailable component 'rust-bongo",
            );
        })
    });
}

#[test]
fn file_override_toml_format_specify_profile() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "set", "profile", "default"]);
            config.expect_stderr_ok(
                &["rustup", "default", "stable"],
                "downloading component 'rust-docs'",
            );

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(
                &toolchain_file,
                r#"
[toolchain]
profile = "minimal"
channel = "nightly"
"#,
            )
            .unwrap();
            config.expect_not_stdout_ok(
                &["rustup", "component", "list"],
                for_host!("rust-docs-{} (installed)"),
            );
        })
    });
}

#[test]
fn close_file_override_beats_far_directory_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "beta"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            config.expect_ok(&["rustup", "override", "set", "beta"]);
            config.expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");

            let cwd = config.current_dir();

            let subdir = cwd.join("subdir");
            fs::create_dir_all(&subdir).unwrap();

            let toolchain_file = subdir.join("rust-toolchain");
            raw::write_file(&toolchain_file, "nightly").unwrap();

            config.change_dir(&subdir, &|config| {
                config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
            });
        })
    });
}

#[test]
// Check that toolchain overrides have the correct priority.
fn override_order() {
    test(&|config| {
        config.with_scenario(Scenario::ArchivesV2, &|config| {
            let host = this_host_triple();
            // give each override type a different toolchain
            let default_tc = &format!("beta-2015-01-01-{}", host);
            let env_tc = &format!("stable-2015-01-01-{}", host);
            let dir_tc = &format!("beta-2015-01-02-{}", host);
            let file_tc = &format!("stable-2015-01-02-{}", host);
            let command_tc = &format!("nightly-2015-01-01-{}", host);
            config.expect_ok(&["rustup", "install", default_tc]);
            config.expect_ok(&["rustup", "install", env_tc]);
            config.expect_ok(&["rustup", "install", dir_tc]);
            config.expect_ok(&["rustup", "install", file_tc]);
            config.expect_ok(&["rustup", "install", command_tc]);

            // No default
            config.expect_ok(&["rustup", "default", "none"]);
            config.expect_stdout_ok(
                &["rustup", "show", "active-toolchain"],
                "There isn't an active toolchain\n",
            );

            // Default
            config.expect_ok(&["rustup", "default", default_tc]);
            config.expect_stdout_ok(&["rustup", "show", "active-toolchain"], default_tc);

            // file > default
            let toolchain_file = config.current_dir().join("rust-toolchain.toml");
            raw::write_file(
                &toolchain_file,
                &format!("[toolchain]\nchannel='{}'", file_tc),
            )
            .unwrap();
            config.expect_stdout_ok(&["rustup", "show", "active-toolchain"], file_tc);

            // dir override > file > default
            config.expect_ok(&["rustup", "override", "set", dir_tc]);
            config.expect_stdout_ok(&["rustup", "show", "active-toolchain"], dir_tc);

            // env > dir override > file > default
            let out = config.run(
                "rustup",
                ["show", "active-toolchain"],
                &[("RUSTUP_TOOLCHAIN", env_tc)],
            );
            assert!(out.ok);
            assert!(out.stdout.contains(env_tc));

            // +toolchain > env > dir override > file > default
            let out = config.run(
                "rustup",
                [&format!("+{}", command_tc), "show", "active-toolchain"],
                &[("RUSTUP_TOOLCHAIN", env_tc)],
            );
            assert!(out.ok);
            assert!(out.stdout.contains(command_tc));
        })
    });
}

#[test]
fn directory_override_doesnt_need_to_exist_unless_it_is_selected() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "beta"]);
            // not installing nightly

            config.expect_ok(&["rustup", "override", "set", "beta"]);
            config.expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(&toolchain_file, "nightly").unwrap();

            config.expect_stdout_ok(&["rustc", "--version"], "hash-beta-1.2.0");
        })
    });
}

#[test]
fn env_override_beats_file_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "beta"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(&toolchain_file, "nightly").unwrap();

            let mut cmd = clitools::cmd(config, "rustc", ["--version"]);
            clitools::env(config, &mut cmd);
            cmd.env("RUSTUP_TOOLCHAIN", "beta");

            let out = cmd.output().unwrap();
            assert!(String::from_utf8(out.stdout)
                .unwrap()
                .contains("hash-beta-1.2.0"));
        })
    });
}

#[test]
fn plus_override_beats_file_override() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "beta"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            raw::write_file(&toolchain_file, "nightly").unwrap();

            config.expect_stdout_ok(&["rustc", "+beta", "--version"], "hash-beta-1.2.0");
        })
    });
}

#[test]
fn file_override_not_installed_custom() {
    test(&|config| {
        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "gumbo").unwrap();

        config.expect_err(&["rustc", "--version"], "custom and not installed");
    });
}

#[test]
fn bad_file_override() {
    test(&|config| {
        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        // invalid name - cannot specify no toolchain in a toolchain file
        raw::write_file(&toolchain_file, "none").unwrap();

        config.expect_err(&["rustc", "--version"], "invalid toolchain name 'none'");
    });
}

#[test]
fn valid_override_settings() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain");
            config.expect_ok(&["rustup", "default", "nightly"]);
            raw::write_file(&toolchain_file, "nightly").unwrap();
            config.expect_ok(&["rustc", "--version"]);
            // Special case: same version as is installed is permitted.
            raw::write_file(&toolchain_file, for_host!("nightly-{}")).unwrap();
            config.expect_ok(&["rustc", "--version"]);
            let fullpath = config
                .rustupdir
                .clone()
                .join("toolchains")
                .join(for_host!("nightly-{}"));
            config.expect_ok(&[
                "rustup",
                "toolchain",
                "link",
                "system",
                &format!("{}", fullpath.display()),
            ]);
            raw::write_file(&toolchain_file, "system").unwrap();
            config.expect_ok(&["rustc", "--version"]);
        })
    })
}

#[test]
fn file_override_with_target_info() {
    // Target info is not portable between machines, so we reject toolchain
    // files that include it.
    test(&|config| {
        let cwd = config.current_dir();
        let toolchain_file = cwd.join("rust-toolchain");
        raw::write_file(&toolchain_file, "nightly-x86_64-unknown-linux-gnu").unwrap();

        config.expect_err(
            &["rustc", "--version"],
            "target triple in channel name 'nightly-x86_64-unknown-linux-gnu'",
        );
    });
}

#[test]
fn docs_with_path() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "stable"]);
            config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

            let mut cmd = clitools::cmd(config, "rustup", ["doc", "--path"]);
            clitools::env(config, &mut cmd);

            let out = cmd.output().unwrap();
            let path = format!("share{MAIN_SEPARATOR}doc{MAIN_SEPARATOR}rust{MAIN_SEPARATOR}html");
            assert!(String::from_utf8(out.stdout).unwrap().contains(&path));

            config.expect_stdout_ok(
                &["rustup", "doc", "--path", "--toolchain", "nightly"],
                "nightly",
            );
        })
    });
}

#[test]
fn docs_topical_with_path() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {

        config.expect_ok(&["rustup", "default", "stable"]);
        config.expect_ok(&["rustup", "toolchain", "install", "nightly"]);

        for (topic, path) in mock::topical_doc_data::test_cases() {
            let mut cmd = clitools::cmd(config, "rustup", ["doc", "--path", topic]);
            clitools::env(config, &mut cmd);

            let out = cmd.output().unwrap();
            eprintln!("{:?}", String::from_utf8(out.stderr).unwrap());
            let out_str = String::from_utf8(out.stdout).unwrap();
            assert!(
                out_str.contains(&path),
                "comparing path\ntopic: '{topic}'\nexpected path: '{path}'\noutput: {out_str}\n\n\n",
            );
        }
    })
    });
}

#[test]
fn docs_missing() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "set", "profile", "minimal"]);
            config.expect_ok(&["rustup", "default", "nightly"]);
            config.expect_err(
                &["rustup", "doc"],
                "error: unable to view documentation which is not installed",
            );
        })
    });
}

#[test]
fn docs_custom() {
    test(&|config| {
        let path = config.customdir.join("custom-1");
        let path = path.to_string_lossy();
        config.expect_ok(&["rustup", "toolchain", "link", "custom", &path]);
        config.expect_ok(&["rustup", "default", "custom"]);
        config.expect_stdout_ok(&["rustup", "doc", "--path"], "custom");
    });
}

#[cfg(unix)]
#[test]
fn non_utf8_arg() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            let out = config.run(
                "rustc",
                [
                    OsStr::new("--echo-args"),
                    OsStr::new("echoed non-utf8 arg:"),
                    OsStr::from_bytes(b"\xc3\x28"),
                ],
                &[("RUST_BACKTRACE", "1")],
            );
            assert!(out.stderr.contains("echoed non-utf8 arg"));
        })
    });
}

#[cfg(windows)]
#[test]
fn non_utf8_arg() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            let out = config.run(
                "rustc",
                [
                    OsString::from("--echo-args".to_string()),
                    OsString::from("echoed non-utf8 arg:".to_string()),
                    OsString::from_wide(&[0xd801, 0xd801]),
                ],
                &[("RUST_BACKTRACE", "1")],
            );
            assert!(out.stderr.contains("echoed non-utf8 arg"));
        })
    });
}

#[cfg(unix)]
#[test]
fn non_utf8_toolchain() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            let out = config.run(
                "rustc",
                [OsStr::from_bytes(b"+\xc3\x28")],
                &[("RUST_BACKTRACE", "1")],
            );
            assert!(out.stderr.contains("toolchain '�(' is not installable"));
        })
    });
}

#[cfg(windows)]
#[test]
fn non_utf8_toolchain() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
            let out = config.run(
                "rustc",
                [OsString::from_wide(&[u16::from(b'+'), 0xd801, 0xd801])],
                &[("RUST_BACKTRACE", "1")],
            );
            assert!(out.stderr.contains("toolchain '��' is not installable"));
        })
    });
}

#[test]
fn check_host_goes_away() {
    test(&|config| {
        config.with_scenario(Scenario::HostGoesMissingBefore, &|config| {
            config.expect_ok(&["rustup", "default", "nightly"]);
        });
        config.with_scenario(Scenario::HostGoesMissingAfter, &|config| {
            config.expect_err(
                &["rustup", "update", "nightly"],
                for_host!("target '{}' not found in channel"),
            );
        })
    })
}

#[cfg(unix)]
#[test]
fn check_unix_settings_fallback() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            // No default toolchain specified yet
            config.expect_stdout_ok(&["rustup", "default"], "no default toolchain is configured");

            // Default toolchain specified in fallback settings file
            let mock_settings_file = config.current_dir().join("mock_fallback_settings.toml");
            raw::write_file(
                &mock_settings_file,
                for_host!(r"default_toolchain = 'nightly-{0}'"),
            )
            .unwrap();

            let mut cmd = clitools::cmd(config, "rustup", ["default"]);
            clitools::env(config, &mut cmd);

            // Override the path to the fallback settings file to be the mock file
            cmd.env("RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS", mock_settings_file);

            let out = cmd.output().unwrap();
            assert!(out.status.success());
            let stdout = String::from_utf8(out.stdout).unwrap();
            assert_eq!(
                &stdout,
                for_host!(
                    r"nightly-{0} (default)
"
                )
            );
        })
    });
}

#[test]
fn warn_on_unmatch_build() {
    test(&|config| {
        config.with_scenario(Scenario::MultiHost, &|config| {
        let arch = clitools::MULTI_ARCH1;
        config.expect_stderr_ok(
            &["rustup", "toolchain", "install", &format!("nightly-{arch}")],
            &format!(
                r"warning: toolchain 'nightly-{arch}' may not be able to run on this system.
warning: If you meant to build software to target that platform, perhaps try `rustup target add {arch}` instead?",
            ),
        );
        })
    });
}

#[test]
fn dont_warn_on_partial_build() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let triple = this_host_triple();
            let arch = triple.split('-').next().unwrap();
            let mut cmd = clitools::cmd(
                config,
                "rustup",
                ["toolchain", "install", &format!("nightly-{arch}")],
            );
            clitools::env(config, &mut cmd);
            let out = cmd.output().unwrap();
            assert!(out.status.success());
            let stderr = String::from_utf8(out.stderr).unwrap();
            assert!(stderr.contains(&format!(
                r"info: syncing channel updates for 'nightly-{triple}'"
            )));
            assert!(!stderr.contains(&format!(
                r"warning: toolchain 'nightly-{arch}' may not be able to run on this system."
            )));
        })
    })
}

/// Checks that `rust-toolchain.toml` files are considered
#[test]
fn rust_toolchain_toml() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            config.expect_err(
                &["rustc", "--version"],
                "rustup could not choose a version of rustc to run",
            );

            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain.toml");
            raw::write_file(&toolchain_file, "[toolchain]\nchannel = \"nightly\"").unwrap();

            config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
        })
    });
}

/// Ensures that `rust-toolchain.toml` files (with `.toml` extension) only allow TOML contents
#[test]
fn only_toml_in_rust_toolchain_toml() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let cwd = config.current_dir();
            let toolchain_file = cwd.join("rust-toolchain.toml");
            raw::write_file(&toolchain_file, "nightly").unwrap();

            config.expect_err(&["rustc", "--version"], "error parsing override file");
        })
    });
}

/// Checks that a warning occurs if both `rust-toolchain` and `rust-toolchain.toml` files exist
#[test]
fn warn_on_duplicate_rust_toolchain_file() {
    test(&|config| {
        config.with_scenario(Scenario::SimpleV2, &|config| {
            let cwd = config.current_dir();
            let toolchain_file_1 = cwd.join("rust-toolchain");
            raw::write_file(&toolchain_file_1, "stable").unwrap();
            let toolchain_file_2 = cwd.join("rust-toolchain.toml");
            raw::write_file(&toolchain_file_2, "[toolchain]").unwrap();

            config.expect_stderr_ok(
                &["rustc", "--version"],
                &format!(
                    "warning: both `{0}` and `{1}` exist. Using `{0}`",
                    toolchain_file_1.canonicalize().unwrap().display(),
                    toolchain_file_2.canonicalize().unwrap().display(),
                ),
            );
        })
    });
}
