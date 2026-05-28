//! Firefox history parser.
//!
//! This baseline slice supports Firefox-family `places.sqlite` databases for
//! visits and URLs. Downloads, search terms, and favicons intentionally remain
//! unsupported here so the parser stays honest about what it can prove.

use crate::{
    ParseError, ParsedHistory,
    observation::{capability_snapshot, capture_native_row, capture_native_rows, inspect_schema},
    types::{
        CapabilityCoverage, ContextEvidence, DatabaseInspection, HistoryBatchConsumer,
        NavigationEvidence, ParsedUrl, ParsedVisit, ParserWarning, SourceEvidenceChunk,
        StreamHistoryError, StreamedHistory, TypedEvidenceBatch,
    },
};
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags, Row, params};
use std::convert::Infallible;
use std::path::Path;

const INSPECT_TABLES_SQL: &str = "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name";
/// Incremental URL ingest query used by re-imports after at least one
/// previous import. Mirrors the Chromium `INGEST_URLS_SQL` pattern:
///
/// - `last_visit_date >= ?1` catches every place whose most recent visit
///   landed at or after the URL cursor (the common path).
/// - `id IN (SELECT DISTINCT place_id FROM moz_historyvisits WHERE id > ?2)`
///   widens the set to any place referenced by a new visit beyond the visit
///   cursor, even when Firefox didn't bump `moz_places.last_visit_date`.
///   Without this OR, long-tail revisited pages lose their new visits to
///   `skipped_visits++` because the URL is absent from `url_id_map` (B2).
const URLS_SQL: &str = r#"
SELECT
  moz_places.id,
  moz_places.url,
  moz_places.title,
  moz_places.visit_count,
  COALESCE(moz_places.hidden, 0),
  COALESCE(moz_places.last_visit_date, 0)
FROM moz_places
WHERE COALESCE(moz_places.last_visit_date, 0) >= ?1
   OR moz_places.id IN (SELECT DISTINCT place_id FROM moz_historyvisits WHERE id > ?2)
ORDER BY COALESCE(moz_places.last_visit_date, 0) ASC
"#;

/// First-import URL ingest query. When both watermarks are at zero, the
/// `last_visit_date >= 0` predicate already matches every moz_places row,
/// so the OR's `SELECT DISTINCT place_id FROM moz_historyvisits WHERE id > 0`
/// subquery is pure waste — it forces SQLite to scan the entire
/// `moz_historyvisits` table and materialize an ephemeral B-tree of every
/// distinct place_id before the outer filter runs. On a 14.4M-visit Firefox
/// profile that's a multi-GB transient plus multi-minute stall added to
/// the very first import. Mirrors the Chromium `INGEST_URLS_FULL_SQL`
/// optimization — stripping the OR removes the hazard without losing any
/// rows.
const URLS_FULL_SQL: &str = r#"
SELECT
  moz_places.id,
  moz_places.url,
  moz_places.title,
  moz_places.visit_count,
  COALESCE(moz_places.hidden, 0),
  COALESCE(moz_places.last_visit_date, 0)
FROM moz_places
ORDER BY COALESCE(moz_places.last_visit_date, 0) ASC
"#;
const VISITS_SQL: &str = r#"
SELECT
  moz_historyvisits.id,
  moz_historyvisits.place_id,
  moz_places.url,
  moz_places.title,
  moz_historyvisits.visit_date,
  moz_historyvisits.from_visit,
  moz_historyvisits.visit_type
FROM moz_historyvisits
JOIN moz_places
  ON moz_places.id = moz_historyvisits.place_id
WHERE moz_historyvisits.id > ?1
ORDER BY moz_historyvisits.id ASC
"#;

#[derive(Debug, Default)]
struct FirefoxHistoryCollector {
    urls: Vec<ParsedUrl>,
    visits: Vec<ParsedVisit>,
}

impl HistoryBatchConsumer for FirefoxHistoryCollector {
    type Error = Infallible;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        self.urls.extend(batch);
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.visits.extend(batch);
        Ok(())
    }
}

