use std::path::{Path, PathBuf};

use tracing_subscriber::{EnvFilter, filter::LevelFilter};

use crate::config::{LogFormat, LogLevel};
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSettings {
    level: LogLevel,
    file: Option<PathBuf>,
    format: LogFormat,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            file: None,
            format: LogFormat::Human,
        }
    }
}

impl LogSettings {
    pub fn new(level: impl AsRef<str>, file: Option<&Path>, format: &str) -> Result<Self> {
        Ok(Self {
            level: LogLevel::try_from(level.as_ref())?,
            file: file.map(Path::to_path_buf),
            format: LogFormat::try_from(format)?,
        })
    }
}

pub fn env_filter(settings: &LogSettings) -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::from(settings.level).into())
        .parse_lossy("")
}

pub fn file(settings: &LogSettings) -> Option<&Path> {
    settings.file.as_deref()
}

pub fn format(settings: &LogSettings) -> LogFormat {
    settings.format
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::LogSettings;
    use crate::config::{LogFormat, LogLevel};
    use crate::error::Error;

    #[test]
    fn builds_settings() {
        let settings = LogSettings::new("warn", Some(Path::new("/tmp/ticonnect.log")), "json")
            .expect("valid settings");

        assert_eq!(settings.level, LogLevel::Warn);
        assert_eq!(
            settings.file.as_deref(),
            Some(Path::new("/tmp/ticonnect.log"))
        );
        assert_eq!(settings.format, LogFormat::Json);
    }

    #[test]
    fn new_rejects_unknown_format() {
        let error = LogSettings::new("info", None, "xml").expect_err("unknown format must fail");

        assert!(matches!(error, Error::Logging(_)));
    }

    #[test]
    fn new_rejects_invalid_level() {
        let error =
            LogSettings::new("verbose", None, "human").expect_err("invalid level must fail");

        assert!(matches!(error, Error::Logging(_)));
    }
}
