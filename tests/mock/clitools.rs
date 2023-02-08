//! A mock distribution server used by tests/cli-v1.rs and
//! tests/cli-v2.rs
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use lazy_static::lazy_static;
use url::Url;

use rustup::cli::rustup_mode;
use rustup::currentprocess;
use rustup::test as rustup_test;
use rustup::test::this_host_triple;
use rustup::utils::{raw, utils};

use crate::mock::dist::{
    change_channel_date, ManifestVersion, MockChannel, MockComponent, MockDistServer, MockPackage,
    MockTargetedPackage,
};
use crate::mock::topical_doc_data;
use crate::mock::{MockComponentBuilder, MockFile, MockInstallerBuilder};

/// The configuration used by the tests in this module
pub struct Config {
    /// Where we put the rustup / rustc / cargo bins
    pub exedir: PathBuf,
    /// The distribution server
    pub distdir: PathBuf,
    /// RUSTUP_HOME
    pub rustupdir: rustup_test::RustupHome,
    /// Custom toolchains
    pub customdir: PathBuf,
    /// CARGO_HOME
    pub cargodir: PathBuf,
    /// ~
    pub homedir: PathBuf,
    /// An empty directory. Tests should not write to this.
    pub emptydir: PathBuf,
    /// Root for updates to rustup itself aka RUSTUP_UPDATE_ROOT
    pub rustup_update_root: Option<String>,
    /// This is cwd for the test
    pub workdir: RefCell<PathBuf>,
}

// Describes all the features of the mock dist server.
// Building the mock server is slow, so use simple scenario when possible.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Scenario {
    /// No dist server content
    Empty,
    /// Two dates, two manifests
    Full,
    /// Two dates, v2 manifests
    ArchivesV2,
    /// Two dates, v1 manifests
    ArchivesV1,
    /// One date, v2 manifests
    SimpleV2,
    /// One date, v1 manifests
    SimpleV1,
    /// One date, v2 manifests, MULTI_ARCH1 host
    MultiHost,
    /// Two dates, v2 manifests, everything unavailable in second date.
    Unavailable,
    /// Two dates, v2 manifests, RLS unavailable in first date, restored on second.
    UnavailableRls,
    /// Three dates, v2 manifests, RLS available in first and last, not middle
    MissingComponent,
    /// Three dates, v2 manifests, RLS available in first, middle missing nightly
    MissingNightly,
    /// Two dates, v2 manifests, host and MULTI_ARCH1 in first, host not in second
    HostGoesMissing,
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

