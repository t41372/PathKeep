//! Regression tests for the canonical archive domain.
use super::*;
use crate::{
    config::{ProjectPaths, project_paths_with_root},
    models::{
        ArchiveMode, BrowserProfile, RetentionPruneRequest, SnapshotRestoreRequest, TakeoutRequest,
    },
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

fn seed_missing_chrome_history_fixture(root: &Path) -> PathBuf {
    let chrome_root = root.join("chrome-missing-history");
    let profile_dir = chrome_root.join("Default");
    fs::create_dir_all(&profile_dir).expect("create missing chrome profile dir");
    fs::write(chrome_root.join("Last Version"), "146.0.0.0").expect("write version");
    fs::write(
        chrome_root.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"tim@example.com"}}}}"#,
    )
    .expect("write local state");
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
    assert!(progress_events.iter().any(|event| {
        event.phase == "ingest-profile"
            && event.processed_records == Some(2)
            && event.imported_records == Some(2)
            && event.progress_percent.is_none()
            && event.log_events[0].processed_records == Some(2)
    }));
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
    assert!(history.items.iter().all(|entry| entry.favicon.is_none()));

    let loaded_favicons = load_history_favicons(
        &paths,
        &config,
        None,
        history
            .items
            .iter()
            .map(|entry| HistoryFaviconLookupEntry {
                profile_id: entry.profile_id.clone(),
                url: entry.url.clone(),
                visit_time: entry.visit_time,
            })
            .collect(),
    )
    .expect("load history favicons");
    assert_eq!(loaded_favicons.len(), 2);
    let empty_favicons =
        load_history_favicons(&paths, &config, None, Vec::new()).expect("empty favicon lookup");
    assert!(empty_favicons.is_empty());
    let duplicate_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![
            HistoryFaviconLookupEntry {
                profile_id: history.items[0].profile_id.clone(),
                url: history.items[0].url.clone(),
                visit_time: history.items[0].visit_time,
            },
            HistoryFaviconLookupEntry {
                profile_id: history.items[0].profile_id.clone(),
                url: history.items[0].url.clone(),
                visit_time: history.items[0].visit_time,
            },
        ],
    )
    .expect("duplicate favicon lookup");
    assert_eq!(duplicate_favicon_lookup.len(), 1);
    let missing_profile_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Missing".to_string(),
            url: history.items[0].url.clone(),
            visit_time: history.items[0].visit_time,
        }],
    )
    .expect("missing profile favicon lookup");
    assert!(missing_profile_favicon_lookup[0].favicon.is_none());
    let second_visit_favicon = loaded_favicons
        .iter()
        .find(|entry| entry.visit_time == history.items[0].visit_time)
        .expect("second visit favicon result");
    assert!(
        second_visit_favicon
            .favicon
            .as_ref()
            .is_some_and(|favicon| favicon.data_url.starts_with("data:image/")),
        "expected the visit at the icon observation time to load the exact page icon"
    );
    let first_visit_favicon = loaded_favicons
        .iter()
        .find(|entry| entry.visit_time == history.items[1].visit_time)
        .expect("first visit favicon result");
    assert!(
        first_visit_favicon.favicon.is_none(),
        "favicon lookup must not use an exact page icon first observed after the visit"
    );

    let connection = open_archive_connection(&paths, &config, None).expect("open archive");
    let favicon_blob_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM favicon_blobs", [], |row| row.get(0))
        .expect("favicon blob count");
    let favicon_reference_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM favicons WHERE image_blob_hash IS NOT NULL", [], |row| {
            row.get(0)
        })
        .expect("favicon reference count");
    assert_eq!(favicon_blob_count, 1);
    assert_eq!(favicon_reference_count, 2);

    let search_term_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { q: Some("deep recall".to_string()), ..HistoryQuery::default() },
    )
    .expect("list search term history");
    assert_eq!(search_term_history.total, 2);
    let search_term_first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("search term first page");
    assert_eq!(search_term_first_page.items.len(), 1);
    assert!(search_term_first_page.next_cursor.is_some());
    let search_term_second_cursor_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            cursor: search_term_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("search term second cursor page");
    assert_eq!(search_term_second_cursor_page.items.len(), 1);
    assert!(search_term_second_cursor_page.has_previous);
    let search_term_explicit_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("deep recall".to_string()),
            limit: Some(1),
            page: Some(2),
            ..HistoryQuery::default()
        },
    )
    .expect("search term explicit second page");
    assert_eq!(search_term_explicit_second_page.page, 2);
    assert_eq!(search_term_explicit_second_page.items.len(), 1);

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
    let regex_oldest_first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("regex oldest first page");
    assert!(regex_oldest_first_page.next_cursor.is_some());
    let regex_oldest_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            sort: Some("oldest".to_string()),
            limit: Some(1),
            cursor: regex_oldest_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("regex oldest second page");
    assert_eq!(regex_oldest_second_page.items.len(), 1);
    assert!(regex_oldest_second_page.has_previous);
    let regex_newest_first_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            ..HistoryQuery::default()
        },
    )
    .expect("regex newest first page");
    let regex_newest_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            cursor: regex_newest_first_page.next_cursor.clone(),
            ..HistoryQuery::default()
        },
    )
    .expect("regex newest second page");
    assert_eq!(regex_newest_second_page.items.len(), 1);
    let regex_explicit_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery {
            q: Some("archive\\sdocs".to_string()),
            regex_mode: Some(true),
            limit: Some(1),
            page: Some(2),
            ..HistoryQuery::default()
        },
    )
    .expect("regex explicit second page");
    assert_eq!(regex_explicit_second_page.page, 2);
    assert_eq!(regex_explicit_second_page.items.len(), 1);

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

    let explicit_second_page = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { limit: Some(1), page: Some(2), ..HistoryQuery::default() },
    )
    .expect("explicit second history page");
    assert_eq!(explicit_second_page.total, 2);
    assert_eq!(explicit_second_page.page, 2);
    assert_eq!(explicit_second_page.page_count, 2);
    assert_eq!(explicit_second_page.items.len(), 1);
    assert!(explicit_second_page.has_previous);
    assert!(!explicit_second_page.has_next);
    assert!(explicit_second_page.next_cursor.is_none());
    let empty_domain_history = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { domain: Some("missing.invalid".to_string()), ..HistoryQuery::default() },
    )
    .expect("empty domain history");
    assert_eq!(empty_domain_history.total, 0);
    assert_eq!(empty_domain_history.page_count, 1);

    let mut connection = open_archive_connection(&paths, &config, None).expect("open archive");
    let run_id = connection
        .query_row("SELECT id FROM runs ORDER BY id LIMIT 1", [], |row| row.get::<_, i64>(0))
        .expect("load run id");
    connection
        .execute(
            "INSERT INTO favicons (
               page_url,
               icon_url,
               icon_type,
               width,
               height,
               last_updated_ms,
               last_updated_iso,
               image_data,
               source_profile_id,
               created_by_run_id,
               page_host,
               page_registrable_domain
             )
             VALUES (?1, ?2, 1, 16, 16, ?3, ?4, ?5, 1, ?6, ?7, ?8)",
            params![
                "https://docs.example.co.uk/favicon-source",
                "https://docs.example.co.uk/favicon.ico",
                chrono::DateTime::parse_from_rfc3339("2026-04-05T10:30:00+00:00")
                    .expect("registrable same-profile favicon time")
                    .timestamp_millis(),
                "2026-04-05T10:30:00+00:00",
                vec![0x89_u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x02],
                run_id,
                "docs.example.co.uk",
                "example.co.uk",
            ],
        )
        .expect("insert same-profile registrable favicon");
    let same_profile_registrable_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://blog.example.co.uk/article".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T11:00:00+00:00")
                .expect("same-profile registrable visit time")
                .timestamp_millis(),
        }],
    )
    .expect("same-profile registrable favicon lookup");
    assert!(
        same_profile_registrable_lookup[0].favicon.is_some(),
        "expected registrable-domain fallback when the exact host differs inside the same profile"
    );
    connection
        .execute(
            "INSERT INTO source_profiles (
               id,
               browser_kind,
               browser_version,
               profile_name,
               profile_path,
               discovered_at,
               enabled,
               profile_key,
               user_name,
               updated_at,
               browser_family,
               browser_product
             )
             VALUES (?1, ?2, NULL, ?3, ?4, ?5, 1, ?6, NULL, ?5, ?7, ?8)",
            params![
                2_i64,
                "takeout",
                "Imported",
                "/tmp/imported-profile",
                now_rfc3339(),
                "takeout::browser-history",
                "chromium",
                "Takeout",
            ],
        )
        .expect("insert imported source profile");
    connection
        .execute(
            "INSERT INTO favicons (
               page_url,
               icon_url,
               icon_type,
               width,
               height,
               last_updated_ms,
               last_updated_iso,
               image_data,
               source_profile_id,
               created_by_run_id,
               page_host,
               page_registrable_domain
             )
             VALUES (?1, ?2, 1, 16, 16, ?3, ?4, ?5, 2, ?6, ?7, ?8)",
            params![
                "https://shared.example.org/favicon-source",
                "https://shared.example.org/favicon.ico",
                chrono::DateTime::parse_from_rfc3339("2026-04-05T10:40:00+00:00")
                    .expect("cross-profile host favicon time")
                    .timestamp_millis(),
                "2026-04-05T10:40:00+00:00",
                vec![0x89_u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x04],
                run_id,
                "shared.example.org",
                "example.org",
            ],
        )
        .expect("insert cross-profile host favicon");
    let cross_profile_host_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://shared.example.org/article".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T11:20:00+00:00")
                .expect("cross-profile host visit time")
                .timestamp_millis(),
        }],
    )
    .expect("cross-profile host favicon lookup");
    assert!(
        cross_profile_host_lookup[0].favicon.is_some(),
        "expected host fallback to cross profiles when the current profile has no matching host icon"
    );
    connection
        .execute(
            "INSERT INTO favicons (
               page_url,
               icon_url,
               icon_type,
               width,
               height,
               last_updated_ms,
               last_updated_iso,
               image_data,
               source_profile_id,
               created_by_run_id,
               page_host,
               page_registrable_domain
             )
             VALUES (?1, ?2, 1, 16, 16, ?3, ?4, ?5, 2, ?6, ?7, ?8)",
            params![
                "https://learn.example.net/favicon-source",
                "https://learn.example.net/favicon.ico",
                chrono::DateTime::parse_from_rfc3339("2026-04-05T10:45:00+00:00")
                    .expect("cross-profile registrable favicon time")
                    .timestamp_millis(),
                "2026-04-05T10:45:00+00:00",
                vec![0x89_u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x03],
                run_id,
                "learn.example.net",
                "example.net",
            ],
        )
        .expect("insert cross-profile registrable favicon");
    let cross_profile_registrable_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://www.example.net/article".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T11:30:00+00:00")
                .expect("cross-profile registrable visit time")
                .timestamp_millis(),
        }],
    )
    .expect("cross-profile registrable favicon lookup");
    assert!(
        cross_profile_registrable_lookup[0].favicon.is_some(),
        "expected registrable-domain fallback to cross profiles after same-profile misses"
    );
    connection
        .execute("UPDATE visits SET source_profile_id = 2 WHERE id = 2", [])
        .expect("reassign visit profile");

    let cross_profile_favicon = list_history(
        &paths,
        &config,
        None,
        HistoryQuery { page: Some(1), limit: Some(1), ..HistoryQuery::default() },
    )
    .expect("cross-profile favicon history page");
    assert!(cross_profile_favicon.items[0].favicon.is_none());
    let cross_profile_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: cross_profile_favicon.items[0].profile_id.clone(),
            url: cross_profile_favicon.items[0].url.clone(),
            visit_time: cross_profile_favicon.items[0].visit_time,
        }],
    )
    .expect("cross-profile favicon lookup");
    assert!(
        cross_profile_favicon_lookup[0].favicon.is_some(),
        "expected favicon lookup to fall back across source profiles for the same page URL"
    );

    let same_host_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com/missing-page".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T12:00:00+00:00")
                .expect("same-host visit time")
                .timestamp_millis(),
        }],
    )
    .expect("same-host favicon lookup");
    assert!(
        same_host_favicon_lookup[0].favicon.is_some(),
        "expected same-host fallback to reuse a historical icon without requiring exact page_url"
    );

    let future_host_favicon_lookup = load_history_favicons(
        &paths,
        &config,
        None,
        vec![HistoryFaviconLookupEntry {
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com/before-icon".to_string(),
            visit_time: chrono::DateTime::parse_from_rfc3339("2026-04-05T09:00:00+00:00")
                .expect("future-host visit time")
                .timestamp_millis(),
        }],
    )
    .expect("future-host favicon lookup");
    assert!(
        future_host_favicon_lookup[0].favicon.is_none(),
        "domain fallback must not use an icon first observed after the visit"
    );

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
    let html_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
            format: ExportFormat::Html,
        },
    )
    .expect("export history html");
    assert_eq!(html_export.count, 2);
    let html_content = fs::read_to_string(&html_export.path).expect("read html export");
    assert!(html_content.contains("<article>"));
    assert!(html_content.contains("Archive docs"));
    let markdown_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
            format: ExportFormat::Markdown,
        },
    )
    .expect("export history markdown");
    let markdown_content = fs::read_to_string(&markdown_export.path).expect("read markdown export");
    assert!(markdown_content.contains("- [Archive docs](https://example.com/archive)"));
    let text_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
            format: ExportFormat::Text,
        },
    )
    .expect("export history text");
    let text_content = fs::read_to_string(&text_export.path).expect("read text export");
    assert!(text_content.contains("Archive docs\nhttps://example.com/archive\n"));

    let bulk_url_id = 9_001_i64;
    let bulk_base_ms = chrono::DateTime::parse_from_rfc3339("2026-04-06T00:00:00+00:00")
        .expect("bulk base time")
        .timestamp_millis();
    connection
        .execute(
            "INSERT INTO urls (
               id,
               url,
               title,
               visit_count,
               typed_count,
               first_visit_ms,
               first_visit_iso,
               last_visit_ms,
               last_visit_iso,
               source_profile_id,
               created_by_run_id
             )
             VALUES (?1, ?2, ?3, 1001, 0, ?4, ?5, ?6, ?7, 1, ?8)",
            params![
                bulk_url_id,
                "https://bulk.example/export",
                "Bulk export cursor fixture",
                bulk_base_ms,
                "2026-04-06T00:00:00+00:00",
                bulk_base_ms + 1_000,
                "2026-04-06T00:00:01+00:00",
                run_id,
            ],
        )
        .expect("insert bulk export url");
    let bulk_insert = connection.transaction().expect("bulk transaction");
    for index in 0..=1_000_i64 {
        let visit_time_ms = bulk_base_ms + index;
        let visit_time_iso = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(visit_time_ms)
            .expect("bulk visit time")
            .to_rfc3339();
        bulk_insert
            .execute(
                "INSERT INTO visits (
                   url_id,
                   source_visit_id,
                   visit_time_ms,
                   visit_time_iso,
                   transition_type,
                   visit_duration_ms,
                   source_profile_id,
                   created_by_run_id
                 )
                 VALUES (?1, ?2, ?3, ?4, 805306368, 1000, 1, ?5)",
                params![
                    bulk_url_id,
                    format!("bulk-export-{index}"),
                    visit_time_ms,
                    visit_time_iso,
                    run_id,
                ],
            )
            .expect("insert bulk export visit");
    }
    bulk_insert.commit().expect("commit bulk export rows");
    let bulk_export = export_history(
        &paths,
        &config,
        None,
        ExportRequest {
            query: HistoryQuery {
                domain: Some("bulk.example".to_string()),
                ..HistoryQuery::default()
            },
            format: ExportFormat::Jsonl,
        },
    )
    .expect("export multi-page history");
    assert_eq!(bulk_export.count, 1001);

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

    let visit_time = chrono::DateTime::parse_from_rfc3339("2026-04-05T12:00:00+00:00")
        .expect("query plan visit time")
        .timestamp_millis();
    fn assert_favicon_plan_uses<P: rusqlite::Params>(
        connection: &Connection,
        sql: &str,
        expected_index: &str,
        params: P,
    ) {
        let mut favicon_statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .expect("prepare favicon query plan");
        let favicon_plan = favicon_statement
            .query_map(params, |row| row.get::<_, String>(3))
            .expect("query favicon plan rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect favicon query plan");
        assert!(
            favicon_plan.iter().any(|detail| detail.contains(expected_index)),
            "favicon query is not using {expected_index}: {favicon_plan:?}"
        );
        assert!(
            !favicon_plan.iter().any(|detail| detail.contains("SCAN favicons")),
            "favicon lookup still scans the whole table: {favicon_plan:?}"
        );
    }
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_SAME_PROFILE_PAGE_SQL,
        "idx_favicons_recall_lookup",
        params![1_i64, "https://example.com/archive", visit_time],
    );
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL,
        "idx_favicons_page_lookup",
        params![1_i64, "https://example.com/archive", visit_time],
    );
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_SAME_PROFILE_HOST_SQL,
        "idx_favicons_host_profile_lookup",
        params![1_i64, "example.com", "https://example.com/archive", visit_time],
    );
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_CROSS_PROFILE_HOST_SQL,
        "idx_favicons_host_lookup",
        params![1_i64, "example.com", "https://example.com/archive", visit_time],
    );
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_SAME_PROFILE_REGISTRABLE_SQL,
        "idx_favicons_registrable_profile_lookup",
        params![1_i64, "example.org", "https://example.com/archive", visit_time],
    );
    assert_favicon_plan_uses(
        &connection,
        super::history::LOAD_FAVICON_CROSS_PROFILE_REGISTRABLE_SQL,
        "idx_favicons_registrable_lookup",
        params![1_i64, "example.org", "https://example.com/archive", visit_time],
    );

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn dashboard_read_models_cover_uninitialized_storage_and_cached_totals_edges() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let uninitialized = AppConfig::default();

    let recent_runs =
        load_recent_runs(&paths, &uninitialized, None).expect("uninitialized recent runs");
    assert!(recent_runs.is_empty());

    let snapshot =
        load_dashboard_snapshot(&paths, &uninitialized, None).expect("uninitialized dashboard");
    assert!(snapshot.next_action.as_deref().is_some_and(|copy| copy.contains("Initialize")));
    assert_eq!(snapshot.storage.archive_database_bytes, 0);
    assert_eq!(directory_size(&dir.path().join("missing-dir")), 0);

    let file_instead_of_directory = dir.path().join("not-a-directory");
    fs::write(&file_instead_of_directory, "plain file").expect("write file");
    assert_eq!(directory_size(&file_instead_of_directory), 0);

    let nested = dir.path().join("nested");
    fs::create_dir_all(nested.join("child")).expect("create nested");
    fs::write(nested.join("root.bin"), "1234").expect("write root");
    fs::write(nested.join("child").join("leaf.bin"), "123456").expect("write child");
    assert_eq!(directory_size(&nested), 10);

    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    connection.execute("DELETE FROM runs", []).expect("clear bootstrap run rows");
    drop(connection);
    let initialized_empty =
        load_dashboard_snapshot(&paths, &config, None).expect("initialized dashboard");
    assert!(
        initialized_empty.next_action.as_deref().is_some_and(|copy| copy.contains("manual backup"))
    );

    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    for (id, stats_json) in [
        (100_i64, "{malformed"),
        (99_i64, r#"{"totalProfiles":1}"#),
        (98_i64, r#"{"totalProfiles":2,"totalUrls":3,"totalVisits":5,"totalDownloads":7}"#),
    ] {
        connection
            .execute(
                "INSERT INTO runs
                 (id, run_type, trigger, started_at, timezone, status, profile_scope_json,
                  warnings_json, stats_json, due_only)
                 VALUES (?1, 'backup', 'manual', ?2, 'UTC', 'success', '[]', '[]', ?3, 0)",
                params![id, now_rfc3339(), stats_json],
            )
            .expect("insert cached stats run");
    }

    let totals = read_models::load_cached_archive_totals(&connection)
        .expect("cached totals")
        .expect("valid cached totals");
    assert_eq!(totals.total_profiles, 2);
    assert_eq!(totals.total_urls, 3);
    assert_eq!(totals.total_visits, 5);
    assert_eq!(totals.total_downloads, 7);
}

#[test]
fn backup_guards_initialization_selection_and_due_skip_before_profile_work() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let uninitialized = run_backup(&paths, &AppConfig::default(), None, false)
        .expect_err("uninitialized archive should fail");
    assert!(uninitialized.to_string().contains("archive has not been initialized"));

    let initialized = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &initialized, None).expect("init archive");
    let no_selection = run_backup(&paths, &initialized, None, false)
        .expect_err("empty selected profiles should fail");
    assert!(no_selection.to_string().contains("select at least one readable browser profile"));

    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    let recent = chrono::Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, finished_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'manual', ?1, ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [recent],
        )
        .expect("insert recent successful backup");

    let skipped = run_backup(&paths, &initialized, None, true).expect("due-only backup skip");
    assert!(skipped.due_skipped);
    assert!(skipped.reason.as_deref().is_some_and(|reason| reason.contains("hours old")));
}

