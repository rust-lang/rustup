//! Mocks for testing

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use super::clitools::mock_bin;
use super::{this_host_triple, topical_doc_data};

// Mock of the on-disk structure of rust-installer installers
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct MockInstallerBuilder {
    pub components: Vec<MockComponentBuilder>,
}

impl MockInstallerBuilder {
    pub(super) fn std(trip: &str) -> Self {
        Self {
            components: vec![MockComponentBuilder {
                name: format!("rust-std-{trip}"),
                files: vec![MockFile::new(
                    format!("lib/rustlib/{trip}/libstd.rlib"),
                    b"",
                )],
            }],
        }
    }

    pub(super) fn cross_std(target: &str, date: &str) -> Self {
        Self {
            components: vec![MockComponentBuilder {
                name: format!("rust-std-{target}"),
                files: vec![
                    MockFile::new(format!("lib/rustlib/{target}/lib/libstd.rlib"), b""),
                    MockFile::new(format!("lib/rustlib/{target}/lib/{date}"), b""),
                ],
            }],
        }
    }

    pub(super) fn rustc(target: &str, version: &str, version_hash_: &str) -> Self {
        // For cross-host rustc's modify the version_hash so they can be identified from
        // test cases.
        let this_host = this_host_triple();
        let version_hash = if this_host != target {
            format!("xxxx-{}", &version_hash_[5..])
        } else {
            version_hash_.to_string()
        };

        Self {
            components: vec![MockComponentBuilder {
                name: "rustc".to_string(),
                files: mock_bin("rustc", version, &version_hash),
            }],
        }
    }

    pub(super) fn cargo(version: &str, version_hash: &str) -> Self {
        Self {
            components: vec![MockComponentBuilder {
                name: "cargo".to_string(),
                files: mock_bin("cargo", version, version_hash),
            }],
        }
    }

    pub(super) fn rls(version: &str, version_hash: &str, pkg_name: &str) -> Self {
        Self {
            components: vec![MockComponentBuilder {
                name: pkg_name.to_string(),
                files: mock_bin("rls", version, version_hash),
            }],
        }
    }

    pub(super) fn rust_doc() -> Self {
        let mut files: Vec<MockFile> = topical_doc_data::unique_paths()
            .map(|x| MockFile::new(x, b""))
            .collect();
        files.insert(0, MockFile::new("share/doc/rust/html/index.html", b""));
        Self {
            components: vec![MockComponentBuilder {
                name: "rust-docs".to_string(),
                files,
            }],
        }
    }

    pub(super) fn rust_analysis(trip: &str) -> Self {
        Self {
            components: vec![MockComponentBuilder {
                name: format!("rust-analysis-{trip}"),
                files: vec![MockFile::new(
                    format!("lib/rustlib/{trip}/analysis/libfoo.json"),
                    b"",
                )],
            }],
        }
    }

    pub(super) fn rust_src() -> Self {
        Self {
            components: vec![MockComponentBuilder {
                name: "rust-src".to_string(),
                files: vec![MockFile::new("lib/rustlib/src/rust-src/foo.rs", b"")],
            }],
        }
    }

    pub(super) fn combined(components: &[&Self]) -> Self {
        Self {
            components: components
                .iter()
                .flat_map(|m| m.components.clone())
                .collect(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct MockComponentBuilder {
    pub name: String,
    pub files: Vec<MockFile>,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct MockFile {
    path: String,
    contents: Contents,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum Contents {
    File(MockContents),
    Dir(Vec<(&'static str, MockContents)>),
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct MockContents {
    contents: Arc<Vec<u8>>,
    executable: bool,
}

impl std::fmt::Debug for MockContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockContents")
            .field("content_len", &self.contents.len())
            .field("executable", &self.executable)
            .finish()
    }
}

impl MockInstallerBuilder {
    pub fn build(&self, path: &Path) {
        for component in &self.components {
            // Update the components file
            let comp_file = path.join("components");
            let mut comp_file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(comp_file.clone())
                .unwrap();
            writeln!(comp_file, "{}", component.name).unwrap();

            // Create the component directory
            let component_dir = path.join(&component.name);
            if !component_dir.exists() {
                fs::create_dir(&component_dir).unwrap();
            }

            // Create the component files and manifest
            let mut manifest = File::create(component_dir.join("manifest.in")).unwrap();
            for file in component.files.iter() {
                match file.contents {
                    Contents::Dir(_) => {
                        writeln!(manifest, "dir:{}", file.path).unwrap();
                    }
                    Contents::File(_) => {
                        writeln!(manifest, "file:{}", file.path).unwrap();
                    }
                }
                file.build(&component_dir);
            }
        }

        let mut ver = File::create(path.join("rust-installer-version")).unwrap();
        writeln!(ver, "3").unwrap();
    }
}

impl MockFile {
    pub fn new<S: Into<String>>(path: S, contents: &[u8]) -> MockFile {
        MockFile::_new(path.into(), Arc::new(contents.to_vec()))
    }

    pub fn new_arc<S: Into<String>>(path: S, contents: Arc<Vec<u8>>) -> MockFile {
        MockFile::_new(path.into(), contents)
    }

    fn _new(path: String, contents: Arc<Vec<u8>>) -> MockFile {
        MockFile {
            path,
            contents: Contents::File(MockContents {
                contents,
                executable: false,
            }),
        }
    }

    pub fn new_dir(path: &str, files: &[(&'static str, &'static [u8], bool)]) -> MockFile {
        MockFile {
            path: path.to_string(),
            contents: Contents::Dir(
                files
                    .iter()
                    .map(|&(name, data, exe)| {
                        (
                            name,
                            MockContents {
                                contents: Arc::new(data.to_vec()),
                                executable: exe,
                            },
                        )
                    })
                    .collect(),
            ),
        }
    }

    pub fn executable(mut self, exe: bool) -> Self {
        if let Contents::File(c) = &mut self.contents {
            c.executable = exe;
        }
        self
    }

    pub fn build(&self, path: &Path) {
        let path = path.join(&self.path);
        match self.contents {
            Contents::Dir(ref files) => {
                for (name, contents) in files {
                    let fname = path.join(name);
                    contents.build(&fname);
                }
            }
            Contents::File(ref contents) => contents.build(&path),
        }
    }
}

impl MockContents {
    fn build(&self, path: &Path) {
        let dir_path = path.parent().unwrap().to_owned();
        fs::create_dir_all(dir_path).unwrap();
        File::create(path)
            .unwrap()
            .write_all(&self.contents)
            .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if self.executable {
                let mut perm = fs::metadata(path).unwrap().permissions();
                perm.set_mode(0o755);
                fs::set_permissions(path, perm).unwrap();
            }
        }
    }
}
