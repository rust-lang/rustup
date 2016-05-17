use toml;

use rustup_utils::toml_utils::*;
use errors::*;
use super::manifest::Component;

pub const SUPPORTED_CONFIG_VERSIONS: [&'static str; 1] = ["1"];
pub const DEFAULT_CONFIG_VERSION: &'static str = "1";

#[derive(Clone, Debug)]
pub struct Config {
    pub config_version: String,
    pub components: Vec<Component>,
}

impl Config {
    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        let version = try!(get_string(&mut table, "config_version", path));
        if !SUPPORTED_CONFIG_VERSIONS.contains(&&*version) {
            return Err(ErrorKind::UnsupportedVersion(version).into());
        }

        let components = try!(get_array(&mut table, "components", path));
        let components = try!(Self::toml_to_components(components,
                                                       &format!("{}{}.", path, "components")));

        Ok(Config {
            config_version: version,
            components: components,
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let mut result = toml::Table::new();
        result.insert("config_version".to_owned(),
                      toml::Value::String(self.config_version));
        let components = Self::components_to_toml(self.components);
        if !components.is_empty() {
            result.insert("components".to_owned(), toml::Value::Array(components));
        }
        result
    }

    pub fn parse(data: &str) -> Result<Self> {
        let mut parser = toml::Parser::new(data);
        let value = try!(parser.parse().ok_or_else(move || ErrorKind::Parsing(parser.errors)));

        Self::from_toml(value, "")
    }

    pub fn stringify(self) -> String {
        toml::Value::Table(self.to_toml()).to_string()
    }

    fn toml_to_components(arr: toml::Array, path: &str) -> Result<Vec<Component>> {
        let mut result = Vec::new();

        for (i, v) in arr.into_iter().enumerate() {
            if let toml::Value::Table(t) = v {
                let path = format!("{}[{}]", path, i);
                result.push(try!(Component::from_toml(t, &path)));
            }
        }

        Ok(result)
    }

    fn components_to_toml(components: Vec<Component>) -> toml::Array {
        let mut result = toml::Array::new();
        for v in components {
            result.push(toml::Value::Table(v.to_toml()));
        }
        result
    }

    pub fn new() -> Self {
        Config {
            config_version: DEFAULT_CONFIG_VERSION.to_owned(),
            components: Vec::new(),
        }
    }
}
