//! Responsibilities: build archives in the old SQLCipher format for tests of everything that
//! reads them (conversion, old-format recovery, the update screen), from the schema the old
//! releases shipped. Only the converter reads this format, so the fixtures live with it.
//! Not responsible for: native archives (`vault_core::test_support`), or running the old app's
//! upgrade logic: files are created at the last old schema (migration 016) unless a test builds
//! an older one itself from [`ARCHIVE_MIGRATIONS`].
//! Failure modes: a fixture that drifts from what an old release wrote, so conversion is tested
//! against a shape no user has.
use rusqlite::{Connection, params};
use std::path::Path;
use vault_core::{AppConfig, ArchiveMode, ProjectPaths};

/// The old archive schema migrations exactly as released, 001 (v0.1) to 016 (v0.3).
pub const ARCHIVE_MIGRATIONS: [&str; 16] = [
    include_str!("test_support/legacy-schema/001_initial.sql"),
    include_str!("test_support/legacy-schema/002_archive_runtime_foundation.sql"),
    include_str!("test_support/legacy-schema/003_history_search_fts.sql"),
    include_str!("test_support/legacy-schema/004_favicon_recall_index.sql"),
    include_str!("test_support/legacy-schema/005_visits_recall_lookup.sql"),
    include_str!("test_support/legacy-schema/006_source_evidence_provenance.sql"),
    include_str!("test_support/legacy-schema/007_visible_profile_time_index.sql"),
    include_str!("test_support/legacy-schema/008_favicon_page_lookup.sql"),
    include_str!("test_support/legacy-schema/009_favicon_blob_dedup.sql"),
    include_str!("test_support/legacy-schema/010_favicon_domain_fallback.sql"),
    include_str!("test_support/legacy-schema/011_notes_tags.sql"),
    include_str!("test_support/legacy-schema/012_og_images.sql"),
    include_str!("test_support/legacy-schema/013_urls_last_visit_index.sql"),
    include_str!("test_support/legacy-schema/014_stars.sql"),
    include_str!("test_support/legacy-schema/015_urls_registrable_domain.sql"),
    include_str!("test_support/legacy-schema/016_visible_time_index.sql"),
];

const IMPORT_BATCH_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS import_batches (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  source_kind  TEXT NOT NULL,
  source_path  TEXT NOT NULL,
  profile_id   TEXT NOT NULL,
  created_at   TEXT NOT NULL,
  imported_at  TEXT,
  reverted_at  TEXT,
  status       TEXT NOT NULL,
  summary_json TEXT NOT NULL,
  audit_path   TEXT,
  git_commit   TEXT
);
"#;

