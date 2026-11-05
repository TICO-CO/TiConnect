use std::path::PathBuf;

use figment::{Figment, providers::Env as FigmentEnv};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::log::LogFormat;
use crate::values::Values;

const ENV_PREFIX: &str = "TICONNECT_";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Overrides {
    log_level: Option<String>,
    log_file: Option<PathBuf>,
    log_format: Option<LogFormat>,
    data_dir: Option<PathBuf>,
}

impl Overrides {
    pub(crate) fn load() -> Result<Self> {
        Figment::new()
            .merge(FigmentEnv::prefixed(ENV_PREFIX))
            .extract()
            .map_err(|error| Error::Settings(error.to_string()))
    }

    pub(crate) fn apply(&self, values: &mut Values) {
        if let Some(level) = &self.log_level {
            values.log_level = level.clone();
        }
        if let Some(file) = &self.log_file {
            values.log_file = Some(file.clone());
        }
        if let Some(format) = self.log_format {
            values.log_format = format;
        }
        if let Some(dir) = &self.data_dir {
            values.data_dir = Some(dir.clone());
        }
    }
}
