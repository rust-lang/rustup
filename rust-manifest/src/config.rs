use toml;

use utils::*;
use errors::*;

pub const SUPPORTED_CONFIG_VERSIONS: [&'static str; 1] = ["1"];
pub const DEFAULT_CONFIG_VERSION: &'static str = "1";

#[derive(Clone, Debug)]
pub struct Config {
    pub config_version: String,
    pub remote: Option<ConfigRemote>,
    pub install: ConfigInstall,
}

#[derive(Clone, Debug)]
pub struct ConfigRemote {
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct ConfigInstall {
    pub libdir: Option<String>,
    pub mandir: Option<String>,
}

impl Config {
    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        let version = try!(get_string(&mut table, "config_version", path));
        if !SUPPORTED_CONFIG_VERSIONS.contains(&&*version) {
            return Err(Error::UnsupportedVersion(version));
        }
        let remote = try!(get_opt_table(&mut table, "remote", path));
        let install = try!(get_table(&mut table, "install", path));

        let remote = try!(remote.map_or(Ok(None), |r| {
            ConfigRemote::from_toml(r, &format!("{}{}.", path, "remote")).map(Some)
        }));

        let install = try!(ConfigInstall::from_toml(install, &format!("{}{}.", path, "install")));

        Ok(Config {
            config_version: version,
            remote: remote,
            install: install,
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let install = self.install.to_toml();
        let remote = self.remote.map(|r| r.to_toml());

        let mut result = toml::Table::new();
        result.insert("install".to_owned(), toml::Value::Table(install));
        if let Some(r) = remote {
            result.insert("remote".to_owned(), toml::Value::Table(r));
        }
        result.insert("config_version".to_owned(),
                      toml::Value::String(self.config_version));
        result
    }

    pub fn parse(data: &str) -> Result<Self> {
        let mut parser = toml::Parser::new(data);
        let value = try!(parser.parse().ok_or_else(move || Error::Parsing(parser.errors)));

        Self::from_toml(value, "")
    }

    pub fn stringify(self) -> String {
        toml::Value::Table(self.to_toml()).to_string()
    }

    pub fn new() -> Self {
        Config {
            config_version: DEFAULT_CONFIG_VERSION.to_owned(),
            remote: None,
            install: ConfigInstall::new(),
        }
    }
}

impl ConfigRemote {
    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        Ok(ConfigRemote { url: try!(get_string(&mut table, "url", path)) })
    }
    pub fn to_toml(self) -> toml::Table {
        let mut result = toml::Table::new();
        result.insert("url".to_owned(), toml::Value::String(self.url));
        result
    }
}

impl ConfigInstall {
    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        Ok(ConfigInstall {
            libdir: try!(get_opt_string(&mut table, "libdir", path)),
            mandir: try!(get_opt_string(&mut table, "mandir", path)),
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let mut result = toml::Table::new();
        if let Some(libdir) = self.libdir {
            result.insert("libdir".to_owned(), toml::Value::String(libdir));
        }
        if let Some(mandir) = self.mandir {
            result.insert("mandir".to_owned(), toml::Value::String(mandir));
        }
        result
    }
    pub fn new() -> Self {
        ConfigInstall {
            libdir: None,
            mandir: None,
        }
    }
}
