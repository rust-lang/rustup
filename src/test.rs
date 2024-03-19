#![allow(clippy::box_default)]
//! Test support module; public to permit use from integration tests.

pub mod mock;

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
use anyhow::Result;

pub use crate::cli::self_update::test::{get_path, with_saved_path};
use crate::currentprocess;
use crate::dist::dist::TargetTriple;

// Things that can have environment variables applied to them.
pub trait Env {
    fn env<K, V>(&mut self, key: K, val: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>;
}

impl Env for Command {
    fn env<K, V>(&mut self, key: K, val: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.env(key, val);
    }
}

impl Env for HashMap<String, String> {
    fn env<K, V>(&mut self, key: K, val: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let key = key.as_ref().to_os_string().into_string().unwrap();
        let val = val.as_ref().to_os_string().into_string().unwrap();
        self.insert(key, val);
    }
}

/// The path to a dir for this test binaries state
fn exe_test_dir() -> io::Result<PathBuf> {
    let current_exe_path = env::current_exe().unwrap();
    let mut exe_dir = current_exe_path.parent().unwrap();
    if exe_dir.ends_with("deps") {
        exe_dir = exe_dir.parent().unwrap();
    }
    Ok(exe_dir.parent().unwrap().to_owned())
}

/// Returns a tempdir for running tests in
pub fn test_dir() -> io::Result<tempfile::TempDir> {
    let exe_dir = exe_test_dir()?;
    let test_dir = exe_dir.join("tests");
    fs::create_dir_all(&test_dir).unwrap();
    tempfile::Builder::new()
        .prefix("running-test-")
        .tempdir_in(test_dir)
}

/// Returns a directory for storing immutable distributions in
pub fn const_dist_dir() -> io::Result<tempfile::TempDir> {
    // TODO: do something smart, like managing garbage collection or something.
    let exe_dir = exe_test_dir()?;
    let dists_dir = exe_dir.join("dists");
    fs::create_dir_all(&dists_dir)?;
    let current_exe = env::current_exe().unwrap();
    let current_exe_name = current_exe.file_name().unwrap();
    tempfile::Builder::new()
        .prefix(&format!(
            "dist-for-{}-",
            Path::new(current_exe_name).display()
        ))
        .tempdir_in(dists_dir)
}

/// Returns a tempdir for storing test-scoped distributions in
pub fn test_dist_dir() -> io::Result<tempfile::TempDir> {
    let exe_dir = exe_test_dir()?;
    let test_dir = exe_dir.join("tests");
    fs::create_dir_all(&test_dir).unwrap();
    tempfile::Builder::new()
        .prefix("test-dist-dir-")
        .tempdir_in(test_dir)
}

/// Makes persistent unique directory inside path.
///
/// Should only be used with path=a tempdir that will be cleaned up, as the
/// directory tempdir_in_with_prefix creates won't be automatically cleaned up.
fn tempdir_in_with_prefix<P: AsRef<Path>>(path: P, prefix: &str) -> io::Result<PathBuf> {
    Ok(tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(path.as_ref())?
        .into_path())
}

/// What is this host's triple - seems very redundant with from_host_or_build()
/// ... perhaps this is so that the test data we have is only exercised on known
/// triples?
///
/// NOTE: This *cannot* be called within a currentprocess context as it creates
/// its own context on Windows hosts. This is partly by chance but also partly
/// deliberate: If you need the host triple, or to call for_host(), you can do
/// so outside of calls to run() or unit test code that runs in a currentprocess
/// context.
///
/// IF it becomes very hard to workaround that, then we can either make a second
/// this_host_triple that doesn't make its own currentprocess or use
/// TargetTriple::from_host() from within the currentprocess context as needed.
pub fn this_host_triple() -> String {
    if cfg!(target_os = "windows") {
        // For windows, this host may be different to the target: we may be
        // building with i686 toolchain, but on an x86_64 host, so run the
        // actual detection logic and trust it.
        let tp = currentprocess::TestProcess::default();
        return currentprocess::with(tp.into(), || TargetTriple::from_host().unwrap().to_string());
    }
    let arch = if cfg!(target_arch = "x86") {
        "i686"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "riscv64") {
        "riscv64gc"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "loongarch64") {
        "loongarch64"
    } else {
        unimplemented!()
    };
    let os = if cfg!(target_os = "linux") {
        "unknown-linux"
    } else if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "illumos") {
        "unknown-illumos"
    } else if cfg!(target_os = "freebsd") {
        "unknown-freebsd"
    } else {
        unimplemented!()
    };
    let env = if cfg!(target_env = "gnu") {
        Some("gnu")
    } else {
        None
    };

