//! Chromium history parser.
//!
//! This module reads already-staged Chromium `History` and `Favicons`
//! databases. It deliberately avoids browser discovery and live-file copying;
//! those concerns belong to higher layers.

use crate::{
    error::ParseError,
    observation::{capability_snapshot, capture_native_rows, inspect_schema},
    types::{
        CapabilityCoverage, ChromiumHistory, ChromiumReadCursor, ContextEvidence,
        DatabaseInspection, EngagementEvidence, HistoryDatabaseSet, NavigationEvidence,
        ParsedDownload, ParsedFavicon, ParsedSearchTerm, ParsedUrl, ParsedVisit, ParserWarning,
        SearchEvidence, TypedEvidenceBatch,
    },
};
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags, Row, params};
use std::path::Path;

const CHROME_UNIX_EPOCH_OFFSET_MICROS: i64 = 11_644_473_600_000_000;

/// Incremental URL ingest query used by the archive pipeline.
pub const INGEST_URLS_SQL: &str =
    "SELECT id, url, title, visit_count, typed_count, last_visit_time, hidden
     FROM urls
     WHERE last_visit_time >= ?1
     ORDER BY last_visit_time ASC";
/// Incremental visit ingest query used by the archive pipeline.
pub const INGEST_VISITS_SQL: &str =
    "SELECT visits.id, visits.url, urls.url, urls.title, visits.visit_time, visits.from_visit,
            visits.transition, visits.visit_duration, visits.is_known_to_sync,
            visits.visited_link_id, visits.external_referrer_url, visits.app_id
     FROM visits
     JOIN urls ON urls.id = visits.url
     WHERE visits.id > ?1
     ORDER BY visits.id ASC";
/// Incremental download ingest query used by the archive pipeline.
pub const DOWNLOADS_SQL: &str =
    "SELECT id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state,
            mime_type, original_mime_type
     FROM downloads
     WHERE id > ?1
     ORDER BY id ASC";
/// Incremental keyword-search-term query used by the archive pipeline.
pub const SEARCH_TERMS_SQL: &str = "SELECT keyword_id, url_id, term, normalized_term
     FROM keyword_search_terms
     WHERE url_id IN (
       SELECT id FROM urls WHERE last_visit_time >= ?1
     )";
/// Incremental favicons query used by the archive pipeline.
pub const FAVICONS_SQL: &str = "SELECT icon_mapping.page_url, favicons.url, favicons.icon_type,
            IFNULL(favicon_bitmaps.width, 0), IFNULL(favicon_bitmaps.height, 0),
            IFNULL(favicon_bitmaps.last_updated, 0), favicon_bitmaps.image_data
     FROM icon_mapping
     JOIN favicons ON favicons.id = icon_mapping.icon_id
     LEFT JOIN favicon_bitmaps ON favicon_bitmaps.icon_id = favicons.id
     WHERE IFNULL(favicon_bitmaps.last_updated, 0) >= ?1
     ORDER BY IFNULL(favicon_bitmaps.last_updated, 0) ASC";

/// Inspects a staged Chromium source and reports table coverage/warnings.
pub fn inspect_history(source: &HistoryDatabaseSet) -> Result<DatabaseInspection, ParseError> {
    let connection = open_readonly(&source.history_path)?;
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut warnings = Vec::new();
    for table_name in ["urls", "visits"] {
        if !table_names.iter().any(|existing| existing == table_name) {
            warnings.push(ParserWarning {
                code: "missing-table".to_string(),
                message: format!("required Chromium table `{table_name}` is missing"),
            });
        }
    }

    Ok(DatabaseInspection { table_names, warnings })
}

