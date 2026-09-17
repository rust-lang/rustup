use std::{
    env::consts::EXE_SUFFIX,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    process::{Child, Command},
};

use anyhow::Context;

use super::install_proxies;
use crate::{process::Process, utils};

/// Exclusive right to download the updater or replace the installed rustup.
pub(super) struct SelfUpdateLock {
    _file: File,
}

impl SelfUpdateLock {
    pub(super) fn acquire(process: &Process) -> anyhow::Result<Self> {
        let lock = Self::open(process)?;
        lock._file.lock().context("failed to lock self-update")?;
        Ok(lock)
    }

    fn open(process: &Process) -> anyhow::Result<Self> {
        let directory = stage_root(process)?;
        utils::ensure_dir_exists("self-update", &directory)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            // The file exists only to be locked; never touch its contents.
            .truncate(false)
            .open(directory.join(SELF_UPDATE_LOCK_FILE))
            .context("failed to open self-update lock")?;

        Ok(Self { _file: file })
    }

    /// Removes any leftover updater and reserves its path for the download.
    pub(super) fn prepare_updater(self, process: &Process) -> anyhow::Result<PreparedUpdater> {
        let path = process
            .cargo_home()?
            .join(format!("bin/rustup-init{EXE_SUFFIX}"));
        utils::ensure_file_removed("self-updater", &path)?;
        Ok(PreparedUpdater { path, _lock: self })
    }

    /// Installs the running executable as `$CARGO_HOME/bin/rustup` and refreshes its proxies.
    pub(super) fn install_bins(&self, process: &Process) -> anyhow::Result<()> {
        let bin_path = process.cargo_home()?.join("bin");
        let this_exe_path = utils::current_exe()?;
        let rustup_path = bin_path.join(format!("rustup{EXE_SUFFIX}"));

        utils::ensure_dir_exists("bin", &bin_path)?;
        // NB: Even on Linux we can't just copy the new binary over the (running)
        // old binary; we must unlink it first.
        if rustup_path.exists() {
            utils::remove_file("rustup-bin", &rustup_path)?;
        }
        utils::copy_file_symlink_to_source(&this_exe_path, &rustup_path)?;
        utils::make_executable(&rustup_path)?;
        install_proxies(process)
    }
}

/// The updater path, held together with the lock that protects it.
pub(super) struct PreparedUpdater {
    path: PathBuf,
    _lock: SelfUpdateLock,
}

impl PreparedUpdater {
    /// Starts the updater in `--self-replace` mode and then releases the lock.
    ///
    /// The lock must be held until the child has been spawned: a concurrent
    /// self-update could otherwise replace the updater before it is executed.
    pub(super) fn spawn_replacer(self) -> anyhow::Result<Child> {
        Command::new(&self.path)
            .arg("--self-replace")
            .spawn()
            .with_context(|| format!("unable to run updater ({})", self.path.display()))
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

fn stage_root(process: &Process) -> anyhow::Result<PathBuf> {
    Ok(process.rustup_home()?.join(SELF_UPDATE_DIRECTORY))
}

const SELF_UPDATE_DIRECTORY: &str = "self-update";
const SELF_UPDATE_LOCK_FILE: &str = "self-update.lock";

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs};

    use super::*;
    use crate::{
        process::TestProcess,
        test::{Env, test_dir},
    };

    #[tokio::test]
    async fn self_update_lock_is_global() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let lock = SelfUpdateLock::acquire(&process.process).unwrap();
        let contender = OpenOptions::new()
            .read(true)
            .write(true)
            .open(
                stage_root(&process.process)
                    .unwrap()
                    .join(SELF_UPDATE_LOCK_FILE),
            )
            .unwrap();

        assert!(matches!(
            contender.try_lock(),
            Err(fs::TryLockError::WouldBlock)
        ));
        drop(lock);
        contender.try_lock().unwrap();
    }

    fn test_process(root: &Path) -> TestProcess {
        let mut vars = HashMap::new();
        vars.env("HOME", root);
        vars.env("CARGO_HOME", root.join("cargo"));
        vars.env("RUSTUP_HOME", root.join("rustup"));
        TestProcess::with_vars(vars)
    }
}
