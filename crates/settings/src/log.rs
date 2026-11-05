use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Human,
    Json,
}

/// Effective logging settings after applying runtime overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSettings {
    pub level: String,
    pub file: Option<PathBuf>,
    pub format: LogFormat,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
            file: None,
            format: LogFormat::Human,
        }
    }
}
