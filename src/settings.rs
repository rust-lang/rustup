use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::self_update::SelfUpdateMode;
use crate::dist::{AutoInstallMode, Profile};
use crate::errors::*;
use crate::notifications::*;
use crate::utils;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsFile {
    path: PathBuf,
    cache: RefCell<Option<Settings>>,
}

impl SettingsFile {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            cache: RefCell::new(None),
        }
    }

    fn write_settings(&self) -> Result<()> {
        let settings = self.cache.borrow();
        utils::write_file(
            "settings",
            &self.path,
            &settings.as_ref().unwrap().stringify()?,
        )?;
        Ok(())
    }

    fn read_settings(&self) -> Result<()> {
        let mut needs_save = false;
        {
            let b = self.cache.borrow();
            if b.is_none() {
                drop(b);
                *self.cache.borrow_mut() = Some(if utils::is_file(&self.path) {
                    let content = utils::read_file("settings", &self.path)?;
                    Settings::parse(&content).with_context(|| RustupError::ParsingFile {
                        name: "settings",
                        path: self.path.clone(),
                    })?
                } else {
                    needs_save = true;
                    Default::default()
                });
            }
        }
        if needs_save {
            self.write_settings()?;
        }
        Ok(())
    }

    pub(crate) fn with<T, F: FnOnce(&Settings) -> Result<T>>(&self, f: F) -> Result<T> {
        self.read_settings()?;

        // Settings can no longer be None so it's OK to unwrap
        f(self.cache.borrow().as_ref().unwrap())
    }

    pub(crate) fn with_mut<T, F: FnOnce(&mut Settings) -> Result<T>>(&self, f: F) -> Result<T> {
        self.read_settings()?;

        // Settings can no longer be None so it's OK to unwrap
        let result = { f(self.cache.borrow_mut().as_mut().unwrap())? };
        self.write_settings()?;
        Ok(result)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Settings {
    pub version: MetadataVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_host_triple: Option<String>,
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
    pub auto_install: Option<AutoInstallMode>,
}

impl Settings {
    fn path_to_key(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> String {
        if path.exists() {
            utils::canonicalize_path(path, notify_handler)
                .display()
                .to_string()
        } else {
            path.display().to_string()
        }
    }

    pub(crate) fn remove_override(
        &mut self,
        path: &Path,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> bool {
        let key = Self::path_to_key(path, notify_handler);
        self.overrides.remove(&key).is_some()
    }

    pub(crate) fn add_override(
        &mut self,
        path: &Path,
        toolchain: String,
        notify_handler: &dyn Fn(Notification<'_>),
    ) {
        let key = Self::path_to_key(path, notify_handler);
        notify_handler(Notification::SetOverrideToolchain(path, &toolchain));
        self.overrides.insert(key, toolchain);
    }

    pub(crate) fn dir_override(
        &self,
        dir: &Path,
        notify_handler: &dyn Fn(Notification<'_>),
    ) -> Option<String> {
        let key = Self::path_to_key(dir, notify_handler);
        self.overrides.get(&key).cloned()
    }

    pub(crate) fn parse(data: &str) -> Result<Self> {
        toml::from_str(data).context("error parsing settings")
    }

    fn stringify(&self) -> Result<String> {
        Ok(toml::to_string(self)?)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum MetadataVersion {
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
    use super::*;

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
}
