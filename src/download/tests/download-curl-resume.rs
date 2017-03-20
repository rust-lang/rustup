#![cfg(feature = "curl-backend")]

extern crate download;
extern crate rustup_mock;
extern crate tempdir;

use std::fs::{self, File};
use std::io::{Read};
use std::path::Path;

use tempdir::TempDir;

use rustup_mock::http_server;

use download::*;

fn setup(test: &Fn(TempDir, http_server::Server) -> ()) {
    let tmp = TempDir::new("rustup-download-test-").expect("creating tempdir for test");
    let served_dir = &tmp.path().join("test-files");
    fs::DirBuilder::new().create(served_dir).expect("setting up a folder to server files from");
    let server = http_server::Server::serve_from(served_dir).expect("setting up http server for test");
    test(tmp, server);
}

fn file_contents(path: &Path) -> String {
    let mut result = String::new();
    File::open(&path).unwrap().read_to_string(&mut result).expect("reading test result file");
    result
}

#[test]
fn when_download_is_interrupted_partial_file_is_left_on_disk() {
    setup(&|tmpdir, mut server| {
        let target_path = tmpdir.path().join("downloaded");

        server.put_file_from_bytes("test-file", b"12345");

        server.stop_after_bytes(3);
        download_to_path_with_backend(
            Backend::Curl, &server.address().join("test-file").unwrap(), &target_path, true, None)
            .expect("Test download failed");

        assert_eq!(file_contents(&target_path), "123");
    });
}

#[test]
fn download_interrupted_and_resumed() {
    setup(&|tmpdir, mut server| {
        let target_path = tmpdir.path().join("downloaded");

        server.put_file_from_bytes("test-file", b"12345");

        server.stop_after_bytes(3);
        download_to_path_with_backend(
            Backend::Curl, &server.address().join("test-file").unwrap(), &target_path, true, None)
            .expect("Test download failed");

        server.stop_after_bytes(2);
        download_to_path_with_backend(
            Backend::Curl, &server.address().join("test-file").unwrap(), &target_path, true, None)
            .expect("Test download failed");

        assert_eq!(file_contents(&target_path), "12345");
    });
}

#[test]
fn resuming_download_with_callback_that_needs_to_read_contents() {
    setup(&|tmpdir, mut server| {
        let target_path = tmpdir.path().join("downloaded");

        server.put_file_from_bytes("test-file", b"12345");

        server.stop_after_bytes(3);
        download_to_path_with_backend(
            Backend::Curl, &server.address().join("test-file").unwrap(), &target_path, true, Some(&|_| {Ok(())}))
            .expect("Test download failed");

        server.stop_after_bytes(2);
        download_to_path_with_backend(
            Backend::Curl, &server.address().join("test-file").unwrap(), &target_path, true, Some(&|_| {Ok(())}))
            .expect("Test download failed");

        assert_eq!(file_contents(&target_path), "12345");
    });
}
