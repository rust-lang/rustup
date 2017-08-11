//! A mock distribution server used by tests/cli-v1.rs and
//! tests/cli-v2.rs

use std::cell::RefCell;
use std::collections::HashMap;
use std::env::consts::EXE_SUFFIX;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::mem;
use std::path::{PathBuf, Path};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempdir::TempDir;
use {MockInstallerBuilder, MockCommand};
use dist::{MockDistServer, MockChannel, MockPackage,
           MockTargetedPackage, MockComponent, change_channel_date,
           ManifestVersion};
use url::Url;
use wait_timeout::ChildExt;

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
    Full, // Two dates, two manifests
    ArchivesV2, // Two dates, v2 manifests
    ArchivesV1, // Two dates, v1 manifests
    SimpleV2, // One date, v2 manifests
    SimpleV1, // One date, v1 manifests
    MultiHost, // One date, v2 manifests, MULTI_ARCH1 host
}

pub static CROSS_ARCH1: &'static str = "x86_64-unknown-linux-musl";
pub static CROSS_ARCH2: &'static str = "arm-linux-androideabi";

// Architecture for testing 'multi-host' installation.
// FIXME: Unfortunately the list of supported hosts is hard-coded,
// so we have to use the triple of a host we actually test on. That means
// that when we're testing on that host we can't test 'multi-host'.
pub static MULTI_ARCH1: &'static str = "i686-unknown-linux-gnu";

/// Run this to create the test environment containing rustup, and
/// a mock dist server.
pub fn setup(s: Scenario, f: &Fn(&mut Config)) {
    // Unset env variables that will break our testing
    env::remove_var("RUSTUP_TOOLCHAIN");
    env::remove_var("SHELL");
    env::remove_var("ZDOTDIR");

    let current_exe_path = env::current_exe().map(PathBuf::from).unwrap();
    let mut exe_dir = current_exe_path.parent().unwrap();
    if exe_dir.ends_with("deps") {
        exe_dir = exe_dir.parent().unwrap();
    }
    let test_dir = exe_dir.parent().unwrap().join("tests");
    fs::create_dir_all(&test_dir).unwrap();

    let exedir = TempDir::new_in(&test_dir, "rustup-exe").unwrap();
    let distdir = TempDir::new_in(&test_dir, "rustup-dist").unwrap();
    let rustupdir = TempDir::new_in(&test_dir, "rustup").unwrap();
    let customdir = TempDir::new_in(&test_dir, "rustup-custom").unwrap();
    let cargodir = TempDir::new_in(&test_dir, "rustup-cargo").unwrap();
    let homedir = TempDir::new_in(&test_dir, "rustup-home").unwrap();
    let emptydir = TempDir::new_in(&test_dir, "rustup-empty").unwrap();
    let workdir = TempDir::new_in(&test_dir, "rustup-workdir").unwrap();

    // The uninstall process on windows involves using the directory above
    // CARGO_HOME, so make sure it's a subdir of our tempdir
    let cargodir = cargodir.path().join("ch");
    fs::create_dir(&cargodir).unwrap();

    let mut config = Config {
        exedir: exedir.path().to_owned(),
        distdir: distdir.path().to_owned(),
        rustupdir: rustupdir.path().to_owned(),
        customdir: customdir.path().to_owned(),
        cargodir: cargodir,
        homedir: homedir.path().to_owned(),
        emptydir: emptydir.path().to_owned(),
        workdir: RefCell::new(workdir.path().to_owned()),
    };

    create_mock_dist_server(&config.distdir, s);

    let ref build_path = exe_dir.join(format!("rustup-init{}", EXE_SUFFIX));

    let ref rustup_path = config.exedir.join(format!("rustup{}", EXE_SUFFIX));
    let setup_path = config.exedir.join(format!("rustup-init{}", EXE_SUFFIX));
    let rustc_path = config.exedir.join(format!("rustc{}", EXE_SUFFIX));
    let cargo_path = config.exedir.join(format!("cargo{}", EXE_SUFFIX));
    let rls_path = config.exedir.join(format!("rls{}", EXE_SUFFIX));

    hard_link(&build_path, &rustup_path).unwrap();
    hard_link(rustup_path, setup_path).unwrap();
    hard_link(rustup_path, rustc_path).unwrap();
    hard_link(rustup_path, cargo_path).unwrap();
    hard_link(rustup_path, rls_path).unwrap();

    // Make sure the host triple matches the build triple. Otherwise testing a 32-bit build of
    // rustup on a 64-bit machine will fail, because the tests do not have the host detection
    // functionality built in.
    run(&config, "rustup", &["set", "host", &this_host_triple()], &[]);

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
        where F: FnMut()
    {
        self._change_dir(path, &mut f)
    }

    fn _change_dir(&self, path: &Path, f: &mut FnMut()) {
        let prev = mem::replace(&mut *self.workdir.borrow_mut(), path.to_owned());
        f();
        *self.workdir.borrow_mut() = prev;
    }
}

