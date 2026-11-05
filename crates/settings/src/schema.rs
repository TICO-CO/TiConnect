use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::log::LogFormat;
use crate::settings::Settings;
use crate::storage::default_data_dir;

// Settings values, whether active or stored on disk.
//
// This declaration is the single source of truth for persisted settings,
// their defaults, environment overrides, change tracking, and accessors.
settings_schema! {
    log_level: String = "info".to_owned(),
        get fn log_level(&self) -> &str as values {
            values.log_level.as_str()
        },
        set fn set_log_level(&mut self, level: impl Into<String>) as values {
            values.log_level = level.into();
        };

    #[serde(skip_serializing_if = "Option::is_none")]
    log_file: Option<PathBuf> = None,
        get fn log_file(&self) -> Option<&Path> as values {
            values.log_file.as_deref()
        },
        set fn set_log_file(&mut self, file: Option<PathBuf>) as values {
            values.log_file = file;
        };

    log_format: LogFormat = LogFormat::Human,
        get fn log_format(&self) -> LogFormat as values {
            values.log_format
        },
        set fn set_log_format(&mut self, format: LogFormat) as values {
            values.log_format = format;
        };

    #[serde(skip_serializing_if = "Option::is_none")]
    data_dir: Option<PathBuf> = None,
        get fn data_dir(&self) -> Result<PathBuf> as values {
            values.data_dir.clone().map_or_else(default_data_dir, Ok)
        },
        set fn set_data_dir(&mut self, dir: Option<PathBuf>) as values {
            values.data_dir = dir;
        };
}
