use std::path::{Path, PathBuf};

#[cfg(test)]
use super::save_values;
use super::{Location, acquire_write_lock, load_values, update_values};
use crate::error::{Error, Result};
use crate::settings::schema::{SettingsChanges, SettingsValues};

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

    pub fn config_file(&self) -> &Path {
        self.location.file()
    }

    pub fn config_dir(&self) -> &Path {
        self.location.dir()
    }

    pub fn ensure_config_dir(&self) -> Result<()> {
        self.location.prepare_dir()
    }

    pub fn update(&mut self, active: &SettingsValues, changes: &SettingsChanges) -> Result<()> {
        if changes.is_empty() {
            return Ok(());
        }

        let path = self.location.file();
        self.location.prepare_dir()?;
        let _lock = acquire_write_lock(path)?;
        self.stored = update_values(path, active, changes)?;
        Ok(())
    }

    #[cfg(test)]
    pub fn save(&mut self, values: &SettingsValues) -> Result<()> {
        let path = self.location.file();
        self.location.prepare_dir()?;
        let _lock = acquire_write_lock(path)?;
        save_values(path, values)?;

        self.stored = values.clone();
        Ok(())
    }

    pub fn reset(&mut self) -> Result<()> {
        let path = self.location.file();
        if parent_exists(path) {
            let _lock = acquire_write_lock(path)?;
            self.remove_file()?;
        }

        self.stored = SettingsValues::default();
        Ok(())
    }

    fn from_location(location: Location) -> Result<Self> {
        let stored = load_values(&location)?;

        Ok(Self { location, stored })
    }

    fn remove_file(&self) -> Result<()> {
        let path = self.location.file();
        super::io::remove_file(path).map_err(|error| {
            Error::Settings(format!("could not reset {}: {error}", path.display()))
        })
    }
}

fn parent_exists(path: &Path) -> bool {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .is_none_or(Path::exists)
}
