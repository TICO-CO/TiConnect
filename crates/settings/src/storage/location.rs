use std::env;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::error::{Error, Result};

const APP_NAME: &str = "ticonnect";
const CONFIG_DIR_ENV: &str = "TICONNECT_CONFIG_DIR";

/// Resolved location of the user settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Location {
    dir: PathBuf,
    file: PathBuf,
}

impl Location {
    pub(crate) fn new(dir: PathBuf, file: PathBuf) -> Self {
        Self { dir, file }
    }

    /// Resolve the settings location from the environment or user directories.
    pub(super) fn discover() -> Result<Self> {
        if let Some(dir) = env::var_os(CONFIG_DIR_ENV) {
            return Ok(Self::from_config_dir(PathBuf::from(dir)));
        }

        let dirs = project_dirs()?;
        Ok(Self::from_config_dir(dirs.config_dir().to_path_buf()))
    }

    fn from_config_dir(dir: PathBuf) -> Self {
        let file = dir.join("config.toml");
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

pub(crate) fn default_data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().to_path_buf())
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", APP_NAME)
        .ok_or_else(|| Error::Settings("could not determine user directories".to_owned()))
}