#[test]
fn retention_helpers_fall_back_to_filesystem_counts_when_archive_is_unreadable() {
    let root = tempdir().expect("tempdir");
    let paths = sample_paths(root.path());
    let nested = paths.raw_snapshots_dir.join("nested");
    fs::create_dir_all(&nested).expect("snapshot nested dir");
    fs::write(nested.join("snapshot.sqlite"), b"snapshot").expect("snapshot file");
    fs::write(paths.raw_snapshots_dir.join("root.sqlite"), b"snapshot").expect("root snapshot");

    let missing = root.path().join("missing-retention-root");
    assert_eq!(count_path_entries(&missing), 0);
    assert_eq!(remove_directory_contents(&missing).expect("missing directory"), (0, 0));
    assert_eq!(remove_path(&missing).expect("missing path"), (0, 0));

    let file_instead_of_dir = root.path().join("not-a-directory");
    fs::write(&file_instead_of_dir, b"file").expect("file path");
    assert_eq!(count_path_entries(&file_instead_of_dir), 0);

    let mut unreadable_paths = paths.clone();
    unreadable_paths.archive_database_path = root.path().join("archive-directory");
    fs::create_dir_all(&unreadable_paths.archive_database_path).expect("archive dir");
    let bucket = retention_snapshot_bucket(
        &unreadable_paths,
        &AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        },
        None,
    )
    .expect("retention bucket");

    assert_eq!(bucket.id, "snapshots");
    assert_eq!(bucket.item_count, 3);

    let uninitialized_bucket = retention_snapshot_bucket(
        &paths,
        &AppConfig {
            initialized: false,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        },
        None,
    )
    .expect("uninitialized retention bucket");
    assert_eq!(uninitialized_bucket.item_count, 3);
}

