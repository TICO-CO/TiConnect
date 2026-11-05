use std::path::{Path, PathBuf};

use figment::{Figment, providers::Env as FigmentEnv};
use serde::Deserialize;

use crate::config::{Config, ConfigStore, default_data_dir};
use crate::error::{Error, Result};
use crate::log::{LogFormat, LogSettings};

const ENV_PREFIX: &str = "TICONNECT_";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Overrides {
    log_level: Option<String>,
    log_file: Option<PathBuf>,
    log_format: Option<LogFormat>,
    data_dir: Option<PathBuf>,
}

impl Overrides {
    fn load() -> Result<Self> {
        Figment::new()
            .merge(FigmentEnv::prefixed(ENV_PREFIX))
            .extract()
            .map_err(|error| Error::Config(error.to_string()))
    }
}

#[derive(Debug, Clone, Default)]
struct Changes {
    log_level: Option<String>,
    log_file: Option<Option<PathBuf>>,
    log_format: Option<LogFormat>,
    data_dir: Option<Option<PathBuf>>,
}

impl Changes {
    fn is_empty(&self) -> bool {
        self.log_level.is_none()
            && self.log_file.is_none()
            && self.log_format.is_none()
            && self.data_dir.is_none()
    }

    fn apply(&self, config: &mut Config) {
        if let Some(level) = &self.log_level {
            config.log_level = level.clone();
        }
        if let Some(file) = &self.log_file {
            config.log_file = file.clone();
        }
        if let Some(format) = self.log_format {
            config.log_format = format;
        }
        if let Some(dir) = &self.data_dir {
            config.data_dir = dir.clone();
        }
    }
}

/// Effective TiConnect settings.
///
/// The public surface exposes one value per setting. Whether that value came
/// from defaults, the settings file, the environment, or an explicit setter is
/// an internal detail.
#[derive(Debug, Clone)]
pub struct Settings {
    store: ConfigStore,
    persisted: Config,
    overrides: Overrides,
    changes: Changes,
}

impl Settings {
    /// Load settings from the default location and environment.
    pub fn load() -> Result<Self> {
        Self::from_store(ConfigStore::open_default()?)
    }

