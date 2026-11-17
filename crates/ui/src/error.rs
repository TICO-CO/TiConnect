use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("daemon unavailable: {0}")]
    DaemonUnavailable(String),

    #[error("daemon returned an invalid response: {0}")]
    InvalidResponse(String),

    #[error("settings error: {0}")]
    Settings(#[from] ticonnect_settings::Error),

    #[error("autostart error: {0}")]
    Autostart(String),
}

pub type Result<T> = std::result::Result<T, Error>;
