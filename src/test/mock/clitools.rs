//! A mock distribution server used by tests/cli-v1.rs and
//! tests/cli-v2.rs
use std::{
    cell::RefCell,
    collections::HashMap,
    env::{self, consts::EXE_SUFFIX},
    ffi::OsStr,
    fmt::Debug,
    fs,
    io::{self, Write},
    mem,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, LazyLock, RwLock, RwLockWriteGuard},
    time::Instant,
};

use enum_map::{enum_map, Enum, EnumMap};
use tempfile::TempDir;
use url::Url;

use crate::cli::rustup_mode;
use crate::process;
use crate::test as rustup_test;
use crate::test::const_dist_dir;
use crate::test::this_host_triple;
use crate::utils::utils;

use super::{
    dist::{
        change_channel_date, MockChannel, MockComponent, MockDistServer, MockManifestVersion,
        MockPackage, MockTargetedPackage,
    },
    topical_doc_data, MockComponentBuilder, MockFile, MockInstallerBuilder,
};

/// The configuration used by the tests in this module
#[derive(Debug)]
pub struct Config {
    /// Where we put the rustup / rustc / cargo bins
    pub exedir: PathBuf,
    /// The tempfile for the mutable distribution server
    pub test_dist_dir: tempfile::TempDir,
    /// The mutable distribution server; None if none is set.
    pub distdir: Option<PathBuf>,
    /// The const distribution server; None if none is set
    const_dist_dir: Option<PathBuf>,
    /// RUSTUP_HOME
    pub rustupdir: rustup_test::RustupHome,
    /// Custom toolchains
    pub customdir: PathBuf,
    /// CARGO_HOME
    pub cargodir: PathBuf,
    /// ~
    pub homedir: PathBuf,
    /// Root for updates to rustup itself aka RUSTUP_UPDATE_ROOT
    pub rustup_update_root: Option<String>,
    /// This is cwd for the test
    pub workdir: RefCell<PathBuf>,
    /// This is the test root for keeping stuff together
    pub test_root_dir: PathBuf,
}

// Describes all the features of the mock dist server.
// Building the mock server is slow, so use simple scenario when possible.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Enum)]
pub enum Scenario {
    /// No mutable dist server at all
    None,
    /// No dist server content
    Empty,
    /// Two dates, two manifests
    Full,
    /// Two dates, v2 manifests
    /// See SimpleV2 for the 2015_01_02 date of ArchivesV2
    ArchivesV2,
    /// The 2015-01-01 date of ArchivesV2
    ArchivesV2_2015_01_01,
    /// Two versions of 'stable'
    ArchivesV2TwoVersions,
    /// Two dates, v1 manifests
    ArchivesV1,
    /// One date, v2 manifests
    SimpleV2,
    /// One date, v1 manifests
    SimpleV1,
    /// One date, v2 manifests, MULTI_ARCH1 host
    MultiHost,
    /// One date, v2 manifests, beta with tag
    BetaTag,
    /// Two dates, v2 manifests, everything unavailable in second date.
    Unavailable,
    /// Two dates, v2 manifests, RLS unavailable in first date, restored on second.
    UnavailableRls,
    /// Two dates, v2 manifests, RLS available in first stable, removed on second.
    RemovedRls,
    /// Three dates, v2 manifests, RLS available in first and second, not last
    MissingComponent,
    /// Three dates, v2 manifests, RLS available in first, middle missing nightly
    MissingNightly,
    /// 1 date, v2 manifests, host and MULTI_ARCH1 in first
    HostGoesMissingBefore,
    /// 1 later date, v2 manifests, MULTI_ARCH1 only
    HostGoesMissingAfter,
    /// Three dates, v2 manifests, host and MULTI_ARCH1 in first, host only in second,
    /// host and MULTI_ARCH1 but no RLS in last
    MissingComponentMulti,
}

pub static CROSS_ARCH1: &str = "x86_64-unknown-linux-musl";
pub static CROSS_ARCH2: &str = "arm-linux-androideabi";

// Architecture for testing 'multi-host' installation.
#[cfg(target_pointer_width = "64")]
pub static MULTI_ARCH1: &str = "i686-unknown-linux-gnu";
#[cfg(not(target_pointer_width = "64"))]
pub static MULTI_ARCH1: &str = "x86_64-unknown-linux-gnu";

static CONST_TEST_STATE: LazyLock<ConstState> =
    LazyLock::new(|| ConstState::new(const_dist_dir().unwrap()));

/// Const test state - test dirs that can be reused across tests.
struct ConstState {
    scenarios: EnumMap<Scenario, RwLock<Option<PathBuf>>>,
    const_dist_dir: tempfile::TempDir,
}

/// The lock to be used when creating test environments.
///
/// Essentially we use this in `.read()` mode to gate access to `fork()`
/// new subprocesses, and in `.write()` mode to gate creation of new test
/// environments. In doing this we can ensure that new test environment creation
/// does not result in ETXTBSY because the FDs in question happen to be in
/// newly `fork()`d but not yet `exec()`d subprocesses of other tests.
pub static CMD_LOCK: LazyLock<RwLock<usize>> = LazyLock::new(|| RwLock::new(0));

impl ConstState {
    fn new(const_dist_dir: tempfile::TempDir) -> Self {
        Self {
            const_dist_dir,
            scenarios: enum_map! {
                Scenario::ArchivesV1 => RwLock::new(None),
                Scenario::ArchivesV2 => RwLock::new(None),
                Scenario::ArchivesV2_2015_01_01 => RwLock::new(None),
                Scenario::ArchivesV2TwoVersions => RwLock::new(None),
                Scenario::BetaTag => RwLock::new(None),
                Scenario::Empty => RwLock::new(None),
                Scenario::Full => RwLock::new(None),
                Scenario::HostGoesMissingBefore => RwLock::new(None),
                Scenario::HostGoesMissingAfter => RwLock::new(None),
                Scenario::MissingComponent => RwLock::new(None),
                Scenario::MissingComponentMulti => RwLock::new(None),
                Scenario::MissingNightly => RwLock::new(None),
                Scenario::MultiHost => RwLock::new(None),
                Scenario::None => RwLock::new(None),
                Scenario::RemovedRls => RwLock::new(None),
                Scenario::SimpleV1 => RwLock::new(None),
                Scenario::SimpleV2 => RwLock::new(None),
                Scenario::Unavailable => RwLock::new(None),
                Scenario::UnavailableRls => RwLock::new(None),
            },
        }
    }