/// Parses a staged Chromium source into deterministic parser read models.
pub fn parse_history(
    source: &HistoryDatabaseSet,
    cursor: ChromiumReadCursor,
) -> Result<ChromiumHistory, ParseError> {
    let inspection = inspect_history(source)?;
    let schema_observation =
        inspect_schema(&open_readonly(&source.history_path)?, &["urls", "visits"])?;
    validate_required_tables(&inspection)?;

    let history = open_readonly(&source.history_path)?;
    let urls = parse_urls(&history, cursor.after_url_last_visit_time)?;
    let visits = parse_visits(&history, cursor.after_visit_id)?;
    let mut warnings = inspection.warnings.clone();

    let downloads = if has_table(&inspection, "downloads") {
        parse_downloads(&history, cursor.after_download_id)?
    } else {
        warnings.push(ParserWarning {
            code: "missing-table".to_string(),
            message: "optional Chromium table `downloads` is missing".to_string(),
        });
        Vec::new()
    };

    let search_terms = if has_table(&inspection, "keyword_search_terms") {
        parse_search_terms(&history, cursor.after_url_last_visit_time)?
    } else {
        warnings.push(ParserWarning {
            code: "missing-table".to_string(),
            message: "optional Chromium table `keyword_search_terms` is missing".to_string(),
        });
        Vec::new()
    };

    let favicons = match &source.favicons_path {
        Some(path) => parse_favicons(path, cursor.after_favicon_last_updated)?,
        None => {
            warnings.push(ParserWarning {
                code: "missing-source".to_string(),
                message: "favicons database was not provided".to_string(),
            });
            Vec::new()
        }
    };

    let typed_evidence = build_typed_evidence(&search_terms, &visits);
    let capability_snapshot = build_capability_snapshot(&inspection, &typed_evidence, &visits);
    let mut native_entities = capture_native_rows(
        &history,
        "SELECT * FROM urls WHERE last_visit_time >= ?1 ORDER BY last_visit_time ASC",
        &[&cursor.after_url_last_visit_time],
        "chromium-url-row",
        "id",
        None,
    )?;
    native_entities.extend(capture_native_rows(
        &history,
        "SELECT * FROM visits WHERE id > ?1 ORDER BY id ASC",
        &[&cursor.after_visit_id],
        "chromium-visit-row",
        "id",
        Some("url"),
    )?);
    if has_table(&inspection, "downloads") {
        native_entities.extend(capture_native_rows(
            &history,
            "SELECT * FROM downloads WHERE id > ?1 ORDER BY id ASC",
            &[&cursor.after_download_id],
            "chromium-download-row",
            "id",
            None,
        )?);
    }
    if has_table(&inspection, "keyword_search_terms") {
        native_entities.extend(capture_native_rows(
            &history,
            "SELECT * FROM keyword_search_terms WHERE url_id IN (SELECT id FROM urls WHERE last_visit_time >= ?1)",
            &[&cursor.after_url_last_visit_time],
            "chromium-search-term-row",
            "url_id",
            None,
        )?);
    }

    Ok(ChromiumHistory {
        inspection,
        schema_observation,
        capability_snapshot,
        urls,
        visits,
        downloads,
        search_terms,
        favicons,
        typed_evidence,
        native_entities,
        warnings,
    })
}

/// Converts Chromium's microsecond timestamp format to Unix milliseconds.
pub fn chrome_time_to_unix_ms(value: i64) -> i64 {
    value.saturating_sub(CHROME_UNIX_EPOCH_OFFSET_MICROS).div_euclid(1_000).max(0)
}

/// Converts Chromium's microsecond timestamp format to RFC3339.
pub fn chrome_time_to_iso(value: i64) -> String {
    let milliseconds = chrome_time_to_unix_ms(value);
    Utc.timestamp_millis_opt(milliseconds)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("unix epoch"))
        .to_rfc3339()
}

