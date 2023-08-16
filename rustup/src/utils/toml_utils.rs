use anyhow::{anyhow, Result};
use thiserror::Error as ThisError;

fn get_value(table: &mut toml::value::Table, key: &str, path: &str) -> Result<toml::Value> {
    table
        .remove(key)
        .ok_or_else(|| anyhow!(format!("missing key: '{}'", path.to_owned() + key)))
}

#[derive(Debug, ThisError)]
#[error("expected type: '{0}' for '{1}'")]
struct ExpectedType(&'static str, String);

pub(crate) fn get_string(table: &mut toml::value::Table, key: &str, path: &str) -> Result<String> {
    if let toml::Value::String(s) = get_value(table, key, path)? {
        Ok(s)
    } else {
        Err(ExpectedType("string", path.to_owned() + key).into())
    }
}

pub(crate) fn get_opt_string(
    table: &mut toml::value::Table,
    key: &str,
    path: &str,
) -> Result<Option<String>> {
    if let Ok(v) = get_value(table, key, path) {
        if let toml::Value::String(s) = v {
            Ok(Some(s))
        } else {
            Err(ExpectedType("string", path.to_owned() + key).into())
        }
    } else {
        Ok(None)
    }
}

pub(crate) fn get_bool(table: &mut toml::value::Table, key: &str, path: &str) -> Result<bool> {
    get_value(table, key, path).and_then(|v| {
        if let toml::Value::Boolean(b) = v {
            Ok(b)
        } else {
            Err(ExpectedType("bool", path.to_owned() + key).into())
        }
    })
}

pub(crate) fn get_table(
    table: &mut toml::value::Table,
    key: &str,
    path: &str,
) -> Result<toml::value::Table> {
    if let Some(v) = table.remove(key) {
        if let toml::Value::Table(t) = v {
            Ok(t)
        } else {
            Err(ExpectedType("table", path.to_owned() + key).into())
        }
    } else {
        Ok(toml::value::Table::new())
    }
}

pub(crate) fn get_array(
    table: &mut toml::value::Table,
    key: &str,
    path: &str,
) -> Result<toml::value::Array> {
    if let Some(v) = table.remove(key) {
        if let toml::Value::Array(s) = v {
            Ok(s)
        } else {
            Err(ExpectedType("array", path.to_owned() + key).into())
        }
    } else {
        Ok(toml::value::Array::new())
    }
}
