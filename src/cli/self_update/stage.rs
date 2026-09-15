use std::{
    env::consts::EXE_SUFFIX,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

use anyhow::Context;
use tracing::{debug, warn};

use super::install_proxies;
use crate::{process::Process, utils};

/// Exclusive right to download the updater or replace the installed rustup.
pub(super) struct SelfUpdateLock {
    directory: PathBuf,
    _file: File,
}

impl SelfUpdateLock {
    pub(super) fn acquire(process: &Process) -> anyhow::Result<Self> {
        let lock = Self::open(process)?;
        lock._file.lock().context("failed to lock self-update")?;
        Ok(lock)
    }

    fn try_acquire(process: &Process) -> anyhow::Result<Option<Self>> {
        let lock = Self::open(process)?;
        match lock._file.try_lock() {
            Ok(()) => Ok(Some(lock)),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(error)) => Err(error).context("failed to lock self-update"),
        }
    }

    fn open(process: &Process) -> anyhow::Result<Self> {
        let directory = stage_root(process)?;
        utils::ensure_dir_exists("self-update", &directory)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(SELF_UPDATE_LOCK_FILE))
            .context("failed to open self-update lock")?;

        Ok(Self {
            directory,
            _file: file,
        })
    }

    /// Clears the previous update's leftovers and reserves the managed updater path.
    pub(super) fn prepare_updater(self) -> anyhow::Result<PreparedUpdate> {
        let updater_path = self.updater_path();
        utils::ensure_file_removed("self-updater", &updater_path)?;
        for marker in [Marker::Complete, Marker::Failed] {
            utils::ensure_file_removed(
                "self-update status marker",
                &self.directory.join(marker.as_str()),
            )?;
        }
        Ok(PreparedUpdate {
            updater_path,
            _lock: self,
        })
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

    fn updater_path(&self) -> PathBuf {
        self.directory.join(format!("rustup-init{EXE_SUFFIX}"))
    }
}

/// The managed updater path, held together with the lock that protects it.
pub(super) struct PreparedUpdate {
    updater_path: PathBuf,
    _lock: SelfUpdateLock,
}

impl PreparedUpdate {
    pub(super) fn replacer_command(&self) -> anyhow::Result<Command> {
        let stage = self
            .updater_path
            .parent()
            .context("self-updater path has no parent directory")?;
        let mut command = Command::new(&self.updater_path);
        command.env(STAGE_ENV, stage);
        Ok(command)
    }

    pub(super) fn updater_path(&self) -> &Path {
        &self.updater_path
    }
}

pub(super) fn mark_result(process: &Process, succeeded: bool) {
    let Some(stage) = process.var_os(STAGE_ENV).map(PathBuf::from) else {
        return;
    };
    let marker = if succeeded {
        Marker::Complete
    } else {
        Marker::Failed
    };
    if let Err(error) = mark_stage(process, &stage, marker) {
        warn!("could not record self-update result: {error}");
    }
}

pub(super) fn cleanup(process: &Process) -> anyhow::Result<()> {
    cleanup_at(process, SystemTime::now())
}

fn mark_stage(process: &Process, stage: &Path, marker: Marker) -> anyhow::Result<()> {
    if stage != stage_root(process)? {
        warn!(
            "ignoring self-update stage outside the managed directory: {}",
            stage.display()
        );
        return Ok(());
    }

    utils::write_file(
        "self-update status marker",
        &stage.join(marker.as_str()),
        "",
    )
}

fn cleanup_at(process: &Process, now: SystemTime) -> anyhow::Result<()> {
    if let Some(lock) = SelfUpdateLock::try_acquire(process)? {
        let updater = lock.updater_path();
        if (is_finished(&lock.directory) || is_stale(&updater, now))
            && remove_file_best_effort("self-updater", &updater)
        {
            for marker in [Marker::Complete, Marker::Failed] {
                remove_file_best_effort(
                    "self-update status marker",
                    &lock.directory.join(marker.as_str()),
                );
            }
        }
    }

    let updater = process
        .cargo_home()?
        .join(format!("bin/rustup-init{EXE_SUFFIX}"));
    // Legacy updaters have no result marker, and an older rustup process may
    // still own the shared path.
    if is_stale(&updater, now) {
        remove_file_best_effort("legacy self-updater", &updater);
    }

    Ok(())
}

fn remove_file_best_effort(name: &str, path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => {
            debug!(path = %path.display(), "removed {name}");
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => true,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::PermissionDenied | io::ErrorKind::ResourceBusy
            ) =>
        {
            debug!(path = %path.display(), "leaving busy {name}");
            false
        }
        Err(error) => {
            warn!("could not remove {name} {}: {error}", path.display());
            false
        }
    }
}

fn is_finished(stage: &Path) -> bool {
    [Marker::Complete, Marker::Failed]
        .iter()
        .any(|marker| stage.join(marker.as_str()).is_file())
}