    /// Get a dist server for a scenario
    fn dist_server_for(&self, s: Scenario) -> io::Result<PathBuf> {
        {
            // fast path: the dist already exists
            let lock = self.scenarios[s].read().unwrap();
            if let Some(ref path) = *lock {
                return Ok(path.clone());
            }
        }
        {
            let mut lock = self.scenarios[s].write().unwrap();
            // another writer may have initialized it
            match *lock {
                Some(ref path) => Ok(path.clone()),

                None => {
                    let dist_path = self.const_dist_dir.path().join(format!("{s:?}"));
                    create_mock_dist_server(&dist_path, s);
                    *lock = Some(dist_path.clone());
                    Ok(dist_path)
                }
            }
        }
    }
}

/// State a test can interact and mutate
pub async fn setup_test_state(test_dist_dir: tempfile::TempDir) -> (tempfile::TempDir, Config) {
    // Unset env variables that will break our testing
    env::remove_var("RUSTUP_UPDATE_ROOT");
    env::remove_var("RUSTUP_TOOLCHAIN");
    env::remove_var("SHELL");
    env::remove_var("ZDOTDIR");
    // clap does its own terminal colour probing, and that isn't
    // trait-controllable, but it does honour the terminal. To avoid testing
    // claps code, lie about whatever terminal this process was started under.
    env::set_var("TERM", "dumb");

    match env::var("RUSTUP_BACKTRACE") {
        Ok(val) => env::set_var("RUST_BACKTRACE", val),
        _ => env::remove_var("RUST_BACKTRACE"),
    }

    let current_exe_path = env::current_exe().unwrap();
    let mut built_exe_dir = current_exe_path.parent().unwrap();
    if built_exe_dir.ends_with("deps") {
        built_exe_dir = built_exe_dir.parent().unwrap();
    }
    let test_dir = rustup_test::test_dir().unwrap();

    fn tempdir_in_with_prefix<P: AsRef<Path>>(path: P, prefix: &str) -> PathBuf {
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(path.as_ref())
            .unwrap()
            .into_path()
    }

    let exedir = tempdir_in_with_prefix(&test_dir, "rustup-exe");
    let customdir = tempdir_in_with_prefix(&test_dir, "rustup-custom");
    let cargodir = tempdir_in_with_prefix(&test_dir, "rustup-cargo");
    let homedir = tempdir_in_with_prefix(&test_dir, "rustup-home");
    let workdir = tempdir_in_with_prefix(&test_dir, "rustup-workdir");

    // The uninstall process on windows involves using the directory above
    // CARGO_HOME, so make sure it's a subdir of our tempdir
    let cargodir = cargodir.join("ch");
    fs::create_dir(&cargodir).unwrap();

    let config = Config {
        exedir,
        distdir: None,
        const_dist_dir: None,
        test_dist_dir,
        rustupdir: rustup_test::RustupHome::new_in(&test_dir).unwrap(),
        customdir,
        cargodir,
        homedir,
        rustup_update_root: None,
        workdir: RefCell::new(workdir),
        test_root_dir: test_dir.path().to_path_buf(),
    };

    let build_path = built_exe_dir.join(format!("rustup-init{EXE_SUFFIX}"));

    let rustup_path = config.exedir.join(format!("rustup{EXE_SUFFIX}"));
    // Used to create dist servers. Perhaps should only link when needed?
    let init_path = config.exedir.join(format!("rustup-init{EXE_SUFFIX}"));
    let rustc_path = config.exedir.join(format!("rustc{EXE_SUFFIX}"));
    let cargo_path = config.exedir.join(format!("cargo{EXE_SUFFIX}"));
    let rls_path = config.exedir.join(format!("rls{EXE_SUFFIX}"));
    let rust_lldb_path = config.exedir.join(format!("rust-lldb{EXE_SUFFIX}"));

    const ESTIMATED_LINKS_PER_TEST: usize = 6 * 2;
    // NTFS has a limit of 1023 links per file; test setup creates 6 links, and
    // then some tests will link the cached installer to rustup/cargo etc,
    // adding more links
    const MAX_TESTS_PER_RUSTUP_EXE: usize = 1023 / ESTIMATED_LINKS_PER_TEST;
    // This returning-result inner structure allows failures without poisoning
    // cmd_lock.
    {
        fn link_or_copy(
            original: &Path,
            link: &Path,
            lock: &mut RwLockWriteGuard<'_, usize>,
        ) -> io::Result<()> {
            **lock += 1;
            if **lock < MAX_TESTS_PER_RUSTUP_EXE {
                hard_link(original, link)
            } else {
                // break the *original* so new tests form a new distinct set of
                // links. Do this by copying to link, breaking the source,
                // linking back.
                **lock = 0;
                fs::copy(original, link)?;
                fs::remove_file(original)?;
                hard_link(link, original)
            }
        }
        let mut lock = CMD_LOCK.write().unwrap();
        link_or_copy(&build_path, &rustup_path, &mut lock)
    }
    .unwrap();
    hard_link(&rustup_path, init_path).unwrap();
    hard_link(&rustup_path, rustc_path).unwrap();
    hard_link(&rustup_path, cargo_path).unwrap();
    hard_link(&rustup_path, rls_path).unwrap();
    hard_link(&rustup_path, rust_lldb_path).unwrap();

    // Make sure the host triple matches the build triple. Otherwise testing a 32-bit build of
    // rustup on a 64-bit machine will fail, because the tests do not have the host detection
    // functionality built in.
    config
        .run("rustup", ["set", "default-host", &this_host_triple()], &[])
        .await;

    // Set the auto update mode to disable, as most tests do not want to update rustup itself during the test.
    config
        .run("rustup", ["set", "auto-self-update", "disable"], &[])
        .await;

    // Create some custom toolchains
    create_custom_toolchains(&config.customdir);

    (test_dir, config)
}

pub struct SelfUpdateTestContext {
    pub config: Config,
    _test_dir: TempDir,
    self_dist_tmp: TempDir,
}

impl SelfUpdateTestContext {
    pub async fn new(version: &str) -> Self {
        let mut cx = CliTestContext::new(Scenario::SimpleV2).await;

        // Create a mock self-update server
        let self_dist_tmp = tempfile::Builder::new()
            .prefix("self_dist")
            .tempdir_in(&cx.config.test_root_dir)
            .unwrap();
        let self_dist = self_dist_tmp.path();

        let root_url = create_local_update_server(self_dist, &cx.config.exedir, version);
        cx.config.rustup_update_root = Some(root_url);

        let trip = this_host_triple();
        let dist_dir = self_dist.join(format!("archive/{version}/{trip}"));
        let dist_exe = dist_dir.join(format!("rustup-init{EXE_SUFFIX}"));
        let dist_tmp = dist_dir.join("rustup-init-tmp");

        // Modify the exe so it hashes different
        // 1) move out of the way the file
        fs::rename(&dist_exe, &dist_tmp).unwrap();
        // 2) copy it
        fs::copy(dist_tmp, &dist_exe).unwrap();
        // modify it
        let mut dest_file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(dist_exe)
            .unwrap();
        writeln!(dest_file).unwrap();

        Self {
            config: cx.config,
            _test_dir: cx._test_dir,
            self_dist_tmp,
        }
    }

