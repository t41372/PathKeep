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
    LaunchRecovery, assess_archive_upgrade, check_config_disk_consistency, current_version,
    ensure_archive_initialized_with_progress, max_schema_version, open_archive_connection,
    open_intelligence_connection, open_source_evidence_connection, recover_archive_on_launch,
};
use crate::config::{ProjectPaths, ensure_paths, load_config, project_paths_with_root};
use crate::models::{
    AppConfig, ArchiveMode, ArchiveUpgradePhase, ArchiveUpgradeProgress, OgImageFetchMode,
};
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
fn assess_archive_upgrade_reports_pending_then_clears_after_upgrade() {
    let dir = tempdir().expect("create temp root");
    let paths = project_paths_with_root(dir.path());

    // A brand-new install (no archive file yet) is never an "upgrade".
    let fresh =
        assess_archive_upgrade(&paths, &plaintext_v020_config(), None).expect("assess fresh");
    assert!(!fresh.pending, "a fresh install must not report a pending upgrade");
    assert!(fresh.phases.is_empty(), "a fresh install has no per-phase breakdown");

    build_v020_archive(&paths);
    build_v020_search_projection(&paths);
    build_v6_intelligence_db(&paths);
    std::fs::write(&paths.config_path, V020_CONFIG_JSON).expect("write v0.2.0 config.json");
    let cfg = load_config(&paths).expect("v0.2.0 config must load");

    // A real v0.2.0 archive WITH data reports pending + honest per-phase totals,
    // WITHOUT bootstrapping the schema (the pre-check must not consume the upgrade).
    let pending = assess_archive_upgrade(&paths, &cfg, None).expect("assess pending");
    assert!(pending.pending, "a v0.2.0 archive with data must report an upgrade");
    assert_eq!(pending.current_schema_version, 10, "the archive is still at the v0.2.0 ceiling");
    assert_eq!(pending.target_schema_version, max_schema_version());
    // The pre-check MUST NOT have migrated the archive: a plain reopen still reads v10.
    let raw = Connection::open(&paths.archive_database_path).expect("raw reopen");
    assert_eq!(
        current_version(&raw).expect("still v10"),
        10,
        "assess must not migrate the archive"
    );
    drop(raw);

    let phase_total = |phase: ArchiveUpgradePhase| {
        pending
            .phases
            .iter()
            .find(|entry| entry.phase == phase)
            .unwrap_or_else(|| panic!("missing phase {phase:?} in assessment"))
    };
    let schema = phase_total(ArchiveUpgradePhase::SchemaMigration);
    assert!(schema.pending);
    assert!(schema.streamed, "the schema phase emits live progress");
    assert_eq!(schema.estimated_total, 5, "migrations 011..=015 are pending from v10");
    let backfill = phase_total(ArchiveUpgradePhase::RegistrableDomainBackfill);
    assert!(backfill.pending);
    assert!(backfill.streamed, "the backfill phase emits live progress");
    assert_eq!(backfill.estimated_total, 3, "every existing url row backfills (column is new)");
    let reprojection = phase_total(ArchiveUpgradePhase::SearchReprojection);
    assert!(reprojection.pending);
    assert!(reprojection.streamed, "the reprojection phase emits live progress");
    assert_eq!(reprojection.estimated_total, 3, "the v2→v4 reprojection rebuilds all 3 url docs");
    // The intelligence phase is reported (v6 < the current ceiling) but never gates
    // the screen, and is NOT streamed — it forward-applies lazily, not inside
    // ensure_archive_initialized, so the shell renders it as an informational line.
    let intelligence = phase_total(ArchiveUpgradePhase::Intelligence);
    assert!(intelligence.pending);
    assert!(!intelligence.streamed, "the intelligence phase is informational, never streamed");

    // After the real upgrade, the pre-check clears: nothing left for a screen.
    crate::ensure_archive_initialized(&paths, &cfg, None).expect("upgrade open");
    let done = assess_archive_upgrade(&paths, &cfg, None).expect("assess after upgrade");
    assert!(!done.pending, "an already-migrated archive reports no pending upgrade");
    assert_eq!(done.current_schema_version, max_schema_version());
    assert!(!phase_total_for(&done, ArchiveUpgradePhase::SchemaMigration).pending);
    assert!(!phase_total_for(&done, ArchiveUpgradePhase::RegistrableDomainBackfill).pending);
    assert!(!phase_total_for(&done, ArchiveUpgradePhase::SearchReprojection).pending);
}

