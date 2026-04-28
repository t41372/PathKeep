//! Parser and watermark helpers for canonical backup ingest.
//!
//! ## Responsibilities
//! - Parse staged browser snapshots using the correct parser for each browser family.
//! - Track per-profile incremental watermarks so repeat backups stay bounded.
//! - Decide when a new raw source checkpoint is required.
//!
//! ## Not responsible for
//! - Writing canonical archive rows or source-evidence rows.
//! - Running archive manifests, run ledgers, or retention workflows.
//! - Picking which profiles participate in a backup run.
//!
//! ## Dependencies
//! - `browser_history_parser` family-specific parsers and cursors.
//! - Watermark storage tables owned by the parent archive schema.
//!
//! ## Performance notes
//! - Parser output is still materialized in memory today, so watermark filtering
//!   is the main guardrail that keeps repeat backups from replaying old rows.

use super::super::*;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OpenFlags};

/// Tracks the last successfully ingested source cursors for one profile.
#[derive(Debug, Default)]
pub(super) struct Watermark {
    pub(super) last_visit_id: i64,
    pub(super) last_url_last_visit_time: i64,
    pub(super) last_download_id: i64,
    pub(super) last_favicon_last_updated: i64,
    pub(super) last_checkpoint_at: Option<String>,
    pub(super) last_schema_hash: Option<String>,
    pub(super) last_source_batch_id: Option<i64>,
    pub(super) updated_at: String,
}

/// Parses a saved source checkpoint without incremental cursors so restore preview can size the replay.
pub(super) fn preview_snapshot_counts(
    snapshot: &ProfileSnapshot,
    _config: &AppConfig,
) -> Result<(usize, usize, usize)> {
    match snapshot.profile.browser_family.as_str() {
        "chromium" => preview_chromium_snapshot_counts(snapshot),
        "firefox" => preview_firefox_snapshot_counts(snapshot),
        "safari" => preview_safari_snapshot_counts(snapshot),
        family => anyhow::bail!("browser family `{family}` is not supported by the archive engine"),
    }
}

