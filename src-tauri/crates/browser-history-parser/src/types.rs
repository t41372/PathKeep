//! Shared parser read models.
//!
//! These types intentionally describe parsed source artifacts, not canonical
//! archive rows. They keep parser output deterministic and easy to test before
//! `vault-core` maps the data into archive-specific semantics.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use thiserror::Error;

/// Inclusive upper bound, in Unix milliseconds, that round-trips cleanly
/// through `chrono`'s `timestamp_millis_opt` (`9999-12-31T23:59:59.999Z`).
pub const MAX_VALID_UNIX_MS: i64 = 253_402_300_799_999;

/// Clamps a Unix-millisecond timestamp to `0..=MAX_VALID_UNIX_MS`.
///
/// ## Why this exists
/// Source rows are untrusted: a corrupt or adversarial browser DB / Takeout
/// export can carry a timestamp far outside any real browsing history (Safari
/// stores the field as a raw `REAL`, Chromium as a microsecond `i64`). Without
/// an upper clamp the numeric `*_ms` column keeps an absurd year-294071 value
/// while the derived ISO string silently collapses to the 1970 epoch (because
/// `timestamp_millis_opt` returns `None` out of range and the converters fall
/// back to epoch). The two representations then disagree for the same visit and
/// the absurd number pollutes preview-range / min-max summaries. Flooring at `0`
/// preserves the existing "0 / NULL / negative sentinel ⇒ epoch" behavior every
/// converter already relied on.
pub fn clamp_unix_millis(value: i64) -> i64 {
    value.clamp(0, MAX_VALID_UNIX_MS)
}

/// File paths for a Chromium history database plus its optional favicons sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryDatabaseSet {
    pub history_path: PathBuf,
    pub favicons_path: Option<PathBuf>,
}

/// Incremental cursor used when rereading Chromium data sources.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChromiumReadCursor {
    pub after_visit_id: i64,
    pub after_url_last_visit_time: i64,
    pub after_search_url_time: i64,
    pub url_scan: Option<UrlScan>,
    pub after_download_id: i64,
    pub after_favicon_last_updated: i64,
}

/// The immutable source filter and last row committed during one URL phase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlScan {
    pub after_id: i64,
    pub time_floor: i64,
    pub visit_floor: i64,
    pub visit_ceiling: i64,
    pub full: bool,
}

/// Non-fatal issue discovered while inspecting or parsing a source database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserWarning {
    pub code: String,
    pub message: String,
}

/// Table-level inspection summary for one source database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseInspection {
    pub table_names: Vec<String>,
    pub warnings: Vec<ParserWarning>,
}

/// One observed column inside a source table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedColumn {
    pub name: String,
    pub data_type: Option<String>,
    pub not_null: bool,
    pub primary_key_ordinal: i64,
}

/// One observed source table plus its discovered shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedTable {
    pub name: String,
    pub present: bool,
    pub required: bool,
    pub row_count: Option<i64>,
    pub columns: Vec<ObservedColumn>,
}

/// Machine-readable schema observation used for diffing and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SchemaObservation {
    pub tables: Vec<ObservedTable>,
}

/// One detected capability with availability and coarse coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CapabilityCoverage {
    pub key: String,
    pub available: bool,
    pub populated_rows: usize,
    pub total_rows: usize,
    pub notes: Vec<String>,
}

/// Capability snapshot emitted by one extractor run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CapabilitySnapshot {
    pub items: Vec<CapabilityCoverage>,
}

/// Parsed Chromium/Firefox/Safari URL row before archive ingest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedUrl {
    pub source_url_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub visit_count: i64,
    pub typed_count: i64,
    pub last_visit_ms: i64,
    pub last_visit_iso: String,
    pub hidden: bool,
    /// Raw native timestamp from the source schema, in the unit the
    /// browser-specific SQL predicate expects (chrome_micros for Chromium,
    /// unix_micros for Firefox `moz_places.last_visit_date`, etc.). Used by
    /// `archive/ingest::url_last_visit_marker` to store the incremental
    /// cursor at sub-millisecond precision so the next ingest's
    /// `WHERE last_visit_time >= ?1` predicate doesn't re-match the same
    /// URL. None when the parser hasn't been migrated to expose it; the
    /// marker then falls back to `last_visit_ms` (legacy behaviour, still
    /// loses up to 999 µs per URL).
    #[serde(default)]
    pub source_last_visit_marker: Option<i64>,
}

/// Parsed browser visit row before archive ingest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedVisit {
    pub source_visit_id: i64,
    pub source_url_id: i64,
    pub url: String,
    pub title: Option<String>,
    pub visit_time_ms: i64,
    pub visit_time_iso: String,
    pub from_visit: Option<i64>,
    pub transition: Option<i64>,
    pub visit_duration_ms: Option<i64>,
    pub is_known_to_sync: bool,
    pub visited_link_id: Option<i64>,
    pub external_referrer_url: Option<String>,
    pub app_id: Option<String>,
}

/// Parsed Chromium download row before archive ingest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDownload {
    pub source_download_id: i64,
    pub guid: Option<String>,
    pub current_path: Option<String>,
    pub target_path: Option<String>,
    pub start_time_ms: Option<i64>,
    pub start_time_iso: Option<String>,
    pub received_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub state: Option<i64>,
    pub mime_type: Option<String>,
    pub original_mime_type: Option<String>,
}

/// Parsed Chromium keyword-search term row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedSearchTerm {
    #[serde(default)]
    pub url_last_visit_time: i64,
    pub keyword_id: i64,
    pub url_id: i64,
    pub term: String,
    pub normalized_term: String,
}

