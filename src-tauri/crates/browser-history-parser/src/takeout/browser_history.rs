//! Browser-history payload parsing for Google Takeout.
//!
//! ## Responsibilities
//! - Parse browser-history payload records into canonical URL/visit rows.
//! - Emit canonical URL/visit batches to a caller-provided consumer while the
//!   payload is still being read.
//! - Preserve source-native browser-history context and raw record payloads for
//!   later source-evidence persistence.
//!
//! ## Not responsible for
//! - File discovery or zip traversal.
//! - Merging multiple payload files into one combined Takeout source report.
//! - Native-only typed-url/session payload handling.

use super::{
    KIND_BROWSER_JSON, KIND_JSONL, TakeoutPayloadCounts, TakeoutPayloadStreamReport,
    TakeoutSourceEvidenceChunk, TakeoutSourceEvidenceConsumer, TakeoutStreamOptions, json_stream,
};
use crate::{
    ParseError,
    observation::capability_snapshot,
    types::{
        CapabilityCoverage, ContextEvidence, HistoryBatchConsumer, NativeEntity, ObservedColumn,
        ObservedTable, ParsedUrl, ParsedVisit, ParserWarning, SchemaObservation,
        StreamHistoryError, StreamedHistory, TypedEvidenceBatch,
    },
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader},
    path::PathBuf,
};

