use std::path::{Path, PathBuf};

use super::Settings;
use crate::error::Result;

settings_schema! {
    log_level: String = "info".to_owned() => {
        get(&self) -> &str as active {
            active.log_level.as_str()
        };
        set(&mut self, level: impl Into<String>) -> set_log_level as active {
            active.log_level = level.into();
        };
    };

    #[serde(skip_serializing_if = "Option::is_none")]
    log_file: Option<PathBuf> = None => {
        get(&self) -> Option<&Path> as active {
            active.log_file.as_deref()
        };
        set(&mut self, file: Option<PathBuf>) -> set_log_file as active {
            active.log_file = file;
        };
    };

    log_format: String = "human".to_owned() => {
        get(&self) -> &str as active {
            active.log_format.as_str()
        };
        set(&mut self, format: impl Into<String>) -> set_log_format as active {
            active.log_format = format.into();
        };
    };

    #[serde(skip_serializing_if = "Option::is_none")]
    data_dir: Option<PathBuf> = None => {
        get(&self) -> Result<PathBuf> as active, storage {
            storage.data_dir(active.data_dir.as_deref())
        };
        set(&mut self, dir: Option<PathBuf>) -> set_data_dir as active {
            active.data_dir = dir;
        };
    };
}
