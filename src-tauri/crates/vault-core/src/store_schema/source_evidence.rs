//! Responsibilities: define cold browser-native evidence and batch provenance.
//! Not responsible for: canonical visits, parsing, or cross-database integrity.
//! Failure modes: lost optional payload fields or batch lookup indexes; evidence
//! cannot round-trip or is tied by an invalid FK to a different database.
//! Tests: catalog_and_crud (every table and column, plaintext and encrypted).
//! Drops: none. Archive profile/run IDs remain logical cross-database references;
//! existing evidence tables deliberately have no SQL foreign keys.

use vault_store::Migration;

pub(super) fn migration() -> Migration {
    super::v1(STATEMENTS)
}

const STATEMENTS: &[&str] = &[
    r"CREATE TABLE source_batches (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_profile_id INTEGER NOT NULL,
  run_id INTEGER,
  source_kind TEXT NOT NULL,
  browser_version TEXT,
  schema_version_text TEXT,
  schema_version_int INTEGER,
  schema_fingerprint TEXT NOT NULL,
  parser_version TEXT NOT NULL,
  capability_snapshot_json TEXT NOT NULL,
  coverage_stats_json TEXT NOT NULL,
  artifact_refs_json TEXT,
  notes_json TEXT,
  created_at TEXT NOT NULL
)",
    r"CREATE TABLE schema_observations (
  source_batch_id INTEGER NOT NULL,
  database_label TEXT NOT NULL,
  observation_json TEXT NOT NULL,
  recorded_at TEXT NOT NULL
)",
    r"CREATE TABLE visit_search_evidence (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id TEXT,
  source_url_id TEXT,
  evidence_key TEXT NOT NULL,
  evidence_value TEXT NOT NULL,
  normalized_value TEXT,
  source_field TEXT NOT NULL,
  recorded_at TEXT NOT NULL
)",
    r"CREATE TABLE visit_navigation_evidence (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id TEXT NOT NULL,
  edge_kind TEXT NOT NULL,
  target_visit_id TEXT,
  target_url TEXT,
  transition INTEGER,
  source_field TEXT NOT NULL,
  recorded_at TEXT NOT NULL
)",
    r"CREATE TABLE visit_engagement_evidence (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id TEXT NOT NULL,
  metric_key TEXT NOT NULL,
  metric_value_int INTEGER,
  metric_value_real REAL,
  source_field TEXT NOT NULL,
  recorded_at TEXT NOT NULL
)",
    r"CREATE TABLE visit_context_evidence (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  source_visit_id TEXT,
  source_url_id TEXT,
  context_key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  source_field TEXT NOT NULL,
  recorded_at TEXT NOT NULL
)",
    r"CREATE TABLE native_entities (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_batch_id INTEGER NOT NULL,
  source_profile_id INTEGER NOT NULL,
  entity_kind TEXT NOT NULL,
  native_primary_key TEXT NOT NULL,
  parent_native_primary_key TEXT,
  payload_json TEXT NOT NULL,
  metadata_json TEXT NOT NULL,
  recorded_at TEXT NOT NULL
)",
    "CREATE INDEX idx_source_batches_profile_created ON source_batches(source_profile_id, created_at DESC)",
    "CREATE INDEX idx_source_batches_run ON source_batches(run_id)",
    "CREATE INDEX idx_visit_search_evidence_batch ON visit_search_evidence(source_batch_id, source_profile_id)",
    "CREATE INDEX idx_visit_navigation_evidence_batch ON visit_navigation_evidence(source_batch_id, source_profile_id)",
    "CREATE INDEX idx_visit_engagement_profile_visit ON visit_engagement_evidence(source_profile_id,source_visit_id)",
    "CREATE INDEX idx_visit_context_profile_visit ON visit_context_evidence(source_profile_id,source_visit_id)",
    "CREATE INDEX idx_visit_engagement_evidence_batch ON visit_engagement_evidence(source_batch_id, source_profile_id)",
    "CREATE INDEX idx_visit_context_evidence_batch ON visit_context_evidence(source_batch_id, source_profile_id)",
    "CREATE INDEX idx_native_entities_batch_kind ON native_entities(source_batch_id, entity_kind)",
];
