//! Upgrade regression: shipped v0.2.0 archive → current v0.3.0 open path.
//!
//! Responsibilities
//! - Materialize an authentic v0.2.0 on-disk archive (schema capped at the v0.2.0
//!   ceiling: archive v10, intelligence v6, search-projection v2) plus a v0.2.0
//!   `config.json` (with the since-removed `remoteBackup` block and no `ogImage`).
//! - Drive the REAL production open path with CURRENT code and prove the upgrade is
//!   loss-free and brick-free.
//!
//! What it proves
//! - No data loss: seeded urls/visits/favicons survive the forward migration.
//! - No NOTADB brick: the archive opens, `PRAGMA integrity_check` is `ok`, and every
//!   v0.3.0 table/index the migrations add is present.
//! - No config-parse failure: a v0.2.0 `config.json` loads, the removed `remoteBackup`
//!   field is ignored, and the absent `ogImage` block defaults correctly.
//! - No launch-recovery misfire: `recover_archive_on_launch` reports `Healthy`, does not
//!   rewrite `config.json`, and leaves no `.pk-*` recovery journal behind.
//! - The derived planes upgrade cleanly: the search projection reprojects to its current
//!   schema, the intelligence plane forward-applies past v6 (and re-applying is idempotent —
//!   the version and ledger row count stay put), and the agent plane migrates to its ceiling.
//!   The Phase-E config<->disk consistency invariant holds after upgrade.
//!
//! Not responsible for
//! - Encrypted / at-rest key reconciliation (covered by `archive::at_rest` tests).
//! - Backup, import, or scheduling behavior.
//!
//! Fidelity note
//! - The archive plane is byte-authentic: it applies the shipped v0.2.0 `include_str!` SQL for
//!   migrations 001-010. The rebuildable intelligence plane is materialized at v6 by running the
//!   CURRENT migration fns 1-6 (derived state, never canonical), which is sufficient here — the
//!   loss-free guarantee is asserted against canonical archive facts.

use crate::archive::{
    LaunchRecovery, check_config_disk_consistency, current_version, max_schema_version,
    open_archive_connection, open_intelligence_connection, open_source_evidence_connection,
    recover_archive_on_launch,
};
use crate::config::{ProjectPaths, ensure_paths, load_config, project_paths_with_root};
use crate::models::{AppConfig, ArchiveMode, OgImageFetchMode};
use crate::utils::{now_rfc3339, sha256_hex};
use rusqlite::{Connection, OptionalExtension, params};
use tempfile::tempdir;

/// Authentic v0.2.0 `config.json`: `remoteBackup` PRESENT (removed in v0.3.0), `ogImage`
/// ABSENT (added in v0.3.0). Proves the loader ignores the removed field and defaults the
/// new one.
const V020_CONFIG_JSON: &str = r#"{
  "initialized": true,
  "archiveMode": "Plaintext",
  "preferredLanguage": "system",
  "dueAfterHours": 72.0,
  "scheduleCheckIntervalHours": 6,
  "checkpointDays": 90,
  "captureFavicons": true,
  "selectedProfileIds": ["chrome:Default"],
  "gitEnabled": true,
  "rememberDatabaseKeyInKeyring": false,
  "appAutostart": false,
  "explorerBackgroundPrefetchPages": 5,
  "appLock": { "enabled": false, "idleTimeoutMinutes": 5, "biometricEnabled": false, "passcodeEnabled": true, "passcodeConfigured": false, "recoveryHint": null },
  "remoteBackup": { "enabled": false, "bucket": "", "region": "us-east-1", "endpoint": null, "prefix": "pathkeep", "pathStyle": true, "uploadAfterBackup": false, "credentialsSaved": false, "lastUploadedAt": null, "lastUploadedObjectKey": null, "lastError": null },
  "enrichment": { "plugins": [] },
  "deterministic": { "modules": [] },
  "ai": { "enabled": false }
}"#;

/// In-memory mirror of the v0.2.0 plaintext config, used to materialize the plaintext
/// source-evidence DB via the real open path.
fn plaintext_v020_config() -> AppConfig {
    AppConfig { archive_mode: ArchiveMode::Plaintext, initialized: true, ..AppConfig::default() }
}

