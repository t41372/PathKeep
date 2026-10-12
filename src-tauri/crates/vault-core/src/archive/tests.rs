//! Regression tests for the canonical archive domain.
use super::*;
use crate::{
    config::{ProjectPaths, project_paths_with_root},
    models::{BrowserProfile, SnapshotRestoreRequest, TakeoutRequest},
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

fn seed_lexical_archive(vault: &crate::Vault) {
    let rows = [
        (
            1,
            "https://example.test/preferences",
            "瀏覽器設定中心",
            1_000,
            "2026-05-01T00:00:01+00:00",
        ),
        (
            2,
            "https://example.test/simplified",
            "浏览器设定说明",
            2_000,
            "2026-05-01T00:00:02+00:00",
        ),
        (
            3,
            "https://example.test/github-actions",
            "GitHub Actions manual",
            3_000,
            "2026-05-01T00:00:03+00:00",
        ),
        (4, "https://github.com/releases", "Release notes", 4_000, "2026-05-01T00:00:04+00:00"),
        (
            5,
            "https://example.test/git-hub-guide",
            "Git Hub spacing guide",
            5_000,
            "2026-05-01T00:00:05+00:00",
        ),
        (
            6,
            "https://www.youtube.com/watch?v=recall",
            "YouTube watch queue",
            6_000,
            "2026-05-01T00:00:06+00:00",
        ),
        (
            7,
            "https://example.test/reviews/pull-request",
            "Pull Request review checklist",
            7_000,
            "2026-05-01T00:00:07+00:00",
        ),
    ];

    let mut history = crate::test_support::history::History::chrome(vault);
    for (id, url, title, visit_time, _iso) in rows {
        history = history.page(id, url, Some(title), visit_time);
    }
    history.commit();
}

/// Publishes one GitHub-repo enrichment's search text for the page of visit `history_id`, the way
/// enrichment does, so a lexical search can surface the excerpt. `summary` doubles as the GitHub
/// `description` and `topics` carries an extra searchable keyword.
fn store_lexical_enrichment(vault: &crate::Vault, history_id: i64, summary: &str) {
    let extraction = format!(
        r#"{{"fullName":"o/r","description":"{summary}","topics":["enrichkw"],"language":"Rust"}}"#
    );
    let text = crate::archive::enrichment_text_for_index(Some(summary), Some(&extraction));
    let page = vault
        .read(vault_store::Db::Archive, move |s| {
            Ok(s.query_one(
                "SELECT u.page_id FROM visits v JOIN urls u ON u.id=v.url_id WHERE v.id=?1",
                [history_id],
                |r| Ok(r.get::<i64>(0)?),
            )?)
        })
        .expect("seeded page");
    crate::archive::set_enrichment_text(vault, page, &text).expect("enrichment text");
    vault.search().cycle().expect("index");
}

#[test]
fn plain_browse_history_query_leaves_enrichment_excerpt_none() {
    let vault = crate::Vault::open_temp().unwrap();
    seed_lexical_archive(&vault);
    // Enrichment exists for a row, but a non-search browse query must NOT attach any excerpt: the
    // excerpt is a lexical-search-only affordance, so browse rows stay None regardless.
    store_lexical_enrichment(&vault, 3, "Reusable workflow runner for CI pipelines");
    let browse = list_history(&vault, HistoryQuery { limit: Some(50), ..HistoryQuery::default() })
        .expect("plain browse query");
    assert!(browse.total >= 7, "the browse path returns the full seeded set");
    assert!(
        browse.items.iter().all(|entry| entry.enrichment_excerpt.is_none()),
        "browse rows must never carry an enrichment excerpt"
    );
}

#[test]
fn uncounted_history_pages_walk_the_same_rows_as_counted_pages() {
    // History's infinite list sends `includeTotal: false` so a page never pays an archive-wide
    // COUNT. Walking with the cursor must still reach every row exactly once and stop cleanly.
    let vault = crate::Vault::open_temp().unwrap();
    seed_lexical_archive(&vault);

    let walk = |q: Option<&str>| {
        let mut seen = Vec::new();
        let mut cursor = None;
        loop {
            let page = list_history(
                &vault,
                HistoryQuery {
                    q: q.map(str::to_string),
                    limit: Some(3),
                    cursor: cursor.clone(),
                    include_total: Some(false),
                    ..HistoryQuery::default()
                },
            )
            .expect("uncounted page");
            assert!(!page.total_exact, "an uncounted page must say its total is not exact");
            assert_eq!(page.total, page.items.len(), "the total is only this page's size");
            assert_eq!(page.has_previous, cursor.is_some());
            seen.extend(page.items.iter().map(|entry| entry.url.clone()));
            if !page.has_next {
                assert!(page.next_cursor.is_none());
                break seen;
            }
            cursor = Some(page.next_cursor.expect("a page with more rows carries a cursor"));
        }
    };

    let counted = list_history(&vault, HistoryQuery { limit: Some(50), ..HistoryQuery::default() })
        .expect("counted browse");
    assert!(counted.total_exact);
    let browse = walk(None);
    assert_eq!(browse, counted.items.iter().map(|entry| entry.url.clone()).collect::<Vec<_>>());

    let keyword = walk(Some("example"));
    let counted_keyword = list_history(
        &vault,
        HistoryQuery { q: Some("example".to_string()), limit: Some(50), ..HistoryQuery::default() },
    )
    .expect("counted keyword");
    assert_eq!(keyword.len(), counted_keyword.total);
    let mut unique = keyword.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), keyword.len(), "no row may appear on two pages");
}

