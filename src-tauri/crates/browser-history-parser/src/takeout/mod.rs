//! Google Takeout parser.
//!
//! Takeout is not a SQLite database, but it still enters PathKeep through the
//! same extractor contract: schema observation, capability snapshot, canonical
//! visit facts where possible, and cold preservation of native payloads.

use crate::{
    ParseError,
    observation::capability_snapshot,
    types::{
        CapabilityCoverage, CapabilitySnapshot, ChromiumHistory, ContextEvidence,
        DatabaseInspection, NativeEntity, ObservedColumn, ObservedTable, ParsedUrl, ParsedVisit,
        ParserWarning, SchemaObservation, TypedEvidenceBatch,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use zip::ZipArchive;

pub const KIND_JSONL: &str = "jsonl";
pub const KIND_BROWSER_JSON: &str = "browser-json";
pub const KIND_TYPED_URL_JSON: &str = "typed-url-json";
pub const KIND_SESSION_JSON: &str = "session-json";
pub const KIND_INDEX: &str = "takeout-index";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TakeoutPayloadReport {
    pub kind: String,
    pub history: ChromiumHistory,
    pub record_count: usize,
    pub skipped_missing_visit_time: usize,
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

pub fn inspect_history(path: &Path) -> Result<DatabaseInspection, ParseError> {
    let files = gather_takeout_files(path)?;
    let mut table_names = BTreeSet::new();
    let mut warnings = Vec::new();
    for file in files {
        if let Some(kind) = recognize_payload(&file.path) {
            if kind != KIND_INDEX {
                table_names.insert(kind.to_string());
            }
        }
    }
    if table_names.is_empty() {
        warnings.push(ParserWarning {
            code: "no-recognized-payload".to_string(),
            message: "No importable Takeout payloads were recognized in the provided source."
                .to_string(),
        });
    }
    Ok(DatabaseInspection { table_names: table_names.into_iter().collect(), warnings })
}

pub fn parse_history(path: &Path) -> Result<ChromiumHistory, ParseError> {
    let inspection = inspect_history(path)?;
    let mut reports = Vec::new();
    for file in gather_takeout_files(path)? {
        let Some(kind) = recognize_payload(&file.path) else {
            continue;
        };
        if kind == KIND_INDEX {
            continue;
        }
        let bytes = if file.from_zip {
            read_zip_entry(path, &file.path)?
        } else {
            fs::read(&file.path).map_err(|source| ParseError::ReadSource {
                path: PathBuf::from(&file.path),
                source,
            })?
        };
        reports.push(parse_payload(&file.path, kind, &bytes)?);
    }
    Ok(merge_reports(inspection, reports))
}

pub fn recognize_payload(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".jsonl") {
        Some(KIND_JSONL)
    } else if lower.ends_with(".json")
        && ((lower.contains("typed") && lower.contains("url")) || lower.contains("typedurl"))
    {
        Some(KIND_TYPED_URL_JSON)
    } else if lower.ends_with(".json") && lower.contains("session") {
        Some(KIND_SESSION_JSON)
    } else if lower.ends_with(".json") && (lower.contains("browser") || lower.contains("history")) {
        Some(KIND_BROWSER_JSON)
    } else if lower.ends_with("archive_browser.html") {
        Some(KIND_INDEX)
    } else {
        None
    }
}

pub fn parse_payload(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<TakeoutPayloadReport, ParseError> {
    match kind {
        KIND_JSONL | KIND_BROWSER_JSON => parse_browser_history_payload(source_path, kind, bytes),
        KIND_TYPED_URL_JSON => parse_native_only_payload(source_path, kind, bytes, "takeout-typed-url"),
        KIND_SESSION_JSON => parse_native_only_payload(source_path, kind, bytes, "takeout-session"),
        KIND_INDEX => Ok(empty_report(source_path, kind, vec![ParserWarning {
            code: "index-only".to_string(),
            message: "Takeout index HTML is recognized for audit visibility but does not produce history rows."
                .to_string(),
        }])),
        _ => Err(ParseError::UnsupportedProvider { provider: "google-takeout-payload" }),
    }
}

fn parse_browser_history_payload(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<TakeoutPayloadReport, ParseError> {
    let mut records = Vec::new();
    let mut observed_records = Vec::new();
    let mut skipped_missing_visit_time = 0usize;
    if kind == KIND_JSONL {
        let reader = BufReader::new(bytes);
        for (index, line) in reader.lines().enumerate() {
            let line = line.map_err(|source| ParseError::ReadSource {
                path: PathBuf::from(source_path),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let record = serde_json::from_str::<Value>(&line).map_err(|source| {
                ParseError::Json { path: format!("{source_path} line {}", index + 1), source }
            })?;
            observed_records.push(record.clone());
            match parse_browser_record(source_path, index as i64, &record)? {
                BrowserRecordOutcome::Parsed(record) => records.push(record),
                BrowserRecordOutcome::Ignore => {}
                BrowserRecordOutcome::MissingVisitTime => skipped_missing_visit_time += 1,
            }
        }
    } else {
        observed_records = browser_history_records(bytes, source_path)?;
        for (index, record) in observed_records.iter().enumerate() {
            match parse_browser_record(source_path, index as i64, record)? {
                BrowserRecordOutcome::Parsed(record) => records.push(record),
                BrowserRecordOutcome::Ignore => {}
                BrowserRecordOutcome::MissingVisitTime => skipped_missing_visit_time += 1,
            }
        }
    }

    let urls = dedupe_urls(&records);
    let visits = records
        .iter()
        .map(|record| ParsedVisit {
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
        })
        .collect::<Vec<_>>();

    let native_entities = records
        .iter()
        .map(|record| NativeEntity {
            entity_kind: "takeout-browser-history".to_string(),
            native_primary_key: record.source_visit_id.to_string(),
            parent_native_primary_key: Some(record.source_url_id.to_string()),
            payload_json: record.raw_record.to_string(),
            metadata: BTreeMap::from([
                ("sourcePath".to_string(), record.source_path.clone()),
                ("payloadKind".to_string(), kind.to_string()),
            ]),
        })
        .collect::<Vec<_>>();

    let context = records
        .iter()
        .flat_map(|record| {
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
        })
        .collect::<Vec<_>>();

    let observation = SchemaObservation {
        tables: vec![observed_payload_table("browser-history", observed_records.as_slice())],
    };
    let capabilities = capability_snapshot(vec![
        CapabilityCoverage {
            key: "canonical.visits".to_string(),
            available: !visits.is_empty(),
            populated_rows: visits.len(),
            total_rows: visits.len(),
            notes: vec!["Takeout Browser History payload".to_string()],
        },
        CapabilityCoverage {
            key: "context.takeout.browser_history".to_string(),
            available: true,
            populated_rows: context.len(),
            total_rows: visits.len(),
            notes: vec!["Takeout Browser History metadata".to_string()],
        },
    ]);

    Ok(TakeoutPayloadReport {
        kind: kind.to_string(),
        history: ChromiumHistory {
            inspection: DatabaseInspection {
                table_names: vec!["browser-history".to_string()],
                warnings: Vec::new(),
            },
            schema_observation: observation,
            capability_snapshot: capabilities,
            urls,
            visits,
            downloads: Vec::new(),
            search_terms: Vec::new(),
            favicons: Vec::new(),
            typed_evidence: TypedEvidenceBatch {
                search: Vec::new(),
                navigation: Vec::new(),
                engagement: Vec::new(),
                context,
            },
            native_entities,
            warnings: if skipped_missing_visit_time > 0 {
                vec![ParserWarning {
                    code: "missing-visit-time".to_string(),
                    message: format!(
                        "Skipped {skipped_missing_visit_time} Takeout Browser History record(s) without a usable visit timestamp."
                    ),
                }]
            } else {
                Vec::new()
            },
        },
        record_count: records.len(),
        skipped_missing_visit_time,
    })
}

fn parse_native_only_payload(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
    entity_kind: &str,
) -> Result<TakeoutPayloadReport, ParseError> {
    let records = payload_records(bytes, source_path, kind)?;
    let native_entities = records
        .iter()
        .enumerate()
        .map(|(index, record)| NativeEntity {
            entity_kind: entity_kind.to_string(),
            native_primary_key: native_primary_key(record, index as i64),
            parent_native_primary_key: None,
            payload_json: record.to_string(),
            metadata: BTreeMap::from([
                ("sourcePath".to_string(), source_path.to_string()),
                ("payloadKind".to_string(), kind.to_string()),
            ]),
        })
        .collect::<Vec<_>>();
    let capability_key = match kind {
        KIND_TYPED_URL_JSON => "context.takeout.typed_url",
        KIND_SESSION_JSON => "context.takeout.session",
        _ => "context.takeout.unknown",
    };
    Ok(TakeoutPayloadReport {
        kind: kind.to_string(),
        history: ChromiumHistory {
            inspection: DatabaseInspection {
                table_names: vec![kind.to_string()],
                warnings: Vec::new(),
            },
            schema_observation: SchemaObservation {
                tables: vec![observed_payload_table(kind, records.as_slice())],
            },
            capability_snapshot: capability_snapshot(vec![CapabilityCoverage {
                key: capability_key.to_string(),
                available: !records.is_empty(),
                populated_rows: records.len(),
                total_rows: records.len(),
                notes: vec![format!("Takeout payload kind `{kind}`")],
            }]),
            urls: Vec::new(),
            visits: Vec::new(),
            downloads: Vec::new(),
            search_terms: Vec::new(),
            favicons: Vec::new(),
            typed_evidence: TypedEvidenceBatch::default(),
            native_entities,
            warnings: Vec::new(),
        },
        record_count: records.len(),
        skipped_missing_visit_time: 0,
    })
}

fn empty_report(
    _source_path: &str,
    kind: &str,
    warnings: Vec<ParserWarning>,
) -> TakeoutPayloadReport {
    TakeoutPayloadReport {
        kind: kind.to_string(),
        history: ChromiumHistory {
            inspection: DatabaseInspection {
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
            capability_snapshot: CapabilitySnapshot::default(),
            urls: Vec::new(),
            visits: Vec::new(),
            downloads: Vec::new(),
            search_terms: Vec::new(),
            favicons: Vec::new(),
            typed_evidence: TypedEvidenceBatch::default(),
            native_entities: Vec::new(),
            warnings,
        },
        record_count: 0,
        skipped_missing_visit_time: 0,
    }
}

fn merge_reports(
    inspection: DatabaseInspection,
    reports: Vec<TakeoutPayloadReport>,
) -> ChromiumHistory {
    let mut merged = ChromiumHistory {
        inspection,
        schema_observation: SchemaObservation::default(),
        capability_snapshot: CapabilitySnapshot::default(),
        urls: Vec::new(),
        visits: Vec::new(),
        downloads: Vec::new(),
        search_terms: Vec::new(),
        favicons: Vec::new(),
        typed_evidence: TypedEvidenceBatch::default(),
        native_entities: Vec::new(),
        warnings: Vec::new(),
    };

    let mut capabilities = BTreeMap::<String, CapabilityCoverage>::new();
    for report in reports {
        merged.schema_observation.tables.extend(report.history.schema_observation.tables);
        merged.urls.extend(report.history.urls);
        merged.visits.extend(report.history.visits);
        merged.downloads.extend(report.history.downloads);
        merged.search_terms.extend(report.history.search_terms);
        merged.favicons.extend(report.history.favicons);
        merged.typed_evidence.search.extend(report.history.typed_evidence.search);
        merged.typed_evidence.navigation.extend(report.history.typed_evidence.navigation);
        merged.typed_evidence.engagement.extend(report.history.typed_evidence.engagement);
        merged.typed_evidence.context.extend(report.history.typed_evidence.context);
        merged.native_entities.extend(report.history.native_entities);
        merged.warnings.extend(report.history.warnings);
        for capability in report.history.capability_snapshot.items {
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
    merged.capability_snapshot = CapabilitySnapshot { items: capabilities.into_values().collect() };
    merged
}

fn browser_history_records(bytes: &[u8], source_path: &str) -> Result<Vec<Value>, ParseError> {
    let value = serde_json::from_slice::<Value>(bytes)
        .map_err(|source| ParseError::Json { path: source_path.to_string(), source })?;
    if let Some(array) = value.as_array() {
        return Ok(array.to_vec());
    }
    if let Some(array) = value.get("BrowserHistory").and_then(Value::as_array) {
        return Ok(array.to_vec());
    }
    if let Some(array) = value.get("Browser History").and_then(Value::as_array) {
        return Ok(array.to_vec());
    }
    Ok(Vec::new())
}

fn payload_records(bytes: &[u8], source_path: &str, kind: &str) -> Result<Vec<Value>, ParseError> {
    let value = serde_json::from_slice::<Value>(bytes)
        .map_err(|source| ParseError::Json { path: source_path.to_string(), source })?;
    if let Some(array) = value.as_array() {
        return Ok(array.to_vec());
    }
    let keys = match kind {
        KIND_TYPED_URL_JSON => &["TypedUrl", "Typed Url", "TypedUrls", "typedUrl"][..],
        KIND_SESSION_JSON => &["Session", "Sessions", "session"][..],
        _ => &[][..],
    };
    for key in keys {
        if let Some(array) = value.get(*key).and_then(Value::as_array) {
            return Ok(array.to_vec());
        }
    }
    Ok(Vec::new())
}

fn parse_browser_record(
    source_path: &str,
    ordinal: i64,
    record: &Value,
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
        raw_record: record.clone(),
    }))
}

fn dedupe_urls(records: &[ParsedBrowserRecord]) -> Vec<ParsedUrl> {
    let mut by_url_id = BTreeMap::<i64, ParsedUrl>::new();
    for record in records {
        let last_visit_ms = micros_to_unix_ms(record.visit_time_micros);
        let entry = by_url_id.entry(record.source_url_id).or_insert_with(|| ParsedUrl {
            source_url_id: record.source_url_id,
            url: record.url.clone(),
            title: record.title.clone(),
            visit_count: 0,
            typed_count: 0,
            last_visit_ms,
            last_visit_iso: chrome_time_to_rfc3339(record.visit_time_micros),
            hidden: false,
        });
        entry.visit_count += 1;
        if last_visit_ms >= entry.last_visit_ms {
            entry.last_visit_ms = last_visit_ms;
            entry.last_visit_iso = chrome_time_to_rfc3339(record.visit_time_micros);
            if record.title.is_some() {
                entry.title = record.title.clone();
            }
        }
    }
    by_url_id.into_values().collect()
}

fn observed_payload_table(name: &str, records: &[Value]) -> ObservedTable {
    let mut columns = BTreeSet::new();
    for record in records {
        if let Some(object) = record.as_object() {
            columns.extend(object.keys().cloned());
        }
    }
    ObservedTable {
        name: name.to_string(),
        present: true,
        required: false,
        row_count: Some(records.len() as i64),
        columns: columns
            .into_iter()
            .map(|name| ObservedColumn {
                name,
                data_type: None,
                not_null: false,
                primary_key_ordinal: 0,
            })
            .collect(),
    }
}

fn gather_takeout_files(source: &Path) -> Result<Vec<TakeoutFile>, ParseError> {
    if source.is_dir() {
        return Ok(WalkDir::new(source)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| TakeoutFile { path: entry.path().display().to_string(), from_zip: false })
            .collect());
    }

    let file = fs::File::open(source).map_err(|source_error| ParseError::ReadSource {
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let mut archive = ZipArchive::new(file)?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_file() {
            files.push(TakeoutFile { path: entry.name().to_string(), from_zip: true });
        }
    }
    Ok(files)
}

fn read_zip_entry(source: &Path, entry_name: &str) -> Result<Vec<u8>, ParseError> {
    let file = fs::File::open(source).map_err(|source_error| ParseError::ReadSource {
        path: source.to_path_buf(),
        source: source_error,
    })?;
    let mut archive = ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(|source_error| ParseError::ReadSource {
        path: PathBuf::from(format!("{}::{entry_name}", source.display())),
        source: source_error,
    })?;
    Ok(bytes)
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

fn native_primary_key(record: &Value, ordinal: i64) -> String {
    record
        .get("url")
        .or_else(|| record.get("titleUrl"))
        .or_else(|| record.get("sessionTag"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("row-{ordinal}"))
}

#[derive(Debug, Clone)]
struct TakeoutFile {
    path: String,
    from_zip: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    #[test]
    fn inspect_history_reports_supported_takeout_payloads() {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("BrowserHistory.json"),
            r#"{"BrowserHistory":[{"titleUrl":"https://example.com","pageTitle":"Example","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#,
        )
        .expect("write browser history");
        fs::write(
            dir.path().join("TypedUrl.json"),
            r#"{"TypedUrl":[{"url":"https://example.com","title":"Example"}]}"#,
        )
        .expect("write typed url");
        let inspection = inspect_history(dir.path()).expect("inspect");
        assert!(inspection.table_names.contains(&KIND_BROWSER_JSON.to_string()));
        assert!(inspection.table_names.contains(&KIND_TYPED_URL_JSON.to_string()));
    }

    #[test]
    fn parse_payload_extracts_browser_history_records() {
        let report = parse_payload(
            "BrowserHistory.json",
            KIND_BROWSER_JSON,
            br#"{"BrowserHistory":[{"titleUrl":"https://example.com","pageTitle":"Example","visitedAt":"2026-04-01T10:00:00+00:00","client_id":"abc"}]}"#,
        )
        .expect("parse payload");
        assert_eq!(report.record_count, 1);
        assert_eq!(report.history.visits.len(), 1);
        assert_eq!(report.history.urls.len(), 1);
        assert_eq!(report.history.native_entities.len(), 1);
        assert!(
            report
                .history
                .capability_snapshot
                .items
                .iter()
                .any(|item| item.key == "canonical.visits" && item.available)
        );
    }

    #[test]
    fn parse_payload_preserves_typed_url_and_session_payloads_as_native_entities() {
        let typed = parse_payload(
            "TypedUrl.json",
            KIND_TYPED_URL_JSON,
            br#"{"TypedUrl":[{"url":"https://example.com","title":"Example","hidden":false}]}"#,
        )
        .expect("parse typed url");
        let session = parse_payload(
            "Session.json",
            KIND_SESSION_JSON,
            br#"{"Session":[{"sessionTag":"device-1","tab":[{"navigation":[{"virtual_url":"https://example.com"}]}]}]}"#,
        )
        .expect("parse session");

        assert!(typed.history.visits.is_empty());
        assert_eq!(typed.history.native_entities.len(), 1);
        assert_eq!(session.history.native_entities.len(), 1);
    }

    #[test]
    fn parse_history_reads_directory_and_zip_sources() {
        let dir = tempdir().expect("tempdir");
        fs::write(
            dir.path().join("BrowserHistory.json"),
            r#"{"BrowserHistory":[{"titleUrl":"https://example.com","pageTitle":"Example","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#,
        )
        .expect("write browser history");
        fs::write(dir.path().join("Session.json"), r#"{"Session":[{"sessionTag":"device-1"}]}"#)
            .expect("write session");
        let parsed = parse_history(dir.path()).expect("parse directory");
        assert_eq!(parsed.visits.len(), 1);
        assert_eq!(parsed.native_entities.len(), 2);

        let zip_path = dir.path().join("takeout.zip");
        let file = fs::File::create(&zip_path).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("BrowserHistory.json", options).expect("start browser entry");
        zip.write_all(br#"{"BrowserHistory":[{"titleUrl":"https://example.com/zip","pageTitle":"Zip","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#)
            .expect("write browser entry");
        zip.start_file("TypedUrl.json", options).expect("start typed entry");
        zip.write_all(br#"{"TypedUrl":[{"url":"https://example.com/zip","title":"Zip"}]}"#)
            .expect("write typed entry");
        zip.finish().expect("finish zip");

        let parsed_zip = parse_history(&zip_path).expect("parse zip");
        assert_eq!(parsed_zip.visits.len(), 1);
        assert_eq!(parsed_zip.native_entities.len(), 2);
    }
}
