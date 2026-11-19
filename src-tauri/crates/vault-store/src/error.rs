//! Responsibilities: classify storage failures while retaining engine causes.
//! Not responsible for: UI copy, retries or logging secrets.
//! Failure modes: busy/corrupt/key errors are misclassified; diagnostic causes vanish.
//! Tests: opening.
use std::path::PathBuf;

/// The operation that discovered a role could not join its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenStage {
    Preflight,
    Open,
    KeyCheck,
    Migrate,
    Recover,
    Read,
    Write,
}

/// Shared by availability queries and refusals so callers retain the original engine cause.
#[derive(Debug, thiserror::Error)]
#[error("{role:?} {stage:?}: {cause}")]
pub struct StoreOpenFailure {
    pub role: crate::Db,
    pub stage: OpenStage,
    #[source]
    pub cause: Error,
}

impl StoreOpenFailure {
    /// A rejected SQL migration can be reproduced on a private copy. Storage
    /// exhaustion, permission errors and newer formats require a different remedy.
    pub fn rejected_migration(&self) -> bool {
        self.stage == OpenStage::Migrate
            && matches!(
                self.cause,
                Error::Engine(turso::Error::Error(_) | turso::Error::Constraint(_))
            )
    }
}

/// One classification boundary lets callers recover without matching engine text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Open(#[from] std::sync::Arc<StoreOpenFailure>),
    #[error("archive-store-missing: {0} is missing and nothing says it is being created")]
    MissingStore(PathBuf),
    #[error("open database has been replaced at {0}")]
    ReplacedFile(PathBuf),
    #[error("conversation-pair-unverified: the databases have no matching clean pair seal")]
    PairUnverified,
    #[error("conversation-repair-required: {0}")]
    RepairRequired(#[source] std::sync::Arc<StoreOpenFailure>),
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
    /// Keep the first failing role when an outer startup step adds context.
    pub fn at(self, role: crate::Db, stage: OpenStage) -> Self {
        match self {
            Self::Open(_) => self,
            cause => Self::Open(std::sync::Arc::new(StoreOpenFailure { role, stage, cause })),
        }
    }

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