/// Run this to create the test environment containing rustup, and
/// a mock dist server.
pub fn setup(s: Scenario, f: &dyn Fn(&mut Config)) {
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
    let mut exe_dir = current_exe_path.parent().unwrap();
    if exe_dir.ends_with("deps") {
        exe_dir = exe_dir.parent().unwrap();
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
    let distdir = tempdir_in_with_prefix(&test_dir, "rustup-dist");
    let customdir = tempdir_in_with_prefix(&test_dir, "rustup-custom");
    let cargodir = tempdir_in_with_prefix(&test_dir, "rustup-cargo");
    let homedir = tempdir_in_with_prefix(&test_dir, "rustup-home");
    let emptydir = tempdir_in_with_prefix(&test_dir, "rustup-empty");
    let workdir = tempdir_in_with_prefix(&test_dir, "rustup-workdir");

    // The uninstall process on windows involves using the directory above
    // CARGO_HOME, so make sure it's a subdir of our tempdir
    let cargodir = cargodir.join("ch");
    fs::create_dir(&cargodir).unwrap();

    let mut config = Config {
        exedir,
        distdir,
        rustupdir: rustup_test::RustupHome::new_in(&test_dir).unwrap(),
        customdir,
        cargodir,
        homedir,
        emptydir,
        rustup_update_root: None,
        workdir: RefCell::new(workdir),
    };

    create_mock_dist_server(&config.distdir, s);

    let build_path = exe_dir.join(format!("rustup-init{EXE_SUFFIX}"));

    let rustup_path = config.exedir.join(format!("rustup{EXE_SUFFIX}"));
    let setup_path = config.exedir.join(format!("rustup-init{EXE_SUFFIX}"));
    let rustc_path = config.exedir.join(format!("rustc{EXE_SUFFIX}"));
    let cargo_path = config.exedir.join(format!("cargo{EXE_SUFFIX}"));
    let rls_path = config.exedir.join(format!("rls{EXE_SUFFIX}"));
    let rust_lldb_path = config.exedir.join(format!("rust-lldb{EXE_SUFFIX}"));

    copy_binary(&build_path, &rustup_path).unwrap();
    hard_link(&rustup_path, setup_path).unwrap();
    hard_link(&rustup_path, rustc_path).unwrap();
    hard_link(&rustup_path, cargo_path).unwrap();
    hard_link(&rustup_path, rls_path).unwrap();
    hard_link(&rustup_path, rust_lldb_path).unwrap();

    // Make sure the host triple matches the build triple. Otherwise testing a 32-bit build of
    // rustup on a 64-bit machine will fail, because the tests do not have the host detection
    // functionality built in.
    run(
        &config,
        "rustup",
        &["set", "default-host", &this_host_triple()],
        &[],
    );

    // Set the auto update mode to disable, as most tests do not want to update rustup itself during the test.
    run(
        &config,
        "rustup",
        &["set", "auto-self-update", "disable"],
        &[],
    );

    // Create some custom toolchains
    create_custom_toolchains(&config.customdir);

    f(&mut config);

    // These are the bogus values the test harness sets "HOME" and "CARGO_HOME"
    // to during testing. If they exist that means a test unexpectedly used
    // one of these environment variables.
    assert!(!PathBuf::from("./bogus-home").exists());
    assert!(!PathBuf::from("./bogus-cargo-home").exists());
}

fn create_local_update_server(self_dist: &Path, config: &mut Config, version: &str) {
    let trip = this_host_triple();
    let dist_dir = self_dist.join(&format!("archive/{version}/{trip}"));
    let dist_exe = dist_dir.join(&format!("rustup-init{EXE_SUFFIX}"));
    let rustup_bin = config.exedir.join(&format!("rustup-init{EXE_SUFFIX}"));

    fs::create_dir_all(dist_dir).unwrap();
    output_release_file(self_dist, "1", version);
    fs::copy(&rustup_bin, &dist_exe).unwrap();

    let root_url = format!("file://{}", self_dist.display());
    config.rustup_update_root = Some(root_url);
}

pub fn check_update_setup(f: &dyn Fn(&mut Config)) {
    let version = env!("CARGO_PKG_VERSION");

    setup(Scenario::ArchivesV2, &|config| {
        let self_dist_tmp = tempfile::Builder::new()
            .prefix("self_dist")
            .tempdir()
            .unwrap();
        let self_dist = self_dist_tmp.path();

        create_local_update_server(self_dist, config, version);

        f(config);
    });
}

pub fn self_update_setup(f: &dyn Fn(&Config, &Path), version: &str) {
    setup(Scenario::SimpleV2, &|config| {
        // Create a mock self-update server
        let self_dist_tmp = tempfile::Builder::new()
            .prefix("self_dist")
            .tempdir()
            .unwrap();
        let self_dist = self_dist_tmp.path();

        create_local_update_server(self_dist, config, version);

        let trip = this_host_triple();
        let dist_dir = self_dist.join(&format!("archive/{version}/{trip}"));
        let dist_exe = dist_dir.join(&format!("rustup-init{EXE_SUFFIX}"));

        // Modify the exe so it hashes different
        raw::append_file(&dist_exe, "").unwrap();

        f(config, self_dist);
    });
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

    pub fn change_dir<F>(&self, path: &Path, mut f: F)
    where
        F: FnMut(),
    {
        self._change_dir(path, &mut f)
    }

    fn _change_dir(&self, path: &Path, f: &mut dyn FnMut()) {
        let prev = self.workdir.replace(path.to_owned());
        f();
        *self.workdir.borrow_mut() = prev;
    }

    pub fn create_rustup_sh_metadata(&self) {
        let rustup_dir = self.homedir.join(".rustup");
        fs::create_dir_all(&rustup_dir).unwrap();
        let version_file = rustup_dir.join("rustup-version");
        raw::write_file(&version_file, "").unwrap();
    }
}

/// Change the current distribution manifest to a particular date
pub fn set_current_dist_date(config: &Config, date: &str) {
    let url = Url::from_file_path(&config.distdir).unwrap();
    for channel in &["nightly", "beta", "stable"] {
        change_channel_date(&url, channel, date);
    }
}

pub fn expect_ok(config: &Config, args: &[&str]) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok {
        print_command(args, &out);
        println!("expected.ok: true");
        panic!();
    }
}

pub fn expect_err(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if out.ok || !out.stderr.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: false");
        print_indented("expected.stderr.contains", expected);
        panic!();
    }
}

