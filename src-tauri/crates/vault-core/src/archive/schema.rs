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

const MIGRATION_001_INITIAL_SQL: &str = include_str!("../migrations/001_initial.sql");
const MIGRATION_002_RUNTIME_SQL: &str =
    include_str!("../migrations/002_archive_runtime_foundation.sql");

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

const LEGACY_PROFILES_VIEW_SQL: &str = r#"
CREATE VIEW profiles AS
SELECT
  profile_key AS profile_id,
  profile_name,
  user_name,
  profile_path,
  browser_version AS chrome_version,
  COALESCE(updated_at, discovered_at) AS updated_at
FROM source_profiles;
"#;

const LEGACY_VISIT_EVENTS_VIEW_SQL: &str = r#"
CREATE VIEW visit_events AS
SELECT
  visits.id AS id,
  source_profiles.profile_key AS profile_id,
  CAST(visits.source_visit_id AS INTEGER) AS source_visit_id,
  urls.id AS source_url_id,
  urls.url AS url,
  urls.title AS title,
  (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time,
  visits.from_visit AS from_visit,
  visits.transition_type AS transition,
  visits.visit_duration_ms AS visit_duration,
  visits.is_known_to_sync AS is_known_to_sync,
  visits.visited_link_id AS visited_link_id,
  visits.external_referrer_url AS external_referrer_url,
  visits.app_id AS app_id,
  visits.event_fingerprint AS event_fingerprint,
  visits.payload_hash AS payload_hash,
  visits.recorded_at AS recorded_at,
  visits.import_batch_id AS import_batch_id
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
WHERE visits.reverted_at IS NULL;
"#;

const LEGACY_VIEW_TRIGGER_SQL: &str = r#"
CREATE TRIGGER profiles_insert
INSTEAD OF INSERT ON profiles
BEGIN
  INSERT INTO source_profiles (
    browser_kind,
    browser_version,
    profile_name,
    profile_path,
    discovered_at,
    enabled,
    profile_key,
    user_name,
    updated_at
  )
  VALUES (
    CASE
      WHEN instr(NEW.profile_id, ':') > 0 THEN substr(NEW.profile_id, 1, instr(NEW.profile_id, ':') - 1)
      ELSE COALESCE(NEW.profile_id, 'legacy')
    END,
    NEW.chrome_version,
    NEW.profile_name,
    NEW.profile_path,
    COALESCE(NEW.updated_at, CURRENT_TIMESTAMP),
    1,
    NEW.profile_id,
    NEW.user_name,
    COALESCE(NEW.updated_at, CURRENT_TIMESTAMP)
  )
  ON CONFLICT(profile_key) DO UPDATE SET
    browser_version = excluded.browser_version,
    profile_name = excluded.profile_name,
    profile_path = excluded.profile_path,
    user_name = excluded.user_name,
    updated_at = excluded.updated_at,
    enabled = 1;
END;

CREATE TRIGGER visit_events_insert
INSTEAD OF INSERT ON visit_events
BEGIN
  INSERT INTO source_profiles (
    browser_kind,
    browser_version,
    profile_name,
    profile_path,
    discovered_at,
    enabled,
    profile_key,
    updated_at
  )
  VALUES (
    CASE
      WHEN instr(NEW.profile_id, ':') > 0 THEN substr(NEW.profile_id, 1, instr(NEW.profile_id, ':') - 1)
      ELSE COALESCE(NEW.profile_id, 'legacy')
    END,
    NULL,
    COALESCE(NEW.profile_id, 'legacy'),
    COALESCE(NEW.profile_id, 'legacy'),
    COALESCE(NEW.recorded_at, CURRENT_TIMESTAMP),
    1,
    NEW.profile_id,
    COALESCE(NEW.recorded_at, CURRENT_TIMESTAMP)
  )
  ON CONFLICT(profile_key) DO UPDATE SET
    updated_at = excluded.updated_at,
    enabled = 1;

  INSERT INTO urls (
    url,
    title,
    visit_count,
    typed_count,
    first_visit_ms,
    first_visit_iso,
    last_visit_ms,
    last_visit_iso,
    source_profile_id,
    created_by_run_id,
    source_url_id,
    hidden,
    payload_hash,
    recorded_at
  )
  VALUES (
    NEW.url,
    NEW.title,
    1,
    0,
    CAST((NEW.visit_time - 11644473600000000) / 1000 AS INTEGER),
    COALESCE(NEW.recorded_at, CURRENT_TIMESTAMP),
    CAST((NEW.visit_time - 11644473600000000) / 1000 AS INTEGER),
    COALESCE(NEW.recorded_at, CURRENT_TIMESTAMP),
    (SELECT id FROM source_profiles WHERE profile_key = NEW.profile_id),
    0,
    NEW.source_url_id,
    0,
    COALESCE(NEW.payload_hash, NEW.event_fingerprint, 'legacy-view'),
    COALESCE(NEW.recorded_at, CURRENT_TIMESTAMP)
  )
  ON CONFLICT(source_profile_id, source_url_id) DO UPDATE SET
    url = excluded.url,
    title = excluded.title,
    payload_hash = excluded.payload_hash,
    recorded_at = excluded.recorded_at,
    last_visit_ms = CASE
      WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_ms
      ELSE urls.last_visit_ms
    END,
    last_visit_iso = CASE
      WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_iso
      ELSE urls.last_visit_iso
    END;

  INSERT OR REPLACE INTO visits (
    id,
    url_id,
    source_visit_id,
    visit_time_ms,
    visit_time_iso,
    transition_type,
    visit_duration_ms,
    source_profile_id,
    created_by_run_id,
    from_visit,
    is_known_to_sync,
    visited_link_id,
    external_referrer_url,
    app_id,
    event_fingerprint,
    payload_hash,
    recorded_at,
    import_batch_id
  )
  VALUES (
    NEW.id,
    (
      SELECT id
      FROM urls
      WHERE source_profile_id = (SELECT id FROM source_profiles WHERE profile_key = NEW.profile_id)
        AND source_url_id = NEW.source_url_id
    ),
    CAST(NEW.source_visit_id AS TEXT),
    CAST((NEW.visit_time - 11644473600000000) / 1000 AS INTEGER),
    COALESCE(NEW.recorded_at, CURRENT_TIMESTAMP),
    NEW.transition,
    NEW.visit_duration,
    (SELECT id FROM source_profiles WHERE profile_key = NEW.profile_id),
    0,
    NEW.from_visit,
    COALESCE(NEW.is_known_to_sync, 0),
    NEW.visited_link_id,
    NEW.external_referrer_url,
    NEW.app_id,
    NEW.event_fingerprint,
    COALESCE(NEW.payload_hash, NEW.event_fingerprint, 'legacy-view'),
    COALESCE(NEW.recorded_at, CURRENT_TIMESTAMP),
    NEW.import_batch_id
  );
END;

CREATE TRIGGER visit_events_delete
INSTEAD OF DELETE ON visit_events
BEGIN
  DELETE FROM visits WHERE id = OLD.id;
END;
"#;

#[derive(Clone, Copy)]
struct MigrationSpec<'a> {
    version: i64,
    sql: &'a str,
}

