use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result};

use super::manifest::Component;
use crate::errors::*;
use crate::utils::toml_utils::*;

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub config_version: ConfigVersion,
    pub components: Vec<Component>,
}

impl Config {
    pub(crate) fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        let config_version = get_string(&mut table, "config_version", path)?;
        let config_version = ConfigVersion::from_str(&config_version)?;

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
            toml::Value::String(self.config_version.as_str().to_owned()),
        );
        let components = Self::components_to_toml(self.components);
        if !components.is_empty() {
            result.insert("components".to_owned(), toml::Value::Array(components));
        }
        result
    }

    pub(crate) fn parse(data: &str) -> Result<Self> {
        let value = toml::from_str(data).context("error parsing config")?;
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ConfigVersion {
    #[default]
    V1,
}

impl ConfigVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V1 => "1",
        }
    }
}

impl FromStr for ConfigVersion {
    type Err = RustupError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" => Ok(Self::V1),
            _ => Err(RustupError::UnsupportedVersion(s.to_owned())),
        }
    }
}

impl fmt::Display for ConfigVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
