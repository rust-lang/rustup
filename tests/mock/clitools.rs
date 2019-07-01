//! A mock distribution server used by tests/cli-v1.rs and
//! tests/cli-v2.rs

use crate::mock::dist::{
    change_channel_date, ManifestVersion, MockChannel, MockComponent, MockDistServer, MockPackage,
    MockTargetedPackage,
};
use crate::mock::{MockComponentBuilder, MockFile, MockInstallerBuilder};
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::env::consts::EXE_SUFFIX;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use url::Url;

/// The configuration used by the tests in this module
pub struct Config {
    /// Where we put the rustup / rustc / cargo bins
    pub exedir: PathBuf,
    /// The distribution server
    pub distdir: PathBuf,
    /// RUSTUP_HOME
    pub rustupdir: PathBuf,
    /// Custom toolchains
    pub customdir: PathBuf,
    /// CARGO_HOME
    pub cargodir: PathBuf,
    /// ~
    pub homedir: PathBuf,
    /// An empty directory. Tests should not write to this.
    pub emptydir: PathBuf,
    /// This is cwd for the test
    pub workdir: RefCell<PathBuf>,
}

// Describes all the features of the mock dist server.
// Building the mock server is slow, so use simple scenario when possible.
#[derive(PartialEq, Copy, Clone)]
pub enum Scenario {
    Full,        // Two dates, two manifests
    ArchivesV2,  // Two dates, v2 manifests
    ArchivesV1,  // Two dates, v1 manifests
    SimpleV2,    // One date, v2 manifests
    SimpleV1,    // One date, v1 manifests
    MultiHost,   // One date, v2 manifests, MULTI_ARCH1 host
    Unavailable, // Two dates, v2 manifests, everything unavailable in second date.
}

pub static CROSS_ARCH1: &str = "x86_64-unknown-linux-musl";
pub static CROSS_ARCH2: &str = "arm-linux-androideabi";

// Architecture for testing 'multi-host' installation.
// FIXME: Unfortunately the list of supported hosts is hard-coded,
// so we have to use the triple of a host we actually test on. That means
// that when we're testing on that host we can't test 'multi-host'.
pub static MULTI_ARCH1: &str = "i686-unknown-linux-gnu";

