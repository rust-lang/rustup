extern crate multirust_dist;

use multirust_dist::manifest::Manifest;
use multirust_dist::Error;

// Example manifest from https://public.etherpad-mozilla.org/p/Rust-infra-work-week
static EXAMPLE: &'static str = include_str!("channel-rust-nightly-example.toml");

#[test]
fn parse_smoke_test() {
    let pkg = Manifest::parse(EXAMPLE).unwrap();

    pkg.get_package("rust").unwrap();
    pkg.get_package("rustc").unwrap();
    pkg.get_package("cargo").unwrap();
    pkg.get_package("rust-std").unwrap();
    pkg.get_package("rust-docs").unwrap();

    let rust_pkg = pkg.get_package("rust").unwrap();
    assert!(rust_pkg.version.contains("1.3.0"));

    let rust_target_pkg = rust_pkg.get_target("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(rust_target_pkg.available, true);
    assert_eq!(rust_target_pkg.url, "example.com");
    assert_eq!(rust_target_pkg.hash, "...");

    let ref component = rust_target_pkg.components[0];
    assert_eq!(component.pkg, "rustc");
    assert_eq!(component.target, "x86_64-unknown-linux-gnu");

    let ref component = rust_target_pkg.extensions[0];
    assert_eq!(component.pkg, "rust-std");
    assert_eq!(component.target, "x86_64-unknown-linux-musl");

    let docs_pkg = pkg.get_package("rust-docs").unwrap();
    let docs_target_pkg = docs_pkg.get_target("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(docs_target_pkg.url, "example.com");
}

#[test]
fn parse_round_trip() {
    let original = Manifest::parse(EXAMPLE).unwrap();
    let serialized = original.clone().stringify();
    let new = Manifest::parse(&serialized).unwrap();
    assert_eq!(original, new);
}

#[test]
fn validate_components_have_corresponding_packages() {
    let manifest = r#"
manifest-version = "2"
date = "2015-10-10"
[pkg.rust]
  version = "rustc 1.3.0 (9a92aaf19 2015-09-15)"
  [pkg.rust.target.x86_64-unknown-linux-gnu]
    available = true
    url = "example.com"
    hash = "..."
    [[pkg.rust.target.x86_64-unknown-linux-gnu.components]]
      pkg = "rustc"
      target = "x86_64-unknown-linux-gnu"
    [[pkg.rust.target.x86_64-unknown-linux-gnu.extensions]]
      pkg = "rust-std"
      target = "x86_64-unknown-linux-musl"
[pkg.rustc]
  version = "rustc 1.3.0 (9a92aaf19 2015-09-15)"
  [pkg.rustc.target.x86_64-unknown-linux-gnu]
    available = true
    url = "example.com"
    hash = "..."
"#;

    let err = Manifest::parse(manifest).unwrap_err();

    match err {
        Error::MissingPackageForComponent(_) => {},
        _ => panic!(),
    }
}