fn stream_sql<T, E>(result: Result<T, rusqlite::Error>) -> Result<T, StreamHistoryError<E>> {
    result.map_err(ParseError::from).map_err(StreamHistoryError::Parse)
}

fn flush_source_evidence<C>(
    consumer: &mut C,
    retain_source_evidence: bool,
    typed_evidence: &mut TypedEvidenceBatch,
    native_entities: &mut Vec<crate::types::NativeEntity>,
    chunk: &mut SourceEvidenceChunk,
) -> Result<(), StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    if chunk.is_empty() {
        return Ok(());
    }

    let chunk = std::mem::take(chunk);
    if retain_source_evidence {
        typed_evidence.navigation.extend(chunk.typed_evidence.navigation);
        typed_evidence.context.extend(chunk.typed_evidence.context);
        native_entities.extend(chunk.native_entities);
    } else {
        consumer.source_evidence(chunk).map_err(StreamHistoryError::Consumer)?;
    }
    Ok(())
}

/// Inspects a Firefox `places.sqlite` file and reports required-table coverage.
pub fn inspect_history(path: &Path) -> Result<DatabaseInspection, ParseError> {
    let connection = open_readonly(path)?;
    let mut statement = connection.prepare(INSPECT_TABLES_SQL)?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut warnings = Vec::new();
    for table_name in ["moz_places", "moz_historyvisits"] {
        if !table_names.iter().any(|existing| existing == table_name) {
            warnings.push(ParserWarning {
                code: "missing-table".to_string(),
                message: format!("required Firefox table `{table_name}` is missing"),
            });
        }
    }

    warnings.push(ParserWarning {
        code: "baseline-support".to_string(),
        message:
            "Firefox baseline ingest captures visits and URLs. Downloads, search terms, and favicons stay unsupported in this slice."
                .to_string(),
    });

    Ok(DatabaseInspection { table_names, warnings })
}

/// Parses a Firefox `places.sqlite` file into parser read models.
pub fn parse_history(
    path: &Path,
    after_visit_id: i64,
    after_url_last_visit_ms: i64,
) -> Result<ParsedHistory, ParseError> {
    let mut collector = FirefoxHistoryCollector::default();
    let streamed =
        stream_history(path, after_visit_id, after_url_last_visit_ms, 10_000, &mut collector)
            .map_err(|error| match error {
                StreamHistoryError::Parse(error) => error,
                StreamHistoryError::Consumer(never) => match never {},
            })?;
    Ok(ParsedHistory {
        inspection: streamed.inspection,
        schema_observation: streamed.schema_observation,
        capability_snapshot: streamed.capability_snapshot,
        urls: collector.urls,
        visits: collector.visits,
        downloads: Vec::new(),
        search_terms: Vec::new(),
        favicons: Vec::new(),
        typed_evidence: streamed.typed_evidence,
        native_entities: streamed.native_entities,
        warnings: streamed.warnings,
    })
}

