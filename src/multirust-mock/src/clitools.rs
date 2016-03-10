//! A mock distribution server used by tests/cli-v1.rs and
//! tests/cli-v2.rs

use std::path::{PathBuf, Path};
use std::env;
use std::process::Command;
use std::env::consts::EXE_SUFFIX;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::sync::Mutex;
use tempdir::TempDir;
use {MockInstallerBuilder, MockCommand};
use dist::{MockDistServer, MockChannel, MockPackage,
           MockTargettedPackage, MockComponent, change_channel_date,
           ManifestVersion};
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
    /// Custom toolchains
    pub customdir: TempDir,
}

// Describes all the features of the mock dist server.
// Building the mock server is slow, so use simple scenario when possible.
#[derive(PartialEq, Copy, Clone)]
pub enum Scenario {
    Full, // Two dates, two manifests, with cargo
    ArchivesV2, // Two dates, no cargo, v2 manifests
    ArchivesV1, // Two dates, no cargo, v2 manifests
    SimpleV2, // One date, no cargo, v2 manifests
    SimpleV1, // One date, no cargo, v1 manifests
}

/// Run this to create the test environment containing multirust, and
/// a mock dist server.
pub fn setup(s: Scenario, f: &Fn(&Config)) {
    // Unset env variables that will break our testing
    env::remove_var("MULTIRUST_TOOLCHAIN");

    let ref config = Config {
        exedir: TempDir::new("multirust").unwrap(),
        distdir: TempDir::new("multirust").unwrap(),
        homedir: TempDir::new("multirust").unwrap(),
        customdir: TempDir::new("multirust").unwrap(),
    };

    create_mock_dist_server(&config.distdir.path(), s);

    let current_exe_path = env::current_exe().map(PathBuf::from).unwrap();
    let exe_dir = current_exe_path.parent().unwrap();
    let ref multirust_build_path = exe_dir.join(format!("multirust-rs{}", EXE_SUFFIX));

    let multirust_path = config.exedir.path().join(format!("multirust{}", EXE_SUFFIX));
    let rustc_path = config.exedir.path().join(format!("rustc{}", EXE_SUFFIX));

    fs::copy(multirust_build_path, multirust_path).unwrap();
    fs::copy(multirust_build_path, rustc_path).unwrap();

    if s == Scenario::Full {
        let cargo_path = config.exedir.path().join(format!("cargo{}", EXE_SUFFIX));
        fs::copy(multirust_build_path, cargo_path).unwrap();
    }

    // Create some custom toolchains
    create_custom_toolchains(config.customdir.path());

    // Hold a lock while the test is running because they change directories,
    // causing havok
    lazy_static! {
        static ref LOCK: Mutex<()> = Mutex::new(());
    }
    let _g = LOCK.lock();

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

pub fn expect_ok_ex(config: &Config, args: &[&str],
                    stdout: &str, stderr: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    println!("out.ok: {}", out.ok);
    println!("out.stdout:\n\n{}", out.stdout);
    println!("out.stderr:\n\n{}", out.stderr);
    println!("expected.stdout: \n\n{}", stdout);
    println!("expected.stderr: \n\n{}", stderr);
    assert!(out.ok, format!("ok {:?}", args));
    assert!(out.stdout == stdout, format!("out {:?}", args));
    assert!(out.stderr == stderr, format!("err {:?}", args));
}

pub fn expect_err_ex(config: &Config, args: &[&str],
                     stdout: &str, stderr: &str) {
    let out = run(config, args[0], &args[1..], &[]);
    println!("out.ok: {}", out.ok);
    println!("out.stdout:\n\n{}", out.stdout);
    println!("out.stderr:\n\n{}", out.stderr);
    println!("expected.stdout: \n\n{}", stdout);
    println!("expected.stderr: \n\n{}", stderr);
    let args = format!("{:?}", args);
    assert!(out.ok, format!("{:?}", args));
    assert!(out.stdout == stdout, format!("out {:?}", args));
    assert!(out.stderr == stderr, format!("err {:?}", args));
}

#[derive(Debug)]
pub struct SanitizedOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn cmd(config: &Config, name: &str, args: &[&str]) -> Command {
    let exe_path = config.exedir.path().join(format!("{}{}", name, EXE_SUFFIX));
    let mut cmd = Command::new(exe_path);
    cmd.args(args);
    cmd.env("MULTIRUST_HOME", config.homedir.path().to_string_lossy().to_string());
    cmd.env("MULTIRUST_DIST_ROOT", format!("file://{}", config.distdir.path().join("dist").to_string_lossy()));

    cmd
}

pub fn run(config: &Config, name: &str, args: &[&str], env: &[(&str, &str)]) -> SanitizedOutput {
    let mut cmd = cmd(config, name, args);
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

pub fn change_dir(path: &Path, f: &Fn()) {
    let cwd = env::current_dir().unwrap();
    env::set_current_dir(path).unwrap();
    let _g = scopeguard::guard((), move |_| env::set_current_dir(&cwd).unwrap());
    f();
}

// Creates a mock dist server populated with some test data
fn create_mock_dist_server(path: &Path, s: Scenario) {
    let mut chans = Vec::new();
    if s == Scenario::Full || s == Scenario::ArchivesV1 || s == Scenario::ArchivesV2 {
        let c1 = build_mock_channel("nightly", "2015-01-01", "1.2.0", "hash-n-1");
        let c2 = build_mock_channel("beta", "2015-01-01", "1.1.0", "hash-b-1");
        let c3 = build_mock_channel("stable", "2015-01-01", "1.0.0", "hash-s-1");
        chans.extend(vec![c1, c2, c3]);
    }
    let c4 = build_mock_channel("nightly", "2015-01-02", "1.3.0", "hash-n-2");
    let c5 = build_mock_channel("beta", "2015-01-02", "1.2.0", "hash-b-2");
    let c6 = build_mock_channel("stable", "2015-01-02", "1.1.0", "hash-s-2");
    chans.extend(vec![c4, c5, c6]);

    let ref vs = match s {
        Scenario::Full => vec![ManifestVersion::V1, ManifestVersion::V2],
        Scenario::SimpleV1 | Scenario::ArchivesV1 => vec![ManifestVersion::V1],
        Scenario::SimpleV2 | Scenario::ArchivesV2 => vec![ManifestVersion::V2],
    };

    MockDistServer {
        path: path.to_owned(),
        channels: chans,
    }.write(vs);

    // Also create the manifests for stable releases by version
    if s == Scenario::Full || s == Scenario::ArchivesV1 || s == Scenario::ArchivesV2 {
        let _ = fs::copy(path.join("dist/2015-01-01/channel-rust-stable.toml"),
                         path.join("dist/channel-rust-1.0.0.toml"));
        let _ = fs::copy(path.join("dist/2015-01-01/channel-rust-stable.toml.sha256"),
                         path.join("dist/channel-rust-1.0.0.toml.sha256"));
    }
    let _ = fs::copy(path.join("dist/2015-01-02/channel-rust-stable.toml"),
                     path.join("dist/channel-rust-1.1.0.toml"));
    let _ = fs::copy(path.join("dist/2015-01-02/channel-rust-stable.toml.sha256"),
                     path.join("dist/channel-rust-1.1.0.toml.sha256"));

    // Same for v1 manifests. These are just the installers.
    let host_triple = this_host_triple();
    if s == Scenario::Full || s == Scenario::ArchivesV1 || s == Scenario::ArchivesV2 {
        fs::copy(path.join(format!("dist/2015-01-01/rust-stable-{}.tar.gz", host_triple)),
                 path.join(format!("dist/rust-1.0.0-{}.tar.gz", host_triple))).unwrap();
        fs::copy(path.join(format!("dist/2015-01-01/rust-stable-{}.tar.gz.sha256", host_triple)),
                 path.join(format!("dist/rust-1.0.0-{}.tar.gz.sha256", host_triple))).unwrap();
    }
    fs::copy(path.join(format!("dist/2015-01-02/rust-stable-{}.tar.gz", host_triple)),
             path.join(format!("dist/rust-1.1.0-{}.tar.gz", host_triple))).unwrap();
    fs::copy(path.join(format!("dist/2015-01-02/rust-stable-{}.tar.gz.sha256", host_triple)),
             path.join(format!("dist/rust-1.1.0-{}.tar.gz.sha256", host_triple))).unwrap();
}

pub static CROSS_ARCH1: &'static str = "x86_64-unknown-linux-musl";
pub static CROSS_ARCH2: &'static str = "arm-linux-androideabi";

fn build_mock_channel(channel: &str, date: &str,
                      version: &'static str, version_hash: &str) -> MockChannel {
    // Build the mock installers
    let std = build_mock_std_installer();
    let cross_std1 = build_mock_cross_std_installer(CROSS_ARCH1, date);
    let cross_std2 = build_mock_cross_std_installer(CROSS_ARCH2, date);
    let rustc = build_mock_rustc_installer(version, version_hash);
    let cargo = build_mock_cargo_installer(version, version_hash);
    let rust_docs = build_mock_rust_doc_installer();
    let rust = build_combined_installer(&[&std, &rustc, &cargo, &rust_docs]);

    let host_triple = this_host_triple();

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
            name: "rust-std".to_string(),
            target: host_triple.clone()
        });
        target_pkg.components.push(MockComponent {
            name: "rustc".to_string(),
            target: host_triple.clone()
        });
        target_pkg.components.push(MockComponent {
            name: "cargo".to_string(),
            target: host_triple.clone()
        });
        target_pkg.components.push(MockComponent {
            name: "rust-docs".to_string(),
            target: host_triple.clone()
        });
        target_pkg.extensions.push(MockComponent {
            name: "rust-std".to_string(),
            target: CROSS_ARCH1.to_string(),
        });
        target_pkg.extensions.push(MockComponent {
            name: "rust-std".to_string(),
            target: CROSS_ARCH2.to_string(),
        });
    }

    MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages: packages,
    }
}

