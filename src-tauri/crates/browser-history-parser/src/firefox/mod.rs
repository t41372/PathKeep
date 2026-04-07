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
  moz_places.id,
  moz_places.url,
  moz_places.title,
  moz_places.visit_count,
  COALESCE(moz_places.hidden, 0),
  COALESCE(moz_places.last_visit_date, 0)
FROM moz_places
WHERE COALESCE(moz_places.last_visit_date, 0) > ?1
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

pub fn firefox_time_to_unix_ms(value: i64) -> i64 {
    value.div_euclid(1_000).max(0)
}

pub fn unix_ms_to_firefox_time(value: i64) -> i64 {
    value.max(0).saturating_mul(1_000)
}

pub fn firefox_time_to_iso(value: i64) -> String {
    let milliseconds = firefox_time_to_unix_ms(value);
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
    let rows = statement.query_map(
        params![unix_ms_to_firefox_time(after_url_last_visit_ms)],
        parsed_url_from_row,
    )?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::tempdir;

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
        assert_eq!(parsed.visits[0].source_visit_id, 11);
        assert_eq!(parsed.visits[0].app_id.as_deref(), Some("firefox"));
        assert!(parsed.warnings.iter().any(|warning| warning.code == "baseline-support"));
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

        assert!(parsed.urls.is_empty());
        assert!(parsed.visits.is_empty());
    }
}
