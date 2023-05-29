#![cfg(feature = "curl-backend")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use url::Url;

use download::*;

mod support;
use crate::support::{serve_file, tmp_dir, write_file};

#[tokio::test]
async fn partially_downloaded_file_gets_resumed_from_byte_offset() {
    let tmpdir = tmp_dir();
    let from_path = tmpdir.path().join("download-source");
    write_file(&from_path, "xxx45");

    let target_path = tmpdir.path().join("downloaded");
    write_file(&target_path, "123");

    let from_url = Url::from_file_path(&from_path).unwrap();
    download_to_path_with_backend(Backend::Curl, &from_url, &target_path, true, None)
        .await
        .expect("Test download failed");

    assert_eq!(std::fs::read_to_string(&target_path).unwrap(), "12345");
}

#[tokio::test]
async fn callback_gets_all_data_as_if_the_download_happened_all_at_once() {
    let tmpdir = tmp_dir();
    let target_path = tmpdir.path().join("downloaded");
    write_file(&target_path, "123");

    let addr = serve_file(b"xxx45".to_vec());

    let from_url = format!("http://{addr}").parse().unwrap();

    let callback_partial = AtomicBool::new(false);
    let callback_len = Mutex::new(None);
    let received_in_callback = Mutex::new(Vec::new());

    download_to_path_with_backend(
        Backend::Curl,
        &from_url,
        &target_path,
        true,
        Some(&|msg| {
            match msg {
                Event::ResumingPartialDownload => {
                    assert!(!callback_partial.load(Ordering::SeqCst));
                    callback_partial.store(true, Ordering::SeqCst);
                }
                Event::DownloadContentLengthReceived(len) => {
                    let mut flag = callback_len.lock().unwrap();
                    assert!(flag.is_none());
                    *flag = Some(len);
                }
                Event::DownloadDataReceived(data) => {
                    for b in data.iter() {
                        received_in_callback.lock().unwrap().push(*b);
                    }
                }
            }

            Ok(())
        }),
    )
    .await
    .expect("Test download failed");

    assert!(callback_partial.into_inner());
    assert_eq!(*callback_len.lock().unwrap(), Some(5));
    let observed_bytes = received_in_callback.into_inner().unwrap();
    assert_eq!(observed_bytes, vec![b'1', b'2', b'3', b'4', b'5']);
    assert_eq!(std::fs::read_to_string(&target_path).unwrap(), "12345");
}