    pub fn path(&self) -> &Path {
        self.self_dist_tmp.path()
    }
}

pub struct CliTestContext {
    pub config: Config,
    _test_dir: TempDir,
}

impl CliTestContext {
    pub async fn new(scenario: Scenario) -> Self {
        // Things we might cache or what not

        // Mutable dist server - working toward elimination
        let test_dist_dir = crate::test::test_dist_dir().unwrap();
        create_mock_dist_server(test_dist_dir.path(), scenario);

        // Things that are just about the test itself
        let (_test_dir, mut config) = setup_test_state(test_dist_dir).await;
        // Pulled out of setup_test_state for clarity: the long term intent is to
        // not have this at all.
        if scenario != Scenario::None {
            config.distdir = Some(config.test_dist_dir.path().to_path_buf());
        }

        Self { config, _test_dir }
    }

    /// Move the dist server to the specified scenario and restore it
    /// afterwards.
    pub fn with_dist_dir(&mut self, scenario: Scenario) -> DistDirGuard<'_> {
        self.config.distdir = Some(CONST_TEST_STATE.dist_server_for(scenario).unwrap());
        DistDirGuard { inner: self }
    }

    pub fn with_update_server(&mut self, version: &str) -> UpdateServerGuard {
        let self_dist_tmp = tempfile::Builder::new()
            .prefix("self_dist")
            .tempdir()
            .unwrap();
        let self_dist = self_dist_tmp.path();

        let root_url = create_local_update_server(self_dist, &self.config.exedir, version);
        let trip = this_host_triple();
        let dist_dir = self_dist.join(format!("archive/{version}/{trip}"));
        let dist_exe = dist_dir.join(format!("rustup-init{EXE_SUFFIX}"));
        let dist_tmp = dist_dir.join("rustup-init-tmp");

        // Modify the exe so it hashes different
        // 1) move out of the way the file
        fs::rename(&dist_exe, &dist_tmp).unwrap();
        // 2) copy it
        fs::copy(dist_tmp, &dist_exe).unwrap();
        // modify it
        let mut dest_file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(dist_exe)
            .unwrap();
        writeln!(dest_file).unwrap();

        self.config.rustup_update_root = Some(root_url);
        UpdateServerGuard {
            _self_dist: self_dist_tmp,
        }
    }

    pub fn change_dir(&mut self, path: &Path) -> WorkDirGuard<'_> {
        let prev = self.config.workdir.replace(path.to_owned());
        WorkDirGuard { inner: self, prev }
    }
}

#[must_use]
pub struct UpdateServerGuard {
    _self_dist: TempDir,
}

#[must_use]
pub struct WorkDirGuard<'a> {
    inner: &'a mut CliTestContext,
    prev: PathBuf,
}

impl Deref for WorkDirGuard<'_> {
    type Target = CliTestContext;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl DerefMut for WorkDirGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

impl Drop for WorkDirGuard<'_> {
    fn drop(&mut self) {
        self.inner.config.workdir.replace(mem::take(&mut self.prev));
    }
}

#[must_use]
pub struct DistDirGuard<'a> {
    inner: &'a mut CliTestContext,
}

impl Deref for DistDirGuard<'_> {
    type Target = CliTestContext;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl DerefMut for DistDirGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

impl Drop for DistDirGuard<'_> {
    fn drop(&mut self) {
        self.inner.config.distdir = None;
    }
}

fn create_local_update_server(self_dist: &Path, exedir: &Path, version: &str) -> String {
    let trip = this_host_triple();
    let dist_dir = self_dist.join(format!("archive/{version}/{trip}"));
    let dist_exe = dist_dir.join(format!("rustup-init{EXE_SUFFIX}"));
    let rustup_bin = exedir.join(format!("rustup-init{EXE_SUFFIX}"));

    fs::create_dir_all(dist_dir).unwrap();
    output_release_file(self_dist, "1", version);
    // TODO: should this hardlink since the modify-codepath presumes it has to
    // link break?
    fs::copy(rustup_bin, dist_exe).unwrap();

    let root_url = format!("file://{}", self_dist.display());
    root_url
}

pub fn output_release_file(dist_dir: &Path, schema: &str, version: &str) {
    let contents = format!(
        r#"
schema-version = "{schema}"
version = "{version}"
"#
    );
    let file = dist_dir.join("release-stable.toml");
    utils::write_file("release", &file, &contents).unwrap();
}

impl Config {
    pub fn current_dir(&self) -> PathBuf {
        self.workdir.borrow().clone()
    }

    pub fn cmd<I, A>(&self, name: &str, args: I) -> Command
    where
        I: IntoIterator<Item = A>,
        A: AsRef<OsStr>,
    {
        let exe_path = self.exedir.join(format!("{name}{EXE_SUFFIX}"));
        let mut cmd = Command::new(exe_path);
        cmd.args(args);
        cmd.current_dir(&*self.workdir.borrow());
        self.env(&mut cmd);
        cmd
    }

    pub fn env<E: rustup_test::Env>(&self, cmd: &mut E) {
        // Ensure PATH is prefixed with the rustup-exe directory
        let prev_path = env::var_os("PATH");
        let mut new_path = self.exedir.clone().into_os_string();
        if let Some(ref p) = prev_path {
            new_path.push(if cfg!(windows) { ";" } else { ":" });
            new_path.push(p);
        }
        cmd.env("PATH", new_path);
        self.rustupdir.apply(cmd);
        let distdir = match (&self.distdir, &self.const_dist_dir) {
            (None, None) => Path::new("no-such-distdir"),
            // mutable takes precedence
            (Some(distdir), _) => distdir,
            (_, Some(distdir)) => distdir,
        };
        cmd.env(
            "RUSTUP_DIST_SERVER",
            format!("file://{}", distdir.to_string_lossy()),
        );
        cmd.env("CARGO_HOME", self.cargodir.to_string_lossy().to_string());
        cmd.env("RUSTUP_OVERRIDE_HOST_TRIPLE", this_host_triple());

        // These are used in some installation tests that unset RUSTUP_HOME/CARGO_HOME
        cmd.env("HOME", self.homedir.to_string_lossy().to_string());
        cmd.env("USERPROFILE", self.homedir.to_string_lossy().to_string());

        // Setting HOME will confuse the sudo check for rustup-init. Override it
        cmd.env("RUSTUP_INIT_SKIP_SUDO_CHECK", "yes");

        // Skip the MSVC warning check since it's environment dependent
        cmd.env("RUSTUP_INIT_SKIP_MSVC_CHECK", "yes");

        // The test environment may interfere with checking the PATH for the existence of rustc or
        // cargo, so we disable that check globally
        cmd.env("RUSTUP_INIT_SKIP_PATH_CHECK", "yes");

        // Setup pgp test key
        cmd.env(
            "RUSTUP_PGP_KEY",
            std::env::current_dir()
                .unwrap()
                .join("tests/mock/signing-key.pub.asc"),
        );

        // The unix fallback settings file may be present in the test environment, so override
        // the path to the settings file with a non-existing path to avoid interference
        cmd.env(
            "RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS",
            "/bogus-config-file.toml",
        );

        if let Some(root) = self.rustup_update_root.as_ref() {
            cmd.env("RUSTUP_UPDATE_ROOT", root);
        }
    }

