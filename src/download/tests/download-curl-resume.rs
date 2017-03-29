#![cfg(feature = "curl-backend")]

extern crate download;
extern crate tempdir;
extern crate url;

use std::sync::{Arc, Mutex};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use tempdir::TempDir;
use url::Url;

use download::*;

fn tmp_dir() -> TempDir {
    TempDir::new("rustup-download-test-").expect("creating tempdir for test")
}

fn file_contents(path: &Path) -> String {
    let mut result = String::new();
    File::open(&path).unwrap().read_to_string(&mut result).expect("reading test result file");
    result
}


pub fn write_file(path: &Path, contents: &str) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .expect("writing test data");

    io::Write::write_all(&mut file, contents.as_bytes()).expect("writing test data");

    file.sync_data().expect("writing test data");
}

#[test]
fn partially_downloaded_file_gets_resumed_from_byte_offset() {
    let tmpdir = tmp_dir();
    let from_path = tmpdir.path().join("download-source");
    write_file(&from_path, "xxx45");

    let target_path = tmpdir.path().join("downloaded");
    write_file(&target_path, "123");

    let from_url = Url::from_file_path(&from_path).unwrap();
    download_to_path_with_backend(
            Backend::Curl,
            &from_url,
            &target_path,
            true,
            None)
            .expect("Test download failed");

    assert_eq!(file_contents(&target_path), "12345");
}

#[test]
fn callback_gets_all_data_as_if_the_download_happened_all_at_once() {
    let tmpdir = tmp_dir();

    let from_path = tmpdir.path().join("download-source");
    write_file(&from_path, "xxx45");

    let target_path = tmpdir.path().join("downloaded");
    write_file(&target_path, "123");

    let from_url = Url::from_file_path(&from_path).unwrap();

    let received_in_callback = Arc::new(Mutex::new(Vec::new()));

    download_to_path_with_backend(Backend::Curl,
                                  &from_url,
                                  &target_path,
                                  true,
                                  Some(&|msg| {
        match msg {
            Event::DownloadDataReceived(data) => {
                for b in data.iter() {
                    received_in_callback.lock().unwrap().push(b.clone());
                }
            }
            _ => {}
        }


        Ok(())
    }))
            .expect("Test download failed");

    let ref observed_bytes = *received_in_callback.lock().unwrap();
    assert_eq!(observed_bytes, &vec![b'1', b'2', b'3', b'4', b'5']);
    assert_eq!(file_contents(&target_path), "12345");
}
