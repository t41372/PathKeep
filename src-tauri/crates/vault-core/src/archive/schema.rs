//! Archive schema bootstrap and migration pipeline.
//!
//! This module owns the canonical SQLite schema, migration ledger, and the
//! rules for opening the archive database in plaintext or encrypted mode. All
//! higher-level archive flows assume these migrations have already run.

use crate::{
    archive::search_projection::{attach_search_database, seed_search_projection_if_missing},
    config::{ProjectPaths, ensure_paths},
    models::{AppConfig, ArchiveMode},
    utils::{now_rfc3339, sha256_hex},
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
    if target_path.exists() {
        fs::remove_file(target_path)
            .with_context(|| format!("removing {}", target_path.display()))?;
    }

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

/// Creates or upgrades the canonical archive schema in place.
pub fn create_schema(connection: &Connection) -> Result<()> {
    run_migrations(connection)?;
    connection.execute_batch("BEGIN IMMEDIATE").context("acquiring archive bootstrap lock")?;

    let result = (|| -> Result<()> {
        ensure_import_batch_schema(connection)?;
        Ok(())
    })();

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
    fn migration_from_scratch_succeeds() {
        let connection = Connection::open_in_memory().expect("memory db");

        create_schema(&connection).expect("create schema");

        assert_eq!(current_version(&connection).expect("schema version"), 8);
        assert!(has_table(&connection, "runs"));
        assert!(has_table(&connection, "source_profiles"));
        assert!(has_table(&connection, "profile_watermarks"));
        assert!(has_table(&connection, "import_batches"));
        assert!(has_index(&connection, "idx_visits_visible_profile_time_id"));
        assert!(has_index(&connection, "idx_favicons_page_lookup"));
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
        assert_eq!(count, 8);
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
    fn migration_version_reported_correctly() {
        let connection = Connection::open_in_memory().expect("memory db");

        assert_eq!(current_version(&connection).expect("initial version"), 0);
        create_schema(&connection).expect("create schema");
        assert_eq!(current_version(&connection).expect("migrated version"), 8);
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
                assert_eq!(join.join().expect("thread join"), 8);
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
