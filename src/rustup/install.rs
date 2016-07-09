//! Installation and upgrade of both distribution-managed and local
//! toolchains

use rustup_dist::{Notification};
use rustup_dist::prefix::InstallPrefix;
use rustup_utils::utils;
use rustup_dist::temp;
use rustup_dist::dist;
use rustup_dist::component::{Components, TarGzPackage, Transaction, Package};
use errors::Result;
use std::path::Path;

#[derive(Copy, Clone)]
pub enum InstallMethod<'a> {
    Copy(&'a Path),
    Link(&'a Path),
    Installer(&'a Path, &'a temp::Cfg),
    Dist(&'a dist::ToolchainDesc, Option<&'a Path>, dist::DownloadCfg<'a>),
}

impl<'a> InstallMethod<'a> {
    pub fn run(self, path: &Path, notify_handler: &Fn(Notification)) -> Result<bool> {
        if path.exists() {
            // Don't uninstall first for Dist method
            match self {
                InstallMethod::Dist(_, _, _) |
                InstallMethod::Installer(_, _) => {}
                _ => {
                    try!(uninstall(path, notify_handler));
                }
            }
        }

        match self {
            InstallMethod::Copy(src) => {
                try!(utils::copy_dir(src, path, &|n| notify_handler(n.into())));
                Ok(true)
            }
            InstallMethod::Link(src) => {
                try!(utils::symlink_dir(src, &path, &|n| notify_handler(n.into())));
                Ok(true)
            }
            InstallMethod::Installer(src, temp_cfg) => {
                try!(InstallMethod::tar_gz(src, path, &temp_cfg, notify_handler));
                Ok(true)
            }
            InstallMethod::Dist(toolchain, update_hash, dl_cfg) => {
                let prefix = &InstallPrefix::from(path.to_owned());
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

                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    fn tar_gz(src: &Path, path: &Path, temp_cfg: &temp::Cfg,
              notify_handler: &Fn(Notification)) -> Result<()> {
        notify_handler(Notification::Extracting(src, path));

        let prefix = InstallPrefix::from(path.to_owned());
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

pub fn uninstall(path: &Path, notify_handler: &Fn(Notification)) -> Result<()> {
    Ok(try!(utils::remove_dir("install", path,
                              &|n| notify_handler(n.into()))))
}
