use std::path::Path;

use super::super::io;
use crate::error::{Error, Result};
use crate::settings::schema::SettingsValues;

pub struct StoredTable(toml::Table);

impl StoredTable {
    pub fn read(path: &Path) -> Result<Self> {
        let Some(content) = io::read_to_string(path)? else {
            return Ok(Self(toml::Table::new()));
        };

        if content.trim().is_empty() {
            return Ok(Self(toml::Table::new()));
        }

        toml::from_str(&content).map(Self).map_err(invalid_settings)
    }

    pub fn values(&self) -> Result<SettingsValues> {
        toml::Value::Table(self.0.clone())
            .try_into()
            .map_err(invalid_settings)
    }

    pub fn save(mut self, path: &Path, values: &SettingsValues) -> Result<()> {
        if !self.merge(values)? {
            return Ok(());
        }

        let serialized = toml::to_string_pretty(&self.0).map_err(serialize_error)?;
        io::write_atomic(path, serialized.as_bytes())
    }

    fn merge(&mut self, values: &SettingsValues) -> Result<bool> {
        let stored_values = self.values()?;

        if values == &stored_values {
            return Ok(false);
        }

        let stored_fields = encode_fields(&stored_values)?;
        let new_fields = encode_fields(values)?;

        self.0
            .retain(|key, _| !stored_fields.contains_key(key) || new_fields.contains_key(key));

        for (key, value) in new_fields {
            if stored_fields.get(&key) != Some(&value) {
                self.0.insert(key, value);
            }
        }

        Ok(true)
    }
}

fn encode_fields(values: &SettingsValues) -> Result<toml::Table> {
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
