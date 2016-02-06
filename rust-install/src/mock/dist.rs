use mock::{MockInstallerBuilder, MockCommand};
use utils;
use rust_manifest::Component;
use hyper::Url;
use std::path::{PathBuf, Path};
use std::fs::{self, File};
use std::collections::HashMap;
use std::io::{Read, Write};
use tempdir::TempDir;
use openssl::crypto::hash;
use itertools::Itertools;
use toml;
use flate2;
use tar;
use walkdir;

// Creates a mock dist server populated with some test data
pub fn create_mock_dist_server(path: &Path,
                               edit: Option<&Fn(&str, &mut MockPackage)>) -> MockDistServer {
    MockDistServer {
        path: path.to_owned(),
        channels: vec![
            create_mock_channel("nightly", "2016-02-01", edit),
            create_mock_channel("nightly", "2016-02-02", edit),
            ]
    }
}

// This function changes the mock manifest for a given channel to that
// of a particular date. For advancing the build from e.g. 2016-02-1
// to 2016-02-02
pub fn change_channel_date(dist_server: &Url, channel: &str, date: &str) {
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

// The manifest version created by this mock
pub const MOCK_MANIFEST_VERSION: &'static str = "2";

// A mock Rust v2 distribution server. Create it and and run `write`
// to write its structure to a directory.
pub struct MockDistServer {
    // The local path to the dist server root
    pub path: PathBuf,
    pub channels: Vec<MockChannel>,
}

// A Rust distribution channel
pub struct MockChannel {
    // e.g. "nightly"
    pub name: String,
    // YYYY-MM-DD
    pub date: String,
    pub packages: Vec<MockPackage>,
}

// A single rust-installer package
pub struct MockPackage {
    // rust, rustc, rust-std-$triple, rust-doc, etc.
    pub name: &'static str,
    pub version: &'static str,
    pub targets: Vec<MockTargettedPackage>,
}

pub struct MockTargettedPackage {
    // Target triple
    pub target: &'static str,
    // Whether the file actually exists (could be due to build failure)
    pub available: bool,
    // Required components
    pub components: Vec<MockComponent>,
    // Optional components
    pub extensions: Vec<MockComponent>,
    // The mock rust-installer
    pub installer: MockInstallerBuilder,
}

pub struct MockComponent {
    pub name: &'static str,
    pub target: &'static str,
}

impl MockDistServer {
    pub fn write(&self) {
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