pub fn this_host_triple() -> String {
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

fn build_mock_std_installer() -> MockInstallerBuilder {
    let trip = this_host_triple();
    MockInstallerBuilder {
        components: vec![
            (format!("rust-std-{}", trip.clone()),
             vec![MockCommand::File(format!("lib/rustlib/{}/libstd.rlib", trip))],
             vec![(format!("lib/rustlib/{}/libstd.rlib", trip), "".into())])
            ]
    }
}

fn build_mock_cross_std_installer(target: &str, date: &str) -> MockInstallerBuilder {
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
    MockInstallerBuilder {
        components: vec![
            ("rustc".to_string(),
             vec![MockCommand::File(rustc.clone())],
             vec![(rustc, mock_bin("rustc", version, version_hash))])
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

fn build_mock_rust_doc_installer() -> MockInstallerBuilder {
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
        File::create(source_path).and_then(|mut f| f.write_all(source.as_bytes())).unwrap();

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

// These are toolchains for installation with --link-local and --copy-local
fn create_custom_toolchains(customdir: &Path) {
    let ref dir = customdir.join("custom-1/bin");
    fs::create_dir_all(dir).unwrap();
    let rustc = mock_bin("rustc", "1.0.0", "hash-c-1");
    let ref path = customdir.join(format!("custom-1/bin/rustc{}", EXE_SUFFIX));
    let mut file = File::create(path).unwrap();
    file.write_all(&rustc).unwrap();
    make_exe(dir, path);

    let ref dir = customdir.join("custom-2/bin");
    fs::create_dir_all(dir).unwrap();
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