/// Finds a phase entry in a completed assessment (which always carries the full
/// four-phase breakdown, unlike the empty not-pending fresh-install case).
fn phase_total_for(
    assessment: &crate::models::ArchiveUpgradeAssessment,
    phase: ArchiveUpgradePhase,
) -> &crate::models::ArchiveUpgradePhaseAssessment {
    assessment
        .phases
        .iter()
        .find(|entry| entry.phase == phase)
        .unwrap_or_else(|| panic!("missing phase {phase:?}"))
}

#[test]
fn assess_archive_upgrade_treats_uninitialized_and_empty_archives_as_not_pending() {
    let dir = tempdir().expect("create temp root");
    let paths = project_paths_with_root(dir.path());
    let plaintext = plaintext_v020_config();

    // (a) An existing-but-EMPTY archive file (no tables at all): current version 0,
    //     no `urls` table → nothing to upgrade.
    ensure_paths(&paths).expect("ensure project paths");
    drop(Connection::open(&paths.archive_database_path).expect("create empty archive file"));
    let empty_file = assess_archive_upgrade(&paths, &plaintext, None).expect("assess empty file");
    assert!(!empty_file.pending, "an empty archive file has no urls table → no upgrade");
    assert!(empty_file.phases.is_empty());

    // (b) A fully-bootstrapped but DATA-EMPTY archive (every table exists, 0 urls):
    //     even at head-minus this migrates instantly, so no screen.
    drop(open_archive_connection(&paths, &plaintext, None).expect("bootstrap empty archive"));
    let data_empty = assess_archive_upgrade(&paths, &plaintext, None).expect("assess data-empty");
    assert!(!data_empty.pending, "a data-empty archive migrates instantly → no screen");
    assert_eq!(data_empty.current_schema_version, max_schema_version());
    assert!(data_empty.phases.is_empty());
}

#[test]
fn assess_archive_upgrade_reports_pending_without_search_or_intelligence_planes() {
    // A v0.2.0 archive WITH data but NO search projection and NO intelligence DB
    // yet: exercises the projection-absent and intelligence-absent code paths.
    let dir = tempdir().expect("create temp root");
    let paths = project_paths_with_root(dir.path());
    build_v020_archive(&paths); // canonical data only; no derived planes built

    let assessment =
        assess_archive_upgrade(&paths, &plaintext_v020_config(), None).expect("assess");
    assert!(assessment.pending, "a v0.2.0 archive with data is a pending upgrade");
    assert!(
        phase_total_for(&assessment, ArchiveUpgradePhase::SearchReprojection).pending,
        "a missing search projection is a pending reprojection"
    );
    // An absent intelligence plane reads as version 0 (< the ceiling) → reported
    // pending for completeness (it forward-applies lazily, so it never gates).
    assert!(phase_total_for(&assessment, ArchiveUpgradePhase::Intelligence).pending);
}

#[test]
fn assess_archive_upgrade_is_not_pending_when_the_archive_cannot_be_decoded() {
    // An Encrypted config with NO key can't open the (plaintext-on-disk) archive:
    // the cheap pre-check must report not-pending rather than error, so it never
    // blocks bootstrap — the real init still surfaces any error.
    let dir = tempdir().expect("create temp root");
    let paths = project_paths_with_root(dir.path());
    build_v020_archive(&paths);
    let encrypted = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        initialized: true,
        ..AppConfig::default()
    };

    let assessment = assess_archive_upgrade(&paths, &encrypted, None).expect("assess undecodable");
    assert!(!assessment.pending, "an un-decodable archive is not a pending upgrade");
    assert!(assessment.phases.is_empty(), "no breakdown when the archive can't be read");
}

