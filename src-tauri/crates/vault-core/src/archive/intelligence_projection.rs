//! Intelligence projection storage for rebuildable AI and insight state.
//!
//! Canonical archive facts stay in `archive/history-vault.sqlite`. Optional AI,
//! enrichment, and deterministic insight tables live in a separate SQLite
//! plane, with read-only temp views back into the canonical archive for the
//! legacy query helpers that still expect `visit_events` / `search_terms`-like
//! shapes.

use crate::{
    ai::ensure_ai_schema,
    ai_queue,
    config::{ProjectPaths, ensure_paths},
    insights::ensure_insight_schema,
    intelligence_runtime::ensure_intelligence_runtime_schema,
    models::{AppConfig, ArchiveMode},
};
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::time::Duration as StdDuration;

const INTELLIGENCE_ARCHIVE_VIEWS_SQL: &str = r#"
DROP VIEW IF EXISTS temp.visit_events;
CREATE TEMP VIEW visit_events AS
SELECT
  visits.id AS id,
  source_profiles.profile_key AS profile_id,
  CAST(visits.source_visit_id AS INTEGER) AS source_visit_id,
  urls.id AS source_url_id,
  urls.url AS url,
  urls.title AS title,
  (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time,
  visits.from_visit AS from_visit,
  visits.transition_type AS transition,
  visits.visit_duration_ms AS visit_duration,
  visits.is_known_to_sync AS is_known_to_sync,
  visits.visited_link_id AS visited_link_id,
  visits.external_referrer_url AS external_referrer_url,
  visits.app_id AS app_id,
  visits.event_fingerprint AS event_fingerprint,
  visits.payload_hash AS payload_hash,
  visits.recorded_at AS recorded_at,
  visits.import_batch_id AS import_batch_id
FROM archive.visits AS visits
JOIN archive.urls AS urls
  ON urls.id = visits.url_id
JOIN archive.source_profiles AS source_profiles
  ON source_profiles.id = visits.source_profile_id
WHERE visits.reverted_at IS NULL;

DROP VIEW IF EXISTS temp.search_terms;
CREATE TEMP VIEW search_terms AS
SELECT
  source_profiles.profile_key AS profile_id,
  search_terms.id AS id,
  search_terms.url_id AS url_id,
  search_terms.term AS term,
  search_terms.normalized_term AS normalized_term,
  search_terms.reverted_at AS reverted_at
FROM archive.search_terms AS search_terms
JOIN archive.source_profiles AS source_profiles
  ON source_profiles.id = search_terms.source_profile_id;

DROP VIEW IF EXISTS temp.source_profiles;
CREATE TEMP VIEW source_profiles AS
SELECT
  id,
  browser_kind,
  browser_version,
  profile_name,
  profile_path,
  discovered_at,
  enabled,
  profile_key,
  user_name,
  updated_at
FROM archive.source_profiles;
"#;

/// Opens the rebuildable intelligence SQLite plane and attaches the canonical
/// archive for read-only temp views.
pub fn open_intelligence_connection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Connection> {
    ensure_paths(paths)?;
    let connection = Connection::open(&paths.intelligence_database_path)
        .with_context(|| format!("opening {}", paths.intelligence_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    attach_archive_database(&connection, paths, config, key)?;
    install_archive_views(&connection)?;
    ensure_ai_schema(&connection)?;
    ai_queue::ensure_ai_queue_schema(&connection)?;
    ensure_insight_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;
    Ok(connection)
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

fn install_archive_views(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(INTELLIGENCE_ARCHIVE_VIEWS_SQL)
        .context("installing intelligence temp views over canonical archive")?;
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
    fn intelligence_connection_bootstraps_side_db_and_archive_views() {
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
            .query_row("SELECT COUNT(*) FROM visit_events", [], |row| row.get(0))
            .expect("temp visit view");
        let term_count: i64 = intelligence
            .query_row("SELECT COUNT(*) FROM search_terms", [], |row| row.get(0))
            .expect("temp search-term view");
        assert_eq!(visible_visits, 1);
        assert_eq!(term_count, 1);
    }
}
