// Tests of installation and updates from a v2 Rust distribution
// server (mocked on the file system)

extern crate rust_install;
extern crate rust_manifest;
extern crate tempdir;
extern crate tar;
extern crate openssl;
extern crate toml;
extern crate flate2;
extern crate walkdir;
extern crate itertools;
extern crate hyper;

use rust_install::{InstallPrefix, InstallType, Error, NotifyHandler};
use rust_install::mock::{MockInstallerBuilder, MockCommand};
use rust_install::dist::ToolchainDesc;
use rust_install::download::DownloadCfg;
use rust_install::utils;
use rust_install::temp;
use rust_install::manifest::{Manifestation, UpdateStatus, Changes};
use rust_manifest::{Manifest, Component};
use hyper::Url;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::collections::HashMap;
use std::io::{Read, Write};
use tempdir::TempDir;
use openssl::crypto::hash;
use itertools::Itertools;

// The manifest version created by this mock
pub const MOCK_MANIFEST_VERSION: &'static str = "2";

// A mock Rust v2 distribution server. Create it and and run `write`
// to write its structure to a directory.
struct MockDistServer {
    // The local path to the dist server root
    path: PathBuf,
    channels: Vec<MockChannel>,
}

// A Rust distribution channel
struct MockChannel {
    // e.g. "nightly"
    name: String,
    // YYYY-MM-DD
    date: String,
    packages: Vec<MockPackage>,
}

// A single rust-installer package
struct MockPackage {
    // rust, rustc, rust-std-$triple, rust-doc, etc.
    name: &'static str,
    version: &'static str,
    targets: Vec<MockTargettedPackage>,
}

struct MockTargettedPackage {
    // Target triple
    target: &'static str,
    // Whether the file actually exists (could be due to build failure)
    available: bool,
    // Required components
    components: Vec<MockComponent>,
    // Optional components
    extensions: Vec<MockComponent>,
    // The mock rust-installer
    installer: MockInstallerBuilder,
}

struct MockComponent {
    name: &'static str,
    target: &'static str,
}

impl MockDistServer {
    fn write(&self) {
        fs::create_dir_all(&self.path).unwrap();

        // Build channels in reverse chronological order so the
        // top-level manifest is for the oldest date.
        for channel in self.channels.iter().rev() {
            let mut hashes = HashMap::new();
            for package in &channel.packages {
                let new_hashes = self.build_package(&channel, &package);
                hashes.extend(new_hashes.into_iter());
            }
            self.write_manifest(&channel, hashes);
        }
    }

    fn build_package(&self, channel: &MockChannel, package: &MockPackage) -> HashMap<Component, String> {
        let mut hashes = HashMap::new();

        for target_package in &package.targets {
            let hash = self.build_target_package(channel, package, target_package);
            let component = Component {
                pkg: package.name.to_string(),
                target: target_package.target.to_string(),
            };
            hashes.insert(component, hash);
        }

        return hashes;
    }

    // Returns the hash of the tarball
    fn build_target_package(&self,
                            channel: &MockChannel,
                            package: &MockPackage,
                            target_package: &MockTargettedPackage) -> String {
        // This is where the tarball, sums and sigs will go
        let ref outdir = self.path.join("dist").join(&channel.date);

        fs::create_dir_all(outdir).unwrap();

        let tmpdir = TempDir::new("multirust").unwrap();

        let workdir = tmpdir.path().join("work");
        let ref installer_name = format!("{}-{}-{}", package.name, channel.name, target_package.target);
        let ref installer_dir = workdir.join(installer_name);
        let ref installer_tarball = outdir.join(format!("{}.tar.gz", installer_name));
        let ref installer_hash = outdir.join(format!("{}.tar.gz.sha256", installer_name));

        fs::create_dir_all(installer_dir).unwrap();

        target_package.installer.build(installer_dir);
        create_tarball(&PathBuf::from(installer_name),
                       installer_dir, installer_tarball);

        // Create hash
        let hash = create_hash(installer_tarball, installer_hash);

        hash
    }

