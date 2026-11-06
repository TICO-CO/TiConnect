use std::env;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::error::{Error, Result};

const DIR_NAME: &str = "ticonnect";
const CONFIG_DIR_ENV: &str = "TICONNECT_CONFIG_DIR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    dir: PathBuf,
    file: PathBuf,
}

impl Location {
    pub fn new(dir: PathBuf, file: PathBuf) -> Self {
        Self { dir, file }
    }

    pub fn discover() -> Result<Self> {
        if let Some(override_dir) = env::var_os(CONFIG_DIR_ENV) {
            return Ok(Self::from_config_dir(PathBuf::from(override_dir)));
        }

        let dirs = Self::project_dirs()?;
        Ok(Self::from_config_dir(dirs.config_dir().to_path_buf()))
    }

    pub fn default_data_dir() -> Result<PathBuf> {
        Ok(Self::project_dirs()?.data_dir().to_path_buf())
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    #[cfg(test)]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn project_dirs() -> Result<ProjectDirs> {
        ProjectDirs::from("", "", DIR_NAME)
            .ok_or_else(|| Error::Settings("could not determine user directories".to_owned()))
    }

    fn from_config_dir(dir: PathBuf) -> Self {
        let file = dir.join("config.toml");
        Self { dir, file }
    }
}