const MIGRATIONS: &[MigrationSpec<'static>] = &[
    MigrationSpec { version: 1, sql: MIGRATION_001_INITIAL_SQL },
    MigrationSpec { version: 2, sql: MIGRATION_002_RUNTIME_SQL },
];

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
    run_migrations(connection)?;
    ensure_import_batch_schema(connection)?;
    backfill_runtime_columns(connection)?;
    install_legacy_views(connection)?;
    ensure_ai_schema(connection)?;
    ensure_insight_schema(connection)?;
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

fn backfill_runtime_columns(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
UPDATE source_profiles
SET profile_key = COALESCE(
      NULLIF(profile_key, ''),
      browser_kind || ':' || profile_name
    ),
    updated_at = COALESCE(updated_at, discovered_at);

UPDATE raw_row_versions
SET schema_hash = COALESCE(schema_hash, schema_fingerprint),
    chrome_version = COALESCE(chrome_version, browser_version),
    profile_id = COALESCE(
      profile_id,
      (
        SELECT profile_key
        FROM source_profiles
        WHERE source_profiles.id = raw_row_versions.source_profile_id
      )
    )
WHERE schema_hash IS NULL
   OR chrome_version IS NULL
   OR profile_id IS NULL;

UPDATE search_terms
SET profile_id = COALESCE(
      profile_id,
      (
        SELECT profile_key
        FROM source_profiles
        WHERE source_profiles.id = search_terms.source_profile_id
      )
    )
WHERE profile_id IS NULL;
"#,
    )?;
    Ok(())
}

fn install_legacy_views(connection: &Connection) -> Result<()> {
    create_or_replace_view(connection, "profiles", LEGACY_PROFILES_VIEW_SQL)?;
    create_or_replace_view(connection, "visit_events", LEGACY_VISIT_EVENTS_VIEW_SQL)?;
    connection.execute_batch(
        "DROP TRIGGER IF EXISTS profiles_insert;
         DROP TRIGGER IF EXISTS visit_events_insert;
         DROP TRIGGER IF EXISTS visit_events_delete;",
    )?;
    connection.execute_batch(LEGACY_VIEW_TRIGGER_SQL)?;
    Ok(())
}

fn create_or_replace_view(connection: &Connection, name: &str, sql: &str) -> Result<()> {
    match object_type(connection, name)? {
        Some(kind) if kind == "table" => return Ok(()),
        Some(kind) if kind == "view" => {
            connection.execute_batch(&format!("DROP VIEW IF EXISTS {name};"))?;
        }
        _ => {}
    }
    connection.execute_batch(sql)?;
    Ok(())
}

fn object_type(connection: &Connection, object_name: &str) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT type
             FROM sqlite_master
             WHERE name = ?1
             LIMIT 1",
            [object_name],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
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

    fn has_table(connection: &Connection, table_name: &str) -> bool {
        table_exists(connection, table_name).expect("table check")
    }

    #[test]
    fn migration_from_scratch_succeeds() {
        let connection = Connection::open_in_memory().expect("memory db");

        create_schema(&connection).expect("create schema");

        assert_eq!(current_version(&connection).expect("schema version"), 2);
        assert!(has_table(&connection, "runs"));
        assert!(has_table(&connection, "source_profiles"));
        assert!(has_table(&connection, "raw_row_versions"));
        assert!(has_table(&connection, "profile_watermarks"));
        assert!(has_table(&connection, "import_batches"));
        assert_eq!(
            object_type(&connection, "visit_events").expect("view type"),
            Some("view".to_string())
        );
    }

    #[test]
    fn migration_is_idempotent() {
        let connection = Connection::open_in_memory().expect("memory db");

        create_schema(&connection).expect("first migration");
        create_schema(&connection).expect("second migration");

        let count = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get::<_, i64>(0))
            .expect("migration count");
        assert_eq!(count, 2);
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
        assert_eq!(current_version(&connection).expect("migrated version"), 2);
    }
}
