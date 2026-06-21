//! Archive schema bootstrap and migration pipeline.
//!
//! This module owns the canonical SQLite schema, migration ledger, and the
//! rules for opening the archive database in plaintext or encrypted mode. All
//! higher-level archive flows assume these migrations have already run.

use crate::{
    archive::search_projection::{attach_search_database, seed_search_projection_if_missing},
    config::{ProjectPaths, ensure_paths},
    models::{AppConfig, ArchiveMode},
    utils::{now_rfc3339, sha256_hex, url_domain},
    visit_taxonomy::{normalize_visit_url, registrable_domain_for_host},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
    time::Duration as StdDuration,
};

const MIGRATION_001_INITIAL_SQL: &str = include_str!("../migrations/001_initial.sql");
const MIGRATION_002_RUNTIME_SQL: &str =
    include_str!("../migrations/002_archive_runtime_foundation.sql");
const MIGRATION_003_HISTORY_SEARCH_FTS_SQL: &str =
    include_str!("../migrations/003_history_search_fts.sql");
const MIGRATION_004_FAVICON_RECALL_INDEX_SQL: &str =
    include_str!("../migrations/004_favicon_recall_index.sql");
const MIGRATION_005_VISITS_RECALL_LOOKUP_SQL: &str =
    include_str!("../migrations/005_visits_recall_lookup.sql");
const MIGRATION_006_SOURCE_EVIDENCE_PROVENANCE_SQL: &str =
    include_str!("../migrations/006_source_evidence_provenance.sql");
const MIGRATION_007_VISIBLE_PROFILE_TIME_INDEX_SQL: &str =
    include_str!("../migrations/007_visible_profile_time_index.sql");
const MIGRATION_008_FAVICON_PAGE_LOOKUP_SQL: &str =
    include_str!("../migrations/008_favicon_page_lookup.sql");
const MIGRATION_009_FAVICON_BLOB_DEDUP_SQL: &str =
    include_str!("../migrations/009_favicon_blob_dedup.sql");
const MIGRATION_010_FAVICON_DOMAIN_FALLBACK_SQL: &str =
    include_str!("../migrations/010_favicon_domain_fallback.sql");
const MIGRATION_011_NOTES_TAGS_SQL: &str = include_str!("../migrations/011_notes_tags.sql");
const MIGRATION_012_OG_IMAGES_SQL: &str = include_str!("../migrations/012_og_images.sql");
const MIGRATION_013_URLS_LAST_VISIT_INDEX_SQL: &str =
    include_str!("../migrations/013_urls_last_visit_index.sql");
const MIGRATION_014_STARS_SQL: &str = include_str!("../migrations/014_stars.sql");
const SQLITE_CACHE_SIZE_KIB: i64 = -65_536;
const SQLITE_MMAP_SIZE_BYTES: i64 = 268_435_456;

static BOOTSTRAPPED_ARCHIVES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

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

#[derive(Clone, Copy)]
struct MigrationSpec<'a> {
    version: i64,
    sql: &'a str,
}

const MIGRATIONS: &[MigrationSpec<'static>] = &[
    MigrationSpec { version: 1, sql: MIGRATION_001_INITIAL_SQL },
    MigrationSpec { version: 2, sql: MIGRATION_002_RUNTIME_SQL },
    MigrationSpec { version: 3, sql: MIGRATION_003_HISTORY_SEARCH_FTS_SQL },
    MigrationSpec { version: 4, sql: MIGRATION_004_FAVICON_RECALL_INDEX_SQL },
    MigrationSpec { version: 5, sql: MIGRATION_005_VISITS_RECALL_LOOKUP_SQL },
    MigrationSpec { version: 6, sql: MIGRATION_006_SOURCE_EVIDENCE_PROVENANCE_SQL },
    MigrationSpec { version: 7, sql: MIGRATION_007_VISIBLE_PROFILE_TIME_INDEX_SQL },
    MigrationSpec { version: 8, sql: MIGRATION_008_FAVICON_PAGE_LOOKUP_SQL },
    MigrationSpec { version: 9, sql: MIGRATION_009_FAVICON_BLOB_DEDUP_SQL },
    MigrationSpec { version: 10, sql: MIGRATION_010_FAVICON_DOMAIN_FALLBACK_SQL },
    MigrationSpec { version: 11, sql: MIGRATION_011_NOTES_TAGS_SQL },
    MigrationSpec { version: 12, sql: MIGRATION_012_OG_IMAGES_SQL },
    MigrationSpec { version: 13, sql: MIGRATION_013_URLS_LAST_VISIT_INDEX_SQL },
    MigrationSpec { version: 14, sql: MIGRATION_014_STARS_SQL },
];