#[test]
fn stats_with_archive_totals_replaces_non_object_inputs_with_totals() {
    let connection = Connection::open_in_memory().expect("sqlite");
    create_schema(&connection).expect("schema");

    let stats =
        stats_with_archive_totals(&connection, serde_json::json!("not-an-object")).expect("stats");

    assert_eq!(stats["totalProfiles"], 0);
    assert_eq!(stats["totalUrls"], 0);
    assert_eq!(stats["totalVisits"], 0);
    assert_eq!(stats["totalDownloads"], 0);
}

#[test]
fn backup_rejects_selected_profiles_that_are_not_readable() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = dir.path().join("empty-chrome-root");
    fs::create_dir_all(&chrome_root).expect("empty chrome root");
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Missing".to_string()],
        ..AppConfig::default()
    };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let error = run_backup(&paths, &config, None, false)
        .expect_err("unreadable selected profile should fail");

    assert!(error.to_string().contains("selected profiles are not readable"));
    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn backup_progress_and_warning_helpers_preserve_failure_contracts() {
    let profile = BrowserProfile {
        profile_id: "chrome:Default".to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: "Google Chrome".to_string(),
        user_name: None,
        profile_path: "/tmp/chrome/Default".to_string(),
        history_path: Some("/tmp/chrome/Default/History".to_string()),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: None,
        history_file_name: "History".to_string(),
        history_bytes: 0,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: Default::default(),
    };
    let mut last_processed_records = 0;
    let mut events = Vec::new();
    super::backup::emit_backup_ingest_progress_if_changed(
        &mut |event| events.push(event),
        &mut last_processed_records,
        0,
        1,
        &profile,
        super::ingest::ArchiveIngestProgress {
            processed_records: 0,
            imported_records: 0,
            duplicate_records: 0,
            skipped_records: 0,
        },
    );
    assert!(events.is_empty());

    super::backup::emit_backup_ingest_progress_if_changed(
        &mut |event| events.push(event),
        &mut last_processed_records,
        0,
        1,
        &profile,
        super::ingest::ArchiveIngestProgress {
            processed_records: 2,
            imported_records: 1,
            duplicate_records: 1,
            skipped_records: 0,
        },
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].processed_records, Some(2));
    assert_eq!(events[0].source_label.as_deref(), Some("Google Chrome / Default"));
    assert_eq!(events[0].log_events[0].code, "backup.ingest-profile.records");
    assert_eq!(events[0].log_events[0].imported_records, Some(1));

    let source_warning =
        super::backup::source_evidence_rebuild_warning(anyhow::anyhow!("source offline"));
    let search_warning =
        super::backup::keyword_recall_rebuild_warning(anyhow::anyhow!("search offline"));
    assert!(source_warning.contains("source-evidence archive"));
    assert!(search_warning.contains("keyword-recall projection"));
}