/// Streams Firefox URL and visit rows into a caller-provided batch consumer.
pub fn stream_history<C>(
    path: &Path,
    after_visit_id: i64,
    after_url_last_visit_ms: i64,
    chunk_size: usize,
    consumer: &mut C,
) -> Result<StreamedHistory, StreamHistoryError<C::Error>>
where
    C: HistoryBatchConsumer,
{
    let inspection = inspect_history(path)?;
    let schema_observation =
        inspect_schema(&open_readonly(path)?, &["moz_places", "moz_historyvisits"])?;
    validate_required_tables(&inspection)?;

    let connection = open_readonly(path)?;
    let chunk_size = chunk_size.max(1);
    let warnings = inspection.warnings.clone();
    let mut visit_count = 0usize;
    let mut navigation_count = 0usize;
    let mut from_visit_count = 0usize;
    let retain_source_evidence = consumer.retain_source_evidence_in_report();
    let mut typed_evidence = TypedEvidenceBatch::default();
    let mut native_entities = Vec::new();
    let mut source_evidence_chunk = SourceEvidenceChunk::default();

    {
        // First-import branch: when both watermarks are zero, the OR
        // subquery in URLS_SQL is wasted work over potentially millions
        // of moz_historyvisits rows. Use URLS_FULL_SQL (no OR clause,
        // no bound params) to skip the materialization. Matches the
        // Chromium pattern at `chromium/mod.rs:383-384`.
        let first_import = after_visit_id == 0 && after_url_last_visit_ms == 0;
        let sql = if first_import { URLS_FULL_SQL } else { URLS_SQL };
        let mut statement = stream_sql(connection.prepare(sql))?;
        let column_names =
            statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
        let mut rows =
            if first_import {
                stream_sql(statement.query([]))?
            } else {
                stream_sql(statement.query(params![
                    unix_ms_to_firefox_time(after_url_last_visit_ms),
                    after_visit_id
                ]))?
            };
        let mut batch = Vec::with_capacity(chunk_size);
        while let Some(row) = stream_sql(rows.next())? {
            batch.push(stream_sql(parsed_url_from_row(row))?);
            source_evidence_chunk.native_entities.push(stream_sql(capture_native_row(
                row,
                &column_names,
                "firefox-place-row",
                "id",
                None,
            ))?);
            if batch.len() >= chunk_size {
                consumer.urls(std::mem::take(&mut batch)).map_err(StreamHistoryError::Consumer)?;
                flush_source_evidence(
                    consumer,
                    retain_source_evidence,
                    &mut typed_evidence,
                    &mut native_entities,
                    &mut source_evidence_chunk,
                )?;
            }
        }
        if !batch.is_empty() {
            consumer.urls(batch).map_err(StreamHistoryError::Consumer)?;
        }
        flush_source_evidence(
            consumer,
            retain_source_evidence,
            &mut typed_evidence,
            &mut native_entities,
            &mut source_evidence_chunk,
        )?;
    }

    {
        let mut statement = stream_sql(connection.prepare(VISITS_SQL))?;
        let column_names =
            statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
        let mut rows = stream_sql(statement.query(params![after_visit_id]))?;
        let mut batch = Vec::with_capacity(chunk_size);
        while let Some(row) = stream_sql(rows.next())? {
            let visit = stream_sql(parsed_visit_from_row(row))?;
            visit_count += 1;
            if let Some(evidence) = navigation_evidence_for_visit(&visit) {
                navigation_count += 1;
                if evidence.target_visit_id.is_some() {
                    from_visit_count += 1;
                }
                source_evidence_chunk.typed_evidence.navigation.push(evidence)
            }
            source_evidence_chunk.typed_evidence.context.push(context_evidence_for_visit(&visit));
            source_evidence_chunk.native_entities.push(stream_sql(capture_native_row(
                row,
                &column_names,
                "firefox-historyvisit-row",
                "id",
                Some("place_id"),
            ))?);
            batch.push(visit);
            if batch.len() >= chunk_size {
                consumer
                    .visits(std::mem::take(&mut batch))
                    .map_err(StreamHistoryError::Consumer)?;
                flush_source_evidence(
                    consumer,
                    retain_source_evidence,
                    &mut typed_evidence,
                    &mut native_entities,
                    &mut source_evidence_chunk,
                )?;
            }
        }
        if !batch.is_empty() {
            consumer.visits(batch).map_err(StreamHistoryError::Consumer)?;
        }
        flush_source_evidence(
            consumer,
            retain_source_evidence,
            &mut typed_evidence,
            &mut native_entities,
            &mut source_evidence_chunk,
        )?;
    }

    for optional_table in
        ["moz_inputhistory", "moz_places_metadata", "moz_places_metadata_search_queries"]
    {
        if inspection.table_names.iter().any(|existing| existing == optional_table) {
            let sql = format!("SELECT * FROM {optional_table}");
            let entity_kind = format!("firefox-{}", optional_table.replace('_', "-"));
            let optional_rows =
                capture_native_rows(&connection, &sql, &[], &entity_kind, "id", None)?;
            if retain_source_evidence {
                native_entities.extend(optional_rows);
            } else {
                source_evidence_chunk.native_entities.extend(optional_rows);
                flush_source_evidence(
                    consumer,
                    retain_source_evidence,
                    &mut typed_evidence,
                    &mut native_entities,
                    &mut source_evidence_chunk,
                )?;
            }
        }
    }

    let capability_snapshot =
        build_capability_snapshot(from_visit_count, navigation_count, visit_count);
    Ok(StreamedHistory {
        inspection,
        schema_observation,
        capability_snapshot,
        typed_evidence,
        native_entities,
        warnings,
    })
}