    /// Expect an ok status
    pub async fn expect_ok(&mut self, args: &[&str]) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if !out.ok {
            print_command(args, &out);
            println!("expected.ok: true");
            panic!();
        }
    }

    /// Expect an err status and a string in stderr
    pub async fn expect_err(&self, args: &[&str], expected: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if out.ok || !out.stderr.contains(expected) {
            print_command(args, &out);
            println!("expected.ok: false");
            print_indented("expected.stderr.contains", expected);
            panic!();
        }
    }

    /// Expect an ok status and a string in stdout
    pub async fn expect_stdout_ok(&self, args: &[&str], expected: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if !out.ok || !out.stdout.contains(expected) {
            print_command(args, &out);
            println!("expected.ok: true");
            print_indented("expected.stdout.contains", expected);
            panic!();
        }
    }

    pub async fn expect_not_stdout_ok(&self, args: &[&str], expected: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if !out.ok || out.stdout.contains(expected) {
            print_command(args, &out);
            println!("expected.ok: true");
            print_indented("expected.stdout.does_not_contain", expected);
            panic!();
        }
    }

    pub async fn expect_not_stderr_ok(&self, args: &[&str], expected: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if !out.ok || out.stderr.contains(expected) {
            print_command(args, &out);
            println!("expected.ok: false");
            print_indented("expected.stderr.does_not_contain", expected);
            panic!();
        }
    }

    pub async fn expect_not_stderr_err(&self, args: &[&str], expected: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if out.ok || out.stderr.contains(expected) {
            print_command(args, &out);
            println!("expected.ok: false");
            print_indented("expected.stderr.does_not_contain", expected);
            panic!();
        }
    }

    /// Expect an ok status and a string in stderr
    pub async fn expect_stderr_ok(&self, args: &[&str], expected: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if !out.ok || !out.stderr.contains(expected) {
            print_command(args, &out);
            println!("expected.ok: true");
            print_indented("expected.stderr.contains", expected);
            panic!();
        }
    }

    /// Expect an exact strings on stdout/stderr with an ok status code
    pub async fn expect_ok_ex(&mut self, args: &[&str], stdout: &str, stderr: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if !out.ok || out.stdout != stdout || out.stderr != stderr {
            print_command(args, &out);
            println!("expected.ok: true");
            print_indented("expected.stdout", stdout);
            print_indented("expected.stderr", stderr);
            dbg!(out.stdout == stdout);
            dbg!(out.stderr == stderr);
            panic!();
        }
    }

    /// Expect an exact strings on stdout/stderr with an error status code
    pub async fn expect_err_ex(&self, args: &[&str], stdout: &str, stderr: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if out.ok || out.stdout != stdout || out.stderr != stderr {
            print_command(args, &out);
            println!("expected.ok: false");
            print_indented("expected.stdout", stdout);
            print_indented("expected.stderr", stderr);
            if out.ok {
                panic!("expected command to fail");
            } else if out.stdout != stdout {
                panic!("expected stdout to match");
            } else if out.stderr != stderr {
                panic!("expected stderr to match");
            } else {
                unreachable!()
            }
        }
    }

    pub async fn expect_ok_contains(&self, args: &[&str], stdout: &str, stderr: &str) {
        let out = self.run(args[0], &args[1..], &[]).await;
        if !out.ok || !out.stdout.contains(stdout) || !out.stderr.contains(stderr) {
            print_command(args, &out);
            println!("expected.ok: true");
            print_indented("expected.stdout.contains", stdout);
            print_indented("expected.stderr.contains", stderr);
            panic!();
        }
    }

    pub async fn expect_ok_eq(&self, args1: &[&str], args2: &[&str]) {
        let out1 = self.run(args1[0], &args1[1..], &[]).await;
        let out2 = self.run(args2[0], &args2[1..], &[]).await;
        if !out1.ok || !out2.ok || out1.stdout != out2.stdout || out1.stderr != out2.stderr {
            print_command(args1, &out1);
            println!("expected.ok: true");
            print_command(args2, &out2);
            println!("expected.ok: true");
            panic!();
        }
    }

    pub async fn expect_component_executable(&self, cmd: &str) {
        let out1 = self.run(cmd, ["--version"], &[]).await;
        if !out1.ok {
            print_command(&[cmd, "--version"], &out1);
            println!("expected.ok: true");
            panic!()
        }
    }

    pub async fn expect_component_not_executable(&self, cmd: &str) {
        let out1 = self.run(cmd, ["--version"], &[]).await;
        if out1.ok {
            print_command(&[cmd, "--version"], &out1);
            println!("expected.ok: false");
            panic!()
        }
    }

    pub async fn run<I, A>(&self, name: &str, args: I, env: &[(&str, &str)]) -> SanitizedOutput
    where
        I: IntoIterator<Item = A> + Clone + Debug,
        A: AsRef<OsStr>,
    {
        let inprocess = allow_inprocess(name, args.clone());
        let start = Instant::now();
        let out = if inprocess {
            self.run_inprocess(name, args.clone(), env).await
        } else {
            self.run_subprocess(name, args.clone(), env)
        };
        let duration = Instant::now() - start;
        let output = SanitizedOutput {
            ok: matches!(out.status, Some(0)),
            stdout: String::from_utf8(out.stdout).unwrap(),
            stderr: String::from_utf8(out.stderr).unwrap(),
        };

        println!("ran: {} {:?}", name, args);
        println!("inprocess: {inprocess}");
        println!("status: {:?}", out.status);
        println!("duration: {:.3}s", duration.as_secs_f32());
        println!("stdout:\n====\n{}\n====\n", output.stdout);
        println!("stderr:\n====\n{}\n====\n", output.stderr);

        output
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) async fn run_inprocess<I, A>(
        &self,
        name: &str,
        args: I,
        env: &[(&str, &str)],
    ) -> Output
    where
        I: IntoIterator<Item = A>,
        A: AsRef<OsStr>,
    {
        // should we use vars_os, or skip over non-stringable vars? This is test
        // code after all...
        let mut vars: HashMap<String, String> = HashMap::default();
        self::env(self, &mut vars);
        vars.extend(env.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        let mut arg_strings: Vec<Box<str>> = Vec::new();
        arg_strings.push(name.to_owned().into_boxed_str());
        for arg in args {
            arg_strings.push(
                arg.as_ref()
                    .to_os_string()
                    .into_string()
                    .unwrap()
                    .into_boxed_str(),
            );
        }

        let tp = process::TestProcess::new(&*self.workdir.borrow(), &arg_strings, vars, "");
        let process_res = rustup_mode::main(tp.process.current_dir().unwrap(), &tp.process).await;
        // convert Err's into an ec
        let ec = match process_res {
            Ok(process_res) => process_res,
            Err(e) => {
                crate::cli::common::report_error(&e, &tp.process);
                utils::ExitCode(1)
            }
        };
        Output {
            status: Some(ec.0),
            stderr: tp.stderr(),
            stdout: tp.stdout(),
        }
    }

    #[track_caller]
    pub fn run_subprocess<I, A>(&self, name: &str, args: I, env: &[(&str, &str)]) -> Output
    where
        I: IntoIterator<Item = A>,
        A: AsRef<OsStr>,
    {
        let mut cmd = self.cmd(name, args);
        for env in env {
            cmd.env(env.0, env.1);
        }

        let mut retries = 8;
        let out = loop {
            let lock = CMD_LOCK.read().unwrap();
            let out = cmd.output();
            drop(lock);
            match out {
                Ok(out) => break out,
                Err(e) => {
                    retries -= 1;
                    if retries > 0
                        && e.kind() == std::io::ErrorKind::Other
                        && e.raw_os_error() == Some(26)
                    {
                        // This is an ETXTBSY situation
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    } else {
                        panic!("Unable to run test command: {e:?}");
                    }
                }
            }
        };
        Output {
            status: out.status.code(),
            stdout: out.stdout,
            stderr: out.stderr,
        }
    }
}

