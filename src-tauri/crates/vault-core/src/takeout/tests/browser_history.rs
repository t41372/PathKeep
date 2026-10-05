//! Failure modes: family/source detection drift, duplicate reimports, lost
//! evidence, missing timestamps, invalid files. Tests use the real importer.
//! Browser Direct import regressions for local history databases.

use super::sample_paths;
use crate::test_support::store::Probe;
use crate::{
    config::ensure_paths,
    models::BrowserHistoryImportRequest,
    takeout::{
        import_browser_history, import_browser_history_with_progress, inspect_browser_history,
        restore_import_batch, revert_import_batch,
    },
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

fn atlas_browser_history_request(source: &Path, dry_run: bool) -> BrowserHistoryImportRequest {
    BrowserHistoryImportRequest {
        source_path: source.display().to_string(),
        dry_run,
        browser_family: Some("chromium".to_string()),
        profile_id: Some("atlas:user-test".to_string()),
        browser_name: Some("ChatGPT Atlas".to_string()),
        profile_name: Some("Atlas Work".to_string()),
    }
}

fn comet_browser_history_request(source: &Path, dry_run: bool) -> BrowserHistoryImportRequest {
    BrowserHistoryImportRequest {
        source_path: source.display().to_string(),
        dry_run,
        browser_family: Some("chromium".to_string()),
        profile_id: Some("comet:Default".to_string()),
        browser_name: Some("Perplexity Comet".to_string()),
        profile_name: Some("Default".to_string()),
    }
}

fn edge_browser_history_request(source: &Path, dry_run: bool) -> BrowserHistoryImportRequest {
    BrowserHistoryImportRequest {
        source_path: source.display().to_string(),
        dry_run,
        browser_family: Some("chromium".to_string()),
        profile_id: Some("edge:Default".to_string()),
        browser_name: Some("Microsoft Edge".to_string()),
        profile_name: Some("Default".to_string()),
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

fn write_chromium_history_db(dir: &Path) -> PathBuf {
    let source = dir.join("History");
    let connection = Connection::open(&source).expect("open chrome db");
    connection
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
             );",
        )
        .expect("create chrome db schema");
    let chrome_time = 13_358_534_400_000_000_i64;
    connection
        .execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
             VALUES (?1, ?2, ?3, 1, 0, ?4, 0)",
            params![1_i64, "https://example.com/chrome", "Chrome", chrome_time],
        )
        .expect("insert chrome url");
    connection
        .execute(
            "INSERT INTO visits (
               id, url, visit_time, from_visit, transition, visit_duration,
               is_known_to_sync, visited_link_id, external_referrer_url, app_id
             )
             VALUES (?1, ?2, ?3, NULL, 1, 1000, 0, NULL, NULL, ?4)",
            params![7_i64, 1_i64, chrome_time, "chrome"],
        )
        .expect("insert chrome visit");
    source
}

fn write_firefox_history_db(dir: &Path) -> PathBuf {
    let source = dir.join("places.sqlite");
    let connection = Connection::open(&source).expect("open firefox db");
    connection
        .execute_batch(
            "CREATE TABLE moz_places (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL,
               title TEXT,
               visit_count INTEGER,
               hidden INTEGER,
               last_visit_date INTEGER
             );
             CREATE TABLE moz_historyvisits (
               id INTEGER PRIMARY KEY,
               place_id INTEGER NOT NULL,
               visit_date INTEGER NOT NULL,
               from_visit INTEGER,
               visit_type INTEGER
             );",
        )
        .expect("create firefox db schema");
    let firefox_time = 1_744_146_000_000_000_i64;
    connection
        .execute(
            "INSERT INTO moz_places (
               id, url, title, visit_count, hidden, last_visit_date
             )
             VALUES (?1, ?2, ?3, 2, 0, ?4), (?5, ?6, ?7, 1, 0, ?8)",
            params![
                1_i64,
                "https://example.com/firefox-one",
                "Firefox One",
                firefox_time,
                2_i64,
                "https://example.com/firefox-two",
                "Firefox Two",
                firefox_time + 1_000_000
            ],
        )
        .expect("insert firefox urls");
    connection
        .execute(
            "INSERT INTO moz_historyvisits (
               id, place_id, visit_date, from_visit, visit_type
             )
             VALUES (?1, ?2, ?3, NULL, 1), (?4, ?5, ?6, ?7, 2)",
            params![9_i64, 1_i64, firefox_time, 10_i64, 2_i64, firefox_time + 1_000_000, 9_i64],
        )
        .expect("insert firefox visits");
    source
}

