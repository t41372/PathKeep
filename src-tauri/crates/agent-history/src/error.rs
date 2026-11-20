//! Responsibilities: distinguish storage failures from explicit source limitations.
//! Not responsible for: user-facing translations or swallowing partial failures.

/// Errors abort an operation; source coverage limitations belong in diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(std::io::Error),
    #[error("invalid native zstd transport: {0}")]
    NativeTransport(#[source] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Store(vault_store::Error),
    #[error(transparent)]
    Envelope(#[from] vault_crypto::EnvelopeError),
    #[error("this archive requires a password")]
    PasswordRequired,
    #[error("this archive is not encrypted")]
    ArchiveNotEncrypted,
    #[error("conversation not found")]
    ConversationNotFound,
    #[error("the conversation reader requires an offline rebuild")]
    ReaderRebuildRequired,
    #[error("invalid search regular expression")]
    InvalidRegex,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("archive integrity check failed: {0}")]
    Integrity(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    #[error("the archive format is not recognized: {0}")]
    InvalidArchive(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("resource limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("the source changed during capture: {0}")]
    SourceChanged(String),
    #[error("the search index has not caught up with the archive")]
    IndexBehind,
    #[error("the page cursor belongs to a replaced projection")]
    StaleCursor,
    #[error("operation cancelled")]
    Cancelled,
}

impl Error {
    /// Stable codes let callers translate failures without parsing engine text.
    pub fn code(&self) -> &'static str {
        match self {
            Self::PasswordRequired => "password_required",
            Self::ArchiveNotEncrypted => "archive_not_encrypted",
            Self::ReaderRebuildRequired => "reader_rebuild_required",
            Self::ConversationNotFound => "conversation_not_found",
            Self::NativeTransport(_) => "invalid_native_transport",
            Self::Io(_) => "io_error",
            Self::Json(_) => "invalid_json",
            Self::Store(_) => "storage_error",
            Self::Envelope(vault_crypto::EnvelopeError::WrongPassword) => "wrong_password",
            Self::Envelope(_) => "invalid_envelope",
            Self::InvalidRegex => "invalid_regex",
            Self::InvalidInput(_) => "invalid_input",
            Self::Integrity(_) => "integrity_error",
            Self::NotFound(_) => "not_found",
            Self::AlreadyExists(_) => "already_exists",
            Self::InvalidArchive(_) => "invalid_archive",
            Self::Unsupported(_) => "unsupported",
            Self::LimitExceeded(_) => "limit_exceeded",
            Self::SourceChanged(_) => "source_changed",
            Self::IndexBehind => "index_behind",
            Self::StaleCursor => "stale_cursor",
            Self::Cancelled => "cancelled",
        }
    }
}

impl From<vault_store::Error> for Error {
    fn from(store: vault_store::Error) -> Self {
        match store {
            vault_store::Error::Operation(cause) => match cause.downcast::<Self>() {
                Ok(error) => *error,
                Err(other) => Self::Store(vault_store::Error::Operation(other)),
            },
            other => Self::Store(other),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        if let Some(vault_store::Error::Open(reason)) =
            error.get_ref().and_then(|e| e.downcast_ref())
        {
            return Self::Store(vault_store::Error::Open(reason.clone()));
        }
        Self::Io(error)
    }
}