/// Change the current distribution manifest to a particular date
pub fn set_current_dist_date(config: &Config, date: &str) {
    let url = Url::from_file_path(config.distdir.as_ref().unwrap()).unwrap();
    for channel in &["nightly", "beta", "stable"] {
        change_channel_date(&url, channel, date);
    }
}

pub fn print_command(args: &[&str], out: &SanitizedOutput) {
    print!("\n>");
    for arg in args {
        if arg.contains(' ') {
            print!(" {arg:?}");
        } else {
            print!(" {arg}");
        }
    }
    println!();
    println!("out.ok: {}", out.ok);
    print_indented("out.stdout", &out.stdout);
    print_indented("out.stderr", &out.stderr);
}

pub fn print_indented(heading: &str, text: &str) {
    let mut lines = text.lines().count();
    // The standard library treats `a\n` and `a` as both being one line.
    // This is confusing when the test fails because of a missing newline.
    if !text.is_empty() && !text.ends_with('\n') {
        lines -= 1;
    }
    println!(
        "{} ({} lines):\n    {}",
        heading,
        lines,
        text.replace('\n', "\n    ")
    );
}

pub struct Output {
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub struct SanitizedOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn cmd<I, A>(config: &Config, name: &str, args: I) -> Command
where
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    config.cmd(name, args)
}

pub fn env<E: rustup_test::Env>(config: &Config, cmd: &mut E) {
    config.env(cmd)
}

fn allow_inprocess<I, A>(name: &str, args: I) -> bool
where
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    // Only the rustup alias is currently ready for in-process testing:
    // - -init performs self-update which monkey with global external state.
    // - proxies themselves behave appropriately the proxied output needs to be
    //   collected for assertions to be made on it as our tests traverse layers.
    // - self update executions cannot run in-process because on windows the
    //    process replacement dance would replace the test process.
    // - any command with --version in it is testing to see something was
    //   installed properly, so we have to shell out to it to be sure
    if name != "rustup" {
        return false;
    }
    let mut is_update = false;
    let mut no_self_update = false;
    let mut self_cmd = false;
    let mut run = false;
    let mut version = false;
    for arg in args {
        if arg.as_ref() == "update" {
            is_update = true;
        } else if arg.as_ref() == "--no-self-update" {
            no_self_update = true;
        } else if arg.as_ref() == "self" {
            self_cmd = true;
        } else if arg.as_ref() == "run" {
            run = true;
        } else if arg.as_ref() == "--version" {
            version = true;
        }
    }
    !(run || self_cmd || version || (is_update && !no_self_update))
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum RlsStatus {
    Available,
    Renamed,
    Unavailable,
}

impl RlsStatus {
    fn pkg_name(self) -> &'static str {
        match self {
            Self::Renamed => "rls-preview",
            _ => "rls",
        }
    }
}

struct Release {
    // Either "nightly", "stable", "beta", or an explicit version number
    channel: String,
    date: String,
    version: String,
    hash: String,
    rls: RlsStatus,
    available: bool,
    multi_arch: bool,
}

impl Release {
    fn stable(version: &str, date: &str) -> Self {
        Release::new("stable", version, date, version)
    }

    fn beta(version: &str, date: &str) -> Self {
        Release::new("beta", version, date, version)
    }

    fn beta_with_tag(tag: Option<&str>, version: &str, date: &str) -> Self {
        let channel = match tag {
            Some(tag) => format!("{version}-beta.{tag}"),
            None => format!("{version}-beta"),
        };
        Release::new(&channel, version, date, version)
    }

    fn with_rls(mut self, status: RlsStatus) -> Self {
        self.rls = status;
        self
    }

    fn unavailable(mut self) -> Self {
        self.available = false;
        self
    }

    fn multi_arch(mut self) -> Self {
        self.multi_arch = true;
        self
    }

    fn only_multi_arch(mut self) -> Self {
        self.multi_arch = true;
        self.available = false;
        self
    }

    fn new(channel: &str, version: &str, date: &str, suffix: &str) -> Self {
        Release {
            channel: channel.to_string(),
            date: date.to_string(),
            version: version.to_string(),
            hash: format!("hash-{channel}-{suffix}"),
            available: true,
            multi_arch: false,
            rls: RlsStatus::Available,
        }
    }

