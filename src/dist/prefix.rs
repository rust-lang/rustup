use std::path::{Path, PathBuf};

const REL_MANIFEST_DIR: &str = "lib/rustlib";

#[derive(Clone, Debug)]
pub struct InstallPrefix {
    path: PathBuf,
}
impl InstallPrefix {
    pub fn from(path: PathBuf) -> Self {
        InstallPrefix { path }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn abs_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.path.join(path)
    }
    pub fn manifest_dir(&self) -> PathBuf {
        let mut path = self.path.clone();
        path.push(REL_MANIFEST_DIR);
        path
    }
    pub fn manifest_file(&self, name: &str) -> PathBuf {
        let mut path = self.manifest_dir();
        path.push(name);
        path
    }
    pub fn rel_manifest_file(&self, name: &str) -> PathBuf {
        let mut path = PathBuf::from(REL_MANIFEST_DIR);
        path.push(name);
        path
    }
}
