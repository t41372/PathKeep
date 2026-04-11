use crate::{
    ParseError, ParsedHistory,
    types::{DatabaseInspection, ParsedUrl, ParsedVisit, ParserWarning},
};
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags, Row, params};
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

pub fn parse_history(
    path: &Path,
    after_visit_id: i64,
    after_url_last_visit_ms: i64,
) -> Result<ParsedHistory, ParseError> {
    let inspection = inspect_history(path)?;
    validate_required_tables(&inspection)?;

    let connection = open_readonly(path)?;
    let urls = parse_urls(&connection, after_url_last_visit_ms)?;
    let visits = parse_visits(&connection, after_visit_id)?;

    Ok(ParsedHistory {
        inspection: inspection.clone(),
        urls,
        visits,
        downloads: Vec::new(),
        search_terms: Vec::new(),
        favicons: Vec::new(),
        warnings: inspection.warnings,
    })
}

pub fn safari_time_to_unix_ms(value: f64) -> i64 {
    (((value + SAFARI_UNIX_EPOCH_OFFSET_SECONDS) * 1_000.0).round() as i64).max(0)
}

pub fn unix_ms_to_safari_time(value: i64) -> f64 {
    (value.max(0) as f64 / 1_000.0) - SAFARI_UNIX_EPOCH_OFFSET_SECONDS
}

pub fn safari_time_to_iso(value: f64) -> String {
    let milliseconds = safari_time_to_unix_ms(value);
    Utc.timestamp_millis_opt(milliseconds)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("unix epoch"))
        .to_rfc3339()
}

fn parse_urls(
    connection: &Connection,
    after_url_last_visit_ms: i64,
) -> Result<Vec<ParsedUrl>, ParseError> {
    let mut statement = connection.prepare(URLS_SQL)?;
    let rows = statement
        .query_map(params![unix_ms_to_safari_time(after_url_last_visit_ms)], parsed_url_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn parse_visits(
    connection: &Connection,
    after_visit_id: i64,
) -> Result<Vec<ParsedVisit>, ParseError> {
    let mut statement = connection.prepare(VISITS_SQL)?;
    let rows = statement.query_map(params![after_visit_id], parsed_visit_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
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
