//! Responsibilities: Preserve engine and encrypted-directory failure information.
//! Not responsible for: Repairing indexes or hiding I/O errors.
//! Failure modes: Tantivy drops an integrity cause; a permissions error is called
//! corruption. Tested in recovery.

use vault_crypto::{EncryptedDirectory, IndexFileError};

pub type Result<T> = std::result::Result<T, Error>;

/// Distinguishes rebuildable derived data from input errors and operational I/O failures.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("search index must be rebuilt: {reason}")]
    RebuildNeeded { reason: String, integrity: Option<IndexFileError> },
    #[error("invalid search input: {0}")]
    Input(String),
    #[error("search rebuild cancelled")]
    Cancelled,
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Engine(#[from] tantivy::TantivyError),
}
impl Error {
    pub(crate) fn rebuild(reason: impl Into<String>) -> Self {
        Self::RebuildNeeded { reason: reason.into(), integrity: None }
    }
}

pub(crate) fn checked<T>(
    result: tantivy::Result<T>,
    dir: Option<&EncryptedDirectory>,
) -> Result<T> {
    // SegmentReader can swallow a positions-file I/O error even on apparent success.
    if let Some(integrity) = dir.and_then(EncryptedDirectory::integrity_error) {
        return Err(Error::RebuildNeeded {
            reason: integrity.to_string(),
            integrity: Some(integrity),
        });
    }
    result.map_err(|error| {
        if let Some(integrity) = IndexFileError::find_in(&error) {
            return Error::RebuildNeeded {
                reason: integrity.to_string(),
                integrity: Some(integrity.clone()),
            };
        }
        match &error {
            tantivy::TantivyError::DataCorruption(_) | tantivy::TantivyError::SchemaError(_) => {
                Error::rebuild(error.to_string())
            }
            tantivy::TantivyError::IoError(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound
                        | std::io::ErrorKind::InvalidData
                        | std::io::ErrorKind::UnexpectedEof
                ) =>
            {
                Error::rebuild(error.to_string())
            }
            tantivy::TantivyError::OpenReadError(
                tantivy::directory::error::OpenReadError::FileDoesNotExist(_),
            ) => Error::rebuild(error.to_string()),
            _ => Error::Engine(error),
        }
    })
}