/// Parsed Chromium favicon bitmap metadata and bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFavicon {
    pub page_url: String,
    pub icon_url: String,
    pub icon_type: Option<i64>,
    pub width: i64,
    pub height: i64,
    pub last_updated_ms: i64,
    pub last_updated_iso: String,
    pub image_data: Option<Vec<u8>>,
}

/// Browser-independent search evidence extracted from source-native rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchEvidence {
    pub source_visit_id: Option<i64>,
    pub source_url_id: Option<i64>,
    pub evidence_key: String,
    pub evidence_value: String,
    pub normalized_value: Option<String>,
    pub source_field: String,
}

/// Browser-independent navigation evidence extracted from source-native rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationEvidence {
    pub source_visit_id: i64,
    pub edge_kind: String,
    pub target_visit_id: Option<i64>,
    pub target_url: Option<String>,
    pub transition: Option<i64>,
    pub source_field: String,
}

/// Browser-independent engagement evidence extracted from source-native rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngagementEvidence {
    pub source_visit_id: i64,
    pub metric_key: String,
    pub metric_value_int: Option<i64>,
    pub metric_value_real: Option<f64>,
    pub source_field: String,
}

/// Browser-independent context evidence extracted from source-native rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextEvidence {
    pub source_visit_id: Option<i64>,
    pub source_url_id: Option<i64>,
    pub context_key: String,
    pub value_json: String,
    pub source_field: String,
}

/// Typed evidence families emitted by one extractor run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TypedEvidenceBatch {
    pub search: Vec<SearchEvidence>,
    pub navigation: Vec<NavigationEvidence>,
    pub engagement: Vec<EngagementEvidence>,
    pub context: Vec<ContextEvidence>,
}

/// Cold preserved native entity captured from a source artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeEntity {
    pub entity_kind: String,
    pub native_primary_key: String,
    pub parent_native_primary_key: Option<String>,
    pub payload_json: String,
    pub metadata: BTreeMap<String, String>,
}

/// One bounded source-evidence chunk emitted while parser rows are still streaming.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SourceEvidenceChunk {
    pub typed_evidence: TypedEvidenceBatch,
    pub native_entities: Vec<NativeEntity>,
}

impl SourceEvidenceChunk {
    /// Keeps streaming callers from writing empty source-evidence chunks.
    pub fn is_empty(&self) -> bool {
        self.typed_evidence.search.is_empty()
            && self.typed_evidence.navigation.is_empty()
            && self.typed_evidence.engagement.is_empty()
            && self.typed_evidence.context.is_empty()
            && self.native_entities.is_empty()
    }
}

/// Full parsed Chromium-style history payload returned by the parser crate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChromiumHistory {
    pub inspection: DatabaseInspection,
    pub schema_observation: SchemaObservation,
    pub capability_snapshot: CapabilitySnapshot,
    pub urls: Vec<ParsedUrl>,
    pub visits: Vec<ParsedVisit>,
    pub downloads: Vec<ParsedDownload>,
    pub search_terms: Vec<ParsedSearchTerm>,
    pub favicons: Vec<ParsedFavicon>,
    pub typed_evidence: TypedEvidenceBatch,
    pub native_entities: Vec<NativeEntity>,
    pub warnings: Vec<ParserWarning>,
}

/// Parser metadata returned after canonical row batches have been streamed away.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamedHistory {
    pub inspection: DatabaseInspection,
    pub schema_observation: SchemaObservation,
    pub capability_snapshot: CapabilitySnapshot,
    pub typed_evidence: TypedEvidenceBatch,
    pub native_entities: Vec<NativeEntity>,
    pub warnings: Vec<ParserWarning>,
}

/// Consumer trait for parser-side canonical row batches.
///
/// Archive ingest implements this so hot canonical rows can be persisted while
/// the parser is still scanning the staged source database, instead of waiting
/// for one giant `ParsedHistory` allocation to complete first.
pub trait HistoryBatchConsumer {
    type Error;

    /// The archive persists this source snapshot boundary with each URL batch.
    /// Collectors need no durable cursor and leave this notification unused.
    fn begin_urls(&mut self, _scan: UrlScan) -> Result<(), Self::Error> {
        Ok(())
    }

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error>;

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error>;

    fn downloads(&mut self, _batch: Vec<ParsedDownload>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn search_terms(&mut self, _batch: Vec<ParsedSearchTerm>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn favicons(&mut self, _batch: Vec<ParsedFavicon>) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Receives cold source evidence while the parser is still scanning the source.
    ///
    /// Import callers override this with a bounded spool sink. Read-only parser
    /// callers keep the default and let evidence remain in the returned report.
    fn source_evidence(&mut self, _chunk: SourceEvidenceChunk) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Signals whether typed evidence/native rows should be retained in `StreamedHistory`.
    ///
    /// Large import consumers return `false` so source evidence flows through
    /// `source_evidence` instead of accumulating one full in-memory report.
    fn retain_source_evidence_in_report(&self) -> bool {
        true
    }
}

/// Wraps parser failures and downstream consumer failures for streamed ingest.
#[derive(Debug, Error)]
pub enum StreamHistoryError<E> {
    #[error(transparent)]
    Parse(#[from] crate::ParseError),
    #[error("stream consumer failed: {0}")]
    Consumer(E),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_unix_millis_floors_negatives_and_caps_far_future() {
        assert_eq!(clamp_unix_millis(-1), 0);
        assert_eq!(clamp_unix_millis(i64::MIN), 0);
        assert_eq!(clamp_unix_millis(1_711_965_600_000), 1_711_965_600_000);
        assert_eq!(clamp_unix_millis(MAX_VALID_UNIX_MS), MAX_VALID_UNIX_MS);
        assert_eq!(clamp_unix_millis(i64::MAX), MAX_VALID_UNIX_MS);
    }
}
