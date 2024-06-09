use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};

use crate::cli::self_update::SelfUpdateMode;
use crate::dist::dist::Profile;
use crate::errors::*;
use crate::notifications::*;
use crate::utils::toml_utils::*;
use crate::utils::utils;

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
        let s = self.cache.borrow().as_ref().unwrap().clone();
        utils::write_file("settings", &self.path, &s.stringify())?;
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Settings {
    pub version: MetadataVersion,
    pub default_host_triple: Option<String>,
    pub default_toolchain: Option<String>,
    pub profile: Option<Profile>,
    pub overrides: BTreeMap<String, String>,
    pub pgp_keys: Option<String>,
    pub auto_self_update: Option<SelfUpdateMode>,
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
        let value = toml::from_str(data).context("error parsing settings")?;
        Self::from_toml(value, "")
    }

    pub(crate) fn stringify(self) -> String {
        self.into_toml().to_string()
    }

    pub(crate) fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        let version = get_string(&mut table, "version", path)?;
        let version = MetadataVersion::from_str(&version)?;

        let auto_self_update = get_opt_string(&mut table, "auto_self_update", path)?
            .and_then(|mode| SelfUpdateMode::from_str(mode.as_str()).ok());
        let profile = get_opt_string(&mut table, "profile", path)?
            .and_then(|p| Profile::from_str(p.as_str()).ok());
        Ok(Self {
            version,
            default_host_triple: get_opt_string(&mut table, "default_host_triple", path)?,
            default_toolchain: get_opt_string(&mut table, "default_toolchain", path)?,
            profile,
            overrides: Self::table_to_overrides(&mut table, path)?,
            pgp_keys: get_opt_string(&mut table, "pgp_keys", path)?,
            auto_self_update,
        })
    }
    pub(crate) fn into_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();

        result.insert(
            "version".to_owned(),
            toml::Value::String(self.version.as_str().to_owned()),
        );

        if let Some(v) = self.default_host_triple {
            result.insert("default_host_triple".to_owned(), toml::Value::String(v));
        }

        if let Some(v) = self.default_toolchain {
            result.insert("default_toolchain".to_owned(), toml::Value::String(v));
        }

        if let Some(v) = self.profile {
            result.insert("profile".to_owned(), toml::Value::String(v.to_string()));
        }

        if let Some(v) = self.pgp_keys {
            result.insert("pgp_keys".to_owned(), toml::Value::String(v));
        }

        if let Some(v) = self.auto_self_update {
            result.insert(
                "auto_self_update".to_owned(),
                toml::Value::String(v.to_string()),
            );
        }

        let overrides = Self::overrides_to_table(self.overrides);
        result.insert("overrides".to_owned(), toml::Value::Table(overrides));

        result
    }

    fn table_to_overrides(
        table: &mut toml::value::Table,
        path: &str,
    ) -> Result<BTreeMap<String, String>> {
        let mut result = BTreeMap::new();
        let pkg_table = get_table(table, "overrides", path)?;

        for (k, v) in pkg_table {
            if let toml::Value::String(t) = v {
                result.insert(k, t);
            }
        }

        Ok(result)
    }

    fn overrides_to_table(overrides: BTreeMap<String, String>) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        for (k, v) in overrides {
            result.insert(k, toml::Value::String(v));
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MetadataVersion {
    V2,
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
