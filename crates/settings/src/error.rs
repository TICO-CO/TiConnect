use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("settings error: {0}")]
    Settings(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
