-- Canonical archive schema v1
--
-- Gap table from the legacy browser-history-backup schema:
-- profiles           -> source_profiles
-- backup_runs        -> runs
-- url_versions       -> urls
-- visit_events       -> visits
-- source_schemas     -> raw_row_versions.schema_fingerprint
-- profile_watermarks -> kept in legacy runtime bridge until M1 archive engine lands
-- AI / insight tables remain derived-state concerns outside canonical v1

CREATE TABLE schema_migrations (
  version     INTEGER PRIMARY KEY,
  applied_at  TEXT    NOT NULL,
  checksum    TEXT    NOT NULL,
  backup_path TEXT
);

CREATE TABLE runs (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  run_type           TEXT    NOT NULL,
  trigger            TEXT    NOT NULL,
  started_at         TEXT    NOT NULL,
  finished_at        TEXT,
  timezone           TEXT,
  status             TEXT    NOT NULL,
  profile_scope_json TEXT,
  stats_json         TEXT,
  warnings_json      TEXT,
  error_message      TEXT
);

CREATE TABLE source_profiles (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  browser_kind    TEXT    NOT NULL,
  browser_version TEXT,
  profile_name    TEXT    NOT NULL,
  profile_path    TEXT    NOT NULL,
  discovered_at   TEXT    NOT NULL,
  enabled         INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE urls (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  url               TEXT    NOT NULL,
  title             TEXT,
  visit_count       INTEGER NOT NULL DEFAULT 0,
  typed_count       INTEGER NOT NULL DEFAULT 0,
  first_visit_ms    INTEGER NOT NULL,
  first_visit_iso   TEXT    NOT NULL,
  last_visit_ms     INTEGER NOT NULL,
  last_visit_iso    TEXT    NOT NULL,
  source_profile_id INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id INTEGER NOT NULL REFERENCES runs(id)
);

CREATE TABLE visits (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  url_id             INTEGER NOT NULL REFERENCES urls(id),
  source_visit_id    TEXT,
  visit_time_ms      INTEGER NOT NULL,
  visit_time_iso     TEXT    NOT NULL,
  transition_type    INTEGER,
  visit_duration_ms  INTEGER,
  source_profile_id  INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id  INTEGER NOT NULL REFERENCES runs(id),
  reverted_at        TEXT,
  reverted_by_run_id INTEGER REFERENCES runs(id)
);

CREATE TABLE downloads (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  source_download_id TEXT,
  guid               TEXT,
  current_path       TEXT,
  target_path        TEXT,
  start_time_ms      INTEGER,
  start_time_iso     TEXT,
  total_bytes        INTEGER,
  received_bytes     INTEGER,
  state              INTEGER,
  mime_type          TEXT,
  original_mime_type TEXT,
  source_profile_id  INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id  INTEGER NOT NULL REFERENCES runs(id),
  reverted_at        TEXT,
  reverted_by_run_id INTEGER REFERENCES runs(id)
);

CREATE TABLE search_terms (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  url_id             INTEGER NOT NULL REFERENCES urls(id),
  term               TEXT    NOT NULL,
  normalized_term    TEXT    NOT NULL,
  source_profile_id  INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id  INTEGER NOT NULL REFERENCES runs(id),
  reverted_at        TEXT,
  reverted_by_run_id INTEGER REFERENCES runs(id)
);

CREATE TABLE favicons (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  page_url          TEXT    NOT NULL,
  icon_url          TEXT    NOT NULL,
  icon_type         INTEGER,
  width             INTEGER,
  height            INTEGER,
  last_updated_ms   INTEGER,
  last_updated_iso  TEXT,
  image_data        BLOB,
  source_profile_id INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id INTEGER NOT NULL REFERENCES runs(id)
);

CREATE TABLE raw_row_versions (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  source_profile_id  INTEGER NOT NULL REFERENCES source_profiles(id),
  source_kind        TEXT    NOT NULL,
  table_name         TEXT    NOT NULL,
  source_pk          TEXT    NOT NULL,
  payload_hash       TEXT    NOT NULL,
  schema_fingerprint TEXT    NOT NULL,
  browser_version    TEXT,
  payload_json       TEXT    NOT NULL,
  recorded_at        TEXT    NOT NULL,
  run_id             INTEGER NOT NULL REFERENCES runs(id),
  UNIQUE(source_kind, source_profile_id, table_name, source_pk, payload_hash)
);

CREATE TABLE manifests (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id             INTEGER NOT NULL REFERENCES runs(id),
  parent_manifest_id INTEGER REFERENCES manifests(id),
  content_hash       TEXT    NOT NULL,
  row_counts_json    TEXT,
  created_at         TEXT    NOT NULL
);

CREATE TABLE snapshots (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id     INTEGER NOT NULL REFERENCES runs(id),
  file_path  TEXT    NOT NULL,
  file_size  INTEGER,
  checksum   TEXT,
  reason     TEXT,
  created_at TEXT    NOT NULL
);

CREATE TABLE settings (
  key        TEXT PRIMARY KEY,
  value      TEXT,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_runs_started_at ON runs(started_at DESC);
CREATE INDEX idx_urls_profile_last_visit ON urls(source_profile_id, last_visit_ms DESC);
CREATE INDEX idx_visits_profile_time ON visits(source_profile_id, visit_time_ms DESC);
CREATE INDEX idx_downloads_profile_start ON downloads(source_profile_id, start_time_ms DESC);
CREATE INDEX idx_search_terms_profile_term ON search_terms(source_profile_id, normalized_term);
CREATE INDEX idx_raw_row_versions_profile_table ON raw_row_versions(source_profile_id, table_name);
