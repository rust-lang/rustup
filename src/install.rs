//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::path::{Path, PathBuf};

use tracing::debug;

use crate::{
    config::Cfg,
    dist::{DistOptions, manifest::ManifestWithHash, prefix::InstallPrefix},
    errors::RustupError,
    toolchain::{CustomToolchainName, LocalToolchainName, Toolchain},
    utils,
};

#[derive(Clone, Debug)]
pub(crate) enum UpdateStatus {
    Installed,
    Updated(String), // Stores the version of rustc *before* the update
    Unchanged,
}

pub(crate) enum InstallMethod<'cfg, 'a> {
    Link {
        src: &'a Path,
        dest: &'a CustomToolchainName,
        cfg: &'cfg Cfg<'cfg>,
    },
    Dist(DistOptions<'cfg, 'a>),
}

impl InstallMethod<'_, '_> {
    // Install a toolchain
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn install(
        self,
        manifest: Option<ManifestWithHash>,
    ) -> anyhow::Result<UpdateStatus> {
        // Initialize rayon for use by the remove_dir_all crate limiting the number of threads.
        // This will error if rayon is already initialized but it's fine to ignore that.
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(self.cfg().process.io_thread_count()?.into())
            .build_global();
        let local_name = self.local_name();
        match &self {
            InstallMethod::Link { .. }
            | InstallMethod::Dist(DistOptions {
                old_date_version: None,
                ..
            }) => debug!("installing toolchain {local_name}",),
            _ => debug!("updating existing install for '{local_name}'"),
        }

        let dest_path = &self.dest_path();
        debug!("toolchain directory: {}", dest_path.display());
        if dest_path.exists() && !matches!(self, Self::Dist { .. }) {
            uninstall(dest_path)?;
        }

        let updated = match &self {
            Self::Link { src, .. } => {
                utils::symlink_dir(src, dest_path)?;
                true
            }
            Self::Dist(opts) => {
                let prefix = &InstallPrefix::from(dest_path.clone());
                let maybe_new_hash = opts.install_into(prefix, manifest).await?;

                if let Some(hash) = maybe_new_hash {
                    utils::write_file("update hash", &opts.update_hash, &hash)?;
                    true
                } else {
                    false
                }
            }
        };

        let status = match updated {
            false => {
                debug!("toolchain is already up to date");
                UpdateStatus::Unchanged
            }
            true => {
                debug!("toolchain {local_name} installed");
                match &self {
                    InstallMethod::Dist(DistOptions {
                        old_date_version: Some((_, v)),
                        ..
                    }) => UpdateStatus::Updated(v.clone()),
                    InstallMethod::Link { .. } | InstallMethod::Dist { .. } => {
                        UpdateStatus::Installed
                    }
                }
            }
        };

        // Final check, to ensure we're installed
        match Toolchain::exists(self.cfg(), &local_name)? {
            true => Ok(status),
            false => Err(RustupError::ToolchainNotInstallable(local_name.to_string()).into()),
        }
    }

    fn cfg(&self) -> &Cfg<'_> {
        match self {
            InstallMethod::Link { cfg, .. } => cfg,
            InstallMethod::Dist(DistOptions { cfg, .. }) => cfg,
        }
    }

    fn local_name(&self) -> LocalToolchainName {
        match self {
            InstallMethod::Link { dest, .. } => (*dest).clone().into(),
            InstallMethod::Dist(DistOptions {
                toolchain: desc, ..
            }) => (*desc).clone().into(),
        }
    }

    fn dest_path(&self) -> PathBuf {
        self.cfg().toolchain_path(&self.local_name())
    }
}

pub(crate) fn uninstall(path: &Path) -> anyhow::Result<()> {
    utils::remove_dir("install", path)
}