    /// Load settings from an explicit file and the environment.
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_store(ConfigStore::at(path.as_ref()))
    }

    fn from_store(store: ConfigStore) -> Result<Self> {
        let persisted = store.load()?;
        let overrides = Overrides::load()?;

        Ok(Self {
            store,
            persisted,
            overrides,
            changes: Changes::default(),
        })
    }

    pub fn log_level(&self) -> &str {
        if let Some(level) = &self.changes.log_level {
            return level;
        }
        if let Some(level) = &self.overrides.log_level {
            return level;
        }
        &self.persisted.log_level
    }

    pub fn log_file(&self) -> Option<&Path> {
        if let Some(file) = &self.changes.log_file {
            return file.as_deref();
        }
        if let Some(file) = &self.overrides.log_file {
            return Some(file.as_path());
        }
        self.persisted.log_file.as_deref()
    }

    pub fn log_format(&self) -> LogFormat {
        self.changes
            .log_format
            .or(self.overrides.log_format)
            .unwrap_or(self.persisted.log_format)
    }

    pub fn data_dir(&self) -> Result<PathBuf> {
        if let Some(dir) = &self.changes.data_dir {
            return match dir {
                Some(dir) => Ok(dir.clone()),
                None => default_data_dir(),
            };
        }
        if let Some(dir) = &self.overrides.data_dir {
            return Ok(dir.clone());
        }
        if let Some(dir) = &self.persisted.data_dir {
            return Ok(dir.clone());
        }
        default_data_dir()
    }

    /// Resolve effective logging settings.
    pub fn log_settings(&self) -> LogSettings {
        LogSettings {
            level: self.log_level().to_owned(),
            file: self.log_file().map(Path::to_path_buf),
            format: self.log_format(),
        }
    }

    pub fn set_log_level(&mut self, level: impl Into<String>) {
        self.changes.log_level = Some(level.into());
        self.overrides.log_level = None;
    }

    pub fn set_log_file(&mut self, file: Option<PathBuf>) {
        self.changes.log_file = Some(file);
        self.overrides.log_file = None;
    }

    pub fn set_log_format(&mut self, format: LogFormat) {
        self.changes.log_format = Some(format);
        self.overrides.log_format = None;
    }

    pub fn set_data_dir(&mut self, dir: Option<PathBuf>) {
        self.changes.data_dir = Some(dir);
        self.overrides.data_dir = None;
    }

    /// Persist explicit changes. Untouched environment values are not saved.
    pub fn save(&mut self) -> Result<()> {
        if self.changes.is_empty() {
            return Ok(());
        }

        let mut next = self.persisted.clone();
        self.changes.apply(&mut next);
        self.store.save(&next)?;

        self.persisted = next;
        self.changes = Changes::default();
        Ok(())
    }

    /// Remove the settings file and restore defaults. Environment stays active.
    pub fn reset(&mut self) -> Result<()> {
        let overrides = Overrides::load()?;
        self.store.reset()?;
        self.persisted = Config::default();
        self.overrides = overrides;
        self.changes = Changes::default();
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests {
    use std::fs;

    use figment::Jail;

    use super::Settings;
    use crate::log::LogFormat;

    #[test]
    fn environment_overrides_file_at_runtime_only() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("user.toml", "log_level = \"debug\"\n")?;
            jail.set_env("TICONNECT_LOG_LEVEL", "trace");

            let user = jail.directory().join("user.toml");
            let settings = Settings::load_from(&user).expect("valid settings");

            assert_eq!(settings.log_level(), "trace");
            Ok(())
        });
    }

    #[test]
    fn save_does_not_persist_untouched_environment() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("config.toml", "log_level = \"debug\"\n")?;
            jail.set_env("TICONNECT_LOG_LEVEL", "trace");

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");
            assert_eq!(settings.log_level(), "trace");

            settings.save().expect("save");

            let contents = fs::read_to_string(&path).expect("saved settings");
            assert!(contents.contains("log_level = \"debug\""));
            assert!(!contents.contains("trace"));
            Ok(())
        });
    }

    #[test]
    fn set_changes_effective_value_and_clears_env_shadow() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("config.toml", "log_level = \"debug\"\n")?;
            jail.set_env("TICONNECT_LOG_LEVEL", "trace");

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");
            assert_eq!(settings.log_level(), "trace");

            settings.set_log_level("error");

            assert_eq!(settings.log_level(), "error");
            Ok(())
        });
    }

    #[test]
    fn save_persists_explicit_changes() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("config.toml", "log_level = \"debug\"\n")?;

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");
            settings.set_log_level("error");
            settings.save().expect("save");

            assert_eq!(settings.log_level(), "error");

            let contents = fs::read_to_string(&path).expect("saved settings");
            assert!(contents.contains("log_level = \"error\""));
            Ok(())
        });
    }

    #[test]
    fn explicit_set_is_not_written_until_save() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("config.toml", "log_level = \"debug\"\n")?;

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");
            settings.set_log_level("error");

            let contents = fs::read_to_string(&path).expect("settings file");
            assert!(contents.contains("log_level = \"debug\""));

            settings.save().expect("save");

            let contents = fs::read_to_string(&path).expect("settings file");
            assert!(contents.contains("log_level = \"error\""));
            Ok(())
        });
    }

    #[test]
    fn save_clears_optional_path() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("config.toml", "log_file = \"/tmp/ticonnect.log\"\n")?;

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");
            assert_eq!(
                settings.log_file().map(|path| path.to_path_buf()),
                Some("/tmp/ticonnect.log".into())
            );

            settings.set_log_file(None);
            settings.save().expect("save");

            assert_eq!(settings.log_file(), None);
            let contents = fs::read_to_string(&path).expect("saved settings");
            assert!(!contents.contains("log_file"));
            Ok(())
        });
    }

    #[test]
    fn reset_restores_defaults_but_keeps_environment() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("config.toml", "log_level = \"debug\"\n")?;
            jail.set_env("TICONNECT_LOG_LEVEL", "trace");

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");
            assert_eq!(settings.log_level(), "trace");

            settings.reset().expect("reset");

            assert!(!path.exists());
            assert_eq!(settings.log_level(), "trace");
            assert_eq!(settings.log_format(), LogFormat::Human);
            Ok(())
        });
    }

    #[test]
    fn reset_restores_environment_after_explicit_set() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file("config.toml", "log_level = \"debug\"\n")?;
            jail.set_env("TICONNECT_LOG_LEVEL", "trace");

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");
            settings.set_log_level("error");
            assert_eq!(settings.log_level(), "error");

            settings.reset().expect("reset");

            assert_eq!(settings.log_level(), "trace");
            Ok(())
        });
    }

    #[test]
    fn missing_files_yield_default_settings() {
        Jail::expect_with(|jail| {
            jail.clear_env();

            let missing = jail.directory().join("missing.toml");
            let settings = Settings::load_from(&missing).expect("valid settings");

            assert_eq!(settings.log_level(), "info");
            assert_eq!(settings.log_file(), None);
            assert_eq!(settings.log_format(), LogFormat::Human);
            Ok(())
        });
    }

    #[test]
    fn environment_data_dir_override_is_not_persisted() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            let file_data = jail.create_dir("file-data")?;
            let env_data = jail.create_dir("env-data")?;
            jail.create_file(
                "config.toml",
                &format!("data_dir = \"{}\"\n", file_data.display()),
            )?;
            jail.set_env("TICONNECT_DATA_DIR", env_data.display());

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");

            assert_eq!(settings.data_dir().expect("valid data dir"), env_data);
            settings.save().expect("save");

            let contents = fs::read_to_string(&path).expect("saved settings");
            assert!(contents.contains(&file_data.display().to_string()));
            assert!(!contents.contains(&env_data.display().to_string()));
            Ok(())
        });
    }

    #[test]
    fn settings_preserves_unknown_toml_keys() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            jail.create_file(
                "config.toml",
                "log_level = \"debug\"\nfuture_key = \"keep\"\n",
            )?;

            let path = jail.directory().join("config.toml");
            let mut settings = Settings::load_from(&path).expect("valid settings");
            settings.set_log_level("warn");
            settings.save().expect("save");

            let contents = fs::read_to_string(&path).expect("saved settings");
            let table: toml::Table = toml::from_str(&contents).expect("valid toml");
            assert_eq!(table["log_level"].as_str(), Some("warn"));
            assert_eq!(table["future_key"].as_str(), Some("keep"));
            Ok(())
        });
    }

    #[test]
    fn environment_config_dir_selects_default_settings_file() {
        Jail::expect_with(|jail| {
            jail.clear_env();
            let dir = jail.create_dir("custom")?;
            jail.create_file("custom/config.toml", "log_level = \"debug\"\n")?;
            jail.set_env("TICONNECT_CONFIG_DIR", dir.display());

            let settings = Settings::load().expect("valid settings");

            assert_eq!(settings.log_level(), "debug");
            Ok(())
        });
    }
}
