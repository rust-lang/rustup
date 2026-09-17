use std::{
    env::consts::EXE_SUFFIX,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, SystemTime},
};

use anyhow::Context;
use tracing::warn;

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
            // The file exists only to be locked; never touch its contents.
            .truncate(false)
            .open(directory.join(SELF_UPDATE_LOCK_FILE))
            .context("failed to open self-update lock")?;

        Ok(Self {
            directory,
            _file: file,
        })
    }

    /// Clears the previous update's leftovers and reserves the managed updater path.
    pub(super) fn prepare_updater(self) -> anyhow::Result<PreparedUpdater> {
        let path = self.updater_path();
        utils::ensure_file_removed("self-updater", &path)?;
        for marker in [Marker::Complete, Marker::Failed] {
            utils::ensure_file_removed("self-update status marker", &marker.path(&self.directory))?;
        }
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

    fn updater_path(&self) -> PathBuf {
        self.directory.join(format!("rustup-init{EXE_SUFFIX}"))
    }
}

/// The managed updater path, held together with the lock that protects it.
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
        let stage = self
            .path
            .parent()
            .context("self-updater path has no parent directory")?;
        Command::new(&self.path)
            .env(STAGE_ENV, stage)
            .arg("--self-replace")
            .spawn()
            .with_context(|| format!("unable to run updater ({})", self.path.display()))
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

pub(super) fn mark_result(succeeded: bool, process: &Process) {
    let Some(stage) = process.var_os(STAGE_ENV).map(PathBuf::from) else {
        return;
    };
    let marker = if succeeded {
        Marker::Complete
    } else {
        Marker::Failed
    };
    if let Err(error) = marker.record(process, &stage) {
        warn!("could not record self-update result: {error}");
    }
}

pub(super) fn cleanup(process: &Process) -> anyhow::Result<()> {
    cleanup_at(process, SystemTime::now())
}

fn cleanup_at(process: &Process, now: SystemTime) -> anyhow::Result<()> {
    if let Some(lock) = SelfUpdateLock::try_acquire(process)? {
        let updater = lock.updater_path();
        // The replacer records an outcome only once it has finished. An unmarked
        // updater may still be about to run, or its replacer may have died before
        // recording anything; only its age tells those two cases apart.
        let markers = [Marker::Complete, Marker::Failed];
        let finished = markers
            .iter()
            .any(|marker| marker.path(&lock.directory).is_file());
        if (finished || is_stale(&updater, now))
            && utils::remove_file_best_effort("self-updater", &updater)
        {
            for marker in markers {
                utils::remove_file_best_effort(
                    "self-update status marker",
                    &marker.path(&lock.directory),
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
        utils::remove_file_best_effort("legacy self-updater", &updater);
    }

    Ok(())
}

fn is_stale(path: &Path, now: SystemTime) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| now.duration_since(modified).ok())
        .is_some_and(|age| age >= ABANDONED_UPDATE_AGE)
}

/// Outcome recorded next to the managed updater once replacement has finished.
#[derive(Clone, Copy)]
enum Marker {
    Complete,
    Failed,
}

impl Marker {
    /// Records this outcome in `stage`, ignoring stages outside the managed directory.
    fn record(self, process: &Process, stage: &Path) -> anyhow::Result<()> {
        if stage != stage_root(process)? {
            warn!(
                "ignoring self-update stage outside the managed directory: {}",
                stage.display()
            );
            return Ok(());
        }

        utils::write_file("self-update status marker", &self.path(stage), "")
    }

    fn path(self, stage: &Path) -> PathBuf {
        stage.join(self.as_str())
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }
}

fn stage_root(process: &Process) -> anyhow::Result<PathBuf> {
    Ok(process.rustup_home()?.join(SELF_UPDATE_DIRECTORY))
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
        fs::write(Marker::Complete.path(&stage), "").unwrap();
        drop(first);
        let second = SelfUpdateLock::acquire(&process.process)
            .unwrap()
            .prepare_updater()
            .unwrap();

        assert_eq!(&first_path, second.path());
        assert!(!Marker::Complete.path(&stage).exists());
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
        fs::write(Marker::Complete.path(&lock.directory), "").unwrap();

        cleanup_at(&process.process, SystemTime::now()).unwrap();

        assert!(updater.exists());
        drop(lock);
        cleanup_at(&process.process, SystemTime::now()).unwrap();
        assert!(!updater.exists());
    }

    #[tokio::test]
    async fn spawn_replacer_rejects_parentless_path() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let prepared_updater = PreparedUpdater {
            path: PathBuf::new(),
            _lock: SelfUpdateLock::acquire(&process.process).unwrap(),
        };
        let error = prepared_updater.spawn_replacer().err().unwrap();

        assert_eq!(
            error.to_string(),
            "self-updater path has no parent directory"
        );
    }

    #[tokio::test]
    async fn cleanup_keeps_fresh_updater() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let prepared_updater = SelfUpdateLock::acquire(&process.process)
            .unwrap()
            .prepare_updater()
            .unwrap();
        let updater = prepared_updater.path().to_owned();
        fs::write(&updater, "").unwrap();
        drop(prepared_updater);

        cleanup_at(&process.process, SystemTime::now()).unwrap();

        assert!(updater.exists());
    }

    #[tokio::test]
    async fn cleanup_removes_finished_updater() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let stage = stage_root(&process.process).unwrap();

        for marker in [Marker::Complete, Marker::Failed] {
            let prepared_updater = SelfUpdateLock::acquire(&process.process)
                .unwrap()
                .prepare_updater()
                .unwrap();
            let updater = prepared_updater.path().to_owned();
            fs::write(&updater, "").unwrap();
            drop(prepared_updater);
            marker.record(&process.process, &stage).unwrap();

            cleanup_at(&process.process, SystemTime::now()).unwrap();

            assert!(!updater.exists());
            assert!(!marker.path(&stage).exists());
        }
    }

    #[tokio::test]
    async fn cleanup_removes_abandoned_updater() {
        let root = test_dir().unwrap();
        let process = test_process(root.path());
        let prepared_updater = SelfUpdateLock::acquire(&process.process)
            .unwrap()
            .prepare_updater()
            .unwrap();
        let updater = prepared_updater.path().to_owned();
        fs::write(&updater, "").unwrap();
        drop(prepared_updater);

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