    fn mock(&self) -> MockChannel {
        if self.available {
            build_mock_channel(
                &self.channel,
                &self.date,
                &self.version,
                &self.hash,
                self.rls,
                self.multi_arch,
                false,
            )
        } else if self.multi_arch {
            // unavailable but multiarch means to build only with host==MULTI_ARCH1
            // instead of true multiarch
            build_mock_channel(
                &self.channel,
                &self.date,
                &self.version,
                &self.hash,
                self.rls,
                false,
                true,
            )
        } else {
            build_mock_unavailable_channel(&self.channel, &self.date, &self.version, &self.hash)
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn link(&self, path: &Path) {
        // Also create the manifests for releases by version
        let _ = hard_link(
            path.join(format!(
                "dist/{}/channel-rust-{}.toml",
                self.date, self.channel
            )),
            path.join(format!("dist/channel-rust-{}.toml", self.version)),
        );
        let _ = hard_link(
            path.join(format!(
                "dist/{}/channel-rust-{}.toml.asc",
                self.date, self.channel
            )),
            path.join(format!("dist/channel-rust-{}.toml.asc", self.version)),
        );
        let _ = hard_link(
            path.join(format!(
                "dist/{}/channel-rust-{}.toml.sha256",
                self.date, self.channel
            )),
            path.join(format!("dist/channel-rust-{}.toml.sha256", self.version)),
        );

        if self.channel == "stable" {
            // Same for v1 manifests. These are just the installers.
            let host_triple = this_host_triple();

            hard_link(
                path.join(format!(
                    "dist/{}/rust-stable-{}.tar.gz",
                    self.date, host_triple
                )),
                path.join(format!("dist/rust-{}-{}.tar.gz", self.version, host_triple)),
            )
            .unwrap();
            hard_link(
                path.join(format!(
                    "dist/{}/rust-stable-{}.tar.gz.sha256",
                    self.date, host_triple
                )),
                path.join(format!(
                    "dist/rust-{}-{}.tar.gz.sha256",
                    self.version, host_triple
                )),
            )
            .unwrap();
        }
    }
}

// Creates a mock dist server populated with some test data
#[tracing::instrument(level = "trace", skip_all)]
fn create_mock_dist_server(path: &Path, s: Scenario) {
    let chans = match s {
        Scenario::None => return,
        Scenario::Empty => vec![],
        Scenario::MissingComponent => vec![
            Release::new("nightly", "1.37.0", "2019-09-12", "1"),
            Release::new("nightly", "1.37.0", "2019-09-13", "2"),
            Release::new("nightly", "1.37.0", "2019-09-14", "3").with_rls(RlsStatus::Unavailable),
        ],
        Scenario::MissingNightly => vec![
            Release::new("nightly", "1.37.0", "2019-09-16", "1"),
            Release::stable("1.37.0", "2019-09-17"),
            Release::new("nightly", "1.37.0", "2019-09-18", "2").with_rls(RlsStatus::Unavailable),
        ],
        Scenario::Unavailable => vec![
            Release::new("nightly", "1.2.0", "2015-01-01", "1"),
            Release::beta("1.1.0", "2015-01-01"),
            Release::stable("1.0.0", "2015-01-01"),
            Release::new("nightly", "1.3.0", "2015-01-02", "2").unavailable(),
        ],
        Scenario::ArchivesV2_2015_01_01 => vec![
            Release::new("nightly", "1.2.0", "2015-01-01", "1").with_rls(RlsStatus::Available),
            Release::beta("1.1.0", "2015-01-01"),
            Release::stable("1.0.0", "2015-01-01"),
        ],
        Scenario::ArchivesV2TwoVersions => vec![
            Release::stable("0.100.99", "2014-12-31"),
            Release::stable("1.0.0", "2015-01-01"),
        ],
        Scenario::Full | Scenario::ArchivesV1 | Scenario::ArchivesV2 | Scenario::UnavailableRls => {
            vec![
                Release::new("nightly", "1.2.0", "2015-01-01", "1").with_rls(
                    if s == Scenario::UnavailableRls {
                        RlsStatus::Unavailable
                    } else {
                        RlsStatus::Available
                    },
                ),
                Release::beta("1.1.0", "2015-01-01"),
                // Pre-release "stable" ?
                Release::stable("0.100.99", "2014-12-31"),
                Release::stable("1.0.0", "2015-01-01"),
                Release::new("nightly", "1.3.0", "2015-01-02", "2").with_rls(RlsStatus::Renamed),
                Release::beta("1.2.0", "2015-01-02"),
                Release::stable("1.1.0", "2015-01-02"),
            ]
        }
        Scenario::RemovedRls => vec![
            Release::stable("1.78.0", "2024-05-01"),
            Release::stable("1.79.0", "2024-06-15").with_rls(RlsStatus::Unavailable),
        ],
        Scenario::SimpleV1 | Scenario::SimpleV2 => vec![
            Release::new("nightly", "1.3.0", "2015-01-02", "2").with_rls(RlsStatus::Renamed),
            Release::beta("1.2.0", "2015-01-02"),
            Release::stable("1.1.0", "2015-01-02"),
        ],
        Scenario::MultiHost => vec![
            Release::new("nightly", "1.3.0", "2015-01-02", "2").multi_arch(),
            Release::beta("1.2.0", "2015-01-02").multi_arch(),
            Release::stable("1.1.0", "2015-01-02").multi_arch(),
        ],
        Scenario::BetaTag => vec![
            Release::beta("1.78.0", "2024-03-19"),
            Release::beta_with_tag(None, "1.78.0", "2024-03-19"),
            Release::beta("1.79.0", "2024-05-03"),
            Release::beta_with_tag(Some("1"), "1.79.0", "2024-04-29"),
            Release::beta_with_tag(Some("2"), "1.79.0", "2024-05-03"),
        ],
        Scenario::HostGoesMissingBefore => {
            vec![Release::new("nightly", "1.3.0", "2019-12-09", "1")]
        }
        Scenario::HostGoesMissingAfter => {
            vec![Release::new("nightly", "1.3.0", "2019-12-10", "2").only_multi_arch()]
        }
        Scenario::MissingComponentMulti => vec![
            Release::new("nightly", "1.37.0", "2019-09-12", "1")
                .multi_arch()
                .with_rls(RlsStatus::Renamed),
            Release::new("nightly", "1.37.0", "2019-09-13", "2").with_rls(RlsStatus::Renamed),
            Release::new("nightly", "1.37.0", "2019-09-14", "3")
                .multi_arch()
                .with_rls(RlsStatus::Unavailable),
        ],
    };

    let vs = match s {
        Scenario::None => unreachable!("None exits above"),
        Scenario::Empty => vec![],
        Scenario::Full => vec![MockManifestVersion::V1, MockManifestVersion::V2],
        Scenario::SimpleV1 | Scenario::ArchivesV1 => vec![MockManifestVersion::V1],
        Scenario::SimpleV2
        | Scenario::ArchivesV2
        | Scenario::ArchivesV2_2015_01_01
        | Scenario::ArchivesV2TwoVersions
        | Scenario::BetaTag
        | Scenario::MultiHost
        | Scenario::Unavailable
        | Scenario::UnavailableRls
        | Scenario::RemovedRls
        | Scenario::MissingNightly
        | Scenario::HostGoesMissingBefore
        | Scenario::HostGoesMissingAfter
        | Scenario::MissingComponent
        | Scenario::MissingComponentMulti => vec![MockManifestVersion::V2],
    };

    MockDistServer {
        path: path.to_owned(),
        channels: chans.iter().map(|c| c.mock()).collect(),
    }
    .write(&vs, true, true);

    for chan in &chans {
        chan.link(path)
    }
}

#[derive(Default)]
struct MockChannelContent {
    std: Vec<(MockInstallerBuilder, String)>,
    rustc: Vec<(MockInstallerBuilder, String)>,
    cargo: Vec<(MockInstallerBuilder, String)>,
    rls: Vec<(MockInstallerBuilder, String)>,
    docs: Vec<(MockInstallerBuilder, String)>,
    src: Vec<(MockInstallerBuilder, String)>,
    analysis: Vec<(MockInstallerBuilder, String)>,
    combined: Vec<(MockInstallerBuilder, String)>,
}

impl MockChannelContent {
    fn into_packages(
        self,
        rls_name: &'static str,
    ) -> Vec<(&'static str, Vec<(MockInstallerBuilder, String)>)> {
        vec![
            ("rust-std", self.std),
            ("rustc", self.rustc),
            ("cargo", self.cargo),
            (rls_name, self.rls),
            ("rust-docs", self.docs),
            ("rust-src", self.src),
            ("rust-analysis", self.analysis),
            ("rust", self.combined),
        ]
    }
}

fn build_mock_channel(
    channel: &str,
    date: &str,
    version: &str,
    version_hash: &str,
    rls: RlsStatus,
    multi_arch: bool,
    swap_triples: bool,
) -> MockChannel {
    // Build the mock installers
    let host_triple = if swap_triples {
        MULTI_ARCH1.to_owned()
    } else {
        this_host_triple()
    };
    let std = build_mock_std_installer(&host_triple);
    let rustc = build_mock_rustc_installer(&host_triple, version, version_hash);
    let cargo = build_mock_cargo_installer(version, version_hash);
    let rust_docs = build_mock_rust_doc_installer();
    let rust = build_combined_installer(&[&std, &rustc, &cargo, &rust_docs]);
    let cross_std1 = build_mock_cross_std_installer(CROSS_ARCH1, date);
    let cross_std2 = build_mock_cross_std_installer(CROSS_ARCH2, date);
    let rust_src = build_mock_rust_src_installer();
    let rust_analysis = build_mock_rust_analysis_installer(&host_triple);

    // Convert the mock installers to mock package definitions for the
    // mock dist server
    let mut all = MockChannelContent::default();
    all.std.extend(vec![
        (std, host_triple.clone()),
        (cross_std1, CROSS_ARCH1.to_string()),
        (cross_std2, CROSS_ARCH2.to_string()),
    ]);
    all.rustc.push((rustc, host_triple.clone()));
    all.cargo.push((cargo, host_triple.clone()));

    if rls != RlsStatus::Unavailable {
        let rls = build_mock_rls_installer(version, version_hash, rls.pkg_name());
        all.rls.push((rls, host_triple.clone()));
    } else {
        all.rls.push((
            MockInstallerBuilder { components: vec![] },
            host_triple.clone(),
        ));
    }

    all.docs.push((rust_docs, host_triple.clone()));
    all.src.push((rust_src, "*".to_string()));
    all.analysis.push((rust_analysis, "*".to_string()));
    all.combined.push((rust, host_triple));

    if multi_arch {
        let std = build_mock_std_installer(MULTI_ARCH1);
        let rustc = build_mock_rustc_installer(MULTI_ARCH1, version, version_hash);
        let cargo = build_mock_cargo_installer(version, version_hash);
        let rust_docs = build_mock_rust_doc_installer();
        let rust = build_combined_installer(&[&std, &rustc, &cargo, &rust_docs]);

        let triple = MULTI_ARCH1.to_string();
        all.std.push((std, triple.clone()));
        all.rustc.push((rustc, triple.clone()));
        all.cargo.push((cargo, triple.clone()));

        if rls != RlsStatus::Unavailable {
            let rls = build_mock_rls_installer(version, version_hash, rls.pkg_name());
            all.rls.push((rls, triple.clone()));
        } else {
            all.rls
                .push((MockInstallerBuilder { components: vec![] }, triple.clone()));
        }

        all.docs.push((rust_docs, triple.to_string()));
        all.combined.push((rust, triple));
    }

    let all_std_archs: Vec<String> = all.std.iter().map(|(_, arch)| arch).cloned().collect();

    let all = all.into_packages(rls.pkg_name());

    let packages = all.into_iter().map(|(name, target_pkgs)| {
        let target_pkgs = target_pkgs
            .into_iter()
            .map(|(installer, triple)| MockTargetedPackage {
                target: triple,
                available: !installer.components.is_empty(),
                components: vec![],
                installer,
            });

        MockPackage {
            name,
            version: format!("{version} ({version_hash})"),
            targets: target_pkgs.collect(),
        }
    });
    let mut packages: Vec<_> = packages.collect();

    // Add subcomponents of the rust package
    {
        let rust_pkg = packages.last_mut().unwrap();
        for target_pkg in rust_pkg.targets.iter_mut() {
            let target = &target_pkg.target;
            target_pkg.components.push(MockComponent {
                name: "rust-std".to_string(),
                target: target.to_string(),
                is_extension: false,
            });
            target_pkg.components.push(MockComponent {
                name: "rustc".to_string(),
                target: target.to_string(),
                is_extension: false,
            });
            target_pkg.components.push(MockComponent {
                name: "cargo".to_string(),
                target: target.to_string(),
                is_extension: false,
            });
            target_pkg.components.push(MockComponent {
                name: "rust-docs".to_string(),
                target: target.to_string(),
                is_extension: false,
            });
            if rls == RlsStatus::Renamed {
                target_pkg.components.push(MockComponent {
                    name: "rls-preview".to_string(),
                    target: target.to_string(),
                    is_extension: true,
                });
            } else if rls == RlsStatus::Available {
                target_pkg.components.push(MockComponent {
                    name: "rls".to_string(),
                    target: target.to_string(),
                    is_extension: true,
                });
            } else {
                target_pkg.components.push(MockComponent {
                    name: "rls".to_string(),
                    target: target.to_string(),
                    is_extension: true,
                })
            }
            for other_target in &all_std_archs {
                if other_target != target {
                    target_pkg.components.push(MockComponent {
                        name: "rust-std".to_string(),
                        target: other_target.to_string(),
                        is_extension: false,
                    });
                }
            }

            target_pkg.components.push(MockComponent {
                name: "rust-src".to_string(),
                target: "*".to_string(),
                is_extension: true,
            });
            target_pkg.components.push(MockComponent {
                name: "rust-analysis".to_string(),
                target: target.to_string(),
                is_extension: true,
            });
        }
    }

    let mut renames = HashMap::new();
    if rls == RlsStatus::Renamed {
        renames.insert("rls".to_owned(), "rls-preview".to_owned());
    }

    MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages,
        renames,
    }
}

fn build_mock_unavailable_channel(
    channel: &str,
    date: &str,
    version: &str,
    version_hash: &str,
) -> MockChannel {
    let host_triple = this_host_triple();

    let packages = [
        "cargo",
        "rust",
        "rust-docs",
        "rust-std",
        "rustc",
        "rls",
        "rust-analysis",
    ];
    let packages = packages
        .iter()
        .map(|name| MockPackage {
            name,
            version: format!("{version} ({version_hash})"),
            targets: vec![MockTargetedPackage {
                target: host_triple.clone(),
                available: false,
                components: vec![],
                installer: MockInstallerBuilder { components: vec![] },
            }],
        })
        .collect();

    MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages,
        renames: HashMap::new(),
    }
}

