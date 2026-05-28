//! Shared parser read models.
//!
//! These types intentionally describe parsed source artifacts, not canonical
//! archive rows. They keep parser output deterministic and easy to test before
//! `vault-core` maps the data into archive-specific semantics.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use thiserror::Error;

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
    pub after_download_id: i64,
    pub after_favicon_last_updated: i64,
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

    fn native_entity() -> NativeEntity {
        NativeEntity {
            entity_kind: "source-row".to_string(),
            native_primary_key: "row-1".to_string(),
            parent_native_primary_key: None,
            payload_json: "{}".to_string(),
            metadata: BTreeMap::new(),
        }
    }

    #[derive(Default)]
    struct MinimalConsumer {
        urls: usize,
        visits: usize,
    }

    impl HistoryBatchConsumer for MinimalConsumer {
        type Error = std::convert::Infallible;

        fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            self.urls += batch.len();
            Ok(())
        }

        fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            self.visits += batch.len();
            Ok(())
        }
    }

    #[test]
    fn source_evidence_chunk_empty_contract_tracks_each_evidence_family() {
        assert!(SourceEvidenceChunk::default().is_empty());

        let mut search = SourceEvidenceChunk::default();
        search.typed_evidence.search.push(SearchEvidence {
            source_visit_id: Some(1),
            source_url_id: Some(10),
            evidence_key: "query".to_string(),
            evidence_value: "pathkeep".to_string(),
            normalized_value: Some("pathkeep".to_string()),
            source_field: "term".to_string(),
        });
        assert!(!search.is_empty());

        let mut navigation = SourceEvidenceChunk::default();
        navigation.typed_evidence.navigation.push(NavigationEvidence {
            source_visit_id: 1,
            edge_kind: "from_visit".to_string(),
            target_visit_id: Some(2),
            target_url: None,
            transition: Some(1),
            source_field: "from_visit".to_string(),
        });
        assert!(!navigation.is_empty());

        let mut engagement = SourceEvidenceChunk::default();
        engagement.typed_evidence.engagement.push(EngagementEvidence {
            source_visit_id: 1,
            metric_key: "duration".to_string(),
            metric_value_int: Some(50),
            metric_value_real: None,
            source_field: "visit_duration".to_string(),
        });
        assert!(!engagement.is_empty());

        let mut context = SourceEvidenceChunk::default();
        context.typed_evidence.context.push(ContextEvidence {
            source_visit_id: Some(1),
            source_url_id: Some(10),
            context_key: "sync".to_string(),
            value_json: "true".to_string(),
            source_field: "is_known_to_sync".to_string(),
        });
        assert!(!context.is_empty());

        let mut native = SourceEvidenceChunk::default();
        native.native_entities.push(native_entity());
        assert!(!native.is_empty());
    }

    #[test]
    fn optional_consumer_batches_default_to_noop_for_minimal_row_sinks() {
        let mut consumer = MinimalConsumer::default();

        consumer
            .downloads(vec![ParsedDownload {
                source_download_id: 1,
                guid: None,
                current_path: None,
                target_path: None,
                start_time_ms: None,
                start_time_iso: None,
                received_bytes: None,
                total_bytes: None,
                state: None,
                mime_type: None,
                original_mime_type: None,
            }])
            .expect("download default no-op");
        consumer
            .search_terms(vec![ParsedSearchTerm {
                keyword_id: 1,
                url_id: 1,
                term: "pathkeep".to_string(),
                normalized_term: "pathkeep".to_string(),
            }])
            .expect("search term default no-op");
        consumer
            .favicons(vec![ParsedFavicon {
                page_url: "https://example.com".to_string(),
                icon_url: "https://example.com/favicon.ico".to_string(),
                icon_type: Some(1),
                width: 16,
                height: 16,
                last_updated_ms: 1,
                last_updated_iso: "1970-01-01T00:00:00+00:00".to_string(),
                image_data: Some(vec![0x89, b'P', b'N', b'G']),
            }])
            .expect("favicon default no-op");

        assert_eq!(consumer.urls, 0);
        assert_eq!(consumer.visits, 0);
    }
}