/// Builds the byte-authentic v0.2.0 archive: applies embedded migrations 001-010 (identical
/// to the shipped v0.2.0 SQL) and records each in the ledger with the checksum current code
/// expects, so `run_migrations` recognizes them as already-applied and only forward-applies
/// 011-015 on open. Seeds urls/visits/favicons in FK order as loss witnesses.
fn build_v020_archive(paths: &ProjectPaths) {
    ensure_paths(paths).expect("ensure project paths");
    let conn = Connection::open(&paths.archive_database_path).expect("open archive db");
    conn.execute_batch("PRAGMA foreign_keys = ON;").expect("enable foreign keys");

    let migrations: [(i64, &str); 10] = [
        (1, include_str!("../migrations/001_initial.sql")),
        (2, include_str!("../migrations/002_archive_runtime_foundation.sql")),
        (3, include_str!("../migrations/003_history_search_fts.sql")),
        (4, include_str!("../migrations/004_favicon_recall_index.sql")),
        (5, include_str!("../migrations/005_visits_recall_lookup.sql")),
        (6, include_str!("../migrations/006_source_evidence_provenance.sql")),
        (7, include_str!("../migrations/007_visible_profile_time_index.sql")),
        (8, include_str!("../migrations/008_favicon_page_lookup.sql")),
        (9, include_str!("../migrations/009_favicon_blob_dedup.sql")),
        (10, include_str!("../migrations/010_favicon_domain_fallback.sql")),
    ];

    for (version, sql) in migrations {
        let tx = conn.unchecked_transaction().expect("begin migration tx");
        tx.execute_batch(sql).expect("apply migration sql");
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at, checksum, backup_path) VALUES (?1, ?2, ?3, NULL)",
            params![version, now_rfc3339(), sha256_hex(sql.as_bytes())],
        )
        .expect("record migration ledger row");
        tx.commit().expect("commit migration tx");
    }

    conn.execute_batch(
        r#"INSERT INTO runs (id, run_type, trigger, started_at, finished_at, status)
             VALUES (1, 'backup', 'manual', '2026-04-24T00:00:00Z', '2026-04-24T00:00:05Z', 'success');
           INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path, discovered_at)
             VALUES (1, 'chrome', 'Default', '/tmp/Default', '2026-04-24T00:00:00Z');
           INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms,
               first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
             VALUES (1, 'https://docs.news.bbc.co.uk/story', 'BBC', 1, 0, 1, '', 1, '', 1, 1);
           INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms,
               first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
             VALUES (2, 'https://www.example.com/page', 'Example', 1, 0, 1, '', 1, '', 1, 1);
           INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms,
               first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
             VALUES (3, 'about:blank', NULL, 1, 0, 1, '', 1, '', 1, 1);
           INSERT INTO visits (url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id)
             VALUES (1, 1, '2026-04-24T00:00:00Z', 1, 1);
           INSERT INTO visits (url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id)
             VALUES (2, 2, '2026-04-24T00:00:01Z', 1, 1);
           INSERT INTO favicons (page_url, icon_url, source_profile_id, created_by_run_id)
             VALUES ('https://www.example.com/page', 'https://www.example.com/icon.png', 1, 1);"#,
    )
    .expect("seed v0.2.0 fixture rows");

    assert_eq!(
        current_version(&conn).expect("read archive version"),
        10,
        "fixture archive must be capped at the v0.2.0 ceiling"
    );
    let recorded: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
        .expect("count migration ledger rows");
    assert_eq!(recorded, 10, "ledger must record exactly migrations 001-010");

    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    drop(conn);
}

