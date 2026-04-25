//! Takeout payload routing and native-only payload parsing.
//!
//! ## Responsibilities
//! - Dispatch recognized payload kinds to the correct parser implementation.
//! - Parse native-only payload families that do not emit canonical rows.
//! - Preserve compatibility helpers such as empty/index reports without
//!   reintroducing one giant Takeout parser module.

use super::{
    KIND_BROWSER_JSON, KIND_INDEX, KIND_JSONL, KIND_SESSION_JSON, KIND_TYPED_URL_JSON,
    TakeoutPayloadCounts, TakeoutPayloadStreamReport, TakeoutSourceEvidenceChunk,
    TakeoutSourceEvidenceConsumer, TakeoutStreamOptions, browser_history, json_stream,
};
use crate::{
    ParseError,
    observation::capability_snapshot,
    types::{
        CapabilityCoverage, HistoryBatchConsumer, NativeEntity, ObservedColumn, ObservedTable,
        ParserWarning, SchemaObservation, StreamHistoryError, StreamedHistory, TypedEvidenceBatch,
    },
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Routes one recognized Takeout payload through the appropriate parser path.
pub(super) fn stream_payload<C>(
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
    match kind {
        KIND_JSONL | KIND_BROWSER_JSON => {
            browser_history::stream_browser_history_payload(
                source_path,
                kind,
                bytes,
                chunk_size,
                options,
                consumer,
            )
        }
        KIND_TYPED_URL_JSON => {
            stream_native_only_payload::<C>(
                source_path,
                kind,
                bytes,
                chunk_size,
                "takeout-typed-url",
                options,
                consumer,
            )
        }
        KIND_SESSION_JSON => {
            stream_native_only_payload::<C>(
                source_path,
                kind,
                bytes,
                chunk_size,
                "takeout-session",
                options,
                consumer,
            )
        }
        KIND_INDEX => Ok(empty_stream_report(source_path, kind, vec![ParserWarning {
            code: "index-only".to_string(),
            message: "Takeout index HTML is recognized for audit visibility but does not produce history rows."
                .to_string(),
        }])),
        _ => Err(StreamHistoryError::Parse(ParseError::UnsupportedProvider {
            provider: "google-takeout-payload",
        })),
    }
}

fn stream_native_only_payload<C>(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
    chunk_size: usize,
    entity_kind: &str,
    options: TakeoutStreamOptions,
    consumer: &mut C,
) -> Result<TakeoutPayloadStreamReport, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer + TakeoutSourceEvidenceConsumer<C::Error>,
{
    let mut observed_columns = BTreeSet::new();
    let mut native_entities = Vec::new();
    let mut pending_native_entities = Vec::new();
    let chunk_size = chunk_size.max(1);
    let record_count = json_stream::stream_payload_records(
        bytes,
        source_path,
        payload_record_keys(kind),
        |record, ordinal| {
            if let Some(object) = record.as_object() {
                observed_columns.extend(object.keys().cloned());
            }
            if options.collect_source_evidence {
                let entity = NativeEntity {
                    entity_kind: entity_kind.to_string(),
                    native_primary_key: native_primary_key(&record, ordinal as i64),
                    parent_native_primary_key: None,
                    payload_json: record.to_string(),
                    metadata: BTreeMap::from([
                        ("sourcePath".to_string(), source_path.to_string()),
                        ("payloadKind".to_string(), kind.to_string()),
                    ]),
                };
                if options.retain_report_source_evidence {
                    native_entities.push(entity.clone());
                }
                pending_native_entities.push(entity);
                if pending_native_entities.len() >= chunk_size {
                    flush_source_evidence_chunk(
                        consumer,
                        TakeoutSourceEvidenceChunk {
                            typed_evidence: TypedEvidenceBatch::default(),
                            native_entities: std::mem::take(&mut pending_native_entities),
                        },
                    )?;
                }
            }
            Ok::<(), StreamHistoryError<C::Error>>(())
        },
    )
    .map_err(|error| match error {
        json_stream::JsonRecordStreamError::Parse(error) => StreamHistoryError::Parse(error),
        json_stream::JsonRecordStreamError::Callback(error) => error,
    })?;
    flush_source_evidence_chunk(
        consumer,
        TakeoutSourceEvidenceChunk {
            typed_evidence: TypedEvidenceBatch::default(),
            native_entities: pending_native_entities,
        },
    )?;

    let capability_key = match kind {
        KIND_TYPED_URL_JSON => "context.takeout.typed_url",
        KIND_SESSION_JSON => "context.takeout.session",
        _ => "context.takeout.unknown",
    };
    Ok(TakeoutPayloadStreamReport {
        kind: kind.to_string(),
        history: StreamedHistory {
            inspection: crate::types::DatabaseInspection {
                table_names: vec![kind.to_string()],
                warnings: Vec::new(),
            },
            schema_observation: SchemaObservation {
                tables: vec![ObservedTable {
                    name: kind.to_string(),
                    present: true,
                    required: false,
                    row_count: Some(record_count as i64),
                    columns: observed_columns
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
            capability_snapshot: capability_snapshot(vec![CapabilityCoverage {
                key: capability_key.to_string(),
                available: record_count > 0,
                populated_rows: record_count,
                total_rows: record_count,
                notes: vec![format!("Takeout payload kind `{kind}`")],
            }]),
            typed_evidence: TypedEvidenceBatch::default(),
            native_entities,
            warnings: Vec::new(),
        },
        counts: TakeoutPayloadCounts::default(),
        record_count,
        skipped_missing_visit_time: 0,
        earliest_visit_iso: None,
        latest_visit_iso: None,
    })
}

fn flush_source_evidence_chunk<C>(
    consumer: &mut C,
    chunk: TakeoutSourceEvidenceChunk,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer + TakeoutSourceEvidenceConsumer<C::Error>,
{
    if chunk.is_empty() {
        return Ok(());
    }
    TakeoutSourceEvidenceConsumer::source_evidence(consumer, chunk)
        .map_err(StreamHistoryError::Consumer)
}

fn empty_stream_report(
    _source_path: &str,
    kind: &str,
    warnings: Vec<ParserWarning>,
) -> TakeoutPayloadStreamReport {
    TakeoutPayloadStreamReport {
        kind: kind.to_string(),
        history: StreamedHistory {
            inspection: crate::types::DatabaseInspection {
                table_names: vec![kind.to_string()],
                warnings: warnings.clone(),
            },
            schema_observation: SchemaObservation {
                tables: vec![ObservedTable {
                    name: kind.to_string(),
                    present: true,
                    required: false,
                    row_count: Some(0),
                    columns: Vec::new(),
                }],
            },
            capability_snapshot: crate::types::CapabilitySnapshot::default(),
            typed_evidence: TypedEvidenceBatch::default(),
            native_entities: Vec::new(),
            warnings,
        },
        counts: TakeoutPayloadCounts::default(),
        record_count: 0,
        skipped_missing_visit_time: 0,
        earliest_visit_iso: None,
        latest_visit_iso: None,
    }
}

fn payload_record_keys(kind: &str) -> &[&str] {
    match kind {
        KIND_TYPED_URL_JSON => &["TypedUrl", "Typed Url", "TypedUrls", "typedUrl"],
        KIND_SESSION_JSON => &["Session", "Sessions", "session"],
        _ => &[],
    }
}

fn native_primary_key(record: &Value, ordinal: i64) -> String {
    record
        .get("url")
        .or_else(|| record.get("titleUrl"))
        .or_else(|| record.get("sessionTag"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("row-{ordinal}"))
}