/// Converts Firefox's microsecond timestamp format to Unix milliseconds.
pub fn firefox_time_to_unix_ms(value: i64) -> i64 {
    value.div_euclid(1_000).max(0)
}

/// Converts Unix milliseconds back into Firefox's microsecond timestamp format.
pub fn unix_ms_to_firefox_time(value: i64) -> i64 {
    value.max(0).saturating_mul(1_000)
}

/// Converts Firefox's microsecond timestamp format to RFC3339.
pub fn firefox_time_to_iso(value: i64) -> String {
    let milliseconds = firefox_time_to_unix_ms(value);
    Utc.timestamp_millis_opt(milliseconds)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("unix epoch"))
        .to_rfc3339()
}

fn open_readonly(path: &Path) -> Result<Connection, ParseError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|source| ParseError::OpenDatabase { path: path.to_path_buf(), source })
}

fn validate_required_tables(inspection: &DatabaseInspection) -> Result<(), ParseError> {
    for table_name in ["moz_places", "moz_historyvisits"] {
        if !inspection.table_names.iter().any(|existing| existing == table_name) {
            return Err(ParseError::MissingTable { table: table_name });
        }
    }
    Ok(())
}

fn parsed_url_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedUrl> {
    let last_visit_date = row.get::<_, i64>(5)?;
    // Firefox's `moz_places.last_visit_date` is microseconds-since-Unix.
    // `last_visit_ms` truncates that to ms; today's incremental watermark
    // is also stored in ms and converted back to firefox_time via
    // `unix_ms_to_firefox_time` (* 1000) at query time, which means the
    // same sub-ms precision loss the Chromium path used to have. Leaving
    // `source_last_visit_marker` unset (None) keeps the legacy watermark
    // semantic until the Firefox cursor unit migration ships; see
    // BACKLOG.md for the planned follow-up.
    Ok(ParsedUrl {
        source_url_id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        visit_count: row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
        typed_count: 0,
        last_visit_ms: firefox_time_to_unix_ms(last_visit_date),
        last_visit_iso: firefox_time_to_iso(last_visit_date),
        hidden: row.get::<_, i64>(4)? != 0,
        source_last_visit_marker: None,
    })
}

fn parsed_visit_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedVisit> {
    let visit_date = row.get::<_, i64>(4)?;
    Ok(ParsedVisit {
        source_visit_id: row.get(0)?,
        source_url_id: row.get(1)?,
        url: row.get(2)?,
        title: row.get(3)?,
        visit_time_ms: firefox_time_to_unix_ms(visit_date),
        visit_time_iso: firefox_time_to_iso(visit_date),
        from_visit: row.get(5)?,
        transition: row.get(6)?,
        visit_duration_ms: None,
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: Some("firefox".to_string()),
    })
}

fn build_capability_snapshot(
    from_visit_count: usize,
    navigation_count: usize,
    visit_count: usize,
) -> crate::types::CapabilitySnapshot {
    capability_snapshot(vec![
        CapabilityCoverage {
            key: "nav.from_visit".to_string(),
            available: from_visit_count > 0,
            populated_rows: from_visit_count,
            total_rows: visit_count,
            notes: vec!["Firefox moz_historyvisits.from_visit".to_string()],
        },
        CapabilityCoverage {
            key: "nav.transition".to_string(),
            available: navigation_count > 0,
            populated_rows: navigation_count,
            total_rows: visit_count,
            notes: vec!["Firefox moz_historyvisits.visit_type".to_string()],
        },
    ])
}

