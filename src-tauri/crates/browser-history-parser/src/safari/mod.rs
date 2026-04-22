//! Safari history parser.
//!
//! This slice reads already-staged `History.db` files and extracts visits/URLs.
//! It does not attempt broader Safari artifact coverage; the goal is a
//! trustworthy baseline parser, not speculative inference.

use crate::{
    ParseError, ParsedHistory,
    observation::{capability_snapshot, capture_native_row, capture_native_rows, inspect_schema},
    types::{
        CapabilityCoverage, DatabaseInspection, HistoryBatchConsumer, ParsedUrl, ParsedVisit,
        ParserWarning, StreamHistoryError, StreamedHistory, TypedEvidenceBatch,
    },
};
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags, Row, params};
use std::convert::Infallible;
use std::path::Path;

const INSPECT_TABLES_SQL: &str = "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name";
const URLS_SQL: &str = r#"
SELECT
  history_items.id,
  history_items.url,
  (
    SELECT history_visits.title
    FROM history_visits
    WHERE history_visits.history_item = history_items.id
      AND history_visits.title IS NOT NULL
    ORDER BY history_visits.visit_time DESC, history_visits.id DESC
    LIMIT 1
  ) AS title,
  (
    SELECT COUNT(*)
    FROM history_visits
    WHERE history_visits.history_item = history_items.id
  ) AS visit_count,
  (
    SELECT MAX(history_visits.visit_time)
    FROM history_visits
    WHERE history_visits.history_item = history_items.id
  ) AS last_visit_time
FROM history_items
WHERE EXISTS (
  SELECT 1
  FROM history_visits
  WHERE history_visits.history_item = history_items.id
)
  AND (
    SELECT MAX(history_visits.visit_time)
    FROM history_visits
    WHERE history_visits.history_item = history_items.id
  ) >= ?1
ORDER BY last_visit_time ASC
"#;
const VISITS_SQL: &str = r#"
SELECT
  history_visits.id,
  history_visits.history_item,
  history_items.url,
  history_visits.title,
  history_visits.visit_time
FROM history_visits
JOIN history_items
  ON history_items.id = history_visits.history_item
WHERE history_visits.id > ?1
ORDER BY history_visits.id ASC
"#;
const SAFARI_UNIX_EPOCH_OFFSET_SECONDS: f64 = 978_307_200.0;

#[derive(Debug, Default)]
struct SafariHistoryCollector {
    urls: Vec<ParsedUrl>,
    visits: Vec<ParsedVisit>,
}

