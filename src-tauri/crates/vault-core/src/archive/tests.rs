//! Regression tests for the canonical archive domain.
use super::*;
use crate::{
    config::{ProjectPaths, project_paths_with_root},
    models::{ArchiveMode, RetentionPruneRequest, SnapshotRestoreRequest, TakeoutRequest},
    utils::{restore_test_env_var, test_env_lock},
};
use browser_history_parser::{ContextEvidence, NativeEntity, TypedEvidenceBatch};
use rusqlite::Connection;
use std::collections::BTreeMap;
use tempfile::tempdir;

fn sample_paths(root: &Path) -> ProjectPaths {
    project_paths_with_root(root)
}

fn seed_chrome_fixture(root: &Path) -> PathBuf {
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
            "INSERT INTO icon_mapping (page_url, icon_id) VALUES ('https://example.com/archive', 1)",
            [],
        )
        .expect("insert icon mapping");
    favicons
        .execute(
            "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
             VALUES (1, 16, 16, ?1, X'89504E470D0A1A0A01')",
            [second_visit],
        )
        .expect("insert favicon bitmap");

    chrome_root
}

fn seed_firefox_fixture(root: &Path) -> PathBuf {
    let firefox_root = root.join("firefox");
    let profiles_dir = firefox_root.join("Profiles");
    let profile_dir = profiles_dir.join("abcd.default-release");
    fs::create_dir_all(&profile_dir).expect("create firefox profile dir");
    fs::write(
        firefox_root.join("profiles.ini"),
        "[Profile0]\nName=Work Firefox\nPath=abcd.default-release\nIsRelative=1\n",
    )
    .expect("write firefox profiles.ini");

    let history = Connection::open(profile_dir.join("places.sqlite")).expect("open firefox db");
    history
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
        .expect("create firefox tables");
    history
        .execute(
            "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
             VALUES (1, 'https://example.com/firefox', 'Firefox docs', 1, 0, 1744146000000000)",
            [],
        )
        .expect("insert firefox place");
    history
        .execute(
            "INSERT INTO moz_historyvisits (id, place_id, visit_date, from_visit, visit_type)
             VALUES (1, 1, 1744146000000000, NULL, 1)",
            [],
        )
        .expect("insert firefox visit");

    profiles_dir
}

fn seed_safari_fixture(root: &Path) -> PathBuf {
    let safari_root = root.join("Safari");
    fs::create_dir_all(&safari_root).expect("create safari root");
    let history = Connection::open(safari_root.join("History.db")).expect("open safari db");
    history
        .execute_batch(
            "CREATE TABLE history_items (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL
             );
             CREATE TABLE history_visits (
               id INTEGER PRIMARY KEY,
               history_item INTEGER NOT NULL,
               title TEXT,
               visit_time REAL NOT NULL
             );",
        )
        .expect("create safari tables");
    history
        .execute("INSERT INTO history_items (id, url) VALUES (1, 'https://example.com/safari')", [])
        .expect("insert safari item");
    history
        .execute(
            "INSERT INTO history_visits (id, history_item, title, visit_time)
             VALUES (1, 1, 'Safari docs', 765838800.0)",
            [],
        )
        .expect("insert safari visit");
    safari_root
}

fn seed_takeout_fixture(root: &Path) -> PathBuf {
    let source_dir = root.join("takeout-source");
    fs::create_dir_all(&source_dir).expect("create takeout dir");
    fs::write(
        source_dir.join("entries.jsonl"),
        r#"{"url":"https://example.com/import","title":"Imported","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
    )
    .expect("write takeout fixture");
    source_dir
}

