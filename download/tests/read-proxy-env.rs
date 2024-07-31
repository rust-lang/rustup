#![cfg(feature = "reqwest-backend")]

use std::env::{remove_var, set_var};
use std::error::Error;
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use std::thread;
use std::time::Duration;

use env_proxy::for_url;
use reqwest::{Client, Proxy};
use tokio::sync::Mutex;
use url::Url;

static SERIALISE_TESTS: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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
#[tokio::test]
async fn read_basic_proxy_params() {
    let _guard = SERIALISE_TESTS.lock().await;
    scrub_env();
    set_var("https_proxy", "http://proxy.example.com:8080");
    let u = Url::parse("https://www.example.org").ok().unwrap();
    assert_eq!(
        for_url(&u).host_port(),
        Some(("proxy.example.com".to_string(), 8080))
    );
}

// Tests to verify if socks feature is available and being used
#[tokio::test]
async fn socks_proxy_request() {
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
    let _guard = SERIALISE_TESTS.lock().await;

    scrub_env();
    set_var("all_proxy", "socks5://127.0.0.1:1080");

    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:1080").unwrap();
        let incoming = listener.incoming();
        for _ in incoming {
            CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    });

    let env_proxy = |url: &Url| for_url(url).to_url();
    let url = Url::parse("http://192.168.0.1/").unwrap();

    let client = Client::builder()
        // HACK: set `pool_max_idle_per_host` to `0` to avoid an issue in the underlying
        // `hyper` library that causes the `reqwest` client to hang in some cases.
        // See <https://github.com/hyperium/hyper/issues/2312> for more details.
        .pool_max_idle_per_host(0)
        .proxy(Proxy::custom(env_proxy))
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let res = client.get(url.as_str()).send().await;

    if let Err(e) = res {
        let s = e.source().unwrap();
        assert!(
            s.to_string().contains("client error (Connect)"),
            "Expected socks connect error, got: {s}",
        );
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
    } else {
        panic!("Socks proxy was ignored")
    }
}
