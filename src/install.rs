//! Installation and upgrade of both distribution-managed and local
//! toolchains
use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
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
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .expect("toolchain destination must have a UTF-8 base name");
        utils::ensure_dir_exists("toolchains", parent)?;

        // The stage lives at a deterministic path so that interrupted
        // installs of the same toolchain reuse a single staging directory
        // instead of accumulating abandoned ones.
        let root = parent.join(format!("{STAGING_DIR_PREFIX}{name}"));
        utils::ensure_dir_exists("staging toolchain", &root)?;

        // The lock is held for the lifetime of the stage and released by the
        // OS if this process dies, so an abandoned stage can be reclaimed
        // safely. Blocking on (rather than failing under) contention and
        // serializing whole operations is left to the locking work tracked
        // in rust-lang/rustup#988.
        let lock = fs::File::create(root.join(STAGING_LOCK_FILE))
            .with_context(|| format!("failed to create lock file in {}", root.display()))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => bail!(
                "toolchain '{name}' is already being installed by another rustup process; \
                 wait for it to finish and retry"
            ),
            Err(fs::TryLockError::Error(error)) => {
                return Err(error).with_context(|| {
                    format!("failed to lock staging directory {}", root.display())
                });
            }
        }

        // This process owns the stage: clear anything left over from an
        // interrupted attempt so installation starts from a clean prefix.
        let prefix = root.join("toolchain");
        if utils::path_exists(&prefix) {
            utils::remove_dir("staging toolchain", &prefix)?;
        }
        utils::ensure_file_removed("staging update hash", &root.join(STAGING_HASH_FILE))?;

        Ok(Self {
            root,
            prefix,
            lock: Some(lock),
        })
    }

    fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// Path of the update hash tracked next to the stage. The hash under
    /// `$RUSTUP_HOME/update-hashes` is alias-scoped metadata and must not be
    /// touched before the staged toolchain is published.
    fn update_hash(&self) -> PathBuf {
        self.root.join(STAGING_HASH_FILE)
    }

    fn publish(self, destination: &Path) -> Result<()> {
        // Staging is a sibling of the destination inside `toolchains/`, so
        // publication must be a same-filesystem rename. Never permit the
        // copy-and-delete fallback.
        match utils::rename("toolchain", &self.prefix, destination, false) {
            Err(error) if utils::path_exists(destination) => Err(error).with_context(|| {
                format!(
                    "toolchain directory {} was created by another process \
                     while this installation was in progress",
                    destination.display()
                )
            }),
            result => result,
        }
    }
}

impl Drop for StagedToolchain {
    fn drop(&mut self) {
        // Release the lock before removing the stage: an open handle inside
        // the directory can prevent its removal on Windows.
        drop(self.lock.take());
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
        let updated = self.run(&destination, manifest).await?;

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
            // Updates modify the published toolchain in place, while fresh
            // installs are staged and only published once complete.
            InstallMethod::Dist(opts) if opts.exists => {
                let prefix = &InstallPrefix::from(path.to_owned());
                let Some(hash) = opts
                    .install_into(prefix, &opts.update_hash, manifest)
                    .await?
                else {
                    return Ok(false);
                };

                utils::write_file("update hash", &opts.update_hash, &hash)?;
                Ok(true)
            }
            InstallMethod::Dist(opts) => Self::run_staged_dist(opts, path, manifest).await,
        }
    }

    /// Constructs a fresh toolchain under a private stage and only publishes
    /// it to `destination` once complete, so that an interruption can never
    /// leave a partial toolchain behind at a selectable path.
    async fn run_staged_dist(
        opts: &DistOptions<'_, '_>,
        destination: &Path,
        manifest: Option<ManifestWithHash>,
    ) -> Result<bool> {
        let staging = StagedToolchain::new(destination)?;
        let prefix = InstallPrefix::from(staging.prefix().to_owned());
        // A stray alias-scoped update hash (e.g. left behind by an
        // interrupted uninstall) is deliberately ignored rather than removed:
        // the staged install only ever reads the staging-local hash, and the
        // alias-scoped one is rewritten after publication.
        let staging_hash = staging.update_hash();
        let Some(hash) = opts.install_into(&prefix, &staging_hash, manifest).await? else {
            return Ok(false);
        };

        #[cfg(feature = "test")]
        checkpoint(opts.cfg.process, CHECKPOINT_INSTALL_BEFORE_PUBLISH);

        staging.publish(destination)?;

        #[cfg(feature = "test")]
        checkpoint(opts.cfg.process, CHECKPOINT_INSTALL_AFTER_PUBLISH);

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

/// Prefix of staging directory names inside `toolchains/`. `+` is not valid
/// at the start of a toolchain name, so stages are identifiable and excluded
/// by the existing toolchain enumeration.
pub const STAGING_DIR_PREFIX: &str = "+rustup-staging-";

/// Lock file inside a stage, held exclusively by the owning process.
const STAGING_LOCK_FILE: &str = "lock";

/// Update hash file inside a stage (see [`StagedToolchain::update_hash`]).
const STAGING_HASH_FILE: &str = "update-hash";

/// Checkpoint reached once a staged toolchain is complete, right before its
/// publication.
#[cfg(feature = "test")]
pub const CHECKPOINT_INSTALL_BEFORE_PUBLISH: &str = "install-before-publish";

/// Checkpoint reached right after publication, before the alias-scoped
/// update hash is written.
#[cfg(feature = "test")]
pub const CHECKPOINT_INSTALL_AFTER_PUBLISH: &str = "install-after-publish";

struct StagedToolchain {
    root: PathBuf,
    prefix: PathBuf,
    /// Exclusive lock marking the stage as owned by a live process; `None`
    /// only while dropping.
    lock: Option<fs::File>,
}
