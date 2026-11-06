use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use super::location::Location;
use crate::error::{Error, Result};
use crate::settings::schema::SettingsValues;

static SETTINGS_WRITE_LOCK: Mutex<()> = Mutex::new(());

const LEGACY_KEYS: [&str; 1] = ["config_dir"];

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
    pub fn location(&self) -> &Location {
        &self.location
    }

    /// Settings values currently persisted on disk.
    pub fn saved(&self) -> &SettingsValues {
        &self.saved
    }

    /// Persist settings, preserving unknown keys in the existing file.
    pub fn save(&mut self, values: &SettingsValues) -> Result<()> {
        let path = self.location.file();
        create_parent(path)?;
        let _lock = acquire_write_lock(path)?;
        write_changes(path, values)?;

        self.saved = values.clone();
        Ok(())
    }

    /// Remove the settings file so built-in defaults apply.
    ///
    /// A missing file is not an error. No directories are created.
    pub fn reset(&mut self) -> Result<()> {
        let path = self.location.file();
        if path.exists() {
            let _lock = acquire_write_lock(path)?;
            self.remove_file()?;
        }

        self.saved = SettingsValues::default();
        Ok(())
    }

    fn remove_file(&self) -> Result<()> {
        let path = self.location.file();
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Settings(format!(
                "could not reset {}: {error}",
                path.display()
            ))),
        }
    }
}

/// Load persisted settings. Missing or empty files yield defaults.
fn load_values(location: &Location) -> Result<SettingsValues> {
    let contents = match fs::read_to_string(location.file()) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(SettingsValues::default());
        }
        Err(error) => return Err(error.into()),
    };

    if contents.trim().is_empty() {
        return Ok(SettingsValues::default());
    }

    toml::from_str(&contents)
        .map_err(|error| Error::Settings(format!("invalid settings file: {error}")))
}

fn write_changes(path: &Path, values: &SettingsValues) -> Result<()> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };

    let non_empty = contents
        .as_deref()
        .is_some_and(|contents| !contents.trim().is_empty());

    let mut table = match contents.as_deref() {
        Some(contents) if non_empty => toml::from_str::<toml::Table>(contents)
            .map_err(|error| Error::Settings(format!("invalid settings file: {error}")))?,
        _ => toml::Table::new(),
    };

    let file_values: SettingsValues = if non_empty {
        toml::Value::Table(table.clone())
            .try_into()
            .map_err(|error| Error::Settings(format!("invalid settings file: {error}")))?
    } else {
        SettingsValues::default()
    };

    let legacy_present = LEGACY_KEYS.iter().any(|key| table.contains_key(*key));
    if values == &file_values && !legacy_present {
        return Ok(());
    }

    let current = values_table(values)?;
    let previous = values_table(&file_values)?;
    for key in previous.keys() {
        if !current.contains_key(key.as_str()) {
            table.remove(key.as_str());
        }
    }
    table.extend(current);

    for key in LEGACY_KEYS {
        table.remove(key);
    }

    let serialized = toml::to_string_pretty(&table)
        .map_err(|error| Error::Settings(format!("could not serialize values: {error}")))?;
    write_atomic(path, serialized.as_bytes())
}

fn values_table(values: &SettingsValues) -> Result<toml::Table> {
    match toml::Value::try_from(values)
        .map_err(|error| Error::Settings(format!("could not serialize values: {error}")))?
    {
        toml::Value::Table(table) => Ok(table),
        _ => Err(Error::Settings(
            "settings did not serialize to a table".to_owned(),
        )),
    }
}

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

struct WriteLock {
    _process: MutexGuard<'static, ()>,
    _file: File,
}

fn acquire_write_lock(path: &Path) -> Result<WriteLock> {
    let process = SETTINGS_WRITE_LOCK
        .lock()
        .map_err(|_| Error::Settings("settings write lock poisoned".to_owned()))?;

    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(sidecar(path, ".lock"))?;
    file.lock()?;

    Ok(WriteLock {
        _process: process,
        _file: file,
    })
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let temporary = sidecar(path, ".tmp");
    let result = fs::write(&temporary, contents).and_then(|()| fs::rename(&temporary, path));

    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }

    result.map_err(Error::from)
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("settings"), OsString::from);
    name.push(suffix);
    path.with_file_name(name)
}