#[test]
fn canonical_backup_pipeline_writes_runs_manifests_snapshots_and_queries() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let mut progress_events = Vec::new();
    let report = run_backup_with_progress(&paths, &config, None, false, |event| {
        progress_events.push(event);
    })
    .expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
    assert_eq!(report.run.as_ref().expect("run").new_downloads, 1);
    assert!(report.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));
    assert!(report.profiles[0].checkpoint_created);
    assert!(progress_events.iter().any(|event| event.phase == "prepare"));
    assert!(progress_events.iter().any(|event| event.phase == "stage-profile"));
    assert!(progress_events.iter().any(|event| event.phase == "ingest-profile"));
    assert!(progress_events.iter().any(|event| event.phase == "finalize"));

    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
    assert!(!recent_runs.is_empty());
    assert!(recent_runs.iter().any(|run| run.run_type == "backup" && run.status == "success"));

    let history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
    )
    .expect("list history");
    assert_eq!(history.total, 2);
    assert!(history.items.iter().all(|entry| {
        entry.favicon.as_ref().is_some_and(|favicon| favicon.data_url.starts_with("data:image/"))
    }));

    let search_term_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("deep recall".to_string()), ..HistoryQuery::default() },
    )
    .expect("list search term history");
    assert_eq!(search_term_history.total, 2);

    let url_fragment_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("example.com/archive".to_string()), ..HistoryQuery::default() },
    )
    .expect("list url fragment history");
    assert_eq!(url_fragment_history.total, 2);

    let punctuation_only_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("!!!".to_string()), ..HistoryQuery::default() },
    )
    .expect("list punctuation-only history");
    assert_eq!(punctuation_only_history.total, 0);

    let regex_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            ..HistoryQuery::default()
        },
    )
    .expect("regex history");
    assert_eq!(regex_history.total, 2);

    let invalid_regex = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive(".to_string()),
            regex_mode: Some(true),
            ..HistoryQuery::default()
        },
    )
    .expect_err("invalid regex");
    assert!(
        format!("{invalid_regex:#}").contains("invalid regex pattern"),
        "unexpected error: {invalid_regex:#}"
    );

    let first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("first history page");
    assert_eq!(first_page.total, 2);
    assert_eq!(first_page.items.len(), 1);
    assert!(first_page.next_cursor.is_some());

    let second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            limit: Some(1),
            cursor: first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("second history page");
    assert_eq!(second_page.total, 2);
    assert_eq!(second_page.items.len(), 1);
    assert!(second_page.next_cursor.is_none());

    let paged_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery {
                q: Some("archive".to_string()),
                limit: Some(1),
                page: Some(2),
                ..HistoryQuery::default()
            },
            format: ExportFormat::Jsonl,
        },
    )
    .expect("export all visible history even when current query is paged");
    assert_eq!(paged_export.count, 2);

    let report_again = run_backup(&paths, &config, None, false).expect("rerun backup");
    assert_eq!(report_again.run.as_ref().expect("run").new_visits, 0);

    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    let mut statement = connection
        .prepare(
            "EXPLAIN QUERY PLAN
             SELECT visits.id
             FROM visits
             JOIN urls ON urls.id = visits.url_id
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             JOIN search.history_search AS history_search ON history_search.rowid = urls.id
             WHERE visits.reverted_at IS NULL
               AND history_search MATCH ?1",
        )
        .expect("prepare query plan");
    let plan = statement
        .query_map(["\"deep\"*"], |row| row.get::<_, String>(3))
        .expect("query plan rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect query plan");
    assert!(
        plan.iter().any(|detail| detail.contains("VIRTUAL TABLE INDEX")),
        "unexpected query plan: {plan:?}"
    );
    assert!(
        plan.iter().any(|detail| detail.contains("idx_visits_visible_url_time")),
        "fts history query is not using the visible visit lookup index: {plan:?}"
    );
    assert!(
        !plan.iter().any(|detail| detail == "SCAN visits"),
        "fts history query still scans the whole visits table: {plan:?}"
    );

    let mut favicon_statement = connection
        .prepare(
            "EXPLAIN QUERY PLAN
             SELECT visits.id,
                    (
                      SELECT favicons.image_data
                      FROM favicons
                      WHERE favicons.source_profile_id = source_profiles.id
                        AND favicons.page_url = urls.url
                        AND favicons.image_data IS NOT NULL
                      ORDER BY
                        favicons.last_updated_ms DESC,
                        favicons.width DESC,
                        favicons.height DESC,
                        favicons.id DESC
                      LIMIT 1
                    ) AS favicon_image_data
             FROM visits
             JOIN urls ON urls.id = visits.url_id
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE visits.reverted_at IS NULL
             ORDER BY visits.visit_time_ms DESC, visits.id DESC
             LIMIT 150",
        )
        .expect("prepare favicon query plan");
    let favicon_plan = favicon_statement
        .query_map([], |row| row.get::<_, String>(3))
        .expect("query favicon plan rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect favicon query plan");
    assert!(
        favicon_plan.iter().any(|detail| detail.contains("idx_favicons_recall_lookup")),
        "unexpected favicon query plan: {favicon_plan:?}"
    );
    assert!(
        !favicon_plan.iter().any(|detail| detail.contains("SCAN favicons")),
        "favicon lookup still scans the whole table: {favicon_plan:?}"
    );

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn multi_browser_backup_ingests_firefox_and_safari_history() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let safari_root = seed_safari_fixture(dir.path());
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_FIREFOX_PROFILES_DIR", &firefox_profiles);
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec![
            "firefox:abcd.default-release".to_string(),
            "safari:default".to_string(),
        ],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report = run_backup(&paths, &config, None, false).expect("run multi-browser backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 2);
    assert_eq!(report.profiles.len(), 2);
    assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("firefox:")));
    assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("safari:")));
    assert!(report.warnings.iter().any(|warning| warning.contains("Firefox baseline ingest")));
    assert!(report.warnings.iter().any(|warning| warning.contains("Safari baseline ingest")));

    let history = list_history(&paths, &config, None, HistoryQuery::default()).expect("history");
    assert_eq!(history.total, 2);
    assert!(history.items.iter().any(|entry| entry.profile_id.starts_with("firefox:")));
    assert!(history.items.iter().any(|entry| entry.profile_id.starts_with("safari:")));

    let rerun = run_backup(&paths, &config, None, false).expect("rerun multi-browser backup");
    assert_eq!(rerun.run.as_ref().expect("rerun").new_visits, 0);
    assert_eq!(rerun.run.as_ref().expect("rerun").new_urls, 0);

    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn safari_backup_baseline_ingests_history_without_firefox_or_chrome_dependency() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let safari_root = seed_safari_fixture(dir.path());
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", dir.path().join("missing-chrome"));
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["safari:default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report = run_backup(&paths, &config, None, false).expect("run safari backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 1);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "safari:default");
    assert!(report.warnings.iter().any(|warning| warning.contains("Safari baseline ingest")));

    let history = list_history(&paths, &config, None, HistoryQuery::default()).expect("history");
    assert_eq!(history.total, 1);
    assert_eq!(history.items[0].profile_id, "safari:default");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn backup_keeps_chrome_successful_when_selected_safari_is_unreadable() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let safari_root = dir.path().join("Safari");
    fs::create_dir_all(&safari_root).expect("create safari root");
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

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report = run_backup(&paths, &config, None, false).expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "chrome:Default");
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("safari:default")
            && warning.contains("grant Full Disk Access before the next backup")
    }));

    let history = list_history(&paths, &config, None, HistoryQuery::default()).expect("history");
    assert_eq!(history.total, 2);
    assert!(history.items.iter().all(|entry| entry.profile_id.starts_with("chrome:")));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn doctor_detects_missing_snapshot_artifacts() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_paths(&paths).expect("ensure paths");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
             VALUES (0, ?1, 0, 'missing', 'test', ?2)",
            params![dir.path().join("missing").display().to_string(), now_rfc3339()],
        )
        .expect("insert missing snapshot");

    let report = doctor(&paths, &config, None).expect("doctor");
    assert!(report.checks.iter().any(|check| check.name == "Snapshot artifacts" && !check.ok));
}