/// Change the current distribution manifest to a particular date
pub fn set_current_dist_date(config: &Config, date: &str) {
    let ref url = Url::from_file_path(&config.distdir).unwrap();
    for channel in &["nightly", "beta", "stable"] {
        change_channel_date(url, channel, date);
    }
}

pub fn expect_ok(config: &Config, args: &[&str]) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok {
        print_command(args, &out);
        println!("expected.ok: {}", true);
        panic!();
    }
}

pub fn expect_err(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if out.ok || !out.stderr.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: {}", false);
        print_indented("expected.stderr.contains", expected);
        panic!();
    }
}

pub fn expect_stdout_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || !out.stdout.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: {}", true);
        print_indented("expected.stdout.contains", expected);
        panic!();
    }
}

pub fn expect_not_stdout_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || out.stdout.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: {}", true);
        print_indented("expected.stdout.does_not_contain", expected);
        panic!();
    }
}

pub fn expect_stderr_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || !out.stderr.contains(expected) {
        print_command(args, &out);
        println!("expected.ok: {}", true);
        print_indented("expected.stderr.contains", expected);
        panic!();
    }
}

pub fn expect_ok_ex(config: &Config, args: &[&str],
                    stdout: &str, stderr: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if !out.ok || out.stdout != stdout || out.stderr != stderr {
        print_command(args, &out);
        println!("expected.ok: {}", true);
        print_indented("expected.stdout", stdout);
        print_indented("expected.stderr", stderr);
        panic!();
    }
}

pub fn expect_err_ex(config: &Config, args: &[&str],
                     stdout: &str, stderr: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    if out.ok || out.stdout != stdout || out.stderr != stderr {
        print_command(args, &out);
        println!("expected.ok: {}", false);
        print_indented("expected.stdout", stdout);
        print_indented("expected.stderr", stderr);
        panic!();
    }
}

pub fn expect_timeout_ok(config: &Config, timeout: Duration, args: &[&str]) {
    let mut child = cmd(config, args[0], &args[1..])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn().unwrap();

    match child.wait_timeout(timeout).unwrap() {
        Some(status) => {
            assert!(status.success(), "not ok {:?}", args);
        }
        None => {
            // child hasn't exited yet
            child.kill().unwrap();
            panic!("command timed out: {:?}", args);
        }
    }
}

