#![cfg(feature = "reqwest-backend")]

use download::*;

mod support;
use crate::support::{file_contents, serve_file, tmp_dir};

// There are two separate files because this crate caches reqwest clients
// and all tests in one file use either the safe or the unsafe client.
// See download-reqwest-unsafe.rs for the complementary test case.

#[test]
fn downloading_with_no_certificate() {
    let tmpdir = tmp_dir();
    let target_path = tmpdir.path().join("downloaded");

    let addr = serve_file(b"12345".to_vec(), false);
    let from_url = format!("http://{}", addr).parse().unwrap();

    download_to_path_with_backend(Backend::Reqwest, &from_url, &target_path, false, None)
        .expect("Test download failed");

    assert_eq!(file_contents(&target_path), "12345");
}

#[test]
#[should_panic]
fn downloading_with_bad_certificate() {
    let tmpdir = tmp_dir();
    let target_path = tmpdir.path().join("downloaded");

    let addr = serve_file(b"12345".to_vec(), true);
    let from_url = format!("https://{}", addr).parse().unwrap();

    std::env::remove_var("RUSTUP_USE_UNSAFE_SSL");

    assert_eq!(std::env::var_os("RUSTUP_USE_UNSAFE_SSL").is_none(), true);

    download_to_path_with_backend(Backend::Reqwest, &from_url, &target_path, false, None)
        .expect("Test download failed");

    assert_eq!(file_contents(&target_path), "12345");
}

#[test]
#[should_panic]
fn downloading_with_bad_certificate_using_wrong_env_value() {
    let tmpdir = tmp_dir();
    let target_path = tmpdir.path().join("downloaded");

    let addr = serve_file(b"12345".to_vec(), true);
    let from_url = format!("https://{}", addr).parse().unwrap();

    std::env::set_var("RUSTUP_USE_UNSAFE_SSL", "FOOBAR");

    assert_eq!(std::env::var_os("RUSTUP_USE_UNSAFE_SSL").is_some(), true);

    download_to_path_with_backend(Backend::Reqwest, &from_url, &target_path, false, None)
        .expect("Test download failed");

    assert_eq!(file_contents(&target_path), "12345");
}
