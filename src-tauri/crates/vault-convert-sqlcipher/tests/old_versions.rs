//! Responsibilities: archives written by older releases convert without loss. The old app
//! migrated an archive forward each time it opened it; someone who skipped versions arrives
//! here with the schema their last version left, and only the converter reads it now.
//! Not responsible for: the newest old schema (conversion.rs) or encryption (snapshot.rs).
//! Failure modes: a table or column added after that release is assumed to exist, rows of an
//! older shape are dropped, an archive that skipped versions can't be opened at all.
use rusqlite::{Connection, params};
use vault_convert_sqlcipher::{assess, convert, test_support::ARCHIVE_MIGRATIONS};
use vault_core::{ArchiveMode, ProjectPaths, project_paths_with_root};
use vault_store::{CancelToken, Db};

/// A v0.2.0 `config.json`: `remoteBackup` present (removed later), `ogImage` absent (added later).
const V020_CONFIG_JSON: &str = r#"{
  "initialized": true,
  "archiveMode": "Plaintext",
  "preferredLanguage": "system",
  "dueAfterHours": 72.0,
  "scheduleCheckIntervalHours": 6,
  "checkpointDays": 90,
  "captureFavicons": true,
  "selectedProfileIds": ["chrome:Default"],
  "gitEnabled": false,
  "rememberDatabaseKeyInKeyring": false,
  "appAutostart": false,
  "explorerBackgroundPrefetchPages": 5,
  "appLock": { "enabled": false, "idleTimeoutMinutes": 5, "biometricEnabled": false, "passcodeEnabled": true, "passcodeConfigured": false, "recoveryHint": null },
  "remoteBackup": { "enabled": false, "bucket": "", "region": "us-east-1", "endpoint": null, "prefix": "pathkeep", "pathStyle": true, "uploadAfterBackup": false, "credentialsSaved": false, "lastUploadedAt": null, "lastUploadedObjectKey": null, "lastError": null },
  "enrichment": { "plugins": [] },
  "deterministic": { "modules": [] },
  "ai": { "enabled": false }
}"#;

/// A v0.2.0 archive as that release left it: schema 001–010 and its migration ledger, with
/// three pages, two visits and a favicon.
fn v020_archive(paths: &ProjectPaths) {
    vault_core::config::ensure_paths(paths).unwrap();
    let db = Connection::open(&paths.legacy.archive).unwrap();
    db.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    // v0.2.0 shipped migrations 001–010.
    for (index, sql) in ARCHIVE_MIGRATIONS[..10].iter().enumerate() {
        db.execute_batch(sql).unwrap();
        db.execute(
            "INSERT INTO schema_migrations(version, applied_at, checksum, backup_path) VALUES (?1, '2026-04-24T00:00:00Z', 'v0.2.0', NULL)",
            params![index as i64 + 1],
        )
        .unwrap();
    }
    db.execute_batch(
        r#"INSERT INTO runs (id, run_type, trigger, started_at, finished_at, status)
             VALUES (1, 'backup', 'manual', '2026-04-24T00:00:00Z', '2026-04-24T00:00:05Z', 'success');
           INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path, discovered_at)
             VALUES (1, 'chrome', 'Default', '/tmp/Default', '2026-04-24T00:00:00Z');
           INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms,
               first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
             VALUES (1, 'https://docs.news.bbc.co.uk/story', 'BBC', 1, 0, 1, '', 1, '', 1, 1),
                    (2, 'https://www.example.com/page', 'Example', 1, 0, 1, '', 1, '', 1, 1),
                    (3, 'about:blank', NULL, 1, 0, 1, '', 1, '', 1, 1);
           INSERT INTO visits (url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id)
             VALUES (1, 1, '2026-04-24T00:00:00Z', 1, 1),
                    (2, 2, '2026-04-24T00:00:01Z', 1, 1);
           INSERT INTO favicons (page_url, icon_url, source_profile_id, created_by_run_id)
             VALUES ('https://www.example.com/page', 'https://www.example.com/icon.png', 1, 1);
           PRAGMA wal_checkpoint(TRUNCATE);"#,
    )
    .unwrap();
    std::fs::write(&paths.config_path, V020_CONFIG_JSON).unwrap();
}

fn count(vault: &vault_core::Vault, sql: &'static str) -> i64 {
    vault
        .store()
        .read(Db::Archive, move |s| s.query_one(sql, (), |r| Ok(r.get::<i64>(0)?)))
        .unwrap()
}

#[test]
fn a_v020_archive_converts_without_loss() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(dir.path());
    v020_archive(&paths);
    let config = vault_core::load_config(&paths).expect("a v0.2.0 config loads");
    assert_eq!(config.archive_mode, ArchiveMode::Plaintext);

    let assessment = assess(&paths, &config, None).unwrap();
    assert!(assessment.required);
    let original = std::fs::read(&paths.legacy.archive).unwrap();

    let converted = convert(&paths, &config, None, CancelToken::new(), |_| {})
        .expect("a v0.2.0 archive converts");
    let vault = &converted.vault;
    assert_eq!(count(vault, "SELECT count(*) FROM urls"), 3);
    assert_eq!(count(vault, "SELECT count(*) FROM visits WHERE reverted_at IS NULL"), 2);
    assert_eq!(count(vault, "SELECT count(*) FROM favicons"), 1);
    let titles = vault
        .store()
        .read(Db::Archive, |s| {
            s.query("SELECT url FROM urls ORDER BY id", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    assert_eq!(
        titles,
        ["https://docs.news.bbc.co.uk/story", "https://www.example.com/page", "about:blank"]
    );
    assert_eq!(std::fs::read(&paths.legacy.archive).unwrap(), original, "kept as it was");
}

#[test]
fn the_config_of_a_skipped_version_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(dir.path());
    vault_core::config::ensure_paths(&paths).unwrap();
    std::fs::write(&paths.config_path, V020_CONFIG_JSON).unwrap();
    let config = vault_core::load_config(&paths).expect("v0.2.0 config loads");
    assert!(config.initialized);
    assert!(config.capture_favicons);
    assert_eq!(config.selected_profile_ids, vec!["chrome:Default".to_string()]);
    assert!(config.og_image.fetch_enabled, "a setting added later takes its default");
}