#[test]
fn inspect_browser_history_previews_safari_database() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let fixture_path = write_safari_history_db(dir.path());
    let inspection = inspect_browser_history(
        &vault,
        &browser_history_request(&fixture_path, true, "safari", "safari:reference"),
    )
    .expect("inspect safari browser history");

    assert_eq!(inspection.source_path, fixture_path.display().to_string());
    assert!(inspection.dry_run);
    assert_eq!(inspection.candidate_items, 2);
    assert_eq!(inspection.preview_entries.len(), 2);
    assert_eq!(inspection.recognized_files.len(), 1);
    assert_eq!(inspection.recognized_files[0].kind, "safari-history-db");
    assert_eq!(inspection.recognized_files[0].records, 2);
    assert!(inspection.preview_range_start.is_some());
    assert!(inspection.notes.iter().any(|note| note.contains("Safari baseline ingest")));
}

#[test]
fn import_browser_history_safari_import_batch_is_reversible_and_deduplicated() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let source = write_safari_history_db(dir.path());
    let request = browser_history_request(&source, false, "safari", "safari:primary");
    let first = import_browser_history(&vault, &request).expect("import safari");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "safari:primary");
    assert_eq!(first.imported_items, 2);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].kind, "safari-history-db");
    assert_eq!(first.preview_entries.len(), 2);

    let second = import_browser_history(&vault, &request).expect("re-import safari");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 2);

    let archive = Probe::new(&vault);
    let profile_family: String = archive
        .query_row(
            "SELECT browser_family FROM source_profiles WHERE profile_key = 'safari:primary'",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("load safari profile");
    assert_eq!(profile_family, "safari");

    let source_evidence = Probe::for_db(&vault, Db::SourceEvidence);
    let navigation_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM visit_navigation_evidence", (), |row| Ok(row.get(0)?))
        .expect("navigation evidence count");
    let native_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM native_entities", (), |row| Ok(row.get(0)?))
        .expect("native evidence count");
    assert_eq!(navigation_rows, 4);
    assert!(native_rows >= 4);

    let reverted = revert_import_batch(&vault, batch.id).expect("revert batch");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&vault, batch.id).expect("restore batch");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 2);
}

#[test]
fn import_browser_history_accepts_chromium_history_database() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let source = write_chromium_history_db(dir.path());
    let request = browser_history_request(&source, false, "chromium", "chrome:Primary");
    let inspection = import_browser_history(&vault, &request).expect("import chromium");

    assert_eq!(inspection.imported_items, 1);
    assert_eq!(inspection.duplicate_items, 0);
    assert_eq!(inspection.recognized_files[0].kind, "chromium-history-db");
    assert_eq!(inspection.import_batch.expect("import batch").source_kind, "browser-history");

    let search = Probe::new(&vault);
    let search_documents: i64 = search
        .query_row("SELECT COUNT(*) FROM search_documents", (), |row| Ok(row.get(0)?))
        .expect("search document count");
    let chrome_matches: i64 = search
        .query_row(
            "SELECT COUNT(*) FROM search_documents WHERE normalized_title LIKE ?1",
            ["%chrome%"],
            |row| Ok(row.get(0)?),
        )
        .expect("search document normalized title");
    assert_eq!(search_documents, 1);
    assert_eq!(chrome_matches, 1);
}

#[test]
fn import_browser_history_preserves_microsoft_edge_product_metadata() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let source = write_chromium_history_db(dir.path());
    let request = edge_browser_history_request(&source, false);
    let first = import_browser_history(&vault, &request).expect("import edge");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "edge:Default");
    assert_eq!(first.imported_items, 1);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].kind, "chromium-history-db");

    let archive = Probe::new(&vault);
    let (profile_family, profile_product): (String, String) = archive
        .query_row(
            "SELECT browser_family, browser_product
               FROM source_profiles
              WHERE profile_key = 'edge:Default'",
            (),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load edge profile");
    assert_eq!(profile_family, "chromium");
    assert_eq!(profile_product, "Microsoft Edge");
    drop(archive);

    let second = import_browser_history(&vault, &request).expect("re-import edge");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 1);

    let reverted = revert_import_batch(&vault, batch.id).expect("revert edge");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&vault, batch.id).expect("restore edge");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 1);
}