#[test]
fn backup_marks_run_failed_when_readable_profile_cannot_be_staged() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = dir.path().join("broken-chrome-root");
    let profile_dir = chrome_root.join("Default");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    fs::write(chrome_root.join("Last Version"), "146.0.0.0").expect("write version");
    fs::write(
        chrome_root.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{"name":"Default"}}}}"#,
    )
    .expect("write local state");
    fs::create_dir(profile_dir.join("History")).expect("create bad history directory");
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

    let error = run_backup(&paths, &config, None, false).expect_err("staging should fail");
    let status = Connection::open(&paths.archive_database_path)
        .expect("archive")
        .query_row("SELECT status FROM runs ORDER BY id DESC LIMIT 1", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("run status");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());

    assert!(format!("{error:#}").contains("staging profile chrome:Default"));
    assert_eq!(status, "failed");
}

#[test]
fn backup_skips_unreadable_selected_profile_when_another_profile_is_readable() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let chrome_root = seed_missing_chrome_history_fixture(dir.path());
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_FIREFOX_PROFILES_DIR", &firefox_profiles);
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        selected_profile_ids: vec![
            "firefox:abcd.default-release".to_string(),
            "chrome:Default".to_string(),
        ],
        ..AppConfig::default()
    };

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let report = run_backup(&paths, &config, None, false).expect("run backup with skipped profile");

    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());

    assert_eq!(report.run.as_ref().expect("run").new_visits, 1);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "firefox:abcd.default-release");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.contains("chrome:Default") && warning.contains("unreadable"))
    );
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

