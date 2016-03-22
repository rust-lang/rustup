//! Yet more cli test cases. These are testing that the output
//! is exactly as expected.

extern crate multirust_dist;
extern crate multirust_mock;

use multirust_mock::clitools::{self, Config, Scenario,
                               expect_ok, expect_ok_ex,
                               expect_err_ex,
                               this_host_triple};
use std::env;

fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, f);
}

#[test]
fn update() {
    setup(&|config| {
        expect_ok_ex(config, &["multirust", "update", "nightly"],
r"
nightly revision:

1.3.0 (hash-n-2)
1.3.0 (hash-n-2)

",
r"info: installing toolchain 'nightly'
info: downloading toolchain manifest
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: toolchain 'nightly' installed
");
    });
}

#[test]
fn update_again() {
    setup(&|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_ok_ex(config, &["multirust", "update", "nightly"],
r"
nightly revision:

1.3.0 (hash-n-2)
1.3.0 (hash-n-2)

",
r"info: updating existing install for 'nightly'
info: downloading toolchain manifest
info: toolchain is already up to date
");
    });
}

#[test]
fn default() {
    setup(&|config| {
        expect_ok_ex(config, &["multirust", "default", "nightly"],
r"
nightly revision:

1.3.0 (hash-n-2)
1.3.0 (hash-n-2)

",
r"info: installing toolchain 'nightly'
info: downloading toolchain manifest
info: downloading component 'rust-std'
info: downloading component 'rustc'
info: downloading component 'cargo'
info: downloading component 'rust-docs'
info: installing component 'rust-std'
info: installing component 'rustc'
info: installing component 'cargo'
info: installing component 'rust-docs'
info: toolchain 'nightly' installed
info: default toolchain set to 'nightly'
");
    });
}

#[test]
fn override_again() {
    setup(&|config| {
        let cwd = env::current_dir().unwrap();
        expect_ok(config, &["multirust", "override", "nightly"]);
        expect_ok_ex(config, &["multirust", "override", "nightly"],
r"
nightly revision:

1.3.0 (hash-n-2)
1.3.0 (hash-n-2)

",
&format!(
r"info: using existing install for 'nightly'
info: override toolchain for '{}' set to 'nightly'
", cwd.display()));
    });
}

#[test]
fn remove_override() {
    setup(&|config| {
        let cwd = env::current_dir().unwrap();
        expect_ok(config, &["multirust", "override", "nightly"]);
        expect_ok_ex(config, &["multirust", "remove-override"],
r"",
&format!(r"info: override toolchain for '{}' removed
", cwd.display()));
    });
}

#[test]
fn remove_override_none() {
    setup(&|config| {
        let cwd = env::current_dir().unwrap();
        expect_ok_ex(config, &["multirust", "remove-override"],
r"",
&format!(r"info: no override toolchain for '{}'
", cwd.display()));
    });
}

#[test]
fn update_no_manifest() {
    setup(&|config| {
        expect_err_ex(config, &["multirust", "update", "nightly-2016-01-01"],
r"",
r"info: installing toolchain 'nightly-2016-01-01'
info: downloading toolchain manifest
error: no release found for 'nightly-2016-01-01'
");
    });
}

#[test]
fn delete_data() {
    setup(&|config| {
        expect_ok(config, &["multirust", "default", "nightly"]);
        assert!(config.rustupdir.path().exists());
        expect_ok_ex(config, &["multirust", "delete-data", "-y"],
r"",
&format!(
r"info: deleted directory '{}'
", config.rustupdir.path().display()));
    });
}

// Issue #111
// multirust update nightly-2016-03-1
#[test]
fn update_invalid_toolchain() {
   setup(&|config| {
        expect_err_ex(config, &["multirust", "update", "nightly-2016-03-1"],
r"",
r"error: toolchain 'nightly-2016-03-1' is not installed
");
   });
 }

#[test]
fn default_invalid_toolchain() {
   setup(&|config| {
        expect_err_ex(config, &["multirust", "default", "nightly-2016-03-1"],
r"",
r"error: toolchain 'nightly-2016-03-1' is not installed
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

        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_ok(config, &["multirust", "add-target", "nightly",
                            clitools::CROSS_ARCH1]);
        expect_ok_ex(config, &["multirust", "list-targets", "nightly"],
&expected,
r"");
    });
}
