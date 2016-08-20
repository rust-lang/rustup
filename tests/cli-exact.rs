//! Yet more cli test cases. These are testing that the output
//! is exactly as expected.

extern crate rustup_dist;
extern crate rustup_mock;
extern crate tempdir;
extern crate rustup_utils;

use rustup_mock::clitools::{self, Config, Scenario,
                               expect_ok, expect_ok_ex,
                               expect_err_ex,
                               this_host_triple, change_dir};
use std::env;

macro_rules! for_host { ($s: expr) => (&format!($s, this_host_triple())) }

fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, f);
}

#[test]
fn update() {
    setup(&|config| {
        expect_ok_ex(config, &["rustup", "update", "nightly"],
for_host!(r"
  nightly-{0} installed - 1.3.0 (hash-n-2)

"),
for_host!(r"info: syncing channel updates for 'nightly-{0}'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
"));
    });
}

#[test]
fn update_again() {
    setup(&|config| {
        expect_ok(config, &["rustup", "update", "nightly"]);
        expect_ok_ex(config, &["rustup", "update", "nightly"],
for_host!(r"
  nightly-{0} unchanged - 1.3.0 (hash-n-2)

"),
for_host!(r"info: syncing channel updates for 'nightly-{0}'
"));
    });
}

#[test]
fn default() {
    setup(&|config| {
        expect_ok_ex(config, &["rustup", "default", "nightly"],
for_host!(r"
  nightly-{0} installed - 1.3.0 (hash-n-2)

"),
for_host!(r"info: syncing channel updates for 'nightly-{0}'
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: default toolchain set to 'nightly-{0}'
"));
    });
}

#[test]
fn override_again() {
    setup(&|config| {
        let cwd = env::current_dir().unwrap();
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_ok_ex(config, &["rustup", "override", "add", "nightly"],
for_host!(r"
  nightly-{} unchanged - 1.3.0 (hash-n-2)

"),
&format!(
r"info: using existing install for 'nightly-{1}'
info: override toolchain for '{}' set to 'nightly-{1}'
", cwd.display(), &this_host_triple()));
    });
}

#[test]
fn remove_override() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let cwd = env::current_dir().unwrap();
            expect_ok(config, &["rustup", "override", "add", "nightly"]);
            expect_ok_ex(config, &["rustup", "override", keyword],
                         r"",
                         &format!("info: override toolchain for '{}' removed\n", cwd.display()));
        });

    }
}

#[test]
fn remove_override_none() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let cwd = env::current_dir().unwrap();
            expect_ok_ex(config, &["rustup", "override", keyword],
                         r"",
                         &format!("info: no override toolchain for '{}'
info: you may use `--path <path>` option to remove override toolchain for a specific path\n",
                                  cwd.display()));
        });
    }
}

#[test]
fn remove_override_with_path() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let dir = tempdir::TempDir::new("rustup-test").unwrap();
            change_dir(dir.path(), &|| {
                expect_ok(config, &["rustup", "override", "add", "nightly"]);
            });
            expect_ok_ex(config, &["rustup", "override", keyword, "--path", dir.path().to_str().unwrap()],
                         r"",
                         &format!("info: override toolchain for '{}' removed\n", dir.path().display()));
        });

    }
}

#[test]
fn remove_override_with_path_deleted() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let path = {
                let dir = tempdir::TempDir::new("rustup-test").unwrap();
                let path = std::fs::canonicalize(dir.path()).unwrap();
                change_dir(&path, &|| {
                  expect_ok(config, &["rustup", "override", "add", "nightly"]);
                });
              path
            };
            expect_ok_ex(config, &["rustup", "override", keyword, "--path", path.to_str().unwrap()],
                         r"",
                         &format!("info: override toolchain for '{}' removed\n", path.display()));
        });
    }
}

#[test]
fn remove_override_nonexistent() {
    for keyword in &["remove", "unset"] {
        setup(&|config| {
            let path = {
                let dir = tempdir::TempDir::new("rustup-test").unwrap();
                let path = std::fs::canonicalize(dir.path()).unwrap();
                change_dir(&path, &|| {
                  expect_ok(config, &["rustup", "override", "add", "nightly"]);
                });
                path
            };
            // FIXME TempDir seems to succumb to difficulties removing dirs on windows
            let _ = rustup_utils::raw::remove_dir(&path);
            assert!(!path.exists());
            expect_ok_ex(config, &["rustup", "override", keyword, "--nonexistent"],
                         r"",
                         &format!("info: override toolchain for '{}' removed\n", path.display()));
        });
    }
}