/// Builds the v0.2.0 search projection (schema_version 2) with a stale document that the
/// current reprojection must drop when it detects the version drift.
fn build_v020_search_projection(paths: &ProjectPaths) {
    ensure_paths(paths).expect("ensure project paths");
    let conn = Connection::open(&paths.search_database_path).expect("open search db");
    conn.execute_batch(
        r#"CREATE TABLE IF NOT EXISTS search_projection_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS search_documents (
  url_id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT NOT NULL, search_terms TEXT NOT NULL,
  normalized_url TEXT NOT NULL, normalized_title TEXT NOT NULL, normalized_search_terms TEXT NOT NULL,
  compact_text TEXT NOT NULL, cjk_grams TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE VIRTUAL TABLE IF NOT EXISTS history_search_terms USING fts5(
  url, title, search_terms, normalized_url, normalized_title, normalized_search_terms, cjk_grams,
  content='search_documents', content_rowid='url_id', tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3 4');
CREATE VIRTUAL TABLE IF NOT EXISTS history_search_trigram USING fts5(
  compact_text, content='search_documents', content_rowid='url_id', tokenize = 'trigram');"#,
    )
    .expect("create v0.2.0 search projection schema");

    conn.execute(
        "INSERT INTO search_projection_meta (key, value) VALUES ('schema_version', '2')",
        [],
    )
    .expect("seed v0.2.0 search schema_version");
    conn.execute(
        r#"INSERT INTO search_documents (url_id, url, title, search_terms, normalized_url,
             normalized_title, normalized_search_terms, compact_text, cjk_grams, updated_at)
           VALUES (999, 'https://stale.example/', 'stale', '', '', '', '', '', '', '2026-04-24T00:00:00Z')"#,
        [],
    )
    .expect("seed stale search document");

    drop(conn);
}

/// Builds the v0.2.0 intelligence plane by applying migrations only through v6 on a raw
/// connection (NOT via `open_intelligence_connection`, which force-migrates to MAX). Seeds a
/// `sessions` marker row that must survive the forward-apply.
fn build_v6_intelligence_db(paths: &ProjectPaths) {
    ensure_paths(paths).expect("ensure project paths");
    let conn = Connection::open(&paths.intelligence_database_path).expect("open intelligence db");
    conn.execute_batch("PRAGMA foreign_keys = ON;").expect("enable foreign keys");

    crate::intelligence::apply_intelligence_migrations_through(&conn, 6)
        .expect("apply intelligence migrations through v6");

    let max_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM intelligence_schema_migrations",
            [],
            |row| row.get(0),
        )
        .expect("read intelligence schema version");
    assert_eq!(max_version, 6, "intelligence plane must be capped at v6");

    conn.execute(
        r#"INSERT INTO sessions (session_id, profile_id, first_visit_ms, last_visit_ms,
             visit_count, search_count, domain_count, computed_at)
           VALUES ('v020-upgrade-marker', 'chrome:Default', 1, 2, 1, 0, 1, '2026-04-24T00:00:00Z')"#,
        [],
    )
    .expect("seed intelligence session marker");

    drop(conn);
}

/// Whether a table with `name` exists in the connected database.
fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![name],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .expect("query sqlite_master for table")
    .is_some()
}

/// Whether an index with `name` exists in the connected database.
fn index_exists(conn: &Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name = ?1",
        params![name],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .expect("query sqlite_master for index")
    .is_some()
}

/// Asserts no `.pk-*` launch-recovery journal (import/rekey/restore) was left in the archive
/// directory, which would signal a launch-recovery misfire.
fn assert_no_crash_markers(paths: &ProjectPaths) {
    let dir = paths.archive_database_path.parent().expect("archive db path has a parent directory");
    for entry in std::fs::read_dir(dir).expect("read archive directory") {
        let entry = entry.expect("read archive directory entry");
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        assert!(
            !file_name.starts_with(".pk-"),
            "unexpected launch-recovery marker present: {file_name}"
        );
    }
}

