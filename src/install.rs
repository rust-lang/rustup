//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::path::Path;

use tracing::debug;

use crate::{
    config::Cfg,
    dist::{DistOptions, manifest::ManifestWithHash, prefix::InstallPrefix},
    errors::RustupError,
    toolchain::{CustomToolchainName, Toolchain},
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

        let local_name = match &self {
            Self::Link { dest, .. } => {
                let name = (*dest).clone().into();
                debug!("linking toolchain {name}");
                name
            }
            Self::Dist(DistOptions {
                toolchain: desc,
                old_date_version,
                ..
            }) => {
                let name = (*desc).clone().into();
                if old_date_version.is_some() {
                    debug!("updating existing install for '{name}'");
                } else {
                    debug!("installing toolchain {name}");
                }
                name
            }
        };

        let dest_path = &self.cfg().toolchain_path(&local_name);
        debug!("toolchain directory: {}", dest_path.display());
        if dest_path.exists() && !matches!(self, Self::Dist { .. }) {
            uninstall(dest_path)?;
        }

        let status = match &self {
            Self::Link { src, .. } => {
                utils::symlink_dir(src, dest_path)?;
                UpdateStatus::Installed
            }
            Self::Dist(opts) => match opts
                .install_into(&InstallPrefix::from(dest_path.clone()), manifest)
                .await?
            {
                None => UpdateStatus::Unchanged,
                Some(hash) => {
                    utils::write_file("update hash", &opts.update_hash, &hash)?;
                    match opts {
                        DistOptions {
                            old_date_version: Some((_, v)),
                            ..
                        } => UpdateStatus::Updated(v.clone()),
                        _ => UpdateStatus::Installed,
                    }
                }
            },
        };

        // Final check, to ensure we're installed
        if !Toolchain::exists(self.cfg(), &local_name)? {
            return Err(RustupError::ToolchainNotInstallable(local_name.to_string()).into());
        }

        match &status {
            UpdateStatus::Unchanged => debug!("toolchain is already up to date"),
            _ => debug!("toolchain {local_name} installed"),
        };

        Ok(status)
    }

    fn cfg(&self) -> &Cfg<'_> {
        match self {
            InstallMethod::Link { cfg, .. } => cfg,
            InstallMethod::Dist(DistOptions { cfg, .. }) => cfg,
        }
    }
}

pub(crate) fn uninstall(path: &Path) -> anyhow::Result<()> {
    utils::remove_dir("install", path)
}
