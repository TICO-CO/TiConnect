use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("logging error: {0}")]
    Logging(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
