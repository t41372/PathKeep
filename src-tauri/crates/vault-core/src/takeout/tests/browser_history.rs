//! Failure modes: family/source detection drift, duplicate reimports, lost
//! evidence, missing timestamps, invalid files. Tests use the real importer.
//! Browser Direct import regressions for local history databases.

use crate::test_support::store::Probe;
use crate::{
    models::BrowserHistoryImportRequest,
    takeout::{import_browser_history, import_browser_history_with_progress},
};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use vault_store::Db;

fn browser_history_request(
    source: &Path,
    dry_run: bool,
    browser_family: &str,
    profile_id: &str,
) -> BrowserHistoryImportRequest {
    BrowserHistoryImportRequest {
        source_path: source.display().to_string(),
        dry_run,
        browser_family: Some(browser_family.to_string()),
        profile_id: Some(profile_id.to_string()),
        browser_name: Some(
            match browser_family {
                "firefox" => "Firefox",
                "safari" => "Safari",
                _ => "Google Chrome",
            }
            .to_string(),
        ),
        profile_name: Some("Primary".to_string()),
    }
}

fn write_safari_history_db(dir: &Path) -> PathBuf {
    let source = dir.join("History.db");
    let connection = Connection::open(&source).expect("open safari db");
    connection
        .execute_batch(
            "CREATE TABLE history_items (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL
             );
             CREATE TABLE history_visits (
               id INTEGER PRIMARY KEY,
               history_item INTEGER NOT NULL,
               title TEXT,
               visit_time REAL NOT NULL,
               load_successful INTEGER,
               http_non_get INTEGER,
               synthesized INTEGER,
               redirect_source INTEGER,
               redirect_destination INTEGER,
               origin INTEGER,
               generation INTEGER,
               attributes INTEGER,
               score REAL
             );",
        )
        .expect("create safari db schema");
    connection
        .execute(
            "INSERT INTO history_items (id, url) VALUES (?1, ?2), (?3, ?4)",
            params![
                1_i64,
                "https://example.com/safari-one",
                2_i64,
                "https://example.com/safari-two"
            ],
        )
        .expect("insert safari urls");
    connection
        .execute(
            "INSERT INTO history_visits (
               id, history_item, title, visit_time, load_successful,
               http_non_get, synthesized, redirect_source, redirect_destination,
               origin, generation, attributes, score
             )
             VALUES (?1, ?2, ?3, ?4, 1, 0, 0, NULL, ?5, 1, 1, 0, ?6)",
            params![11_i64, 1_i64, "One", 765_838_800.0_f64, 12_i64, 0.8_f64],
        )
        .expect("insert safari visit one");
    connection
        .execute(
            "INSERT INTO history_visits (
               id, history_item, title, visit_time, load_successful,
               http_non_get, synthesized, redirect_source, redirect_destination,
               origin, generation, attributes, score
             )
             VALUES (?1, ?2, ?3, ?4, 1, 0, 0, ?5, NULL, 1, 1, 0, ?6)",
            params![12_i64, 2_i64, "Two", 765_838_900.0_f64, 11_i64, 0.6_f64],
        )
        .expect("insert safari visit two");
    source
}

