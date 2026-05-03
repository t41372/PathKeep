//! Browser Direct import regressions for local history databases.

use super::{initialized_plaintext_config, sample_paths};
use crate::{
    archive::{create_schema, open_archive_connection, open_source_evidence_connection},
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
    let dir = tempdir().expect("tempdir");
    let fixture_path = write_safari_history_db(dir.path());
    let inspection = inspect_browser_history(
        &sample_paths(dir.path()),
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
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_safari_history_db(dir.path());
    let request = browser_history_request(&source, false, "safari", "safari:primary");
    let first = import_browser_history(&paths, &config, None, &request).expect("import safari");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "safari:primary");
    assert_eq!(first.imported_items, 2);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].kind, "safari-history-db");
    assert_eq!(first.preview_entries.len(), 2);

    let second = import_browser_history(&paths, &config, None, &request).expect("re-import safari");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 2);

    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    let profile_family: String = archive
        .query_row(
            "SELECT browser_family FROM source_profiles WHERE profile_key = 'safari:primary'",
            [],
            |row| row.get(0),
        )
        .expect("load safari profile");
    assert_eq!(profile_family, "safari");

    let source_evidence =
        open_source_evidence_connection(&paths, &config, None).expect("open source evidence");
    let navigation_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM visit_navigation_evidence", [], |row| row.get(0))
        .expect("navigation evidence count");
    let native_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM native_entities", [], |row| row.get(0))
        .expect("native evidence count");
    assert_eq!(navigation_rows, 4);
    assert!(native_rows >= 4);

    let reverted = revert_import_batch(&paths, &config, None, batch.id).expect("revert batch");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&paths, &config, None, batch.id).expect("restore batch");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 2);
}

#[test]
fn import_browser_history_accepts_chromium_history_database() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_chromium_history_db(dir.path());
    let request = browser_history_request(&source, false, "chromium", "chrome:Primary");
    let inspection =
        import_browser_history(&paths, &config, None, &request).expect("import chromium");

    assert_eq!(inspection.imported_items, 1);
    assert_eq!(inspection.duplicate_items, 0);
    assert_eq!(inspection.recognized_files[0].kind, "chromium-history-db");
    assert_eq!(inspection.import_batch.expect("import batch").source_kind, "browser-history");

    let search = Connection::open(&paths.search_database_path).expect("open search projection");
    let search_documents: i64 = search
        .query_row("SELECT COUNT(*) FROM search_documents", [], |row| row.get(0))
        .expect("search document count");
    let chrome_matches: i64 = search
        .query_row(
            "SELECT COUNT(*) FROM history_search WHERE history_search MATCH ?1",
            ["chrome"],
            |row| row.get(0),
        )
        .expect("fts chrome match count");
    assert_eq!(search_documents, 1);
    assert_eq!(chrome_matches, 1);
}

#[test]
fn import_browser_history_preserves_microsoft_edge_product_metadata() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_chromium_history_db(dir.path());
    let request = edge_browser_history_request(&source, false);
    let first = import_browser_history(&paths, &config, None, &request).expect("import edge");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "edge:Default");
    assert_eq!(first.imported_items, 1);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].kind, "chromium-history-db");

    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    let (profile_family, profile_product): (String, String) = archive
        .query_row(
            "SELECT browser_family, browser_product
               FROM source_profiles
              WHERE profile_key = 'edge:Default'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load edge profile");
    assert_eq!(profile_family, "chromium");
    assert_eq!(profile_product, "Microsoft Edge");
    drop(archive);

    let second = import_browser_history(&paths, &config, None, &request).expect("re-import edge");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 1);

    let reverted = revert_import_batch(&paths, &config, None, batch.id).expect("revert edge");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&paths, &config, None, batch.id).expect("restore edge");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 1);
}

