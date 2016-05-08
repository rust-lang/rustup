use errors::*;
use notifications::*;
use toml_utils::*;
use utils;
use toml;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SUPPORTED_METADATA_VERSIONS: [&'static str; 2] = ["2", "12"];
pub const DEFAULT_METADATA_VERSION: &'static str = "12";


#[derive(Clone, Debug, PartialEq)]
pub struct SettingsFile(pub PathBuf);

impl SettingsFile {
    fn write_settings(&self, settings: Settings) -> Result<()> {
        try!(utils::write_file("settings", &self.0, &settings.stringify()));
        Ok(())
    }
    fn read_settings(&self) -> Result<Settings> {
        if !utils::is_file(&self.0) {
            try!(self.write_settings(Default::default()));
        }
        let content = try!(utils::read_file("settings", &self.0));
        Settings::parse(&content)
    }
    pub fn with<T, F: FnOnce(&Settings) -> Result<T>>(&self, f: F) -> Result<T> {
        f(&try!(self.read_settings()))
    }
    pub fn with_mut<T, F: FnOnce(&mut Settings) -> Result<T>>(&self, f: F) -> Result<T> {
        let mut settings = try!(self.read_settings());
        let result = try!(f(&mut settings));
        try!(self.write_settings(settings));
        Ok(result)
    }
}


#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub version: String,
    pub default_toolchain: Option<String>,
    pub overrides: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: DEFAULT_METADATA_VERSION.to_owned(),
            default_toolchain: None,
            overrides: BTreeMap::new()
        }
    }
}

impl Settings {
    fn path_to_key(path: &Path, notify_handler: NotifyHandler) -> String {
        utils::canonicalize_path(path, ntfy!(&notify_handler))
               .display()
               .to_string()
    }

    pub fn remove_override(&mut self, path: &Path, notify_handler: NotifyHandler) -> bool {
        let key = Self::path_to_key(path, notify_handler);
        self.overrides.remove(&key).is_some()
    }

    pub fn add_override(&mut self, path: &Path, toolchain: String, notify_handler: NotifyHandler) {
        let key = Self::path_to_key(path, notify_handler);
        notify_handler.call(Notification::SetOverrideToolchain(path, &toolchain));
        self.overrides.insert(key, toolchain);
    }

    pub fn find_override(&self, dir_unresolved: &Path, notify_handler: NotifyHandler)
            -> Option<(String, PathBuf)> {
        let dir = utils::canonicalize_path(dir_unresolved, ntfy!(&notify_handler));
        let mut maybe_path = Some(&*dir);
        while let Some(path) = maybe_path {
            let key = Self::path_to_key(path, notify_handler);
            if let Some(toolchain) = self.overrides.get(&key) {
                return Some((toolchain.to_owned(), path.to_owned()));
            }
            maybe_path = path.parent();
        }
        None
    }

    pub fn parse(data: &str) -> Result<Self> {
        let mut parser = toml::Parser::new(data);
        let value = try!(parser.parse().ok_or_else(move || ErrorKind::ParsingSettings(parser.errors)));

        Self::from_toml(value, "")
    }
    pub fn stringify(self) -> String {
        toml::Value::Table(self.to_toml()).to_string()
    }

    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        let version = try!(get_string(&mut table, "version", path));
        if !SUPPORTED_METADATA_VERSIONS.contains(&&*version) {
            return Err(ErrorKind::UnknownMetadataVersion(version).into());
        }
        Ok(Settings {
            version: version,
            default_toolchain: try!(get_opt_string(&mut table, "default_toolchain", path)),
            overrides: try!(Self::table_to_overrides(table, path)),
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let mut result = toml::Table::new();

        result.insert("version".to_owned(),
                      toml::Value::String(self.version));

        if let Some(v) = self.default_toolchain {
            result.insert("default_toolchain".to_owned(), toml::Value::String(v));
        }

        let overrides = Self::overrides_to_table(self.overrides);
        result.insert("overrides".to_owned(), toml::Value::Table(overrides));

        result
    }

    fn table_to_overrides(mut table: toml::Table, path: &str) -> Result<BTreeMap<String, String>> {
        let mut result = BTreeMap::new();
        let pkg_table = try!(get_table(&mut table, "overrides", path));

        for (k, v) in pkg_table {
            if let toml::Value::String(t) = v {
                result.insert(k, t);
            }
        }

        Ok(result)
    }

    fn overrides_to_table(overrides: BTreeMap<String, String>) -> toml::Table {
        let mut result = toml::Table::new();
        for (k, v) in overrides {
            result.insert(k, toml::Value::String(v));
        }
        result
    }
}
