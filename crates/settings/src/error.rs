use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("settings error: {0}")]
    Settings(String),

    #[error("unknown setting: {0}")]
    UnknownSetting(String),

    #[error("invalid setting {key}={value}: {reason}")]
    InvalidSetting {
        key: String,
        value: String,
        reason: String,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
