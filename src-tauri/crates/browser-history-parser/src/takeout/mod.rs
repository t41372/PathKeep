//! Google Takeout parser boundary.
//!
//! ## Responsibilities
//! - Recognize Takeout payload files by path, and by a short read of their content.
//! - Parse payloads into canonical visit/url rows where possible while keeping
//!   source-native evidence attached to the same extractor contract.
//! - Offer payload-level streaming so archive import can persist canonical rows
//!   before an entire Takeout payload has been materialized in memory.
//!
//! ## Not responsible for
//! - Browser discovery or staging live browser databases.
//! - Walking a Takeout folder or zip (`vault_core::takeout` does that).
//! - Writing canonical archive/source-evidence rows.
//! - Quarantine policy or import-batch review logic.
//!
//! ## Dependencies
//! - `serde_json` for payload parsing.
//! - `zip` to read the start of a payload inside a Takeout zip.
//! - Shared parser read models from `crate::types`.
//!
//! ## Performance notes
//! - Payload streaming keeps canonical URL/visit rows chunked for import flows,
//!   and can now hand source-native evidence chunks to downstream import code
//!   before one payload has fully materialized.
//! - Read-only preview/report helpers can still retain a full payload report,
//!   but import flows should disable report-side source-evidence retention so
//!   payload-sized native blobs do not stay in RAM unnecessarily.

mod browser_history;
mod json_stream;
mod my_activity;
mod payloads;
mod source;

#[cfg(test)]
mod tests;

use crate::{
    ParseError,
    types::{
        ChromiumHistory, HistoryBatchConsumer, NativeEntity, ParsedDownload, ParsedFavicon,
        ParsedSearchTerm, ParsedUrl, ParsedVisit, StreamHistoryError, StreamedHistory,
        TypedEvidenceBatch,
    },
};
use serde::{Deserialize, Serialize};

pub use source::{classify_payload_path, classify_payload_path_with_sniff, recognize_payload};

pub const KIND_BROWSER_JSON: &str = "browser-json";
/// My Activity's Chrome entries as JSON (`docs/architecture/takeout-my-activity-tradeoff.md`).
pub const KIND_MY_ACTIVITY_JSON: &str = "my-activity-json";
pub const KIND_TYPED_URL_JSON: &str = "typed-url-json";
pub const KIND_SESSION_JSON: &str = "session-json";
pub const KIND_INDEX: &str = "takeout-index";

/// High-level import disposition for one Takeout file path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeoutPathDisposition {
    /// Dedicated Chrome history payload that PathKeep should import into canonical visits.
    WillImport,
    /// Known file that PathKeep intentionally keeps out of the current Chrome-first import scope.
    KnownIgnored,
    /// History-adjacent file that deserves user review because it might look importable.
    NeedsReview,
}

/// Classification result for one Takeout file path before parsing starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TakeoutPathMatch {
    pub family: &'static str,
    pub recognized_kind: Option<&'static str>,
    pub locale: Option<&'static str>,
    pub disposition: TakeoutPathDisposition,
    pub reason_code: &'static str,
}

/// Controls which non-canonical payloads are accumulated during Takeout streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TakeoutStreamOptions {
    /// Keeps typed evidence and native entities for later cold source-evidence persistence.
    pub collect_source_evidence: bool,
    /// Retains source-evidence vectors in the returned parser report.
    ///
    /// Read-only callers keep this enabled so they can inspect the full parser
    /// output. Import flows should disable it and rely on streamed chunks
    /// instead so a single payload does not retain one giant native blob batch.
    pub retain_report_source_evidence: bool,
}

impl Default for TakeoutStreamOptions {
    fn default() -> Self {
        Self { collect_source_evidence: true, retain_report_source_evidence: true }
    }
}

/// One streamed slice of Takeout source-native evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TakeoutSourceEvidenceChunk {
    pub typed_evidence: TypedEvidenceBatch,
    pub native_entities: Vec<NativeEntity>,
}

impl TakeoutSourceEvidenceChunk {
    /// Returns `true` when no evidence rows are present in this chunk.
    pub fn is_empty(&self) -> bool {
        self.typed_evidence.search.is_empty()
            && self.typed_evidence.navigation.is_empty()
            && self.typed_evidence.engagement.is_empty()
            && self.typed_evidence.context.is_empty()
            && self.native_entities.is_empty()
    }
}

