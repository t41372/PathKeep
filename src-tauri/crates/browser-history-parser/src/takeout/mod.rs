//! Google Takeout parser boundary.
//!
//! ## Responsibilities
//! - Recognize Takeout payload files from directories or zip archives.
//! - Parse payloads into canonical visit/url rows where possible while keeping
//!   source-native evidence attached to the same extractor contract.
//! - Offer payload-level streaming so archive import can persist canonical rows
//!   before an entire Takeout payload has been materialized in memory.
//!
//! ## Not responsible for
//! - Browser discovery or staging live browser databases.
//! - Writing canonical archive/source-evidence rows.
//! - Quarantine policy or import-batch review logic.
//!
//! ## Dependencies
//! - `serde_json` for payload parsing.
//! - `walkdir` / `zip` for directory and zip traversal.
//! - Shared parser read models from `crate::types`.
//!
//! ## Performance notes
//! - Payload streaming keeps canonical URL/visit rows chunked for import flows,
//!   but Takeout source-native evidence still accumulates until the parser has
//!   finished one payload. Downstream archive code is responsible for spilling
//!   that deferred evidence out of memory when needed.

mod browser_history;
mod json_stream;
mod payloads;
mod source;

#[cfg(test)]
mod tests;

use crate::{
    ParseError,
    types::{
        ChromiumHistory, HistoryBatchConsumer, ParsedUrl, ParsedVisit, StreamHistoryError,
        StreamedHistory,
    },
};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub use source::{inspect_history, recognize_payload};

pub const KIND_JSONL: &str = "jsonl";
pub const KIND_BROWSER_JSON: &str = "browser-json";
pub const KIND_TYPED_URL_JSON: &str = "typed-url-json";
pub const KIND_SESSION_JSON: &str = "session-json";
pub const KIND_INDEX: &str = "takeout-index";

/// Describes the canonical row counts emitted from one Takeout payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TakeoutPayloadCounts {
    pub urls: usize,
    pub visits: usize,
    pub downloads: usize,
    pub search_terms: usize,
    pub favicons: usize,
}

/// Parser metadata returned after one Takeout payload has streamed canonical rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TakeoutPayloadStreamReport {
    pub kind: String,
    pub history: StreamedHistory,
    pub counts: TakeoutPayloadCounts,
    pub record_count: usize,
    pub skipped_missing_visit_time: usize,
}

/// Full Takeout payload report used by preview/read-only consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TakeoutPayloadReport {
    pub kind: String,
    pub history: ChromiumHistory,
    pub record_count: usize,
    pub skipped_missing_visit_time: usize,
}

/// Aggregated parser metadata returned after a whole Takeout source has streamed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TakeoutStreamReport {
    pub history: StreamedHistory,
    pub counts: TakeoutPayloadCounts,
    pub record_count: usize,
    pub skipped_missing_visit_time: usize,
}

/// Parses a whole Takeout source into an in-memory Chromium-style history payload.
///
/// This is the compatibility collector path used by read-only consumers. Import
/// and other large-data flows should prefer [`stream_payload`] so canonical rows
/// can be written before an entire payload has been materialized.
pub fn parse_history(path: &Path) -> Result<ChromiumHistory, ParseError> {
    let mut collector = TakeoutHistoryCollector::default();
    let streamed = stream_history(path, 10_000, &mut collector).map_err(|error| match error {
        StreamHistoryError::Parse(error) => error,
        StreamHistoryError::Consumer(never) => match never {},
    })?;
    Ok(ChromiumHistory {
        inspection: streamed.history.inspection,
        schema_observation: streamed.history.schema_observation,
        capability_snapshot: streamed.history.capability_snapshot,
        urls: collector.urls.into_values().collect(),
        visits: collector.visits,
        downloads: Vec::new(),
        search_terms: Vec::new(),
        favicons: Vec::new(),
        typed_evidence: streamed.history.typed_evidence,
        native_entities: streamed.history.native_entities,
        warnings: streamed.history.warnings,
    })
}

/// Streams one Takeout source into a caller-provided canonical row consumer.
///
/// The consumer sees URL and visit batches as each payload is parsed, which
/// lets archive import start persisting canonical rows before the whole Takeout
/// source has been merged into one giant `ChromiumHistory` allocation.
pub fn stream_history<C>(
    path: &Path,
    chunk_size: usize,
    consumer: &mut C,
) -> Result<TakeoutStreamReport, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
    C::Error: std::fmt::Display,
{
    let inspection = inspect_history(path)?;
    let mut reports = Vec::new();
    for file in source::gather_takeout_files(path)? {
        let Some(kind) = recognize_payload(&file.path) else {
            continue;
        };
        if kind == KIND_INDEX {
            continue;
        }
        let bytes = source::read_takeout_file(path, &file)?;
        reports.push(stream_payload(&file.path, kind, &bytes, chunk_size, consumer)?);
    }
    Ok(merge_stream_reports(inspection, reports))
}

