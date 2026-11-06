use std::path::{Path, PathBuf};

use super::Settings;
use crate::error::Result;
use crate::storage::location::default_data_dir;

settings_schema! {
    log_level: String = "info".to_owned() => {
        get(&self) -> &str as active {
            active.log_level.as_str()
        };
        set(&mut self, level: impl Into<String>) as active {
            active.log_level = level.into();
        };
    };

    #[serde(skip_serializing_if = "Option::is_none")]
    log_file: Option<PathBuf> = None => {
        get(&self) -> Option<&Path> as active {
            active.log_file.as_deref()
        };
        set(&mut self, file: Option<PathBuf>) as active {
            active.log_file = file;
        };
    };

    log_format: String = "human".to_owned() => {
        get(&self) -> &str as active {
            active.log_format.as_str()
        };
        set(&mut self, format: impl Into<String>) as active {
            active.log_format = format.into();
        };
    };

    #[serde(skip_serializing_if = "Option::is_none")]
    data_dir: Option<PathBuf> = None => {
        get(&self) -> Result<PathBuf> as active {
            active.data_dir.clone().map_or_else(default_data_dir, Ok)
        };
        set(&mut self, dir: Option<PathBuf>) as active {
            active.data_dir = dir;
        };
    };
}
