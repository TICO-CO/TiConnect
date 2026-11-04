//! Configuration loading, defaults, and path resolution.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;

use crate::error::{Error, Result};

const APP_NAME: &str = "ticonnect";
const ENV_PREFIX: &str = "TICONNECT_";
const SYSTEM_CONFIG_FILE: &str = "/etc/ticonnect/config.toml";

/// Output format for log records.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Human-readable text output.
    #[default]
    Human,
    /// Structured JSON output.
    Json,
}

/// Runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Logging filter directive, for example `info` or `ticonnect_net=debug`.
    pub log_level: String,
    /// Optional path for log output. Defaults to stdout when unset.
    pub log_file: Option<PathBuf>,
    /// Log output format.
    pub log_format: LogFormat,
    /// Override for the user configuration directory.
    pub config_dir: Option<PathBuf>,
    /// Override for the user data directory.
    pub data_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_level: "info".to_owned(),
            log_file: None,
            log_format: LogFormat::Human,
            config_dir: None,
            data_dir: None,
        }
    }
}

/// Explicit overrides applied after files and environment variables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOverrides {
    /// Override for [`Config::log_level`].
    pub log_level: Option<String>,
    /// Override for [`Config::log_file`].
    pub log_file: Option<PathBuf>,
    /// Override for [`Config::log_format`].
    pub log_format: Option<LogFormat>,
    /// Override for [`Config::config_dir`].
    pub config_dir: Option<PathBuf>,
    /// Override for [`Config::data_dir`].
    pub data_dir: Option<PathBuf>,
}

impl Config {
    /// Load configuration using the standard system and user paths.
    pub fn load() -> Result<Self> {
        Self::load_with(ConfigOverrides::default())
    }

    /// Load configuration and apply explicit overrides last.
    pub fn load_with(overrides: ConfigOverrides) -> Result<Self> {
        let user_file = user_config_file();
        Self::from_paths(
            Some(Path::new(SYSTEM_CONFIG_FILE)),
            user_file.as_deref(),
            overrides,
        )
    }

    /// Resolve the user configuration directory.
    pub fn config_dir(&self) -> Result<PathBuf> {
        match &self.config_dir {
            Some(path) => Ok(path.clone()),
            None => Ok(project_dirs()?.config_dir().to_path_buf()),
        }
    }

    /// Resolve the user data directory.
    pub fn data_dir(&self) -> Result<PathBuf> {
        match &self.data_dir {
            Some(path) => Ok(path.clone()),
            None => Ok(project_dirs()?.data_dir().to_path_buf()),
        }
    }

    fn from_paths(
        system_file: Option<&Path>,
        user_file: Option<&Path>,
        overrides: ConfigOverrides,
    ) -> Result<Self> {
        let mut figment = Figment::new();

        if let Some(path) = system_file {
            figment = figment.merge(Toml::file(path));
        }
        if let Some(path) = user_file {
            figment = figment.merge(Toml::file(path));
        }

        figment = figment.merge(Env::prefixed(ENV_PREFIX));

        let mut config: Self = figment
            .extract()
            .map_err(|error| Error::Config(error.to_string()))?;

        config.apply_overrides(overrides);
        Ok(config)
    }

    fn apply_overrides(&mut self, overrides: ConfigOverrides) {
        if let Some(log_level) = overrides.log_level {
            self.log_level = log_level;
        }
        if let Some(log_file) = overrides.log_file {
            self.log_file = Some(log_file);
        }
        if let Some(log_format) = overrides.log_format {
            self.log_format = log_format;
        }
        if let Some(config_dir) = overrides.config_dir {
            self.config_dir = Some(config_dir);
        }
        if let Some(data_dir) = overrides.data_dir {
            self.data_dir = Some(data_dir);
        }
    }
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", APP_NAME)
        .ok_or_else(|| Error::Config("could not determine user directories".to_owned()))
}

fn user_config_file() -> Option<PathBuf> {
    ProjectDirs::from("", "", APP_NAME).map(|dirs| dirs.config_dir().join("config.toml"))
}

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests {
    use figment::Jail;

    use super::{Config, ConfigOverrides, Error};

    #[test]
    fn defaults_are_correct() {
        let config = Config::default();

        assert_eq!(config.log_level, "info");
        assert_eq!(config.log_file, None);
        assert_eq!(config.log_format, super::LogFormat::Human);
        assert_eq!(config.config_dir, None);
        assert_eq!(config.data_dir, None);
    }

    #[test]
    fn user_file_overrides_system_file() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("system.toml", "log_level = \"warn\"\n")?;
            jail.create_file("user.toml", "log_level = \"debug\"\n")?;

            let system = jail.directory().join("system.toml");
            let user = jail.directory().join("user.toml");
            let config = Config::from_paths(Some(&system), Some(&user), ConfigOverrides::default())
                .expect("valid config");

            assert_eq!(config.log_level, "debug");
            Ok(())
        });
    }

    #[test]
    fn environment_overrides_file() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("user.toml", "log_level = \"debug\"\n")?;
            jail.set_env("TICONNECT_LOG_LEVEL", "trace");

            let user = jail.directory().join("user.toml");
            let config = Config::from_paths(None, Some(&user), ConfigOverrides::default())
                .expect("valid config");

            assert_eq!(config.log_level, "trace");
            Ok(())
        });
    }

    #[test]
    fn explicit_overrides_win_over_environment() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.set_env("TICONNECT_LOG_LEVEL", "trace");

            let overrides = ConfigOverrides {
                log_level: Some("error".to_owned()),
                ..ConfigOverrides::default()
            };
            let config = Config::from_paths(None, None, overrides).expect("valid config");

            assert_eq!(config.log_level, "error");
            Ok(())
        });
    }

    #[test]
    fn invalid_config_returns_typed_error() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("invalid.toml", "log_level = [\n")?;

            let invalid = jail.directory().join("invalid.toml");
            let error = Config::from_paths(None, Some(&invalid), ConfigOverrides::default())
                .expect_err("invalid config must fail");

            assert!(matches!(error, Error::Config(_)));
            Ok(())
        });
    }

    #[test]
    fn missing_files_are_skipped() {
        Jail::expect_with(|jail| {
            jail.clear_env();

            let missing = jail.directory().join("missing.toml");
            let config =
                Config::from_paths(Some(&missing), Some(&missing), ConfigOverrides::default())
                    .expect("missing files must not fail");

            assert_eq!(config, Config::default());
            Ok(())
        });
    }
}