#[test]
fn canonical_backup_pipeline_writes_runs_manifests_snapshots_and_queries() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };

    let mut progress_events = Vec::new();
    let report = run_backup_with_progress(&vault.with_config(config.clone()), false, |event| {
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

    vault.store().read(vault_store::Db::Archive,|db| {
        for (sql,expected) in [
            ("SELECT COUNT(*) FROM visits",2), ("SELECT COUNT(*) FROM urls",1),
            ("SELECT COUNT(*) FROM downloads",1),("SELECT COUNT(*) FROM search_terms",1),
            ("SELECT COUNT(*) FROM favicon_blobs",1),("SELECT COUNT(*) FROM favicons WHERE image_blob_hash IS NOT NULL",2),
            ("SELECT COUNT(*) FROM search_documents WHERE title='Archive docs' AND search_terms='deep recall token'",1),
            ("SELECT visible_visit_count FROM search_document_profiles",2),
            ("SELECT COUNT(*) FROM pages p WHERE NOT EXISTS(SELECT 1 FROM search_outbox o WHERE o.page_id=p.id)",0),
        ] {assert_eq!(db.query_one(sql,(),|r|Ok(r.get::<i64>(0)?))?,expected,"{sql}");}
        assert_eq!(db.query_one("SELECT status FROM runs WHERE run_type='backup'",(),|r|Ok(r.get::<String>(0)?))?,"success");
        Ok(())
    }).unwrap();
    let report_again = run_backup(&vault.with_config(config), false).unwrap();
    assert_eq!(report_again.run.unwrap().new_visits, 0);
    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn backup_guards_initialization_selection_and_due_skip_before_profile_work() {
    let vault = crate::Vault::open_temp().unwrap();
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    let uninitialized = run_backup(&vault.with_config(config.clone()), false)
        .expect_err("uninitialized archive should fail");
    assert!(uninitialized.to_string().contains("archive has not been initialized"));

    let config = AppConfig { initialized: true, ..AppConfig::default() };
    crate::config::ensure_paths(&paths).expect("paths");
    let no_selection = run_backup(&vault.with_config(config.clone()), false)
        .expect_err("empty selected profiles should fail");
    assert!(
        format!("{no_selection:#}").contains("select at least one readable browser profile"),
        "{no_selection:#}"
    );

    vault.store().write(vault_store::Db::Archive,vault_store::Priority::Interactive,|s| {
        s.execute("INSERT INTO runs(run_type,trigger,started_at,finished_at,status) VALUES('backup','manual',?1,?1,'success')",[chrono::Utc::now().to_rfc3339()])?;Ok(())
    }).unwrap();
    let skipped =
        run_backup(&vault.with_config(config.clone()), true).expect("due-only backup skip");
    assert!(skipped.due_skipped);
    assert_eq!(skipped.reason_code.as_deref(), Some("not-due"));
    assert!(skipped.reason.as_deref().is_some_and(|reason| reason.contains("minutes old")));
    let skipped_run = skipped.run.expect("due skip must have unified runs evidence");
    assert_eq!(skipped_run.status, "skipped");
    assert_eq!(skipped_run.trigger, "schedule");
    let connection = crate::test_support::store::Probe::new(&vault);
    let persisted: (String, String) = connection
        .query_row(
            "SELECT status, json_extract(stats_json, '$.reasonCode') FROM runs WHERE id = ?1",
            [skipped_run.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("persisted skip row");
    assert_eq!(persisted, ("skipped".to_string(), "not-due".to_string()));
}

#[test]
fn remove_path_except_skips_a_nonexistent_path() {
    // The `!path.exists()` defensive arm of the D3 last-good-keep recursion: a path that does not
    // exist removes nothing and counts nothing.
    assert_eq!(
        remove_path_except(Path::new("/nonexistent/pathkeep-x"), None).expect("missing path"),
        (0, 0),
    );
}

#[test]
fn remove_directory_contents_except_skips_a_missing_root() {
    // The `!root.exists()` guard of the D3 last-good-keep helper: a missing root removes nothing.
    assert_eq!(
        remove_directory_contents_except(Path::new("/nonexistent/pathkeep-root"), None)
            .expect("missing root"),
        (0, 0),
    );
}

#[test]
fn backup_rejects_selected_profiles_that_are_not_readable() {
    let vault = crate::Vault::open_temp().unwrap();
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
    crate::config::ensure_paths(&paths).expect("paths");

    let error = run_backup(&vault.with_config(config.clone()), false)
        .expect_err("unreadable selected profile should fail");

    assert!(error.to_string().contains("selected profiles are not readable"));

    // A backup tool must never fail silently: the failure is RECORDED as a `failed` run AND carries
    // its reason, so it shows in the run history (it used to bail before any run row was written).
    vault.store().read(vault_store::Db::Archive,|s| {
        let (kind,status,reason)=s.query_one("SELECT run_type,status,error_message FROM runs WHERE id>0 ORDER BY id DESC LIMIT 1",(),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?)))?;
        assert_eq!((kind.as_str(),status.as_str()),("backup","failed"));
        assert!(reason.contains("not readable"));Ok(())
    }).unwrap();

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn classify_browser_access_error_tags_permission_denial_as_full_disk_access() {
    use std::io::{Error, ErrorKind};

    // A macOS TCC denial (io PermissionDenied) is rewritten into actionable Full Disk Access guidance.
    let denied =
        anyhow::Error::new(Error::new(ErrorKind::PermissionDenied, "Operation not permitted"))
            .context("reading /Users/me/Library/Application Support/Google/Chrome");
    let classified = super::backup::classify_browser_access_error(denied);
    assert!(
        format!("{classified:#}").contains("Full Disk Access"),
        "permission-denied must be tagged with Full Disk Access guidance, got: {classified:#}"
    );

    // A stringified permission error (no downcastable io::Error) is caught by the message fallback.
    let stringified = anyhow::anyhow!("reading the profile: Operation not permitted (os error 1)");
    assert!(
        format!("{:#}", super::backup::classify_browser_access_error(stringified))
            .contains("Full Disk Access")
    );

    // An unrelated failure is left exactly as-is (no spurious Full Disk Access advice).
    let unrelated = super::backup::classify_browser_access_error(anyhow::anyhow!("disk full"));
    assert_eq!(format!("{unrelated:#}"), "disk full");
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
}

#[test]
fn backup_surfaces_a_degraded_staging_fallback_as_a_run_warning() {
    let vault = crate::Vault::open_temp().unwrap();
    // A History captured mid-write (a hot rollback journal) cannot take the online
    // snapshot, so staging falls back to the raw-copy + recover path. The recovered
    // copy still parses (the uncommitted write rolls back), so the backup COMPLETES
    // — but the degraded path must surface as a visible run warning, not be
    // swallowed (the backup-level degraded-staging emit).
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let chrome_root = seed_chrome_fixture(dir.path());
    let history = chrome_root.join("Default").join("History");

    // Freeze a hot rollback journal next to the fixture History (mirrors the
    // chrome staging test's `capture_hot_journal_pair`): flush an uncommitted
    // header write into a journal on a scratch copy, then snapshot the (db,
    // journal) pair over the fixture.
    {
        let scratch = dir.path().join("History.hot");
        fs::copy(&history, &scratch).expect("copy clean history");
        let writer = Connection::open(&scratch).expect("reopen history");
        writer
            .execute_batch("BEGIN IMMEDIATE;\nUPDATE urls SET title = 'in-flight' WHERE id = 1;")
            .expect("start uncommitted write");
        writer.cache_flush().expect("flush dirty pages into the rollback journal");
        let scratch_journal = PathBuf::from(format!("{}-journal", scratch.display()));
        assert!(scratch_journal.exists(), "an uncommitted flush must leave a rollback journal");
        fs::copy(&scratch, &history).expect("freeze hot db over fixture");
        fs::copy(&scratch_journal, PathBuf::from(format!("{}-journal", history.display())))
            .expect("freeze hot journal over fixture");
        writer.execute_batch("ROLLBACK").ok();
    }

    let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
    }

    let paths = sample_paths(dir.path());
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    crate::config::ensure_paths(&paths).expect("paths");

    let report = run_backup(&vault.with_config(config.clone()), false);
    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
    let report = report.expect("backup completes after recovering the hot-journal history");

    assert!(
        report.warnings.iter().any(|warning| warning.contains("recovered file copy")),
        "a recovered hot-journal staging must be reported as a run warning: {:?}",
        report.warnings
    );
}

#[test]
fn backup_marks_run_failed_when_readable_profile_cannot_be_staged() {
    let vault = crate::Vault::open_temp().unwrap();
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
        git_enabled: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    crate::config::ensure_paths(&paths).expect("paths");

    let error =
        run_backup(&vault.with_config(config.clone()), false).expect_err("staging should fail");
    let status = crate::test_support::store::Probe::new(&vault)
        .query_row("SELECT status FROM runs ORDER BY id DESC LIMIT 1", (), |row| {
            Ok(row.get::<String>(0)?)
        })
        .expect("run status");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());

    assert!(format!("{error:#}").contains("staging profile chrome:Default"));
    assert_eq!(status, "failed");
}

#[test]
fn backup_skips_unreadable_selected_profile_when_another_profile_is_readable() {
    let vault = crate::Vault::open_temp().unwrap();
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

    crate::config::ensure_paths(&paths).expect("paths");
    let report = run_backup(&vault.with_config(config.clone()), false)
        .expect("run backup with skipped profile");

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
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let empty_chrome_root = dir.path().join("empty-chrome");
    fs::create_dir_all(&empty_chrome_root).expect("empty chrome root");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let safari_root = seed_safari_fixture(dir.path());
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &empty_chrome_root);
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

    crate::config::ensure_paths(&paths).expect("paths");
    let report =
        run_backup(&vault.with_config(config.clone()), false).expect("run multi-browser backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 2);
    assert_eq!(report.profiles.len(), 2);
    assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("firefox:")));
    assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("safari:")));

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 2);
    assert!(history.iter().any(|entry| entry.starts_with("firefox:")));
    assert!(history.iter().any(|entry| entry.starts_with("safari:")));

    let rerun =
        run_backup(&vault.with_config(config.clone()), false).expect("rerun multi-browser backup");
    assert_eq!(rerun.run.as_ref().expect("rerun").new_visits, 0);
    assert_eq!(rerun.run.as_ref().expect("rerun").new_urls, 0);

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn safari_backup_baseline_ingests_history_without_firefox_or_chrome_dependency() {
    let vault = crate::Vault::open_temp().unwrap();
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

    crate::config::ensure_paths(&paths).expect("paths");
    let report = run_backup(&vault.with_config(config.clone()), false).expect("run safari backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 1);
    assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "safari:default");

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0], "safari:default");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn backup_keeps_chrome_successful_when_selected_safari_is_unreadable() {
    let vault = crate::Vault::open_temp().unwrap();
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

    crate::config::ensure_paths(&paths).expect("paths");
    let report = run_backup(&vault.with_config(config.clone()), false).expect("run backup");
    assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "chrome:Default");
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("safari:default")
            && warning.contains("grant Full Disk Access before the next backup")
    }));

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|entry| entry.starts_with("chrome:")));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
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
    assert_eq!(report.profiles.len(), 1);
    assert_eq!(report.profiles[0].profile_id, "chrome:Default");
    assert!(report.warnings.iter().any(|warning| {
        warning.contains("safari:default")
            && warning.contains("grant Full Disk Access before the next backup")
    }));

    let history = vault.store().read(vault_store::Db::Archive,|s| s.query("SELECT p.profile_key FROM visits v JOIN source_profiles p ON p.id=v.source_profile_id",(),|r|Ok(r.get::<String>(0)?))).unwrap();
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|entry| entry.starts_with("chrome:")));

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn dashboard_snapshot_tracks_cached_totals_across_import_visibility_changes() {
    let vault = crate::Vault::open_temp().unwrap();
    // The search outbox's high-water mark: the background indexer empties the outbox whenever it
    // catches up, but the mark only rises, so it shows each step queued search work.
    let queued = |vault: &crate::Vault| {
        vault
            .store()
            .read(vault_store::Db::Archive, |s| {
                s.query_one(
                    "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='search_outbox'),0)",
                    (),
                    |r| Ok(r.get::<i64>(0)?),
                )
            })
            .unwrap()
    };
    let mut before = queued(&vault);
    let source = seed_takeout_fixture(&vault.paths().app_root);
    let imported = crate::import_takeout(
        &vault,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .unwrap();
    let batch = imported.import_batch.unwrap().id;
    for (step, expected) in [("import", 1), ("rollback", 0), ("restore", 1)] {
        if step == "rollback" {
            crate::revert_import_batch(&vault, batch).unwrap();
        }
        if step == "restore" {
            crate::restore_import_batch(&vault, batch).unwrap();
        }
        let after = queued(&vault);
        assert!(after > before, "{step} queues search work");
        before = after;
        vault.store().read(vault_store::Db::Archive,move |s| {
            let stats=s.query_one("SELECT stats_json FROM runs WHERE run_type=?1 ORDER BY id DESC LIMIT 1",[step],|r|Ok(r.get::<String>(0)?))?;
            let stats:Value=serde_json::from_str(&stats).unwrap();
            assert_eq!(stats["totalVisits"],expected);assert_eq!(stats["totalUrls"],1);
            assert_eq!(s.query_one("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",(),|r|Ok(r.get::<i64>(0)?))?,expected);
            assert_eq!(s.query_one("SELECT COALESCE(SUM(visible_visit_count),0) FROM search_document_profiles",(),|r|Ok(r.get::<i64>(0)?))?,expected);
            assert_eq!(s.query_one("SELECT COUNT(*) FROM search_documents WHERE title LIKE '%Imported%' AND newest_visit_ms IS NOT NULL",(),|r|Ok(r.get::<i64>(0)?))?,expected);
            Ok(())
        }).unwrap();
    }
}

#[test]
fn snapshot_restore_preview_and_run_record_the_saved_checkpoint() {
    let vault = crate::Vault::open_temp().unwrap();
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
        git_enabled: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };

    crate::config::ensure_paths(&paths).expect("paths");
    let backup = run_backup(&vault.with_config(config.clone()), false).expect("run backup");
    let snapshot_path: String = crate::test_support::store::Probe::new(&vault)
        .query_row(
            "SELECT file_path
             FROM snapshots
             ORDER BY id DESC
             LIMIT 1",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("latest snapshot path");

    let recorded = vault
        .store()
        .read(vault_store::Db::Archive, |s| {
            s.query_one("SELECT created_at FROM snapshots ORDER BY id LIMIT 1", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })
        .unwrap();
    let unrecorded = dir.path().join("unrecorded");
    fs::create_dir_all(&unrecorded).unwrap();
    assert!(
        run_snapshot_restore(
            &vault,
            &SnapshotRestoreRequest { snapshot_path: unrecorded.display().to_string() }
        )
        .unwrap_err()
        .to_string()
        .contains("recorded ledger")
    );
    let restored = run_snapshot_restore(
        &vault,
        &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
    )
    .expect("run snapshot restore");
    let restore_run = restored.run.expect("restore run");
    assert_eq!(restore_run.run_type, "snapshot_restore");
    assert_eq!(restore_run.status, "success");
    assert!(backup.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));

    let row = vault
        .store()
        .read(vault_store::Db::Archive, move |s| {
            s.query_one(
                "SELECT reason,created_at FROM snapshots WHERE run_id=?1",
                [restore_run.id],
                |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
            )
        })
        .unwrap();
    assert_eq!(row, ("restored-source-checkpoint".into(), recorded));
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(restored.manifest_path.unwrap()).unwrap())
            .unwrap();
    assert!(!manifest["sourceHashes"]["chrome:Default"].as_object().unwrap().is_empty());

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
}

