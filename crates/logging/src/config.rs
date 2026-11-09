use tracing_subscriber::filter::LevelFilter;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl TryFrom<&str> for LogLevel {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            "trace" => Ok(Self::Trace),
            _ => Err(Error::Logging(format!("invalid log level: {value}"))),
        }
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Off => Self::OFF,
            LogLevel::Error => Self::ERROR,
            LogLevel::Warn => Self::WARN,
            LogLevel::Info => Self::INFO,
            LogLevel::Debug => Self::DEBUG,
            LogLevel::Trace => Self::TRACE,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::{LogFormat, LogLevel};
    use crate::error::Error;

    #[test]
    fn parses_known_levels() {
        assert_eq!(LogLevel::try_from("off").expect("off"), LogLevel::Off);
        assert_eq!(LogLevel::try_from("error").expect("error"), LogLevel::Error);
        assert_eq!(LogLevel::try_from("warn").expect("warn"), LogLevel::Warn);
        assert_eq!(LogLevel::try_from("info").expect("info"), LogLevel::Info);
        assert_eq!(LogLevel::try_from("debug").expect("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::try_from("trace").expect("trace"), LogLevel::Trace);
        assert_eq!(LogLevel::try_from("INFO").expect("info"), LogLevel::Info);
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
    fn try_from_rejects_unknown_level() {
        let error = LogLevel::try_from("verbose").expect_err("unknown level must fail");

        assert!(matches!(error, Error::Logging(_)));
    }

    #[test]
    fn try_from_rejects_unknown_format() {
        let error = LogFormat::try_from("xml").expect_err("unknown format must fail");

        assert!(matches!(error, Error::Logging(_)));
    }
}