#[cfg(unix)]
#[test]
fn backup_keeps_readable_profiles_when_safari_staging_loses_access() {
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
fn doctor_detects_manifest_parent_and_hash_damage() {
    let parent_dir = tempdir().expect("tempdir");
    let parent_paths = sample_paths(parent_dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&parent_paths, &config, None).expect("init parent archive");
    let parent_connection =
        Connection::open(&parent_paths.archive_database_path).expect("open parent archive");
    create_schema(&parent_connection).expect("parent schema");
    parent_connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, finished_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'manual', ?1, ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("insert parent run");
    let run_id = parent_connection.last_insert_rowid();
    parent_connection
        .execute(
            "INSERT INTO manifests (run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path)
             VALUES (?1, NULL, 'first-hash', '{}', ?2, NULL)",
            params![run_id, now_rfc3339()],
        )
        .expect("insert first manifest");
    parent_connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys for damaged fixture");
    parent_connection
        .execute(
            "INSERT INTO manifests (run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path)
             VALUES (?1, 9999, 'second-hash', '{}', ?2, NULL)",
            params![run_id, now_rfc3339()],
        )
        .expect("insert broken parent manifest");

    let parent_report = doctor(&parent_paths, &config, None).expect("doctor parent");
    assert!(parent_report.checks.iter().any(|check| {
        check.name == "Manifest chain"
            && !check.ok
            && check.detail.contains("does not point to the previous manifest")
    }));

    let hash_dir = tempdir().expect("tempdir");
    let hash_paths = sample_paths(hash_dir.path());
    ensure_archive_initialized(&hash_paths, &config, None).expect("init hash archive");
    let hash_connection =
        Connection::open(&hash_paths.archive_database_path).expect("open hash archive");
    create_schema(&hash_connection).expect("hash schema");
    hash_connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, finished_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'manual', ?1, ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("insert hash run");
    let run_id = hash_connection.last_insert_rowid();
    let manifest_path = hash_dir.path().join("manifest.json");
    fs::write(&manifest_path, r#"{"ok":true}"#).expect("write manifest artifact");
    hash_connection
        .execute(
            "INSERT INTO manifests (run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path)
             VALUES (?1, NULL, 'not-the-real-hash', '{}', ?2, ?3)",
            params![run_id, now_rfc3339(), manifest_path.display().to_string()],
        )
        .expect("insert hash mismatch manifest");

    let hash_report = doctor(&hash_paths, &config, None).expect("doctor hash");
    assert!(hash_report.checks.iter().any(|check| {
        check.name == "Manifest chain"
            && !check.ok
            && check.detail.contains("manifest hash mismatch")
    }));
}

#[test]
fn doctor_detects_import_batches_without_audit_artifacts() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "INSERT INTO import_batches (source_kind, source_path, profile_id, created_at, imported_at, status, summary_json, audit_path)
             VALUES ('takeout', '/tmp/takeout', 'takeout::browser-history', ?1, ?1, 'imported', '{}', NULL)",
            [now_rfc3339()],
        )
        .expect("insert import batch without audit path");

    let report = doctor(&paths, &config, None).expect("doctor");
    assert!(report.checks.iter().any(|check| {
        check.name == "Import audit artifacts"
            && !check.ok
            && check.detail.contains("does not have an audit artifact")
    }));
}