fn print_command(args: &[&str], out: &SanitizedOutput) {
    print!("\n>");
    for arg in args {
        if arg.contains(" ") {
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
    println!("{}:\n    {}", heading, text.replace("\n", "\n    "));
}

#[derive(Debug)]
pub struct SanitizedOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn cmd(config: &Config, name: &str, args: &[&str]) -> Command {
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
    cmd.env("RUSTUP_HOME", config.rustupdir.to_string_lossy().to_string());
    cmd.env("RUSTUP_DIST_SERVER", format!("file://{}", config.distdir.to_string_lossy()));
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

pub fn run(config: &Config, name: &str, args: &[&str], env: &[(&str, &str)]) -> SanitizedOutput {
    let mut cmd = cmd(config, name, args);
    for env in env {
        cmd.env(env.0, env.1);
    }
    let out = cmd.output().expect("failed to run test command");

    SanitizedOutput {
        ok: out.status.success(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

// Creates a mock dist server populated with some test data
fn create_mock_dist_server(path: &Path, s: Scenario) {
    let mut chans = Vec::new();
    if s == Scenario::Full || s == Scenario::ArchivesV1 || s == Scenario::ArchivesV2 {
        let c1 = build_mock_channel(s, "nightly", "2015-01-01", "1.2.0", "hash-n-1");
        let c2 = build_mock_channel(s, "beta", "2015-01-01", "1.1.0", "hash-b-1");
        let c3 = build_mock_channel(s, "stable", "2015-01-01", "1.0.0", "hash-s-1");
        chans.extend(vec![c1, c2, c3]);
    }
    let c4 = build_mock_channel(s, "nightly", "2015-01-02", "1.3.0", "hash-n-2");
    let c5 = build_mock_channel(s, "beta", "2015-01-02", "1.2.0", "hash-b-2");
    let c6 = build_mock_channel(s, "stable", "2015-01-02", "1.1.0", "hash-s-2");
    chans.extend(vec![c4, c5, c6]);

    let ref vs = match s {
        Scenario::Full => vec![ManifestVersion::V1, ManifestVersion::V2],
        Scenario::SimpleV1 | Scenario::ArchivesV1 => vec![ManifestVersion::V1],
        Scenario::SimpleV2 | Scenario::ArchivesV2 |
        Scenario::MultiHost => vec![ManifestVersion::V2],
    };

    MockDistServer {
        path: path.to_owned(),
        channels: chans,
    }.write(vs, true);

    // Also create the manifests for stable releases by version
    if s == Scenario::Full || s == Scenario::ArchivesV1 || s == Scenario::ArchivesV2 {
        let _ = hard_link(path.join("dist/2015-01-01/channel-rust-stable.toml"),
                          path.join("dist/channel-rust-1.0.0.toml"));
        let _ = hard_link(path.join("dist/2015-01-01/channel-rust-stable.toml.sha256"),
                          path.join("dist/channel-rust-1.0.0.toml.sha256"));
    }
    let _ = hard_link(path.join("dist/2015-01-02/channel-rust-stable.toml"),
                      path.join("dist/channel-rust-1.1.0.toml"));
    let _ = hard_link(path.join("dist/2015-01-02/channel-rust-stable.toml.sha256"),
                      path.join("dist/channel-rust-1.1.0.toml.sha256"));

    // Same for v1 manifests. These are just the installers.
    let host_triple = this_host_triple();
    if s == Scenario::Full || s == Scenario::ArchivesV1 || s == Scenario::ArchivesV2 {
        hard_link(path.join(format!("dist/2015-01-01/rust-stable-{}.tar.gz", host_triple)),
                  path.join(format!("dist/rust-1.0.0-{}.tar.gz", host_triple))).unwrap();
        hard_link(path.join(format!("dist/2015-01-01/rust-stable-{}.tar.gz.sha256", host_triple)),
                  path.join(format!("dist/rust-1.0.0-{}.tar.gz.sha256", host_triple))).unwrap();
    }
    hard_link(path.join(format!("dist/2015-01-02/rust-stable-{}.tar.gz", host_triple)),
              path.join(format!("dist/rust-1.1.0-{}.tar.gz", host_triple))).unwrap();
    hard_link(path.join(format!("dist/2015-01-02/rust-stable-{}.tar.gz.sha256", host_triple)),
              path.join(format!("dist/rust-1.1.0-{}.tar.gz.sha256", host_triple))).unwrap();
}

fn build_mock_channel(s: Scenario, channel: &str, date: &str,
                      version: &'static str, version_hash: &str) -> MockChannel {
    // Build the mock installers
    let ref host_triple = this_host_triple();
    let std = build_mock_std_installer(host_triple);
    let rustc = build_mock_rustc_installer(host_triple, version, version_hash);
    let cargo = build_mock_cargo_installer(version, version_hash);
    let rls = build_mock_rls_installer(version, version_hash);
    let rust_docs = build_mock_rust_doc_installer();
    let rust = build_combined_installer(&[&std, &rustc, &cargo, &rust_docs]);
    let cross_std1 = build_mock_cross_std_installer(CROSS_ARCH1, date);
    let cross_std2 = build_mock_cross_std_installer(CROSS_ARCH2, date);
    let rust_src = build_mock_rust_src_installer();
    let rust_analysis = build_mock_rust_analysis_installer(host_triple);

    // Convert the mock installers to mock package definitions for the
    // mock dist server
    let mut all = vec![("rust-std", vec![(std, host_triple.clone()),
                                     (cross_std1, CROSS_ARCH1.to_string()),
                                     (cross_std2, CROSS_ARCH2.to_string())]),
                   ("rustc", vec![(rustc, host_triple.clone())]),
                   ("cargo", vec![(cargo, host_triple.clone())]),
                   ("rls", vec![(rls, host_triple.clone())]),
                   ("rust-docs", vec![(rust_docs, host_triple.clone())]),
                   ("rust-src", vec![(rust_src, "*".to_string())]),
                   ("rust-analysis", vec![(rust_analysis, "*".to_string())]),
                   ("rust", vec![(rust, host_triple.clone())])];

    if s == Scenario::MultiHost {
        let std = build_mock_std_installer(MULTI_ARCH1);
        let rustc = build_mock_rustc_installer(MULTI_ARCH1, version, version_hash);
        let cargo = build_mock_cargo_installer(version, version_hash);
        let rls = build_mock_rls_installer(version, version_hash);
        let rust_docs = build_mock_rust_doc_installer();
        let rust = build_combined_installer(&[&std, &rustc, &cargo, &rust_docs]);
        let cross_std1 = build_mock_cross_std_installer(CROSS_ARCH1, date);
        let cross_std2 = build_mock_cross_std_installer(CROSS_ARCH2, date);
        let rust_src = build_mock_rust_src_installer();

        let triple = MULTI_ARCH1.to_string();
        let more = vec![("rust-std", vec![(std, triple.clone()),
                                     (cross_std1, CROSS_ARCH1.to_string()),
                                     (cross_std2, CROSS_ARCH2.to_string())]),
                        ("rustc", vec![(rustc, triple.clone())]),
                        ("cargo", vec![(cargo, triple.clone())]),
                        ("rls", vec![(rls, triple.clone())]),
                        ("rust-docs", vec![(rust_docs, triple.clone())]),
                        ("rust-src", vec![(rust_src, "*".to_string())]),
                        ("rust", vec![(rust, triple.clone())])];

        all.extend(more);
    }

    let packages = all.into_iter().map(|(name, target_pkgs)| {
        let target_pkgs = target_pkgs.into_iter().map(|(installer, triple)| {
            MockTargetedPackage {
                target: triple,
                available: true,
                components: vec![],
                extensions: vec![],
                installer: installer,
            }
        });

        MockPackage {
            name: name,
            version: version,
            targets: target_pkgs.collect()
        }
    });
    let mut packages: Vec<_> = packages.collect();

    // Add subcomponents of the rust package
    {
        let rust_pkg = packages.last_mut().unwrap();
        for target_pkg in rust_pkg.targets.iter_mut() {
            let ref target = target_pkg.target;
            target_pkg.components.push(MockComponent {
                name: "rust-std".to_string(),
                target: target.to_string()
            });
            target_pkg.components.push(MockComponent {
                name: "rustc".to_string(),
                target: target.to_string()
            });
            target_pkg.components.push(MockComponent {
                name: "cargo".to_string(),
                target: target.to_string()
            });
            target_pkg.components.push(MockComponent {
                name: "rust-docs".to_string(),
                target: target.to_string()
            });
            target_pkg.extensions.push(MockComponent {
                name: "rls".to_string(),
                target: target.to_string()
            });
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

    MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages: packages,
    }
}

pub fn this_host_triple() -> String {
    if let Some(triple) = option_env!("RUSTUP_OVERRIDE_BUILD_TRIPLE") {
        triple.to_owned()
    } else {
        let arch = if cfg!(target_arch = "x86") { "i686" }
        else if cfg!(target_arch = "x86_64") { "x86_64" }
        else { unimplemented!() };
        let os = if cfg!(target_os = "linux") { "unknown-linux" }
        else if cfg!(target_os = "windows") { "pc-windows" }
        else if cfg!(target_os = "macos") { "apple-darwin" }
        else { unimplemented!() };
        let env = if cfg!(target_env = "gnu") { Some("gnu") }
        else if cfg!(target_env = "msvc") { Some("msvc") }
        else { None };

        if let Some(env) = env {
            format!("{}-{}-{}", arch, os, env)
        } else {
            format!("{}-{}", arch, os)
        }
    }
}

fn build_mock_std_installer(trip: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![
            (format!("rust-std-{}", trip.clone()),
             vec![MockCommand::File(format!("lib/rustlib/{}/libstd.rlib", trip))],
             vec![(format!("lib/rustlib/{}/libstd.rlib", trip), empty(), false)])
            ]
    }
}

fn build_mock_cross_std_installer(target: &str, date: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![
            (format!("rust-std-{}", target.clone()),
             vec![MockCommand::File(format!("lib/rustlib/{}/lib/libstd.rlib", target)),
                  MockCommand::File(format!("lib/rustlib/{}/lib/{}", target, date))],
             vec![(format!("lib/rustlib/{}/lib/libstd.rlib", target), empty(), false),
                  (format!("lib/rustlib/{}/lib/{}", target, date), empty(), false)])
            ]
    }
}

fn build_mock_rustc_installer(target: &str, version: &str, version_hash_: &str) -> MockInstallerBuilder {
    // For cross-host rustc's modify the version_hash so they can be identified from
    // test cases.
    let this_host = this_host_triple();
    let version_hash;
    if this_host != target {
        version_hash = format!("xxxx-{}", &version_hash_[5..]);
    } else {
        version_hash = version_hash_.to_string();
    }

    let rustc = format!("bin/rustc{}", EXE_SUFFIX);
    MockInstallerBuilder {
        components: vec![
            ("rustc".to_string(),
             vec![MockCommand::File(rustc.clone())],
             vec![(rustc, mock_bin("rustc", version, &version_hash), false)])
                ]
    }
}

fn build_mock_cargo_installer(version: &str, version_hash: &str) -> MockInstallerBuilder {
    let cargo = format!("bin/cargo{}", EXE_SUFFIX);
    MockInstallerBuilder {
        components: vec![
            ("cargo".to_string(),
             vec![MockCommand::File(cargo.clone())],
             vec![(cargo, mock_bin("cargo", version, version_hash), false)])
                ]
    }
}

fn build_mock_rls_installer(version: &str, version_hash: &str) -> MockInstallerBuilder {
    let cargo = format!("bin/rls{}", EXE_SUFFIX);
    MockInstallerBuilder {
        components: vec![
            ("rls".to_string(),
             vec![MockCommand::File(cargo.clone())],
             vec![(cargo, mock_bin("rls", version, version_hash), false)])
                ]
    }
}

fn build_mock_rust_doc_installer() -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![
            ("rust-docs".to_string(),
             vec![MockCommand::File("share/doc/rust/html/index.html".to_string())],
             vec![("share/doc/rust/html/index.html".to_string(), empty(), false)])
                ]
    }
}

fn build_mock_rust_analysis_installer(trip: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![
            (format!("rust-analysis-{}", trip),
             vec![MockCommand::File(format!("lib/rustlib/{}/analysis/libfoo.json", trip))],
             vec![(format!("lib/rustlib/{}/analysis/libfoo.json", trip), empty(), false)])
                ]
    }
}

fn build_mock_rust_src_installer() -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![
            ("rust-src".to_string(),
             vec![MockCommand::File("lib/rustlib/src/rust-src/foo.rs".to_string())],
             vec![("lib/rustlib/src/rust-src/foo.rs".to_string(), empty(), false)])
                ]
    }
}