/// Opens (creating at the last old schema if needed) `archive/history-vault.sqlite`, keyed
/// with `key` when `config` says Encrypted, as the old app opened it.
pub fn open_archive(paths: &ProjectPaths, config: &AppConfig, key: Option<&str>) -> Connection {
    open_keyed(&paths.legacy.archive, config, key, |db| {
        // The old app ran its archive in WAL mode.
        db.pragma_update(None, "journal_mode", "WAL").unwrap();
        let version: i64 = db
            .query_row(
                "SELECT CASE WHEN EXISTS(SELECT 1 FROM sqlite_schema WHERE name='schema_migrations') THEN 1 ELSE 0 END",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let applied: i64 = if version == 0 {
            0
        } else {
            db.query_row("SELECT COALESCE(MAX(version),0) FROM schema_migrations", [], |r| r.get(0))
                .unwrap()
        };
        for (index, sql) in ARCHIVE_MIGRATIONS.iter().enumerate().skip(applied as usize) {
            db.execute_batch(sql).unwrap();
            db.execute(
                "INSERT INTO schema_migrations(version, applied_at, checksum, backup_path) VALUES (?1, 'fixture', 'fixture', NULL)",
                params![index as i64 + 1],
            )
            .unwrap();
        }
        db.execute_batch(IMPORT_BATCH_SCHEMA_SQL).unwrap();
    })
}

/// Opens (creating if needed) the old `archive/source-evidence.sqlite`.
pub fn open_source_evidence(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Connection {
    open_keyed(&paths.legacy.source_evidence, config, key, |db| {
        db.execute_batch(SOURCE_EVIDENCE_SCHEMA_SQL).unwrap()
    })
}

/// The old intelligence plane (`derived/history-intelligence.sqlite`), keyed like the archive
/// when `password` is set, with the tables conversion reads: the job queue, fetched page
/// content, refind pages and the AI tables.
pub fn open_intelligence(paths: &ProjectPaths, password: Option<&str>) -> Connection {
    std::fs::create_dir_all(&paths.derived_dir).unwrap();
    let connection =
        Connection::open(paths.derived_dir.join("history-intelligence.sqlite")).unwrap();
    if let Some(password) = password {
        connection.pragma_update(None, "key", password).unwrap();
    }
    connection.pragma_update(None, "foreign_keys", true).unwrap();
    connection.execute_batch(LEGACY_INTELLIGENCE_SQL).unwrap();
    connection.execute_batch(VISIT_CONTENT_ENRICHMENTS_SCHEMA_SQL).unwrap();
    connection
}

/// The SQLite agent plane PathKeep wrote before Turso (`derived/agent.sqlite`,
/// plaintext): its v1 chat and v2 run-trace schema, verbatim, so conversion tests
/// read the same tables an existing install has.
pub fn open_agent(paths: &ProjectPaths) -> Connection {
    std::fs::create_dir_all(&paths.derived_dir).unwrap();
    let connection = Connection::open(paths.derived_dir.join("agent.sqlite")).unwrap();
    connection.pragma_update(None, "foreign_keys", true).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE agent_schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at TEXT NOT NULL);
             CREATE TABLE conversations (id TEXT PRIMARY KEY, title TEXT NOT NULL, provider_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, message_count INTEGER NOT NULL DEFAULT 0);
             CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);
             CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE, seq INTEGER NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL, reasoning TEXT, tool_calls_json TEXT, status TEXT, created_at TEXT NOT NULL);
             CREATE INDEX idx_messages_conversation_seq ON messages(conversation_id, seq);
             CREATE TABLE agent_runs (id TEXT PRIMARY KEY, conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE, message_id TEXT, provider_id TEXT, embedding_provider_id TEXT, status TEXT NOT NULL, iterations INTEGER NOT NULL DEFAULT 0, prompt_tokens INTEGER NOT NULL DEFAULT 0, completion_tokens INTEGER NOT NULL DEFAULT 0, error TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
             CREATE INDEX idx_agent_runs_conversation ON agent_runs(conversation_id);
             CREATE TABLE agent_steps (id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE, seq INTEGER NOT NULL, turn INTEGER NOT NULL, kind TEXT NOT NULL, tool_name TEXT, tool_call_id TEXT, payload TEXT NOT NULL, created_at TEXT NOT NULL);
             CREATE INDEX idx_agent_steps_run_seq ON agent_steps(run_id, seq);
             CREATE TABLE agent_citations (run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE, history_id INTEGER NOT NULL, canonical_url TEXT NOT NULL, url TEXT NOT NULL, title TEXT, visited_at TEXT, score REAL, PRIMARY KEY(run_id, history_id));
             INSERT INTO agent_schema_migrations VALUES (1, 'chat-history-baseline', 'fixture', 'saved'), (2, 'agent-run-trace', 'fixture', 'saved');",
        )
        .unwrap();
    connection
}

/// An old-format archive holding `pages` (one visit each), a run, a Chrome profile and a
/// download, the shape every release since v0.3 wrote.
pub fn seed_reader_pages(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    pages: &[(&str, &str)],
) {
    let db = open_archive(paths, config, key);
    let at = chrono::Utc::now().to_rfc3339();
    let ms = chrono::Utc::now().timestamp_millis();
    db.execute("INSERT OR IGNORE INTO runs(id,run_type,trigger,started_at,finished_at,status,stats_json,warnings_json) VALUES(1,'backup','manual',?1,?1,'success','{}','[]')",[&at]).unwrap();
    db.execute("INSERT OR IGNORE INTO source_profiles(id,browser_kind,browser_family,browser_product,profile_name,profile_path,discovered_at,enabled,profile_key,updated_at) VALUES(1,'chrome','chromium','Chrome','Default','/fixture',?1,1,'chrome:Default',?1)",[&at]).unwrap();
    for (index, (url, title)) in pages.iter().enumerate() {
        let id = index as i64 + 1;
        db.execute("INSERT OR IGNORE INTO urls(id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id,registrable_domain) VALUES(?5,?1,?2,1,1,?3,?4,?3,?4,1,1,?5,'example.com')",rusqlite::params![url,title,ms,at,id]).unwrap();
        db.execute("INSERT OR IGNORE INTO visits(id,url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id,source_visit_id) VALUES(?3,?3,1,?1,?2,1,?4)",rusqlite::params![ms,at,id,id.to_string()]).unwrap();
    }
    db.execute("INSERT OR IGNORE INTO downloads(id,source_profile_id,source_download_id,created_by_run_id,target_path,start_time_ms) VALUES(1,1,1,1,'/fixture/download',?1)",[ms]).unwrap();
}