/// Streams one Takeout browser-history payload into canonical row batches.
pub(super) fn stream_browser_history_payload<C>(
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
    let chunk_size = chunk_size.max(1);
    let mut accumulator = BrowserHistoryAccumulator::new(source_path, kind, chunk_size, options);

    if kind == KIND_JSONL {
        let reader = BufReader::new(bytes);
        for (ordinal, line) in reader.lines().enumerate() {
            let line = line.map_err(|source| {
                StreamHistoryError::Parse(ParseError::ReadSource {
                    path: PathBuf::from(source_path),
                    source,
                })
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let record = serde_json::from_str::<Value>(&line).map_err(|source| {
                StreamHistoryError::Parse(ParseError::Json {
                    path: format!("{source_path} line {}", ordinal + 1),
                    source,
                })
            })?;
            accumulator.observe_record(record, ordinal, consumer)?;
        }
    } else if kind == KIND_BROWSER_JSON {
        json_stream::stream_payload_records(
            bytes,
            source_path,
            &["BrowserHistory", "Browser History"],
            |record, ordinal| accumulator.observe_record(record, ordinal, consumer),
        )
        .map_err(|error| match error {
            json_stream::JsonRecordStreamError::Parse(error) => StreamHistoryError::Parse(error),
            json_stream::JsonRecordStreamError::Callback(error) => error,
        })?;
    } else {
        return Err(StreamHistoryError::Parse(ParseError::UnsupportedProvider {
            provider: "google-takeout-browser-history",
        }));
    }

    accumulator.finish(consumer)
}

#[derive(Debug, Clone)]
struct ParsedBrowserRecord {
    source_path: String,
    url: String,
    title: Option<String>,
    visit_time_micros: i64,
    source_url_id: i64,
    source_visit_id: i64,
    raw_record: Value,
}

enum BrowserRecordOutcome {
    Parsed(ParsedBrowserRecord),
    Ignore,
    MissingVisitTime,
}

struct BrowserHistoryAccumulator<'a> {
    source_path: &'a str,
    kind: &'a str,
    chunk_size: usize,
    options: TakeoutStreamOptions,
    pending_urls: BTreeMap<i64, ParsedUrl>,
    pending_visits: Vec<ParsedVisit>,
    observed_columns: BTreeSet<String>,
    seen_url_ids: BTreeSet<i64>,
    pending_source_evidence: TakeoutSourceEvidenceChunk,
    report_typed_evidence: TypedEvidenceBatch,
    report_native_entities: Vec<NativeEntity>,
    context_evidence_count: usize,
    record_count: usize,
    observed_record_count: usize,
    skipped_missing_visit_time: usize,
}

impl<'a> BrowserHistoryAccumulator<'a> {
    /// Initializes a streaming accumulator for one browser-history payload.
    fn new(
        source_path: &'a str,
        kind: &'a str,
        chunk_size: usize,
        options: TakeoutStreamOptions,
    ) -> Self {
        Self {
            source_path,
            kind,
            chunk_size,
            options,
            pending_urls: BTreeMap::new(),
            pending_visits: Vec::new(),
            observed_columns: BTreeSet::new(),
            seen_url_ids: BTreeSet::new(),
            pending_source_evidence: TakeoutSourceEvidenceChunk::default(),
            report_typed_evidence: TypedEvidenceBatch::default(),
            report_native_entities: Vec::new(),
            context_evidence_count: 0,
            record_count: 0,
            observed_record_count: 0,
            skipped_missing_visit_time: 0,
        }
    }

    /// Observes one raw JSON record and pushes canonical rows/evidence forward.
    fn observe_record<C>(
        &mut self,
        record: Value,
        ordinal: usize,
        consumer: &mut C,
    ) -> Result<(), StreamHistoryError<C::Error>>
    where
        C: HistoryBatchConsumer + TakeoutSourceEvidenceConsumer<C::Error>,
    {
        self.observed_record_count += 1;
        if let Some(object) = record.as_object() {
            self.observed_columns.extend(object.keys().cloned());
        }

        match parse_browser_record(self.source_path, ordinal as i64, record)? {
            BrowserRecordOutcome::Parsed(record) => {
                self.record_count += 1;
                self.seen_url_ids.insert(record.source_url_id);
                self.pending_urls
                    .entry(record.source_url_id)
                    .and_modify(|existing| merge_url_state(existing, &record))
                    .or_insert_with(|| parsed_url_from_record(&record));
                self.pending_visits.push(parsed_visit_from_record(&record));
                if self.options.collect_source_evidence {
                    let native_entity = native_entity_from_record(self.kind, &record);
                    let context_evidence = context_evidence_from_record(&record);
                    self.context_evidence_count += context_evidence.len();
                    if self.options.retain_report_source_evidence {
                        self.report_native_entities.push(native_entity.clone());
                        self.report_typed_evidence.context.extend(context_evidence.iter().cloned());
                    }
                    self.pending_source_evidence.native_entities.push(native_entity);
                    self.pending_source_evidence.typed_evidence.context.extend(context_evidence);
                }
                if self.pending_visits.len() >= self.chunk_size {
                    self.flush(consumer)?;
                }
            }
            BrowserRecordOutcome::Ignore => {}
            BrowserRecordOutcome::MissingVisitTime => self.skipped_missing_visit_time += 1,
        }
        Ok(())
    }

    /// Flushes pending URL/visit rows into the caller-provided consumer.
    fn flush<C>(&mut self, consumer: &mut C) -> Result<(), StreamHistoryError<C::Error>>
    where
        C: HistoryBatchConsumer + TakeoutSourceEvidenceConsumer<C::Error>,
    {
        if !self.pending_urls.is_empty() {
            consumer
                .urls(self.pending_urls.values().cloned().collect())
                .map_err(StreamHistoryError::Consumer)?;
            self.pending_urls.clear();
        }
        if !self.pending_visits.is_empty() {
            consumer
                .visits(std::mem::take(&mut self.pending_visits))
                .map_err(StreamHistoryError::Consumer)?;
        }
        if !self.pending_source_evidence.is_empty() {
            consumer
                .source_evidence(std::mem::take(&mut self.pending_source_evidence))
                .map_err(StreamHistoryError::Consumer)?;
        }
        Ok(())
    }

    /// Finalizes the payload after all records have been observed.
    fn finish<C>(
        mut self,
        consumer: &mut C,
    ) -> Result<TakeoutPayloadStreamReport, StreamHistoryError<C::Error>>
    where
        C: HistoryBatchConsumer + TakeoutSourceEvidenceConsumer<C::Error>,
    {
        self.flush(consumer)?;
        let warnings = if self.skipped_missing_visit_time > 0 {
            vec![ParserWarning {
                code: "missing-visit-time".to_string(),
                message: format!(
                    "Skipped {} Takeout Browser History record(s) without a usable visit timestamp.",
                    self.skipped_missing_visit_time
                ),
            }]
        } else {
            Vec::new()
        };
        Ok(TakeoutPayloadStreamReport {
            kind: self.kind.to_string(),
            history: StreamedHistory {
                inspection: crate::types::DatabaseInspection {
                    table_names: vec!["browser-history".to_string()],
                    warnings: Vec::new(),
                },
                schema_observation: SchemaObservation {
                    tables: vec![ObservedTable {
                        name: "browser-history".to_string(),
                        present: true,
                        required: false,
                        row_count: Some(self.observed_record_count as i64),
                        columns: self
                            .observed_columns
                            .into_iter()
                            .map(|name| ObservedColumn {
                                name,
                                data_type: None,
                                not_null: false,
                                primary_key_ordinal: 0,
                            })
                            .collect(),
                    }],
                },
                capability_snapshot: capability_snapshot(vec![
                    CapabilityCoverage {
                        key: "canonical.visits".to_string(),
                        available: self.record_count > 0,
                        populated_rows: self.record_count,
                        total_rows: self.record_count,
                        notes: vec!["Takeout Browser History payload".to_string()],
                    },
                    CapabilityCoverage {
                        key: "context.takeout.browser_history".to_string(),
                        available: true,
                        populated_rows: self.context_evidence_count,
                        total_rows: self.record_count,
                        notes: vec!["Takeout Browser History metadata".to_string()],
                    },
                ]),
                typed_evidence: self.report_typed_evidence,
                native_entities: self.report_native_entities,
                warnings,
            },
            counts: TakeoutPayloadCounts {
                urls: self.seen_url_ids.len(),
                visits: self.record_count,
                ..TakeoutPayloadCounts::default()
            },
            record_count: self.record_count,
            skipped_missing_visit_time: self.skipped_missing_visit_time,
        })
    }
}

fn parse_browser_record(
    source_path: &str,
    ordinal: i64,
    record: Value,
) -> Result<BrowserRecordOutcome, ParseError> {
    let url = record
        .get("url")
        .or_else(|| record.get("titleUrl"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if url.is_empty() {
        return Ok(BrowserRecordOutcome::Ignore);
    }
    let Some(visit_time_micros) = record
        .get("visitTime")
        .and_then(Value::as_i64)
        .or_else(|| record.get("timeUsec").and_then(Value::as_i64))
        .or_else(|| record.get("visitedAt").and_then(Value::as_str).and_then(parse_iso_to_micros))
    else {
        return Ok(BrowserRecordOutcome::MissingVisitTime);
    };
    let title = record
        .get("title")
        .or_else(|| record.get("pageTitle"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(BrowserRecordOutcome::Parsed(ParsedBrowserRecord {
        source_path: source_path.to_string(),
        source_url_id: stable_key_i64(format!("url::{url}").as_bytes()),
        source_visit_id: stable_key_i64(format!("{source_path}:{ordinal}:{url}").as_bytes()),
        url,
        title,
        visit_time_micros,
        raw_record: record,
    }))
}

fn parsed_url_from_record(record: &ParsedBrowserRecord) -> ParsedUrl {
    ParsedUrl {
        source_url_id: record.source_url_id,
        url: record.url.clone(),
        title: record.title.clone(),
        visit_count: 1,
        typed_count: 0,
        last_visit_ms: micros_to_unix_ms(record.visit_time_micros),
        last_visit_iso: chrome_time_to_rfc3339(record.visit_time_micros),
        hidden: false,
    }
}

fn merge_url_state(existing: &mut ParsedUrl, record: &ParsedBrowserRecord) {
    let last_visit_ms = micros_to_unix_ms(record.visit_time_micros);
    existing.visit_count += 1;
    if last_visit_ms >= existing.last_visit_ms {
        existing.last_visit_ms = last_visit_ms;
        existing.last_visit_iso = chrome_time_to_rfc3339(record.visit_time_micros);
        if record.title.is_some() {
            existing.title = record.title.clone();
        }
    }
}

fn parsed_visit_from_record(record: &ParsedBrowserRecord) -> ParsedVisit {
    ParsedVisit {
        source_visit_id: record.source_visit_id,
        source_url_id: record.source_url_id,
        url: record.url.clone(),
        title: record.title.clone(),
        visit_time_ms: micros_to_unix_ms(record.visit_time_micros),
        visit_time_iso: chrome_time_to_rfc3339(record.visit_time_micros),
        from_visit: None,
        transition: None,
        visit_duration_ms: None,
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: Some("takeout".to_string()),
    }
}

fn native_entity_from_record(kind: &str, record: &ParsedBrowserRecord) -> NativeEntity {
    NativeEntity {
        entity_kind: "takeout-browser-history".to_string(),
        native_primary_key: record.source_visit_id.to_string(),
        parent_native_primary_key: Some(record.source_url_id.to_string()),
        payload_json: record.raw_record.to_string(),
        metadata: BTreeMap::from([
            ("sourcePath".to_string(), record.source_path.clone()),
            ("payloadKind".to_string(), kind.to_string()),
        ]),
    }
}

fn context_evidence_from_record(record: &ParsedBrowserRecord) -> Vec<ContextEvidence> {
    let mut items = Vec::new();
    for (field, key) in [
        ("client_id", "context.takeout.client_id"),
        ("favicon_url", "context.takeout.favicon_url"),
        ("page_transition", "context.takeout.page_transition"),
        ("ptoken", "context.takeout.ptoken"),
    ] {
        if let Some(value) = record.raw_record.get(field) {
            items.push(ContextEvidence {
                source_visit_id: Some(record.source_visit_id),
                source_url_id: Some(record.source_url_id),
                context_key: key.to_string(),
                value_json: value.to_string(),
                source_field: field.to_string(),
            });
        }
    }
    items
}

fn parse_iso_to_micros(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value).ok().map(|date| date.timestamp_micros())
}

fn micros_to_unix_ms(value: i64) -> i64 {
    value.div_euclid(1_000)
}

fn chrome_time_to_rfc3339(value: i64) -> String {
    chrono::DateTime::from_timestamp_micros(value)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("unix epoch"))
        .to_rfc3339()
}

fn stable_key_i64(bytes: &[u8]) -> i64 {
    let hex = hex::encode(bytes);
    hex.bytes().fold(0_i64, |acc, byte| acc.wrapping_mul(31).wrapping_add(byte as i64)).abs()
}