fn build_combined_installer(components: &[&MockInstallerBuilder]) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: components.iter().flat_map(|m| m.components.clone()).collect()
    }
}

/// This is going to run the compiler to create an executable that
/// prints some version information. These binaries are stuffed into
/// the mock installers so we have executables for rustup to run.
///
/// This does a really crazy thing. Because we need to generate a lot
/// of these, and running rustc is slow, it does it once, stuffs the
/// bin into memory, then does a string replacement of the version
/// information it needs to report to create subsequent bins.
fn mock_bin(_name: &str, version: &str, version_hash: &str) -> Arc<Vec<u8>> {
    lazy_static! {
        static ref MOCK_BIN_TEMPLATE: Mutex<HashMap<(String, String), Arc<Vec<u8>>>> =
            Mutex::new(HashMap::new());
    }

    let key = (version.to_string(), version_hash.to_string());
    let map = MOCK_BIN_TEMPLATE.lock().unwrap();
    if let Some(ret) = map.get(&key) {
        return ret.clone()
    }
    drop(map);

    // Create a temp directory to hold the source and the output
    let ref tempdir = TempDir::new("rustup").unwrap();
    let ref source_path = tempdir.path().join("in.rs");
    let ref dest_path = tempdir.path().join(&format!("out{}", EXE_SUFFIX));

    // Write the source
    let source = include_str!("mock_bin_src.rs");
    File::create(source_path).and_then(|mut f| f.write_all(source.as_bytes())).unwrap();

    // Create the executable
    let status = Command::new("rustc")
        .arg(&source_path)
        .arg("-C").arg("panic=abort")
        .arg("-O")
        .arg("-o").arg(&dest_path)
        .env("EXAMPLE_VERSION", version)
        .env("EXAMPLE_VERSION_HASH", version_hash)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(dest_path.exists());

    // If we're on unix this will remove debuginfo, otherwise we just ignore
    // the return result here
    drop(Command::new("strip").arg(&dest_path).status());

    // Now load it into memory
    let mut f = File::open(dest_path).unwrap();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();

    let buf = Arc::new(buf);
    MOCK_BIN_TEMPLATE.lock().unwrap().insert(key, buf.clone());
    return buf
}

