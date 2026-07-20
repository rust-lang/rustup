//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tracing::{debug, warn};

use crate::{
    config::Cfg,
    dist::{DistOptions, manifest::ManifestWithHash, prefix::InstallPrefix},
    errors::RustupError,
    toolchain::{CustomToolchainName, LocalToolchainName, Toolchain},
    utils,
};

#[cfg(feature = "test")]
use crate::test::checkpoint;

impl StagedToolchain {
    fn new(destination: &Path) -> Result<Self> {
        let parent = destination
            .parent()
            .expect("toolchain destination must have a parent");
        utils::ensure_dir_exists("toolchains", parent)?;

        loop {
            let root = parent.join(format!(
                "{STAGING_DIR_PREFIX}{}",
                utils::raw::random_string(16)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    let prefix = root.join("toolchain");
                    return Ok(Self { root, prefix });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| RustupError::CreatingDirectory {
                        name: "staging toolchain",
                        path: root,
                    });
                }
            }
        }
    }

    fn prefix(&self) -> &Path {
        &self.prefix
    }

    fn publish(self, destination: &Path) -> Result<()> {
        // Staging is a child of the destination directory, so publication must
        // be a same-filesystem rename. Never permit the copy-and-delete fallback.
        utils::rename("toolchain", &self.prefix, destination, false)
    }
}

impl Drop for StagedToolchain {
    fn drop(&mut self) {
        if utils::path_exists(&self.root)
            && let Err(error) = utils::remove_dir("staging toolchain", &self.root)
        {
            warn!(
                path = %self.root.display(),
                "could not remove staging toolchain: {error}"
            );
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum UpdateStatus {
    Installed,
    Updated(String), // Stores the version of rustc *before* the update
    Unchanged,
}

pub(crate) enum InstallMethod<'cfg, 'a> {
    Copy {
        src: &'a Path,
        dest: &'a CustomToolchainName,
        cfg: &'cfg Cfg<'cfg>,
    },
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
    pub(crate) async fn install(self, manifest: Option<ManifestWithHash>) -> Result<UpdateStatus> {
        // Initialize rayon for use by the remove_dir_all crate limiting the number of threads.
        // This will error if rayon is already initialized but it's fine to ignore that.
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(self.cfg().process.io_thread_count()?.into())
            .build_global();
        match &self {
            InstallMethod::Copy { .. }
            | InstallMethod::Link { .. }
            | InstallMethod::Dist(DistOptions {
                old_date_version: None,
                ..
            }) => debug!("installing toolchain {}", self.dest_basename()),
            _ => debug!("updating existing install for '{}'", self.dest_basename()),
        }

        let destination = self.dest_path();
        debug!("toolchain directory: {}", destination.display());
        let updated = match &self {
            InstallMethod::Dist(DistOptions { exists: false, .. }) => {
                self.run_staged_dist(&destination, manifest).await?
            }
            _ => self.run(&destination, manifest).await?,
        };

        let status = match updated {
            false => {
                debug!("toolchain is already up to date");
                UpdateStatus::Unchanged
            }
            true => {
                debug!("toolchain {} installed", self.dest_basename());
                match &self {
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

    async fn run(&self, path: &Path, manifest: Option<ManifestWithHash>) -> Result<bool> {
        if path.exists() {
            // Don't uninstall first for Dist method
            match self {
                InstallMethod::Dist { .. } => {}
                _ => {
                    uninstall(path)?;
                }
            }
        }

        match self {
            InstallMethod::Copy { src, .. } => {
                utils::copy_dir(src, path)?;
                Ok(true)
            }
            InstallMethod::Link { src, .. } => {
                utils::symlink_dir(src, path)?;
                Ok(true)
            }
            InstallMethod::Dist(opts) => {
                let prefix = &InstallPrefix::from(path.to_owned());
                let maybe_new_hash = opts
                    .install_into(prefix, &opts.update_hash, manifest)
                    .await?;

                if let Some(hash) = maybe_new_hash {
                    utils::write_file("update hash", &opts.update_hash, &hash)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    async fn run_staged_dist(
        &self,
        destination: &Path,
        manifest: Option<ManifestWithHash>,
    ) -> Result<bool> {
        let InstallMethod::Dist(opts) = self else {
            unreachable!("only distribution installs can be staged");
        };

        if opts.update_hash.exists() {
            warn!(
                "removing stray hash file in order to continue: {}",
                opts.update_hash.display()
            );
        }

        let staging = StagedToolchain::new(destination)?;
        let prefix = InstallPrefix::from(staging.prefix().to_owned());
        // Installation must not touch alias-scoped metadata before the object
        // is published, including a stale update hash from an earlier attempt.
        let staging_hash = staging.root.join("update-hash");
        let Some(hash) = opts.install_into(&prefix, &staging_hash, manifest).await? else {
            return Ok(false);
        };

        #[cfg(feature = "test")]
        checkpoint(opts.cfg.process, "install-before-publish");

        staging.publish(destination)?;

        #[cfg(feature = "test")]
        checkpoint(opts.cfg.process, "install-after-publish");

        utils::write_file("update hash", &opts.update_hash, &hash)?;
        Ok(true)
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
            InstallMethod::Copy { dest, .. } | InstallMethod::Link { dest, .. } => {
                (*dest).clone().into()
            }
            InstallMethod::Dist(DistOptions {
                toolchain: desc, ..
            }) => (*desc).clone().into(),
        }
    }

    fn dest_basename(&self) -> String {
        self.local_name().to_string()
    }

    fn dest_path(&self) -> PathBuf {
        match self {
            InstallMethod::Copy { cfg, dest, .. } | InstallMethod::Link { cfg, dest, .. } => {
                cfg.toolchain_path(&(*dest).clone().into())
            }
            InstallMethod::Dist(DistOptions {
                cfg,
                toolchain: desc,
                ..
            }) => cfg.toolchain_path(&(*desc).clone().into()),
        }
    }
}

pub(crate) fn uninstall(path: &Path) -> Result<()> {
    utils::remove_dir("install", path)
}

// `+` is not valid at the start of a toolchain name, so abandoned stages are
// identifiable and excluded by the existing toolchain enumeration.
const STAGING_DIR_PREFIX: &str = "+rustup-staging-";

struct StagedToolchain {
    root: PathBuf,
    prefix: PathBuf,
}
