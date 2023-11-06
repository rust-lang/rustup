//! Tools for building and working with the filesystem of a mock Rust
//! distribution server, with v1 and v2 manifests.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use url::Url;

use super::clitools::hard_link;
use super::MockInstallerBuilder;

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
            hard_link(&dir.path(), path).unwrap();
        }
    }
}

// The manifest version created by this mock
pub const MOCK_MANIFEST_VERSION: &str = "2";

// A mock Rust v2 distribution server. Create it and and run `write`
// to write its structure to a directory.
#[derive(Debug)]
pub struct MockDistServer {
    // The local path to the dist server root
    pub path: PathBuf,
    pub channels: Vec<MockChannel>,
}

// A Rust distribution channel
#[derive(Debug)]
pub struct MockChannel {
    // e.g. "nightly"
    pub name: String,
    // YYYY-MM-DD
    pub date: String,
    pub packages: Vec<MockPackage>,
    pub renames: HashMap<String, String>,
}

// A single rust-installer package
#[derive(Debug, Hash, Eq, PartialEq)]
pub struct MockPackage {
    // rust, rustc, rust-std-$triple, rust-doc, etc.
    pub name: &'static str,
    pub version: String,
    pub targets: Vec<MockTargetedPackage>,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct MockTargetedPackage {
    // Target triple
    pub target: String,
    // Whether the file actually exists (could be due to build failure)
    pub available: bool,
    pub components: Vec<MockComponent>,
    // The mock rust-installer
    pub installer: MockInstallerBuilder,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct MockComponent {
    pub name: String,
    pub target: String,
    pub is_extension: bool,
}

#[derive(Clone)]
pub struct MockHashes {
    pub gz: String,
    pub xz: Option<String>,
    pub zst: Option<String>,
}

pub enum ManifestVersion {
    V1,
    V2,
}

impl MockDistServer {
    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
    pub fn write(&self, vs: &[ManifestVersion], enable_xz: bool, enable_zst: bool) {
        fs::create_dir_all(&self.path).unwrap();

        for channel in self.channels.iter() {
            let mut hashes = HashMap::new();
            for package in &channel.packages {
                let new_hashes = self.build_package(channel, package, enable_xz, enable_zst);
                hashes.extend(new_hashes);
            }
            for v in vs {
                match *v {
                    ManifestVersion::V1 => self.write_manifest_v1(channel),
                    ManifestVersion::V2 => self.write_manifest_v2(channel, &hashes),
                }
            }
        }
    }

    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
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
    #[cfg_attr(feature = "otel", tracing::instrument(skip_all, fields(format=%format)))]
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
        static TARBALLS: Lazy<Mutex<Tarball>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
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

    #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
    fn write_manifest_v2(
        &self,
        channel: &MockChannel,
        hashes: &HashMap<MockComponent, MockHashes>,
    ) {
        let mut toml_manifest = toml::value::Table::new();

        toml_manifest.insert(
            String::from("manifest-version"),
            toml::Value::String(MOCK_MANIFEST_VERSION.to_owned()),
        );
        toml_manifest.insert(
            String::from("date"),
            toml::Value::String(channel.date.to_owned()),
        );

        // [pkg.*]
        let mut toml_packages = toml::value::Table::new();
        for package in &channel.packages {
            let mut toml_package = toml::value::Table::new();
            toml_package.insert(
                String::from("version"),
                toml::Value::String(package.version.to_owned()),
            );

            // [pkg.*.target.*]
            let mut toml_targets = toml::value::Table::new();
            for target in &package.targets {
                let mut toml_target = toml::value::Table::new();
                toml_target.insert(
                    String::from("available"),
                    toml::Value::Boolean(target.available),
                );

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
                let url = format!("file://{}", path.to_string_lossy());
                toml_target.insert(String::from("url"), toml::Value::String(url.clone()));

                let component = MockComponent {
                    name: package.name.to_owned(),
                    target: target.target.to_owned(),
                    is_extension: false,
                };
                let hash = hashes[&component].clone();
                toml_target.insert(String::from("hash"), toml::Value::String(hash.gz));

                if let Some(xz_hash) = hash.xz {
                    toml_target.insert(
                        String::from("xz_url"),
                        toml::Value::String(url.replace(".tar.gz", ".tar.xz")),
                    );
                    toml_target.insert(String::from("xz_hash"), toml::Value::String(xz_hash));
                }
                if let Some(zst_hash) = hash.zst {
                    toml_target.insert(
                        String::from("zst_url"),
                        toml::Value::String(url.replace(".tar.gz", ".tar.zst")),
                    );
                    toml_target.insert(String::from("zst_hash"), toml::Value::String(zst_hash));
                }

                // [pkg.*.target.*.components.*] and [pkg.*.target.*.extensions.*]
                let mut toml_components = toml::value::Array::new();
                let mut toml_extensions = toml::value::Array::new();
                for component in &target.components {
                    let mut toml_component = toml::value::Table::new();
                    toml_component.insert(
                        String::from("pkg"),
                        toml::Value::String(component.name.to_owned()),
                    );
                    toml_component.insert(
                        String::from("target"),
                        toml::Value::String(component.target.to_owned()),
                    );
                    if component.is_extension {
                        toml_extensions.push(toml::Value::Table(toml_component));
                    } else {
                        toml_components.push(toml::Value::Table(toml_component));
                    }
                }
                toml_target.insert(
                    String::from("components"),
                    toml::Value::Array(toml_components),
                );
                toml_target.insert(
                    String::from("extensions"),
                    toml::Value::Array(toml_extensions),
                );

                toml_targets.insert(target.target.clone(), toml::Value::Table(toml_target));
            }
            toml_package.insert(String::from("target"), toml::Value::Table(toml_targets));

            toml_packages.insert(String::from(package.name), toml::Value::Table(toml_package));
        }
        toml_manifest.insert(String::from("pkg"), toml::Value::Table(toml_packages));

        let mut toml_renames = toml::value::Table::new();
        for (from, to) in &channel.renames {
            let mut toml_rename = toml::value::Table::new();
            toml_rename.insert(String::from("to"), toml::Value::String(to.to_owned()));
            toml_renames.insert(from.to_owned(), toml::Value::Table(toml_rename));
        }
        toml_manifest.insert(String::from("renames"), toml::Value::Table(toml_renames));

        let mut toml_profiles = toml::value::Table::new();
        let profiles = &[
            ("minimal", vec!["rustc"]),
            ("default", vec!["rustc", "cargo", "rust-std", "rust-docs"]),
            (
                "complete",
                vec!["rustc", "cargo", "rust-std", "rust-docs", "rls"],
            ),
        ];
        for (profile, values) in profiles {
            let array = values
                .iter()
                .map(|v| toml::Value::String((**v).to_owned()))
                .collect();
            toml_profiles.insert((*profile).to_string(), toml::Value::Array(array));
        }
        toml_manifest.insert(String::from("profiles"), toml::Value::Table(toml_profiles));

        let manifest_name = format!("dist/channel-rust-{}", channel.name);
        let manifest_path = self.path.join(format!("{manifest_name}.toml"));
        let manifest_content = toml::to_string(&toml_manifest).unwrap();
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
    write_file(dst, &file_contents);
    hex
}

pub fn write_file(dst: &Path, contents: &str) {
    drop(fs::remove_file(dst));
    File::create(dst)
        .and_then(|mut f| f.write_all(contents.as_bytes()))
        .unwrap();
}
