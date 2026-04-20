//! Intelligence projection storage for rebuildable AI and insight state.
//!
//! Canonical archive facts stay in `archive/history-vault.sqlite`. Optional AI,
//! enrichment, and deterministic insight tables live in a separate SQLite
//! plane, while read paths use the attached canonical archive directly.

use crate::{
    ai::ensure_ai_schema,
    ai_queue,
    config::{ProjectPaths, ensure_paths},
    intelligence::ensure_core_intelligence_schema,
    intelligence_runtime::ensure_intelligence_runtime_schema,
    models::{AppConfig, ArchiveMode},
};
use anyhow::{Context, Result};
use rusqlite::Connection;
#[cfg(test)]
use std::panic::Location;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(test)]
use std::sync::{Mutex, OnceLock};
#[cfg(test)]
use std::thread::{self, ThreadId};
use std::time::Duration as StdDuration;

const SQLITE_CACHE_SIZE_KIB: i64 = -65_536;
const SQLITE_MMAP_SIZE_BYTES: i64 = 268_435_456;

#[cfg(test)]
static OPEN_INTELLIGENCE_CONNECTION_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static OPEN_INTELLIGENCE_CONNECTION_CALL_SITES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
#[cfg(test)]
static OPEN_INTELLIGENCE_CONNECTION_MONITOR_THREAD: OnceLock<Mutex<Option<ThreadId>>> =
    OnceLock::new();

/// Opens the rebuildable intelligence SQLite plane and attaches the canonical
/// archive for direct read access.
#[track_caller]
pub fn open_intelligence_connection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Connection> {
    #[cfg(test)]
    {
        let current_thread = thread::current().id();
        let should_record = OPEN_INTELLIGENCE_CONNECTION_MONITOR_THREAD
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("open intelligence connection monitor thread lock")
            .as_ref()
            .is_some_and(|thread_id| *thread_id == current_thread);
        if should_record {
            OPEN_INTELLIGENCE_CONNECTION_CALLS.fetch_add(1, Ordering::Relaxed);
            OPEN_INTELLIGENCE_CONNECTION_CALL_SITES
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .expect("open intelligence connection call sites lock")
                .push(Location::caller().to_string());
        }
    }
    ensure_paths(paths)?;
    let connection = Connection::open(&paths.intelligence_database_path)
        .with_context(|| format!("opening {}", paths.intelligence_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "cache_size", SQLITE_CACHE_SIZE_KIB)?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    let _ = connection.pragma_update(None, "mmap_size", SQLITE_MMAP_SIZE_BYTES);
    attach_archive_database(&connection, paths, config, key)?;
    connection.pragma_update(Some("archive"), "cache_size", SQLITE_CACHE_SIZE_KIB)?;
    let _ = connection.pragma_update(Some("archive"), "mmap_size", SQLITE_MMAP_SIZE_BYTES);
    ensure_ai_schema(&connection)?;
    ai_queue::ensure_ai_queue_schema(&connection)?;
    ensure_core_intelligence_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;
    Ok(connection)
}

#[cfg(test)]
pub(crate) fn reset_open_intelligence_connection_call_count() {
    OPEN_INTELLIGENCE_CONNECTION_CALLS.store(0, Ordering::Relaxed);
    *OPEN_INTELLIGENCE_CONNECTION_MONITOR_THREAD
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("open intelligence connection monitor thread lock") = Some(thread::current().id());
    OPEN_INTELLIGENCE_CONNECTION_CALL_SITES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("open intelligence connection call sites lock")
        .clear();
}

#[cfg(test)]
pub(crate) fn open_intelligence_connection_call_count() -> usize {
    OPEN_INTELLIGENCE_CONNECTION_CALLS.load(Ordering::Relaxed)
}

#[cfg(test)]
pub(crate) fn open_intelligence_connection_call_sites() -> Vec<String> {
    OPEN_INTELLIGENCE_CONNECTION_CALL_SITES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .expect("open intelligence connection call sites lock")
        .clone()
}

fn attach_archive_database(
    connection: &Connection,
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<()> {
    let archive_path = paths.archive_database_path.display().to_string().replace('\'', "''");
    let archive_key = match config.archive_mode {
        ArchiveMode::Encrypted => key.context(
            "database key is required for intelligence reads against encrypted archives",
        )?,
        ArchiveMode::Plaintext => "",
    }
    .replace('\'', "''");
    connection
        .execute_batch(&format!("ATTACH DATABASE '{archive_path}' AS archive KEY '{archive_key}';"))
        .context("attaching canonical archive to intelligence storage")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        archive::open_archive_connection,
        config::project_paths_with_root,
        models::{AppConfig, ArchiveMode},
    };

    #[test]
    fn intelligence_connection_bootstraps_side_db_and_attached_archive() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        archive
            .execute(
                "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0)",
                [],
            )
            .expect("run");
        archive
            .execute(
                "INSERT INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
                 VALUES (1, 'chrome', '1', 'Default', '/tmp/profile', '2026-04-14T00:00:00Z', 1, 'chrome:Default', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("profile");
        archive
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
                 VALUES (1, 'https://example.com', 'Example', 1, 0, 1, '1970-01-01T00:00:00Z', 1, '1970-01-01T00:00:00Z', 1, 1, 9, 0, 'hash', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("url");
        archive
            .execute(
                "INSERT INTO visits (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at)
                 VALUES (1, 1, '5', 1, '1970-01-01T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint', 'hash', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("visit");
        archive
            .execute(
                "INSERT INTO search_terms (id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id)
                 VALUES (1, 1, 'Example', 'example', 1, 1, 'chrome:Default')",
                [],
            )
            .expect("search term");

        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("intelligence");
        let visible_visits: i64 = intelligence
            .query_row("SELECT COUNT(*) FROM archive.visits WHERE reverted_at IS NULL", [], |row| {
                row.get(0)
            })
            .expect("attached archive visits");
        let term_count: i64 = intelligence
            .query_row("SELECT COUNT(*) FROM archive.search_terms", [], |row| row.get(0))
            .expect("attached archive search terms");
        assert_eq!(visible_visits, 1);
        assert_eq!(term_count, 1);
    }
}