#[test]
fn upgrade_v020_with_progress_streams_phases_and_migrates_cleanly() {
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
    let cfg = load_config(&paths).expect("v0.2.0 config must load");

    // Drive the REAL upgrade open path with progress, capturing every event.
    let mut events: Vec<ArchiveUpgradeProgress> = Vec::new();
    ensure_archive_initialized_with_progress(&paths, &cfg, None, |event| events.push(event))
        .expect("upgrade open with progress");

    // Every event carries the stable i18n label key for its phase.
    for event in &events {
        assert_eq!(event.phase_label, event.phase.label_key());
        assert!(event.processed <= event.total, "processed never exceeds total: {event:?}");
    }

    // Exactly one terminal `done`, and it is the last event.
    assert!(
        events.last().map(|event| event.done).unwrap_or(false),
        "last event is the terminal done"
    );
    assert_eq!(
        events.iter().filter(|event| event.done).count(),
        1,
        "exactly one terminal done event"
    );

    // Schema-migration phase: ordinal step progress climbing to 5/5 (011..=015).
    let schema: Vec<_> =
        events.iter().filter(|e| e.phase == ArchiveUpgradePhase::SchemaMigration).collect();
    assert!(!schema.is_empty(), "the schema-migration phase must emit progress");
    assert_eq!(schema.first().unwrap().processed, 0, "schema progress starts at 0, not a jump");
    assert_eq!(schema.last().unwrap().processed, 5, "all 5 pending migrations are applied");
    assert_eq!(schema.last().unwrap().total, 5);
    assert!(
        schema.windows(2).all(|pair| pair[0].processed <= pair[1].processed),
        "schema step progress is monotonic"
    );

    // Backfill phase: REAL per-batch counts (initial 0/3 then 3/3), not a single jump.
    let backfill: Vec<_> = events
        .iter()
        .filter(|e| e.phase == ArchiveUpgradePhase::RegistrableDomainBackfill)
        .collect();
    assert!(backfill.len() >= 2, "backfill emits an initial 0 then per-batch ticks, not one jump");
    assert_eq!(backfill.first().unwrap().processed, 0);
    assert_eq!(backfill.last().unwrap().processed, 3, "backfill completes to the row total");
    assert_eq!(backfill.last().unwrap().total, 3);

    // Search-reprojection phase: initial 0/3 then 3/3.
    let reprojection: Vec<_> =
        events.iter().filter(|e| e.phase == ArchiveUpgradePhase::SearchReprojection).collect();
    assert!(reprojection.len() >= 2, "reprojection emits real progress, not a single jump");
    assert_eq!(reprojection.first().unwrap().processed, 0);
    assert_eq!(
        reprojection.last().unwrap().processed,
        3,
        "reprojection completes to the doc total"
    );
    assert_eq!(reprojection.last().unwrap().total, 3);

    // ── And the archive still migrated correctly (mirrors the loss-free regression). ──
    let conn = open_archive_connection(&paths, &cfg, None).expect("post-upgrade open");
    assert_eq!(current_version(&conn).expect("post-upgrade version"), 15);
    assert_eq!(current_version(&conn).expect("version"), max_schema_version());
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .expect("integrity check");
    assert_eq!(integrity, "ok", "the progress path must not brick the archive");
    let url_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM urls", [], |row| row.get(0)).expect("count urls");
    assert_eq!(url_count, 3, "no urls lost across the progress upgrade");
    let visit_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM visits", [], |row| row.get(0)).expect("count visits");
    assert_eq!(visit_count, 2, "no visits lost across the progress upgrade");
    let null_domains: i64 = conn
        .query_row("SELECT COUNT(*) FROM urls WHERE registrable_domain IS NULL", [], |row| {
            row.get(0)
        })
        .expect("count null registrable_domain");
    assert_eq!(null_domains, 0, "the backfill leaves no NULL registrable_domain");
    let bbc: String = conn
        .query_row("SELECT registrable_domain FROM urls WHERE id = 1", [], |row| row.get(0))
        .expect("read registrable_domain 1");
    assert_eq!(bbc, "bbc.co.uk");
    drop(conn);

    // Search projection reprojected to v4 with the 3 real docs; stale doc dropped.
    let search_conn = Connection::open(&paths.search_database_path).expect("open search db");
    let search_version: String = search_conn
        .query_row(
            "SELECT value FROM search_projection_meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("read search schema_version");
    assert_eq!(search_version, "4", "search projection reprojects to current");
    let search_docs: i64 = search_conn
        .query_row("SELECT COUNT(*) FROM search_documents", [], |row| row.get(0))
        .expect("count search documents");
    assert_eq!(search_docs, 3, "reprojection rebuilds from the 3 urls");
    let stale_docs: i64 = search_conn
        .query_row("SELECT COUNT(*) FROM search_documents WHERE url_id = 999", [], |row| row.get(0))
        .expect("count stale search documents");
    assert_eq!(stale_docs, 0, "the stale v0.2.0 doc is dropped");
    drop(search_conn);

    // Phase-E config<->disk consistency still holds after the progress upgrade.
    check_config_disk_consistency(&paths).expect("Phase-E invariant holds after progress upgrade");
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