#[test]
fn doctor_repair_restores_missing_import_artifacts_visibility_and_derived_state() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch = inspection.import_batch.expect("batch");
    let audit_path = batch.audit_path.expect("audit path");
    fs::remove_file(&audit_path).expect("remove import audit artifact");

    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "UPDATE visits SET reverted_at = ?1, reverted_by_run_id = NULL WHERE import_batch_id = ?2",
            params![now_rfc3339(), batch.id],
        )
        .expect("break visibility");
    let intelligence =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");
    intelligence
        .execute(
            "INSERT INTO ai_embeddings
             (history_id, profile_id, url, title, domain, visited_at, content_hash, content_bytes, provider_id, model, indexed_at)
             VALUES (999, 'takeout::browser-history', 'https://example.com/import', 'Imported', 'example.com', ?1, 'hash', 8, 'provider', 'model', ?1)",
            rusqlite::params![now_rfc3339()],
        )
        .expect("insert stale ai embedding");
    intelligence
        .execute(
            "INSERT INTO search_trail_members (trail_id, profile_id, visit_id, ordinal, role)
             VALUES ('trail-1', 'takeout::browser-history', 999, 0, 'result')",
            [],
        )
        .expect("insert stale trail member");
    intelligence
        .execute(
            "INSERT INTO visit_derived_facts
             (visit_id, profile_id, session_id, trail_id, registrable_domain, canonical_url, domain_category, page_category, search_engine, search_query, is_new_domain, is_search_event, evidence_tier, taxonomy_source, taxonomy_pack, taxonomy_version, computed_at)
             VALUES (999, 'takeout::browser-history', 'session-1', 'trail-1', 'example.com', 'https://example.com/import', 'reference', 'article', NULL, NULL, 0, 0, 'tier-c', 'builtin', 'core-intelligence', 'test', ?1)",
            [now_rfc3339()],
        )
        .expect("insert stale visit-derived facts");

    let report = doctor(&paths, &config, None).expect("doctor before repair");
    assert!(report.checks.iter().any(|check| check.name == "Import audit artifacts" && !check.ok));
    assert!(
        report.checks.iter().any(|check| check.name == "Broken visibility references" && !check.ok)
    );
    assert!(report.checks.iter().any(|check| check.name == "Derived state freshness" && !check.ok));

    let repair = repair_health_issues(&paths, &config, None).expect("repair health");
    assert!(repair.run_id.is_some());
    assert_eq!(repair.repaired_import_audits, 1);
    assert_eq!(repair.repaired_visibility_rows, 1);
    assert!(repair.cleared_derived_rows >= 2);

    let repaired_report = doctor(&paths, &config, None).expect("doctor after repair");
    assert!(repaired_report.checks.iter().all(|check| check.ok));
}