#[test]
fn doctor_repair_noops_on_healthy_archive() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let repair = repair_health_issues(&paths, &config, None).expect("repair health");

    assert!(repair.run_id.is_none());
    assert_eq!(repair.repaired_import_audits, 0);
    assert_eq!(repair.repaired_visibility_rows, 0);
    assert_eq!(repair.cleared_derived_rows, 0);
    assert!(repair.notes.iter().any(|note| note.contains("found no actionable damage")));
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
fn doctor_repair_tolerates_missing_optional_intelligence_tables() {
    for dropped_tables in [
        vec!["ai_embeddings", "search_trail_members"],
        vec!["visit_derived_facts"],
        vec!["search_trail_members", "visit_derived_facts"],
    ] {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = AppConfig { initialized: true, ..AppConfig::default() };
        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let intelligence =
            open_intelligence_connection(&paths, &config, None).expect("open intelligence");
        for table in dropped_tables {
            intelligence
                .execute(&format!("DROP TABLE IF EXISTS {table}"), [])
                .expect("drop optional intelligence table");
        }
        drop(intelligence);

        let repair = repair_health_issues(&paths, &config, None).expect("repair health");

        assert!(repair.run_id.is_none());
        assert_eq!(repair.cleared_derived_rows, 0);
    }
}

#[test]
fn doctor_repair_restores_visibility_when_import_audits_are_intact() {
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
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "UPDATE visits SET reverted_at = ?1, reverted_by_run_id = NULL WHERE import_batch_id = ?2",
            params![now_rfc3339(), batch.id],
        )
        .expect("break visibility");

    let repair = repair_health_issues(&paths, &config, None).expect("repair health");

    assert_eq!(repair.repaired_import_audits, 0);
    assert_eq!(repair.repaired_visibility_rows, 1);
    assert!(repair.notes.iter().any(|note| note.contains("Re-linked 1 reverted visit rows")));
}

#[test]
fn doctor_repair_commits_rebuilt_import_artifacts_when_git_is_enabled() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let import_config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &import_config, None).expect("init archive");

    let takeout_source = seed_takeout_fixture(dir.path());
    let inspection = crate::takeout::import_takeout(
        &paths,
        &import_config,
        None,
        &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    let batch = inspection.import_batch.expect("batch");
    fs::remove_file(batch.audit_path.expect("audit path")).expect("remove import audit artifact");
    let repair_config = AppConfig { git_enabled: true, ..import_config };

    let repair = repair_health_issues(&paths, &repair_config, None).expect("repair health");

    assert_eq!(repair.repaired_import_audits, 1);
    assert!(
        repair
            .notes
            .iter()
            .any(|note| note.contains("Recorded repaired import artifacts in audit commit"))
    );
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let git_commit: String = connection
        .query_row("SELECT git_commit FROM import_batches WHERE id = ?1", [batch.id], |row| {
            row.get(0)
        })
        .expect("git commit");
    assert!(!git_commit.is_empty());
}

