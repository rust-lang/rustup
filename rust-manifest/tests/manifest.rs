extern crate rust_manifest;

use rust_manifest::Manifest;

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