#[test]
fn import_browser_history_accepts_firefox_places_database_and_review_contract() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let profile_dir = dir.path().join("FirefoxProfile");
    std::fs::create_dir_all(&profile_dir).expect("profile dir");
    let source = write_firefox_history_db(&profile_dir);
    let request = browser_history_request(&profile_dir, false, "firefox", "firefox:Primary");
    let first = import_browser_history(&vault, &request).expect("import firefox");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "firefox:Primary");
    assert_eq!(first.imported_items, 2);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].path, source.display().to_string());
    assert_eq!(first.recognized_files[0].kind, "firefox-places-db");
    assert_eq!(first.recognized_files[0].reason_code.as_deref(), Some("firefox-history-sqlite"));
    assert!(
        first
            .notes
            .iter()
            .any(|note| note.contains("Firefox baseline ingest captures visits and URLs"))
    );

    let archive = Probe::new(&vault);
    let (profile_family, profile_product): (String, String) = archive
        .query_row(
            "SELECT browser_family, browser_product
               FROM source_profiles
              WHERE profile_key = 'firefox:Primary'",
            (),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load firefox profile");
    assert_eq!(profile_family, "firefox");
    assert_eq!(profile_product, "Firefox");
    drop(archive);

    let source_evidence = Probe::for_db(&vault, Db::SourceEvidence);
    let source_batches: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM source_batches", (), |row| Ok(row.get(0)?))
        .expect("source batch count");
    let native_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM native_entities", (), |row| Ok(row.get(0)?))
        .expect("native evidence count");
    assert_eq!(source_batches, 1);
    assert!(native_rows >= 2);

    let second = import_browser_history(&vault, &request).expect("re-import firefox");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 2);

    let search = Probe::new(&vault);
    let firefox_matches: i64 = search
        .query_row(
            "SELECT COUNT(*) FROM search_documents WHERE normalized_title LIKE ?1",
            ["%firefox%"],
            |row| Ok(row.get(0)?),
        )
        .expect("fts firefox match count");
    assert_eq!(firefox_matches, 2);

    let reverted = revert_import_batch(&vault, batch.id).expect("revert firefox");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&vault, batch.id).expect("restore firefox");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 2);
}

#[test]
fn import_browser_history_progress_reports_record_batches() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let source = write_chromium_history_db(dir.path());
    let request = browser_history_request(&source, false, "chromium", "chrome:Primary");
    let mut events = Vec::new();
    let inspection =
        import_browser_history_with_progress(&vault, &request, |event| events.push(event))
            .expect("import chromium with progress");

    assert_eq!(inspection.imported_items, 1);
    let record_event = events
        .iter()
        .find(|event| event.phase == "import-file" && event.processed_records == Some(1))
        .expect("browser-direct record progress event");
    assert_eq!(record_event.total_records, None);
    assert_eq!(record_event.imported_records, Some(1));
    assert_eq!(record_event.duplicate_records, Some(0));
    assert_eq!(record_event.source_label.as_deref(), Some("Google Chrome / Primary"));
}

#[test]
fn import_browser_history_preserves_chatgpt_atlas_product_and_review_contract() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let source = write_chromium_history_db(dir.path());
    let request = atlas_browser_history_request(&source, false);
    let first = import_browser_history(&vault, &request).expect("import atlas");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "atlas:user-test");
    assert_eq!(first.imported_items, 1);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].kind, "chromium-history-db");

    let archive = Probe::new(&vault);
    let (profile_family, profile_product): (String, String) = archive
        .query_row(
            "SELECT browser_family, browser_product
               FROM source_profiles
              WHERE profile_key = 'atlas:user-test'",
            (),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load atlas profile");
    assert_eq!(profile_family, "chromium");
    assert_eq!(profile_product, "ChatGPT Atlas");
    drop(archive);

    let source_evidence = Probe::for_db(&vault, Db::SourceEvidence);
    let source_batches: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM source_batches", (), |row| Ok(row.get(0)?))
        .expect("source batch count");
    let native_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM native_entities", (), |row| Ok(row.get(0)?))
        .expect("native evidence count");
    assert_eq!(source_batches, 1);
    assert!(native_rows >= 2);

    let second = import_browser_history(&vault, &request).expect("re-import atlas");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 1);

    let reverted = revert_import_batch(&vault, batch.id).expect("revert atlas");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&vault, batch.id).expect("restore atlas");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 1);
}

#[test]
fn import_browser_history_preserves_perplexity_comet_product_and_review_contract() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let source = write_chromium_history_db(dir.path());
    let request = comet_browser_history_request(&source, false);
    let first = import_browser_history(&vault, &request).expect("import comet");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "comet:Default");
    assert_eq!(first.imported_items, 1);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].kind, "chromium-history-db");

    let archive = Probe::new(&vault);
    let (profile_family, profile_product): (String, String) = archive
        .query_row(
            "SELECT browser_family, browser_product
               FROM source_profiles
              WHERE profile_key = 'comet:Default'",
            (),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load comet profile");
    assert_eq!(profile_family, "chromium");
    assert_eq!(profile_product, "Perplexity Comet");
    drop(archive);

    let source_evidence = Probe::for_db(&vault, Db::SourceEvidence);
    let source_batches: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM source_batches", (), |row| Ok(row.get(0)?))
        .expect("source batch count");
    let native_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM native_entities", (), |row| Ok(row.get(0)?))
        .expect("native evidence count");
    assert_eq!(source_batches, 1);
    assert!(native_rows >= 2);

    let second = import_browser_history(&vault, &request).expect("re-import comet");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 1);

    let reverted = revert_import_batch(&vault, batch.id).expect("revert comet");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&vault, batch.id).expect("restore comet");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 1);
}

