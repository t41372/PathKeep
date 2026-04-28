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
        NavigationEvidence, ParsedUrl, ParsedVisit, ParserWarning, StreamHistoryError,
        StreamedHistory, TypedEvidenceBatch,
    },
};
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags, Row, params};
use std::convert::Infallible;
use std::path::Path;

const INSPECT_TABLES_SQL: &str = "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name";
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
    let mut typed_evidence = TypedEvidenceBatch::default();
    let mut native_entities = Vec::new();

    {
        let mut statement = stream_sql(connection.prepare(URLS_SQL))?;
        let column_names =
            statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
        let mut rows =
            stream_sql(statement.query(params![unix_ms_to_firefox_time(after_url_last_visit_ms)]))?;
        let mut batch = Vec::with_capacity(chunk_size);
        while let Some(row) = stream_sql(rows.next())? {
            batch.push(stream_sql(parsed_url_from_row(row))?);
            native_entities.push(stream_sql(capture_native_row(
                row,
                &column_names,
                "firefox-place-row",
                "id",
                None,
            ))?);
            if batch.len() >= chunk_size {
                consumer.urls(std::mem::take(&mut batch)).map_err(StreamHistoryError::Consumer)?;
            }
        }
        if !batch.is_empty() {
            consumer.urls(batch).map_err(StreamHistoryError::Consumer)?;
        }
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
                typed_evidence.navigation.push(evidence)
            }
            typed_evidence.context.push(context_evidence_for_visit(&visit));
            native_entities.push(stream_sql(capture_native_row(
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
            }
        }
        if !batch.is_empty() {
            consumer.visits(batch).map_err(StreamHistoryError::Consumer)?;
        }
    }

    for optional_table in
        ["moz_inputhistory", "moz_places_metadata", "moz_places_metadata_search_queries"]
    {
        if inspection.table_names.iter().any(|existing| existing == optional_table) {
            let sql = format!("SELECT * FROM {optional_table}");
            let entity_kind = format!("firefox-{}", optional_table.replace('_', "-"));
            native_entities.extend(capture_native_rows(
                &connection,
                &sql,
                &[],
                &entity_kind,
                "id",
                None,
            )?);
        }
    }

    let capability_snapshot = build_capability_snapshot(&typed_evidence, visit_count);
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
    Ok(ParsedUrl {
        source_url_id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        visit_count: row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
        typed_count: 0,
        last_visit_ms: firefox_time_to_unix_ms(last_visit_date),
        last_visit_iso: firefox_time_to_iso(last_visit_date),
        hidden: row.get::<_, i64>(4)? != 0,
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
    typed_evidence: &TypedEvidenceBatch,
    visit_count: usize,
) -> crate::types::CapabilitySnapshot {
    capability_snapshot(vec![
        CapabilityCoverage {
            key: "nav.from_visit".to_string(),
            available: typed_evidence
                .navigation
                .iter()
                .any(|evidence| evidence.target_visit_id.is_some()),
            populated_rows: typed_evidence
                .navigation
                .iter()
                .filter(|evidence| evidence.target_visit_id.is_some())
                .count(),
            total_rows: visit_count,
            notes: vec!["Firefox moz_historyvisits.from_visit".to_string()],
        },
        CapabilityCoverage {
            key: "nav.transition".to_string(),
            available: !typed_evidence.navigation.is_empty(),
            populated_rows: typed_evidence.navigation.len(),
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
