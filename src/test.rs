#![allow(
    clippy::box_default,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::dbg_macro
)]
//! Test support module; public to permit use from integration tests.

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::dist::TargetTriple;
use crate::process::TestProcess;

#[cfg(windows)]
pub use crate::cli::self_update::{RegistryGuard, RegistryValueId, USER_PATH, get_path};

mod clitools;
pub use clitools::{
    CliTestContext, Config, SanitizedOutput, Scenario, SelfUpdateTestContext, output_release_file,
    print_command, print_indented,
};
pub(crate) mod dist;
pub(crate) mod mock;
pub use mock::{MockComponentBuilder, MockFile, MockInstallerBuilder};

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
/// NOTE: This *cannot* be called within a process context as it creates
/// its own context on Windows hosts. This is partly by chance but also partly
/// deliberate: If you need the host triple, or to call for_host(), you can do
/// so outside of calls to run() or unit test code that runs in a process
/// context.
///
/// IF it becomes very hard to workaround that, then we can either make a second
/// this_host_triple that doesn't make its own process or use
/// TargetTriple::from_host() from within the process context as needed.
pub fn this_host_triple() -> String {
    if cfg!(target_os = "windows") {
        // For windows, this host may be different to the target: we may be
        // building with i686 toolchain, but on an x86_64 host, so run the
        // actual detection logic and trust it.
        let tp = TestProcess::default();
        return TargetTriple::from_host(&tp.process).unwrap().to_string();
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
    } else if cfg!(target_arch = "powerpc64") && cfg!(target_endian = "little") {
        "powerpc64le"
    } else if cfg!(target_arch = "s390x") {
        "s390x"
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
    ($s:tt $($arg:tt)*) => {
        &format!($s, $crate::test::this_host_triple() $($arg)*)
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

pub mod topical_doc_data {
    use std::collections::HashSet;
    use std::path::PathBuf;

    // Paths are written as a string in the UNIX format to make it easy
    // to maintain.
    static TEST_CASES: &[(&[&str], &str)] = &[
        (&["core"], "core/index.html"),
        (&["core::arch"], "core/arch/index.html"),
        (&["fn"], "std/keyword.fn.html"),
        (&["keyword:fn"], "std/keyword.fn.html"),
        (&["primitive:fn"], "std/primitive.fn.html"),
        (&["macro:file!"], "std/macro.file!.html"),
        (&["macro:file"], "std/macro.file.html"),
        (&["std::fs"], "std/fs/index.html"),
        (&["std::fs::read_dir"], "std/fs/fn.read_dir.html"),
        (&["std::io::Bytes"], "std/io/struct.Bytes.html"),
        (&["std::iter::Sum"], "std/iter/trait.Sum.html"),
        (&["std::io::error::Result"], "std/io/error/type.Result.html"),
        (&["usize"], "std/primitive.usize.html"),
        (&["eprintln"], "std/macro.eprintln.html"),
        (&["alloc::format"], "alloc/macro.format.html"),
        (&["std::mem::MaybeUninit"], "std/mem/union.MaybeUninit.html"),
        (&["--rustc", "lints"], "rustc/lints/index.html"),
        (&["--rustdoc", "lints"], "rustdoc/lints.html"),
        (
            &["lints::broken_intra_doc_links", "--rustdoc"],
            "rustdoc/lints.html",
        ),
    ];

    fn repath(origin: &str) -> String {
        // Add doc prefix and rewrite string paths for the current platform
        let with_prefix = "share/doc/rust/html/".to_owned() + origin;
        let splitted = with_prefix.split('/');
        let repathed = splitted.fold(PathBuf::new(), |acc, e| acc.join(e));
        repathed.into_os_string().into_string().unwrap()
    }

    pub fn test_cases<'a>() -> impl Iterator<Item = (&'a [&'a str], String)> {
        TEST_CASES.iter().map(|(args, path)| (*args, repath(path)))
    }

    pub fn unique_paths() -> impl Iterator<Item = String> {
        // Hashset used to test uniqueness of values through insert method.
        let mut unique_paths = HashSet::new();
        TEST_CASES
            .iter()
            .filter(move |(_, p)| unique_paths.insert(p))
            .map(|(_, p)| repath(p))
    }
}

pub fn calc_hash(src: &Path) -> String {
    let mut buf = Vec::new();
    File::open(src).unwrap().read_to_end(&mut buf).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(buf);
    format!("{:x}", hasher.finalize())
}

pub fn create_hash(src: &Path, dst: &Path) -> String {
    let hex = calc_hash(src);
    let src_file = src.file_name().unwrap();
    let file_contents = format!("{} *{}\n", hex, src_file.to_string_lossy());
    dist::write_file(dst, &file_contents);
    hex
}

pub static CROSS_ARCH1: &str = "x86_64-unknown-linux-musl";
pub static CROSS_ARCH2: &str = "arm-linux-androideabi";

// Architecture for testing 'multi-host' installation.
#[cfg(target_pointer_width = "64")]
pub static MULTI_ARCH1: &str = "i686-unknown-linux-gnu";
#[cfg(not(target_pointer_width = "64"))]
pub static MULTI_ARCH1: &str = "x86_64-unknown-linux-gnu";