    fn write_manifest(&self, channel: &MockChannel, hashes: HashMap<Component, String>) {
        let mut toml_manifest = toml::Table::new();

        toml_manifest.insert(String::from("manifest-version"), toml::Value::String(MOCK_MANIFEST_VERSION.to_owned()));
        toml_manifest.insert(String::from("date"), toml::Value::String(channel.date.to_owned()));

        // [pkg.*]
        let mut toml_packages = toml::Table::new();
        for package in &channel.packages {
            let mut toml_package = toml::Table::new();
            toml_package.insert(String::from("version"), toml::Value::String(package.version.to_owned()));

            // [pkg.*.target.*]
            let mut toml_targets = toml::Table::new();
            for target in &package.targets {
                let mut toml_target = toml::Table::new();
                toml_target.insert(String::from("available"), toml::Value::Boolean(target.available));

                let package_file_name = format!("{}-{}-{}.tar.gz", package.name, channel.name, target.target);
                let path = self.path.join("dist").join(&channel.date).join(package_file_name);
                let url = format!("file://{}", path.to_string_lossy());
                toml_target.insert(String::from("url"), toml::Value::String(url));

                let ref component = Component {
                    pkg: package.name.to_owned(),
                    target: target.target.to_owned(),
                };
                let hash = hashes[component].clone();
                toml_target.insert(String::from("hash"), toml::Value::String(hash));

                // [pkg.*.target.*.components.*]
                let mut toml_components = toml::Array::new();
                for component in &target.components {
                    let mut toml_component = toml::Table::new();
                    toml_component.insert(String::from("pkg"), toml::Value::String(component.name.to_owned()));
                    toml_component.insert(String::from("target"), toml::Value::String(component.target.to_owned()));
                    toml_components.push(toml::Value::Table(toml_component));
                }
                toml_target.insert(String::from("components"), toml::Value::Array(toml_components));

                // [pkg.*.target.*.extensions.*]
                let mut toml_extensions = toml::Array::new();
                for extension in &target.extensions {
                    let mut toml_extension = toml::Table::new();
                    toml_extension.insert(String::from("pkg"), toml::Value::String(extension.name.to_owned()));
                    toml_extension.insert(String::from("target"), toml::Value::String(extension.target.to_owned()));
                    toml_extensions.push(toml::Value::Table(toml_extension));
                }
                toml_target.insert(String::from("extensions"), toml::Value::Array(toml_extensions));

                toml_targets.insert(String::from(target.target), toml::Value::Table(toml_target));
            }
            toml_package.insert(String::from("target"), toml::Value::Table(toml_targets));

            toml_packages.insert(String::from(package.name), toml::Value::Table(toml_package));
        }
        toml_manifest.insert(String::from("pkg"), toml::Value::Table(toml_packages));

        let manifest_name = format!("dist/channel-rust-{}", channel.name);
        let ref manifest_path = self.path.join(format!("{}.toml", manifest_name));
        utils::raw::write_file(manifest_path, &toml::encode_str(&toml_manifest)).unwrap();

        let ref hash_path = self.path.join(format!("{}.toml.sha256", manifest_name));
        create_hash(manifest_path, hash_path);

        // Also copy the manifest and hash into the archive folder
        let archive_manifest_name = format!("dist/{}/channel-rust-{}", channel.date, channel.name);
        let ref archive_manifest_path = self.path.join(format!("{}.toml", archive_manifest_name));
        utils::copy_file(manifest_path, archive_manifest_path).unwrap();

        let ref archive_hash_path = self.path.join(format!("{}.toml.sha256", archive_manifest_name));
        utils::copy_file(hash_path, archive_hash_path).unwrap();
    }
}

fn create_tarball(relpath: &Path, src: &Path, dst: &Path) {
    let outfile = File::create(dst).unwrap();
    let gzwriter = flate2::write::GzEncoder::new(outfile, flate2::Compression::None);
    let mut tar = tar::Builder::new(gzwriter);
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.unwrap();
        let parts: Vec<_> = entry.path().iter().map(|p| p.to_owned()).collect();
        let parts_len = parts.len();
        let parts = parts.into_iter().skip(parts_len - entry.depth());
        let mut relpath = relpath.to_owned();
        for part in parts {
            relpath = relpath.join(part);
        }
        if entry.file_type().is_file() {
            let ref mut srcfile = File::open(entry.path()).unwrap();
            tar.append_file(relpath, srcfile).unwrap();
        } else if entry.file_type().is_dir() {
            tar.append_dir(relpath, entry.path()).unwrap();
        }
    }
    tar.finish().unwrap();
}