fn build_mock_std_installer(trip: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: format!("rust-std-{trip}"),
            files: vec![MockFile::new(
                format!("lib/rustlib/{trip}/libstd.rlib"),
                b"",
            )],
        }],
    }
}

fn build_mock_cross_std_installer(target: &str, date: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: format!("rust-std-{target}"),
            files: vec![
                MockFile::new(format!("lib/rustlib/{target}/lib/libstd.rlib"), b""),
                MockFile::new(format!("lib/rustlib/{target}/lib/{date}"), b""),
            ],
        }],
    }
}

fn build_mock_rustc_installer(
    target: &str,
    version: &str,
    version_hash_: &str,
) -> MockInstallerBuilder {
    // For cross-host rustc's modify the version_hash so they can be identified from
    // test cases.
    let this_host = this_host_triple();
    let version_hash = if this_host != target {
        format!("xxxx-{}", &version_hash_[5..])
    } else {
        version_hash_.to_string()
    };

    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "rustc".to_string(),
            files: mock_bin("rustc", version, &version_hash),
        }],
    }
}

fn build_mock_cargo_installer(version: &str, version_hash: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "cargo".to_string(),
            files: mock_bin("cargo", version, version_hash),
        }],
    }
}

fn build_mock_rls_installer(
    version: &str,
    version_hash: &str,
    pkg_name: &str,
) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: pkg_name.to_string(),
            files: mock_bin("rls", version, version_hash),
        }],
    }
}

