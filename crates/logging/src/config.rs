use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogFormat {
    #[default]
    Human,
    Json,
}

impl TryFrom<&str> for LogFormat {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "human" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            _ => Err(Error::Logging(format!("invalid log format: {value}"))),
        }
    }
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

impl LogSettings {
    pub fn from_parts(level: impl Into<String>, file: Option<&Path>, format: &str) -> Result<Self> {
        Ok(Self {
            level: level.into(),
            file: file.map(Path::to_path_buf),
            format: LogFormat::try_from(format)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{LogFormat, LogSettings};
    use crate::error::Error;

    #[test]
    fn builds_from_parts() {
        let settings =
            LogSettings::from_parts("warn", Some(Path::new("/tmp/ticonnect.log")), "json")
                .expect("valid parts");

        assert_eq!(settings.level, "warn");
        assert_eq!(
            settings.file.as_deref(),
            Some(Path::new("/tmp/ticonnect.log"))
        );
        assert_eq!(settings.format, LogFormat::Json);
    }

    #[test]
    fn rejects_unknown_format_from_parts() {
        let error =
            LogSettings::from_parts("info", None, "xml").expect_err("unknown format must fail");

        assert!(matches!(error, Error::Logging(_)));
    }

    #[test]
    fn parses_known_formats() {
        assert_eq!(
            LogFormat::try_from("human").expect("human"),
            LogFormat::Human
        );
        assert_eq!(LogFormat::try_from("json").expect("json"), LogFormat::Json);
        assert_eq!(LogFormat::try_from("JSON").expect("json"), LogFormat::Json);
    }

    #[test]
    fn rejects_unknown_format() {
        let error = LogFormat::try_from("xml").expect_err("unknown format must fail");

        assert!(matches!(error, Error::Logging(_)));
    }
}
