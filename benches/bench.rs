#![recursion_limit = "256"]

use std::{ffi::OsString, fs, process::Stdio};

#[cfg(feature = "__bench_codspeed")]
use codspeed_criterion_compat::{Criterion, criterion_group, criterion_main};
#[cfg(not(feature = "__bench_codspeed"))]
use criterion::{Criterion, criterion_group, criterion_main};
use rustup::test::{CliTestContext, Scenario};
use tokio::runtime::Runtime;

/// Returns the name of the enclosing function.
///
/// Borrowed from <https://docs.rs/stdext/0.3.3/src/stdext/macros.rs.html#63-74>,
/// original code licensed under MIT.
macro_rules! fn_name {
    () => {{
        // Okay, this is ugly, I get it. However, this is the best we can get on a stable rust.
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // `3` is the length of the `::f`.
        &name[..name.len() - 3]
    }};
}

fn proxy_startup_plus_toolchain(c: &mut Criterion) {
    let mut r = Runtime::new().unwrap().block_on(RustcProxyRunner::new());
    r.args(&["+stable"]);
    c.bench_function(fn_name!(), move |b| b.iter(|| r.run()));
}

fn proxy_startup_env(c: &mut Criterion) {
    let mut r = Runtime::new().unwrap().block_on(RustcProxyRunner::new());
    r.envs(&[("RUSTUP_TOOLCHAIN", "stable")]);
    c.bench_function(fn_name!(), move |b| b.iter(|| r.run()));
}

fn proxy_startup_dir_override(c: &mut Criterion) {
    let r = Runtime::new().unwrap().block_on(async {
        let r = RustcProxyRunner::new().await;

        r.cx.config
            .expect(["rustup", "override", "set", "stable"])
            .await
            .is_ok();

        r
    });

    c.bench_function(fn_name!(), move |b| b.iter(|| r.run()));
}

fn proxy_startup_toml_override(c: &mut Criterion) {
    let r = Runtime::new().unwrap().block_on(RustcProxyRunner::new());

    let toml = r.cx.config.current_dir().join("rust-toolchain.toml");
    fs::write(&toml, "[toolchain]\nchannel = \"stable\"\n").unwrap();

    c.bench_function(fn_name!(), move |b| b.iter(|| r.run()));
}

fn proxy_startup_default(c: &mut Criterion) {
    let r = Runtime::new().unwrap().block_on(RustcProxyRunner::new());
    c.bench_function(fn_name!(), move |b| b.iter(|| r.run()));
}

struct RustcProxyRunner {
    cx: CliTestContext,
    args: Vec<OsString>,
    envs: Vec<(OsString, OsString)>,
}

impl RustcProxyRunner {
    /// Initializes a [`RustcProxyRunner`] with the stable toolchain set as the default.
    async fn new() -> Self {
        let cx = CliTestContext::new(Scenario::SimpleV2).await;
        cx.config
            .expect(["rustup", "default", "stable"])
            .await
            .is_ok();

        Self {
            cx,
            args: vec![],
            envs: vec![],
        }
    }

    fn args(&mut self, args: &[&str]) -> &mut Self {
        self.args.extend(args.iter().map(OsString::from));
        self
    }

    fn envs(&mut self, envs: &[(&str, &str)]) -> &mut Self {
        self.envs
            .extend(envs.iter().map(|(k, v)| (k.into(), v.into())));
        self
    }

    fn run(&self) {
        let status = self
            .cx
            .config
            .cmd("rustc", &self.args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("CAKE", "A lie!")
            .args(["--echo-env", "CAKE"])
            .status()
            .expect("failed to execute rustc proxy");

        assert!(status.success());
    }
}

criterion_group!(
    proxy_startup,
    proxy_startup_plus_toolchain,
    proxy_startup_env,
    proxy_startup_dir_override,
    proxy_startup_toml_override,
    proxy_startup_default,
);
criterion_main!(proxy_startup);
