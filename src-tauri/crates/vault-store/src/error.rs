//! Responsibilities: classify storage failures while retaining engine causes.
//! Not responsible for: UI copy, retries or logging secrets.
//! Failure modes: busy/corrupt/key errors are misclassified; diagnostic causes vanish.
//! Tests: opening.
use std::path::{Path, PathBuf};

/// Responsibility for a member an archive operation must read or keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFileRole {
    Browsing,
    Evidence,
    ReadModel,
    HeldGeneration,
}

impl From<crate::Db> for ArchiveFileRole {
    fn from(db: crate::Db) -> Self {
        match db {
            crate::Db::AgentHistoryEvidence => Self::Evidence,
            crate::Db::AgentHistoryReadModel => Self::ReadModel,
            _ => Self::Browsing,
        }
    }
}

impl ArchiveFileRole {
    /// Resolve an archive-relative member, including companions and opaque directory children.
    pub fn for_member(relative: &Path) -> Self {
        if relative.starts_with(vault_layout::HELD_CONVERSATIONS) {
            return Self::HeldGeneration;
        }
        if relative.starts_with(
            vault_layout::spec(vault_layout::ArtifactId::AgentHistorySearch).path.relative(),
        ) {
            return Self::ReadModel;
        }
        for db in [crate::Db::AgentHistoryEvidence, crate::Db::AgentHistoryReadModel] {
            for suffix in vault_layout::database_spec(db)
                .companions
                .suffixes(vault_layout::CompanionUse::Install)
            {
                if relative.starts_with(format!("{}{suffix}", vault_layout::database_path(db))) {
                    return db.into();
                }
            }
        }
        Self::Browsing
    }
}

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
    pub path: PathBuf,
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
    #[error("database integrity check failed: {detail}")]
    IntegrityCheck { detail: String },
    #[error("retained revision {revision}: {detail}")]
    RetainedBytes { revision: i64, detail: String },
    #[error(
        "conversation-preservation-failed: could not preserve {path} ({source}), nothing was changed"
    )]
    PreserveFile {
        role: ArchiveFileRole,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("archive-file-write-failed: could not keep {path} ({source})")]
    WriteFile {
        role: ArchiveFileRole,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("open database has been replaced at {0}")]
    ReplacedFile(PathBuf),
    #[error("conversation-pair-unverified: the databases have no matching clean pair seal")]
    PairUnverified,
    #[error("conversation-repair-required: {0}")]
    RepairRequired(#[source] std::sync::Arc<StoreOpenFailure>),
    #[error("conversation-access-unavailable: {0}")]
    AccessUnavailable(#[source] std::sync::Arc<StoreOpenFailure>),
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
    /// A row requested by a typed consumer is missing or its value has the wrong type.
    pub fn is_row_data_error(&self) -> bool {
        matches!(
            self,
            Self::Engine(turso::Error::QueryReturnedNoRows | turso::Error::ConversionFailure(_))
        )
    }

    pub fn unavailable(reason: std::sync::Arc<StoreOpenFailure>) -> Self {
        if reason.cause.access_refused() {
            Self::AccessUnavailable(reason)
        } else {
            Self::RepairRequired(reason)
        }
    }

    pub fn access_refused(&self) -> bool {
        let transient = |k| {
            matches!(
                k,
                std::io::ErrorKind::PermissionDenied
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::TimedOut
            )
        };
        match self {
            Self::Io(e) => {
                transient(e.kind())
                    || cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33 | 303))
            }
            Self::Engine(turso::Error::IoError(k, _)) => transient(*k),
            Self::Busy { .. } => true,
            _ => false,
        }
    }

    /// A fixed source-column decoder could not interpret a stored SQL value.
    pub fn invalid_stored_value(&self) -> bool {
        matches!(self, Self::Engine(turso::Error::ConversionFailure(_)))
    }

    /// File-aware failures keep the actual input path (including a selected copy or stage).
    pub fn at_file(self, role: crate::Db, stage: OpenStage, path: &Path) -> Self {
        match self {
            Self::Open(_) => self,
            cause => Self::Open(std::sync::Arc::new(StoreOpenFailure {
                role,
                stage,
                cause,
                path: path.to_owned(),
            })),
        }
    }

    pub fn preserve(role: ArchiveFileRole, path: &Path, source: std::io::Error) -> Self {
        Self::PreserveFile { role, path: path.to_owned(), source }
    }

    pub fn writing(role: ArchiveFileRole, path: &Path, source: std::io::Error) -> Self {
        Self::WriteFile { role, path: path.to_owned(), source }
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
