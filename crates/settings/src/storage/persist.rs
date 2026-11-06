use std::path::Path;

use super::{Location, io};
use crate::error::{Error, Result};
use crate::settings::schema::SettingsValues;

pub fn load_values(location: &Location) -> Result<SettingsValues> {
    values_from_table(&read_table(location.file())?)
}

pub fn save_values(path: &Path, values: &SettingsValues) -> Result<()> {
    let mut table = read_table(path)?;
    let file_values = values_from_table(&table)?;

    if values == &file_values {
        return Ok(());
    }

    let current = values_to_table(values)?;
    let previous = values_to_table(&file_values)?;

    for (key, value) in &current {
        if previous.get(key) != Some(value) {
            table.insert(key.clone(), value.clone());
        }
    }

    for key in previous.keys() {
        if !current.contains_key(key) {
            table.remove(key);
        }
    }

    let serialized = toml::to_string_pretty(&table).map_err(serialize_error)?;
    io::write_atomic(path, serialized.as_bytes())
}

fn read_table(path: &Path) -> Result<toml::Table> {
    let Some(content) = io::read_to_string(path)? else {
        return Ok(toml::Table::new());
    };

    if content.trim().is_empty() {
        return Ok(toml::Table::new());
    }

    parse_table(&content)
}

fn parse_table(content: &str) -> Result<toml::Table> {
    toml::from_str(content).map_err(invalid_settings)
}

fn values_from_table(table: &toml::Table) -> Result<SettingsValues> {
    toml::Value::Table(table.clone())
        .try_into()
        .map_err(invalid_settings)
}

fn values_to_table(values: &SettingsValues) -> Result<toml::Table> {
    let value = toml::Value::try_from(values).map_err(serialize_error)?;

    match value {
        toml::Value::Table(table) => Ok(table),
        _ => Err(Error::Settings(
            "settings did not serialize to a table".to_owned(),
        )),
    }
}

fn invalid_settings(error: impl std::fmt::Display) -> Error {
    Error::Settings(format!("invalid settings file: {error}"))
}

fn serialize_error(error: impl std::fmt::Display) -> Error {
    Error::Settings(format!("could not serialize values: {error}"))
}