pub fn expect_stdout_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || !out.stdout.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: true");
        print_indented("expected.stdout.contains", expected);
        panic!();
    }
}

pub fn expect_not_stdout_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || out.stdout.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: true");
        print_indented("expected.stdout.does_not_contain", expected);
        panic!();
    }
}

pub fn expect_not_stderr_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || out.stderr.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: false");
        print_indented("expected.stderr.does_not_contain", expected);
        panic!();
    }
}

pub fn expect_not_stderr_err(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if out.ok || out.stderr.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: false");
        print_indented("expected.stderr.does_not_contain", expected);
        panic!();
    }
}

pub fn expect_stderr_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || !out.stderr.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: true");
        print_indented("expected.stderr.contains", expected);
        panic!();
    }
}

pub fn expect_ok_ex(config: &Config, args: &[&str], stdout: &str, stderr: &str) {
    let out = run(config, args[0], &args[1..], &[]);
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

pub fn expect_err_ex(config: &Config, args: &[&str], stdout: &str, stderr: &str) {
    let out = run(config, args[0], &args[1..], &[]);
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

pub fn expect_ok_contains(config: &Config, args: &[&str], stdout: &str, stderr: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || !out.stdout.contains(stdout) || !out.stderr.contains(stderr) {
        print_command(args, &out);
        println!("expected.ok: true");
        print_indented("expected.stdout.contains", stdout);
        print_indented("expected.stderr.contains", stderr);
        panic!();
    }
}

pub fn expect_ok_eq(config: &Config, args1: &[&str], args2: &[&str]) {
    let out1 = run(config, args1[0], &args1[1..], &[]);
    let out2 = run(config, args2[0], &args2[1..], &[]);
    if !out1.ok || !out2.ok || out1.stdout != out2.stdout || out1.stderr != out2.stderr {
        print_command(args1, &out1);
        println!("expected.ok: true");
        print_command(args2, &out2);
        println!("expected.ok: true");
        panic!();
    }
}

pub fn expect_component_executable(config: &Config, cmd: &str) {
    let out1 = run(config, cmd, &["--version"], &[]);
    if !out1.ok {
        print_command(&[cmd, "--version"], &out1);
        println!("expected.ok: true");
        panic!()
    }
}

pub fn expect_component_not_executable(config: &Config, cmd: &str) {
    let out1 = run(config, cmd, &["--version"], &[]);
    if out1.ok {
        print_command(&[cmd, "--version"], &out1);
        println!("expected.ok: false");
        panic!()
    }
}

pub(crate) fn print_command(args: &[&str], out: &SanitizedOutput) {
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

pub(crate) fn print_indented(heading: &str, text: &str) {
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
    let exe_path = config.exedir.join(format!("{name}{EXE_SUFFIX}"));
    let mut cmd = Command::new(exe_path);
    cmd.args(args);
    cmd.current_dir(&*config.workdir.borrow());
    env(config, &mut cmd);
    cmd
}

pub fn env<E: rustup_test::Env>(config: &Config, cmd: &mut E) {
    // Ensure PATH is prefixed with the rustup-exe directory
    let prev_path = env::var_os("PATH");
    let mut new_path = config.exedir.clone().into_os_string();
    if let Some(ref p) = prev_path {
        new_path.push(if cfg!(windows) { ";" } else { ":" });
        new_path.push(p);
    }
    cmd.env("PATH", new_path);
    config.rustupdir.apply(cmd);
    cmd.env(
        "RUSTUP_DIST_SERVER",
        format!("file://{}", config.distdir.to_string_lossy()),
    );
    cmd.env("CARGO_HOME", config.cargodir.to_string_lossy().to_string());
    cmd.env("RUSTUP_OVERRIDE_HOST_TRIPLE", this_host_triple());

    // These are used in some installation tests that unset RUSTUP_HOME/CARGO_HOME
    cmd.env("HOME", config.homedir.to_string_lossy().to_string());
    cmd.env("USERPROFILE", config.homedir.to_string_lossy().to_string());

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

    if let Some(root) = config.rustup_update_root.as_ref() {
        cmd.env("RUSTUP_UPDATE_ROOT", root);
    }
}

use std::sync::RwLock;

/// Returns the lock to be used when creating test environments.
///
/// Essentially we use this in `.read()` mode to gate access to `fork()`
/// new subprocesses, and in `.write()` mode to gate creation of new test
/// environments. In doing this we can ensure that new test environment creation
/// does not result in ETXTBSY because the FDs in question happen to be in
/// newly `fork()`d but not yet `exec()`d subprocesses of other tests.
pub fn cmd_lock() -> &'static RwLock<()> {
    lazy_static! {
        static ref LOCK: RwLock<()> = RwLock::new(());
    };
    &LOCK
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

pub fn run<I, A>(config: &Config, name: &str, args: I, env: &[(&str, &str)]) -> SanitizedOutput
where
    I: IntoIterator<Item = A> + Clone,
    A: AsRef<OsStr>,
{
    let inprocess = allow_inprocess(name, args.clone());
    let out = if inprocess {
        run_inprocess(config, name, args, env)
    } else {
        run_subprocess(config, name, args, env)
    };
    let output = SanitizedOutput {
        ok: matches!(out.status, Some(0)),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    };

    println!("inprocess: {inprocess}");
    println!("status: {:?}", out.status);
    println!("----- stdout\n{}", output.stdout);
    println!("----- stderr\n{}", output.stderr);

    output
}

pub(crate) fn run_inprocess<I, A>(
    config: &Config,
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
    self::env(config, &mut vars);
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
    let tp = Box::new(currentprocess::TestProcess::new(
        &*config.workdir.borrow(),
        &arg_strings,
        vars,
        "",
    ));
    let process_res = currentprocess::with(tp.clone(), rustup_mode::main);
    // convert Err's into an ec
    let ec = match process_res {
        Ok(process_res) => process_res,
        Err(e) => {
            currentprocess::with(tp.clone(), || rustup::cli::common::report_error(&e));
            utils::ExitCode(1)
        }
    };
    Output {
        status: Some(ec.0),
        stderr: (*tp).get_stderr(),
        stdout: (*tp).get_stdout(),
    }
}

pub fn run_subprocess<I, A>(config: &Config, name: &str, args: I, env: &[(&str, &str)]) -> Output
where
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    let mut cmd = cmd(config, name, args);
    for env in env {
        cmd.env(env.0, env.1);
    }

    let mut retries = 8;
    let out = loop {
        let lock = cmd_lock().read().unwrap();
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
fn create_mock_dist_server(path: &Path, s: Scenario) {
    let chans = match s {
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
        Scenario::HostGoesMissing => vec![
            Release::new("nightly", "1.3.0", "2019-12-09", "1"),
            Release::new("nightly", "1.3.0", "2019-12-10", "2").only_multi_arch(),
        ],
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
        Scenario::Empty => vec![],
        Scenario::Full => vec![ManifestVersion::V1, ManifestVersion::V2],
        Scenario::SimpleV1 | Scenario::ArchivesV1 => vec![ManifestVersion::V1],
        Scenario::SimpleV2
        | Scenario::ArchivesV2
        | Scenario::MultiHost
        | Scenario::Unavailable
        | Scenario::UnavailableRls
        | Scenario::MissingNightly
        | Scenario::HostGoesMissing
        | Scenario::MissingComponent
        | Scenario::MissingComponentMulti => vec![ManifestVersion::V2],
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
    all.std.extend(
        vec![
            (std, host_triple.clone()),
            (cross_std1, CROSS_ARCH1.to_string()),
            (cross_std2, CROSS_ARCH2.to_string()),
        ]
        .into_iter(),
    );
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
    lazy_static! {
        static ref MOCK_BIN: Arc<Vec<u8>> = {
            // Create a temp directory to hold the source and the output
            let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
            let source_path = tempdir.path().join("in.rs");
            let dest_path = tempdir.path().join(&format!("out{EXE_SUFFIX}"));

            // Write the source
            let source = include_bytes!("mock_bin_src.rs");
            fs::write(&source_path, &source[..]).unwrap();

            // Create the executable
            let status = Command::new("rustc")
                .arg(&source_path)
                .arg("-C").arg("panic=abort")
                .arg("-O")
                .arg("-o").arg(&dest_path)
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
        };
    }

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

pub fn hard_link<A, B>(a: A, b: B) -> io::Result<()>
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
    inner(a.as_ref(), b.as_ref())
}

pub fn copy_binary<A, B>(a: A, b: B) -> io::Result<()>
where
    A: AsRef<Path>,
    B: AsRef<Path>,
{
    fn inner(a: &Path, b: &Path) -> io::Result<()> {
        match fs::remove_file(b) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        fs::copy(a, b).map(drop)
    }
    let _lock = cmd_lock().write().unwrap();
    inner(a.as_ref(), b.as_ref())
}