#[test]
fn import_browser_history_accepts_firefox_places_database_and_review_contract() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let profile_dir = dir.path().join("FirefoxProfile");
    std::fs::create_dir_all(&profile_dir).expect("profile dir");
    let source = write_firefox_history_db(&profile_dir);
    let request = browser_history_request(&profile_dir, false, "firefox", "firefox:Primary");
    let first = import_browser_history(&paths, &config, None, &request).expect("import firefox");
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

    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    let (profile_family, profile_product): (String, String) = archive
        .query_row(
            "SELECT browser_family, browser_product
               FROM source_profiles
              WHERE profile_key = 'firefox:Primary'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load firefox profile");
    assert_eq!(profile_family, "firefox");
    assert_eq!(profile_product, "Firefox");
    drop(archive);

    let source_evidence =
        open_source_evidence_connection(&paths, &config, None).expect("open source evidence");
    let source_batches: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get(0))
        .expect("source batch count");
    let native_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM native_entities", [], |row| row.get(0))
        .expect("native evidence count");
    assert_eq!(source_batches, 1);
    assert!(native_rows >= 2);

    let second =
        import_browser_history(&paths, &config, None, &request).expect("re-import firefox");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 2);

    let search = Connection::open(&paths.search_database_path).expect("open search projection");
    let firefox_matches: i64 = search
        .query_row(
            "SELECT COUNT(*) FROM history_search WHERE history_search MATCH ?1",
            ["firefox"],
            |row| row.get(0),
        )
        .expect("fts firefox match count");
    assert_eq!(firefox_matches, 2);

    let reverted = revert_import_batch(&paths, &config, None, batch.id).expect("revert firefox");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&paths, &config, None, batch.id).expect("restore firefox");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 2);
}

#[test]
fn import_browser_history_progress_reports_record_batches() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_chromium_history_db(dir.path());
    let request = browser_history_request(&source, false, "chromium", "chrome:Primary");
    let mut events = Vec::new();
    let inspection =
        import_browser_history_with_progress(&paths, &config, None, &request, |event| {
            events.push(event)
        })
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
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_chromium_history_db(dir.path());
    let request = atlas_browser_history_request(&source, false);
    let first = import_browser_history(&paths, &config, None, &request).expect("import atlas");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "atlas:user-test");
    assert_eq!(first.imported_items, 1);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].kind, "chromium-history-db");

    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    let (profile_family, profile_product): (String, String) = archive
        .query_row(
            "SELECT browser_family, browser_product
               FROM source_profiles
              WHERE profile_key = 'atlas:user-test'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load atlas profile");
    assert_eq!(profile_family, "chromium");
    assert_eq!(profile_product, "ChatGPT Atlas");
    drop(archive);

    let source_evidence =
        open_source_evidence_connection(&paths, &config, None).expect("open source evidence");
    let source_batches: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get(0))
        .expect("source batch count");
    let native_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM native_entities", [], |row| row.get(0))
        .expect("native evidence count");
    assert_eq!(source_batches, 1);
    assert!(native_rows >= 2);

    let second = import_browser_history(&paths, &config, None, &request).expect("re-import atlas");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 1);

    let reverted = revert_import_batch(&paths, &config, None, batch.id).expect("revert atlas");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&paths, &config, None, batch.id).expect("restore atlas");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 1);
}

#[test]
fn import_browser_history_preserves_perplexity_comet_product_and_review_contract() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_chromium_history_db(dir.path());
    let request = comet_browser_history_request(&source, false);
    let first = import_browser_history(&paths, &config, None, &request).expect("import comet");
    let batch = first.import_batch.clone().expect("import batch");

    assert_eq!(batch.source_kind, "browser-history");
    assert_eq!(batch.profile_id, "comet:Default");
    assert_eq!(first.imported_items, 1);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(first.recognized_files[0].kind, "chromium-history-db");

    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    let (profile_family, profile_product): (String, String) = archive
        .query_row(
            "SELECT browser_family, browser_product
               FROM source_profiles
              WHERE profile_key = 'comet:Default'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("load comet profile");
    assert_eq!(profile_family, "chromium");
    assert_eq!(profile_product, "Perplexity Comet");
    drop(archive);

    let source_evidence =
        open_source_evidence_connection(&paths, &config, None).expect("open source evidence");
    let source_batches: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get(0))
        .expect("source batch count");
    let native_rows: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM native_entities", [], |row| row.get(0))
        .expect("native evidence count");
    assert_eq!(source_batches, 1);
    assert!(native_rows >= 2);

    let second = import_browser_history(&paths, &config, None, &request).expect("re-import comet");
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 1);

    let reverted = revert_import_batch(&paths, &config, None, batch.id).expect("revert comet");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    let restored = restore_import_batch(&paths, &config, None, batch.id).expect("restore comet");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 1);
}

#[test]
fn inspect_browser_history_reports_safari_access_guidance_for_unreadable_files() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("History.db");
    let error = inspect_browser_history(
        &sample_paths(dir.path()),
        &browser_history_request(&source, true, "safari", "safari:blocked"),
    )
    .expect_err("missing safari db should explain access");

    assert!(format!("{error:#}").contains("Full Disk Access"));
}
