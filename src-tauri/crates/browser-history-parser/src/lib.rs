//! Pure parsing crate for browser history artifacts.
//!
//! This crate only understands already-provided files such as Chromium
//! `History`, Firefox `places.sqlite`, Safari `History.db`, and Google Takeout
//! exports. It does not discover installed browsers, copy live databases, or
//! talk to the canonical archive.
//!
//! That separation matters for testability and trust: platform/discovery code
//! can evolve independently while parser behavior remains deterministic over
//! concrete input files.

mod canonical_json;
pub use canonical_json::canonicalize_json;

pub mod chromium;
pub mod error;
pub mod firefox;
mod observation;
pub mod safari;
pub mod takeout;
pub mod types;
mod url_pages;
pub use types::UrlScan;

pub use error::ParseError;
/// Legacy alias for the crate's Chromium parse result.
pub type ParsedHistory = types::ChromiumHistory;
/// Shared parser-side read models returned by the individual browser parsers.
pub use types::{
    CapabilityCoverage, CapabilitySnapshot, ChromiumHistory, ChromiumReadCursor, ContextEvidence,
    DatabaseInspection, EngagementEvidence, HistoryBatchConsumer, HistoryDatabaseSet, NativeEntity,
    NavigationEvidence, ObservedColumn, ObservedTable, ParsedDownload, ParsedFavicon,
    ParsedSearchTerm, ParsedUrl, ParsedVisit, ParserWarning, SchemaObservation, SearchEvidence,
    SourceEvidenceChunk, StreamHistoryError, StreamedHistory, TypedEvidenceBatch,
};
