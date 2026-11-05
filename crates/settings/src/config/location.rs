use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::error::{Error, Result};

const APP_NAME: &str = "ticonnect";
const CONFIG_DIR_ENV: &str = "TICONNECT_CONFIG_DIR";

/// Resolved location of the user settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigLocation {
    dir: PathBuf,
    file: PathBuf,
}

impl ConfigLocation {
    pub(crate) fn new(dir: PathBuf, file: PathBuf) -> Self {
        Self { dir, file }
    }

    #[cfg(test)]
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn file(&self) -> &Path {
        &self.file
    }
}

pub(super) fn default_location() -> Result<Option<ConfigLocation>> {
    if let Some(dir) = env::var_os(CONFIG_DIR_ENV) {
        let dir = PathBuf::from(dir);
        return Ok(Some(ConfigLocation::new(dir.clone(), config_file(&dir))));
    }

    Ok(project_dirs().ok().map(|dirs| {
        let dir = dirs.config_dir().to_path_buf();
        ConfigLocation::new(dir.clone(), config_file(&dir))
    }))
}

pub(super) fn require_default_location() -> Result<ConfigLocation> {
    default_location()?
        .ok_or_else(|| Error::Config("could not determine user directories".to_owned()))
}

pub(crate) fn default_data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().to_path_buf())
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", APP_NAME)
        .ok_or_else(|| Error::Config("could not determine user directories".to_owned()))
}

fn config_file(config_dir: &Path) -> PathBuf {
    config_dir.join("config.toml")
}

pub(super) fn remove_config_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::Config(format!(
            "could not reset {}: {error}",
            path.display()
        ))),
    }
}