fn navigation_evidence_for_visit(visit: &ParsedVisit) -> Option<NavigationEvidence> {
    (visit.from_visit.is_some() || visit.transition.is_some()).then(|| NavigationEvidence {
        source_visit_id: visit.source_visit_id,
        edge_kind: "visit-navigation".to_string(),
        target_visit_id: visit.from_visit,
        target_url: None,
        transition: visit.transition,
        source_field: "moz_historyvisits.from_visit/visit_type".to_string(),
    })
}

fn context_evidence_for_visit(visit: &ParsedVisit) -> ContextEvidence {
    ContextEvidence {
        source_visit_id: Some(visit.source_visit_id),
        source_url_id: Some(visit.source_url_id),
        context_key: "context.app_id".to_string(),
        value_json: "\"firefox\"".to_string(),
        source_field: "derived.firefox-family".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::tempdir;

    #[derive(Default)]
    struct RecordingConsumer {
        url_batches: Vec<usize>,
        visit_batches: Vec<usize>,
        source_evidence_chunks: Vec<SourceEvidenceChunk>,
        spool_source_evidence: bool,
    }

    impl HistoryBatchConsumer for RecordingConsumer {
        type Error = Infallible;

        fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            self.url_batches.push(batch.len());
            Ok(())
        }

        fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            self.visit_batches.push(batch.len());
            Ok(())
        }

        fn source_evidence(&mut self, chunk: SourceEvidenceChunk) -> Result<(), Self::Error> {
            self.source_evidence_chunks.push(chunk);
            Ok(())
        }

        fn retain_source_evidence_in_report(&self) -> bool {
            !self.spool_source_evidence
        }
    }

    fn write_history_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open firefox fixture");
        connection
            .execute_batch(
                "CREATE TABLE moz_places (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER,
                   hidden INTEGER,
                   last_visit_date INTEGER
                 );
                 CREATE TABLE moz_historyvisits (
                   id INTEGER PRIMARY KEY,
                   place_id INTEGER NOT NULL,
                   visit_date INTEGER NOT NULL,
                   from_visit INTEGER,
                   visit_type INTEGER
                 );",
            )
            .expect("create firefox schema");
        connection
            .execute(
                "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    7_i64,
                    "https://example.com/firefox",
                    "Firefox Example",
                    3_i64,
                    0_i64,
                    1_744_146_000_000_000_i64,
                ],
            )
            .expect("insert place");
        connection
            .execute(
                "INSERT INTO moz_historyvisits (id, place_id, visit_date, from_visit, visit_type)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![11_i64, 7_i64, 1_744_146_000_000_000_i64, Option::<i64>::None, 1_i64,],
            )
            .expect("insert visit");
    }

    #[test]
    fn parse_history_reads_firefox_places_and_visits() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("places.sqlite");
        write_history_fixture(&history_path);

        let parsed = parse_history(&history_path, 0, 0).expect("parse firefox history");

        assert_eq!(parsed.urls.len(), 1);
        assert_eq!(parsed.visits.len(), 1);
        assert_eq!(parsed.urls[0].url, "https://example.com/firefox");
        assert!(!parsed.urls[0].hidden);
        assert_eq!(parsed.visits[0].source_visit_id, 11);
        assert_eq!(parsed.visits[0].app_id.as_deref(), Some("firefox"));
        assert!(parsed.warnings.iter().any(|warning| warning.code == "baseline-support"));
    }

    #[test]
    fn stream_history_flushes_small_batches_and_captures_optional_native_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("places.sqlite");
        write_history_fixture(&history_path);
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute(
                "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    8_i64,
                    "https://example.com/firefox-two",
                    "Firefox Example Two",
                    1_i64,
                    0_i64,
                    1_744_146_000_001_000_i64,
                ],
            )
            .expect("insert second place");
        connection
            .execute(
                "INSERT INTO moz_historyvisits (id, place_id, visit_date, from_visit, visit_type)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![12_i64, 8_i64, 1_744_146_000_001_000_i64, Some(11_i64), 1_i64,],
            )
            .expect("insert second visit");
        connection
            .execute_batch(
                "CREATE TABLE moz_inputhistory (id INTEGER PRIMARY KEY, place_id INTEGER, input TEXT);
                 CREATE TABLE moz_places_metadata (id INTEGER PRIMARY KEY, place_id INTEGER);
                 CREATE TABLE moz_places_metadata_search_queries (id INTEGER PRIMARY KEY, query TEXT);
                 INSERT INTO moz_inputhistory (id, place_id, input) VALUES (1, 7, 'firefox');
                 INSERT INTO moz_places_metadata (id, place_id) VALUES (2, 7);
                 INSERT INTO moz_places_metadata_search_queries (id, query) VALUES (3, 'firefox');",
            )
            .expect("optional firefox tables");
        drop(connection);

        let mut consumer = RecordingConsumer::default();
        let streamed =
            stream_history(&history_path, 0, 0, 1, &mut consumer).expect("stream firefox");

        assert_eq!(consumer.url_batches, vec![1, 1]);
        assert_eq!(consumer.visit_batches, vec![1, 1]);
        assert!(
            streamed
                .native_entities
                .iter()
                .any(|entity| entity.entity_kind == "firefox-moz-inputhistory")
        );
        assert!(
            streamed
                .native_entities
                .iter()
                .any(|entity| entity.entity_kind == "firefox-moz-places-metadata")
        );
        assert!(
            streamed.native_entities.iter().any(|entity| {
                entity.entity_kind == "firefox-moz-places-metadata-search-queries"
            })
        );
    }

    #[test]
    fn incremental_url_query_does_not_take_the_first_import_fast_path_with_one_cursor_zero() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("places.sqlite");
        write_history_fixture(&history_path);
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute(
                "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    8_i64,
                    "https://example.com/firefox-stale",
                    "Firefox stale",
                    1_i64,
                    0_i64,
                    1_000_000_i64,
                ],
            )
            .expect("insert stale place");
        drop(connection);

        let parsed_with_url_cursor =
            parse_history(&history_path, 0, 1_744_146_000_000).expect("parse url cursor only");
        assert!(
            parsed_with_url_cursor
                .urls
                .iter()
                .all(|url| url.url != "https://example.com/firefox-stale"),
            "url-cursor-only incremental imports must exclude stale places without new visits",
        );

        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute(
                "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    9_i64,
                    "https://example.com/firefox-negative",
                    "Firefox negative",
                    1_i64,
                    0_i64,
                    -1_i64,
                ],
            )
            .expect("insert negative-date place");
        drop(connection);

        let parsed_with_visit_cursor =
            parse_history(&history_path, 50, 0).expect("parse visit cursor only");
        assert!(
            parsed_with_visit_cursor
                .urls
                .iter()
                .all(|url| url.url != "https://example.com/firefox-negative"),
            "visit-cursor-only incremental imports must still use the bounded URL query",
        );
    }

    #[test]
    fn stream_history_batches_rows_and_reports_firefox_evidence_counts() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("places.sqlite");
        write_history_fixture(&history_path);
        let connection = Connection::open(&history_path).expect("open fixture");
        for id in 8_i64..=9 {
            connection
                .execute(
                    "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        id,
                        format!("https://example.com/firefox-{id}"),
                        format!("Firefox {id}"),
                        1_i64,
                        0_i64,
                        1_744_146_000_000_000_i64 + id,
                    ],
                )
                .expect("insert place");
        }
        let visits = [
            (12_i64, 8_i64, Some(11_i64), Option::<i64>::None),
            (13_i64, 9_i64, Option::<i64>::None, Some(2_i64)),
        ];
        for (visit_id, place_id, from_visit, visit_type) in visits {
            connection
                .execute(
                    "INSERT INTO moz_historyvisits (id, place_id, visit_date, from_visit, visit_type)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        visit_id,
                        place_id,
                        1_744_146_000_000_000_i64 + visit_id,
                        from_visit,
                        visit_type,
                    ],
                )
                .expect("insert visit");
        }
        drop(connection);

        let mut consumer = RecordingConsumer::default();
        let streamed =
            stream_history(&history_path, 0, 0, 2, &mut consumer).expect("stream firefox");

        assert_eq!(consumer.url_batches, vec![2, 1]);
        assert_eq!(consumer.visit_batches, vec![2, 1]);
        assert_eq!(streamed.typed_evidence.navigation.len(), 3);
        assert_eq!(streamed.typed_evidence.context.len(), 3);
        let capability = |key: &str| {
            streamed.capability_snapshot.items.iter().find(|item| item.key == key).unwrap()
        };
        assert_eq!(capability("nav.from_visit").populated_rows, 1);
        assert_eq!(capability("nav.from_visit").total_rows, 3);
        assert_eq!(capability("nav.transition").populated_rows, 3);
        assert_eq!(capability("nav.transition").total_rows, 3);
    }

    #[test]
    fn stream_history_can_move_optional_native_tables_out_of_the_returned_report() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("places.sqlite");
        write_history_fixture(&history_path);
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute_batch(
                "CREATE TABLE moz_inputhistory (id INTEGER PRIMARY KEY, place_id INTEGER, input TEXT);
                 INSERT INTO moz_inputhistory (id, place_id, input) VALUES (1, 7, 'firefox');",
            )
            .expect("optional firefox table");
        drop(connection);

        let mut consumer =
            RecordingConsumer { spool_source_evidence: true, ..RecordingConsumer::default() };
        let streamed =
            stream_history(&history_path, 0, 0, 10, &mut consumer).expect("stream firefox");

        assert!(streamed.native_entities.is_empty());
        assert!(consumer.source_evidence_chunks.iter().any(|chunk| {
            chunk
                .native_entities
                .iter()
                .any(|entity| entity.entity_kind == "firefox-moz-inputhistory")
        }));
    }

    #[test]
    fn inspect_history_reports_missing_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("places.sqlite");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute("CREATE TABLE moz_places (id INTEGER PRIMARY KEY)", [])
            .expect("create places table");

        let inspection = inspect_history(&history_path).expect("inspect firefox history");
        assert!(inspection.warnings.iter().any(|warning| {
            warning.message.contains("required Firefox table `moz_historyvisits` is missing")
        }));
    }

    #[test]
    fn parse_history_requires_firefox_required_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("places.sqlite");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute(
                "CREATE TABLE moz_places (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER,
                   hidden INTEGER,
                   last_visit_date INTEGER
                 )",
                [],
            )
            .expect("create places table");

        let error = parse_history(&history_path, 0, 0).expect_err("missing visits should fail");
        assert!(matches!(error, ParseError::MissingTable { table: "moz_historyvisits" }));
    }

    #[test]
    fn firefox_time_helpers_keep_dates_stable() {
        assert_eq!(firefox_time_to_unix_ms(1_000), 1);
        assert_eq!(unix_ms_to_firefox_time(1), 1_000);
        assert_eq!(firefox_time_to_iso(0), "1970-01-01T00:00:00+00:00");
        assert!(firefox_time_to_iso(1_744_146_000_000_000_i64).starts_with("2025-04-"));
    }

    #[test]
    fn parse_history_respects_visit_and_url_cursors() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("places.sqlite");
        write_history_fixture(&history_path);

        let parsed = parse_history(&history_path, 11, 1_744_146_000_000).expect("cursor parse");

        assert_eq!(parsed.urls.len(), 1);
        assert!(parsed.visits.is_empty());
    }
}
