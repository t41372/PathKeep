//! Responsibilities: classify storage failures while retaining engine causes.
//! Not responsible for: UI copy, retries or logging secrets.
//! Failure modes: busy/corrupt/key errors are misclassified; diagnostic causes vanish.
//! Tests: opening.
use std::path::PathBuf;

/// One classification boundary lets callers recover without matching engine text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("this archive is owned by another process: {path}")]
    OwnedElsewhere {
        path: PathBuf,
        #[source]
        source: std::fs::TryLockError,
    },
    #[error("the database key is missing or incorrect")]
    WrongKey {
        #[source]
        source: turso::Error,
    },
    #[error("the database is busy")]
    Busy {
        #[source]
        source: turso::Error,
    },
    #[error("the database is corrupt or is not a Turso database")]
    Corrupt {
        #[source]
        source: turso::Error,
    },
    #[error("operation cancelled")]
    Cancelled {
        #[source]
        source: Option<turso::Error>,
    },
    #[error("{db:?} schema version {found} is newer than supported version {supported}")]
    SchemaTooNew { db: crate::Db, found: i64, supported: i64 },
    #[error("storage engine: {0}")]
    Engine(#[source] turso::Error),
    #[error("storage I/O: {0}")]
    Io(#[from] std::io::Error),
    /// A caller's validation or serialization failure also rolls back its job.
    #[error("storage operation: {0}")]
    Operation(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("invalid store configuration: {0}")]
    InvalidConfig(String),
    #[error("SQL is outside the storage contract: {0}")]
    RestrictedSql(&'static str),
    #[error("storage closure panicked: {message}")]
    JobPanicked { message: String },
    #[error("a storage closure cannot call Store synchronously")]
    Reentrant,
    #[error("storage worker has stopped")]
    Closed,
    #[error("{original}; rollback also failed: {rollback}")]
    Rollback {
        #[source]
        original: Box<Error>,
        rollback: Box<Error>,
    },
}

/// Used consistently by storage operations and owner-provided migration closures.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Cooperative cancellation has no engine failure underneath it.
    pub fn cancelled() -> Self {
        Self::Cancelled { source: None }
    }
}

impl From<turso::Error> for Error {
    fn from(source: turso::Error) -> Self {
        match source {
            turso::Error::Busy(_) | turso::Error::BusySnapshot(_) => Self::Busy { source },
            turso::Error::Corrupt(_) | turso::Error::NotAdb(_) => Self::Corrupt { source },
            turso::Error::Interrupt(_) => Self::Cancelled { source: Some(source) },
            // 0.8.1 erases the encryption error's type at the public SDK boundary.
            turso::Error::Error(ref text) if text.contains("Decryption failed for page=") => {
                Self::WrongKey { source }
            }
            source => Self::Engine(source),
        }
    }
}
