//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::path::Path;

use tracing::debug;

use crate::{
    config::Cfg,
    dist::{DistOptions, manifest::ManifestWithHash, prefix::InstallPrefix},
    errors::RustupError,
    toolchain::{CustomToolchainName, Toolchain, ToolchainNameExt as _, ToolchainNameOrPath},
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

        let (toolchain, prev_ver) = match &self {
            Self::Link { toolchain, .. } => {
                let toolchain = ToolchainNameOrPath::from((*toolchain).clone());
                debug!("linking toolchain `{toolchain}`");
                (toolchain, None)
            }
            Self::Dist(DistOptions {
                toolchain,
                old_manifest_date,
                ..
            }) => {
                let toolchain = (*toolchain).clone();
                let prev_ver = match old_manifest_date {
                    Some(_) => {
                        debug!("updating existing install for `{toolchain}`");
                        Some(Toolchain::new(cfg, toolchain.clone().into())?.rustc_version())
                    }
                    // A missing manifest may probably indicates an incomplete install, so don't
                    // report an old rustc version in this case.
                    None => {
                        debug!("installing toolchain `{toolchain}`");
                        None
                    }
                };
                (ToolchainNameOrPath::from(toolchain), prev_ver)
            }
        };

        let toolchain_path = &toolchain.path(cfg);
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
                    match prev_ver {
                        None => UpdateStatus::Installed,
                        Some(v) => UpdateStatus::Updated(v),
                    }
                }
            },
        };

        // Final check, to ensure we're installed
        if !toolchain.exists(cfg)? {
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
