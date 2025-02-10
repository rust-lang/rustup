use std::path::{Path, PathBuf};

use crate::utils;

/// The relative path to the manifest directory in a Rust installation,
/// with path components separated by [`std::path::MAIN_SEPARATOR`].
const REL_MANIFEST_DIR: &str = match std::path::MAIN_SEPARATOR {
    '/' => "lib/rustlib",
    '\\' => r"lib\rustlib",
    _ => panic!("unknown `std::path::MAIN_SEPARATOR`"),
};

static V1_COMMON_COMPONENT_LIST: &[&str] = &["cargo", "rustc", "rust-docs"];

#[derive(Clone, Debug)]
pub struct InstallPrefix {
    path: PathBuf,
}
impl InstallPrefix {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn abs_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.path.join(path)
    }

    pub(crate) fn manifest_dir(&self) -> PathBuf {
        let mut path = self.path.clone();
        path.push(REL_MANIFEST_DIR);
        path
    }

    pub fn manifest_file(&self, name: &str) -> PathBuf {
        let mut path = self.manifest_dir();
        path.push(name);
        path
    }

    pub(crate) fn rel_manifest_file(&self, name: &str) -> PathBuf {
        let mut path = PathBuf::from(REL_MANIFEST_DIR);
        path.push(name);
        path
    }

    /// Guess whether this is a V1 or V2 manifest distribution.
    pub(crate) fn guess_v1_manifest(&self) -> bool {
        // If all the v1 common components are present this is likely to be
        // a v1 manifest install.  The v1 components are not called the same
        // in a v2 install.
        for component in V1_COMMON_COMPONENT_LIST {
            let manifest = format!("manifest-{component}");
            let manifest_path = self.manifest_file(&manifest);
            if !utils::path_exists(manifest_path) {
                return false;
            }
        }
        // It's reasonable to assume this is a v1 manifest installation
        true
    }
}

impl From<&Path> for InstallPrefix {
    fn from(value: &Path) -> Self {
        Self { path: value.into() }
    }
}

impl From<PathBuf> for InstallPrefix {
    fn from(path: PathBuf) -> Self {
        Self { path }
    }
}
