//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    config::Cfg,
    dist::{dist, download::DownloadCfg, prefix::InstallPrefix, Notification},
    errors::RustupError,
    notifications::Notification as RootNotification,
    toolchain::{
        names::{CustomToolchainName, LocalToolchainName},
        toolchain::Toolchain,
    },
    utils::utils,
};

#[derive(Clone, Debug)]
pub(crate) enum UpdateStatus {
    Installed,
    Updated(String), // Stores the version of rustc *before* the update
    Unchanged,
}

#[derive(Clone)]
pub(crate) enum InstallMethod<'a> {
    Copy {
        src: &'a Path,
        dest: &'a CustomToolchainName,
        cfg: &'a Cfg,
    },
    Link {
        src: &'a Path,
        dest: &'a CustomToolchainName,
        cfg: &'a Cfg,
    },
    Dist {
        cfg: &'a Cfg,
        desc: &'a dist::ToolchainDesc,
        profile: dist::Profile,
        update_hash: Option<&'a Path>,
        dl_cfg: DownloadCfg<'a>,
        /// --force bool is whether to force an update/install
        force: bool,
        /// --allow-downgrade
        allow_downgrade: bool,
        /// toolchain already exists
        exists: bool,
        /// currently installed date and version
        old_date_version: Option<(String, String)>,
        /// Extra components to install from dist
        components: &'a [&'a str],
        /// Extra targets to install from dist
        targets: &'a [&'a str],
    },
}

impl<'a> InstallMethod<'a> {
    // Install a toolchain
    #[cfg_attr(feature = "otel", tracing::instrument(err, skip_all))]
    pub(crate) fn install(&self) -> Result<UpdateStatus> {
        let nh = self.cfg().notify_handler.clone();
        match self {
            InstallMethod::Copy { .. }
            | InstallMethod::Link { .. }
            | InstallMethod::Dist {
                old_date_version: None,
                ..
            } => (nh)(RootNotification::InstallingToolchain(&self.dest_basename())),
            _ => (nh)(RootNotification::UpdatingToolchain(&self.dest_basename())),
        }

        (self.cfg().notify_handler)(RootNotification::ToolchainDirectory(
            &self.dest_path(),
            &self.dest_basename(),
        ));
        let updated = self.run(&self.dest_path(), &|n| {
            (self.cfg().notify_handler)(n.into())
        })?;

        let status = match updated {
            false => {
                (nh)(RootNotification::UpdateHashMatches);
                UpdateStatus::Unchanged
            }
            true => {
                (nh)(RootNotification::InstalledToolchain(&self.dest_basename()));
                match self {
                    InstallMethod::Dist {
                        old_date_version: Some((_, v)),
                        ..
                    } => UpdateStatus::Updated(v.clone()),
                    InstallMethod::Copy { .. }
                    | InstallMethod::Link { .. }
                    | InstallMethod::Dist { .. } => UpdateStatus::Installed,
                }
            }
        };

        // Final check, to ensure we're installed
        match Toolchain::exists(self.cfg(), &self.local_name())? {
            true => Ok(status),
            false => Err(RustupError::ToolchainNotInstallable(self.dest_basename()).into()),
        }
    }

    fn run(&self, path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<bool> {
        if path.exists() {
            // Don't uninstall first for Dist method
            match self {
                InstallMethod::Dist { .. } => {}
                _ => {
                    uninstall(path, notify_handler)?;
                }
            }
        }

        match self {
            InstallMethod::Copy { src, .. } => {
                utils::copy_dir(src, path, notify_handler)?;
                Ok(true)
            }
            InstallMethod::Link { src, .. } => {
                utils::symlink_dir(src, path, notify_handler)?;
                Ok(true)
            }
            InstallMethod::Dist {
                desc,
                profile,
                update_hash,
                dl_cfg,
                force: force_update,
                allow_downgrade,
                exists,
                old_date_version,
                components,
                targets,
                ..
            } => {
                let prefix = &InstallPrefix::from(path.to_owned());
                let maybe_new_hash = dist::update_from_dist(
                    *dl_cfg,
                    update_hash.as_deref(),
                    desc,
                    if *exists { None } else { Some(*profile) },
                    prefix,
                    *force_update,
                    *allow_downgrade,
                    old_date_version.as_ref().map(|dv| dv.0.as_str()),
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

    fn cfg(&self) -> &Cfg {
        match self {
            InstallMethod::Copy { cfg, .. } => cfg,
            InstallMethod::Link { cfg, .. } => cfg,
            InstallMethod::Dist { cfg, .. } => cfg,
        }
    }

    fn local_name(&self) -> LocalToolchainName {
        match self {
            InstallMethod::Copy { dest, .. } => (*dest).into(),
            InstallMethod::Link { dest, .. } => (*dest).into(),
            InstallMethod::Dist { desc, .. } => (*desc).into(),
        }
    }

    fn dest_basename(&self) -> String {
        self.local_name().to_string()
    }

    fn dest_path(&self) -> PathBuf {
        match self {
            InstallMethod::Copy { cfg, dest, .. } => cfg.toolchain_path(&(*dest).into()),
            InstallMethod::Link { cfg, dest, .. } => cfg.toolchain_path(&(*dest).into()),
            InstallMethod::Dist { cfg, desc, .. } => cfg.toolchain_path(&(*desc).into()),
        }
    }
}

pub(crate) fn uninstall(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<()> {
    utils::remove_dir("install", path, notify_handler)
}