impl HistoryBatchConsumer for SafariHistoryCollector {
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

/// Inspects a Safari `History.db` file and reports required-table coverage.
pub fn inspect_history(path: &Path) -> Result<DatabaseInspection, ParseError> {
    let connection = open_readonly(path)?;
    let mut statement = connection.prepare(INSPECT_TABLES_SQL)?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut warnings = Vec::new();
    for table_name in ["history_items", "history_visits"] {
        if !table_names.iter().any(|existing| existing == table_name) {
            warnings.push(ParserWarning {
                code: "missing-table".to_string(),
                message: format!("required Safari table `{table_name}` is missing"),
            });
        }
    }

    warnings.push(ParserWarning {
        code: "baseline-support".to_string(),
        message:
            "Safari baseline ingest captures history visits only. Full Disk Access may still be required before the desktop app can stage History.db."
                .to_string(),
    });

    Ok(DatabaseInspection { table_names, warnings })
}

/// Parses a Safari `History.db` file into parser read models.
pub fn parse_history(
    path: &Path,
    after_visit_id: i64,
    after_url_last_visit_ms: i64,
) -> Result<ParsedHistory, ParseError> {
    let mut collector = SafariHistoryCollector::default();
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

/// Streams Safari URL and visit rows into a caller-provided batch consumer.
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
        inspect_schema(&open_readonly(path)?, &["history_items", "history_visits"])?;
    validate_required_tables(&inspection)?;

    let connection = open_readonly(path)?;
    let chunk_size = chunk_size.max(1);
    let warnings = inspection.warnings.clone();
    let mut visit_count = 0usize;
    let typed_evidence = TypedEvidenceBatch::default();
    let mut native_entities = Vec::new();

    {
        let mut statement = stream_sql(connection.prepare(URLS_SQL))?;
        let column_names =
            statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
        let mut rows =
            stream_sql(statement.query(params![unix_ms_to_safari_time(after_url_last_visit_ms)]))?;
        let mut batch = Vec::with_capacity(chunk_size);
        while let Some(row) = stream_sql(rows.next())? {
            batch.push(stream_sql(parsed_url_from_row(row))?);
            native_entities.push(stream_sql(capture_native_row(
                row,
                &column_names,
                "safari-history-item-row",
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
            native_entities.push(stream_sql(capture_native_row(
                row,
                &column_names,
                "safari-history-visit-row",
                "id",
                Some("history_item"),
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

    for optional_table in ["history_tombstones", "history_tags", "history_items_to_tags"] {
        if inspection.table_names.iter().any(|existing| existing == optional_table) {
            let sql = format!("SELECT * FROM {optional_table}");
            let entity_kind = format!("safari-{}", optional_table.replace('_', "-"));
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

    let capability_snapshot = capability_snapshot(vec![CapabilityCoverage {
        key: "canonical.history_visits".to_string(),
        available: visit_count > 0,
        populated_rows: visit_count,
        total_rows: visit_count,
        notes: vec!["Safari History.db baseline".to_string()],
    }]);
    Ok(StreamedHistory {
        inspection,
        schema_observation,
        capability_snapshot,
        typed_evidence,
        native_entities,
        warnings,
    })
}

/// Converts Safari's Cocoa timestamp format to Unix milliseconds.
pub fn safari_time_to_unix_ms(value: f64) -> i64 {
    (((value + SAFARI_UNIX_EPOCH_OFFSET_SECONDS) * 1_000.0).round() as i64).max(0)
}

/// Converts Unix milliseconds back into Safari's Cocoa timestamp format.
pub fn unix_ms_to_safari_time(value: i64) -> f64 {
    (value.max(0) as f64 / 1_000.0) - SAFARI_UNIX_EPOCH_OFFSET_SECONDS
}

/// Converts Safari's Cocoa timestamp format to RFC3339.
pub fn safari_time_to_iso(value: f64) -> String {
    let milliseconds = safari_time_to_unix_ms(value);
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
    for table_name in ["history_items", "history_visits"] {
        if !inspection.table_names.iter().any(|existing| existing == table_name) {
            return Err(ParseError::MissingTable { table: table_name });
        }
    }
    Ok(())
}

fn parsed_url_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedUrl> {
    let last_visit_time = row.get::<_, Option<f64>>(4)?.unwrap_or_default();
    Ok(ParsedUrl {
        source_url_id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        visit_count: row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
        typed_count: 0,
        last_visit_ms: safari_time_to_unix_ms(last_visit_time),
        last_visit_iso: safari_time_to_iso(last_visit_time),
        hidden: false,
    })
}

fn parsed_visit_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedVisit> {
    let visit_time = row.get::<_, f64>(4)?;
    Ok(ParsedVisit {
        source_visit_id: row.get(0)?,
        source_url_id: row.get(1)?,
        url: row.get(2)?,
        title: row.get(3)?,
        visit_time_ms: safari_time_to_unix_ms(visit_time),
        visit_time_iso: safari_time_to_iso(visit_time),
        from_visit: None,
        transition: None,
        visit_duration_ms: None,
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: Some("safari".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::tempdir;

    fn write_history_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open safari fixture");
        connection
            .execute_batch(
                "CREATE TABLE history_items (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL
                 );
                 CREATE TABLE history_visits (
                   id INTEGER PRIMARY KEY,
                   history_item INTEGER NOT NULL,
                   title TEXT,
                   visit_time REAL NOT NULL
                 );",
            )
            .expect("create safari schema");
        connection
            .execute(
                "INSERT INTO history_items (id, url) VALUES (?1, ?2)",
                params![5_i64, "https://example.com/safari"],
            )
            .expect("insert history item");
        connection
            .execute(
                "INSERT INTO history_visits (id, history_item, title, visit_time)
                 VALUES (?1, ?2, ?3, ?4)",
                params![9_i64, 5_i64, "Safari Example", 765_838_800.0_f64],
            )
            .expect("insert safari visit");
    }

    #[test]
    fn parse_history_reads_safari_items_and_visits() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        write_history_fixture(&history_path);

        let parsed = parse_history(&history_path, 0, 0).expect("parse safari history");
        assert_eq!(parsed.urls.len(), 1);
        assert_eq!(parsed.visits.len(), 1);
        assert_eq!(parsed.urls[0].title.as_deref(), Some("Safari Example"));
        assert_eq!(parsed.visits[0].source_visit_id, 9);
        assert_eq!(parsed.visits[0].app_id.as_deref(), Some("safari"));
        assert!(parsed.warnings.iter().any(|warning| warning.code == "baseline-support"));
    }

    #[test]
    fn inspect_history_reports_missing_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute("CREATE TABLE history_items (id INTEGER PRIMARY KEY)", [])
            .expect("create history items table");

        let inspection = inspect_history(&history_path).expect("inspect safari history");
        assert!(inspection.warnings.iter().any(|warning| {
            warning.message.contains("required Safari table `history_visits` is missing")
        }));
    }

    #[test]
    fn parse_history_requires_safari_required_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute(
                "CREATE TABLE history_items (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL
                 )",
                [],
            )
            .expect("create history items table");

        let error = parse_history(&history_path, 0, 0).expect_err("missing visits should fail");
        assert!(matches!(error, ParseError::MissingTable { table: "history_visits" }));
    }

    #[test]
    fn safari_time_helpers_keep_dates_stable() {
        assert_eq!(safari_time_to_unix_ms(0.0), 978_307_200_000);
        assert_eq!(unix_ms_to_safari_time(978_307_200_000), 0.0);
        assert_eq!(
            safari_time_to_unix_ms(unix_ms_to_safari_time(1_744_146_000_000)),
            1_744_146_000_000
        );
        assert_eq!(
            safari_time_to_iso(-SAFARI_UNIX_EPOCH_OFFSET_SECONDS),
            "1970-01-01T00:00:00+00:00"
        );
        assert!(safari_time_to_iso(765_838_800.0).starts_with("2025-04-"));
    }

    #[test]
    fn parse_history_respects_visit_and_url_cursors() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History.db");
        write_history_fixture(&history_path);

        let parsed = parse_history(&history_path, 9, 1_744_146_000_000).expect("cursor parse");

        assert_eq!(parsed.urls.len(), 1);
        assert!(parsed.visits.is_empty());
    }
}