/// Parses one recognized Takeout payload into a full in-memory parser report.
///
/// This remains the canonical preview/read-model contract for inspection flows.
/// Import/write-heavy callers should prefer [`stream_payload`] so URL/visit
/// rows can be consumed incrementally.
pub fn parse_payload(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<TakeoutPayloadReport, ParseError> {
    let mut collector = TakeoutHistoryCollector::default();
    let streamed =
        stream_payload(source_path, kind, bytes, 10_000, &mut collector).map_err(|error| {
            match error {
                StreamHistoryError::Parse(error) => error,
                StreamHistoryError::Consumer(never) => match never {},
            }
        })?;
    Ok(TakeoutPayloadReport {
        kind: streamed.kind,
        history: ChromiumHistory {
            inspection: streamed.history.inspection,
            schema_observation: streamed.history.schema_observation,
            capability_snapshot: streamed.history.capability_snapshot,
            urls: collector.urls.into_values().collect(),
            visits: collector.visits,
            downloads: Vec::new(),
            search_terms: Vec::new(),
            favicons: Vec::new(),
            typed_evidence: streamed.history.typed_evidence,
            native_entities: streamed.history.native_entities,
            warnings: streamed.history.warnings,
        },
        record_count: streamed.record_count,
        skipped_missing_visit_time: streamed.skipped_missing_visit_time,
    })
}

/// Streams one recognized Takeout payload into a caller-provided canonical row consumer.
///
/// Browser-history payloads emit canonical URL and visit batches while they are
/// parsed. Native-only payloads still return source-native evidence and
/// capability metadata but do not emit canonical rows.
pub fn stream_payload<C>(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
    chunk_size: usize,
    consumer: &mut C,
) -> Result<TakeoutPayloadStreamReport, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
    C::Error: std::fmt::Display,
{
    payloads::stream_payload(source_path, kind, bytes, chunk_size, consumer)
}

/// Collector used to preserve the old `parse_*` API on top of streamed rows.
#[derive(Debug, Default)]
struct TakeoutHistoryCollector {
    urls: std::collections::BTreeMap<i64, ParsedUrl>,
    visits: Vec<ParsedVisit>,
}

impl HistoryBatchConsumer for TakeoutHistoryCollector {
    type Error = std::convert::Infallible;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        for url in batch {
            self.urls
                .entry(url.source_url_id)
                .and_modify(|existing| merge_collected_url(existing, &url))
                .or_insert(url);
        }
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.visits.extend(batch);
        Ok(())
    }
}

fn merge_collected_url(existing: &mut ParsedUrl, next: &ParsedUrl) {
    existing.visit_count += next.visit_count;
    existing.typed_count += next.typed_count;
    if next.last_visit_ms >= existing.last_visit_ms {
        existing.last_visit_ms = next.last_visit_ms;
        existing.last_visit_iso = next.last_visit_iso.clone();
        if next.title.is_some() {
            existing.title = next.title.clone();
        }
    }
}

fn merge_stream_reports(
    inspection: crate::types::DatabaseInspection,
    reports: Vec<TakeoutPayloadStreamReport>,
) -> TakeoutStreamReport {
    let mut history = StreamedHistory {
        inspection,
        schema_observation: crate::types::SchemaObservation::default(),
        capability_snapshot: crate::types::CapabilitySnapshot::default(),
        typed_evidence: crate::types::TypedEvidenceBatch::default(),
        native_entities: Vec::new(),
        warnings: Vec::new(),
    };
    let mut counts = TakeoutPayloadCounts::default();
    let mut record_count = 0usize;
    let mut skipped_missing_visit_time = 0usize;
    let mut capabilities =
        std::collections::BTreeMap::<String, crate::types::CapabilityCoverage>::new();

    for report in reports {
        let TakeoutPayloadStreamReport {
            history:
                StreamedHistory {
                    schema_observation,
                    capability_snapshot,
                    typed_evidence,
                    native_entities,
                    warnings,
                    ..
                },
            counts: report_counts,
            record_count: report_record_count,
            skipped_missing_visit_time: report_skipped_missing_visit_time,
            ..
        } = report;
        history.schema_observation.tables.extend(schema_observation.tables);
        history.typed_evidence.search.extend(typed_evidence.search);
        history.typed_evidence.navigation.extend(typed_evidence.navigation);
        history.typed_evidence.engagement.extend(typed_evidence.engagement);
        history.typed_evidence.context.extend(typed_evidence.context);
        history.native_entities.extend(native_entities);
        history.warnings.extend(warnings);
        counts.urls += report_counts.urls;
        counts.visits += report_counts.visits;
        counts.downloads += report_counts.downloads;
        counts.search_terms += report_counts.search_terms;
        counts.favicons += report_counts.favicons;
        record_count += report_record_count;
        skipped_missing_visit_time += report_skipped_missing_visit_time;
        for capability in capability_snapshot.items {
            capabilities
                .entry(capability.key.clone())
                .and_modify(|existing| {
                    existing.available |= capability.available;
                    existing.populated_rows += capability.populated_rows;
                    existing.total_rows += capability.total_rows;
                    existing.notes.extend(capability.notes.clone());
                })
                .or_insert(capability);
        }
    }

    history.capability_snapshot =
        crate::types::CapabilitySnapshot { items: capabilities.into_values().collect() };
    TakeoutStreamReport { history, counts, record_count, skipped_missing_visit_time }
}