/// Consumer for streamed Takeout source-native evidence chunks.
pub trait TakeoutSourceEvidenceConsumer<E> {
    /// Receives one chunk of typed evidence and native entities while a payload is still parsing.
    fn source_evidence(&mut self, chunk: TakeoutSourceEvidenceChunk) -> Result<(), E>;
}

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
    pub earliest_visit_iso: Option<String>,
    pub latest_visit_iso: Option<String>,
}

/// Full Takeout payload report used by preview/read-only consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TakeoutPayloadReport {
    pub kind: String,
    pub history: ChromiumHistory,
    pub record_count: usize,
    pub skipped_missing_visit_time: usize,
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
    stream_payload_with_options(
        source_path,
        kind,
        bytes,
        chunk_size,
        TakeoutStreamOptions::default(),
        consumer,
    )
}

/// Streams one recognized Takeout payload with explicit source-evidence options.
///
/// Inspection flows use this to skip source-native evidence accumulation when
/// they only need candidate counts and capped preview rows.
pub fn stream_payload_with_options<C>(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
    chunk_size: usize,
    options: TakeoutStreamOptions,
    consumer: &mut C,
) -> Result<TakeoutPayloadStreamReport, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
    C::Error: std::fmt::Display,
{
    let mut adapter = CanonicalOnlyTakeoutConsumer { inner: consumer };
    payloads::stream_payload(source_path, kind, bytes, chunk_size, options, &mut adapter)
}

/// Streams one recognized Takeout payload and forwards source-evidence chunks to the consumer.
///
/// Import flows use this path so canonical URL/visit rows and source-native
/// evidence can both move forward while the payload is still parsing, without
/// retaining a second full evidence batch in the returned report.
pub fn stream_payload_with_sink<C>(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
    chunk_size: usize,
    options: TakeoutStreamOptions,
    consumer: &mut C,
) -> Result<TakeoutPayloadStreamReport, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer + TakeoutSourceEvidenceConsumer<C::Error>,
    C::Error: std::fmt::Display,
{
    payloads::stream_payload(source_path, kind, bytes, chunk_size, options, consumer)
}

/// Collector used to preserve the old `parse_*` API on top of streamed rows.
#[derive(Debug, Default)]
struct TakeoutHistoryCollector {
    urls: std::collections::BTreeMap<i64, ParsedUrl>,
    visits: Vec<ParsedVisit>,
}

/// Adapter used by read-only callers that only need canonical row batches.
struct CanonicalOnlyTakeoutConsumer<'a, C> {
    inner: &'a mut C,
}

impl<C> HistoryBatchConsumer for CanonicalOnlyTakeoutConsumer<'_, C>
where
    C: HistoryBatchConsumer,
{
    type Error = C::Error;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        self.inner.urls(batch)
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.inner.visits(batch)
    }

    fn downloads(&mut self, batch: Vec<ParsedDownload>) -> Result<(), Self::Error> {
        self.inner.downloads(batch)
    }

    fn search_terms(&mut self, batch: Vec<ParsedSearchTerm>) -> Result<(), Self::Error> {
        self.inner.search_terms(batch)
    }

    fn favicons(&mut self, batch: Vec<ParsedFavicon>) -> Result<(), Self::Error> {
        self.inner.favicons(batch)
    }
}

impl<C> TakeoutSourceEvidenceConsumer<C::Error> for CanonicalOnlyTakeoutConsumer<'_, C>
where
    C: HistoryBatchConsumer,
{
    fn source_evidence(&mut self, _chunk: TakeoutSourceEvidenceChunk) -> Result<(), C::Error> {
        Ok(())
    }
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

/// Accepts a bounded file/ZIP reader so imports never retain the complete payload.
pub fn stream_payload_reader_with_sink<C>(
    source_path: &str,
    kind: &str,
    reader: impl std::io::Read,
    chunk_size: usize,
    options: TakeoutStreamOptions,
    consumer: &mut C,
) -> Result<TakeoutPayloadStreamReport, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer + TakeoutSourceEvidenceConsumer<C::Error>,
    C::Error: std::fmt::Display,
{
    payloads::stream_payload(source_path, kind, reader, chunk_size, options, consumer)
}
#[cfg(test)]
mod reader_tests;