fn open_keyed(
    path: &Path,
    config: &AppConfig,
    key: Option<&str>,
    schema: impl FnOnce(&Connection),
) -> Connection {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let connection = Connection::open(path).unwrap();
    if config.archive_mode == ArchiveMode::Encrypted {
        connection
            .pragma_update(None, "key", key.expect("an encrypted fixture needs its key"))
            .unwrap();
    }
    connection.pragma_update(None, "foreign_keys", true).unwrap();
    schema(&connection);
    connection
}

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

const LEGACY_INTELLIGENCE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS intelligence_jobs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_type TEXT NOT NULL,
  plugin_id TEXT,
  run_id INTEGER,
  state TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 100,
  attempt INTEGER NOT NULL DEFAULT 0,
  dedupe_key TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  artifact_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  scheduled_at TEXT NOT NULL,
  started_at TEXT,
  finished_at TEXT,
  heartbeat_at TEXT,
  lease_owner TEXT,
  lease_expires_at TEXT,
  updated_at TEXT NOT NULL,
  last_error TEXT,
  cancellation_reason TEXT,
  stop_requested INTEGER NOT NULL DEFAULT 0,
  UNIQUE(dedupe_key)
);
CREATE INDEX IF NOT EXISTS idx_intelligence_jobs_state
  ON intelligence_jobs(job_type, state, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_intelligence_jobs_plugin_state
  ON intelligence_jobs(plugin_id, state, updated_at DESC);
CREATE TABLE IF NOT EXISTS intelligence_job_triggers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_id INTEGER NOT NULL REFERENCES intelligence_jobs(id) ON DELETE CASCADE,
  run_id INTEGER,
  reason TEXT,
  requested_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_intelligence_job_triggers_job_id
  ON intelligence_job_triggers(job_id, requested_at DESC);
CREATE TABLE IF NOT EXISTS deterministic_module_runtime (
  module_id TEXT PRIMARY KEY,
  version TEXT NOT NULL,
  status TEXT NOT NULL,
  depends_on_json TEXT NOT NULL,
  derived_tables_json TEXT NOT NULL,
  last_run_id INTEGER,
  last_built_at TEXT,
  last_invalidated_at TEXT,
  stale_reason TEXT,
  stale_reason_code TEXT,
  notes_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS refind_pages (
  profile_id            TEXT NOT NULL,
  canonical_url         TEXT NOT NULL,
  url                   TEXT NOT NULL,
  title                 TEXT,
  registrable_domain    TEXT NOT NULL,
  cross_day_count       INTEGER NOT NULL,
  trail_count           INTEGER NOT NULL,
  search_arrival_count  INTEGER NOT NULL,
  typed_revisit_count   INTEGER NOT NULL,
  refind_score          REAL NOT NULL,
  evidence_json         TEXT NOT NULL,
  first_seen_ms         INTEGER NOT NULL,
  last_seen_ms          INTEGER NOT NULL,
  computed_at           TEXT NOT NULL,
  PRIMARY KEY(profile_id, canonical_url)
);

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

const VISIT_CONTENT_ENRICHMENTS_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS visit_content_enrichments (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  history_id          INTEGER NOT NULL,
  content_source      TEXT NOT NULL,
  fetch_status        TEXT NOT NULL,
  fetched_at          TEXT NOT NULL,
  final_url           TEXT,
  language            TEXT,
  readable_title      TEXT,
  readable_text_blob_path TEXT,
  readable_text_bytes INTEGER NOT NULL DEFAULT 0,
  text_hash           TEXT,
  snippet_json        TEXT NOT NULL,
  extraction_json     TEXT NOT NULL,
  pipeline_version    TEXT NOT NULL,
  -- W-ENRICH-1 (migration 015 / intelligence-plane v8): the extractor schema
  -- version (bump → bounded refetch of just this source's rows) and the capped
  -- inline summary that feeds BOTH the dedup content_hash (05 §1) and the FTS5
  -- `enrichment_text` mirror without reading the (possibly large) blob.
  extractor_version   INTEGER,
  enrichment_summary  TEXT,
  -- W-ENRICH-1 negative-cache cadence: the ISO timestamp before which a failed
  -- fetch must not be retried (mirrors og_images' refetch_after) and the http
  -- status of the last fetch, kept honest for the detail panel + diagnostics.
  refetch_after       TEXT,
  http_status         INTEGER,
  UNIQUE(history_id, content_source)
);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_history_id
  ON visit_content_enrichments(history_id);
CREATE INDEX IF NOT EXISTS idx_visit_content_enrichments_status
  ON visit_content_enrichments(fetch_status, fetched_at);
"#;