#[test]
fn dashboard_snapshot_tracks_cached_totals_across_import_visibility_changes() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch_id = inspection.import_batch.expect("batch").id;

    let dashboard_after_import =
        load_dashboard_snapshot(&paths, &config, None).expect("dashboard after import");
    assert_eq!(dashboard_after_import.total_visits, 1);
    assert_eq!(dashboard_after_import.total_urls, 1);
    let visible_after_import = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
    )
    .expect("query after import");
    assert_eq!(visible_after_import.total, 1);

    let after_import_stats: Value = Connection::open(&paths.archive_database_path)
        .expect("open archive")
        .query_row(
            "SELECT stats_json
             FROM runs
             WHERE run_type = 'import'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .map(|value| serde_json::from_str(&value).expect("parse import stats"))
        .expect("load import stats");
    assert_eq!(after_import_stats["totalVisits"], 1);

    crate::takeout::revert_import_batch(&paths, &config, None, batch_id)
        .expect("revert import batch");
    let dashboard_after_revert =
        load_dashboard_snapshot(&paths, &config, None).expect("dashboard after revert");
    assert_eq!(dashboard_after_revert.total_visits, 0);
    assert_eq!(dashboard_after_revert.total_urls, 1);
    let hidden_after_revert = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
    )
    .expect("query after revert");
    assert_eq!(hidden_after_revert.total, 0);

    let after_revert_stats: Value = Connection::open(&paths.archive_database_path)
        .expect("open archive")
        .query_row(
            "SELECT stats_json
             FROM runs
             WHERE run_type = 'rollback'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .map(|value| serde_json::from_str(&value).expect("parse revert stats"))
        .expect("load revert stats");
    assert_eq!(after_revert_stats["totalVisits"], 0);

    crate::takeout::restore_import_batch(&paths, &config, None, batch_id)
        .expect("restore import batch");
    let dashboard_after_restore =
        load_dashboard_snapshot(&paths, &config, None).expect("dashboard after restore");
    assert_eq!(dashboard_after_restore.total_visits, 1);
    assert_eq!(dashboard_after_restore.total_urls, 1);
    let visible_after_restore = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
    )
    .expect("query after restore");
    assert_eq!(visible_after_restore.total, 1);
}