fn create_hash(src: &Path, dst: &Path) -> String {
    let ref mut buf = Vec::new();
    File::open(src).unwrap().read_to_end(buf).unwrap();
    let mut hasher = hash::Hasher::new(hash::Type::SHA256);
    hasher.write_all(&buf).unwrap();
    let hex = hasher.finish().iter().map(|b| format!("{:02x}", b)).join("");
    let src_file = src.file_name().unwrap();
    let ref file_contents = format!("{} *{}\n", hex, src_file.to_string_lossy());
    utils::raw::write_file(dst, file_contents).unwrap();

    hex
}

// This function changes the mock manifest for a given channel to that
// of a particular date. For advancing the build from e.g. 2016-02-1
// to 2016-02-02
fn change_channel_date(dist_server: &Url, channel: &str, date: &str) {
    let path = dist_server.to_file_path().unwrap();

    let manifest_name = format!("dist/channel-rust-{}", channel);
    let ref manifest_path = path.join(format!("{}.toml", manifest_name));
    let ref hash_path = path.join(format!("{}.toml.sha256", manifest_name));

    let archive_manifest_name = format!("dist/{}/channel-rust-{}", date, channel);
    let ref archive_manifest_path = path.join(format!("{}.toml", archive_manifest_name));
    let ref archive_hash_path = path.join(format!("{}.toml.sha256", archive_manifest_name));

    utils::copy_file(archive_manifest_path, manifest_path).unwrap();
    utils::copy_file(archive_hash_path, hash_path).unwrap();
}

// Creates the single mock dist server used by all the tests in this file
fn create_mock_dist_server(path: &Path,
                           edit: Option<&Fn(&str, &mut MockPackage)>) -> MockDistServer {
    MockDistServer {
        path: path.to_owned(),
        channels: vec![
            create_mock_channel("nightly", "2016-02-01", edit),
            create_mock_channel("nightly", "2016-02-02", edit),
            ]
    }
}

