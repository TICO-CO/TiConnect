use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use super::location::{Location, remove_settings_file, require_default_location};
use crate::error::{Error, Result};
use crate::schema::Values;

static SETTINGS_WRITE_LOCK: Mutex<()> = Mutex::new(());

const LEGACY_KEYS: [&str; 1] = ["config_dir"];

/// File-backed settings store.
///
/// Owns the resolved settings location and all filesystem access. The [`Values`]
/// type itself stays a passive data model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileStore {
    location: Location,
}

impl FileStore {
    /// Build a store for an explicit settings file.
    pub(crate) fn at(path: impl Into<PathBuf>) -> Self {
        let file = path.into();
        let dir = file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map_or_else(PathBuf::new, Path::to_path_buf);

        Self {
            location: Location::new(dir, file),
        }
    }

    pub(crate) fn open_default() -> Result<Self> {
        Ok(Self {
            location: require_default_location()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn location(&self) -> &Location {
        &self.location
    }

    /// Load persisted settings. Missing or empty files yield defaults.
    pub(crate) fn load(&self) -> Result<Values> {
        let contents = match fs::read_to_string(self.location.file()) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Values::default()),
            Err(error) => return Err(error.into()),
        };

        if contents.trim().is_empty() {
            return Ok(Values::default());
        }

        toml::from_str(&contents)
            .map_err(|error| Error::Settings(format!("invalid settings file: {error}")))
    }

    /// Persist settings, preserving unknown keys in the existing file.
    pub(crate) fn save(&self, values: &Values) -> Result<()> {
        let path = self.location.file();
        create_parent(path)?;
        let _lock = acquire_write_lock(path)?;
        write_changes(path, values)
    }

    /// Remove the settings file so built-in defaults apply.
    ///
    /// A missing file is not an error. No directories are created.
    pub(crate) fn reset(&self) -> Result<()> {
        let path = self.location.file();
        if !path.exists() {
            return Ok(());
        }

        let _lock = acquire_write_lock(path)?;
        remove_settings_file(path)
    }
}

fn write_changes(path: &Path, values: &Values) -> Result<()> {
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

    let file_values: Values = if non_empty {
        toml::Value::Table(table.clone())
            .try_into()
            .map_err(|error| Error::Settings(format!("invalid settings file: {error}")))?
    } else {
        Values::default()
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

fn values_table(values: &Values) -> Result<toml::Table> {
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