#[test]
fn inspect_browser_history_reports_safari_access_guidance_for_unreadable_files() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("History.db");
    let error = inspect_browser_history(
        &vault,
        &browser_history_request(&source, true, "safari", "safari:blocked"),
    )
    .expect_err("missing safari db should explain access");

    assert!(format!("{error:#}").contains("Full Disk Access"));
}

/// The preview says how many visits an import would add: it checks the archive
/// for the same source profile, and the import then adds exactly that many.
#[test]
fn inspect_browser_history_counts_visits_the_archive_already_has() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let profile_dir = dir.path().join("FirefoxProfile");
    std::fs::create_dir_all(&profile_dir).expect("profile dir");
    let source = write_firefox_history_db(&profile_dir);
    let preview = browser_history_request(&source, true, "firefox", "firefox:Probe");
    let import = browser_history_request(&source, false, "firefox", "firefox:Probe");

    // Nothing archived yet: every visit is new.
    let fresh = inspect_browser_history(&vault, &preview).expect("fresh");
    assert_eq!((fresh.candidate_items, fresh.duplicate_items), (2, 0));
    assert!(fresh.preview_entries.iter().all(|entry| entry.status == "candidate"));
    import_browser_history(&vault, &import).expect("first import");

    // One more visit in the browser: the preview sees 1 new, 2 already archived.
    let connection = Connection::open(&source).expect("open firefox db");
    connection
        .execute(
            "INSERT INTO moz_historyvisits (id, place_id, visit_date, from_visit, visit_type)
             VALUES (3, 2, 1744146005000000, NULL, 1)",
            (),
        )
        .expect("append visit");
    drop(connection);
    let again = inspect_browser_history(&vault, &preview).expect("again");
    assert_eq!((again.candidate_items, again.duplicate_items), (3, 2));
    assert_eq!(again.preview_entries.iter().filter(|entry| entry.status == "duplicate").count(), 2);
    // Without the archive nothing is compared.
    let blind =
        inspect_browser_history(&crate::Vault::open_temp().unwrap(), &preview).expect("blind");
    assert_eq!(blind.duplicate_items, 0);

    let second = import_browser_history(&vault, &import).expect("second import");
    assert_eq!(second.imported_items, again.candidate_items - again.duplicate_items);
    assert_eq!(second.duplicate_items, again.duplicate_items);

    // Another profile id is another source: nothing counts as a duplicate.
    let other = browser_history_request(&source, true, "firefox", "firefox:Other");
    let other = inspect_browser_history(&vault, &other).expect("other");
    assert_eq!((other.candidate_items, other.duplicate_items), (3, 0));
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
                let batch = crate::load_import_batches(&vault).unwrap().remove(0);
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

#[test]
fn failed_browser_import_retains_recognized_files_and_failure_notes() {
    for fail_parsing in [true, false] {
        let vault = crate::Vault::open_temp().unwrap();
        let root = tempdir().unwrap();
        let source = write_chromium_history_db(root.path());
        if fail_parsing {
            Connection::open(&source)
                .unwrap()
                .execute("UPDATE visits SET visit_time='not an integer'", [])
                .unwrap();
        } else {
            vault
                .store()
                .write(Db::SourceEvidence, vault_store::Priority::Batch, |s| {
                    s.execute("DROP TABLE source_batches", ())
                })
                .unwrap();
        }
        let error = import_browser_history(
            &vault,
            &browser_history_request(&source, false, "chromium", "chrome:broken"),
        )
        .unwrap_err();
        let batch = crate::load_import_batches(&vault).unwrap().remove(0);
        let detail = crate::preview_import_batch(&vault, batch.id).unwrap();
        assert_eq!(detail.batch.status, "failed");
        assert_eq!(detail.recognized_files.len(), 1);
        assert_eq!(detail.recognized_files[0].path, source.display().to_string());
        assert!(!detail.notes.is_empty());
        assert!(
            detail.notes.iter().any(|note| note.contains(&error.to_string())),
            "{:?}: {error:#}",
            detail.notes
        );
    }
}
