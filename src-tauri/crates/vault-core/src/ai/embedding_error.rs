//! Responsibilities: identify failures at an embedding service boundary through anyhow contexts.
//! Not responsible for: retry timing, queue outcomes or configuration errors.

/// Lets workers pause an unavailable service without guessing from translated error text.
#[derive(Debug, thiserror::Error)]
#[error("embedding service unavailable")]
pub struct EmbeddingServiceError(#[source] anyhow::Error);

impl EmbeddingServiceError {
    pub(super) fn wrap(error: anyhow::Error) -> anyhow::Error {
        Self(error).into()
    }
}