#[test]
fn snapshot_restore_preview_and_run_record_the_saved_checkpoint() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let backup = run_backup(&paths, &config, None, false).expect("run backup");
    let snapshot_path: String = Connection::open(&paths.archive_database_path)
        .expect("open archive")
        .query_row(
            "SELECT file_path
             FROM snapshots
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("latest snapshot path");

    let preview = preview_snapshot_restore(
        &paths,
        &config,
        None,
        &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
    )
    .expect("preview snapshot restore");
    assert!(preview.execute_supported);
    assert_eq!(preview.snapshot_kind, "raw-source-checkpoint");
    assert_eq!(preview.estimated_visits, 2);
    assert_eq!(preview.estimated_urls, 1);

    let restored = run_snapshot_restore(
        &paths,
        &config,
        None,
        &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
    )
    .expect("run snapshot restore");
    let restore_run = restored.run.expect("restore run");
    assert_eq!(restore_run.run_type, "snapshot_restore");
    assert_eq!(restore_run.status, "success");
    assert!(backup.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));

    let detail =
        load_audit_run_detail(&paths, &config, None, restore_run.id).expect("restore detail");
    assert!(
        detail
            .artifacts
            .iter()
            .any(|artifact| artifact.reason.as_deref() == Some("restored-source-checkpoint"))
    );

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn source_evidence_spools_large_deferred_payloads_and_cleans_up_tempfiles() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let spool_dir = paths.staging_dir.join("source-evidence-spool");
    let large_blob = "x".repeat(300_000);

    {
        let deferred = defer_source_evidence_payload(
            &paths,
            "large-source-evidence",
            SourceEvidencePayload {
                typed_evidence: TypedEvidenceBatch {
                    context: vec![ContextEvidence {
                        source_visit_id: Some(1),
                        source_url_id: Some(1),
                        context_key: "context.takeout.large".to_string(),
                        value_json: serde_json::to_string(&large_blob).expect("serialize context"),
                        source_field: "payload".to_string(),
                    }],
                    ..TypedEvidenceBatch::default()
                },
                native_entities: vec![NativeEntity {
                    entity_kind: "takeout-browser-history".to_string(),
                    native_primary_key: "row-1".to_string(),
                    parent_native_primary_key: None,
                    payload_json: serde_json::json!({ "blob": large_blob }).to_string(),
                    metadata: BTreeMap::new(),
                }],
            },
        )
        .expect("defer source evidence");

        assert!(deferred.is_spooled());
        assert_eq!(fs::read_dir(&spool_dir).expect("read spool dir").count(), 1);
    }

    assert_eq!(fs::read_dir(&spool_dir).expect("read cleaned spool dir").count(), 0);
}

