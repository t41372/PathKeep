//! Failure modes: dependency features reorder visit/schema fingerprint JSON and break deduplication.
//! Regression tests for the canonical archive domain.
use super::*;
use crate::{
    config::{ProjectPaths, project_paths_with_root},
    utils::{restore_test_env_var, test_env_lock},
};
use browser_history_parser::{ContextEvidence, NativeEntity, TypedEvidenceBatch};
use rusqlite::{Connection, params};
use std::collections::BTreeMap;
use tempfile::tempdir;

fn sample_paths(root: &Path) -> ProjectPaths {
    project_paths_with_root(root)
}

pub(super) fn seed_chrome_fixture(root: &Path) -> PathBuf {
    let chrome_root = root.join("chrome-user-data");
    let profile_dir = chrome_root.join("Default");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    fs::write(chrome_root.join("Last Version"), "146.0.0.0").expect("write version");
    fs::write(
        chrome_root.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"tim@example.com"}}}}"#,
    )
    .expect("write local state");

    let history = Connection::open(profile_dir.join("History")).expect("open history");
    history
        .execute_batch(
            "CREATE TABLE urls (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               title TEXT,
               visit_count INTEGER NOT NULL,
               typed_count INTEGER NOT NULL,
               last_visit_time INTEGER NOT NULL,
               hidden INTEGER NOT NULL
             );
             CREATE TABLE visits (
               id INTEGER PRIMARY KEY,
               url INTEGER NOT NULL,
               visit_time INTEGER NOT NULL,
               from_visit INTEGER,
               transition INTEGER,
               visit_duration INTEGER,
               is_known_to_sync INTEGER,
               visited_link_id INTEGER,
               external_referrer_url TEXT,
               app_id TEXT
             );
             CREATE TABLE downloads (
               id INTEGER PRIMARY KEY,
               guid TEXT,
               current_path TEXT,
               target_path TEXT,
               start_time INTEGER,
               received_bytes INTEGER,
               total_bytes INTEGER,
               state INTEGER,
               mime_type TEXT,
               original_mime_type TEXT
             );
             CREATE TABLE keyword_search_terms (
               keyword_id INTEGER,
               url_id INTEGER,
               term TEXT,
               normalized_term TEXT
             );",
        )
        .expect("create history tables");
    let first_visit = crate::utils::iso_to_chrome_time_micros("2026-04-05T10:00:00+00:00")
        .expect("first visit time");
    let second_visit = crate::utils::iso_to_chrome_time_micros("2026-04-05T11:00:00+00:00")
        .expect("second visit time");
    history
        .execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
             VALUES (1, 'https://example.com/archive', 'Archive docs', 2, 0, ?1, 0)",
            [second_visit],
        )
        .expect("insert url");
    history
        .execute(
            "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
             VALUES
             (1, 1, ?1, NULL, 805306368, 24000, 1, NULL, 'https://google.com', NULL),
             (2, 1, ?2, 1, 805306368, 12000, 1, NULL, NULL, NULL)",
            params![first_visit, second_visit],
        )
        .expect("insert visits");
    history
        .execute(
            "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
             VALUES (9, 'guid-9', '/tmp/archive.pdf', '/tmp/archive.pdf', ?1, 10, 10, 1, 'application/pdf', 'application/pdf')",
            [second_visit],
        )
        .expect("insert download");
    history
        .execute(
            "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
             VALUES (1, 1, 'deep recall token', 'deep recall token')",
            [],
        )
        .expect("insert search term");

    let favicons = Connection::open(profile_dir.join("Favicons")).expect("open favicons");
    favicons
        .execute_batch(
            "CREATE TABLE favicons (id INTEGER PRIMARY KEY, url TEXT NOT NULL, icon_type INTEGER);
             CREATE TABLE icon_mapping (page_url TEXT NOT NULL, icon_id INTEGER NOT NULL);
             CREATE TABLE favicon_bitmaps (icon_id INTEGER NOT NULL, width INTEGER, height INTEGER, last_updated INTEGER, image_data BLOB);",
        )
        .expect("create favicons tables");
    favicons
        .execute(
            "INSERT INTO favicons (id, url, icon_type) VALUES (1, 'https://example.com/favicon.ico', 1)",
            [],
        )
        .expect("insert favicon");
    favicons
        .execute(
            "INSERT INTO favicons (id, url, icon_type) VALUES (2, 'https://example.com/app-icon.ico', 1)",
            [],
        )
        .expect("insert duplicate favicon");
    favicons
        .execute(
            "INSERT INTO icon_mapping (page_url, icon_id) VALUES ('https://example.com/archive', 1)",
            [],
        )
        .expect("insert icon mapping");
    favicons
        .execute(
            "INSERT INTO icon_mapping (page_url, icon_id) VALUES ('https://example.com/archive', 2)",
            [],
        )
        .expect("insert duplicate icon mapping");
    favicons
        .execute(
            "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
             VALUES (1, 16, 16, ?1, X'89504E470D0A1A0A01')",
            [second_visit],
        )
        .expect("insert favicon bitmap");
    favicons
        .execute(
            "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
             VALUES (2, 16, 16, ?1, X'89504E470D0A1A0A01')",
            [second_visit],
        )
        .expect("insert duplicate favicon bitmap");

    chrome_root
}

