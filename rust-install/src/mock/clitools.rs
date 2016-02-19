//! A mock distribution server used by tests/cli-v1.rs and
//! tests/cli-v2.rs

use std::path::{PathBuf, Path};
use std::env;
use std::process::Command;
use std::env::consts::EXE_SUFFIX;
use std::fs::{self, File};
use std::io::Read;
use std::sync::Mutex;
use tempdir::TempDir;
use mock::{MockInstallerBuilder, MockCommand};
use mock::dist::{MockDistServer, MockChannel, MockPackage,
                 MockTargettedPackage, MockComponent, change_channel_date,
                 ManifestVersion};
use dist::ToolchainDesc;
use utils;
use hyper::Url;
use scopeguard;

/// The configuration used by the tests in this module
pub struct Config {
    /// Where we put the multirust / rustc / cargo bins
    pub exedir: TempDir,
    /// The distribution server
    pub distdir: TempDir,
    /// MULTIRUST_HOME
    pub homedir: TempDir,
}

/// Run this to create the test environment containing multirust, and
/// a mock dist server.
pub fn setup(vs: &[ManifestVersion], f: &Fn(&Config)) {
    // Unset env variables that will break our testing
    env::remove_var("MULTIRUST_TOOLCHAIN");

    let ref config = Config {
        exedir: TempDir::new("multirust").unwrap(),
        distdir: TempDir::new("multirust").unwrap(),
        homedir: TempDir::new("multirust").unwrap(),
    };

    create_mock_dist_server(&config.distdir.path(), vs);

    let current_exe_path = env::current_exe().map(PathBuf::from).unwrap();
    let exe_dir = current_exe_path.parent().unwrap();
    let ref multirust_build_path = exe_dir.join(format!("multirust-rs{}", EXE_SUFFIX));

    let multirust_path = config.exedir.path().join(format!("multirust{}", EXE_SUFFIX));
    let rustc_path = config.exedir.path().join(format!("rustc{}", EXE_SUFFIX));
    let rustdoc_path = config.exedir.path().join(format!("rustdoc{}", EXE_SUFFIX));
    let cargo_path = config.exedir.path().join(format!("cargo{}", EXE_SUFFIX));

    fs::copy(multirust_build_path, multirust_path).unwrap();
    fs::copy(multirust_build_path, rustc_path).unwrap();
    fs::copy(multirust_build_path, rustdoc_path).unwrap();
    fs::copy(multirust_build_path, cargo_path).unwrap();

    f(config);
}

/// Change the current distribution manifest to a particular date
pub fn set_current_dist_date(config: &Config, date: &str) {
    let ref url = Url::from_file_path(config.distdir.path()).unwrap();
    for channel in &["nightly", "beta", "stable"] {
        change_channel_date(url, channel, date);
    }
}

pub fn expect_ok(config: &Config, args: &[&str]) {
    expect_stdout_ok(config, args, "");
}

pub fn expect_err(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    println!("out.ok: {}", out.ok);
    println!("out.stdout:\n\n{}", out.stdout);
    println!("out.stderr:\n\n{}", out.stderr);
    println!("expected: {}", expected);
    let args = format!("{:?}", args);
    assert!(!out.ok, args);
    assert!(out.stderr.contains(expected), args);
}

pub fn expect_stdout_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    println!("out.ok: {}", out.ok);
    println!("out.stdout:\n\n{}", out.stdout);
    println!("out.stderr:\n\n{}", out.stderr);
    println!("expected: {}", expected);
    let args = format!("{:?}", args);
    assert!(out.ok, args);
    assert!(out.stdout.contains(expected), args);
}

pub fn expect_stderr_ok(config: &Config, args: &[&str], expected: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    println!("out.ok: {}", out.ok);
    println!("out.stdout:\n\n{}", out.stdout);
    println!("out.stderr:\n\n{}", out.stderr);
    println!("expected: {}", expected);
    let args = format!("{:?}", args);
    assert!(out.ok, args);
    assert!(out.stderr.contains(expected), args);
}

#[derive(Debug)]
pub struct SanitizedOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(config: &Config, name: &str, args: &[&str], env: &[(&str, &str)]) -> SanitizedOutput {
    let exe_path = config.exedir.path().join(format!("{}{}", name, EXE_SUFFIX));
    let mut cmd = Command::new(exe_path);
    cmd.args(args);
    cmd.env("MULTIRUST_HOME", config.homedir.path().to_string_lossy().to_string());
    cmd.env("MULTIRUST_DIST_ROOT", format!("file://{}", config.distdir.path().join("dist").to_string_lossy()));
    for env in env {
        cmd.env(env.0, env.1);
    }
    let out = cmd.output().unwrap();

