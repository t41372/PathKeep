//! Failure modes: wrong legacy keys, mode drift, or omitted source-evidence tables. Tests: retained intelligence/maintenance fixtures.
//! Responsibilities: open the source-evidence file for unported read/maintenance code.
//! Not responsible for: Turso writes, backup or import. Removed with area 5.
use crate::{
    archive::apply_cipher_key,
    config::{ProjectPaths, ensure_paths},
    models::{AppConfig, ArchiveMode},
};
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::time::Duration as StdDuration;
const SOURCE_EVIDENCE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS source_batches (
  id                       INTEGER PRIMARY KEY AUTOINCREMENT,
  source_profile_id        INTEGER NOT NULL,
  run_id                   INTEGER,
  source_kind              TEXT NOT NULL,
  browser_version          TEXT,
  schema_version_text      TEXT,
  schema_version_int       INTEGER,
  schema_fingerprint       TEXT NOT NULL,
  parser_version           TEXT NOT NULL,
  capability_snapshot_json TEXT NOT NULL,
  coverage_stats_json      TEXT NOT NULL,
  artifact_refs_json       TEXT,
  notes_json               TEXT,
  created_at               TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_observations (
  source_batch_id  INTEGER NOT NULL,
  database_label   TEXT NOT NULL,
  observation_json TEXT NOT NULL,
  recorded_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS visit_search_evidence (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id   INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id   TEXT,
  source_url_id     TEXT,
  evidence_key      TEXT NOT NULL,
  evidence_value    TEXT NOT NULL,
  normalized_value  TEXT,
  source_field      TEXT NOT NULL,
  recorded_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS visit_navigation_evidence (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id   INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id   TEXT NOT NULL,
  edge_kind         TEXT NOT NULL,
  target_visit_id   TEXT,
  target_url        TEXT,
  transition        INTEGER,
  source_field      TEXT NOT NULL,
  recorded_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS visit_engagement_evidence (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id    INTEGER NOT NULL,
  source_profile_id  INTEGER NOT NULL,
  source_visit_id    TEXT NOT NULL,
  metric_key         TEXT NOT NULL,
  metric_value_int   INTEGER,
  metric_value_real  REAL,
  source_field       TEXT NOT NULL,
  recorded_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS visit_context_evidence (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id   INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id   TEXT,
  source_url_id     TEXT,
  context_key       TEXT NOT NULL,
  value_json        TEXT NOT NULL,
  source_field      TEXT NOT NULL,
  recorded_at       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS native_entities (
  id                         INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id            INTEGER NOT NULL,
  source_profile_id          INTEGER NOT NULL,
  entity_kind                TEXT NOT NULL,
  native_primary_key         TEXT NOT NULL,
  parent_native_primary_key  TEXT,
  payload_json               TEXT NOT NULL,
  metadata_json              TEXT NOT NULL,
  recorded_at                TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_source_batches_profile_created
  ON source_batches(source_profile_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_source_batches_run
  ON source_batches(run_id);
CREATE INDEX IF NOT EXISTS idx_visit_search_evidence_batch
  ON visit_search_evidence(source_batch_id, source_profile_id);
CREATE INDEX IF NOT EXISTS idx_visit_navigation_evidence_batch
  ON visit_navigation_evidence(source_batch_id, source_profile_id);
CREATE INDEX IF NOT EXISTS idx_visit_engagement_evidence_batch
  ON visit_engagement_evidence(source_batch_id, source_profile_id);
CREATE INDEX IF NOT EXISTS idx_visit_context_evidence_batch
  ON visit_context_evidence(source_batch_id, source_profile_id);
CREATE INDEX IF NOT EXISTS idx_native_entities_batch_kind
  ON native_entities(source_batch_id, entity_kind);
"#;

pub fn open_legacy_source_evidence_connection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Connection> {
    super::schema::require_legacy_password(paths, config, key)?;
    ensure_paths(paths)?;
    let connection = Connection::open(&paths.source_evidence_database_path)
        .with_context(|| format!("opening {}", paths.source_evidence_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        let key = key.context("database key is required for encrypted archives")?;
        apply_cipher_key(&connection, key)?;
    }
    connection.execute_batch(SOURCE_EVIDENCE_SCHEMA_SQL)?;
    Ok(connection)
}
