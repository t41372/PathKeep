ALTER TABLE runs ADD COLUMN due_only INTEGER NOT NULL DEFAULT 0;

ALTER TABLE source_profiles ADD COLUMN profile_key TEXT;
ALTER TABLE source_profiles ADD COLUMN user_name TEXT;
ALTER TABLE source_profiles ADD COLUMN updated_at TEXT;
UPDATE source_profiles
SET profile_key = COALESCE(NULLIF(profile_key, ''), browser_kind || ':' || profile_name),
    updated_at = COALESCE(updated_at, discovered_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_source_profiles_profile_key
  ON source_profiles(profile_key);

ALTER TABLE urls ADD COLUMN source_url_id INTEGER;
ALTER TABLE urls ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;
ALTER TABLE urls ADD COLUMN payload_hash TEXT;
ALTER TABLE urls ADD COLUMN recorded_at TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_urls_profile_source_url_id
  ON urls(source_profile_id, source_url_id);

ALTER TABLE visits ADD COLUMN from_visit INTEGER;
ALTER TABLE visits ADD COLUMN is_known_to_sync INTEGER NOT NULL DEFAULT 0;
ALTER TABLE visits ADD COLUMN visited_link_id INTEGER;
ALTER TABLE visits ADD COLUMN external_referrer_url TEXT;
ALTER TABLE visits ADD COLUMN app_id TEXT;
ALTER TABLE visits ADD COLUMN event_fingerprint TEXT;
ALTER TABLE visits ADD COLUMN payload_hash TEXT;
ALTER TABLE visits ADD COLUMN recorded_at TEXT;
ALTER TABLE visits ADD COLUMN import_batch_id INTEGER;
CREATE UNIQUE INDEX IF NOT EXISTS idx_visits_profile_source_visit_id
  ON visits(source_profile_id, source_visit_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_visits_profile_event_fingerprint
  ON visits(source_profile_id, event_fingerprint)
  WHERE event_fingerprint IS NOT NULL AND event_fingerprint != '';
CREATE INDEX IF NOT EXISTS idx_visits_import_batch_id
  ON visits(import_batch_id);

ALTER TABLE downloads ADD COLUMN payload_hash TEXT;
ALTER TABLE downloads ADD COLUMN recorded_at TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_downloads_profile_source_download_id
  ON downloads(source_profile_id, source_download_id);

ALTER TABLE search_terms ADD COLUMN profile_id TEXT;
ALTER TABLE search_terms ADD COLUMN keyword_id INTEGER;
ALTER TABLE search_terms ADD COLUMN recorded_at TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_search_terms_profile_url_term
  ON search_terms(source_profile_id, url_id, normalized_term);

ALTER TABLE favicons ADD COLUMN payload_hash TEXT;
ALTER TABLE favicons ADD COLUMN recorded_at TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_favicons_profile_page_icon_payload
  ON favicons(source_profile_id, page_url, icon_url, payload_hash)
  WHERE payload_hash IS NOT NULL;

ALTER TABLE manifests ADD COLUMN file_path TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_manifests_content_hash
  ON manifests(content_hash);

CREATE TABLE IF NOT EXISTS profile_watermarks (
  profile_id                 TEXT PRIMARY KEY,
  last_visit_id              INTEGER NOT NULL DEFAULT 0,
  last_url_last_visit_time   INTEGER NOT NULL DEFAULT 0,
  last_download_id           INTEGER NOT NULL DEFAULT 0,
  last_favicon_last_updated  INTEGER NOT NULL DEFAULT 0,
  last_checkpoint_at         TEXT,
  last_schema_hash           TEXT,
  updated_at                 TEXT NOT NULL
);

INSERT OR IGNORE INTO runs (
  id,
  run_type,
  trigger,
  started_at,
  finished_at,
  timezone,
  status,
  profile_scope_json,
  stats_json,
  warnings_json,
  error_message,
  due_only
)
VALUES (
  0,
  'system',
  'compat',
  '1970-01-01T00:00:00+00:00',
  '1970-01-01T00:00:00+00:00',
  'UTC',
  'success',
  '[]',
  '{}',
  '[]',
  NULL,
  0
);