    SanitizedOutput {
        ok: out.status.success(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

/// Holds a process wide mutex while working in another directory.
/// Use this any time you are invoking 'multurust override', since
/// that captures the current directory from the environment, and
/// will race without the mutex.
pub fn change_dir(path: &Path, f: &Fn()) {
    lazy_static! {
        static ref LOCK: Mutex<()> = Mutex::new(());
    }
    let _g = LOCK.lock();
    let cwd = env::current_dir().unwrap();
    env::set_current_dir(path).unwrap();
    let _g = scopeguard::guard((), move |_| env::set_current_dir(&cwd).unwrap());
    f();
}

// Creates a mock dist server populated with some test data
fn create_mock_dist_server(path: &Path, vs: &[ManifestVersion]) {
    let c1 = build_mock_channel("nightly", "2015-01-01", "1.2.0", "hash-n-1");
    let c2 = build_mock_channel("beta", "2015-01-01", "1.1.0", "hash-b-1");
    let c3 = build_mock_channel("stable", "2015-01-01", "1.0.0", "hash-s-1");
    let c4 = build_mock_channel("nightly", "2015-01-02", "1.3.0", "hash-n-2");
    let c5 = build_mock_channel("beta", "2015-01-02", "1.2.0", "hash-b-2");
    let c6 = build_mock_channel("stable", "2015-01-02", "1.1.0", "hash-s-2");

    MockDistServer {
        path: path.to_owned(),
        channels: vec![c1, c2, c3, c4, c5, c6],
    }.write(vs);
}

static CROSS_ARCH1: &'static str = "x86_64-unknown-linux-musl";
static CROSS_ARCH2: &'static str = "arm-linux-androideabi";

fn build_mock_channel(channel: &str, date: &str,
                      version: &'static str, version_hash: &str) -> MockChannel {
    // Build the mock installers
    let std = build_mock_std_installer(channel);
    let cross_std1 = build_mock_cross_std_installer(channel, CROSS_ARCH1, date);
    let cross_std2 = build_mock_cross_std_installer(channel, CROSS_ARCH2, date);
    let rustc = build_mock_rustc_installer(version, version_hash);
    let cargo = build_mock_cargo_installer(version, version_hash);
    let rust_docs = build_mock_rust_doc_installer(channel);
    let rust = build_combined_installer(&[&std, &rustc, &cargo, &rust_docs]);

    let host_triple = this_host_triple(channel);

    // Convert the mock installers to mock package definitions for the
    // mock dist server
    let all = vec![("rust-std", vec![(std, host_triple.clone()),
                                     (cross_std1, CROSS_ARCH1.to_string()),
                                     (cross_std2, CROSS_ARCH2.to_string())]),
                   ("rustc", vec![(rustc, host_triple.clone())]),
                   ("cargo", vec![(cargo, host_triple.clone())]),
                   ("rust-docs", vec![(rust_docs, host_triple.clone())]),
                   ("rust", vec![(rust, host_triple.clone())])];

    let packages = all.into_iter().map(|(name, target_pkgs)| {
        let target_pkgs = target_pkgs.into_iter().map(|(installer, triple)| {
            MockTargettedPackage {
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
        let target_pkg = rust_pkg.targets.first_mut().unwrap();
        target_pkg.components.push(MockComponent {
            name: "rust-std",
            target: host_triple.clone()
        });
        target_pkg.components.push(MockComponent {
            name: "rustc",
            target: host_triple.clone()
        });
        target_pkg.components.push(MockComponent {
            name: "cargo",
            target: host_triple.clone()
        });
        target_pkg.components.push(MockComponent {
            name: "rust-docs",
            target: host_triple.clone()
        });
        target_pkg.extensions.push(MockComponent {
            name: "rust-std",
            target: CROSS_ARCH1.to_string(),
        });
        target_pkg.extensions.push(MockComponent {
            name: "rust-std",
            target: CROSS_ARCH2.to_string(),
        });
    }

    MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages: packages,
    }
}

fn this_host_triple(channel: &str) -> String {
    ToolchainDesc::from_str(channel).and_then(|t| t.target_triple()).unwrap()
}

fn build_mock_std_installer(channel: &str) -> MockInstallerBuilder {
    let trip = this_host_triple(channel);
    MockInstallerBuilder {
        components: vec![
            (format!("rust-std-{}", trip.clone()),
             vec![MockCommand::File(format!("lib/rustlib/{}/libstd.rlib", trip))],
             vec![(format!("lib/rustlib/{}/libstd.rlib", trip), "".into())])
            ]
    }
}

fn build_mock_cross_std_installer(_channel: &str, target: &str, date: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![
            (format!("rust-std-{}", target.clone()),
             vec![MockCommand::File(format!("lib/rustlib/{}/lib/libstd.rlib", target)),
                  MockCommand::File(format!("lib/rustlib/{}/lib/{}", target, date))],
             vec![(format!("lib/rustlib/{}/lib/libstd.rlib", target), "".into()),
                  (format!("lib/rustlib/{}/lib/{}", target, date), "".into())])
            ]
    }
}

fn build_mock_rustc_installer(version: &str, version_hash: &str) -> MockInstallerBuilder {
    let rustc = format!("bin/rustc{}", EXE_SUFFIX);
    let rustdoc = format!("bin/rustdoc{}", EXE_SUFFIX);
    MockInstallerBuilder {
        components: vec![
            ("rustc".to_string(),
             vec![MockCommand::File(rustc.clone()),
                  MockCommand::File(rustdoc.clone())],
             vec![(rustc, mock_bin("rustc", version, version_hash)),
                  (rustdoc, mock_bin("rustdoc", version, version_hash))])
                ]
    }
}

fn build_mock_cargo_installer(version: &str, version_hash: &str) -> MockInstallerBuilder {
    let cargo = format!("bin/cargo{}", EXE_SUFFIX);
    MockInstallerBuilder {
        components: vec![
            ("cargo".to_string(),
             vec![MockCommand::File(cargo.clone())],
             vec![(cargo, mock_bin("cargo", version, version_hash))])
                ]
    }
}

fn build_mock_rust_doc_installer(_channel: &str) -> MockInstallerBuilder {
    MockInstallerBuilder {
        components: vec![
            ("rust-docs".to_string(),
             vec![MockCommand::File("share/doc/rust/html/index.html".to_string())],
             vec![("share/doc/rust/html/index.html".to_string(), "".into())])
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
/// the mock installers so we have executables for multirust to run.
///
/// This does a really crazy thing. Because we need to generate a lot
/// of these, and running rustc is slow, it does it once, stuffs the
/// bin into memory, then does a string replacement of the version
/// information it needs to report to create subsequent bins.
fn mock_bin(_name: &str, version: &str, version_hash: &str) -> Vec<u8> {

    // This is what version and version_hash look like. We'll use
    // these as template values in our template binary, to be replaced
    // with the actual version and veresion_hash values in the final
    // binary.
    static EXAMPLE_VERSION: &'static str = "1.1.0";
    static EXAMPLE_VERSION_HASH: &'static str = "hash-s-2";
    assert_eq!(version.len(), EXAMPLE_VERSION.len());
    assert_eq!(version_hash.len(), EXAMPLE_VERSION_HASH.len());

    // If this is the first time, create the template binary
    lazy_static! {
        static ref MOCK_BIN_TEMPLATE: Vec<u8> = make_mock_bin_template();
    }

    fn make_mock_bin_template() -> Vec<u8> {
        // Create a temp directory to hold the source and the output
        let ref tempdir = TempDir::new("multirust").unwrap();
        let ref source_path = tempdir.path().join("in.rs");
        let ref dest_path = tempdir.path().join(&format!("out{}", EXE_SUFFIX));

        // Write the source
        let ref source = format!(r#"
            fn main() {{
                println!("{} ({})");
            }}
            "#, EXAMPLE_VERSION, EXAMPLE_VERSION_HASH);
        utils::raw::write_file(source_path, source).unwrap();

        // Create the executable
        let status = Command::new("rustc").arg(&*source_path.to_string_lossy())
            .arg("-o").arg(&*dest_path.to_string_lossy())
            .status().unwrap();
        assert!(status.success());
        assert!(dest_path.exists());

        // Now load it into memory
        let mut f = File::open(dest_path).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        buf
    }

    let mut bin = MOCK_BIN_TEMPLATE.clone();

    // Replace the version strings
    {
        let version_index = bin
            .windows(EXAMPLE_VERSION.len())
            .enumerate()
            .find(|&(_, slice)| slice == EXAMPLE_VERSION.as_bytes())
            .map(|(i, _)| i)
            .unwrap();
        let version_slice = &mut bin[version_index..(version_index + EXAMPLE_VERSION.len())];
        version_slice.clone_from_slice(version.as_bytes());
    }

    {
        let version_hash_index = bin
            .windows(EXAMPLE_VERSION_HASH.len())
            .enumerate()
            .find(|&(_, slice)| slice == EXAMPLE_VERSION_HASH.as_bytes())
            .map(|(i, _)| i)
            .unwrap();
        let version_hash_slice = &mut bin[version_hash_index..(version_hash_index + EXAMPLE_VERSION_HASH.len())];
        version_hash_slice.clone_from_slice(version_hash.as_bytes());
    }

    bin
}