#[test]
fn snapshot_restore_preview_sizes_firefox_and_safari_checkpoints() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let safari_root = seed_safari_fixture(dir.path());
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_FIREFOX_PROFILES_DIR", &firefox_profiles);
        std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec![
            "firefox:abcd.default-release".to_string(),
            "safari:default".to_string(),
        ],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    run_backup(&paths, &config, None, false).expect("run multi-browser backup");

    let snapshot_paths = Connection::open(&paths.archive_database_path)
        .expect("open archive")
        .prepare("SELECT file_path FROM snapshots ORDER BY id")
        .expect("prepare snapshot query")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query snapshots")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect snapshot paths");
    let previews = snapshot_paths
        .iter()
        .map(|snapshot_path| {
            preview_snapshot_restore(
                &paths,
                &config,
                None,
                &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
            )
        })
        .collect::<Result<Vec<_>>>()
        .expect("preview snapshots");

    assert!(previews.iter().any(|preview| {
        preview
            .source_browser_name
            .as_deref()
            .is_some_and(|browser_name| browser_name.eq_ignore_ascii_case("firefox"))
            && preview.estimated_visits == 1
            && preview.estimated_urls == 1
    }));
    assert!(previews.iter().any(|preview| {
        preview
            .source_browser_name
            .as_deref()
            .is_some_and(|browser_name| browser_name.eq_ignore_ascii_case("safari"))
            && preview.estimated_visits == 1
            && preview.estimated_urls == 1
    }));

    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn retention_preview_and_prune_clear_local_artifacts_and_record_a_run() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    run_backup(&paths, &config, None, false).expect("run backup");
    fs::create_dir_all(&paths.exports_dir).expect("create exports dir");
    fs::write(paths.exports_dir.join("export.jsonl"), "[]").expect("write export fixture");

    let preview = preview_retention(&paths, &config, None).expect("preview retention");
    assert!(preview.buckets.iter().any(|bucket| bucket.id == "snapshots" && bucket.bytes > 0));
    assert!(preview.buckets.iter().any(|bucket| bucket.id == "exports" && bucket.bytes > 0));

    let result = run_retention_prune(
        &paths,
        &config,
        None,
        &RetentionPruneRequest { bucket_ids: vec!["snapshots".to_string(), "exports".to_string()] },
    )
    .expect("run retention prune");
    assert!(result.run_id.is_some());
    assert!(result.deleted_bytes > 0);
    assert_eq!(directory_size(&paths.raw_snapshots_dir), 0);
    assert_eq!(directory_size(&paths.exports_dir), 0);

    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let snapshot_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
        .expect("snapshot count");
    assert_eq!(snapshot_count, 0);
    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
    assert!(recent_runs.iter().any(|run| run.run_type == "retention_prune"));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn rekey_archive_keeps_a_safety_snapshot() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let status =
        rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, Some("vault-passphrase"))
            .expect("rekey archive");

    let rekey_dir = paths.raw_snapshots_dir.join("rekey");
    let snapshots = fs::read_dir(&rekey_dir)
        .expect("read rekey snapshot dir")
        .filter_map(|entry| entry.ok())
        .collect::<Vec<_>>();

    assert!(status.encrypted);
    assert_eq!(snapshots.len(), 1);
    assert!(snapshots[0].path().is_file());

    let encrypted_config = AppConfig { archive_mode: ArchiveMode::Encrypted, ..config.clone() };
    let recent_runs = load_recent_runs(&paths, &encrypted_config, Some("vault-passphrase"))
        .expect("recent runs after rekey");
    let rekey_run =
        recent_runs.iter().find(|run| run.run_type == "rekey").expect("rekey run in ledger");
    let detail =
        load_audit_run_detail(&paths, &encrypted_config, Some("vault-passphrase"), rekey_run.id)
            .expect("rekey audit detail");
    assert!(detail.manifest_path.is_some());
    assert!(
        detail.artifacts.iter().any(|artifact| artifact.reason.as_deref() == Some("before-rekey"))
    );
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