#[test]
fn doctor_repair_records_failed_run_when_audit_artifact_rewrite_fails() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    create_schema(&connection).expect("schema");
    connection
        .execute(
            "INSERT INTO import_batches (source_kind, source_path, profile_id, created_at, imported_at, status, summary_json, audit_path)
             VALUES ('takeout', '/tmp/takeout', 'takeout::browser-history', ?1, ?1, 'imported', '{}', NULL)",
            [now_rfc3339()],
        )
        .expect("insert import batch without audit path");
    fs::create_dir_all(&paths.audit_repo_path).expect("audit repo dir");
    fs::write(paths.audit_repo_path.join("imports"), "not a directory")
        .expect("block audit imports path");

    let error = repair_health_issues(&paths, &config, None)
        .expect_err("blocked audit repo should fail repair");

    assert!(!error.to_string().is_empty());
    let failed_runs: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE run_type = 'doctor' AND status = 'failed'",
            [],
            |row| row.get(0),
        )
        .expect("failed doctor run count");
    assert_eq!(failed_runs, 1);
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
fn snapshot_restore_records_failed_run_when_replay_cannot_persist() {
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
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let snapshot_path: String = connection
        .query_row(
            "SELECT file_path
             FROM snapshots
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("latest snapshot path");
    connection.execute("DROP TABLE visits", []).expect("damage archive schema");
    drop(connection);

    let restore_error =
        run_snapshot_restore(&paths, &config, None, &SnapshotRestoreRequest { snapshot_path })
            .expect_err("damaged archive should fail snapshot restore");
    let restore_error_chain = format!("{restore_error:#}");
    assert!(restore_error_chain.contains("visits"), "{restore_error_chain}");

    let failed_status: String = Connection::open(&paths.archive_database_path)
        .expect("reopen archive")
        .query_row(
            "SELECT status
             FROM runs
             WHERE run_type = 'snapshot_restore'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("failed restore run");
    assert_eq!(failed_status, "failed");

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
fn maintenance_guards_manual_snapshots_and_retention_edge_cases() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");

    let safety_snapshot = paths.raw_snapshots_dir.join("manual-safety.sqlite");
    fs::create_dir_all(&paths.raw_snapshots_dir).expect("create snapshot dir");
    fs::write(&safety_snapshot, "manual safety copy").expect("write safety snapshot");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    connection
        .execute(
            "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (90, 'backup', 'manual', ?1, 'UTC', 'success', '[\"profile-a\"]', '[]', '{}', 0)",
            params![now_rfc3339()],
        )
        .expect("insert run");
    connection
        .execute(
            "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
             VALUES (90, ?1, 18, 'manual', 'safety-copy', ?2)",
            params![safety_snapshot.display().to_string(), now_rfc3339()],
        )
        .expect("insert safety snapshot");

    let preview = preview_snapshot_restore(
        &paths,
        &config,
        None,
        &SnapshotRestoreRequest { snapshot_path: safety_snapshot.display().to_string() },
    )
    .expect("manual safety snapshot preview");
    assert_eq!(preview.snapshot_kind, "archive-safety-snapshot");
    assert!(!preview.execute_supported);
    assert_eq!(preview.source_profile_id.as_deref(), Some("profile-a"));
    assert!(preview.warnings[0].contains("manual recovery"));
    let run_error = run_snapshot_restore(
        &paths,
        &config,
        None,
        &SnapshotRestoreRequest { snapshot_path: safety_snapshot.display().to_string() },
    )
    .expect_err("manual safety copies are not automatically restored");
    assert!(run_error.to_string().contains("automatic restore"));

    let uninitialized = AppConfig { initialized: false, ..AppConfig::default() };
    let restore_uninitialized = run_snapshot_restore(
        &paths,
        &uninitialized,
        None,
        &SnapshotRestoreRequest { snapshot_path: safety_snapshot.display().to_string() },
    )
    .expect_err("uninitialized archive cannot restore snapshots");
    assert!(restore_uninitialized.to_string().contains("archive has not been initialized"));

    let prune_error = run_retention_prune(
        &paths,
        &uninitialized,
        None,
        &RetentionPruneRequest { bucket_ids: vec!["exports".to_string()] },
    )
    .expect_err("uninitialized archive cannot prune");
    assert!(prune_error.to_string().contains("initialize the archive"));

    let empty = run_retention_prune(
        &paths,
        &config,
        None,
        &RetentionPruneRequest { bucket_ids: Vec::new() },
    )
    .expect("empty prune request");
    assert!(empty.run_id.is_none());
    assert!(empty.warnings[0].contains("Choose at least one"));

    let unknown = run_retention_prune(
        &paths,
        &config,
        None,
        &RetentionPruneRequest { bucket_ids: vec!["not-a-bucket".to_string()] },
    )
    .expect("unknown bucket prune request");
    assert!(unknown.run_id.is_none());
    assert!(unknown.warnings[0].contains("No matching"));

    fs::create_dir_all(&paths.exports_dir).expect("exports dir");
    fs::create_dir_all(&paths.staging_dir).expect("staging dir");
    fs::create_dir_all(&paths.quarantine_dir).expect("quarantine dir");
    fs::write(paths.exports_dir.join("export.json"), "{}").expect("export file");
    fs::write(paths.staging_dir.join("stage.tmp"), "stage").expect("staging file");
    fs::write(paths.quarantine_dir.join("bad.txt"), "bad").expect("quarantine file");
    let all = run_retention_prune(
        &paths,
        &config,
        None,
        &RetentionPruneRequest {
            bucket_ids: vec![
                "snapshots".to_string(),
                "exports".to_string(),
                "staging".to_string(),
                "quarantine".to_string(),
            ],
        },
    )
    .expect("all retention buckets");
    assert!(all.run_id.is_some());
    assert!(all.deleted_files >= 4);
    assert_eq!(directory_size(&paths.exports_dir), 0);
    assert_eq!(directory_size(&paths.staging_dir), 0);
    assert_eq!(directory_size(&paths.quarantine_dir), 0);
}

#[test]
fn rekey_archive_keeps_a_safety_snapshot() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    fs::write(paths.archive_database_path.with_extension("rekey.sqlite"), "stale rekey temp")
        .expect("write stale rekey temp");
    fs::write(paths.archive_database_path.with_extension("backup.sqlite"), "stale rekey backup")
        .expect("write stale rekey backup");

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
    assert!(!paths.archive_database_path.with_extension("rekey.sqlite").exists());
    assert!(!paths.archive_database_path.with_extension("backup.sqlite").exists());

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

    let plaintext_status = rekey_archive(
        &paths,
        &encrypted_config,
        Some("vault-passphrase"),
        ArchiveMode::Plaintext,
        None,
    )
    .expect("rekey back to plaintext");
    assert!(!plaintext_status.encrypted);
}

#[test]
fn rekey_archive_reports_missing_database_and_missing_new_key() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };

    let missing_database = rekey_archive(&paths, &config, None, ArchiveMode::Plaintext, None)
        .expect_err("missing archive database");
    assert!(missing_database.to_string().contains("archive database does not exist"));

    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let missing_key = rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, None)
        .expect_err("missing new encryption key");
    assert!(missing_key.to_string().contains("new encryption key is required"));
}

