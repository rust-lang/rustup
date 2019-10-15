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
use std::path::Path;

#[derive(Copy, Clone)]
pub enum InstallMethod<'a> {
    Copy(&'a Path),
    Link(&'a Path),
    Installer(&'a Path, &'a temp::Cfg),
    // bool is whether to force an update
    Dist(
        &'a dist::ToolchainDesc,
        Option<dist::Profile>,
        Option<&'a Path>,
        DownloadCfg<'a>,
        // --force
        bool,
        // toolchain already exists
        bool,
        // currently installed date
        Option<&'a str>,
        // Extra components to install from dist
        &'a [&'a str],
        // Extra targets to install from dist
        &'a [&'a str],
    ),
}

impl<'a> InstallMethod<'a> {
    pub fn run(self, path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<bool> {
        if path.exists() {
            // Don't uninstall first for Dist method
            match self {
                InstallMethod::Dist(..) | InstallMethod::Installer(..) => {}
                _ => {
                    uninstall(path, notify_handler)?;
                }
            }
        }

        match self {
            InstallMethod::Copy(src) => {
                utils::copy_dir(src, path, notify_handler)?;
                Ok(true)
            }
            InstallMethod::Link(src) => {
                utils::symlink_dir(src, &path, notify_handler)?;
                Ok(true)
            }
            InstallMethod::Installer(src, temp_cfg) => {
                InstallMethod::tar_gz(src, path, &temp_cfg, notify_handler)?;
                Ok(true)
            }
            InstallMethod::Dist(
                toolchain,
                profile,
                update_hash,
                dl_cfg,
                force_update,
                exists,
                old_date,
                components,
                targets,
            ) => {
                let prefix = &InstallPrefix::from(path.to_owned());
                let maybe_new_hash = dist::update_from_dist(
                    dl_cfg,
                    update_hash,
                    toolchain,
                    if exists { None } else { profile },
                    prefix,
                    force_update,
                    old_date,
                    components,
                    targets,
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
        notify_handler(Notification::Extracting(src, path));

        let prefix = InstallPrefix::from(path.to_owned());
        let installation = Components::open(prefix.clone())?;
        let notification_converter = |notification: crate::utils::Notification<'_>| {
            notify_handler(notification.into());
        };
        let reader = utils::FileReaderWithProgress::new_file(&src, &notification_converter)?;
        let package: &dyn Package =
            &TarGzPackage::new(reader, temp_cfg, Some(&notification_converter))?;

        let mut tx = Transaction::new(prefix, temp_cfg, notify_handler);

        for component in package.components() {
            tx = package.install(&installation, &component, None, tx)?;
        }

        tx.commit();

        Ok(())
    }
}

pub fn uninstall(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<()> {
    utils::remove_dir("install", path, notify_handler)
}
