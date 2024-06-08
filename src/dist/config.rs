use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::manifest::Component;
use crate::errors::*;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub config_version: ConfigVersion,
    pub components: Vec<Component>,
}

impl Config {
    pub(crate) fn parse(data: &str) -> Result<Self> {
        toml::from_str(data).context("error parsing config")
    }

    pub(crate) fn stringify(&self) -> Result<String> {
        Ok(toml::to_string(&self)?)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum ConfigVersion {
    #[serde(rename = "1")]
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