#[test]
fn list_overrides() {
    setup(&|config| {
        let cwd = std::fs::canonicalize(env::current_dir().unwrap()).unwrap();
        let mut cwd_formatted = format!("{}", cwd.display()).to_string();

        if cfg!(windows) {
            cwd_formatted = cwd_formatted[4..].to_owned();
        }

        let trip = this_host_triple();
        expect_ok(config, &["rustup", "override", "add", "nightly"]);
        expect_ok_ex(config, &["rustup", "override", "list"],
                     &format!("{:<40}\t{:<20}\n", cwd_formatted, &format!("nightly-{}", trip)), r"");
    });
}


#[test]
fn list_overrides_with_nonexistent() {
    setup(&|config| {

        let trip = this_host_triple();

        let nonexistent_path = {
            let dir = tempdir::TempDir::new("rustup-test").unwrap();
            change_dir(dir.path(), &|| {
                expect_ok(config, &["rustup", "override", "add", "nightly"]);
            });
            std::fs::canonicalize(dir.path()).unwrap()
        };
        // FIXME TempDir seems to succumb to difficulties removing dirs on windows
        let _ = rustup_utils::raw::remove_dir(&nonexistent_path);
        assert!(!nonexistent_path.exists());
        let mut path_formatted = format!("{}", nonexistent_path.display()).to_string();

        if cfg!(windows) {
            path_formatted = path_formatted[4..].to_owned();
        }

        expect_ok_ex(config, &["rustup", "override", "list"],
                     &format!("{:<40}\t{:<20}\n\n",
                              path_formatted + " (not a directory)",
                              &format!("nightly-{}", trip)),
                              "info: you may remove overrides for non-existent directories with
`rustup override unset --nonexistent`\n");
    });
}



#[test]
fn update_no_manifest() {
    setup(&|config| {
        expect_err_ex(config, &["rustup", "update", "nightly-2016-01-01"],
r"",
for_host!(r"info: syncing channel updates for 'nightly-2016-01-01-{0}'
error: no release found for 'nightly-2016-01-01'
"));
    });
}

// Issue #111
#[test]
fn update_invalid_toolchain() {
   setup(&|config| {
        expect_err_ex(config, &["rustup", "update", "nightly-2016-03-1"],
r"",
r"info: syncing channel updates for 'nightly-2016-03-1'
error: target not found: '2016-03-1'
");
   });
 }

#[test]
fn default_invalid_toolchain() {
   setup(&|config| {
        expect_err_ex(config, &["rustup", "default", "nightly-2016-03-1"],
r"",
r"info: syncing channel updates for 'nightly-2016-03-1'
error: target not found: '2016-03-1'
");
   });
}

#[test]
fn list_targets() {
    setup(&|config| {
        let trip = this_host_triple();
        let mut sorted = vec![format!("{} (default)", &*trip),
                              format!("{} (installed)", clitools::CROSS_ARCH1),
                              clitools::CROSS_ARCH2.to_string()];
        sorted.sort();

        let expected = format!("{}\n{}\n{}\n", sorted[0], sorted[1], sorted[2]);

        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "target", "add",
                            clitools::CROSS_ARCH1]);
        expect_ok_ex(config, &["rustup", "target", "list"],
&expected,
r"");
    });
}

#[test]
fn cross_install_indicates_target() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok_ex(config, &["rustup", "target", "add", clitools::CROSS_ARCH1],
r"",
&format!(r"info: downloading component 'rust-std' for '{0}'
info: installing component 'rust-std' for '{0}'
", clitools::CROSS_ARCH1));
    });
}


#[test]
fn enable_telemetry() {
    setup(&|config| {
        expect_ok_ex(config,
                     &["rustup", "telemetry", "enable"],
                     r"",
                     &format!("info: telemetry set to 'on'\n"));
    });
}

#[test]
fn disable_telemetry() {
    setup(&|config| {
        expect_ok_ex(config,
                     &["rustup", "telemetry", "disable"],
                     r"",
                     &format!("info: telemetry set to 'off'\n"));
    });
}