#[test]
fn browser_import_retains_review_and_audit_without_advancing_live_cursors() {
    for key in [None, Some([86; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let root = tempdir().unwrap();
        let source = write_safari_history_db(root.path());
        let request = browser_history_request(&source, false, "safari", "safari:primary");
        vault.store().write(vault_store::Db::Archive,vault_store::Priority::Interactive,|s| {
            s.execute("INSERT INTO profile_watermarks(profile_id,last_visit_id,last_url_last_visit_time,last_download_id,last_favicon_last_updated,updated_at)
                VALUES('safari:primary',98,1234,55,12345,'kept')",())?;Ok(())
        }).unwrap();
        let mut saw_running = false;
        let imported = import_browser_history_with_progress(&vault, &request, |event| {
            if event.phase == "prepare" {
                let batch = crate::load_import_batches(&vault, None, 16).unwrap().items.remove(0);
                let detail = crate::preview_import_batch(&vault, batch.id).unwrap();
                assert_eq!(detail.batch.status, "running");
                assert_eq!(detail.recognized_files.len(), 1);
                assert!(
                    detail.batch.audit_path.is_none(),
                    "an in-flight preview must not freeze a partial audit artifact"
                );
                saw_running = true;
            }
        })
        .unwrap();
        assert!(saw_running);
        assert_eq!(
            vault
                .store()
                .read(Db::Archive, |s| s.query_one(
                    "SELECT value FROM archive_totals WHERE name='sourceProfiles'",
                    (),
                    |r| Ok(r.get::<i64>(0)?)
                ))
                .unwrap(),
            1
        );
        let batch = imported.import_batch.as_ref().unwrap();
        assert!(Path::new(batch.audit_path.as_ref().unwrap()).is_file());
        let detail = crate::preview_import_batch(&vault, batch.id).unwrap();
        assert_eq!(detail.batch.status, "imported");
        assert_eq!(detail.recognized_files.len(), 1);
        assert_eq!(detail.recognized_files[0].kind, "safari-history-db");
        assert_eq!(detail.recognized_files[0].path, source.display().to_string());
        assert_eq!(detail.quarantined_files.len(), imported.quarantined_files.len());
        assert_eq!(detail.notes, imported.notes);
        assert!(detail.preview_range_start.is_some());
        assert!(detail.preview_range_end.is_some());
        assert_eq!(detail.preview_range_start, imported.preview_range_start);
        assert_eq!(detail.preview_range_end, imported.preview_range_end);
        vault.store().read(vault_store::Db::Archive,|s| {
            assert_eq!(s.query_one("SELECT last_visit_id,last_url_last_visit_time,last_download_id,last_favicon_last_updated,last_source_batch_id,updated_at FROM profile_watermarks WHERE profile_id='safari:primary'",(),|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?,r.get::<i64>(3)?,r.get::<i64>(4)?,r.get::<String>(5)?)))?,(98,1234,55,12345,1,"kept".into()));
            assert_eq!(s.query_one("SELECT COUNT(*) FROM pages p WHERE NOT EXISTS(SELECT 1 FROM search_outbox o WHERE o.page_id=p.id)",(),|r|Ok(r.get::<i64>(0)?))?,0);
            Ok(())
        }).unwrap();
        vault.store().read(vault_store::Db::SourceEvidence,|s| {
            let (fingerprint,observation)=s.query_one("SELECT b.schema_fingerprint,o.observation_json FROM source_batches b JOIN schema_observations o ON o.source_batch_id=b.id",(),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?)))?;
            assert_eq!(fingerprint,crate::utils::sha256_hex(observation.as_bytes()));
            Ok(())
        }).unwrap();
    }
}

/// The records are in the archive when the import's review record is written. That step failing
/// made the import report a failure, while its run said it succeeded, and marked the file it had
/// read as unreadable (`WORK-RUN-LIFECYCLE-GUARDS`).
#[test]
fn an_import_whose_review_record_fails_is_still_imported_with_a_warning() {
    use std::os::unix::fs::PermissionsExt;
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let source = write_safari_history_db(dir.path());
    // Only the import's review records: the run's manifest, written before it ends, still lands.
    let audit = vault.paths().audit_repo_path.join("imports");
    std::fs::create_dir_all(&audit).unwrap();
    std::fs::set_permissions(&audit, std::fs::Permissions::from_mode(0o500)).unwrap();
    let request = browser_history_request(&source, false, "safari", "safari:primary");
    let imported = import_browser_history(&vault, &request);
    std::fs::set_permissions(&audit, std::fs::Permissions::from_mode(0o700)).unwrap();

    let imported = imported.expect("the import succeeds");
    assert_eq!(imported.imported_items, 2);
    let batch = imported.import_batch.expect("its batch");
    assert_eq!(batch.status, "imported");
    assert!(imported.recognized_files.iter().all(|file| file.kind == "safari-history-db"));
    let (status, warnings): (String, String) = Probe::new(&vault)
        .query_row(
            "SELECT status,warnings_json FROM runs WHERE run_type='import' ORDER BY id DESC LIMIT 1",
            (),
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "success");
    let warnings: Vec<serde_json::Value> = serde_json::from_str(&warnings).unwrap();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["code"], crate::archive::write_runs::REVIEW_RECORD_FAILED);
    assert!(warnings[0]["diagnostic"].as_str().is_some_and(|text| !text.is_empty()));
}
