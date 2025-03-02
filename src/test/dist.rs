//! Tools for building and working with the filesystem of a mock Rust
//! distribution server, with v1 and v2 manifests.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use url::Url;

use crate::dist::{
    Profile, TargetTriple,
    manifest::{
        Component, CompressionKind, HashedBinary, Manifest, ManifestVersion, Package,
        PackageTargets, Renamed, TargetedPackage,
    },
};

use super::clitools::hard_link;
use super::mock::MockInstallerBuilder;
use super::{CROSS_ARCH1, CROSS_ARCH2, MULTI_ARCH1, create_hash, this_host_triple};

pub(super) struct Release {
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
    pub(super) fn stable(version: &str, date: &str) -> Self {
        Release::new("stable", version, date, version)
    }

    pub(super) fn beta(version: &str, date: &str) -> Self {
        Release::new("beta", version, date, version)
    }

    pub(super) fn beta_with_tag(tag: Option<&str>, version: &str, date: &str) -> Self {
        let channel = match tag {
            Some(tag) => format!("{version}-beta.{tag}"),
            None => format!("{version}-beta"),
        };
        Release::new(&channel, version, date, version)
    }

    pub(super) fn with_rls(mut self, status: RlsStatus) -> Self {
        self.rls = status;
        self
    }

    pub(super) fn unavailable(mut self) -> Self {
        self.available = false;
        self
    }

    pub(super) fn multi_arch(mut self) -> Self {
        self.multi_arch = true;
        self
    }

    pub(super) fn only_multi_arch(mut self) -> Self {
        self.multi_arch = true;
        self.available = false;
        self
    }