/// Run this to create the test environment containing rustup, and
/// a mock dist server.
pub fn setup(s: Scenario, f: &dyn Fn(&mut Config)) {
    // Unset env variables that will break our testing
    env::remove_var("RUSTUP_TOOLCHAIN");
    env::remove_var("SHELL");
    env::remove_var("ZDOTDIR");

    match env::var("RUSTUP_BACKTRACE") {
        Ok(val) => env::set_var("RUST_BACKTRACE", val),
        _ => env::remove_var("RUST_BACKTRACE"),
    }

    let current_exe_path = env::current_exe().map(PathBuf::from).unwrap();
    let mut exe_dir = current_exe_path.parent().unwrap();
    if exe_dir.ends_with("deps") {
        exe_dir = exe_dir.parent().unwrap();
    }
    let test_dir = exe_dir.parent().unwrap().join("tests");
    fs::create_dir_all(&test_dir).unwrap();

    fn tempdir_in_with_prefix(path: &Path, prefix: &str) -> PathBuf {
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(path)
            .unwrap()
            .into_path()
    }

    let exedir = tempdir_in_with_prefix(&test_dir, "rustup-exe");
    let distdir = tempdir_in_with_prefix(&test_dir, "rustup-dist");
    let rustupdir = tempdir_in_with_prefix(&test_dir, "rustup");
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
        rustupdir,
        customdir,
        cargodir,
        homedir,
        emptydir,
        workdir: RefCell::new(workdir),
    };

    create_mock_dist_server(&config.distdir, s);

    let build_path = exe_dir.join(format!("rustup-init{}", EXE_SUFFIX));

    let rustup_path = config.exedir.join(format!("rustup{}", EXE_SUFFIX));
    let setup_path = config.exedir.join(format!("rustup-init{}", EXE_SUFFIX));
    let rustc_path = config.exedir.join(format!("rustc{}", EXE_SUFFIX));
    let cargo_path = config.exedir.join(format!("cargo{}", EXE_SUFFIX));
    let rls_path = config.exedir.join(format!("rls{}", EXE_SUFFIX));

    copy_binary(&build_path, &rustup_path).unwrap();
    hard_link(&rustup_path, setup_path).unwrap();
    hard_link(&rustup_path, rustc_path).unwrap();
    hard_link(&rustup_path, cargo_path).unwrap();
    hard_link(&rustup_path, rls_path).unwrap();

    // Make sure the host triple matches the build triple. Otherwise testing a 32-bit build of
    // rustup on a 64-bit machine will fail, because the tests do not have the host detection
    // functionality built in.
    run(
        &config,
        "rustup",
        &["set", "default-host", &this_host_triple()],
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
        panic!();
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

fn print_command(args: &[&str], out: &SanitizedOutput) {
    print!("\n>");
    for arg in args {
        if arg.contains(' ') {
            print!(" {:?}", arg);
        } else {
            print!(" {}", arg);
        }
    }
    println!();
    println!("out.ok: {}", out.ok);
    print_indented("out.stdout", &out.stdout);
    print_indented("out.stderr", &out.stderr);
}

fn print_indented(heading: &str, text: &str) {
    println!(
        "{} ({} lines):\n    {}",
        heading,
        text.lines().count(),
        text.replace("\n", "\n    ")
    );
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
    let exe_path = config.exedir.join(format!("{}{}", name, EXE_SUFFIX));
    let mut cmd = Command::new(exe_path);
    cmd.args(args);
    cmd.current_dir(&*config.workdir.borrow());
    env(config, &mut cmd);
    cmd
}

pub fn env(config: &Config, cmd: &mut Command) {
    // Ensure PATH is prefixed with the rustup-exe directory
    let prev_path = env::var_os("PATH");
    let mut new_path = config.exedir.clone().into_os_string();
    if let Some(ref p) = prev_path {
        new_path.push(if cfg!(windows) { ";" } else { ":" });
        new_path.push(p);
    }
    cmd.env("PATH", new_path);
    cmd.env(
        "RUSTUP_HOME",
        config.rustupdir.to_string_lossy().to_string(),
    );
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

pub fn run<I, A>(config: &Config, name: &str, args: I, env: &[(&str, &str)]) -> SanitizedOutput
where
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    let mut cmd = cmd(config, name, args);
    for env in env {
        cmd.env(env.0, env.1);
    }

    println!("running {:?}", cmd);
    let lock = cmd_lock().read().unwrap();
    let out = cmd.output();
    drop(lock);
    let out = out.expect("failed to run test command");

    let output = SanitizedOutput {
        ok: out.status.success(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    };

    println!("status: {}", out.status);
    println!("----- stdout\n{}", output.stdout);
    println!("----- stderr\n{}", output.stderr);

    output
}

// Creates a mock dist server populated with some test data
fn create_mock_dist_server(path: &Path, s: Scenario) {
    let mut chans = Vec::new();

    let dates_count = match s {
        Scenario::SimpleV1 | Scenario::SimpleV2 | Scenario::MultiHost => 1,
        Scenario::Full | Scenario::ArchivesV1 | Scenario::ArchivesV2 | Scenario::Unavailable => 2,
    };

    if dates_count > 1 {
        let c1 = build_mock_channel(s, "nightly", "2015-01-01", "1.2.0", "hash-n-1", false);
        let c2 = build_mock_channel(s, "beta", "2015-01-01", "1.1.0", "hash-b-1", false);
        let c3 = build_mock_channel(s, "stable", "2015-01-01", "1.0.0", "hash-s-1", false);
        chans.extend(vec![c1, c2, c3]);
    }
    let c4 = if s == Scenario::Unavailable {
        build_mock_unavailable_channel("nightly", "2015-01-02", "1.3.0")
    } else {
        build_mock_channel(s, "nightly", "2015-01-02", "1.3.0", "hash-n-2", true)
    };
    let c5 = build_mock_channel(s, "beta", "2015-01-02", "1.2.0", "hash-b-2", false);
    let c6 = build_mock_channel(s, "stable", "2015-01-02", "1.1.0", "hash-s-2", false);
    chans.extend(vec![c4, c5, c6]);

    let vs = match s {
        Scenario::Full => vec![ManifestVersion::V1, ManifestVersion::V2],
        Scenario::SimpleV1 | Scenario::ArchivesV1 => vec![ManifestVersion::V1],
        Scenario::SimpleV2 | Scenario::ArchivesV2 | Scenario::MultiHost | Scenario::Unavailable => {
            vec![ManifestVersion::V2]
        }
    };

    MockDistServer {
        path: path.to_owned(),
        channels: chans,
    }
    .write(&vs, true);

    // Also create the manifests for stable releases by version
    if dates_count > 1 {
        let _ = hard_link(
            path.join("dist/2015-01-01/channel-rust-stable.toml"),
            path.join("dist/channel-rust-1.0.0.toml"),
        );
        let _ = hard_link(
            path.join("dist/2015-01-01/channel-rust-stable.toml.sha256"),
            path.join("dist/channel-rust-1.0.0.toml.sha256"),
        );
    }
    let _ = hard_link(
        path.join("dist/2015-01-02/channel-rust-stable.toml"),
        path.join("dist/channel-rust-1.1.0.toml"),
    );
    let _ = hard_link(
        path.join("dist/2015-01-02/channel-rust-stable.toml.sha256"),
        path.join("dist/channel-rust-1.1.0.toml.sha256"),
    );

    // Same for v1 manifests. These are just the installers.
    let host_triple = this_host_triple();
    if dates_count > 1 {
        hard_link(
            path.join(format!(
                "dist/2015-01-01/rust-stable-{}.tar.gz",
                host_triple
            )),
            path.join(format!("dist/rust-1.0.0-{}.tar.gz", host_triple)),
        )
        .unwrap();
        hard_link(
            path.join(format!(
                "dist/2015-01-01/rust-stable-{}.tar.gz.sha256",
                host_triple
            )),
            path.join(format!("dist/rust-1.0.0-{}.tar.gz.sha256", host_triple)),
        )
        .unwrap();
    }
    hard_link(
        path.join(format!(
            "dist/2015-01-02/rust-stable-{}.tar.gz",
            host_triple
        )),
        path.join(format!("dist/rust-1.1.0-{}.tar.gz", host_triple)),
    )
    .unwrap();
    hard_link(
        path.join(format!(
            "dist/2015-01-02/rust-stable-{}.tar.gz.sha256",
            host_triple
        )),
        path.join(format!("dist/rust-1.1.0-{}.tar.gz.sha256", host_triple)),
    )
    .unwrap();
}

fn build_mock_channel(
    s: Scenario,
    channel: &str,
    date: &str,
    version: &'static str,
    version_hash: &str,
    rename_rls: bool,
) -> MockChannel {
    // Build the mock installers
    let host_triple = this_host_triple();
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
    let mut all = vec![
        (
            "rust-std",
            vec![
                (std, host_triple.clone()),
                (cross_std1, CROSS_ARCH1.to_string()),
                (cross_std2, CROSS_ARCH2.to_string()),
            ],
        ),
        ("rustc", vec![(rustc, host_triple.clone())]),
        ("cargo", vec![(cargo, host_triple.clone())]),
    ];

    if rename_rls {
        let rls = build_mock_rls_installer(version, version_hash, true);
        all.push(("rls-preview", vec![(rls, host_triple.clone())]));
    } else {
        let rls = build_mock_rls_installer(version, version_hash, false);
        all.push(("rls", vec![(rls, host_triple.clone())]));
    }

    let more = vec![
        ("rust-docs", vec![(rust_docs, host_triple.clone())]),
        ("rust-src", vec![(rust_src, "*".to_string())]),
        ("rust-analysis", vec![(rust_analysis, "*".to_string())]),
        ("rust", vec![(rust, host_triple.clone())]),
    ];
    all.extend(more);

    if s == Scenario::MultiHost {
        let std = build_mock_std_installer(MULTI_ARCH1);
        let rustc = build_mock_rustc_installer(MULTI_ARCH1, version, version_hash);
        let cargo = build_mock_cargo_installer(version, version_hash);
        let rust_docs = build_mock_rust_doc_installer();
        let rust = build_combined_installer(&[&std, &rustc, &cargo, &rust_docs]);
        let cross_std1 = build_mock_cross_std_installer(CROSS_ARCH1, date);
        let cross_std2 = build_mock_cross_std_installer(CROSS_ARCH2, date);
        let rust_src = build_mock_rust_src_installer();

        let triple = MULTI_ARCH1.to_string();
        let more = vec![
            (
                "rust-std",
                vec![
                    (std, triple.clone()),
                    (cross_std1, CROSS_ARCH1.to_string()),
                    (cross_std2, CROSS_ARCH2.to_string()),
                ],
            ),
            ("rustc", vec![(rustc, triple.clone())]),
            ("cargo", vec![(cargo, triple.clone())]),
        ];
        all.extend(more);

        if rename_rls {
            let rls = build_mock_rls_installer(version, version_hash, true);
            all.push(("rls-preview", vec![(rls, triple.clone())]));
        } else {
            let rls = build_mock_rls_installer(version, version_hash, false);
            all.push(("rls", vec![(rls, triple.clone())]));
        }

        let more = vec![
            ("rust-docs", vec![(rust_docs, triple.clone())]),
            ("rust-src", vec![(rust_src, "*".to_string())]),
            ("rust", vec![(rust, triple.clone())]),
        ];

        all.extend(more);
    }

    let packages = all.into_iter().map(|(name, target_pkgs)| {
        let target_pkgs = target_pkgs
            .into_iter()
            .map(|(installer, triple)| MockTargetedPackage {
                target: triple,
                available: true,
                components: vec![],
                extensions: vec![],
                installer,
            });

        MockPackage {
            name,
            version,
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
            });
            target_pkg.components.push(MockComponent {
                name: "rustc".to_string(),
                target: target.to_string(),
            });
            target_pkg.components.push(MockComponent {
                name: "cargo".to_string(),
                target: target.to_string(),
            });
            target_pkg.components.push(MockComponent {
                name: "rust-docs".to_string(),
                target: target.to_string(),
            });
            if rename_rls {
                target_pkg.extensions.push(MockComponent {
                    name: "rls-preview".to_string(),
                    target: target.to_string(),
                });
            } else {
                target_pkg.extensions.push(MockComponent {
                    name: "rls".to_string(),
                    target: target.to_string(),
                });
            }
            target_pkg.extensions.push(MockComponent {
                name: "rust-std".to_string(),
                target: CROSS_ARCH1.to_string(),
            });
            target_pkg.extensions.push(MockComponent {
                name: "rust-std".to_string(),
                target: CROSS_ARCH2.to_string(),
            });
            target_pkg.extensions.push(MockComponent {
                name: "rust-src".to_string(),
                target: "*".to_string(),
            });
            target_pkg.extensions.push(MockComponent {
                name: "rust-analysis".to_string(),
                target: target.to_string(),
            });
        }
    }

    let mut renames = HashMap::new();
    if rename_rls {
        renames.insert("rls".to_owned(), "rls-preview".to_owned());
    }

    MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages,
        renames,
    }
}