#[cfg(unix)]
#[test]
fn backup_keeps_readable_profiles_when_safari_staging_loses_access() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let safari_root = dir.path().join("Safari");
    fs::create_dir_all(&safari_root).expect("create safari root");
    let safari_history = safari_root.join("History.db");
    fs::create_dir(&safari_history).expect("create unreadable staging source");
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string(), "safari:default".to_string()],
        ..AppConfig::default()
    };

    crate::config::ensure_paths(&paths).expect("paths");
    let report = run_backup(&vault.with_config(config.clone()), false).expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.run.as_ref().expect("run").status, "partial");
    assert_eq!(report.profiles.len(), 2);
    assert_eq!(report.profiles[0].profile_id, "chrome:Default");
    assert!(matches!(&report.profiles[1].outcome,
        crate::models::ProfileOutcome::Unavailable { code } if code == "source-unreadable"));
    assert!(
        report
            .warning_details
            .iter()
            .any(|warning| warning.profile_id.as_deref() == Some("safari:default")
                && warning.code == "source-unreadable")
    );

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|entry| entry.starts_with("chrome:")));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn source_evidence_keeps_large_payloads_without_plaintext_spool_files() {
    let vault = crate::Vault::open_temp_with_key(Some([81; 32])).unwrap();
    let large = "x".repeat(300_000);
    let payload = SourceEvidencePayload {
        typed_evidence: TypedEvidenceBatch {
            context: vec![ContextEvidence {
                source_visit_id: Some(1),
                source_url_id: Some(1),
                context_key: "large".into(),
                value_json: large.clone(),
                source_field: "payload".into(),
            }],
            ..Default::default()
        },
        native_entities: vec![NativeEntity {
            entity_kind: "large".into(),
            native_primary_key: "1".into(),
            parent_native_primary_key: None,
            payload_json: large.clone(),
            metadata: BTreeMap::new(),
        }],
    };
    let mut writer = super::source_evidence::EvidenceWriter::new(&vault, 1, 1);
    writer.push(payload).unwrap();
    let restored: String =
        crate::test_support::store::Probe::for_db(&vault, vault_store::Db::SourceEvidence)
            .query_row("SELECT payload_json FROM native_entities", (), |r| Ok(r.get(0)?))
            .unwrap();
    assert_eq!(restored, large);
    assert!(!vault.paths().staging_dir.join("source-evidence-spool").exists());
}

#[test]
fn visit_event_fingerprint_is_stable() {
    let fingerprint = visit_event_fingerprint(
        "chromium-history",
        "https://example.com",
        1,
        Some("Title"),
        Some(805306368),
        None,
    );
    assert_eq!(fingerprint, "da53df0772e36b09afd187a0454da559fe451c828a40353f4e5c7514d17ecc59");
}

/// Source schema bytes feed a persisted fingerprint and must retain the old sorted encoding.
#[test]
fn browser_schema_payload_keeps_the_existing_sorted_json_encoding() {
    let root = tempdir().unwrap();
    let path = root.path().join("source.sqlite");
    let source = Connection::open(&path).unwrap();
    source.execute_batch("CREATE TABLE z (id INTEGER PRIMARY KEY)").unwrap();
    drop(source);
    let payload = super::browser_schema::collect_schema_payload(&path).unwrap();
    assert_eq!(
        payload.to_string(),
        r#"[{"name":"z","sql":"CREATE TABLE z (id INTEGER PRIMARY KEY)","tableName":"z","type":"table"}]"#
    );
}
