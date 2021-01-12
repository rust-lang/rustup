#![cfg(feature = "reqwest-backend")]

use std::env::{remove_var, set_var};
use std::error::Error;
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use env_proxy::for_url;
use lazy_static::lazy_static;
use reqwest::{blocking::Client, Proxy};
use url::Url;

lazy_static! {
    static ref SERIALISE_TESTS: Mutex<()> = Mutex::new(());
}

fn scrub_env() {
    remove_var("http_proxy");
    remove_var("https_proxy");
    remove_var("HTTPS_PROXY");
    remove_var("ftp_proxy");
    remove_var("FTP_PROXY");
    remove_var("all_proxy");
    remove_var("ALL_PROXY");
    remove_var("no_proxy");
    remove_var("NO_PROXY");
}

// Tests for correctly retrieving the proxy (host, port) tuple from $https_proxy
#[test]
fn read_basic_proxy_params() {
    let _guard = SERIALISE_TESTS
        .lock()
        .expect("Unable to lock the test guard");
    scrub_env();
    set_var("https_proxy", "http://proxy.example.com:8080");
    let u = Url::parse("https://www.example.org").ok().unwrap();
    assert_eq!(
        for_url(&u).host_port(),
        Some(("proxy.example.com".to_string(), 8080))
    );
}

// Tests to verify if socks feature is available and being used
#[test]
fn socks_proxy_request() {
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
    let _guard = SERIALISE_TESTS
        .lock()
        .expect("Unable to lock the test guard");

    scrub_env();
    set_var("all_proxy", "socks5://127.0.0.1:1080");

    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:1080").unwrap();
        let incoming = listener.incoming();
        for _ in incoming {
            CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    });

    let env_proxy = |url: &Url| for_url(&url).to_url();
    let url = Url::parse("http://192.168.0.1/").unwrap();

    let client = Client::builder()
        .proxy(Proxy::custom(env_proxy))
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let res = client.get(url.as_str()).send();

    if let Err(e) = res {
        let s = e.source().unwrap();
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
        assert!(s.to_string().contains("socks connect error"));
    } else {
        panic!("Socks proxy was ignored")
    }
}
