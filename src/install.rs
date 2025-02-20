//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    config::Cfg,
    dist::{self, DistOptions, Notification, prefix::InstallPrefix},
    errors::RustupError,
    notifications::Notification as RootNotification,
    toolchain::{CustomToolchainName, LocalToolchainName, Toolchain},
    utils,
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
        cfg: &'a Cfg<'a>,
    },
    Link {
        src: &'a Path,
        dest: &'a CustomToolchainName,
        cfg: &'a Cfg<'a>,
    },
    Dist(DistOptions<'a>),
}

impl InstallMethod<'_> {
    // Install a toolchain
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn install(&self) -> Result<UpdateStatus> {
        let nh = &self.cfg().notify_handler;
        match self {
            InstallMethod::Copy { .. }
            | InstallMethod::Link { .. }
            | InstallMethod::Dist(DistOptions {
                old_date_version: None,
                ..
            }) => nh(RootNotification::InstallingToolchain(&self.dest_basename())),
            _ => nh(RootNotification::UpdatingToolchain(&self.dest_basename())),
        }

        nh(RootNotification::ToolchainDirectory(&self.dest_path()));
        let updated = self.run(&self.dest_path(), &|n| nh(n.into())).await?;

        let status = match updated {
            false => {
                nh(RootNotification::UpdateHashMatches);
                UpdateStatus::Unchanged
            }
            true => {
                nh(RootNotification::InstalledToolchain(&self.dest_basename()));
                match self {
                    InstallMethod::Dist(DistOptions {
                        old_date_version: Some((_, v)),
                        ..
                    }) => UpdateStatus::Updated(v.clone()),
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

    async fn run(&self, path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<bool> {
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
            InstallMethod::Dist(opts) => {
                let prefix = &InstallPrefix::from(path.to_owned());
                let maybe_new_hash = dist::update_from_dist(prefix, opts).await?;

                if let Some(hash) = maybe_new_hash {
                    if let Some(hash_file) = opts.update_hash {
                        utils::write_file("update hash", hash_file, &hash)?;
                    }

                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    fn cfg(&self) -> &Cfg<'_> {
        match self {
            InstallMethod::Copy { cfg, .. } => cfg,
            InstallMethod::Link { cfg, .. } => cfg,
            InstallMethod::Dist(DistOptions { cfg, .. }) => cfg,
        }
    }

    fn local_name(&self) -> LocalToolchainName {
        match self {
            InstallMethod::Copy { dest, .. } => (*dest).into(),
            InstallMethod::Link { dest, .. } => (*dest).into(),
            InstallMethod::Dist(DistOptions {
                toolchain: desc, ..
            }) => (*desc).into(),
        }
    }

    fn dest_basename(&self) -> String {
        self.local_name().to_string()
    }

    fn dest_path(&self) -> PathBuf {
        match self {
            InstallMethod::Copy { cfg, dest, .. } => cfg.toolchain_path(&(*dest).into()),
            InstallMethod::Link { cfg, dest, .. } => cfg.toolchain_path(&(*dest).into()),
            InstallMethod::Dist(DistOptions {
                cfg,
                toolchain: desc,
                ..
            }) => cfg.toolchain_path(&(*desc).into()),
        }
    }
}

pub(crate) fn uninstall(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<()> {
    utils::remove_dir("install", path, notify_handler)
}
