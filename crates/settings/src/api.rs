use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::state::SettingsState;

#[derive(Debug, Clone)]
pub struct Settings {
    state: SettingsState,
}

impl Settings {
    /// Load settings from the default location and environment.
    pub fn load() -> Result<Self> {
        Ok(Self {
            state: SettingsState::load()?,
        })
    }

    /// Load settings from an explicit file and the environment.
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            state: SettingsState::load_from(path.as_ref())?,
        })
    }

    /// Persist explicit changes. Untouched environment values are not saved.
    pub fn save(&mut self) -> Result<()> {
        self.state.save()
    }

    /// Remove the settings file and restore defaults. Environment stays active.
    pub fn reset(&mut self) -> Result<()> {
        self.state.reset()
    }

    /// Effective log level.
    pub fn log_level(&self) -> &str {
        self.state.log_level()
    }

    /// Set the effective log level.
    pub fn set_log_level(&mut self, level: impl Into<String>) {
        self.state.set_log_level(level);
    }

    /// Effective log file path.
    pub fn log_file(&self) -> Option<&Path> {
        self.state.log_file()
    }

    /// Set the effective log file path.
    pub fn set_log_file(&mut self, file: Option<PathBuf>) {
        self.state.set_log_file(file);
    }

    /// Effective log format.
    pub fn log_format(&self) -> &str {
        self.state.log_format()
    }

    /// Set the effective log format.
    pub fn set_log_format(&mut self, format: impl Into<String>) {
        self.state.set_log_format(format);
    }

    /// Effective data directory.
    pub fn data_dir(&self) -> Result<PathBuf> {
        self.state.data_dir()
    }

    /// Set the effective data directory.
    pub fn set_data_dir(&mut self, dir: Option<PathBuf>) {
        self.state.set_data_dir(dir);
    }
}
