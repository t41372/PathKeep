PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS profiles (
  profile_id TEXT PRIMARY KEY,
  profile_name TEXT NOT NULL,
  user_name TEXT,
  profile_path TEXT NOT NULL,
  chrome_version TEXT,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS source_schemas (
  schema_hash TEXT PRIMARY KEY,
  source_kind TEXT NOT NULL,
  chrome_version TEXT,
  payload_json TEXT NOT NULL,
  seen_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS backup_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  status TEXT NOT NULL,
  due_only INTEGER NOT NULL,
  profiles_json TEXT NOT NULL,
  manifest_path TEXT,
  manifest_hash TEXT,
  previous_manifest_hash TEXT,
  summary_json TEXT
);
CREATE TABLE IF NOT EXISTS manifests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL UNIQUE,
  manifest_hash TEXT NOT NULL UNIQUE,
  previous_manifest_hash TEXT,
  path TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS import_batches (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_kind TEXT NOT NULL,
  source_path TEXT NOT NULL,
  profile_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  imported_at TEXT,
  reverted_at TEXT,
  status TEXT NOT NULL,
  summary_json TEXT NOT NULL,
  audit_path TEXT,
  git_commit TEXT
);
CREATE TABLE IF NOT EXISTS raw_row_versions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL,
  profile_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  table_name TEXT NOT NULL,
  source_pk TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  schema_hash TEXT NOT NULL,
  chrome_version TEXT,
  recorded_at TEXT NOT NULL,
  UNIQUE(profile_id, source_kind, table_name, source_pk, payload_hash)
);
CREATE TABLE IF NOT EXISTS url_versions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  profile_id TEXT NOT NULL,
  source_url_id INTEGER NOT NULL,
  url TEXT NOT NULL,
  title TEXT,
  visit_count INTEGER,
  typed_count INTEGER,
  last_visit_time INTEGER NOT NULL,
  hidden INTEGER NOT NULL,
  payload_hash TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  UNIQUE(profile_id, source_url_id, payload_hash)
);
CREATE TABLE IF NOT EXISTS visit_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  profile_id TEXT NOT NULL,
  source_visit_id INTEGER NOT NULL,
  source_url_id INTEGER NOT NULL,
  url TEXT NOT NULL,
  title TEXT,
  visit_time INTEGER NOT NULL,
  from_visit INTEGER,
  transition INTEGER,
  visit_duration INTEGER,
  is_known_to_sync INTEGER,
  visited_link_id INTEGER,
  external_referrer_url TEXT,
  app_id TEXT,
  event_fingerprint TEXT,
  payload_hash TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  UNIQUE(profile_id, source_visit_id)
);
CREATE TABLE IF NOT EXISTS download_versions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  profile_id TEXT NOT NULL,
  source_download_id INTEGER NOT NULL,
  guid TEXT,
  current_path TEXT,
  target_path TEXT,
  start_time INTEGER,
  total_bytes INTEGER,
  received_bytes INTEGER,
  state INTEGER,
  mime_type TEXT,
  original_mime_type TEXT,
  payload_hash TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  UNIQUE(profile_id, source_download_id, payload_hash)
);
CREATE TABLE IF NOT EXISTS search_terms (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  profile_id TEXT NOT NULL,
  keyword_id INTEGER NOT NULL,
  url_id INTEGER NOT NULL,
  term TEXT NOT NULL,
  normalized_term TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  UNIQUE(profile_id, keyword_id, url_id, term, normalized_term)
);
CREATE TABLE IF NOT EXISTS favicons (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  profile_id TEXT NOT NULL,
  page_url TEXT NOT NULL,
  icon_url TEXT NOT NULL,
  icon_type INTEGER,
  width INTEGER,
  height INTEGER,
  last_updated INTEGER,
  image_data BLOB,
  payload_hash TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  UNIQUE(profile_id, page_url, icon_url, payload_hash)
);
CREATE TABLE IF NOT EXISTS profile_watermarks (
  profile_id TEXT PRIMARY KEY,
  last_visit_id INTEGER NOT NULL DEFAULT 0,
  last_url_last_visit_time INTEGER NOT NULL DEFAULT 0,
  last_download_id INTEGER NOT NULL DEFAULT 0,
  last_favicon_last_updated INTEGER NOT NULL DEFAULT 0,
  last_checkpoint_at TEXT,
  last_schema_hash TEXT,
  updated_at TEXT NOT NULL
);