#[test]
fn rekey_archive_records_failed_run_when_config_save_fails_after_swap() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    fs::remove_file(&paths.config_path).expect("remove config file");
    fs::create_dir(&paths.config_path).expect("replace config path with directory");

    let error = rekey_archive(&paths, &config, None, ArchiveMode::Plaintext, None)
        .expect_err("config save failure should abort rekey closeout");
    assert!(error.to_string().contains("writing"));

    let connection =
        Connection::open(&paths.archive_database_path).expect("open archive after swap");
    let (status, error_message): (String, Option<String>) = connection
        .query_row(
            "SELECT status, error_message
             FROM runs
             WHERE run_type = 'rekey'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("failed rekey run");
    assert_eq!(status, "failed");
    assert!(error_message.as_deref().is_some_and(|message| message.contains("writing")));
}

#[test]
fn run_support_failed_runs_and_due_windows_stay_truthful() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig { initialized: true, due_after_hours: 72, ..AppConfig::default() };
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection = Connection::open(&paths.archive_database_path).expect("open archive");
    let started_at = now_rfc3339();
    connection
        .execute(
            "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'manual', ?1, 'UTC', 'running', '[\"profile-a\"]', '[]', '{}', 0)",
            params![started_at],
        )
        .expect("insert running run");
    let run_id = connection.last_insert_rowid();

    finalize_failed_run(
        &connection,
        run_id,
        &[
            BackupProfileSummary {
                profile_id: "profile-a".to_string(),
                new_visits: 2,
                new_urls: 1,
                new_downloads: 0,
                checkpoint_created: true,
                notes: vec!["partial".to_string()],
            },
            BackupProfileSummary {
                profile_id: "profile-b".to_string(),
                new_visits: 3,
                new_urls: 2,
                new_downloads: 1,
                checkpoint_created: false,
                notes: Vec::new(),
            },
        ],
        &["warning".to_string()],
        &anyhow::anyhow!("fixture failure"),
    )
    .expect("finalize failed run");

    let (status, stats_json, warnings_json, error_message): (String, String, String, String) =
        connection
            .query_row(
                "SELECT status, stats_json, warnings_json, error_message FROM runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("failed run row");
    let stats: serde_json::Value = serde_json::from_str(&stats_json).expect("stats json");
    let warnings: Vec<String> = serde_json::from_str(&warnings_json).expect("warnings json");
    assert_eq!(status, "failed");
    assert_eq!(stats["profilesProcessed"], 2);
    assert_eq!(stats["newVisits"], 5);
    assert_eq!(stats["newUrls"], 3);
    assert_eq!(stats["newDownloads"], 1);
    assert_eq!(warnings, vec!["warning"]);
    assert!(error_message.contains("fixture failure"));

    let now = chrono::Utc::now();
    let recent = now - chrono::Duration::hours(2);
    let old = now - chrono::Duration::hours(96);
    let recent_reason = super::run_support::backup_due_skip_reason_at(recent, &config, now)
        .expect("recent backup should skip");
    assert!(recent_reason.contains("2 hours old"));
    assert!(super::run_support::backup_due_skip_reason_at(old, &config, now).is_none());

    connection
        .execute(
            "UPDATE runs
             SET status = 'success', finished_at = ?1, error_message = NULL
             WHERE id = ?2",
            params![recent.to_rfc3339(), run_id],
        )
        .expect("mark successful backup");
    let due_reason = super::run_support::backup_due_skip_reason(&connection, &config)
        .expect("due skip query")
        .expect("recent success should skip");
    assert!(due_reason.contains("hours old"));
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
