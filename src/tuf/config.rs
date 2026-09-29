use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::anyhow;
use chrono::{DateTime, NaiveDate, Utc};
use tokio::sync::{MappedMutexGuard, Mutex, MutexGuard};
use tracing::{trace, warn};

use crate::{
    download::DownloadOptions,
    process::Process,
    tuf::{
        TufRepository,
        consts::{DEFAULT_HOME_DIR, DEFAULT_SERVER},
    },
};

/// TUF-related settings, resolved from the `RUSTUP_TUF_*` environment variables.
#[derive(Debug)]
pub struct TufConfig {
    /// The TUF repository for dist files and rustup's own updates, a URL or
    /// a local directory (`RUSTUP_TUF_SERVER`).
    pub server: String,
    /// A local `root.json` to use instead of the one shipped with rustup
    /// (`RUSTUP_TUF_ROOT`).
    pub root: Option<PathBuf>,
    /// Directory holding the local copy of the TUF repository and key files
    /// (`RUSTUP_TUF_HOME`, default `<RUSTUP_HOME>/tuf`).
    pub home: PathBuf,
    /// Whether, and how strictly, TUF validation runs
    /// (`RUSTUP_TUF_ENABLE`, default `off`).
    pub mode: TufMode,
    /// Ignore all validation failures, including metadata expiry
    /// (`RUSTUP_TUF_IGNORE=1`).
    pub ignore_failures: bool,
    /// Ignore metadata expiry failures whose expiry is after this instant
    /// (`RUSTUP_TUF_IGNOREDATE`).
    pub ignore_expiry_after: Option<DateTime<Utc>>,
    /// The repository itself, opened on first use by [`TufConfig::repository`].
    pub(super) repository: LazyTufRepository,
}

impl TufConfig {
    /// Reads every `RUSTUP_TUF_*` variable from `process`.
    ///
    /// `rustup_dir` is the resolved `RUSTUP_HOME`, used for the default
    /// [`TufConfig::home`]. Unparsable values fall back to their defaults.
    pub fn from_env(rustup_dir: &Path, process: &Process) -> Self {
        let server = match process.var("RUSTUP_TUF_SERVER") {
            Ok(url) => {
                trace!("`RUSTUP_TUF_SERVER` has been set to `{url}`");
                url
            }
            Err(_) => DEFAULT_SERVER.to_owned(),
        };

        let root = process
            .var("RUSTUP_TUF_ROOT")
            .inspect(|path| trace!("`RUSTUP_TUF_ROOT` has been set to `{path}`"))
            .ok()
            .map(PathBuf::from);

        let home = match process.var("RUSTUP_TUF_HOME") {
            Ok(path) => {
                trace!("`RUSTUP_TUF_HOME` has been set to `{path}`");
                PathBuf::from(path)
            }
            Err(_) => rustup_dir.join(DEFAULT_HOME_DIR),
        };

        let mode = match process.var("RUSTUP_TUF_ENABLE") {
            Ok(value) => {
                trace!("`RUSTUP_TUF_ENABLE` has been set to `{value}`");
                value.parse().unwrap_or_else(|err| {
                    warn!("{err}, TUF validation stays off");
                    TufMode::Off
                })
            }
            Err(_) => TufMode::Off,
        };

        let ignore_failures = process.var("RUSTUP_TUF_IGNORE").is_ok_and(|s| s == "1");

        let ignore_expiry_after = process
            .var("RUSTUP_TUF_IGNOREDATE")
            .ok()
            .and_then(|s| parse_date_time(&s));

        Self {
            server,
            root,
            home,
            mode,
            ignore_failures,
            ignore_expiry_after,
            repository: LazyTufRepository::default(),
        }
    }

    /// Returns the repository, opening and verifying it on first use.
    ///
    /// `options` drive the downloads the repository makes for its own
    /// metadata. The guard holds the repository's lock, so keep it short.
    #[tracing::instrument(level = "trace", err(level = "trace"), skip_all)]
    pub(crate) async fn repository(
        &self,
        options: DownloadOptions,
    ) -> anyhow::Result<MappedMutexGuard<'_, TufRepository>> {
        let mut guard = self.repository.0.lock().await;
        let repository = match guard.take() {
            Some(repository) => repository,
            None => {
                let mut repository = TufRepository::open(self, options).await?;
                repository.verify().await?;
                repository
            }
        };
        Ok(MutexGuard::map(guard, |slot| slot.insert(repository)))
    }

    pub(crate) fn enabled(&self) -> bool {
        self.mode != TufMode::Off
    }
}

impl PartialEq for TufConfig {
    fn eq(&self, other: &Self) -> bool {
        self.server == other.server
            && self.root == other.root
            && self.home == other.home
            && self.mode == other.mode
            && self.ignore_failures == other.ignore_failures
            && self.ignore_expiry_after == other.ignore_expiry_after
    }
}

impl Eq for TufConfig {}

/// The slot [`TufConfig::repository`] fills on first use.
#[derive(Default)]
pub(crate) struct LazyTufRepository(Mutex<Option<TufRepository>>);

impl fmt::Debug for LazyTufRepository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let open = self.0.try_lock().map(|slot| slot.is_some()).ok();
        f.debug_struct("LazyTufRepository")
            .field("open", &open)
            .finish()
    }
}

/// How TUF validation behaves.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TufMode {
    /// Do not touch the TUF repository at all.
    #[default]
    Off,
    /// Synchronize the TUF repository and validate signatures, failing on error.
    On,
    /// Synchronize the TUF repository over the network but skip signature
    /// validation, only reporting what would have failed.
    Warn,
}

impl TufMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::Warn => "warn",
        }
    }
}

impl FromStr for TufMode {
    type Err = anyhow::Error;

    fn from_str(mode: &str) -> anyhow::Result<Self> {
        Ok(match mode.to_ascii_lowercase().as_str() {
            "on" | "true" | "1" => Self::On,
            "warn" => Self::Warn,
            "off" | "false" | "0" => Self::Off,
            _ => {
                return Err(anyhow!(
                    "invalid TUF mode `{mode}`, expected `on`, `warn` or `off`"
                ));
            }
        })
    }
}

impl fmt::Display for TufMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parses either an RFC 3339 timestamp (`2026-09-16T12:00:00Z`) or a bare
/// `YYYY-MM-DD` date, which is taken as midnight UTC.
pub(super) fn parse_date_time(value: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Some(dt.with_timezone(&Utc));
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc())
}
