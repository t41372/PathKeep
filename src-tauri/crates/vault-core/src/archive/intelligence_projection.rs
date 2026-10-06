//! Responsibilities: retain the SQLCipher opener still used by maintenance and independent fixture inputs.
//! Not responsible for: native intelligence, AI indexing, queues or enrichment storage.
//! Failure modes: an excluded legacy caller opens a Turso file or an area-4 read
//! accidentally returns to per-call SQLite setup. Native intelligence attaches Archive
//! through Vault's read pool; its acceptance tests assert this opener is never called.

use crate::{
    config::{ProjectPaths, ensure_paths},
    models::{AppConfig, ArchiveMode},
};
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashSet;
#[cfg(test)]
use std::panic::Location;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
#[cfg(test)]
use std::thread::{self, ThreadId};
use std::time::Duration as StdDuration;

const SQLITE_CACHE_SIZE_KIB: i64 = -65_536;
const SQLITE_MMAP_SIZE_BYTES: i64 = 268_435_456;

/// Cache legacy DDL setup for repeated maintenance and fixture connections.
/// Native intelligence uses Vault and does not consult this cache.
static INTELLIGENCE_SCHEMA_READY: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[cfg(test)]
static OPEN_INTELLIGENCE_CONNECTION_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static OPEN_INTELLIGENCE_CONNECTION_CALL_SITES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
#[cfg(test)]
static OPEN_INTELLIGENCE_CONNECTION_MONITOR_THREAD: OnceLock<Mutex<Option<ThreadId>>> =
    OnceLock::new();

/// Retains access to the legacy projection for wipe and independent fixture inputs.
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
    super::schema::require_legacy_password(paths, config, key)?;
    ensure_paths(paths)?;
    let schema_key = paths.intelligence_database_path.display().to_string();
    // Checked before `open` creates the file, so a deleted database is rebuilt.
    let schema_ready = paths.intelligence_database_path.exists()
        && INTELLIGENCE_SCHEMA_READY
            .get_or_init(Default::default)
            .lock()
            .expect("intelligence schema flag lock")
            .contains(&schema_key);
    let connection = Connection::open(&paths.intelligence_database_path)
        .with_context(|| format!("opening {}", paths.intelligence_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    if !schema_ready {
        connection.pragma_update(None, "journal_mode", "WAL")?;
    }
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "cache_size", SQLITE_CACHE_SIZE_KIB)?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    let _ = connection.pragma_update(None, "mmap_size", SQLITE_MMAP_SIZE_BYTES);
    attach_archive_database(&connection, paths, config, key)?;
    connection.pragma_update(Some("archive"), "cache_size", SQLITE_CACHE_SIZE_KIB)?;
    let _ = connection.pragma_update(Some("archive"), "mmap_size", SQLITE_MMAP_SIZE_BYTES);
    if !schema_ready {
        connection.execute_batch(LEGACY_AI_SCHEMA_SQL)?;
        INTELLIGENCE_SCHEMA_READY
            .get_or_init(Default::default)
            .lock()
            .expect("intelligence schema flag lock")
            .insert(schema_key);
    }
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

const LEGACY_AI_SCHEMA_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS ai_embeddings (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      history_id INTEGER NOT NULL,
      profile_id TEXT NOT NULL,
      url TEXT NOT NULL,
      title TEXT,
      domain TEXT NOT NULL,
      visited_at TEXT NOT NULL,
      content_hash TEXT NOT NULL,
      content_bytes INTEGER NOT NULL,
      provider_id TEXT NOT NULL,
      model TEXT NOT NULL,
      indexed_at TEXT NOT NULL,
      UNIQUE(history_id, provider_id, model, content_hash)
    );
    CREATE TABLE IF NOT EXISTS ai_assistant_runs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      run_id INTEGER,
      question TEXT NOT NULL,
      answer TEXT NOT NULL,
      provider_id TEXT NOT NULL,
      embedding_provider_id TEXT NOT NULL,
      citations_json TEXT NOT NULL,
      notes_json TEXT NOT NULL,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS ai_index_ledger (
      provider_id TEXT NOT NULL,
      model TEXT NOT NULL,
      sidecar_table TEXT NOT NULL,
      index_version TEXT NOT NULL,
      state TEXT NOT NULL,
      source_watermark INTEGER,
      last_run_id INTEGER,
      build_started_at TEXT,
      build_finished_at TEXT,
      last_indexed_at TEXT,
      last_cleared_at TEXT,
      last_failure_at TEXT,
      failure_reason TEXT,
      PRIMARY KEY(provider_id, model)
    );
    CREATE INDEX IF NOT EXISTS idx_ai_embeddings_provider_model
      ON ai_embeddings(provider_id, model);
    CREATE INDEX IF NOT EXISTS idx_ai_embeddings_history_id
      ON ai_embeddings(history_id);
"#;

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

    #[test]
    fn intelligence_connection_call_monitor_tracks_current_thread_sites() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        reset_open_intelligence_connection_call_count();

        let _ = open_intelligence_connection(&paths, &config, None).expect("intelligence");

        assert_eq!(open_intelligence_connection_call_count(), 1);
        let call_sites = open_intelligence_connection_call_sites();
        assert_eq!(call_sites.len(), 1);
        assert!(
            call_sites[0].contains("intelligence_projection.rs"),
            "unexpected call sites: {call_sites:?}"
        );
    }

    #[test]
    fn intelligence_connection_requires_key_for_encrypted_archive_attach() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Encrypted,
            ..AppConfig::default()
        };

        let error = open_intelligence_connection(&paths, &config, None)
            .expect_err("encrypted intelligence attach requires a key");
        assert!(format!("{error:#}").contains("database key is required"));
    }
}
