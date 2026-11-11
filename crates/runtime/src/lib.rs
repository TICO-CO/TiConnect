#![forbid(unsafe_code)]

pub mod bootstrap;
pub mod daemon;
pub mod supervisor;

pub use bootstrap::init;
pub use daemon::run_daemon;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Settings(#[from] ticonnect_settings::Error),

    #[error(transparent)]
    Logging(#[from] ticonnect_logging::Error),

    #[error("runtime error: {0}")]
    Runtime(String),

    #[error(transparent)]
    Identity(ticonnect_daemon::identity::IdentityError),

    #[error(transparent)]
    Storage(ticonnect_daemon::storage::StorageError),

    #[error(transparent)]
    Daemon(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
