use std::path::{Path, PathBuf};

use super::{Location, acquire_write_lock, io, load_values, save_values};
use crate::error::{Error, Result};
use crate::settings::schema::SettingsValues;

/// File-backed settings storage.
///
/// Owns the resolved settings location, the values last loaded from or saved to
/// disk, and all filesystem access. The [`SettingsValues`] type itself stays a passive
/// data model.
#[derive(Debug, Clone, PartialEq)]
pub struct Storage {
    location: Location,
    saved: SettingsValues,
}

impl Storage {
    /// Open storage for an explicit settings file and load its values.
    pub fn at(path: impl Into<PathBuf>) -> Result<Self> {
        let file = path.into();
        let dir = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(PathBuf::new, Path::to_path_buf);

        Self::from_location(Location::new(dir, file))
    }

    /// Open storage at the default location and load its values.
    pub fn open_default() -> Result<Self> {
        Self::from_location(Location::discover()?)
    }

    fn from_location(location: Location) -> Result<Self> {
        let saved = load_values(&location)?;

        Ok(Self { location, saved })
    }

    #[cfg(test)]
    pub fn config_file(&self) -> &Path {
        self.location.file()
    }

    #[cfg(test)]
    pub fn config_dir(&self) -> &Path {
        self.location.dir()
    }

    /// Resolve the data directory from a configured value or the platform default.
    pub fn data_dir(&self, configured: Option<&Path>) -> Result<PathBuf> {
        configured.map_or_else(Location::default_data_dir, |dir| Ok(dir.to_path_buf()))
    }

    /// Settings values currently persisted on disk.
    pub fn saved(&self) -> &SettingsValues {
        &self.saved
    }

    /// Persist settings, preserving unknown keys in the existing file.
    pub fn save(&mut self, values: &SettingsValues) -> Result<()> {
        let path = self.location.file();
        io::create_parent(path)?;
        let _lock = acquire_write_lock(path)?;
        save_values(path, values)?;

        self.saved = values.clone();
        Ok(())
    }

    /// Remove the settings file so built-in defaults apply.
    ///
    /// A missing file is not an error. No directories are created.
    pub fn reset(&mut self) -> Result<()> {
        let path = self.location.file();
        if io::exists(path) {
            let _lock = acquire_write_lock(path)?;
            self.remove_file()?;
        }

        self.saved = SettingsValues::default();
        Ok(())
    }

    fn remove_file(&self) -> Result<()> {
        let path = self.location.file();
        io::remove_file(path).map_err(|error| {
            Error::Settings(format!("could not reset {}: {error}", path.display()))
        })
    }
}
