//! Responsibilities: mark errors at browser-source boundaries for per-profile outcomes.
//! Not responsible for: classifying Archive/Store, destination IO or cancellation failures.
use crate::models::ProfileOutcome;

/// Only explicitly marked source errors allow the backup to continue to another profile.
/// Attach as anyhow context so the original diagnostic chain remains intact.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SourceError {
    #[error("{0}")]
    Unavailable(&'static str),
    #[error("{0}")]
    Failed(&'static str),
}
impl SourceError {
    pub(crate) fn outcome(&self) -> ProfileOutcome {
        match self {
            Self::Unavailable(code) => ProfileOutcome::Unavailable { code: (*code).into() },
            Self::Failed(code) => ProfileOutcome::Failed { code: (*code).into() },
        }
    }
    /// A discovered profile whose files cannot be opened. A profile that is no longer
    /// discovered at all is `source-profile-not-discovered`, decided before staging.
    pub(crate) fn access(error: &std::io::Error) -> Self {
        Self::Unavailable(match error.kind() {
            std::io::ErrorKind::PermissionDenied => "source-permission-denied",
            _ => "source-unreadable",
        })
    }
}
