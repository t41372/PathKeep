//! Responsibilities: define canonical history, page identity and search handoff.
//! Not responsible for: ingest, URL normalization, Tantivy indexing or encryption.
//! Failure modes: omitted columns/indexes, orphan facts, duplicate pages, profile
//! observations merged, reverted visits retained in search filters, split document
//! and outbox commits, reused outbox sequence numbers, lost deletion tombstones.
//! Tests: catalog_and_crud, foreign_keys, pages_and_visibility, search_outbox,
//! archive_v1_upgrades_read_indexes_and_preserves_every_fact.
//!
//! Pages hold the exact URL string and its registrable domain, which do not vary
//! by browser profile. Titles, counts, source IDs, observed timestamps, hidden
//! flags and provenance stay on urls: they describe one browser's observation.
//! urls.url remains the observed URL used by production import/export. Notes and
//! tags keep their exact-URL keys; stars retain canonical-URL/domain keys because
//! tracking variants and domain stars are broader than one exact page.
//!
//! Search has ten weighted text fields plus compact_text for gram generation.
//! search_document_profiles records visible_visit_count and newest_visit_ms per
//! page/profile. Unlike a bitmask it has no profile limit and supports removing
//! membership when the last visible visit is reverted. Writers recompute affected
//! pairs from visible visits and the document maximum in the same transaction.
//! Date/profile eligibility still comes from the same canonical visit, not from
//! these summaries. A document with no visible visits has a NULL newest time.
//! Outbox sequences never reuse committed IDs after pruning. Tombstones have no
//! page FK so they survive page deletion until the index acknowledges them.
//!
//! Drops: settings (key/value/updated_at) has no production consumer; encryption
//! keys belong in vault-crypto's envelope. schema_migrations and all its columns
//! belong to the old migration runner. urls.registrable_domain moves to pages,
//! with idx_urls_registrable_domain replaced by idx_pages_registrable_domain.
//! search_documents.url_id becomes page_id because search is across profiles.
//! search_projection_meta (key/value) only tracks the discarded FTS bootstrap;
//! history_search, history_search_terms, history_search_trigram and their FTS
//! shadow tables/indexes are replaced by the independent Tantivy index.

use vault_store::Migration;

/// Native v1 archives must acquire these indexes once, before their read pools open.
pub(super) fn read_indexes() -> Migration {
    Migration::rust(2, |s| {
        s.execute("DROP INDEX idx_urls_last_visit_ms", ())?;
        for sql in READ_INDEXES {
            s.execute(sql, ())?;
        }
        Ok(())
    })
}

