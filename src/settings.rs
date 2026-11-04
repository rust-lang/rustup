use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt,
    fs::OpenOptions,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    cli::self_update::SelfUpdateMode,
    dist::{Profile, Switch},
    errors::RustupError,
    utils,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsFile {
    pub(crate) path: PathBuf,
    cache: RefCell<Option<Settings>>,
}

impl SettingsFile {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            cache: RefCell::new(None),
        }
    }

    pub(crate) fn with<T, F: FnOnce(&Settings) -> anyhow::Result<T>>(
        &self,
        f: F,
    ) -> anyhow::Result<T> {
        if self.cache.borrow().is_none() {
            *self.cache.borrow_mut() = Some(self.read_settings()?);
        }

        // Settings can no longer be None so it's OK to unwrap
        f(self.cache.borrow().as_ref().unwrap())
    }

    pub(crate) fn with_mut<T, F: FnOnce(&mut Settings) -> anyhow::Result<T>>(
        &self,
        f: F,
    ) -> anyhow::Result<T> {
        // Serialize the whole read-modify-write, including creation. This is a
        // separate, persistent file because the I/O helpers lock settings.toml
        // themselves. Never remove it: waiters must all lock the same file.
        let lock_path = self.path.with_extension("toml.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| RustupError::WritingFile {
                name: "settings lock",
                path: lock_path.clone(),
            })?;
        utils::raw::lock(&lock)
            .with_context(|| format!("could not lock settings file '{}'", lock_path.display()))?;

        // A cached read may predate another process's update. Only publish the
        // new cache after both the mutation and persistence have succeeded.
        let mut settings = self.read_settings()?;
        let result = f(&mut settings)?;
        utils::write_locked_file("settings", &self.path, &settings.stringify()?)?;
        *self.cache.borrow_mut() = Some(settings);
        Ok(result)
    }

    fn read_settings(&self) -> anyhow::Result<Settings> {
        if !utils::is_file(&self.path) {
            return Ok(Settings::default());
        }
        let content = utils::read_locked_file("settings", &self.path)?;
        Settings::parse(&content).with_context(|| RustupError::ParsingFile {
            name: "settings",
            path: self.path.clone(),
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Settings {
    pub version: MetadataVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "default_host_triple")]
    pub default_host_tuple: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_toolchain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<Profile>,
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pgp_keys: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_self_update: Option<SelfUpdateMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_install: Option<Switch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_hint: Option<Switch>,
}

impl Settings {
    fn path_to_key(path: &Path) -> String {
        if path.exists() {
            utils::canonicalize_path(path).display().to_string()
        } else {
            path.display().to_string()
        }
    }

    pub(crate) fn remove_override(&mut self, path: &Path) -> bool {
        let key = Self::path_to_key(path);
        self.overrides.remove(&key).is_some()
    }

    pub(crate) fn add_override(&mut self, path: &Path, toolchain: String) {
        let key = Self::path_to_key(path);
        info!(
            "override toolchain for {} set to {toolchain}",
            path.display(),
        );
        self.overrides.insert(key, toolchain);
    }

    pub(crate) fn dir_override(&self, dir: &Path) -> Option<String> {
        let key = Self::path_to_key(dir);
        self.overrides.get(&key).cloned()
    }

    pub(crate) fn parse(data: &str) -> anyhow::Result<Self> {
        toml::from_str(data).context("error parsing settings")
    }

    fn stringify(&self) -> anyhow::Result<String> {
        Ok(toml::to_string(self)?)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum MetadataVersion {
    #[serde(rename = "2")]
    V2,
    #[serde(rename = "12")]
    #[default]
    V12,
}

impl MetadataVersion {
    fn as_str(&self) -> &'static str {
        match self {
            Self::V2 => "2",
            Self::V12 => "12",
        }
    }
}

impl FromStr for MetadataVersion {
    type Err = RustupError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "2" => Ok(Self::V2),
            "12" => Ok(Self::V12),
            _ => Err(RustupError::UnknownMetadataVersion(s.to_owned())),
        }
    }
}