fn build_mock_unavailable_channel(channel: &str, date: &str, version: &'static str) -> MockChannel {
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
            version,
            targets: vec![MockTargetedPackage {
                target: host_triple.clone(),
                available: false,
                components: vec![],
                extensions: vec![],
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

pub fn this_host_triple() -> String {
    if let Some(triple) = option_env!("RUSTUP_OVERRIDE_BUILD_TRIPLE") {
        triple.to_owned()
    } else {
        let arch = if cfg!(target_arch = "x86") {
            "i686"
        } else if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else {
            unimplemented!()
        };
        let os = if cfg!(target_os = "linux") {
            "unknown-linux"
        } else if cfg!(target_os = "windows") {
            "pc-windows"
        } else if cfg!(target_os = "macos") {
            "apple-darwin"
        } else {
            unimplemented!()
        };
        let env = if cfg!(target_env = "gnu") {
            Some("gnu")
        } else if cfg!(target_env = "msvc") {
            Some("msvc")
        } else {
            None
        };

        if let Some(env) = env {
            format!("{}-{}-{}", arch, os, env)
        } else {
            format!("{}-{}", arch, os)
        }
    }
}

fn build_mock_std_installer(trip: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: format!("rust-std-{}", trip),
            files: vec![MockFile::new(
                format!("lib/rustlib/{}/libstd.rlib", trip),
                b"",
            )],
        }],
    }
}

