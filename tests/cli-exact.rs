//! Yet more cli test cases. These are testing that the output
//! is exactly as expected.

extern crate multirust_dist;
extern crate multirust_mock;

use multirust_mock::clitools::{self, Config, Scenario,
                               expect_ok, expect_ok_ex,
                               expect_err_ex};
use std::env;

pub fn setup(f: &Fn(&Config)) {
    clitools::setup(Scenario::SimpleV2, f);
}

#[test]
pub fn update() {
    setup(&|config| {
        expect_ok_ex(config, &["multirust", "update", "nightly"],
r"
nightly revision:

1.3.0 (hash-n-2)
1.3.0 (hash-n-2)

",
r"info: installing toolchain 'nightly'
info: downloading manifest
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
pub fn update_again() {
    setup(&|config| {
        expect_ok(config, &["multirust", "update", "nightly"]);
        expect_ok_ex(config, &["multirust", "update", "nightly"],
r"
nightly revision:

1.3.0 (hash-n-2)
1.3.0 (hash-n-2)

",
r"info: updating existing install for 'nightly'
info: downloading manifest
info: toolchain is already up to date
");
    });
}

#[test]
pub fn default() {
    setup(&|config| {
        expect_ok_ex(config, &["multirust", "update", "nightly"],
r"
nightly revision:

1.3.0 (hash-n-2)
1.3.0 (hash-n-2)

",
r"info: installing toolchain 'nightly'
info: downloading manifest
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
pub fn override_again() {
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
pub fn update_no_manifest() {
    setup(&|config| {
        expect_err_ex(config, &["multirust", "update", "nightly-2016-01-01"],
r"",
r"info: installing toolchain 'nightly-2016-01-01'
info: downloading manifest
error: no release found for 'nightly-2016-01-01'
");
    });
}
