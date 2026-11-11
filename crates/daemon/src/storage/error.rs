use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error("database schema version {current} is newer than supported version {latest}")]
    SchemaTooNew { current: i64, latest: i64 },

    #[error("database integrity check failed: {0}")]
    Integrity(String),

    #[error("peer fingerprint already exists: {0}")]
    DuplicateFingerprint(String),

    #[error("invalid peer policy: {0}")]
    InvalidPolicy(String),

    #[error("database worker stopped")]
    WorkerStopped,
}