    pub(super) fn new(channel: &str, version: &str, date: &str, suffix: &str) -> Self {
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

    pub(super) fn mock(&self) -> MockChannel {
        if self.available {
            MockChannel::new(
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
            MockChannel::new(
                &self.channel,
                &self.date,
                &self.version,
                &self.hash,
                self.rls,
                false,
                true,
            )
        } else {
            MockChannel::unavailable(&self.channel, &self.date, &self.version, &self.hash)
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) fn link(&self, path: &Path) {
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

// This function changes the mock manifest for a given channel to that
// of a particular date. For advancing the build from e.g. 2016-02-1
// to 2016-02-02
pub fn change_channel_date(dist_server: &Url, channel: &str, date: &str) {
    let path = dist_server.to_file_path().unwrap();

    // V2
    let manifest_name = format!("dist/channel-rust-{channel}");
    let manifest_path = path.join(format!("{manifest_name}.toml"));
    let hash_path = path.join(format!("{manifest_name}.toml.sha256"));
    let sig_path = path.join(format!("{manifest_name}.toml.asc"));

    let archive_manifest_name = format!("dist/{date}/channel-rust-{channel}");
    let archive_manifest_path = path.join(format!("{archive_manifest_name}.toml"));
    let archive_hash_path = path.join(format!("{archive_manifest_name}.toml.sha256"));
    let archive_sig_path = path.join(format!("{archive_manifest_name}.toml.asc"));

    let _ = hard_link(archive_manifest_path, manifest_path);
    let _ = hard_link(archive_hash_path, hash_path);
    let _ = hard_link(archive_sig_path, sig_path);

    // V1
    let manifest_name = format!("dist/channel-rust-{channel}");
    let manifest_path = path.join(&manifest_name);
    let hash_path = path.join(format!("{manifest_name}.sha256"));
    let sig_path = path.join(format!("{manifest_name}.asc"));

    let archive_manifest_name = format!("dist/{date}/channel-rust-{channel}");
    let archive_manifest_path = path.join(&archive_manifest_name);
    let archive_hash_path = path.join(format!("{archive_manifest_name}.sha256"));
    let archive_sig_path = path.join(format!("{archive_manifest_name}.asc"));

    let _ = hard_link(archive_manifest_path, manifest_path);
    let _ = hard_link(archive_hash_path, hash_path);
    let _ = hard_link(archive_sig_path, sig_path);

    // Copy all files that look like rust-* for the v1 installers
    let archive_path = path.join(format!("dist/{date}"));
    for dir in fs::read_dir(archive_path).unwrap() {
        let dir = dir.unwrap();
        if dir.file_name().to_str().unwrap().contains("rust-") {
            let path = path.join(format!("dist/{}", dir.file_name().to_str().unwrap()));
            hard_link(dir.path(), path).unwrap();
        }
    }
}

// A mock Rust v2 distribution server. Create it and run `write`
// to write its structure to a directory.
#[derive(Debug)]
pub(crate) struct MockDistServer {
    // The local path to the dist server root
    pub path: PathBuf,
    pub channels: Vec<MockChannel>,
}

// A Rust distribution channel
#[derive(Debug)]
pub(crate) struct MockChannel {
    // e.g. "nightly"
    pub name: String,
    // YYYY-MM-DD
    pub date: String,
    pub packages: Vec<MockPackage>,
    pub renames: HashMap<String, String>,
}

impl MockChannel {
    pub(super) fn new(
        channel: &str,
        date: &str,
        version: &str,
        version_hash: &str,
        rls: RlsStatus,
        multi_arch: bool,
        swap_triples: bool,
    ) -> Self {
        // Build the mock installers
        let host_triple = if swap_triples {
            MULTI_ARCH1.to_owned()
        } else {
            this_host_triple()
        };
        let std = MockInstallerBuilder::std(&host_triple);
        let rustc = MockInstallerBuilder::rustc(&host_triple, version, version_hash);
        let cargo = MockInstallerBuilder::cargo(version, version_hash);
        let rust_docs = MockInstallerBuilder::rust_doc();
        let rust = MockInstallerBuilder::combined(&[&std, &rustc, &cargo, &rust_docs]);
        let cross_std1 = MockInstallerBuilder::cross_std(CROSS_ARCH1, date);
        let cross_std2 = MockInstallerBuilder::cross_std(CROSS_ARCH2, date);
        let rust_src = MockInstallerBuilder::rust_src();
        let rust_analysis = MockInstallerBuilder::rust_analysis(&host_triple);

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
            let rls = MockInstallerBuilder::rls(version, version_hash, rls.pkg_name());
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
            let std = MockInstallerBuilder::std(MULTI_ARCH1);
            let rustc = MockInstallerBuilder::rustc(MULTI_ARCH1, version, version_hash);
            let cargo = MockInstallerBuilder::cargo(version, version_hash);
            let rust_docs = MockInstallerBuilder::rust_doc();
            let rust = MockInstallerBuilder::combined(&[&std, &rustc, &cargo, &rust_docs]);

            let triple = MULTI_ARCH1.to_string();
            all.std.push((std, triple.clone()));
            all.rustc.push((rustc, triple.clone()));
            all.cargo.push((cargo, triple.clone()));

            if rls != RlsStatus::Unavailable {
                let rls = MockInstallerBuilder::rls(version, version_hash, rls.pkg_name());
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
            let target_pkgs =
                target_pkgs
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

        Self {
            name: channel.to_string(),
            date: date.to_string(),
            packages,
            renames,
        }
    }

    pub(super) fn unavailable(
        channel: &str,
        date: &str,
        version: &str,
        version_hash: &str,
    ) -> Self {
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

        Self {
            name: channel.to_string(),
            date: date.to_string(),
            packages,
            renames: HashMap::new(),
        }
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

#[derive(Copy, Clone, Eq, PartialEq)]
pub(super) enum RlsStatus {
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

// A single rust-installer package
#[derive(Debug, Hash, Eq, PartialEq)]
pub(crate) struct MockPackage {
    // rust, rustc, rust-std-$triple, rust-doc, etc.
    pub name: &'static str,
    pub version: String,
    pub targets: Vec<MockTargetedPackage>,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub(crate) struct MockTargetedPackage {
    // Target triple
    pub target: String,
    // Whether the file actually exists (could be due to build failure)
    pub available: bool,
    pub components: Vec<MockComponent>,
    // The mock rust-installer
    pub installer: MockInstallerBuilder,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(crate) struct MockComponent {
    pub name: String,
    pub target: String,
    pub is_extension: bool,
}

#[derive(Clone)]
struct MockHashes {
    pub gz: String,
    pub xz: Option<String>,
    pub zst: Option<String>,
}

pub(crate) enum MockManifestVersion {
    V1,
    V2,
}

impl MockDistServer {
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn write(&self, vs: &[MockManifestVersion], enable_xz: bool, enable_zst: bool) {
        fs::create_dir_all(&self.path).unwrap();

        for channel in self.channels.iter() {
            let mut hashes = HashMap::new();
            for package in &channel.packages {
                let new_hashes = self.build_package(channel, package, enable_xz, enable_zst);
                hashes.extend(new_hashes);
            }
            for v in vs {
                match *v {
                    MockManifestVersion::V1 => self.write_manifest_v1(channel),
                    MockManifestVersion::V2 => self.write_manifest_v2(channel, &hashes),
                }
            }
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn build_package(
        &self,
        channel: &MockChannel,
        package: &MockPackage,
        enable_xz: bool,
        enable_zst: bool,
    ) -> HashMap<MockComponent, MockHashes> {
        let mut hashes = HashMap::new();

        for target_package in &package.targets {
            let gz_hash = self.build_target_package(channel, package, target_package, ".tar.gz");
            let xz_hash = if enable_xz {
                Some(self.build_target_package(channel, package, target_package, ".tar.xz"))
            } else {
                None
            };
            let zst_hash = if enable_zst {
                Some(self.build_target_package(channel, package, target_package, ".tar.zst"))
            } else {
                None
            };
            let component = MockComponent {
                name: package.name.to_string(),
                target: target_package.target.to_string(),
                is_extension: false,
            };
            hashes.insert(
                component,
                MockHashes {
                    gz: gz_hash,
                    xz: xz_hash,
                    zst: zst_hash,
                },
            );
        }

        hashes
    }

    // Returns the hash of the tarball
    #[tracing::instrument(level = "trace", skip_all, fields(format=%format))]
    fn build_target_package(
        &self,
        channel: &MockChannel,
        package: &MockPackage,
        target_package: &MockTargetedPackage,
        format: &str,
    ) -> String {
        // This is where the tarball, sums and sigs will go
        let dist_dir = self.path.join("dist");
        let archive_dir = dist_dir.join(&channel.date);

        fs::create_dir_all(&archive_dir).unwrap();

        let tmpdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

        let workdir = tmpdir.path().join("work");
        let installer_name = if target_package.target != "*" {
            format!(
                "{}-{}-{}",
                package.name, channel.name, target_package.target
            )
        } else {
            format!("{}-{}", package.name, channel.name)
        };
        let installer_dir = workdir.join(&installer_name);
        let installer_tarball = archive_dir.join(format!("{installer_name}{format}"));
        let installer_hash = archive_dir.join(format!("{installer_name}{format}.sha256"));

        fs::create_dir_all(&installer_dir).unwrap();

        type Tarball = HashMap<(String, MockTargetedPackage, String), (Vec<u8>, String)>;
        // Tarball creation can be super slow, so cache created tarballs
        // globally to avoid recreating and recompressing tons of tarballs.
        static TARBALLS: LazyLock<Mutex<Tarball>> = LazyLock::new(|| Mutex::new(HashMap::new()));

        let key = (
            installer_name.to_string(),
            target_package.clone(),
            format.to_string(),
        );
        let tarballs = TARBALLS.lock().unwrap();
        let hash = if tarballs.contains_key(&key) {
            let (ref contents, ref hash) = tarballs[&key];
            File::create(&installer_tarball)
                .unwrap()
                .write_all(contents)
                .unwrap();
            File::create(&installer_hash)
                .unwrap()
                .write_all(hash.as_bytes())
                .unwrap();
            hash.clone()
        } else {
            drop(tarballs);
            target_package.installer.build(&installer_dir);
            create_tarball(
                &PathBuf::from(&installer_name),
                &installer_dir,
                &installer_tarball,
            )
            .unwrap();
            let mut contents = Vec::new();
            File::open(&installer_tarball)
                .unwrap()
                .read_to_end(&mut contents)
                .unwrap();
            let hash = create_hash(&installer_tarball, &installer_hash);
            TARBALLS
                .lock()
                .unwrap()
                .insert(key, (contents, hash.clone()));
            hash
        };

        // Copy from the archive to the main dist directory
        if package.name == "rust" {
            let main_installer_tarball = dist_dir.join(format!("{installer_name}{format}"));
            let main_installer_hash = dist_dir.join(format!("{installer_name}{format}.sha256"));
            hard_link(installer_tarball, main_installer_tarball).unwrap();
            hard_link(installer_hash, main_installer_hash).unwrap();
        }

        hash
    }

    // The v1 manifest is just the directory listing of the rust tarballs
    #[tracing::instrument(level = "trace", skip_all)]
    fn write_manifest_v1(&self, channel: &MockChannel) {
        let mut buf = String::new();
        let package = channel.packages.iter().find(|p| p.name == "rust").unwrap();
        for target in &package.targets {
            let package_file_name = if target.target != "*" {
                format!("{}-{}-{}.tar.gz", package.name, channel.name, target.target)
            } else {
                format!("{}-{}.tar.gz", package.name, channel.name)
            };
            buf = buf + &package_file_name + "\n";
        }

        let manifest_name = format!("dist/channel-rust-{}", channel.name);
        let manifest_path = self.path.join(&manifest_name);
        write_file(&manifest_path, &buf);

        let hash_path = self.path.join(format!("{manifest_name}.sha256"));
        create_hash(&manifest_path, &hash_path);

        // Also copy the manifest and hash into the archive folder
        let archive_manifest_name = format!("dist/{}/channel-rust-{}", channel.date, channel.name);
        let archive_manifest_path = self.path.join(&archive_manifest_name);
        hard_link(manifest_path, archive_manifest_path).unwrap();

        let archive_hash_path = self.path.join(format!("{archive_manifest_name}.sha256"));
        hard_link(&hash_path, archive_hash_path).unwrap();
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn write_manifest_v2(
        &self,
        channel: &MockChannel,
        hashes: &HashMap<MockComponent, MockHashes>,
    ) {
        let mut manifest = Manifest {
            manifest_version: ManifestVersion::V2,
            date: channel.date.clone(),
            renames: HashMap::default(),
            packages: HashMap::default(),
            reverse_renames: HashMap::default(),
            profiles: HashMap::default(),
        };

        // [pkg.*]
        for package in &channel.packages {
            let mut targets = HashMap::default();

            // [pkg.*.target.*]
            for target in &package.targets {
                let mut tpkg = TargetedPackage {
                    bins: Vec::new(),
                    components: Vec::new(),
                };

                let package_file_name = if target.target != "*" {
                    format!("{}-{}-{}.tar.gz", package.name, channel.name, target.target)
                } else {
                    format!("{}-{}.tar.gz", package.name, channel.name)
                };

                let path = self
                    .path
                    .join("dist")
                    .join(&channel.date)
                    .join(package_file_name);

                let component = MockComponent {
                    name: package.name.to_owned(),
                    target: target.target.to_owned(),
                    is_extension: false,
                };

                if target.available {
                    let hash = hashes[&component].clone();
                    let url = format!("file://{}", path.to_string_lossy());
                    tpkg.bins.push(HashedBinary {
                        url: url.clone(),
                        hash: hash.gz,
                        compression: CompressionKind::GZip,
                    });

                    if let Some(xz_hash) = hash.xz {
                        tpkg.bins.push(HashedBinary {
                            url: url.replace(".tar.gz", ".tar.xz"),
                            hash: xz_hash,
                            compression: CompressionKind::XZ,
                        });
                    }

                    if let Some(zst_hash) = hash.zst {
                        tpkg.bins.push(HashedBinary {
                            url: url.replace(".tar.gz", ".tar.zst"),
                            hash: zst_hash,
                            compression: CompressionKind::ZStd,
                        });
                    }
                }

                // [pkg.*.target.*.components.*] and [pkg.*.target.*.extensions.*]
                for component in &target.components {
                    tpkg.components.push(Component {
                        pkg: component.name.to_owned(),
                        target: Some(TargetTriple::new(&component.target)),
                        is_extension: component.is_extension,
                    });
                }

                targets.insert(TargetTriple::new(&target.target), tpkg);
            }

            manifest.packages.insert(
                package.name.to_owned(),
                Package {
                    version: package.version.clone(),
                    targets: PackageTargets::Targeted(targets),
                },
            );
        }

        for (from, to) in &channel.renames {
            manifest
                .renames
                .insert(from.to_owned(), Renamed { to: to.to_owned() });
        }

        let profiles = &[
            (Profile::Minimal, &["rustc"][..]),
            (
                Profile::Default,
                &["rustc", "cargo", "rust-std", "rust-docs"],
            ),
            (
                Profile::Complete,
                &["rustc", "cargo", "rust-std", "rust-docs", "rls"],
            ),
        ];

        for (profile, values) in profiles {
            manifest
                .profiles
                .insert(*profile, values.iter().map(|&v| v.to_owned()).collect());
        }

        let manifest_name = format!("dist/channel-rust-{}", channel.name);
        let manifest_path = self.path.join(format!("{manifest_name}.toml"));
        let manifest_content = manifest.stringify().unwrap();
        write_file(&manifest_path, &manifest_content);

        let hash_path = self.path.join(format!("{manifest_name}.toml.sha256"));
        create_hash(&manifest_path, &hash_path);

        // Also copy the manifest and hash into the archive folder
        let archive_manifest_name = format!("dist/{}/channel-rust-{}", channel.date, channel.name);
        let archive_manifest_path = self.path.join(format!("{archive_manifest_name}.toml"));
        hard_link(&manifest_path, archive_manifest_path).unwrap();

        let archive_hash_path = self
            .path
            .join(format!("{archive_manifest_name}.toml.sha256"));
        hard_link(hash_path, archive_hash_path).unwrap();
    }
}

fn create_tarball(relpath: &Path, src: &Path, dst: &Path) -> io::Result<()> {
    match fs::remove_file(dst) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let outfile = File::create(dst)?;
    let mut gzwriter;
    let mut xzwriter;
    let mut zstwriter;
    let writer: &mut dyn Write = match &dst.to_string_lossy() {
        s if s.ends_with(".tar.gz") => {
            gzwriter = flate2::write::GzEncoder::new(outfile, flate2::Compression::none());
            &mut gzwriter
        }
        s if s.ends_with(".tar.xz") => {
            xzwriter = xz2::write::XzEncoder::new(outfile, 0);
            &mut xzwriter
        }
        s if s.ends_with(".tar.zst") => {
            zstwriter = zstd::stream::write::Encoder::new(outfile, 0)?.auto_finish();
            &mut zstwriter
        }
        _ => panic!("Unsupported archive format"),
    };
    let mut tar = tar::Builder::new(writer);
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let parts: Vec<_> = entry.path().iter().map(ToOwned::to_owned).collect();
        let parts_len = parts.len();
        let parts = parts.into_iter().skip(parts_len - entry.depth());
        let mut relpath = relpath.to_owned();
        relpath.extend(parts);
        if entry.file_type().is_file() {
            let mut srcfile = File::open(entry.path())?;
            tar.append_file(relpath, &mut srcfile)?;
        } else if entry.file_type().is_dir() {
            tar.append_dir(relpath, entry.path())?;
        }
    }
    tar.finish()
}

pub(super) fn write_file(dst: &Path, contents: &str) {
    drop(fs::remove_file(dst));
    File::create(dst)
        .and_then(|mut f| f.write_all(contents.as_bytes()))
        .unwrap();
}
