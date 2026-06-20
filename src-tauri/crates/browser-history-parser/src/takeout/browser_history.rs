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
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader},
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
            // `BufRead::lines()` yields `Err(InvalidData)` on any invalid UTF-8
            // byte — independent of I/O — so an adversarial / truncated `.jsonl`
            // Takeout export must fail gracefully here, never panic the worker.
            let line = line.map_err(|source| {
                StreamHistoryError::Parse(ParseError::ReadSource {
                    path: std::path::PathBuf::from(source_path),
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
    earliest_visit_micros: Option<i64>,
    latest_visit_micros: Option<i64>,
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
            earliest_visit_micros: None,
            latest_visit_micros: None,
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
                self.earliest_visit_micros =
                    Some(self.earliest_visit_micros.map_or(record.visit_time_micros, |current| {
                        current.min(record.visit_time_micros)
                    }));
                self.latest_visit_micros =
                    Some(self.latest_visit_micros.map_or(record.visit_time_micros, |current| {
                        current.max(record.visit_time_micros)
                    }));
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
            TakeoutSourceEvidenceConsumer::source_evidence(
                consumer,
                std::mem::take(&mut self.pending_source_evidence),
            )
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
            earliest_visit_iso: self.earliest_visit_micros.map(chrome_time_to_rfc3339),
            latest_visit_iso: self.latest_visit_micros.map(chrome_time_to_rfc3339),
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
        .and_then(value_as_i64)
        .or_else(|| record.get("timeUsec").and_then(value_as_i64))
        .or_else(|| record.get("time_usec").and_then(value_as_i64))
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
        // `ordinal` is the position of this record within the source
        // file. It ties broken otherwise-identical keys when Google
        // emits multiple Takeout records for the same URL within the
        // same microsecond (sync replay, redirect-within-1µs, multiple
        // devices syncing the same event). Without it, identical
        // {url, visit_time_micros} keys collide on the
        // (source_profile_id, source_visit_id) UNIQUE index and the
        // second visit is silently dropped by INSERT OR IGNORE.
        //
        // Google's Takeout JSON is a deterministic database export, so
        // the same record at the same position survives renames of the
        // source file — the cross-path stability the original B3 fix
        // sought is preserved as long as record order is stable.
        source_visit_id: stable_key_i64(format!("{url}:{visit_time_micros}:{ordinal}").as_bytes()),
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
        // Takeout records don't carry a chromium-style native cursor —
        // they ingest as a one-shot import, not an incremental stream —
        // so leaving the marker unset keeps the watermark off the
        // takeout path entirely.
        source_last_visit_marker: None,
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

fn value_as_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_str().and_then(|candidate| candidate.parse::<i64>().ok()))
}

fn micros_to_unix_ms(value: i64) -> i64 {
    // Match the chromium/firefox/safari converters: floor pre-epoch sentinels at
    // the 1970 epoch and clamp absurd far-future values so the numeric and ISO
    // representations stay consistent.
    crate::types::clamp_unix_millis(value.div_euclid(1_000))
}

fn chrome_time_to_rfc3339(value: i64) -> String {
    chrono::DateTime::from_timestamp_micros(value)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("unix epoch"))
        .to_rfc3339()
}

/// Derives a stable, non-negative `i64` dedup key from raw input bytes.
///
/// ## Why this exists
/// Takeout payloads carry no native database rowid (unlike the Chromium /
/// Firefox / Safari parsers, which read SQLite `rowid`s directly), so the
/// canonical `source_url_id` / `source_visit_id` keys must be *synthesized*
/// from the record's natural identity (`url`, visit time, ordinal). Those keys
/// land in the `(source_profile_id, source_url_id)` and
/// `(source_profile_id, source_visit_id)` UNIQUE indexes and drive
/// `INSERT OR IGNORE` dedup plus the staging `url_id_map` lookup, so a hash
/// collision is not cosmetic: a `source_visit_id` collision silently drops a
/// legitimate visit, and a `source_url_id` collision merges unrelated URLs.
///
/// ## Why SHA-256 (not the former polynomial hash)
/// The previous implementation hex-encoded the input and folded it with
/// `wrapping_mul(31).wrapping_add(byte)` — a Java-style `hashCode()` whose low
/// bits dominate and whose effective spread is far below 64 bits. Per the
/// import-dedup audit (§B5), birthday collisions on that hash hit well before
/// the 14.4M-record design ceiling (≈2^15.5 ≈ 47k records). We replace it with
/// SHA-256, a uniform 256-bit digest, and keep the leading 63 bits (the sign
/// bit is masked off below). Across that 2^63-key space the birthday collision
/// probability at 14.4M (≈2^23.8) keys is ≈n²/(2·2^63) ≈ 1.1e-5 (≈2^-16.4) —
/// negligible — so the dedup keys stay practically unique at scale.
/// `sha2` is already a vetted workspace dependency (RustCrypto), used by
/// vault-core for archive/migration digests.
///
/// ## Contract
/// Total on every `&[u8]`; always returns a value in `0..=i64::MAX`. The
/// sign bit is masked off rather than `.abs()`-ed, so there is no `i64::MIN`
/// overflow corner to special-case. Sacrificing one bit of key space is
/// irrelevant to the collision math above and preserves the documented
/// non-negative key invariant. The hex pre-expansion is dropped: SHA-256
/// consumes raw bytes directly, halving the hashed length.
///
/// ## Stability caveat (cross-version dedup)
/// These keys are persisted as `source_url_id` / `source_visit_id` and are the
/// dedup identity across separate Takeout imports. Changing this algorithm
/// therefore breaks dedup *across the version boundary*: Takeout rows written
/// by a pre-SHA-256 binary will not match a re-import of the same period by a
/// post-SHA-256 binary, so overlapping re-imports across that boundary can
/// duplicate visits. No automatic re-key migration is possible because the
/// `source_visit_id` input includes a per-import `ordinal` that is not
/// persisted. This is acceptable (the old keys were already collision-corrupt,
/// so a fresh re-import is strictly more correct) but **must not be changed
/// again without the same analysis** — see `docs/plan/program/import-dedup-audit.md` §B5.
fn stable_key_i64(bytes: &[u8]) -> i64 {
    let digest = Sha256::digest(bytes);
    // SHA-256 is uniform, so any 8-byte window is an unbiased 64-bit value;
    // take the leading bytes big-endian for a stable, endianness-independent
    // key. Masking the top bit clears the sign without an `.abs()` overflow
    // trap and keeps the result in the non-negative `i64` range.
    let leading = u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 digest is 32 bytes"));
    (leading & (i64::MAX as u64)) as i64
}

#[cfg(test)]
mod stable_key_tests {
    use super::stable_key_i64;
    use std::collections::HashSet;

    /// Contract: `stable_key_i64` is total on `&[u8]` inputs and always
    /// returns a value in `0..=i64::MAX`. The sign bit is masked off, so
    /// there is no `i64::MIN` overflow corner — every input, including
    /// empty, all-zero, all-`0xFF`, and non-UTF-8 byte runs, yields a
    /// non-negative key without panicking.
    #[test]
    fn stable_key_i64_is_non_negative_and_total() {
        let inputs: &[&[u8]] = &[
            b"",
            b"a",
            b"https://example.com",
            b"https://example.com:8080/path:200:42",
            &[0u8; 256],
            &[0xFF; 256],
            b"\x80\x81\x82\x83",
        ];
        for input in inputs {
            let key = stable_key_i64(input);
            assert!(key >= 0, "stable_key_i64({input:?}) returned negative: {key}");
        }
    }

    /// The "stable" half of the contract: the same bytes must always map
    /// to the same key, otherwise dedup across re-imports of the same
    /// Takeout export would break.
    #[test]
    fn stable_key_i64_is_deterministic() {
        let input = b"url::https://example.com/watch?v=abc";
        assert_eq!(stable_key_i64(input), stable_key_i64(input));
    }

    /// Regression for B5: distinct-but-similar inputs must produce
    /// distinct keys. The former polynomial hash let the low bits
    /// dominate, so near-identical URL prefixes and adjacent ordinals
    /// (exactly the shape Takeout emits for `{url}:{visit_time}:{ordinal}`
    /// keys) collided early. SHA-256 decorrelates them. We assert no
    /// collisions across a dense family of near-identical keys *and* that
    /// the entropy is spread into the high bits (the bit the mask keeps),
    /// not just the low byte.
    #[test]
    fn stable_key_i64_separates_near_identical_inputs() {
        let mut keys = HashSet::new();
        let mut high_bits = HashSet::new();
        for ordinal in 0..2_000_i64 {
            for url in ["https://example.com/a", "https://example.com/b", "https://example.org/a"] {
                let key = stable_key_i64(format!("{url}:1700000000000000:{ordinal}").as_bytes());
                assert!(key >= 0);
                // The leading 24 bits exercise the high end of the digest
                // window the sign mask preserves; a hash whose spread
                // collapses into the low bits (the old defect) would barely
                // populate this set.
                high_bits.insert((key >> 39) as u32);
                assert!(
                    keys.insert(key),
                    "unexpected stable_key_i64 collision at ordinal {ordinal} url {url}"
                );
            }
        }
        assert_eq!(keys.len(), 6_000, "every distinct input must yield a distinct key");
        assert!(
            high_bits.len() > 1_000,
            "high bits barely varied ({} distinct) — entropy is not reaching the top of the key",
            high_bits.len()
        );
    }

    /// Avalanche: a single-byte (in fact single-bit) change to the input
    /// must change the key. A weak hash that ignored late or low-entropy
    /// bytes could return the same key for these two strings.
    #[test]
    fn stable_key_i64_single_bit_flip_changes_key() {
        // 'A' (0x41) vs 'a' (0x61) differ by one bit in the final byte.
        let a = stable_key_i64(b"https://example.com/A");
        let b = stable_key_i64(b"https://example.com/a");
        assert_ne!(a, b);
    }
}