fn build_mock_cross_std_installer(target: &str, date: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: format!("rust-std-{}", target),
            files: vec![
                MockFile::new(format!("lib/rustlib/{}/lib/libstd.rlib", target), b""),
                MockFile::new(format!("lib/rustlib/{}/lib/{}", target, date), b""),
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
            files: mock_bin("cargo", version, &version_hash),
        }],
    }
}

fn build_mock_rls_installer(
    version: &str,
    version_hash: &str,
    preview: bool,
) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: if preview {
                "rls-preview".to_string()
            } else {
                "rls".to_string()
            },
            files: mock_bin("rls", version, version_hash),
        }],
    }
}

fn build_mock_rust_doc_installer() -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "rust-docs".to_string(),
            files: vec![MockFile::new("share/doc/rust/html/index.html", b"")],
        }],
    }
}

fn build_mock_rust_analysis_installer(trip: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: format!("rust-analysis-{}", trip),
            files: vec![MockFile::new(
                format!("lib/rustlib/{}/analysis/libfoo.json", trip),
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
            let dest_path = tempdir.path().join(&format!("out{}", EXE_SUFFIX));

            // Write the source
            let source = include_str!("mock_bin_src.rs");
            File::create(&source_path).and_then(|mut f| f.write_all(source.as_bytes())).unwrap();

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

            // If we're on unix this will remove debuginfo, otherwise we just ignore
            // the return result here
            if cfg!(unix) {
                drop(Command::new("strip").arg(&dest_path).status());
            }

            // Now load it into memory
            let mut f = File::open(dest_path).unwrap();
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).unwrap();

            Arc::new(buf)
        };
    }

    let name = format!("bin/{}{}", name, EXE_SUFFIX);
    vec![
        MockFile::new(format!("{}.version", name), version.as_bytes()),
        MockFile::new(format!("{}.version-hash", name), version_hash.as_bytes()),
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
    drop(fs::remove_file(b.as_ref()));
    fs::hard_link(a, b).map(|_| ())
}

pub fn copy_binary<A, B>(a: A, b: B) -> io::Result<()>
where
    A: AsRef<Path>,
    B: AsRef<Path>,
{
    eprintln!(
        "Copying from {} to {}",
        a.as_ref().display(),
        b.as_ref().display()
    );
    let _lock = cmd_lock().write().unwrap();
    drop(fs::remove_file(b.as_ref()));
    fs::copy(a, b).map(|_| ())
}
