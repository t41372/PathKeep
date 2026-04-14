//! Shared parser read models.
//!
//! These types intentionally describe parsed source artifacts, not canonical
//! archive rows. They keep parser output deterministic and easy to test before
//! `vault-core` maps the data into archive-specific semantics.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

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
