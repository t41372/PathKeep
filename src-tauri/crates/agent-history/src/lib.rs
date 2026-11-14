//! Responsibilities: preserve native agent history and expose a local evidence archive.
//! Not responsible for: product UI, scheduling, model calls, source edits or native resume.
//!
//! Sources remain read-only. Raw revisions are authoritative; normalized records and
//! search are explicit, versioned interpretations with separate coverage reporting.

pub mod adapters;
pub mod archive;
pub mod capture;
pub mod error;
pub mod export;
pub mod model;
pub mod query;
pub mod search;
pub mod sqlite;
pub mod verification;

pub use archive::Archive;
pub use error::Error;
pub use model::*;
pub use search::{SearchHit, SearchPage, SearchQuery, SearchSyncReport};

/// A single error boundary preserves source, destination and cancellation failures.
pub type Result<T> = std::result::Result<T, Error>;
