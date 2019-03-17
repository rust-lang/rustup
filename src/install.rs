//! Installation and upgrade of both distribution-managed and local
//! toolchains

use crate::dist::component::{Components, Package, TarGzPackage, Transaction};
use crate::dist::dist;
use crate::dist::download::DownloadCfg;
use crate::dist::prefix::InstallPrefix;
use crate::dist::temp;
use crate::dist::Notification;
use crate::errors::Result;
use crate::utils::utils;
use crate::Verbosity;
use log::info;
use std::path::Path;

#[derive(Copy, Clone)]
pub enum InstallMethod<'a> {
    Copy(&'a Path),
    Link(&'a Path),
    Installer(&'a Path, &'a temp::Cfg),
    // bool is whether to force an update
    Dist(
        &'a dist::ToolchainDesc,
        Option<&'a Path>,
        DownloadCfg<'a>,
        bool,
    ),
}

impl<'a> InstallMethod<'a> {
    pub fn run(
        self,
        path: &Path,
        verbosity: Verbosity,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> Result<bool> {
        if path.exists() {
            // Don't uninstall first for Dist method
            match self {
                InstallMethod::Dist(..) | InstallMethod::Installer(..) => {}
                _ => uninstall(path, verbosity)?,
            }
        }

        match self {
            InstallMethod::Copy(src) => {
                utils::copy_dir(src, path, verbosity)?;
                Ok(true)
            }
            InstallMethod::Link(src) => {
                utils::symlink_dir(src, &path, verbosity)?;
                Ok(true)
            }
            InstallMethod::Installer(src, temp_cfg) => {
                InstallMethod::tar_gz(src, path, &temp_cfg, notify_handler)?;
                Ok(true)
            }
            InstallMethod::Dist(toolchain, update_hash, dl_cfg, force_update) => {
                let prefix = &InstallPrefix::from(path.to_owned());
                let maybe_new_hash = dist::update_from_dist(
                    dl_cfg,
                    update_hash,
                    toolchain,
                    prefix,
                    &[],
                    &[],
                    force_update,
                )?;

                if let Some(hash) = maybe_new_hash {
                    if let Some(hash_file) = update_hash {
                        utils::write_file("update hash", hash_file, &hash)?;
                    }

                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    fn tar_gz(
        src: &Path,
        path: &Path,
        temp_cfg: &temp::Cfg,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> Result<()> {
        info!("extracting...");

        let prefix = InstallPrefix::from(path.to_owned());
        let installation = Components::open(prefix.clone())?;
        let package = TarGzPackage::new_file(src, temp_cfg)?;

        let mut tx = Transaction::new(prefix.clone(), temp_cfg, notify_handler);

        for component in package.components() {
            tx = package.install(&installation, &component, None, tx)?;
        }

        tx.commit();

        Ok(())
    }
}

pub fn uninstall(path: &Path, verbosity: Verbosity) -> Result<()> {
    Ok(utils::remove_dir("install", path, verbosity)?)
}