#[test]
fn snapshot_restore_records_failed_run_when_replay_cannot_persist() {
    let vault = crate::Vault::open_temp().unwrap();
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
        git_enabled: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    crate::config::ensure_paths(&paths).expect("paths");
    run_backup(&vault.with_config(config.clone()), false).expect("run backup");
    let connection = crate::test_support::store::Probe::new(&vault);
    let snapshot_path: String = connection
        .query_row(
            "SELECT file_path
             FROM snapshots
             ORDER BY id DESC
             LIMIT 1",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("latest snapshot path");
    let manifests = &vault.paths().manifests_dir;
    let saved_manifests = manifests.with_file_name("saved-manifests");
    fs::rename(manifests, &saved_manifests).unwrap();
    fs::write(manifests, b"prevent manifest persistence").unwrap();
    let error = run_snapshot_restore(
        &vault,
        &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
    )
    .unwrap_err();
    assert!(!error.to_string().is_empty());
    let failed_stats: String = connection.query_row("SELECT stats_json FROM runs WHERE run_type='snapshot_restore' ORDER BY id DESC LIMIT 1", (), |r| Ok(r.get(0)?)).unwrap();
    let failed_stats: Value = serde_json::from_str(&failed_stats).unwrap();
    assert_eq!(failed_stats["profilesProcessed"], 1);
    assert_eq!(failed_stats["profiles"].as_array().unwrap().len(), 1);
    let visible: i64 = connection
        .query_row("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL", (), |r| Ok(r.get(0)?))
        .unwrap();
    assert_eq!(
        failed_stats["processedRecords"], visible,
        "checkpoint replay retains its processed counts when the manifest fails"
    );
    assert_eq!(failed_stats["newVisits"], 0);
    fs::remove_file(manifests).unwrap();
    fs::rename(saved_manifests, manifests).unwrap();
    // One visit batch commits before a corrupt later source row aborts replay.
    let source = Connection::open(Path::new(&snapshot_path).join("History")).unwrap();
    source.execute_batch("WITH RECURSIVE seq(id) AS (SELECT 100 UNION ALL SELECT id+1 FROM seq WHERE id<10100)
        INSERT INTO visits(id,url,visit_time,transition) SELECT id,1,13390000000000000+id,0 FROM seq;
        UPDATE visits SET visit_time='bad timestamp' WHERE id=10100;").unwrap();
    drop(source);
    assert!(
        run_snapshot_restore(
            &vault,
            &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() }
        )
        .is_err()
    );
    let stats:String=connection.query_row("SELECT stats_json FROM runs WHERE run_type='snapshot_restore' ORDER BY id DESC LIMIT 1",(),|r|Ok(r.get(0)?)).unwrap();
    let stats: Value = serde_json::from_str(&stats).unwrap();
    assert_eq!(stats["processedRecords"], 10000);
    assert_eq!(stats["profilesProcessed"], 1);
    assert_eq!(stats["profiles"][0]["newVisits"], stats["newVisits"]);
    assert!(stats["newVisits"].as_u64().unwrap() > 0);
    vault
        .store()
        .write(vault_store::Db::Archive, vault_store::Priority::Interactive, |s| {
            s.execute("DROP TABLE visits", ())?;
            Ok(())
        })
        .expect("damage archive schema");
    drop(connection);

    let restore_error = run_snapshot_restore(&vault, &SnapshotRestoreRequest { snapshot_path })
        .expect_err("damaged archive should fail snapshot restore");
    let restore_error_chain = format!("{restore_error:#}");
    assert!(restore_error_chain.contains("visits"), "{restore_error_chain}");

    let failed_status: String = crate::test_support::store::Probe::new(&vault)
        .query_row(
            "SELECT status
             FROM runs
             WHERE run_type = 'snapshot_restore'
             ORDER BY id DESC
             LIMIT 1",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("failed restore run");
    assert_eq!(failed_status, "failed");

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
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
fn snapshot_restore_preview_sizes_firefox_and_safari_checkpoints() {
    let vault = crate::Vault::open_temp().unwrap();
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempdir().expect("tempdir");
    let empty_chrome_root = dir.path().join("empty-chrome");
    fs::create_dir_all(&empty_chrome_root).expect("empty chrome root");
    let firefox_profiles = seed_firefox_fixture(dir.path());
    let safari_root = seed_safari_fixture(dir.path());
    let original_chrome = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
    let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
    let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
    unsafe {
        std::env::set_var("CHB_CHROME_USER_DATA_DIR", &empty_chrome_root);
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

    crate::config::ensure_paths(&paths).expect("paths");
    run_backup(&vault.with_config(config.clone()), false).expect("run multi-browser backup");

    let snapshot_paths = vault
        .store()
        .read(vault_store::Db::Archive, |s| {
            s.query("SELECT file_path FROM snapshots ORDER BY id", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    let previews = snapshot_paths
        .iter()
        .map(|snapshot_path| {
            preview_snapshot_restore(
                &vault,
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

    restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_chrome.as_deref());
    restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
    restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
}

#[test]
fn run_support_failed_runs_and_due_windows_stay_truthful() {
    let vault = crate::Vault::open_temp().unwrap();
    let config = AppConfig { initialized: true, due_after_hours: 72.0, ..Default::default() };
    let run_id =
        super::write_runs::start(&vault, "backup", "manual", &["profile-a".into()]).unwrap();
    vault
        .write(vault_store::Db::Archive, vault_store::Priority::Batch, move |s| {
            super::write_runs::record_progress(
                s,
                run_id,
                &[("profilesProcessed", 2), ("newVisits", 5), ("newUrls", 3), ("newDownloads", 1)],
            )?;
            s.execute(
                "UPDATE runs SET warnings_json=?2 WHERE id=?1",
                vault_store::params![
                    run_id,
                    serde_json::to_string(&[crate::models::BackupWarning::opaque("warning")])?
                ],
            )?;
            Ok(())
        })
        .unwrap();
    super::write_runs::end_failed(
        &vault,
        run_id,
        &anyhow::anyhow!("fixture failure"),
        json!({}),
        None,
    );
    let connection = crate::test_support::store::Probe::new(&vault);
    let (status, stats_json, warnings_json, error_message): (String, String, String, String) =
        connection
            .query_row(
                "SELECT status, stats_json, warnings_json, error_message FROM runs WHERE id = ?1",
                [run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("failed run row");
    let stats: serde_json::Value = serde_json::from_str(&stats_json).expect("stats json");
    // Backup runs now persist coded warnings, so the ledger stores objects and
    // the read path is what flattens them back to the message channel.
    let warnings = super::read_models::decode_run_warnings(Some(&warnings_json));
    assert_eq!(status, "failed");
    assert_eq!(stats["profilesProcessed"], 2);
    assert_eq!(stats["newVisits"], 5);
    assert_eq!(stats["newUrls"], 3);
    assert_eq!(stats["newDownloads"], 1);
    assert_eq!(crate::models::backup_warning_messages(&warnings), vec!["warning"]);
    assert_eq!(warnings[0].code, "");
    assert!(error_message.contains("fixture failure"));

    // Ledger rows written before warning code-ification stored bare strings, and
    // non-backup run types still do. Both shapes must stay readable or Audit
    // would silently drop warnings that are already on disk.
    assert!(super::read_models::decode_run_warnings(None).is_empty());
    assert!(super::read_models::decode_run_warnings(Some("not json")).is_empty());
    assert!(super::read_models::decode_run_warnings(Some("{}")).is_empty());
    let mixed = super::read_models::decode_run_warnings(Some(
        r#"["legacy string", {"code":"og-cleanup-failed","message":"cleanup failed"}, 7]"#,
    ));
    assert_eq!(mixed.len(), 2);
    assert_eq!(mixed[0].code, "");
    assert_eq!(mixed[0].message, "legacy string");
    assert_eq!(mixed[1].code, "og-cleanup-failed");
    assert_eq!(mixed[1].message, "cleanup failed");

    let now = chrono::Utc::now();
    let recent = now - chrono::Duration::hours(2);
    let old = now - chrono::Duration::hours(96);
    let recent_reason = super::run_support::backup_due_skip_reason_at(recent, &config, now)
        .expect("recent backup should skip");
    assert!(recent_reason.contains("120 minutes old"));
    assert!(super::run_support::backup_due_skip_reason_at(old, &config, now).is_none());

    let minute_config =
        AppConfig { initialized: true, due_after_hours: 1.5, ..AppConfig::default() };
    let recent_minute_backup = now - chrono::Duration::minutes(89);
    let minute_reason =
        super::run_support::backup_due_skip_reason_at(recent_minute_backup, &minute_config, now)
            .expect("backup younger than 90 minutes should skip");
    assert!(minute_reason.contains("89 minutes old"));

    let invalid_config =
        AppConfig { initialized: true, due_after_hours: 0.0, ..AppConfig::default() };
    let sub_minute_backup = now - chrono::Duration::seconds(30);
    assert!(
        super::run_support::backup_due_skip_reason_at(sub_minute_backup, &invalid_config, now)
            .is_some()
    );

    connection
        .execute(
            "UPDATE runs SET status='success',finished_at=?2,error_message=NULL WHERE id=?1",
            vault_store::params![run_id, recent.to_rfc3339()],
        )
        .unwrap();
    assert!(run_backup(&vault.with_config(config), true).unwrap().due_skipped);
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
    fs::create_dir_all(source_dir.join("Chrome")).expect("create takeout dir");
    fs::write(
        source_dir.join("Chrome/BrowserHistory.json"),
        r#"{"Browser History":[{"url":"https://example.com/import","title":"Imported","time_usec":1775037600000000}]}"#,
    )
    .expect("write takeout fixture");
    source_dir
}

mod native_search;