fn build_mock_rust_doc_installer() -> MockInstallerBuilder {
    let mut files: Vec<MockFile> = topical_doc_data::unique_paths()
        .map(|x| MockFile::new(x, b""))
        .collect();
    files.insert(0, MockFile::new("share/doc/rust/html/index.html", b""));
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "rust-docs".to_string(),
            files,
        }],
    }
}

fn build_mock_rust_analysis_installer(trip: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: format!("rust-analysis-{trip}"),
            files: vec![MockFile::new(
                format!("lib/rustlib/{trip}/analysis/libfoo.json"),
                b"",
            )],
        }],
    }
}

fn build_mock_rust_src_installer() -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "rust-src".to_string(),
            files: vec![MockFile::new("lib/rustlib/src/rust-src/foo.rs", b"")],
        }],
    }
}

fn build_combined_installer(components: &[&MockInstallerBuilder]) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: components
            .iter()
            .flat_map(|m| m.components.clone())
            .collect(),
    }
}

/// This is going to run the compiler to create an executable that
/// prints some version information. These binaries are stuffed into
/// the mock installers so we have executables for rustup to run.
///
/// To avoid compiling tons of files we globally cache one compiled executable
/// and then we store some associated files next to it which indicate
/// the version/version hash information.
fn mock_bin(name: &str, version: &str, version_hash: &str) -> Vec<MockFile> {
    static MOCK_BIN: LazyLock<Arc<Vec<u8>>> = LazyLock::new(|| {
        // Create a temp directory to hold the source and the output
        let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let source_path = tempdir.path().join("in.rs");
        let dest_path = tempdir.path().join(format!("out{EXE_SUFFIX}"));

        // Write the source
        let source = include_bytes!("mock_bin_src.rs");
        fs::write(&source_path, &source[..]).unwrap();

        // Create the executable
        let status = Command::new("rustc")
            .arg(&source_path)
            .arg("-C")
            .arg("panic=abort")
            .arg("-O")
            .arg("-o")
            .arg(&dest_path)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(dest_path.exists());

        // Remove debug info from std/core which included in every programs,
        // otherwise we just ignore the return result here
        if cfg!(unix) {
            drop(Command::new("strip").arg(&dest_path).status());
        }

        // Now load it into memory
        let buf = fs::read(dest_path).unwrap();
        Arc::new(buf)
    });

    let name = format!("bin/{name}{EXE_SUFFIX}");
    vec![
        MockFile::new(format!("{name}.version"), version.as_bytes()),
        MockFile::new(format!("{name}.version-hash"), version_hash.as_bytes()),
        MockFile::new_arc(name, MOCK_BIN.clone()).executable(true),
    ]
}

// These are toolchains for installation with --link-local and --copy-local
fn create_custom_toolchains(customdir: &Path) {
    let libdir = customdir.join("custom-1/lib");
    fs::create_dir_all(libdir).unwrap();
    for file in mock_bin("rustc", "1.0.0", "hash-c-1") {
        file.build(&customdir.join("custom-1"));
    }

    let libdir = customdir.join("custom-2/lib");
    fs::create_dir_all(libdir).unwrap();
    for file in mock_bin("rustc", "1.0.0", "hash-c-2") {
        file.build(&customdir.join("custom-2"));
    }
}

pub fn hard_link<A, B>(original: A, link: B) -> io::Result<()>
where
    A: AsRef<Path>,
    B: AsRef<Path>,
{
    fn inner(a: &Path, b: &Path) -> io::Result<()> {
        match fs::remove_file(b) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        fs::hard_link(a, b).map(drop)
    }
    inner(original.as_ref(), link.as_ref())
}