/// Opens the canonical archive connection in plaintext or encrypted mode.
pub fn open_archive_connection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Connection> {
    ensure_paths(paths)?;
    let connection = Connection::open(&paths.archive_database_path)
        .with_context(|| format!("opening {}", paths.archive_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        let key = key.context("database key is required for encrypted archives")?;
        apply_cipher_key(&connection, key)?;
    }
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "cache_size", SQLITE_CACHE_SIZE_KIB)?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    let _ = connection.pragma_update(None, "mmap_size", SQLITE_MMAP_SIZE_BYTES);
    ensure_archive_bootstrapped(&connection, &paths.archive_database_path)?;
    attach_search_database(&connection, paths)?;
    seed_search_projection_if_missing(&connection, paths)?;
    Ok(connection)
}

/// Applies the archive cipher key to an already-open SQLite connection.
pub(crate) fn apply_cipher_key(connection: &Connection, key: &str) -> Result<()> {
    connection.pragma_update(None, "key", key)?;
    Ok(())
}

/// Exports the current archive database to a portable file path.
pub(crate) fn export_archive_database(
    source: &Connection,
    target_path: &Path,
    target_key: Option<&str>,
) -> Result<()> {
    remove_existing_export_target(target_path)?;

    let target = target_path.display().to_string().replace('\'', "''");
    let key = target_key.unwrap_or("").replace('\'', "''");
    source
        .execute_batch(&format!("ATTACH DATABASE '{target}' AS rekeyed KEY '{key}';"))
        .context("attaching target database for export")?;
    let export_result = source
        .query_row("SELECT sqlcipher_export('rekeyed')", [], |_| Ok(()))
        .context("exporting encrypted database");
    let detach_result = source.execute_batch("DETACH DATABASE rekeyed;");
    export_result?;
    detach_result.context("detaching exported database")?;
    Ok(())
}

fn remove_existing_export_target(target_path: &Path) -> Result<()> {
    if target_path.exists() {
        fs::remove_file(target_path)
            .with_context(|| format!("removing {}", target_path.display()))?;
    }
    Ok(())
}

/// Creates or upgrades the canonical archive schema in place.
pub fn create_schema(connection: &Connection) -> Result<()> {
    run_migrations(connection)?;
    connection.execute_batch("BEGIN IMMEDIATE").context("acquiring archive bootstrap lock")?;

    let result = (|| -> Result<()> {
        ensure_import_batch_schema(connection)?;
        Ok(())
    })();

    finish_archive_bootstrap_transaction(connection, result)
}

fn finish_archive_bootstrap_transaction(connection: &Connection, result: Result<()>) -> Result<()> {
    match result {
        Ok(()) => {
            connection.execute_batch("COMMIT").context("committing archive bootstrap")?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

/// Returns the highest schema migration version this binary knows how to
/// apply. The Export/Import flow uses this to decide whether a bundle from
/// an *older* PathKeep build can be safely upgraded forward (the bundle's
/// recorded version must be ≤ this value), or whether a bundle from a
/// *newer* build must be rejected because the local binary lacks the
/// migration to project the new schema down.
pub fn max_schema_version() -> i64 {
    MIGRATIONS.last().map(|spec| spec.version).unwrap_or(0)
}

/// Returns the schema version currently recorded in the archive metadata.
pub fn current_version(connection: &Connection) -> Result<i64> {
    if !table_exists(connection, "schema_migrations")? {
        return Ok(0);
    }

    let version = connection
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |row| row.get(0))
        .context("loading current schema version")?;
    Ok(version)
}

/// Applies any pending schema migrations and compatibility upgrades.
pub fn run_migrations(connection: &Connection) -> Result<()> {
    run_migrations_with_specs(connection, MIGRATIONS)
}

fn run_migrations_with_specs(
    connection: &Connection,
    migrations: &[MigrationSpec<'_>],
) -> Result<()> {
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", true)?;

    let applied = load_applied_migrations(connection)?;
    for migration in migrations {
        let checksum = sha256_hex(migration.sql.as_bytes());
        match applied.get(&migration.version) {
            Some(existing_checksum) if existing_checksum == &checksum => continue,
            Some(_) => anyhow::bail!(
                "migration {} checksum mismatch; the applied migration file was modified",
                migration.version
            ),
            None => apply_migration(connection, migration, &checksum)?,
        }
    }

    Ok(())
}

fn apply_migration(
    connection: &Connection,
    migration: &MigrationSpec<'_>,
    checksum: &str,
) -> Result<()> {
    let transaction = connection.unchecked_transaction()?;
    transaction
        .execute_batch(migration.sql)
        .with_context(|| format!("applying migration {}", migration.version))?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, applied_at, checksum, backup_path)
             VALUES (?1, ?2, ?3, ?4)",
            params![migration.version, now_rfc3339(), checksum, Option::<String>::None,],
        )
        .with_context(|| format!("recording migration {}", migration.version))?;
    transaction.commit()?;
    Ok(())
}

fn ensure_import_batch_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(IMPORT_BATCH_SCHEMA_SQL)?;
    Ok(())
}

