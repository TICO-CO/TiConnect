use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::log::LogFormat;

/// Settings values, whether active or stored on disk.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct Values {
    pub(crate) log_level: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) log_file: Option<PathBuf>,
    pub(crate) log_format: LogFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data_dir: Option<PathBuf>,
}

impl Default for Values {
    fn default() -> Self {
        Self {
            log_level: "info".to_owned(),
            log_file: None,
            log_format: LogFormat::Human,
            data_dir: None,
        }
    }
}
