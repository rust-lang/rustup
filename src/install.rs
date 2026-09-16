//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::path::Path;

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
        toolchain: &'a CustomToolchainName,
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
        let cfg = match self {
            Self::Link { cfg, .. } | Self::Dist(DistOptions { cfg, .. }) => cfg,
        };

        // Initialize rayon for use by the remove_dir_all crate limiting the number of threads.
        // This will error if rayon is already initialized but it's fine to ignore that.
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(cfg.process.io_thread_count()?.into())
            .build_global();

        let toolchain = match &self {
            Self::Link { toolchain, .. } => {
                let toolchain = LocalToolchainName::from((*toolchain).clone());
                debug!("linking toolchain `{toolchain}`");
                toolchain
            }
            Self::Dist(DistOptions {
                toolchain,
                old_date_version,
                ..
            }) => {
                let toolchain = LocalToolchainName::from((*toolchain).clone());
                match old_date_version {
                    Some(_) => debug!("updating existing install for `{toolchain}`"),
                    None => debug!("installing toolchain `{toolchain}`"),
                }
                toolchain
            }
        };

        let toolchain_path = &cfg.toolchain_path(&toolchain);
        debug!("toolchain directory: {}", toolchain_path.display());
        if toolchain_path.exists() && !matches!(self, Self::Dist { .. }) {
            uninstall(toolchain_path)?;
        }

        let status = match &self {
            Self::Link { src, .. } => {
                utils::symlink_dir(src, toolchain_path)?;
                UpdateStatus::Installed
            }
            Self::Dist(opts) => match opts
                .install_into(&InstallPrefix::from(toolchain_path.clone()), manifest)
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
        if !Toolchain::exists(cfg, &toolchain)? {
            return Err(RustupError::ToolchainNotInstallable(toolchain.to_string()).into());
        }

        match &status {
            UpdateStatus::Unchanged => debug!("toolchain is already up to date"),
            _ => debug!("toolchain {toolchain} installed"),
        };

        Ok(status)
    }
}

pub(crate) fn uninstall(path: &Path) -> anyhow::Result<()> {
    utils::remove_dir("install", path)
}