/// Loads the last successful incremental cursors for one source profile.
pub(super) fn load_watermark(archive: &Transaction<'_>, profile_id: &str) -> Result<Watermark> {
    archive
        .query_row(
            "SELECT
               last_visit_id,
               last_url_last_visit_time,
               last_download_id,
               last_favicon_last_updated,
               last_checkpoint_at,
               last_schema_hash,
               last_source_batch_id,
               updated_at
             FROM profile_watermarks
             WHERE profile_id = ?1",
            [profile_id],
            |row| {
                Ok(Watermark {
                    last_visit_id: row.get(0)?,
                    last_url_last_visit_time: row.get(1)?,
                    last_download_id: row.get(2)?,
                    last_favicon_last_updated: row.get(3)?,
                    last_checkpoint_at: row.get(4)?,
                    last_schema_hash: row.get(5)?,
                    last_source_batch_id: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .optional()
        .map(|value| {
            value.unwrap_or_else(|| Watermark { updated_at: now_rfc3339(), ..Watermark::default() })
        })
        .map_err(Into::into)
}

/// Saves the next incremental cursors after one staged profile has been processed.
pub(super) fn save_watermark(
    archive: &Transaction<'_>,
    profile_id: &str,
    watermark: &Watermark,
) -> Result<()> {
    archive.execute(
        "INSERT INTO profile_watermarks (
           profile_id,
           last_visit_id,
           last_url_last_visit_time,
           last_download_id,
           last_favicon_last_updated,
           last_checkpoint_at,
           last_schema_hash,
           last_source_batch_id,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(profile_id) DO UPDATE SET
           last_visit_id = excluded.last_visit_id,
           last_url_last_visit_time = excluded.last_url_last_visit_time,
           last_download_id = excluded.last_download_id,
           last_favicon_last_updated = excluded.last_favicon_last_updated,
           last_checkpoint_at = excluded.last_checkpoint_at,
           last_schema_hash = excluded.last_schema_hash,
           last_source_batch_id = excluded.last_source_batch_id,
           updated_at = excluded.updated_at",
        params![
            profile_id,
            watermark.last_visit_id,
            watermark.last_url_last_visit_time,
            watermark.last_download_id,
            watermark.last_favicon_last_updated,
            watermark.last_checkpoint_at,
            watermark.last_schema_hash,
            watermark.last_source_batch_id,
            watermark.updated_at,
        ],
    )?;
    Ok(())
}

/// Decides whether this profile should emit a raw source checkpoint during the current backup.
pub(super) fn should_checkpoint(
    watermark: &Watermark,
    schema_hash: &str,
    checkpoint_days: u64,
) -> bool {
    if watermark.last_schema_hash.as_deref() != Some(schema_hash) {
        return true;
    }
    let Some(last_checkpoint_at) = &watermark.last_checkpoint_at else {
        return true;
    };
    let Ok(last_checkpoint_at) = DateTime::parse_from_rfc3339(last_checkpoint_at) else {
        return true;
    };
    Utc::now() - last_checkpoint_at.with_timezone(&Utc) > Duration::days(checkpoint_days as i64)
}

fn preview_chromium_snapshot_counts(snapshot: &ProfileSnapshot) -> Result<(usize, usize, usize)> {
    let connection = open_snapshot_connection(&snapshot.history_path)?;
    let urls = query_table_count(&connection, "urls", "SELECT COUNT(*) FROM urls")?;
    let visits = query_table_count(&connection, "visits", "SELECT COUNT(*) FROM visits")?;
    let downloads =
        query_optional_table_count(&connection, "downloads", "SELECT COUNT(*) FROM downloads")?;
    Ok((visits, urls, downloads))
}

fn preview_firefox_snapshot_counts(snapshot: &ProfileSnapshot) -> Result<(usize, usize, usize)> {
    let connection = open_snapshot_connection(&snapshot.history_path)?;
    let urls = query_table_count(
        &connection,
        "moz_places",
        "SELECT COUNT(*) FROM moz_places WHERE last_visit_date IS NOT NULL",
    )?;
    let visits = query_table_count(
        &connection,
        "moz_historyvisits",
        "SELECT COUNT(*) FROM moz_historyvisits",
    )?;
    Ok((visits, urls, 0))
}

fn preview_safari_snapshot_counts(snapshot: &ProfileSnapshot) -> Result<(usize, usize, usize)> {
    let connection = open_snapshot_connection(&snapshot.history_path)?;
    let visits =
        query_table_count(&connection, "history_visits", "SELECT COUNT(*) FROM history_visits")?;
    let urls = query_table_count(
        &connection,
        "history_items",
        "SELECT COUNT(DISTINCT history_item) FROM history_visits",
    )?;
    Ok((visits, urls, 0))
}

fn open_snapshot_connection(path: &std::path::Path) -> Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening checkpoint {}", path.display()))
}

fn query_table_count(connection: &Connection, table: &str, sql: &str) -> Result<usize> {
    if !table_exists(connection, table)? {
        anyhow::bail!("checkpoint is missing required table `{table}`");
    }
    query_count(connection, sql)
}

fn query_optional_table_count(connection: &Connection, table: &str, sql: &str) -> Result<usize> {
    if !table_exists(connection, table)? {
        return Ok(0);
    }
    query_count(connection, sql)
}

fn query_count(connection: &Connection, sql: &str) -> Result<usize> {
    connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .map(|count| count as usize)
        .map_err(Into::into)
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM sqlite_master
               WHERE type = 'table' AND name = ?1
             )",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .map(|exists| exists != 0)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn profile(family: &str) -> crate::models::BrowserProfile {
        crate::models::BrowserProfile {
            profile_id: format!("{family}:Default"),
            profile_name: "Default".to_string(),
            browser_family: family.to_string(),
            browser_name: family.to_string(),
            user_name: None,
            profile_path: "/tmp/profile".to_string(),
            history_path: None,
            favicons_path: None,
            history_exists: true,
            history_readable: true,
            access_issue: None,
            browser_version: None,
            history_file_name: "History".to_string(),
            history_bytes: 0,
            favicons_bytes: 0,
            supporting_bytes: 0,
            retention_boundary: crate::models::BrowserRetentionBoundary::default(),
        }
    }

    fn snapshot(
        temp_dir: tempfile::TempDir,
        family: &str,
        history_path: std::path::PathBuf,
    ) -> ProfileSnapshot {
        ProfileSnapshot {
            profile: profile(family),
            temp_dir,
            history_path,
            favicons_path: None,
            source_hashes: vec![],
        }
    }

    #[test]
    fn preview_snapshot_counts_cover_supported_families_optional_tables_and_errors() {
        let chromium_dir = tempdir().expect("chromium tempdir");
        let chromium_path = chromium_dir.path().join("History");
        Connection::open(&chromium_path)
            .expect("chromium db")
            .execute_batch(
                "CREATE TABLE urls (id INTEGER PRIMARY KEY);
                 CREATE TABLE visits (id INTEGER PRIMARY KEY);
                 INSERT INTO urls (id) VALUES (1), (2);
                 INSERT INTO visits (id) VALUES (1), (2), (3);",
            )
            .expect("chromium schema");
        assert_eq!(
            preview_snapshot_counts(
                &snapshot(chromium_dir, "chromium", chromium_path),
                &AppConfig::default(),
            )
            .expect("chromium counts"),
            (3, 2, 0)
        );

        let firefox_dir = tempdir().expect("firefox tempdir");
        let firefox_path = firefox_dir.path().join("places.sqlite");
        Connection::open(&firefox_path)
            .expect("firefox db")
            .execute_batch(
                "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, last_visit_date INTEGER);
                 CREATE TABLE moz_historyvisits (id INTEGER PRIMARY KEY);
                 INSERT INTO moz_places (id, last_visit_date) VALUES (1, 1), (2, NULL);
                 INSERT INTO moz_historyvisits (id) VALUES (1), (2);",
            )
            .expect("firefox schema");
        assert_eq!(
            preview_snapshot_counts(
                &snapshot(firefox_dir, "firefox", firefox_path),
                &AppConfig::default(),
            )
            .expect("firefox counts"),
            (2, 1, 0)
        );

        let safari_dir = tempdir().expect("safari tempdir");
        let safari_path = safari_dir.path().join("History.db");
        Connection::open(&safari_path)
            .expect("safari db")
            .execute_batch(
                "CREATE TABLE history_items (id INTEGER PRIMARY KEY);
                 CREATE TABLE history_visits (id INTEGER PRIMARY KEY, history_item INTEGER);
                 INSERT INTO history_items (id) VALUES (1), (2);
                 INSERT INTO history_visits (id, history_item) VALUES (1, 1), (2, 1), (3, 2);",
            )
            .expect("safari schema");
        assert_eq!(
            preview_snapshot_counts(
                &snapshot(safari_dir, "safari", safari_path),
                &AppConfig::default()
            )
            .expect("safari counts"),
            (3, 2, 0)
        );

        let unsupported_dir = tempdir().expect("unsupported tempdir");
        let unsupported_path = unsupported_dir.path().join("History");
        Connection::open(&unsupported_path).expect("unsupported db");
        let error = preview_snapshot_counts(
            &snapshot(unsupported_dir, "netscape", unsupported_path),
            &AppConfig::default(),
        )
        .expect_err("unsupported family");
        assert!(error.to_string().contains("not supported"));

        let missing_table_dir = tempdir().expect("missing table tempdir");
        let missing_table_path = missing_table_dir.path().join("History");
        Connection::open(&missing_table_path).expect("missing table db");
        let error = preview_snapshot_counts(
            &snapshot(missing_table_dir, "chromium", missing_table_path),
            &AppConfig::default(),
        )
        .expect_err("missing required table");
        assert!(error.to_string().contains("required table `urls`"));
    }

    #[test]
    fn watermark_helpers_round_trip_and_checkpoint_edges() {
        let connection = Connection::open_in_memory().expect("memory db");
        crate::archive::create_schema(&connection).expect("schema");
        let mut connection = connection;
        let transaction = connection.transaction().expect("transaction");

        let missing = load_watermark(&transaction, "chrome:Default").expect("missing watermark");
        assert_eq!(missing.last_visit_id, 0);
        assert!(!missing.updated_at.is_empty());

        let saved = Watermark {
            last_visit_id: 7,
            last_url_last_visit_time: 8,
            last_download_id: 9,
            last_favicon_last_updated: 10,
            last_checkpoint_at: Some("2026-04-01T00:00:00+00:00".to_string()),
            last_schema_hash: Some("schema-a".to_string()),
            last_source_batch_id: Some(11),
            updated_at: "2026-04-02T00:00:00+00:00".to_string(),
        };
        save_watermark(&transaction, "chrome:Default", &saved).expect("save watermark");
        let loaded = load_watermark(&transaction, "chrome:Default").expect("load watermark");
        assert_eq!(loaded.last_visit_id, 7);
        assert_eq!(loaded.last_url_last_visit_time, 8);
        assert_eq!(loaded.last_download_id, 9);
        assert_eq!(loaded.last_favicon_last_updated, 10);
        assert_eq!(loaded.last_checkpoint_at.as_deref(), Some("2026-04-01T00:00:00+00:00"));
        assert_eq!(loaded.last_schema_hash.as_deref(), Some("schema-a"));
        assert_eq!(loaded.last_source_batch_id, Some(11));

        assert!(should_checkpoint(&loaded, "schema-b", 30));
        assert!(should_checkpoint(
            &Watermark { last_schema_hash: Some("schema-a".to_string()), ..Watermark::default() },
            "schema-a",
            30,
        ));
        assert!(should_checkpoint(
            &Watermark {
                last_schema_hash: Some("schema-a".to_string()),
                last_checkpoint_at: Some("not-a-date".to_string()),
                ..Watermark::default()
            },
            "schema-a",
            30,
        ));
        assert!(should_checkpoint(
            &Watermark {
                last_schema_hash: Some("schema-a".to_string()),
                last_checkpoint_at: Some("2020-01-01T00:00:00+00:00".to_string()),
                ..Watermark::default()
            },
            "schema-a",
            30,
        ));
        assert!(!should_checkpoint(
            &Watermark {
                last_schema_hash: Some("schema-a".to_string()),
                last_checkpoint_at: Some(Utc::now().to_rfc3339()),
                ..Watermark::default()
            },
            "schema-a",
            30,
        ));
    }
}