#[test]
fn upgrade_v020_plaintext_archive_migrates_cleanly() {
    let dir = tempdir().expect("create temp root");
    let paths = project_paths_with_root(dir.path());

    build_v020_archive(&paths);
    build_v020_search_projection(&paths);
    build_v6_intelligence_db(&paths);
    std::fs::write(&paths.config_path, V020_CONFIG_JSON).expect("write v0.2.0 config.json");
    drop(
        open_source_evidence_connection(&paths, &plaintext_v020_config(), None)
            .expect("materialize source-evidence"),
    );

    // 1. The v0.2.0 config loads; removed `remoteBackup` is ignored; absent `ogImage` defaults.
    let cfg = load_config(&paths).expect("v0.2.0 config must load");
    assert_eq!(cfg.archive_mode, ArchiveMode::Plaintext);
    assert!(cfg.initialized);
    assert!(cfg.capture_favicons);
    assert_eq!(cfg.selected_profile_ids, vec!["chrome:Default".to_string()]);
    assert!(cfg.og_image.fetch_enabled);

    // 2. Launch recovery is a Healthy no-op: no config rewrite, no recovery journals.
    let config_before = std::fs::read(&paths.config_path).expect("snapshot config bytes");
    let recovery = recover_archive_on_launch(&paths, &cfg, None).expect("launch recovery");
    assert!(matches!(recovery, LaunchRecovery::Healthy));
    let config_after = std::fs::read(&paths.config_path).expect("re-read config bytes");
    assert_eq!(config_before, config_after, "healthy launch recovery must not rewrite config.json");
    assert_no_crash_markers(&paths);

    // 3. Real upgrade open path forward-applies the archive migrations.
    crate::ensure_archive_initialized(&paths, &cfg, None).expect("upgrade open");

    // 4. Archive is at head, intact, and lossless.
    let conn = open_archive_connection(&paths, &cfg, None).expect("post-upgrade open");
    assert_eq!(current_version(&conn).expect("post-upgrade version"), 15);
    assert_eq!(
        current_version(&conn).expect("post-upgrade version"),
        max_schema_version(),
        "archive must be migrated to the current ceiling"
    );
    let migration_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
        .expect("count post-upgrade migrations");
    assert_eq!(migration_count, 15);
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .expect("integrity check");
    assert_eq!(integrity, "ok", "archive must not be a NOTADB brick");

    let url_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM urls", [], |row| row.get(0)).expect("count urls");
    assert_eq!(url_count, 3, "no urls lost across upgrade");
    let visit_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM visits", [], |row| row.get(0)).expect("count visits");
    assert_eq!(visit_count, 2, "no visits lost across upgrade");
    let favicon_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM favicons", [], |row| row.get(0))
        .expect("count favicons");
    assert_eq!(favicon_count, 1, "no favicons lost across upgrade");

    let domain_1: String = conn
        .query_row("SELECT registrable_domain FROM urls WHERE id = 1", [], |row| row.get(0))
        .expect("read registrable_domain 1");
    assert_eq!(domain_1, "bbc.co.uk");
    let domain_2: String = conn
        .query_row("SELECT registrable_domain FROM urls WHERE id = 2", [], |row| row.get(0))
        .expect("read registrable_domain 2");
    assert_eq!(domain_2, "example.com");
    let domain_3: String = conn
        .query_row("SELECT registrable_domain FROM urls WHERE id = 3", [], |row| row.get(0))
        .expect("read registrable_domain 3");
    assert_eq!(domain_3, "", "opaque URL backfills to the empty-string sentinel");
    let null_domains: i64 = conn
        .query_row("SELECT COUNT(*) FROM urls WHERE registrable_domain IS NULL", [], |row| {
            row.get(0)
        })
        .expect("count null registrable_domain");
    assert_eq!(null_domains, 0, "backfill must leave no NULL registrable_domain");

    assert!(table_exists(&conn, "url_annotations"));
    assert!(table_exists(&conn, "url_tags"));
    assert!(table_exists(&conn, "og_images"));
    assert!(table_exists(&conn, "og_image_blobs"));
    assert!(table_exists(&conn, "star"));
    assert!(index_exists(&conn, "idx_urls_registrable_domain"));
    drop(conn);

    // 5. Search projection reprojects to the current schema and drops the stale doc.
    let search_conn = Connection::open(&paths.search_database_path).expect("open search db");
    let search_version: String = search_conn
        .query_row(
            "SELECT value FROM search_projection_meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("read search schema_version");
    assert_eq!(search_version, "4", "search projection must reproject to current");
    let search_docs: i64 = search_conn
        .query_row("SELECT COUNT(*) FROM search_documents", [], |row| row.get(0))
        .expect("count search documents");
    assert_eq!(search_docs, 3, "reprojection must rebuild from the 3 urls");
    let stale_docs: i64 = search_conn
        .query_row("SELECT COUNT(*) FROM search_documents WHERE url_id = 999", [], |row| row.get(0))
        .expect("count stale search documents");
    assert_eq!(stale_docs, 0, "stale seeded doc must be dropped");
    drop(search_conn);

    // 6. Intelligence plane forward-applies past v6, preserves the marker, and is idempotent.
    crate::intelligence_status(&paths, &cfg, None).expect("intelligence forward-apply");
    let intel_conn =
        open_intelligence_connection(&paths, &cfg, None).expect("open intelligence db");
    let intel_max: i64 = intel_conn
        .query_row("SELECT MAX(version) FROM intelligence_schema_migrations", [], |row| row.get(0))
        .expect("read intelligence version");
    assert!(intel_max > 6, "intelligence plane must forward-apply past v6");
    let intel_ledger_rows: i64 = intel_conn
        .query_row("SELECT COUNT(*) FROM intelligence_schema_migrations", [], |row| row.get(0))
        .expect("count intelligence ledger rows");
    let marker_count: i64 = intel_conn
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE session_id = 'v020-upgrade-marker'",
            [],
            |row| row.get(0),
        )
        .expect("count intelligence marker");
    assert_eq!(marker_count, 1, "intelligence marker must survive forward-apply");
    drop(intel_conn);

    crate::intelligence_status(&paths, &cfg, None).expect("idempotent intelligence forward-apply");
    let intel_conn_again =
        open_intelligence_connection(&paths, &cfg, None).expect("re-open intelligence db");
    let intel_max_again: i64 = intel_conn_again
        .query_row("SELECT MAX(version) FROM intelligence_schema_migrations", [], |row| row.get(0))
        .expect("re-read intelligence version");
    let intel_ledger_rows_again: i64 = intel_conn_again
        .query_row("SELECT COUNT(*) FROM intelligence_schema_migrations", [], |row| row.get(0))
        .expect("re-count intelligence ledger rows");
    // Idempotent: the second forward-apply must neither advance the ceiling nor append a
    // duplicate ledger row (a re-applied migration would inflate the row count).
    assert_eq!(intel_max_again, intel_max, "intelligence forward-apply must be idempotent");
    assert_eq!(
        intel_ledger_rows_again, intel_ledger_rows,
        "re-running intelligence forward-apply must not append duplicate ledger rows"
    );
    drop(intel_conn_again);

    // 7. Agent plane migrates to its current ceiling.
    let agent_conn = crate::open_agent_connection(&paths).expect("agent db");
    let agent_max: i64 = agent_conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM agent_schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read agent version");
    assert_eq!(agent_max, 2);
    drop(agent_conn);

    // 8. Phase-E config<->disk consistency invariant holds after upgrade.
    check_config_disk_consistency(&paths)
        .expect("Phase-E config<->disk invariant must hold after upgrade");

    // 9. The archive mode is unchanged on disk.
    assert_eq!(load_config(&paths).unwrap().archive_mode, ArchiveMode::Plaintext);
}

#[test]
fn upgrade_v020_config_ignores_removed_field_and_defaults_new_fields() {
    let dir = tempdir().expect("create temp root");
    let paths = project_paths_with_root(dir.path());
    ensure_paths(&paths).expect("ensure project paths");
    std::fs::write(&paths.config_path, V020_CONFIG_JSON).expect("write v0.2.0 config.json");

    let cfg = load_config(&paths).expect("v0.2.0 config must load");
    assert_eq!(cfg.archive_mode, ArchiveMode::Plaintext);
    assert_eq!(cfg.og_image.effective_mode(), OgImageFetchMode::Background);
    assert!(cfg.capture_favicons);
    assert_eq!(cfg.selected_profile_ids, vec!["chrome:Default".to_string()]);
}