fn is_stale(path: &Path, now: SystemTime) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| now.duration_since(modified).ok())
        .is_some_and(|age| age >= ABANDONED_UPDATE_AGE)
}

fn stage_root(process: &Process) -> anyhow::Result<PathBuf> {
    Ok(process.rustup_home()?.join(SELF_UPDATE_DIRECTORY))
}

/// Outcome recorded next to the managed updater once replacement has finished.
#[derive(Clone, Copy)]
enum Marker {
    Complete,
    Failed,
}

impl Marker {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }
}

const SELF_UPDATE_DIRECTORY: &str = "self-update";
const SELF_UPDATE_LOCK_FILE: &str = "self-update.lock";
const STAGE_ENV: &str = "RUSTUP_SELF_UPDATE_STAGE";
const ABANDONED_UPDATE_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{
        process::TestProcess,
        test::{Env, test_dir},
    };

    #[tokio::test]
    async fn updater_path_is_stable() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let first = SelfUpdateLock::acquire(&process.process).unwrap();
        let first_path = first.updater_path();
        let stage = first.directory.clone();
        fs::write(&first_path, "").unwrap();
        fs::write(stage.join(Marker::Complete.as_str()), "").unwrap();
        drop(first);
        let second = SelfUpdateLock::acquire(&process.process)
            .unwrap()
            .prepare_updater()
            .unwrap();

        assert_eq!(&first_path, second.updater_path());
        assert!(!stage.join(Marker::Complete.as_str()).exists());
    }

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

    #[tokio::test]
    async fn cleanup_keeps_locked_updater() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let lock = SelfUpdateLock::acquire(&process.process).unwrap();
        let updater = lock.updater_path();
        fs::write(&updater, "").unwrap();
        fs::write(lock.directory.join(Marker::Complete.as_str()), "").unwrap();

        cleanup_at(&process.process, SystemTime::now()).unwrap();

        assert!(updater.exists());
        drop(lock);
        cleanup_at(&process.process, SystemTime::now()).unwrap();
        assert!(!updater.exists());
    }

    #[tokio::test]
    async fn replacer_command_rejects_parentless_path() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let prepared_update = PreparedUpdate {
            updater_path: PathBuf::new(),
            _lock: SelfUpdateLock::acquire(&process.process).unwrap(),
        };
        let error = prepared_update.replacer_command().err().unwrap();

        assert_eq!(
            error.to_string(),
            "self-updater path has no parent directory"
        );
    }

    #[tokio::test]
    async fn cleanup_keeps_fresh_updater() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let prepared_update = SelfUpdateLock::acquire(&process.process)
            .unwrap()
            .prepare_updater()
            .unwrap();
        let updater = prepared_update.updater_path().to_owned();
        fs::write(&updater, "").unwrap();
        drop(prepared_update);

        cleanup_at(&process.process, SystemTime::now()).unwrap();

        assert!(updater.exists());
    }

    #[tokio::test]
    async fn cleanup_removes_finished_updater() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let stage = stage_root(&process.process).unwrap();

        for marker in [Marker::Complete, Marker::Failed] {
            let prepared_update = SelfUpdateLock::acquire(&process.process)
                .unwrap()
                .prepare_updater()
                .unwrap();
            let updater = prepared_update.updater_path().to_owned();
            fs::write(&updater, "").unwrap();
            drop(prepared_update);
            mark_stage(&process.process, &stage, marker).unwrap();

            cleanup_at(&process.process, SystemTime::now()).unwrap();

            assert!(!updater.exists());
            assert!(!stage.join(marker.as_str()).exists());
        }
    }

    #[tokio::test]
    async fn cleanup_removes_abandoned_updater() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let prepared_update = SelfUpdateLock::acquire(&process.process)
            .unwrap()
            .prepare_updater()
            .unwrap();
        let updater = prepared_update.updater_path().to_owned();
        fs::write(&updater, "").unwrap();
        drop(prepared_update);

        cleanup_at(
            &process.process,
            SystemTime::now() + ABANDONED_UPDATE_AGE + Duration::from_secs(1),
        )
        .unwrap();

        assert!(!updater.exists());
    }

    #[tokio::test]
    async fn cleanup_delays_removing_legacy_updater() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let updater = root
            .path()
            .join(format!("cargo/bin/rustup-init{EXE_SUFFIX}"));
        fs::create_dir_all(updater.parent().unwrap()).unwrap();
        fs::write(&updater, "").unwrap();

        cleanup_at(&process.process, SystemTime::now()).unwrap();
        assert!(updater.exists());

        cleanup_at(
            &process.process,
            SystemTime::now() + ABANDONED_UPDATE_AGE + Duration::from_secs(1),
        )
        .unwrap();
        assert!(!updater.exists());
    }

    fn test_process(root: &Path) -> TestProcess {
        let mut vars = HashMap::new();
        vars.env("HOME", root);
        vars.env("CARGO_HOME", root.join("cargo"));
        vars.env("RUSTUP_HOME", root.join("rustup"));
        TestProcess::with_vars(vars)
    }
}