    if let Some(env) = env {
        format!("{arch}-{os}-{env}")
    } else {
        format!("{arch}-{os}")
    }
}

// Format a string with this host triple.
#[macro_export]
macro_rules! for_host {
    ($s: expr) => {
        &format!($s, $crate::test::this_host_triple())
    };
}

#[derive(Clone, Debug)]
/// The smallest form of test isolation: an isolated RUSTUP_HOME, for codepaths
/// that read and write config files but do not invoke processes, download data
/// etc.
pub struct RustupHome {
    pub rustupdir: PathBuf,
}

impl RustupHome {
    pub fn apply<E: Env>(&self, e: &mut E) {
        e.env("RUSTUP_HOME", self.rustupdir.to_string_lossy().to_string())
    }

    pub fn has<P: AsRef<Path>>(&self, path: P) -> bool {
        self.rustupdir.join(path).exists()
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.rustupdir.join(path)
    }

    pub fn new_in<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let rustupdir = tempdir_in_with_prefix(path, "rustup")?;
        Ok(RustupHome { rustupdir })
    }

    pub fn remove(&self) -> io::Result<()> {
        remove_dir_all::remove_dir_all(&self.rustupdir)
    }
}

impl fmt::Display for RustupHome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.rustupdir.display())
    }
}

/// Create an isolated rustup home with no content, then call f with it, and
/// delete it afterwards.
#[cfg(test)]
pub(crate) fn with_rustup_home<F>(f: F) -> Result<()>
where
    F: FnOnce(&RustupHome) -> Result<()>,
{
    let test_dir = test_dir()?;
    let rustup_home = RustupHome::new_in(test_dir)?;
    f(&rustup_home)
}

#[cfg(feature = "otel")]
use once_cell::sync::Lazy;

/// A tokio runtime for the sync tests, permitting the use of tracing. This is
/// never shutdown, instead it is just dropped at end of process.
#[cfg(feature = "otel")]
static TRACE_RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Runtime::new().unwrap());
/// A tracer for the tests.
#[cfg(feature = "otel")]
static TRACER: Lazy<opentelemetry_sdk::trace::Tracer> = Lazy::new(|| {
    use std::time::Duration;

    use opentelemetry::{global, KeyValue};
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{
        propagation::TraceContextPropagator,
        trace::{self, Sampler},
        Resource,
    };
    use tokio::runtime::Handle;
    use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

    // Use the current runtime, or the sync test runtime otherwise.
    let handle = match Handle::try_current() {
        Ok(handle) => handle,
        Err(_) => TRACE_RUNTIME.handle().clone(),
    };
    let _guard = handle.enter();

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_timeout(Duration::from_secs(3)),
        )
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_resource(Resource::new(vec![KeyValue::new("service.name", "rustup")])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .unwrap();

    global::set_text_map_propagator(TraceContextPropagator::new());
    let env_filter = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("INFO"));
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer.clone());
    let subscriber = Registry::default().with(env_filter).with(telemetry);
    tracing::subscriber::set_global_default(subscriber).unwrap();
    tracer
});

pub fn before_test() {
    #[cfg(feature = "otel")]
    {
        Lazy::force(&TRACER);
    }
}

pub async fn before_test_async() {
    #[cfg(feature = "otel")]
    {
        Lazy::force(&TRACER);
    }
}

pub fn after_test() {
    #[cfg(feature = "otel")]
    {
        let handle = TRACE_RUNTIME.handle();
        let _guard = handle.enter();
        TRACER.provider().map(|p| p.force_flush());
    }
}

pub async fn after_test_async() {
    #[cfg(feature = "otel")]
    {
        TRACER.provider().map(|p| p.force_flush());
    }
}