fn parse_urls(connection: &Connection, last_visit_time: i64) -> Result<Vec<ParsedUrl>, ParseError> {
    let mut statement = connection.prepare(INGEST_URLS_SQL)?;
    let rows = statement.query_map(params![last_visit_time], parsed_url_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn parse_visits(
    connection: &Connection,
    last_visit_id: i64,
) -> Result<Vec<ParsedVisit>, ParseError> {
    let mut statement = connection.prepare(INGEST_VISITS_SQL)?;
    let rows = statement.query_map(params![last_visit_id], parsed_visit_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn parse_downloads(
    connection: &Connection,
    last_download_id: i64,
) -> Result<Vec<ParsedDownload>, ParseError> {
    let mut statement = connection.prepare(DOWNLOADS_SQL)?;
    let rows = statement.query_map(params![last_download_id], parsed_download_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn parse_search_terms(
    connection: &Connection,
    last_visit_time: i64,
) -> Result<Vec<ParsedSearchTerm>, ParseError> {
    let mut statement = connection.prepare(SEARCH_TERMS_SQL)?;
    let rows = statement.query_map(params![last_visit_time], parsed_search_term_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn parse_favicons(
    favicons_path: &Path,
    last_favicon_last_updated: i64,
) -> Result<Vec<ParsedFavicon>, ParseError> {
    let connection = open_readonly(favicons_path)?;
    let inspection = inspect_connection_tables(&connection)?;
    if !has_table(&inspection, "favicons")
        || !has_table(&inspection, "icon_mapping")
        || !has_table(&inspection, "favicon_bitmaps")
    {
        return Ok(Vec::new());
    }

    let mut statement = connection.prepare(FAVICONS_SQL)?;
    let rows = statement.query_map(params![last_favicon_last_updated], parsed_favicon_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn open_readonly(path: &Path) -> Result<Connection, ParseError> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|source| ParseError::OpenDatabase { path: path.to_path_buf(), source })
}

fn inspect_connection_tables(connection: &Connection) -> Result<DatabaseInspection, ParseError> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(DatabaseInspection { table_names, warnings: Vec::new() })
}

fn validate_required_tables(inspection: &DatabaseInspection) -> Result<(), ParseError> {
    for table_name in ["urls", "visits"] {
        if !has_table(inspection, table_name) {
            return Err(ParseError::MissingTable { table: table_name });
        }
    }
    Ok(())
}

fn has_table(inspection: &DatabaseInspection, table_name: &str) -> bool {
    inspection.table_names.iter().any(|existing| existing == table_name)
}

fn build_typed_evidence(
    search_terms: &[ParsedSearchTerm],
    visits: &[ParsedVisit],
) -> TypedEvidenceBatch {
    let search = search_terms
        .iter()
        .map(|term| SearchEvidence {
            source_visit_id: None,
            source_url_id: Some(term.url_id),
            evidence_key: "search.native_terms".to_string(),
            evidence_value: term.term.clone(),
            normalized_value: Some(term.normalized_term.clone()),
            source_field: "keyword_search_terms.term".to_string(),
        })
        .collect::<Vec<_>>();
    let navigation = visits
        .iter()
        .filter(|visit| {
            visit.from_visit.is_some()
                || visit.external_referrer_url.is_some()
                || visit.transition.is_some()
        })
        .map(|visit| NavigationEvidence {
            source_visit_id: visit.source_visit_id,
            edge_kind: "visit-navigation".to_string(),
            target_visit_id: visit.from_visit,
            target_url: visit.external_referrer_url.clone(),
            transition: visit.transition,
            source_field: "visits.from_visit/external_referrer_url/transition".to_string(),
        })
        .collect::<Vec<_>>();
    let engagement = visits
        .iter()
        .filter_map(|visit| {
            visit.visit_duration_ms.map(|value| EngagementEvidence {
                source_visit_id: visit.source_visit_id,
                metric_key: "engagement.visit_duration_ms".to_string(),
                metric_value_int: Some(value),
                metric_value_real: None,
                source_field: "visits.visit_duration".to_string(),
            })
        })
        .collect::<Vec<_>>();
    let context = visits
        .iter()
        .flat_map(|visit| {
            let mut items = Vec::new();
            if visit.app_id.is_some() {
                items.push(ContextEvidence {
                    source_visit_id: Some(visit.source_visit_id),
                    source_url_id: Some(visit.source_url_id),
                    context_key: "context.app_id".to_string(),
                    value_json: serde_json::json!(visit.app_id).to_string(),
                    source_field: "visits.app_id".to_string(),
                });
            }
            if visit.is_known_to_sync {
                items.push(ContextEvidence {
                    source_visit_id: Some(visit.source_visit_id),
                    source_url_id: Some(visit.source_url_id),
                    context_key: "context.is_known_to_sync".to_string(),
                    value_json: "true".to_string(),
                    source_field: "visits.is_known_to_sync".to_string(),
                });
            }
            if visit.visited_link_id.is_some() {
                items.push(ContextEvidence {
                    source_visit_id: Some(visit.source_visit_id),
                    source_url_id: Some(visit.source_url_id),
                    context_key: "context.visited_link_id".to_string(),
                    value_json: serde_json::json!(visit.visited_link_id).to_string(),
                    source_field: "visits.visited_link_id".to_string(),
                });
            }
            items
        })
        .collect::<Vec<_>>();

    TypedEvidenceBatch { search, navigation, engagement, context }
}

fn build_capability_snapshot(
    inspection: &DatabaseInspection,
    typed_evidence: &TypedEvidenceBatch,
    visits: &[ParsedVisit],
) -> crate::types::CapabilitySnapshot {
    capability_snapshot(vec![
        CapabilityCoverage {
            key: "search.native_terms".to_string(),
            available: has_table(inspection, "keyword_search_terms"),
            populated_rows: typed_evidence.search.len(),
            total_rows: typed_evidence.search.len(),
            notes: vec!["Chromium keyword_search_terms table".to_string()],
        },
        CapabilityCoverage {
            key: "nav.from_visit".to_string(),
            available: visits.iter().any(|visit| visit.from_visit.is_some()),
            populated_rows: visits.iter().filter(|visit| visit.from_visit.is_some()).count(),
            total_rows: visits.len(),
            notes: vec!["Chromium visits.from_visit".to_string()],
        },
        CapabilityCoverage {
            key: "nav.external_referrer".to_string(),
            available: visits.iter().any(|visit| visit.external_referrer_url.is_some()),
            populated_rows: visits
                .iter()
                .filter(|visit| visit.external_referrer_url.is_some())
                .count(),
            total_rows: visits.len(),
            notes: vec!["Chromium visits.external_referrer_url".to_string()],
        },
        CapabilityCoverage {
            key: "engagement.visit_duration_ms".to_string(),
            available: visits.iter().any(|visit| visit.visit_duration_ms.is_some()),
            populated_rows: typed_evidence.engagement.len(),
            total_rows: visits.len(),
            notes: vec!["Chromium visits.visit_duration".to_string()],
        },
        CapabilityCoverage {
            key: "context.sync_state".to_string(),
            available: visits.iter().any(|visit| visit.is_known_to_sync),
            populated_rows: visits.iter().filter(|visit| visit.is_known_to_sync).count(),
            total_rows: visits.len(),
            notes: vec!["Chromium visits.is_known_to_sync".to_string()],
        },
    ])
}

fn parsed_url_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedUrl> {
    let last_visit_time = row.get::<_, i64>(5)?;
    Ok(ParsedUrl {
        source_url_id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        visit_count: row.get(3)?,
        typed_count: row.get(4)?,
        last_visit_ms: chrome_time_to_unix_ms(last_visit_time),
        last_visit_iso: chrome_time_to_iso(last_visit_time),
        hidden: row.get::<_, i64>(6)? != 0,
    })
}

fn parsed_visit_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedVisit> {
    let visit_time = row.get::<_, i64>(4)?;
    Ok(ParsedVisit {
        source_visit_id: row.get(0)?,
        source_url_id: row.get(1)?,
        url: row.get(2)?,
        title: row.get(3)?,
        visit_time_ms: chrome_time_to_unix_ms(visit_time),
        visit_time_iso: chrome_time_to_iso(visit_time),
        from_visit: row.get(5)?,
        transition: row.get(6)?,
        visit_duration_ms: row.get(7)?,
        is_known_to_sync: row.get::<_, Option<i64>>(8)?.unwrap_or_default() != 0,
        visited_link_id: row.get(9)?,
        external_referrer_url: row.get(10)?,
        app_id: row.get(11)?,
    })
}

fn parsed_download_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedDownload> {
    let start_time = row.get::<_, Option<i64>>(4)?;
    Ok(ParsedDownload {
        source_download_id: row.get(0)?,
        guid: row.get(1)?,
        current_path: row.get(2)?,
        target_path: row.get(3)?,
        start_time_ms: start_time.map(chrome_time_to_unix_ms),
        start_time_iso: start_time.map(chrome_time_to_iso),
        received_bytes: row.get(5)?,
        total_bytes: row.get(6)?,
        state: row.get(7)?,
        mime_type: row.get(8)?,
        original_mime_type: row.get(9)?,
    })
}

fn parsed_search_term_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedSearchTerm> {
    Ok(ParsedSearchTerm {
        keyword_id: row.get(0)?,
        url_id: row.get(1)?,
        term: row.get(2)?,
        normalized_term: row.get(3)?,
    })
}

fn parsed_favicon_from_row(row: &Row<'_>) -> rusqlite::Result<ParsedFavicon> {
    let last_updated = row.get::<_, i64>(5)?;
    Ok(ParsedFavicon {
        page_url: row.get(0)?,
        icon_url: row.get(1)?,
        icon_type: row.get(2)?,
        width: row.get(3)?,
        height: row.get(4)?,
        last_updated_ms: chrome_time_to_unix_ms(last_updated),
        last_updated_iso: chrome_time_to_iso(last_updated),
        image_data: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_history_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open history fixture");
        connection
            .execute_batch(
                "CREATE TABLE urls (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER NOT NULL,
                   typed_count INTEGER NOT NULL,
                   last_visit_time INTEGER NOT NULL,
                   hidden INTEGER NOT NULL
                 );
                 CREATE TABLE visits (
                   id INTEGER PRIMARY KEY,
                   url INTEGER NOT NULL,
                   visit_time INTEGER NOT NULL,
                   from_visit INTEGER,
                   transition INTEGER,
                   visit_duration INTEGER,
                   is_known_to_sync INTEGER,
                   visited_link_id INTEGER,
                   external_referrer_url TEXT,
                   app_id TEXT
                 );
                 CREATE TABLE downloads (
                   id INTEGER PRIMARY KEY,
                   guid TEXT,
                   current_path TEXT,
                   target_path TEXT,
                   start_time INTEGER,
                   received_bytes INTEGER,
                   total_bytes INTEGER,
                   state INTEGER,
                   mime_type TEXT,
                   original_mime_type TEXT
                 );
                 CREATE TABLE keyword_search_terms (
                   keyword_id INTEGER,
                   url_id INTEGER,
                   term TEXT,
                   normalized_term TEXT
                 );",
            )
            .expect("create history schema");
        connection
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    1_i64,
                    "https://example.com/article",
                    "Example Article",
                    5_i64,
                    2_i64,
                    13_000_000_000_000_000_i64,
                    0_i64
                ],
            )
            .expect("insert url");
        connection
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    10_i64,
                    1_i64,
                    13_000_000_000_500_000_i64,
                    Option::<i64>::None,
                    805_306_368_i64,
                    4000_i64,
                    1_i64,
                    7_i64,
                    "https://referrer.example.com",
                    "com.example.browser"
                ],
            )
            .expect("insert visit");
        connection
            .execute(
                "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    4_i64,
                    "download-guid",
                    "/tmp/example.part",
                    "/tmp/example.zip",
                    13_000_000_001_000_000_i64,
                    128_i64,
                    256_i64,
                    1_i64,
                    "application/zip",
                    "application/octet-stream"
                ],
            )
            .expect("insert download");
        connection
            .execute(
                "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
                 VALUES (?1, ?2, ?3, ?4)",
                params![2_i64, 1_i64, "PathKeep", "pathkeep"],
            )
            .expect("insert search term");
    }

    fn write_favicons_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open favicons fixture");
        connection
            .execute_batch(
                "CREATE TABLE favicons (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   icon_type INTEGER
                 );
                 CREATE TABLE icon_mapping (
                   page_url TEXT NOT NULL,
                   icon_id INTEGER NOT NULL
                 );
                 CREATE TABLE favicon_bitmaps (
                   icon_id INTEGER NOT NULL,
                   width INTEGER,
                   height INTEGER,
                   last_updated INTEGER,
                   image_data BLOB
                 );",
            )
            .expect("create favicons schema");
        connection
            .execute(
                "INSERT INTO favicons (id, url, icon_type) VALUES (?1, ?2, ?3)",
                params![3_i64, "https://example.com/favicon.ico", 1_i64],
            )
            .expect("insert favicon");
        connection
            .execute(
                "INSERT INTO icon_mapping (page_url, icon_id) VALUES (?1, ?2)",
                params!["https://example.com/article", 3_i64],
            )
            .expect("insert mapping");
        connection
            .execute(
                "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![3_i64, 32_i64, 32_i64, 13_000_000_002_000_000_i64, vec![1_u8, 2, 3]],
            )
            .expect("insert bitmap");
    }

    fn write_partial_favicons_fixture(path: &Path) {
        let connection = Connection::open(path).expect("open partial favicons fixture");
        connection
            .execute_batch(
                "CREATE TABLE favicons (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   icon_type INTEGER
                 );
                 CREATE TABLE favicon_bitmaps (
                   icon_id INTEGER NOT NULL,
                   width INTEGER,
                   height INTEGER,
                   last_updated INTEGER,
                   image_data BLOB
                 );",
            )
            .expect("create partial favicons schema");
        connection
            .execute(
                "INSERT INTO favicons (id, url, icon_type) VALUES (?1, ?2, ?3)",
                params![3_i64, "https://example.com/favicon.ico", 1_i64],
            )
            .expect("insert partial favicon");
        connection
            .execute(
                "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![3_i64, 32_i64, 32_i64, 13_000_000_002_000_000_i64, vec![1_u8, 2, 3]],
            )
            .expect("insert partial bitmap");
    }

    #[test]
    fn parse_history_returns_incremental_rows_from_provided_paths() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let favicons_path = directory.path().join("Favicons");
        write_history_fixture(&history_path);
        write_favicons_fixture(&favicons_path);

        let parsed = parse_history(
            &HistoryDatabaseSet { history_path, favicons_path: Some(favicons_path) },
            ChromiumReadCursor::default(),
        )
        .expect("parse history");

        assert_eq!(parsed.urls.len(), 1);
        assert_eq!(parsed.visits.len(), 1);
        assert_eq!(parsed.downloads.len(), 1);
        assert_eq!(parsed.search_terms.len(), 1);
        assert_eq!(parsed.favicons.len(), 1);
        assert_eq!(parsed.urls[0].url, "https://example.com/article");
        assert!(!parsed.urls[0].hidden);
        assert_eq!(parsed.search_terms[0].normalized_term, "pathkeep");
        assert!(parsed.visits[0].is_known_to_sync);
        assert_eq!(parsed.favicons[0].width, 32);
    }

    #[test]
    fn inspect_history_reports_missing_required_tables_as_warnings() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute("CREATE TABLE downloads (id INTEGER PRIMARY KEY)", [])
            .expect("create downloads");

        let inspection = inspect_history(&HistoryDatabaseSet { history_path, favicons_path: None })
            .expect("inspect history");

        assert!(inspection.warnings.iter().any(|warning| {
            warning.message.contains("required Chromium table `urls` is missing")
        }));
    }

    #[test]
    fn parse_history_requires_urls_and_visits_tables() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let connection = Connection::open(&history_path).expect("open fixture");
        connection
            .execute(
                "CREATE TABLE urls (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER NOT NULL,
                   typed_count INTEGER NOT NULL,
                   last_visit_time INTEGER NOT NULL,
                   hidden INTEGER NOT NULL
                 )",
                [],
            )
            .expect("create urls table");

        let error = parse_history(
            &HistoryDatabaseSet { history_path, favicons_path: None },
            ChromiumReadCursor::default(),
        )
        .expect_err("missing visits should fail");
        assert!(matches!(error, ParseError::MissingTable { table: "visits" }));
    }

    #[test]
    fn parse_history_skips_favicons_when_support_tables_are_missing() {
        let directory = tempdir().expect("tempdir");
        let history_path = directory.path().join("History");
        let favicons_path = directory.path().join("Favicons");
        write_history_fixture(&history_path);
        write_partial_favicons_fixture(&favicons_path);

        let parsed = parse_history(
            &HistoryDatabaseSet { history_path, favicons_path: Some(favicons_path) },
            ChromiumReadCursor::default(),
        )
        .expect("parse history");

        assert!(parsed.favicons.is_empty());
        assert_eq!(parsed.urls.len(), 1);
        assert_eq!(parsed.visits.len(), 1);
    }

    #[test]
    fn chrome_time_helpers_clamp_invalid_values_and_keep_iso_stable() {
        assert_eq!(chrome_time_to_unix_ms(i64::MIN), 0);
        assert_eq!(chrome_time_to_iso(i64::MIN), "1970-01-01T00:00:00+00:00");
        assert!(chrome_time_to_iso(13_000_000_000_000_000_i64).starts_with("2012-"));
    }
}
