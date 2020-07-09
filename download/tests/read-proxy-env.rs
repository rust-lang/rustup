#![cfg(feature = "reqwest-backend")]

use std::env::{remove_var, set_var};

use env_proxy::for_url;
use url::Url;

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
    scrub_env();
    set_var("https_proxy", "http://proxy.example.com:8080");
    let u = Url::parse("https://www.example.org").ok().unwrap();
    assert_eq!(
        for_url(&u).host_port(),
        Some(("proxy.example.com".to_string(), 8080))
    );
}
