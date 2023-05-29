use anyhow::{bail, Context, Result};

use super::manifest::Component;
use crate::errors::*;
use crate::utils::toml_utils::*;

pub(crate) const SUPPORTED_CONFIG_VERSIONS: [&str; 1] = ["1"];
pub(crate) const DEFAULT_CONFIG_VERSION: &str = "1";

#[derive(Clone, Debug)]
pub struct Config {
    pub config_version: String,
    pub components: Vec<Component>,
}

impl Config {
    pub(crate) fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        let config_version = get_string(&mut table, "config_version", path)?;
        if !SUPPORTED_CONFIG_VERSIONS.contains(&&*config_version) {
            bail!(RustupError::UnsupportedVersion(config_version));
        }

        let components = get_array(&mut table, "components", path)?;
        let components =
            Self::toml_to_components(components, &format!("{}{}.", path, "components"))?;

        Ok(Self {
            config_version,
            components,
        })
    }
    pub(crate) fn into_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        result.insert(
            "config_version".to_owned(),
            toml::Value::String(self.config_version),
        );
        let components = Self::components_to_toml(self.components);
        if !components.is_empty() {
            result.insert("components".to_owned(), toml::Value::Array(components));
        }
        result
    }

    pub(crate) fn parse(data: &str) -> Result<Self> {
        let value = toml::from_str(data).context("error parsing manifest")?;
        Self::from_toml(value, "")
    }

    pub(crate) fn stringify(self) -> String {
        self.into_toml().to_string()
    }

    fn toml_to_components(arr: toml::value::Array, path: &str) -> Result<Vec<Component>> {
        let mut result = Vec::new();

        for (i, v) in arr.into_iter().enumerate() {
            if let toml::Value::Table(t) = v {
                let path = format!("{path}[{i}]");
                result.push(Component::from_toml(t, &path, false)?);
            }
        }

        Ok(result)
    }

    fn components_to_toml(components: Vec<Component>) -> toml::value::Array {
        let mut result = toml::value::Array::new();
        for v in components {
            result.push(toml::Value::Table(v.into_toml()));
        }
        result
    }

    pub(crate) fn new() -> Self {
        Default::default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: DEFAULT_CONFIG_VERSION.to_owned(),
            components: Vec::new(),
        }
    }
}