impl fmt::Display for MetadataVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufRead, BufReader, Read, Write},
        process::{Child, Command, Stdio},
        sync::mpsc::{self, Receiver},
        thread,
        time::Duration,
    };

    use wait_timeout::ChildExt;

    use super::*;

    #[test]
    fn stale_cache_preserves_other_writers_settings() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.toml");
        let first = SettingsFile::new(path.clone());
        let second = SettingsFile::new(path.clone());
        first.with(|s| {
            assert_eq!(s.profile, None);
            Ok(())
        })?;

        second.with_mut(|s| {
            s.profile = Some(Profile::Minimal);
            Ok(())
        })?;
        first.with_mut(|s| {
            s.default_toolchain = Some("stable".into());
            Ok(())
        })?;

        SettingsFile::new(path).with(|s| {
            assert_eq!(s.default_toolchain.as_deref(), Some("stable"));
            assert_eq!(s.profile, Some(Profile::Minimal));
            Ok(())
        })
    }

    #[test]
    fn concurrent_processes_preserve_both_updates() -> anyhow::Result<()> {
        for contents in [Some(BASIC), None] {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("settings.toml");
            if let Some(contents) = contents {
                fs::write(&path, contents)?;
            }
            let mut first = SettingsProcess::spawn(&path, "profile")?;
            let mut second = SettingsProcess::spawn(&path, "toolchain")?;
            first.receive("ready");
            second.receive("ready");

            first.send()?;
            first.receive("mutating");
            second.send()?;
            // The second process must observe the transaction lock while the
            // first is inside its mutation, before either writer can finish.
            second.receive("locked");
            first.send()?;
            first.receive("done");
            second.receive("mutating");
            second.send()?;
            second.receive("done");
            first.finish()?;
            second.finish()?;

            SettingsFile::new(path).with(|s| {
                assert_eq!(s.default_toolchain.as_deref(), Some("nightly"));
                assert_eq!(s.profile, Some(Profile::Minimal));
                Ok(())
            })?;
        }
        Ok(())
    }

    #[test]
    fn reads_remain_cached_without_creating_files() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.toml");
        let settings = SettingsFile::new(path.clone());
        settings.with(|s| {
            assert_eq!(s, &Settings::default());
            settings.with(|nested| {
                assert_eq!(nested, s);
                Ok(())
            })
        })?;
        assert_eq!(fs::read_dir(dir.path())?.count(), 0);

        fs::write(&path, BASIC)?;
        settings.with(|s| {
            assert_eq!(s.profile, None);
            Ok(())
        })?;
        SettingsFile::new(path).with(|s| {
            assert_eq!(s.profile, Some(Profile::Default));
            Ok(())
        })
    }

    #[test]
    fn sequential_updates_refresh_the_cache_and_truncate() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.toml");
        fs::write(&path, BASIC)?;
        let settings = SettingsFile::new(path.clone());
        assert_eq!(
            settings.with_mut(|s| {
                s.profile = Some(Profile::Minimal);
                Ok(42)
            })?,
            42
        );
        settings.with_mut(|s| {
            s.default_toolchain = None;
            Ok(())
        })?;
        settings.with(|s| {
            assert_eq!(s.default_toolchain, None);
            assert_eq!(s.profile, Some(Profile::Minimal));
            Ok(())
        })?;
        assert_eq!(
            fs::read_to_string(&path)?,
            "version = \"12\"\nprofile = \"minimal\"\n\n[overrides]\n"
        );
        Ok(())
    }

    #[test]
    fn failed_mutation_preserves_file_and_cache() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.toml");
        fs::write(&path, BASIC)?;
        let settings = SettingsFile::new(path.clone());
        settings.with(|_| Ok(()))?;
        let result: anyhow::Result<()> = settings.with_mut(|s| {
            s.profile = Some(Profile::Minimal);
            anyhow::bail!("mutation failed")
        });
        assert_eq!(result.unwrap_err().to_string(), "mutation failed");
        assert_eq!(fs::read_to_string(&path)?, BASIC);
        settings.with(|s| {
            assert_eq!(s.profile, Some(Profile::Default));
            Ok(())
        })?;
        SettingsFile::new(path).with_mut(|s| {
            assert_eq!(s.profile, Some(Profile::Default));
            s.default_toolchain = None;
            Ok(())
        })
    }

    #[test]
    fn invalid_settings_are_not_overwritten_by_a_cached_writer() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.toml");
        fs::write(&path, BASIC)?;
        let settings = SettingsFile::new(path.clone());
        settings.with(|_| Ok(()))?;
        for invalid in ["", "invalid toml", "version = \"999\""] {
            fs::write(&path, invalid)?;
            assert!(
                settings
                    .with_mut::<(), _>(|_| panic!("must not mutate invalid settings"))
                    .is_err()
            );
            assert_eq!(fs::read_to_string(&path)?, invalid);
        }
        fs::write(&path, BASIC)?;
        settings.with_mut(|_| Ok(()))?;
        assert_eq!(fs::read_to_string(&path)?, BASIC);
        Ok(())
    }

    #[test]
    fn lock_open_failure_preserves_settings() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.toml");
        fs::write(&path, BASIC)?;
        fs::create_dir(path.with_extension("toml.lock"))?;
        let settings = SettingsFile::new(path.clone());
        assert!(
            settings
                .with_mut::<(), _>(|_| panic!("must not mutate without a lock"))
                .is_err()
        );
        assert_eq!(fs::read_to_string(&path)?, BASIC);
        fs::remove_dir(path.with_extension("toml.lock"))?;
        settings.with_mut(|_| Ok(()))
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn rejected_write_preserves_settings() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("settings.toml");
        fs::write(&path, BASIC)?;
        let mut child = SettingsProcess::spawn(&path, "write_failure")?;
        child.receive("done");
        child.finish()
    }

    #[test]
    #[ignore = "subprocess helper for settings concurrency tests"]
    fn settings_writer_process() -> anyhow::Result<()> {
        let path = std::env::var_os("RUSTUP_TEST_SETTINGS_PATH").unwrap();
        let field = std::env::var("RUSTUP_TEST_SETTINGS_FIELD")?;
        let settings = SettingsFile::new(path.into());
        settings.with(|_| Ok(()))?;
        #[cfg(target_os = "linux")]
        if field == "write_failure" {
            reject_settings_write(&settings)?;
            println!("settings-test:done");
            return Ok(());
        }
        println!("settings-test:ready");
        std::io::stdin().read_exact(&mut [0])?;
        if field == "toolchain" {
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(settings.path.with_extension("toml.lock"))?;
            assert!(matches!(
                utils::raw::try_lock(&lock),
                Err(fs::TryLockError::WouldBlock)
            ));
            println!("settings-test:locked");
        }
        settings.with_mut(|s| {
            println!("settings-test:mutating");
            std::io::stdin().read_exact(&mut [0])?;
            if field == "profile" {
                s.profile = Some(Profile::Minimal);
            } else {
                s.default_toolchain = Some("nightly".into());
            }
            Ok(())
        })?;
        println!("settings-test:done");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn reject_settings_write(settings: &SettingsFile) -> anyhow::Result<()> {
        let mut original = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: The rlimit pointers are valid. These process-wide changes run
        // only in the dedicated child, and SIG_IGN is a valid signal disposition.
        unsafe {
            assert_eq!(libc::getrlimit(libc::RLIMIT_FSIZE, &mut original), 0);
            assert_ne!(libc::signal(libc::SIGXFSZ, libc::SIG_IGN), libc::SIG_ERR);
            let limit = libc::rlimit {
                rlim_cur: 0,
                ..original
            };
            assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &limit), 0);
        }
        let result = settings.with_mut(|s| {
            s.profile = Some(Profile::Minimal);
            Ok(())
        });
        // SAFETY: Restore the limit obtained above using a valid rlimit pointer.
        unsafe { assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &original), 0) };
        assert_eq!(
            result
                .unwrap_err()
                .root_cause()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(libc::EFBIG)
        );
        assert_eq!(fs::read_to_string(&settings.path)?, BASIC);
        settings.with(|s| {
            assert_eq!(s.profile, Some(Profile::Default));
            Ok(())
        })?;
        settings.with_mut(|s| {
            s.profile = Some(Profile::Minimal);
            Ok(())
        })?;
        SettingsFile::new(settings.path.clone()).with(|s| {
            assert_eq!(s.profile, Some(Profile::Minimal));
            Ok(())
        })
    }

    struct SettingsProcess {
        child: Child,
        messages: Receiver<String>,
    }

    impl SettingsProcess {
        fn spawn(path: &Path, field: &str) -> anyhow::Result<Self> {
            let mut child = Command::new(std::env::current_exe()?)
                .args([
                    "--exact",
                    "settings::tests::settings_writer_process",
                    "--ignored",
                    "--nocapture",
                ])
                .env("RUSTUP_TEST_SETTINGS_PATH", path)
                .env("RUSTUP_TEST_SETTINGS_FIELD", field)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;
            let stdout = child.stdout.take().unwrap();
            let (sender, messages) = mpsc::channel();
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if let Some((_, message)) = line.split_once("settings-test:") {
                        let _ = sender.send(message.to_owned());
                    }
                }
            });
            Ok(Self { child, messages })
        }

        fn send(&mut self) -> anyhow::Result<()> {
            Ok(self.child.stdin.as_mut().unwrap().write_all(b"x")?)
        }

        fn receive(&self, expected: &str) {
            assert_eq!(
                self.messages.recv_timeout(Duration::from_secs(30)).unwrap(),
                expected,
            );
        }

        fn finish(&mut self) -> anyhow::Result<()> {
            assert!(
                self.child
                    .wait_timeout(Duration::from_secs(30))?
                    .expect("settings writer did not exit")
                    .success()
            );
            Ok(())
        }
    }

    impl Drop for SettingsProcess {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[test]
    fn serialize_default() {
        let settings = Settings::default();
        let toml = settings.stringify().unwrap();
        assert_eq!(
            toml,
            r#"version = "12"

[overrides]
"#
        );
    }

    #[test]
    fn deserialize_default() {
        let toml = r#"version = "12""#;
        let settings = Settings::parse(toml).unwrap();
        assert_eq!(settings.version, MetadataVersion::V12);
    }

    #[test]
    fn serialize_basic() {
        let settings = Settings {
            version: MetadataVersion::V12,
            default_toolchain: Some("stable-aarch64-apple-darwin".to_owned()),
            profile: Some(Profile::Default),
            ..Default::default()
        };

        let toml = settings.stringify().unwrap();
        assert_eq!(toml, BASIC);
    }

    #[test]
    fn deserialize_basic() {
        let settings = Settings::parse(BASIC).unwrap();
        assert_eq!(settings.version, MetadataVersion::V12);
        assert_eq!(
            settings.default_toolchain,
            Some("stable-aarch64-apple-darwin".to_owned())
        );
        assert_eq!(settings.profile, Some(Profile::Default));
    }

    const BASIC: &str = r#"version = "12"
default_toolchain = "stable-aarch64-apple-darwin"
profile = "default"

[overrides]
"#;

    #[test]
    fn deserialize_default_host_triple() {
        let raw = r#"version = "12"
default_host_triple = "stable-aarch64-apple-darwin"
"#;
        let settings = Settings::parse(raw).unwrap();
        assert_eq!(
            settings.default_host_tuple,
            Some("stable-aarch64-apple-darwin".to_owned())
        );
    }

    #[test]
    fn deserialize_default_host_tuple() {
        let raw = r#"version = "12"
default_host_tuple = "stable-aarch64-apple-darwin"
"#;
        let settings = Settings::parse(raw).unwrap();
        assert_eq!(
            settings.default_host_tuple,
            Some("stable-aarch64-apple-darwin".to_owned())
        );
    }
}
