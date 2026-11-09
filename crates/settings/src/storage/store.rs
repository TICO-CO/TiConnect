use std::path::{Path, PathBuf};

use super::{Location, acquire_write_lock, io, load_values, save_values};
use crate::error::{Error, Result};
use crate::settings::schema::SettingsValues;

#[derive(Debug, Clone, PartialEq)]
pub struct Storage {
    location: Location,
    stored: SettingsValues,
}

impl Storage {
    pub fn open() -> Result<Self> {
        Self::from_location(Location::discover()?)
    }

    pub fn open_at(path: impl Into<PathBuf>) -> Result<Self> {
        let file = path.into();
        let dir = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(PathBuf::new, Path::to_path_buf);

        Self::from_location(Location::new(dir, file))
    }

    pub fn data_dir(&self, configured: Option<&Path>) -> Result<PathBuf> {
        configured.map_or_else(Location::default_data_dir, |dir| Ok(dir.to_path_buf()))
    }

    pub fn stored(&self) -> &SettingsValues {
        &self.stored
    }

    pub fn save(&mut self, values: &SettingsValues) -> Result<()> {
        let path = self.location.file();
        io::create_parent(path)?;
        let _lock = acquire_write_lock(path)?;
        save_values(path, values)?;

        self.stored = values.clone();
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        let path = self.location.file();
        if io::exists(path) {
            let _lock = acquire_write_lock(path)?;
            self.remove_file()?;
        }

        self.stored = SettingsValues::default();
        Ok(())
    }

    #[cfg(test)]
    pub fn config_file(&self) -> &Path {
        self.location.file()
    }

    #[cfg(test)]
    pub fn config_dir(&self) -> &Path {
        self.location.dir()
    }

    fn from_location(location: Location) -> Result<Self> {
        let stored = load_values(&location)?;

        Ok(Self { location, stored })
    }

    fn remove_file(&self) -> Result<()> {
        let path = self.location.file();
        io::remove_file(path).map_err(|error| {
            Error::Settings(format!("could not reset {}: {error}", path.display()))
        })
    }
}
