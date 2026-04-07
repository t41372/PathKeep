use crate::{
    ai::ensure_ai_schema,
    config::{ProjectPaths, ensure_paths},
    insights::ensure_insight_schema,
    models::{AppConfig, ArchiveMode},
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::{collections::BTreeMap, fs, path::Path, time::Duration as StdDuration};

const LEGACY_ARCHIVE_SCHEMA_SQL: &str = include_str!("../archive-schema.sql");
const MIGRATION_001_INITIAL_SQL: &str = include_str!("../migrations/001_initial.sql");

#[derive(Clone, Copy)]
struct MigrationSpec<'a> {
    version: i64,
    sql: &'a str,
}

const MIGRATIONS: &[MigrationSpec<'static>] =
    &[MigrationSpec { version: 1, sql: MIGRATION_001_INITIAL_SQL }];

pub(crate) fn open_archive_connection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Connection> {
    ensure_paths(paths)?;
    let connection = Connection::open(&paths.archive_database_path)
        .with_context(|| format!("opening {}", paths.archive_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        let key = key.context("database key is required for encrypted archives")?;
        apply_cipher_key(&connection, key)?;
    }
    Ok(connection)
}

pub(crate) fn apply_cipher_key(connection: &Connection, key: &str) -> Result<()> {
    connection.pragma_update(None, "key", key)?;
    Ok(())
}

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

pub(crate) fn create_schema(connection: &Connection) -> Result<()> {
    // The existing backup/query pipeline still targets the legacy archive tables.
    // Canonical schema v1 now lives in numbered migrations and is verified separately
    // until the M1 archive engine switches the runtime over to that data plane.
    connection.execute_batch(LEGACY_ARCHIVE_SCHEMA_SQL)?;
    ensure_column(connection, "visit_events", "import_batch_id", "INTEGER")?;
    ensure_column(connection, "visit_events", "event_fingerprint", "TEXT")?;
    ensure_column(connection, "raw_row_versions", "import_batch_id", "INTEGER")?;
    #[rustfmt::skip]
    connection.execute("CREATE INDEX IF NOT EXISTS idx_visit_events_import_batch_id ON visit_events(import_batch_id)", [])?;
    #[rustfmt::skip]
    connection.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_visit_events_profile_event_fingerprint ON visit_events(profile_id, event_fingerprint) WHERE event_fingerprint IS NOT NULL AND event_fingerprint != ''", [])?;
    #[rustfmt::skip]
    connection.execute("CREATE INDEX IF NOT EXISTS idx_raw_row_versions_import_batch_id ON raw_row_versions(import_batch_id)", [])?;
    ensure_ai_schema(connection)?;
    ensure_insight_schema(connection)?;
    Ok(())
}

pub(crate) fn ensure_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .any(|name| name == column_name);

    if !exists {
        #[rustfmt::skip]
        let _ = connection.execute(&format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}"), [])?;
    }

    Ok(())
}

pub fn current_version(connection: &Connection) -> Result<i64> {
    if !table_exists(connection, "schema_migrations")? {
        return Ok(0);
    }

    let version = connection
        .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |row| row.get(0))
        .context("loading current schema version")?;
    Ok(version)
}

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
            params![migration.version, now_rfc3339(), checksum, Option::<String>::None],
        )
        .with_context(|| format!("recording migration {}", migration.version))?;
    transaction.commit()?;
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
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
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

    fn has_table(connection: &Connection, table_name: &str) -> bool {
        table_exists(connection, table_name).expect("table check")
    }

    #[test]
    fn migration_from_scratch_succeeds() {
        let connection = Connection::open_in_memory().expect("memory db");

        run_migrations(&connection).expect("migrate");

        assert_eq!(current_version(&connection).expect("schema version"), 1);
        assert!(has_table(&connection, "runs"));
        assert!(has_table(&connection, "source_profiles"));
        assert!(has_table(&connection, "raw_row_versions"));
    }

    #[test]
    fn migration_is_idempotent() {
        let connection = Connection::open_in_memory().expect("memory db");

        run_migrations(&connection).expect("first migration");
        run_migrations(&connection).expect("second migration");

        let count = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get::<_, i64>(0))
            .expect("migration count");
        assert_eq!(count, 1);
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
        run_migrations(&connection).expect("migrate");
        assert_eq!(current_version(&connection).expect("migrated version"), 1);
    }
}