fn ensure_archive_bootstrapped(
    connection: &Connection,
    archive_database_path: &Path,
) -> Result<()> {
    let cache_key = archive_database_path.display().to_string();
    let bootstrapped = BOOTSTRAPPED_ARCHIVES.get_or_init(|| Mutex::new(HashSet::new()));
    let mut bootstrapped = bootstrapped.lock().expect("archive bootstrap cache lock");
    if bootstrapped.contains(&cache_key) && table_exists(connection, "schema_migrations")? {
        return Ok(());
    }

    create_schema(connection)?;
    bootstrapped.insert(cache_key);
    Ok(())
}

fn load_applied_migrations(connection: &Connection) -> Result<BTreeMap<i64, String>> {
    if !table_exists(connection, "schema_migrations")? {
        return Ok(BTreeMap::new());
    }

    let mut statement = connection
        .prepare("SELECT version, checksum FROM schema_migrations ORDER BY version ASC")?;
    let rows =
        statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
    let applied = rows.collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
    Ok(applied)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaviconUrlMetadata {
    pub(crate) host: Option<String>,
    pub(crate) registrable_domain: Option<String>,
}

pub(crate) fn favicon_url_metadata(page_url: &str) -> FaviconUrlMetadata {
    if let Some(normalized) = normalize_visit_url(page_url) {
        return FaviconUrlMetadata {
            host: Some(normalized.host),
            registrable_domain: Some(normalized.registrable_domain),
        };
    }

    let host = url_domain(page_url)
        .split('@')
        .next_back()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty() {
        return FaviconUrlMetadata { host: None, registrable_domain: None };
    }

    let registrable_domain = registrable_domain_for_host(&host);
    FaviconUrlMetadata {
        host: Some(host),
        registrable_domain: (!registrable_domain.is_empty()).then_some(registrable_domain),
    }
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    let exists = connection
        .query_row(
            "SELECT 1
             FROM sqlite_master
             WHERE type = 'table'
               AND name = ?1
             LIMIT 1",
            [table_name],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::project_paths_with_root,
        models::{AppConfig, ArchiveMode},
    };
    use rusqlite::OptionalExtension;
    use std::sync::{Arc, Barrier};

    fn has_table(connection: &Connection, table_name: &str) -> bool {
        table_exists(connection, table_name).expect("table check")
    }

    fn has_index(connection: &Connection, index_name: &str) -> bool {
        connection
            .query_row(
                "SELECT 1
                 FROM sqlite_master
                 WHERE type = 'index'
                   AND name = ?1
                 LIMIT 1",
                [index_name],
                |_| Ok(()),
            )
            .optional()
            .expect("index lookup")
            .is_some()
    }

    #[test]
    fn export_target_cleanup_removes_existing_files_and_ignores_missing_targets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("export.sqlite");

        remove_existing_export_target(&target).expect("missing target is fine");
        fs::write(&target, "old export").expect("write existing target");
        remove_existing_export_target(&target).expect("remove existing target");
        assert!(!target.exists());
    }

    #[test]
    fn migration_from_scratch_succeeds() {
        let connection = Connection::open_in_memory().expect("memory db");

        create_schema(&connection).expect("create schema");

        assert_eq!(current_version(&connection).expect("schema version"), 14);
        assert!(has_table(&connection, "runs"));
        assert!(has_table(&connection, "source_profiles"));
        assert!(has_table(&connection, "profile_watermarks"));
        assert!(has_table(&connection, "import_batches"));
        assert!(has_table(&connection, "favicon_blobs"));
        assert!(has_table(&connection, "url_annotations"));
        assert!(has_table(&connection, "url_tags"));
        assert!(has_table(&connection, "og_images"));
        assert!(has_table(&connection, "og_image_blobs"));
        assert!(has_table(&connection, "star"));
        assert!(has_index(&connection, "idx_star_kind_starred_at"));
        assert!(has_index(&connection, "idx_urls_url"));
        assert!(has_index(&connection, "idx_visits_visible_profile_time_id"));
        assert!(has_index(&connection, "idx_favicons_page_lookup"));
        assert!(has_index(&connection, "idx_favicons_blob_hash"));
        assert!(has_index(&connection, "idx_favicons_host_profile_lookup"));
        assert!(has_index(&connection, "idx_favicons_registrable_profile_lookup"));
        assert!(has_index(&connection, "idx_url_annotations_updated_at"));
        assert!(has_index(&connection, "idx_url_tags_tag"));
        assert!(has_index(&connection, "idx_og_images_page_url"));
        assert!(has_index(&connection, "idx_og_images_blob_hash"));
        assert!(has_index(&connection, "idx_og_images_refetch"));
        assert!(has_index(&connection, "idx_og_images_last_shown"));
        assert!(!has_table(&connection, "history_search"));
        let legacy_surface_count: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM sqlite_master
                 WHERE name IN ('profiles', 'visit_events', 'profiles_insert', 'visit_events_insert', 'visit_events_delete')",
                [],
                |row| row.get(0),
            )
            .expect("legacy surface count");
        assert_eq!(legacy_surface_count, 0);
    }

    #[test]
    fn migration_is_idempotent() {
        let connection = Connection::open_in_memory().expect("memory db");

        create_schema(&connection).expect("first migration");
        create_schema(&connection).expect("second migration");

        let count = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get::<_, i64>(0))
            .expect("migration count");
        assert_eq!(count, 14);
    }

    #[test]
    fn create_schema_rolls_back_import_batch_schema_failures() {
        let connection = Connection::open_in_memory().expect("memory db");

        create_schema(&connection).expect("initial schema");
        connection.execute_batch("BEGIN IMMEDIATE").expect("begin transaction");
        let error = finish_archive_bootstrap_transaction(&connection, Err(anyhow::anyhow!("boom")))
            .expect_err("bootstrap failure rolls back");
        assert_eq!(error.to_string(), "boom");
        assert!(
            connection.execute_batch("BEGIN IMMEDIATE; ROLLBACK;").is_ok(),
            "failed schema refresh must leave no open transaction"
        );
    }

    #[test]
    fn create_schema_does_not_backfill_existing_favicon_url_metadata() {
        let connection = Connection::open_in_memory().expect("memory db");

        create_schema(&connection).expect("create schema");
        connection
            .execute(
                "INSERT INTO runs (id, run_type, trigger, started_at, status)
                 VALUES (1, 'backup', 'manual', '2026-04-24T00:00:00Z', 'success')",
                [],
            )
            .expect("insert parent run");
        connection
            .execute(
                "INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path, discovered_at)
                 VALUES (1, 'chrome', 'Default', '/tmp/Default', '2026-04-24T00:00:00Z')",
                [],
            )
            .expect("insert parent profile");
        connection
            .execute(
                "INSERT INTO favicons (page_url, icon_url, source_profile_id, created_by_run_id)
                 VALUES ('https://docs.example.com/start', 'https://docs.example.com/icon.png', 1, 1)",
                [],
            )
            .expect("insert legacy favicon row");

        create_schema(&connection).expect("reopen schema");

        let metadata = connection
            .query_row(
                "SELECT page_host, page_registrable_domain
                 FROM favicons
                 WHERE page_url = 'https://docs.example.com/start'",
                [],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .expect("read favicon metadata");
        assert_eq!(metadata, (None, None));
    }

    #[test]
    fn migration_checksum_mismatch_returns_err() {
        let connection = Connection::open_in_memory().expect("memory db");
        let original = [MigrationSpec {
            version: 1,
            sql: "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, checksum TEXT NOT NULL, backup_path TEXT);\nCREATE TABLE sample (id INTEGER PRIMARY KEY);\n",
        }];
        let modified = [MigrationSpec {
            version: 1,
            sql: "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, checksum TEXT NOT NULL, backup_path TEXT);\nCREATE TABLE sample (id INTEGER PRIMARY KEY, label TEXT);\n",
        }];

        run_migrations_with_specs(&connection, &original).expect("initial migration");
        let error =
            run_migrations_with_specs(&connection, &modified).expect_err("checksum mismatch");

        assert!(error.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn migration_014_upgrades_from_v013_and_is_idempotent() {
        // Apply migrations 1..=13 only (the schema as it shipped before stars),
        // then run the full ledger so migration 014 lands as an *upgrade* — the
        // path real users hit when an older archive opens under a stars-aware
        // build. Idempotency + checksum stability are covered by re-running.
        let connection = Connection::open_in_memory().expect("memory db");
        let through_13: Vec<MigrationSpec<'static>> =
            MIGRATIONS.iter().filter(|spec| spec.version <= 13).copied().collect();
        run_migrations_with_specs(&connection, &through_13).expect("apply v1..=v13");
        assert_eq!(current_version(&connection).expect("pre-stars version"), 13);
        assert!(!has_table(&connection, "star"), "star table must not exist before v14");

        // Forward-migrate to v14.
        run_migrations(&connection).expect("apply v14 upgrade");
        assert_eq!(current_version(&connection).expect("post-stars version"), 14);
        assert!(has_table(&connection, "star"));
        assert!(has_index(&connection, "idx_star_kind_starred_at"));
        assert!(has_index(&connection, "idx_urls_url"));

        // Re-running is a no-op: the checksum matches, so the count stays put.
        run_migrations(&connection).expect("idempotent re-run");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
            .expect("migration count");
        assert_eq!(count, 14);

        // The recorded checksum matches the SQL on disk (tamper guard).
        let recorded: String = connection
            .query_row("SELECT checksum FROM schema_migrations WHERE version = 14", [], |row| {
                row.get(0)
            })
            .expect("read v14 checksum");
        assert_eq!(recorded, sha256_hex(MIGRATION_014_STARS_SQL.as_bytes()));
    }

    #[test]
    fn migration_version_reported_correctly() {
        let connection = Connection::open_in_memory().expect("memory db");

        assert_eq!(current_version(&connection).expect("initial version"), 0);
        create_schema(&connection).expect("create schema");
        assert_eq!(current_version(&connection).expect("migrated version"), 14);
    }

    #[test]
    fn favicon_url_metadata_normalizes_host_and_registrable_domain() {
        assert_eq!(
            favicon_url_metadata("https://docs.news.bbc.co.uk/path"),
            FaviconUrlMetadata {
                host: Some("docs.news.bbc.co.uk".to_string()),
                registrable_domain: Some("bbc.co.uk".to_string()),
            }
        );
        assert_eq!(
            favicon_url_metadata("example.com:443/path"),
            FaviconUrlMetadata {
                host: Some("example.com".to_string()),
                registrable_domain: Some("example.com".to_string()),
            }
        );
        assert_eq!(
            favicon_url_metadata("   "),
            FaviconUrlMetadata { host: None, registrable_domain: None }
        );
    }

    #[test]
    fn concurrent_archive_opens_bootstrap_once_without_legacy_bridge_artifacts() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let barrier = Arc::new(Barrier::new(2));

        std::thread::scope(|scope| {
            let mut joins = Vec::new();
            for _ in 0..2 {
                let paths = paths.clone();
                let config = config.clone();
                let barrier = Arc::clone(&barrier);
                joins.push(scope.spawn(move || {
                    barrier.wait();
                    let connection =
                        open_archive_connection(&paths, &config, None).expect("open archive");
                    current_version(&connection).expect("schema version")
                }));
            }

            for join in joins {
                assert_eq!(join.join().expect("thread join"), 14);
            }
        });

        let connection = open_archive_connection(&paths, &config, None).expect("reopen archive");
        let legacy_surface_count: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM sqlite_master
                 WHERE name IN ('profiles', 'visit_events', 'profiles_insert', 'visit_events_insert', 'visit_events_delete')",
                [],
                |row| row.get(0),
            )
            .expect("count legacy bridge objects");
        assert_eq!(legacy_surface_count, 0);
    }
}
