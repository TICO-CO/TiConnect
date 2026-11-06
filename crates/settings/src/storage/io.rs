use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub fn read_to_string(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn remove_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn exists(path: &Path) -> bool {
    path.exists()
}

pub fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let temporary = sidecar(path, ".tmp");
    let result = fs::write(&temporary, contents).and_then(|()| fs::rename(&temporary, path));

    let Err(error) = result else {
        return Ok(());
    };

    match fs::remove_file(&temporary) {
        Ok(()) => Err(Error::from(error)),
        Err(cleanup) if cleanup.kind() == io::ErrorKind::NotFound => Err(Error::from(error)),
        Err(cleanup) => Err(Error::Settings(format!(
            "{error}; additionally could not remove temporary file {}: {cleanup}",
            temporary.display()
        ))),
    }
}

pub fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("settings"), OsString::from);
    name.push(suffix);
    path.with_file_name(name)
}
