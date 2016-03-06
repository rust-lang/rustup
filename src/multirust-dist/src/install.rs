use utils;
use errors::*;
use temp;
use dist;
use component::{Components, TarGzPackage, Transaction, Package};

use std::path::{Path, PathBuf};
use std::env;

const REL_MANIFEST_DIR: &'static str = "lib/rustlib";

#[derive(Clone, Debug)]
pub struct InstallPrefix {
    path: PathBuf,
}
#[derive(Debug)]
pub enum InstallMethod<'a> {
    Copy(&'a Path),
    Link(&'a Path),
    Installer(&'a Path, &'a temp::Cfg),
    Dist(&'a str, Option<&'a Path>, dist::DownloadCfg<'a>),
}

impl<'a> InstallMethod<'a> {
    pub fn run(self, prefix: &InstallPrefix, notify_handler: NotifyHandler) -> Result<()> {
        if prefix.is_installed_here() {
            // Don't uninstall first for Dist method
            match self {
                InstallMethod::Dist(_, _, _) |
                InstallMethod::Installer(_, _) => {}
                _ => {
                    try!(prefix.uninstall(notify_handler));
                }
            }
        }

        match self {
            InstallMethod::Copy(src) => {
                try!(utils::copy_dir(src, &prefix.path, ntfy!(&notify_handler)));
                Ok(())
            }
            InstallMethod::Link(src) => {
                try!(utils::symlink_dir(src, &prefix.path, ntfy!(&notify_handler)));
                Ok(())
            }
            InstallMethod::Installer(src, temp_cfg) => {
                InstallMethod::tar_gz(src, prefix, &temp_cfg, notify_handler)
            }
            InstallMethod::Dist(toolchain, update_hash, dl_cfg) => {
                let maybe_new_hash =
                    try!(dist::update_from_dist(
                        dl_cfg,
                        update_hash,
                        toolchain,
                        prefix,
                        &[], &[]));

                if let Some(hash) = maybe_new_hash {
                    if let Some(hash_file) = update_hash {
                        try!(utils::write_file("update hash", hash_file, &hash));
                    }
                }

                Ok(())
            }
        }
    }

    fn tar_gz(src: &Path, prefix: &InstallPrefix, temp_cfg: &temp::Cfg,
              notify_handler: NotifyHandler) -> Result<()> {
        notify_handler.call(Notification::Extracting(src, prefix.path()));

        let installation = try!(Components::open(prefix.clone()));
        let package = try!(TarGzPackage::new_file(src, temp_cfg));

        let mut tx = Transaction::new(prefix.clone(), temp_cfg, notify_handler);

        for component in package.components() {
            tx = try!(package.install(&installation, &component, None, tx));
        }

        tx.commit();

        Ok(())
    }
}

pub fn bin_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from("bin");
    path.push(name.to_owned() + env::consts::EXE_SUFFIX);
    path
}

impl InstallPrefix {
    pub fn from(path: PathBuf) -> Self {
        InstallPrefix {
            path: path,
        }
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
    pub fn binary_file(&self, name: &str) -> PathBuf {
        let mut path = self.path.clone();
        path.push(bin_path(name));
        path
    }
    pub fn doc_path(&self, relative: &str) -> Result<PathBuf> {
        let parts = vec!["share", "doc", "rust", "html"];
        let mut doc_dir = self.path.clone();
        for part in parts {
            doc_dir.push(part);
        }
        doc_dir.push(relative);

        Ok(doc_dir)
    }
    pub fn is_installed_here(&self) -> bool {
        utils::is_directory(&self.path)
    }
    pub fn uninstall(&self, notify_handler: NotifyHandler) -> Result<()> {
        if self.is_installed_here() {
            Ok(try!(utils::remove_dir("install", &self.path, ntfy!(&notify_handler))))
        } else {
            Err(Error::NotInstalledHere)
        }
    }
    pub fn install(&self, method: InstallMethod, notify_handler: NotifyHandler) -> Result<()> {
        method.run(self, notify_handler)
    }

    pub fn open_docs(&self, relative: &str) -> Result<()> {
        Ok(try!(utils::open_browser(&try!(self.doc_path(relative)))))
    }

    pub fn components(&self) -> Result<Components> {
        Components::open(self.clone())
    }
}