fn create_mock_channel(channel: &str, date: &str,
                       edit: Option<&Fn(&str, &mut MockPackage)>) -> MockChannel {
    // Put the date in the files so they can be differentiated
    let contents = date.to_string();

    let rust_pkg = MockPackage {
        name: "rust",
        version: "1.0.0",
        targets: vec![
            MockTargettedPackage {
                target: "x86_64-apple-darwin",
                available: true,
                components: vec![
                    MockComponent {
                        name: "rustc",
                        target: "x86_64-apple-darwin",
                    },
                    MockComponent {
                        name: "rust-std",
                        target: "x86_64-apple-darwin",
                    },
                    ],
                extensions: vec![
                    MockComponent {
                        name: "rust-std",
                        target: "i686-apple-darwin",
                    },
                    MockComponent {
                        name: "rust-std",
                        target: "i686-unknown-linux-gnu",
                    },
                    ],
                installer: MockInstallerBuilder {
                    components: vec![]
                }
            },
            MockTargettedPackage {
                target: "i686-apple-darwin",
                available: true,
                components: vec![
                    MockComponent {
                        name: "rustc",
                        target: "i686-apple-darwin",
                    },
                    MockComponent {
                        name: "rust-std",
                        target: "i686-apple-darwin",
                    },
                    ],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![]
                }
            }
            ]
    };

    let rustc_pkg = MockPackage {
        name: "rustc",
        version: "1.0.0",
        targets: vec![
            MockTargettedPackage {
                target: "x86_64-apple-darwin",
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("rustc",
                         vec![
                             MockCommand::File("bin/rustc"),
                             ],
                         vec![
                             ("bin/rustc", contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            MockTargettedPackage {
                target: "i686-apple-darwin",
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![]
                }
            }
            ]
    };

    let std_pkg = MockPackage {
        name: "rust-std",
        version: "1.0.0",
        targets: vec![
            MockTargettedPackage {
                target: "x86_64-apple-darwin",
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("rust-std-x86_64-apple-darwin",
                         vec![
                             MockCommand::File("lib/libstd.rlib"),
                             ],
                         vec![
                             ("lib/libstd.rlib", contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            MockTargettedPackage {
                target: "i686-apple-darwin",
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("rust-std-i686-apple-darwin",
                         vec![
                             MockCommand::File("lib/i686-apple-darwin/libstd.rlib"),
                             ],
                         vec![
                             ("lib/i686-apple-darwin/libstd.rlib", contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            MockTargettedPackage {
                target: "i686-unknown-linux-gnu",
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("rust-std-i686-unknown-linux-gnu",
                         vec![
                             MockCommand::File("lib/i686-unknown-linux-gnu/libstd.rlib"),
                             ],
                         vec![
                             ("lib/i686-unknown-linux-gnu/libstd.rlib", contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            ]
    };

    // An extra package that can be used as a component of the other packages
    // for various tests
    let bonus_pkg = MockPackage {
        name: "bonus",
        version: "1.0.0",
        targets: vec![
            MockTargettedPackage {
                target: "x86_64-apple-darwin",
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("bonus-x86_64-apple-darwin",
                         vec![
                             MockCommand::File("bin/bonus"),
                             ],
                         vec![
                             ("bin/bonus", contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            ]
    };

    let mut rust_pkg = rust_pkg;
    if let Some(edit) = edit {
        edit(date, &mut rust_pkg);
    }

    MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages: vec![
            rust_pkg,
            rustc_pkg,
            std_pkg,
            bonus_pkg,
            ]
    }
}

#[test]
fn mock_dist_server_smoke_test() {
    let tempdir = TempDir::new("multirust").unwrap();
    let path = tempdir.path();

    create_mock_dist_server(&path, None).write();

    assert!(utils::path_exists(path.join("dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rustc-nightly-i686-apple-darwin.tar.gz")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rust-std-nightly-x86_64-apple-darwin.tar.gz")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rust-std-nightly-i686-apple-darwin.tar.gz")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz.sha256")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rustc-nightly-i686-apple-darwin.tar.gz.sha256")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rust-std-nightly-x86_64-apple-darwin.tar.gz.sha256")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rust-std-nightly-i686-apple-darwin.tar.gz.sha256")));
    assert!(utils::path_exists(path.join("dist/channel-rust-nightly.toml")));
    assert!(utils::path_exists(path.join("dist/channel-rust-nightly.toml.sha256")));
}

// Installs or updates a toolchain from a dist server.  If an initial
// install then it will be installed with the default components.  If
// an upgrade then all the existing components will be upgraded.
fn update_from_dist(dist_server: &Url,
                    toolchain: &ToolchainDesc,
                    prefix: &InstallPrefix,
                    add: &[Component],
                    remove: &[Component],
                    temp_cfg: &temp::Cfg,
                    notify_handler: NotifyHandler) -> Result<UpdateStatus, Error> {

    // Download the dist manifest and place it into the installation prefix
    let ref manifest_url = try!(make_manifest_url(dist_server, toolchain));
    let download = DownloadCfg {
        temp_cfg: temp_cfg,
        notify_handler: notify_handler.clone(),
        gpg_key: None,
    };
    let manifest_file = try!(download.get(&manifest_url.serialize()));
    let manifest_str = try!(utils::read_file("manifest", &manifest_file));
    let manifest = try!(Manifest::parse(&manifest_str));

    // Read the manifest to update the components
    let trip = try!(toolchain.target_triple().ok_or_else(|| Error::UnsupportedHost(toolchain.full_spec())));
    let manifestation = try!(Manifestation::open(prefix.clone(), &trip));

    let changes = Changes {
        add_extensions: add.to_owned(),
        remove_extensions: remove.to_owned(),
    };

    manifestation.update(&manifest, changes, temp_cfg, notify_handler.clone())
}

fn uninstall(toolchain: &ToolchainDesc, prefix: &InstallPrefix, temp_cfg: &temp::Cfg,
             notify_handler: NotifyHandler) -> Result<(), Error> {
    let trip = try!(toolchain.target_triple().ok_or_else(|| Error::UnsupportedHost(toolchain.full_spec())));
    let manifestation = try!(Manifestation::open(prefix.clone(), &trip));

    try!(manifestation.uninstall(temp_cfg, notify_handler.clone()));

    Ok(())
}

fn make_manifest_url(dist_server: &Url, toolchain: &ToolchainDesc) -> Result<Url, Error> {
    let mut url = dist_server.clone();
    if let Some(mut p) = url.path_mut() {
        p.push(format!("dist/channel-rust-{}.toml", toolchain.channel));
    } else {
        // FIXME
        panic!()
    }

    Ok(url)
}

fn setup(edit: Option<&Fn(&str, &mut MockPackage)>,
         f: &Fn(&Url, &ToolchainDesc, &InstallPrefix, &temp::Cfg)) {
    let dist_tempdir = TempDir::new("multirust").unwrap();
    create_mock_dist_server(dist_tempdir.path(), edit).write();

    let prefix_tempdir = TempDir::new("multirust").unwrap();

    let work_tempdir = TempDir::new("multirust").unwrap();
    let ref temp_cfg = temp::Cfg::new(work_tempdir.path().to_owned(), temp::SharedNotifyHandler::none());

    let ref url = Url::parse(&format!("file://{}", dist_tempdir.path().to_string_lossy())).unwrap();
    let ref toolchain = ToolchainDesc::from_str("x86_64-apple-darwin-nightly").unwrap();
    let ref prefix = InstallPrefix::from(prefix_tempdir.path().to_owned(), InstallType::Shared);

    f(url, toolchain, prefix, temp_cfg);
}

#[test]
fn initial_install() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn test_uninstall() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        uninstall(toolchain, prefix, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(!utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn uninstall_removes_config_file() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.manifest_file("multirust-config.toml")));
        uninstall(toolchain, prefix, temp_cfg, NotifyHandler::none()).unwrap();
        assert!(!utils::path_exists(&prefix.manifest_file("multirust-config.toml")));
    });
}

#[test]
fn upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert_eq!("2016-02-01", utils::raw::read_file(&prefix.path().join("bin/rustc")).unwrap());
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert_eq!("2016-02-02", utils::raw::read_file(&prefix.path().join("bin/rustc")).unwrap());
    });
}

#[test]
fn update_removes_components_that_dont_exist() {
    // On day 1 install the 'bonus' component, on day 2 its no londer a component
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(!utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_preserves_extensions() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));

        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn update_preserves_extensions_that_became_components() {
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.extensions.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
        if date == "2016-02-02" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "bonus".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));

        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_preserves_components_that_became_extensions() {
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
        if date == "2016-02-02" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.extensions.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_makes_no_changes_for_identical_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let status = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert_eq!(status, UpdateStatus::Changed);
        let status = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert_eq!(status, UpdateStatus::Unchanged);
    });
}

#[test]
fn add_extensions_for_initial_install() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_extensions_for_same_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_extensions_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
#[should_panic]
fn add_extension_not_in_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-bogus".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
#[should_panic]
fn add_extension_that_is_required_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rustc".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
#[ignore]
fn add_extensions_for_same_manifest_does_not_reinstall_other_components() {
}

#[test]
#[ignore]
fn add_extensions_for_same_manifest_when_extension_already_installed() {
}

#[test]
fn add_extensions_does_not_remove_other_components() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
    });
}

// Asking to remove extensions on initial install is nonsese.
#[test]
#[should_panic]
fn remove_extensions_for_initial_install() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref removes = vec![
            Component {
                pkg: "rustc".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
fn remove_extensions_for_same_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn remove_extensions_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
#[should_panic]
fn remove_extension_not_in_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "rust-bogus".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

// Extensions that don't exist in the manifest may still exist on disk
// from a previous manifest. The can't be requested to be removed though;
// only things in the manifest can.
#[test]
#[should_panic]
fn remove_extension_not_in_manifest_but_is_already_installed() {
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.extensions.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "bonus".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];
        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "bonus".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];
        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
#[should_panic]
fn remove_extension_that_is_required_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rustc".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
#[should_panic]
fn remove_extension_not_installed() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
#[ignore]
fn remove_extensions_for_same_manifest_does_not_reinstall_other_components() {
}

#[test]
fn remove_extensions_does_not_remove_other_components() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
    });
}

#[test]
fn add_and_remove_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(!utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_and_remove() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(!utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
#[should_panic]
fn add_and_remove_same_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple_darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
fn bad_component_hash() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let path = url.to_file_path().unwrap();
        let path = path.join("dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz");
        utils::raw::write_file(&path, "bogus").unwrap();

        let err = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap_err();

        match err {
            Error::ChecksumFailed { .. } => (),
            _ => panic!()
        }
    });
}

#[test]
fn unable_to_download_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let path = url.to_file_path().unwrap();
        let path = path.join("dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz");
        fs::remove_file(&path).unwrap();

        let err = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap_err();

        match err {
            Error::ComponentDownloadFailed(..) => (),
            _ => panic!()
        }
    });
}