/// Existing facts initialize source counters once; later batches maintain deltas.
pub(super) fn profile_totals() -> Migration {
    Migration::rust(super::ARCHIVE_SCHEMA_VERSION, |s| {
        s.execute("INSERT INTO archive_totals(name,value) SELECT 'sourceVisits:'||p.id,
            (SELECT COUNT(*) FROM visits INDEXED BY idx_visits_visible_profile_time_id WHERE source_profile_id=p.id AND reverted_at IS NULL)
            FROM source_profiles p",())?;
        Ok(())
    })
}

const READ_INDEXES: &[&str] = &[
    "CREATE INDEX idx_urls_last_visit_ms ON urls(last_visit_ms DESC,id DESC)",
    "CREATE INDEX IF NOT EXISTS idx_visits_visible_batch_time_id ON visits(import_batch_id,visit_time_ms DESC,id DESC) WHERE reverted_at IS NULL AND import_batch_id IS NOT NULL",
    "CREATE INDEX IF NOT EXISTS idx_search_outbox_page_seq ON search_outbox(page_id,seq)",
    "CREATE INDEX IF NOT EXISTS idx_manifests_run_id ON manifests(run_id,id DESC)",
    "CREATE INDEX IF NOT EXISTS idx_snapshots_run_id ON snapshots(run_id,id)",
];

pub(super) fn migration() -> Migration {
    super::v1(STATEMENTS)
}

const STATEMENTS: &[&str] = &[
    r"CREATE TABLE runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_type TEXT NOT NULL,
  trigger TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  timezone TEXT,
  status TEXT NOT NULL,
  profile_scope_json TEXT,
  stats_json TEXT,
  warnings_json TEXT,
  error_message TEXT,
  due_only INTEGER NOT NULL DEFAULT 0
)",
    r"CREATE TABLE source_profiles (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  browser_kind TEXT NOT NULL,
  browser_version TEXT,
  profile_name TEXT NOT NULL,
  profile_path TEXT NOT NULL,
  discovered_at TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  profile_key TEXT,
  user_name TEXT,
  updated_at TEXT,
  browser_family TEXT,
  browser_product TEXT
)",
    r"CREATE TABLE pages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL UNIQUE,
  canonical_url TEXT NOT NULL,
  registrable_domain TEXT
)",
    r"CREATE TABLE urls (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL,
  title TEXT,
  visit_count INTEGER NOT NULL DEFAULT 0,
  typed_count INTEGER NOT NULL DEFAULT 0,
  first_visit_ms INTEGER NOT NULL,
  first_visit_iso TEXT NOT NULL,
  last_visit_ms INTEGER NOT NULL,
  last_visit_iso TEXT NOT NULL,
  source_profile_id INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id INTEGER NOT NULL REFERENCES runs(id),
  source_url_id INTEGER,
  hidden INTEGER NOT NULL DEFAULT 0,
  payload_hash TEXT,
  recorded_at TEXT,
  page_id INTEGER NOT NULL REFERENCES pages(id)
)",
    r"CREATE TABLE visits (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url_id INTEGER NOT NULL REFERENCES urls(id),
  source_visit_id TEXT,
  visit_time_ms INTEGER NOT NULL,
  visit_time_iso TEXT NOT NULL,
  transition_type INTEGER,
  visit_duration_ms INTEGER,
  source_profile_id INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id INTEGER NOT NULL REFERENCES runs(id),
  reverted_at TEXT,
  reverted_by_run_id INTEGER REFERENCES runs(id),
  from_visit INTEGER,
  is_known_to_sync INTEGER NOT NULL DEFAULT 0,
  visited_link_id INTEGER,
  external_referrer_url TEXT,
  app_id TEXT,
  event_fingerprint TEXT,
  payload_hash TEXT,
  recorded_at TEXT,
  import_batch_id INTEGER
)",
    r"CREATE TABLE downloads (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_download_id TEXT,
  guid TEXT,
  current_path TEXT,
  target_path TEXT,
  start_time_ms INTEGER,
  start_time_iso TEXT,
  total_bytes INTEGER,
  received_bytes INTEGER,
  state INTEGER,
  mime_type TEXT,
  original_mime_type TEXT,
  source_profile_id INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id INTEGER NOT NULL REFERENCES runs(id),
  reverted_at TEXT,
  reverted_by_run_id INTEGER REFERENCES runs(id),
  payload_hash TEXT,
  recorded_at TEXT
)",
    r"CREATE TABLE search_terms (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url_id INTEGER NOT NULL REFERENCES urls(id),
  term TEXT NOT NULL,
  normalized_term TEXT NOT NULL,
  source_profile_id INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id INTEGER NOT NULL REFERENCES runs(id),
  reverted_at TEXT,
  reverted_by_run_id INTEGER REFERENCES runs(id),
  profile_id TEXT,
  keyword_id INTEGER,
  recorded_at TEXT
)",
    r"CREATE TABLE favicons (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  page_url TEXT NOT NULL,
  icon_url TEXT NOT NULL,
  icon_type INTEGER,
  width INTEGER,
  height INTEGER,
  last_updated_ms INTEGER,
  last_updated_iso TEXT,
  image_data BLOB,
  source_profile_id INTEGER NOT NULL REFERENCES source_profiles(id),
  created_by_run_id INTEGER NOT NULL REFERENCES runs(id),
  payload_hash TEXT,
  recorded_at TEXT,
  image_blob_hash TEXT REFERENCES favicon_blobs(blob_hash),
  page_host TEXT,
  page_registrable_domain TEXT
)",
    r"CREATE TABLE manifests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  parent_manifest_id INTEGER REFERENCES manifests(id),
  content_hash TEXT NOT NULL,
  row_counts_json TEXT,
  created_at TEXT NOT NULL,
  file_path TEXT
)",
    r"CREATE TABLE snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  file_path TEXT NOT NULL,
  file_size INTEGER,
  checksum TEXT,
  reason TEXT,
  created_at TEXT NOT NULL
)",
    r"CREATE TABLE profile_watermarks (
  profile_id TEXT PRIMARY KEY,
  last_visit_id INTEGER NOT NULL DEFAULT 0,
  last_url_last_visit_time INTEGER NOT NULL DEFAULT 0,
  last_download_id INTEGER NOT NULL DEFAULT 0,
  last_favicon_last_updated INTEGER NOT NULL DEFAULT 0,
  last_checkpoint_at TEXT,
  last_schema_hash TEXT,
  updated_at TEXT NOT NULL,
  last_source_batch_id INTEGER,
  last_search_url_time INTEGER NOT NULL DEFAULT 0,
  url_scan_json TEXT
)",
    r"CREATE TABLE favicon_blobs (
  blob_hash TEXT PRIMARY KEY,
  image_data BLOB NOT NULL,
  recorded_at TEXT NOT NULL
)",
    r"CREATE TABLE url_annotations (
  url TEXT PRIMARY KEY,
  canonical_url TEXT NOT NULL,
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  source_profile TEXT
)",
    r"CREATE TABLE url_tags (
  url TEXT NOT NULL,
  canonical_url TEXT NOT NULL,
  tag TEXT NOT NULL,
  created_at TEXT NOT NULL,
  source_profile TEXT,
  PRIMARY KEY (url, tag)
)",
    r"CREATE TABLE og_image_blobs (
  blob_hash TEXT PRIMARY KEY,
  image_data BLOB NOT NULL,
  mime TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  width INTEGER,
  height INTEGER,
  recorded_at TEXT NOT NULL
)",
    r"CREATE TABLE og_images (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  page_url TEXT NOT NULL,
  page_host TEXT,
  source_og_url TEXT,
  image_blob_hash TEXT REFERENCES og_image_blobs(blob_hash),
  fetch_status TEXT NOT NULL,
  http_status INTEGER,
  fetched_at TEXT NOT NULL,
  last_shown_at TEXT,
  refetch_after TEXT,
  fetch_attempts INTEGER NOT NULL DEFAULT 1,
  created_by_run_id INTEGER REFERENCES runs(id)
)",
    r"CREATE TABLE star (
  entity_kind TEXT NOT NULL,
  entity_key TEXT NOT NULL,
  starred_at TEXT NOT NULL,
  source_profile TEXT,
  PRIMARY KEY (entity_kind, entity_key)
)",
    r"CREATE TABLE import_batches (
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
  git_commit TEXT,
  previous_status TEXT NOT NULL DEFAULT 'imported',
  visibility_cursor INTEGER NOT NULL DEFAULT 0,
  visibility_run_id INTEGER,
  visible_items INTEGER NOT NULL DEFAULT 0
)",
    r"CREATE TABLE search_documents (
  page_id INTEGER PRIMARY KEY REFERENCES pages(id) ON DELETE CASCADE,
  url TEXT NOT NULL,
  title TEXT NOT NULL,
  search_terms TEXT NOT NULL,
  normalized_url TEXT NOT NULL,
  normalized_title TEXT NOT NULL,
  normalized_search_terms TEXT NOT NULL,
  cjk_grams TEXT NOT NULL,
  enrichment_text TEXT NOT NULL DEFAULT '',
  notes_text TEXT NOT NULL DEFAULT '',
  tags_text TEXT NOT NULL DEFAULT '',
  compact_text TEXT NOT NULL,
  domain TEXT,
  newest_visit_ms INTEGER,
  updated_at TEXT NOT NULL
)",
    r"CREATE TABLE search_document_profiles (
  page_id INTEGER NOT NULL REFERENCES search_documents(page_id) ON DELETE CASCADE,
  source_profile_id INTEGER NOT NULL REFERENCES source_profiles(id),
  visible_visit_count INTEGER NOT NULL CHECK (visible_visit_count > 0),
  newest_visit_ms INTEGER NOT NULL,
  PRIMARY KEY (page_id, source_profile_id)
)",
    r"CREATE TABLE search_outbox (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  page_id INTEGER NOT NULL,
  change TEXT NOT NULL CHECK (change IN ('upsert', 'delete'))
)",
    "CREATE INDEX idx_runs_started_at ON runs(started_at DESC)",
    "CREATE INDEX idx_urls_profile_last_visit ON urls(source_profile_id, last_visit_ms DESC)",
    "CREATE INDEX idx_visits_profile_time ON visits(source_profile_id, visit_time_ms DESC)",
    "CREATE INDEX idx_downloads_profile_start ON downloads(source_profile_id, start_time_ms DESC)",
    "CREATE INDEX idx_search_terms_profile_term ON search_terms(source_profile_id, normalized_term)",
    "CREATE UNIQUE INDEX idx_source_profiles_profile_key ON source_profiles(profile_key)",
    "CREATE UNIQUE INDEX idx_urls_profile_source_url_id ON urls(source_profile_id, source_url_id)",
    "CREATE UNIQUE INDEX idx_visits_profile_source_visit_id ON visits(source_profile_id, source_visit_id)",
    "CREATE UNIQUE INDEX idx_visits_profile_event_fingerprint ON visits(source_profile_id, event_fingerprint) WHERE event_fingerprint IS NOT NULL AND event_fingerprint != ''",
    "CREATE INDEX idx_visits_import_batch_id ON visits(import_batch_id)",
    "CREATE UNIQUE INDEX idx_downloads_profile_source_download_id ON downloads(source_profile_id, source_download_id)",
    "CREATE UNIQUE INDEX idx_search_terms_profile_url_term ON search_terms(source_profile_id, url_id, normalized_term)",
    "CREATE UNIQUE INDEX idx_favicons_profile_page_icon_payload ON favicons(source_profile_id, page_url, icon_url, payload_hash) WHERE payload_hash IS NOT NULL",
    "CREATE UNIQUE INDEX idx_manifests_content_hash ON manifests(content_hash)",
    "CREATE INDEX idx_visits_visible_url_time ON visits( url_id, visit_time_ms DESC, id DESC ) WHERE reverted_at IS NULL",
    "CREATE INDEX idx_visits_visible_profile_time_id ON visits(source_profile_id, visit_time_ms ASC, id ASC) WHERE reverted_at IS NULL",
    "CREATE INDEX idx_favicons_recall_lookup ON favicons( source_profile_id, page_url, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE image_blob_hash IS NOT NULL OR image_data IS NOT NULL",
    "CREATE INDEX idx_favicons_page_lookup ON favicons( page_url, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE image_blob_hash IS NOT NULL OR image_data IS NOT NULL",
    "CREATE INDEX idx_favicons_blob_hash ON favicons(image_blob_hash) WHERE image_blob_hash IS NOT NULL",
    "CREATE INDEX idx_favicons_host_profile_lookup ON favicons( source_profile_id, page_host, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE page_host IS NOT NULL AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)",
    "CREATE INDEX idx_favicons_host_lookup ON favicons( page_host, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE page_host IS NOT NULL AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)",
    "CREATE INDEX idx_favicons_registrable_profile_lookup ON favicons( source_profile_id, page_registrable_domain, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE page_registrable_domain IS NOT NULL AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)",
    "CREATE INDEX idx_favicons_registrable_lookup ON favicons( page_registrable_domain, last_updated_ms DESC, width DESC, height DESC, id DESC ) WHERE page_registrable_domain IS NOT NULL AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)",
    "CREATE INDEX idx_url_annotations_updated_at ON url_annotations(updated_at DESC)",
    "CREATE INDEX idx_url_tags_tag ON url_tags(tag, url)",
    "CREATE UNIQUE INDEX idx_og_images_page_url ON og_images(page_url)",
    "CREATE INDEX idx_og_images_blob_hash ON og_images(image_blob_hash) WHERE image_blob_hash IS NOT NULL",
    "CREATE INDEX idx_og_images_refetch ON og_images(refetch_after) WHERE refetch_after IS NOT NULL",
    "CREATE INDEX idx_og_images_last_shown ON og_images(last_shown_at) WHERE last_shown_at IS NOT NULL",
    "CREATE INDEX idx_urls_last_visit_ms ON urls(last_visit_ms DESC)",
    "CREATE INDEX idx_star_kind_starred_at ON star(entity_kind, starred_at DESC)",
    "CREATE INDEX idx_urls_url ON urls(url)",
    "CREATE INDEX idx_visits_visible_time_id ON visits(visit_time_ms, id) WHERE reverted_at IS NULL",
    "CREATE INDEX idx_pages_registrable_domain ON pages(registrable_domain) WHERE registrable_domain IS NOT NULL",
    "CREATE INDEX idx_urls_page_id ON urls(page_id)",
    "CREATE INDEX idx_search_documents_domain_time ON search_documents(domain, newest_visit_ms DESC, page_id DESC)",
    "CREATE INDEX idx_search_documents_newest ON search_documents(newest_visit_ms DESC, page_id DESC)",
    "CREATE INDEX idx_search_document_profiles_profile ON search_document_profiles(source_profile_id, newest_visit_ms DESC, page_id)",
    "INSERT INTO runs(id,run_type,trigger,started_at,finished_at,timezone,status,profile_scope_json,stats_json,warnings_json,due_only) VALUES(0,'system','compat','1970-01-01T00:00:00+00:00','1970-01-01T00:00:00+00:00','UTC','success','[]','{}','[]',0)",
    "CREATE TABLE archive_totals(name TEXT PRIMARY KEY,value INTEGER NOT NULL)",
    "INSERT INTO archive_totals VALUES('sourceProfiles',0),('urls',0),('visits',0),('downloads',0),('manifests',0),('snapshots',0)",
    "CREATE INDEX idx_visits_batch_url ON visits(import_batch_id,url_id)",
    "CREATE INDEX idx_og_images_eviction ON og_images(fetched_at,id)",
    "CREATE INDEX idx_og_images_lru ON og_images(last_shown_at IS NULL DESC,last_shown_at,fetched_at,id)",
    "CREATE INDEX idx_pages_canonical_url ON pages(canonical_url)",
    "CREATE INDEX idx_url_annotations_canonical ON url_annotations(canonical_url,updated_at DESC,url)",
    "CREATE INDEX idx_url_tags_canonical ON url_tags(canonical_url,created_at,tag)",
];