// These are toolchains for installation with --link-local and --copy-local
fn create_custom_toolchains(customdir: &Path) {
    let ref dir = customdir.join("custom-1/bin");
    fs::create_dir_all(dir).unwrap();
    let ref libdir = customdir.join("custom-1/lib");
    fs::create_dir_all(libdir).unwrap();
    let rustc = mock_bin("rustc", "1.0.0", "hash-c-1");
    let ref path = customdir.join(format!("custom-1/bin/rustc{}", EXE_SUFFIX));
    let mut file = File::create(path).unwrap();
    file.write_all(&rustc).unwrap();
    make_exe(dir, path);

    let ref dir = customdir.join("custom-2/bin");
    fs::create_dir_all(dir).unwrap();
    let ref libdir = customdir.join("custom-2/lib");
    fs::create_dir_all(libdir).unwrap();
    let rustc = mock_bin("rustc", "1.0.0", "hash-c-2");
    let ref path = customdir.join(format!("custom-2/bin/rustc{}", EXE_SUFFIX));
    let mut file = File::create(path).unwrap();
    file.write_all(&rustc).unwrap();
    make_exe(dir, path);

    #[cfg(unix)]
    fn make_exe(dir: &Path, bin: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dir, perms).unwrap();
        let mut perms = fs::metadata(bin).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(bin, perms).unwrap();
    }

    #[cfg(windows)]
    fn make_exe(_: &Path, _: &Path) { }
}

fn empty() -> Arc<Vec<u8>> {
    Arc::new(Vec::new())
}

pub fn hard_link<A, B>(a: A, b: B) -> io::Result<()>
    where A: AsRef<Path>,
          B: AsRef<Path>,
{
    drop(fs::remove_file(b.as_ref()));
    fs::hard_link(a, b).map(|_| ())
}
