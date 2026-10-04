//! Per-source visit statistics.
//!
//! ## Responsibilities
//! - Count visible visits and find the first and last visit for each source profile.
//!
//! ## Not responsible for
//! - Browser discovery (file sizes, access state); that lives in the app snapshot.
//!
//! ## Performance notes
//! - `idx_visits_visible_profile_time_id` is partial (`reverted_at IS NULL`) and starts with
//!   `(source_profile_id, visit_time_ms)`, so the per-profile count walks only that profile's
//!   visible visits.
//! - The first/last visit are index seeks on `(source_profile_id, visit_time_ms)`. SQLite may pick
//!   either that partial index or `idx_visits_profile_time`; with the latter it reads visit rows
//!   from the end of the range until one is not reverted, which is one row in practice.
//! - A count over 14.4M visits still walks every index entry. The result is cached against the
//!   archive's file stamp, so it is recomputed only after a backup, import, or revert writes.

use crate::{
    chrome::browser_display_name,
    config::ProjectPaths,
    models::{AppConfig, SourceStats},
    utils::{SqliteFileStamp, sqlite_file_stamp},
};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use rusqlite::Connection;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

type CacheEntry = (SqliteFileStamp, Vec<SourceStats>);

/// Loads visit counts and date spans for every source profile in the archive.
pub fn load_source_stats(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Vec<SourceStats>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let cache_key = paths.archive_database_path.display().to_string();
    let stamp = sqlite_file_stamp(&paths.archive_database_path);
    if let Some((cached_stamp, stats)) =
        cache.lock().expect("source stats cache lock").get(&cache_key)
        && *cached_stamp == stamp
        && stamp.0.is_some()
    {
        return Ok(stats.clone());
    }
    let connection = super::open_archive_connection(paths, config, key)?;
    let stats = source_stats_for_connection(&connection)?;
    cache.lock().expect("source stats cache lock").insert(cache_key, (stamp, stats.clone()));
    Ok(stats)
}

pub(crate) fn source_stats_for_connection(connection: &Connection) -> Result<Vec<SourceStats>> {
    let mut statement = connection.prepare(
        "SELECT id,
                COALESCE(NULLIF(profile_key, ''), browser_kind || ':' || profile_name),
                browser_kind,
                profile_name
         FROM source_profiles
         ORDER BY browser_kind, profile_name",
    )?;
    let profiles = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut count = connection.prepare(
        "SELECT COUNT(*) FROM visits WHERE source_profile_id = ?1 AND reverted_at IS NULL",
    )?;
    let mut first = connection.prepare(
        "SELECT MIN(visit_time_ms) FROM visits WHERE source_profile_id = ?1 AND reverted_at IS NULL",
    )?;
    let mut last = connection.prepare(
        "SELECT MAX(visit_time_ms) FROM visits WHERE source_profile_id = ?1 AND reverted_at IS NULL",
    )?;

    let mut stats = Vec::with_capacity(profiles.len());
    for (id, profile_id, browser_kind, profile_name) in profiles {
        let visit_count: i64 = count.query_row([id], |row| row.get(0))?;
        let first_ms: Option<i64> = first.query_row([id], |row| row.get(0))?;
        let last_ms: Option<i64> = last.query_row([id], |row| row.get(0))?;
        stats.push(SourceStats {
            profile_id,
            browser_name: browser_display_name(&browser_kind),
            profile_name,
            visit_count,
            first_visit_at: first_ms.and_then(rfc3339_from_ms),
            last_visit_at: last_ms.and_then(rfc3339_from_ms),
        });
    }
    Ok(stats)
}

pub(crate) fn rfc3339_from_ms(ms: i64) -> Option<String> {
    Utc.timestamp_millis_opt(ms).single().map(|value| value.to_rfc3339())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{config::project_paths_with_root, models::ArchiveMode};

    pub(in crate::archive) fn seeded_archive() -> (tempfile::TempDir, ProjectPaths, AppConfig) {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let connection =
            super::super::open_archive_connection(&paths, &config, None).expect("open");
        connection
            .execute_batch(
                "INSERT INTO runs (id, run_type, trigger, started_at, status, due_only)
                   VALUES (1, 'backup', 'manual', '2026-01-01T00:00:00Z', 'success', 0);
                 INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path, discovered_at, enabled, profile_key)
                   VALUES (1, 'chrome', 'Default', '/p/1', '2026-01-01T00:00:00Z', 1, 'chrome:Default'),
                          (2, 'firefox', 'dev', '/p/2', '2026-01-01T00:00:00Z', 1, 'firefox:dev'),
                          (3, 'edge', 'Empty', '/p/3', '2026-01-01T00:00:00Z', 1, 'edge:Empty');
                 INSERT INTO urls (id, url, title, visit_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id)
                   VALUES (1, 'https://example.com/a', 'A', 3, 0, '', 0, '', 1, 1, 1),
                          (2, 'https://example.com/a', 'A (ff)', 1, 0, '', 0, '', 2, 1, 1);
                 INSERT INTO visits (url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id, reverted_at)
                   VALUES (1, 1000, '', 1, 1, NULL),
                          (1, 5000, '', 1, 1, NULL),
                          (1, 9000, '', 1, 1, '2026-01-02T00:00:00Z'),
                          (2, 2000, '', 2, 1, NULL);",
            )
            .expect("seed");
        (root, paths, config)
    }

    #[test]
    fn counts_visible_visits_per_profile_and_keeps_empty_profiles() {
        let (_root, paths, config) = seeded_archive();
        let stats = load_source_stats(&paths, &config, None).expect("stats");
        assert_eq!(stats.len(), 3);
        let chrome = stats.iter().find(|row| row.profile_id == "chrome:Default").expect("chrome");
        assert_eq!(chrome.browser_name, "Google Chrome");
        assert_eq!(chrome.visit_count, 2, "reverted visits are not counted");
        assert_eq!(chrome.first_visit_at.as_deref(), Some("1970-01-01T00:00:01+00:00"));
        assert_eq!(chrome.last_visit_at.as_deref(), Some("1970-01-01T00:00:05+00:00"));
        let empty = stats.iter().find(|row| row.profile_id == "edge:Empty").expect("edge");
        assert_eq!((empty.visit_count, empty.first_visit_at.clone()), (0, None));
        assert_eq!(empty.browser_name, "Microsoft Edge");
    }

    #[test]
    fn stats_use_the_partial_profile_time_index() {
        let (_root, paths, config) = seeded_archive();
        let connection =
            super::super::open_archive_connection(&paths, &config, None).expect("open");
        let plan = |sql: &str| -> String {
            connection
                .query_row(&format!("EXPLAIN QUERY PLAN {sql}"), [], |row| row.get(3))
                .expect("plan")
        };
        let count =
            plan("SELECT COUNT(*) FROM visits WHERE source_profile_id = 1 AND reverted_at IS NULL");
        assert!(
            count.contains("idx_visits_visible_profile_time_id"),
            "the count must use the partial visible-visit index, got: {count}"
        );
        for bound in ["MIN", "MAX"] {
            let seek = plan(&format!(
                "SELECT {bound}(visit_time_ms) FROM visits
                 WHERE source_profile_id = 1 AND reverted_at IS NULL"
            ));
            assert!(
                seek.contains("USING")
                    && seek.contains("INDEX")
                    && seek.contains("source_profile_id=?"),
                "{bound} must seek a (source_profile_id, visit_time_ms) index, got: {seek}"
            );
        }
    }
}
