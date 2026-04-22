//! Regression tests for the Takeout import boundary.

use super::*;
use crate::{
    archive::{
        create_schema, load_audit_run_detail, load_recent_runs, open_source_evidence_connection,
    },
    config::{ensure_paths, project_paths_with_root},
    models::{AppConfig, ArchiveMode},
};
use std::io::Write;
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

fn chrome_browser_history_payload(records: &[&str]) -> String {
    format!(r#"{{"Browser History":[{}]}}"#, records.join(","))
}

/// Builds temporary project paths for one Takeout test fixture root.
fn sample_paths(root: &Path) -> ProjectPaths {
    project_paths_with_root(root)
}

/// Returns the initialized plaintext config used by most Takeout tests.
fn initialized_plaintext_config() -> AppConfig {
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        git_enabled: false,
        ..AppConfig::default()
    }
}

/// Writes a directory-based Takeout fixture with a chosen folder name.
fn write_takeout_fixture_with_name(dir: &Path, name: &str, lines: &[&str]) -> PathBuf {
    let source_dir = dir.join(name);
    fs::create_dir_all(&source_dir).expect("create takeout source dir");
    let source = source_dir.join("takeout.jsonl");
    fs::write(&source, lines.join("\n")).expect("write takeout fixture");
    source_dir
}

/// Writes the default directory-based Takeout fixture used by core import tests.
fn write_takeout_fixture(dir: &Path) -> PathBuf {
    write_takeout_fixture_with_name(
        dir,
        "takeout-source",
        &[
            r#"{"url":"https://example.com/one","title":"One","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
            r#"{"url":"https://example.com/two","title":"Two","visitedAt":"2026-04-01T11:00:00+00:00"}"#,
        ],
    )
}

/// Writes a directory-based BrowserHistory JSON fixture.
fn write_takeout_browser_json_fixture(dir: &Path, name: &str) -> PathBuf {
    let source_dir = dir.join(name);
    let chrome_dir = source_dir.join("Chrome");
    fs::create_dir_all(&chrome_dir).expect("create browser json source dir");
    fs::write(
        chrome_dir.join("BrowserHistory.json"),
        chrome_browser_history_payload(&[
            r#"{"url":"https://example.com/one","title":"One","time_usec":1711965600000000,"client_id":"alpha"}"#,
            r#"{"url":"https://example.com/two","title":"Two","time_usec":1711969200000000,"client_id":"beta"}"#,
        ]),
    )
    .expect("write browser history json");
    source_dir
}

/// Writes a zipped Takeout fixture with named entries for zip-path coverage.
fn write_takeout_zip(dir: &Path, entries: &[(&str, &str)]) -> PathBuf {
    let zip_path = dir.join("takeout.zip");
    let file = fs::File::create(&zip_path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for (name, contents) in entries {
        zip.start_file(name, options).expect("start zip entry");
        zip.write_all(contents.as_bytes()).expect("write zip entry");
    }
    zip.finish().expect("finish zip");
    zip_path
}

#[test]
fn inspect_takeout_collects_preview_rows() {
    let dir = tempdir().expect("tempdir");
    let source = write_takeout_fixture(dir.path());
    let inspection = inspect_takeout(
        &sample_paths(dir.path()),
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
    )
    .expect("inspect");

    assert_eq!(inspection.source_path, source.display().to_string());
    assert!(inspection.dry_run);
    assert_eq!(inspection.candidate_items, 2);
    assert_eq!(inspection.preview_entries.len(), 2);
    assert_eq!(inspection.recognized_files.len(), 1);
    assert_eq!(inspection.recognized_files[0].status, "previewed");
    assert_eq!(inspection.recognized_files[0].records, 2);
    assert!(inspection.quarantined_files.is_empty());
    assert!(inspection.notes.is_empty());
}

#[test]
fn inspect_takeout_streams_browser_history_json_payloads() {
    let dir = tempdir().expect("tempdir");
    let source = write_takeout_browser_json_fixture(dir.path(), "takeout-browser-json");
    let inspection = inspect_takeout(
        &sample_paths(dir.path()),
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
    )
    .expect("inspect browser history json");

    assert_eq!(inspection.source_path, source.display().to_string());
    assert!(inspection.dry_run);
    assert_eq!(inspection.candidate_items, 2);
    assert_eq!(inspection.preview_entries.len(), 2);
    assert_eq!(inspection.preview_entries[0].url, "https://example.com/one");
    assert_eq!(inspection.preview_entries[1].url, "https://example.com/two");
    assert_eq!(inspection.recognized_files.len(), 1);
    assert_eq!(inspection.recognized_files[0].kind, "browser-json");
    assert_eq!(inspection.recognized_files[0].records, 2);
    assert!(inspection.notes.is_empty());
}

#[test]
fn inspect_takeout_caps_preview_entries_at_preview_limit() {
    let dir = tempdir().expect("tempdir");
    let lines = (0..=PREVIEW_LIMIT)
        .map(|index| {
            format!(
                "{{\"url\":\"https://example.com/{index}\",\"title\":\"Item {index}\",\"visitedAt\":\"2026-04-01T10:{index:02}:00+00:00\"}}"
            )
        })
        .collect::<Vec<_>>();
    let line_refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
    let source = write_takeout_fixture_with_name(dir.path(), "takeout-many", &line_refs);

    let inspection = inspect_takeout(
        &sample_paths(dir.path()),
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
    )
    .expect("inspect");

    assert_eq!(inspection.candidate_items, PREVIEW_LIMIT + 1);
    assert_eq!(inspection.preview_entries.len(), PREVIEW_LIMIT);
    assert_eq!(inspection.preview_entries[0].url, "https://example.com/0");
    assert_eq!(
        inspection.preview_entries.last().expect("last preview entry").url,
        format!("https://example.com/{}", PREVIEW_LIMIT - 1)
    );
    assert!(
        inspection
            .preview_entries
            .iter()
            .all(|entry| entry.url != format!("https://example.com/{PREVIEW_LIMIT}"))
    );
}

#[test]
fn load_import_batches_returns_empty_for_uninitialized_archives() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = AppConfig::default();
    assert!(load_import_batches(&paths, &config, None).expect("empty import batches").is_empty());

    assert!(!paths.archive_database_path.exists());
    let initialized_but_missing_archive =
        load_import_batches(&paths, &initialized_plaintext_config(), None)
            .expect("missing archive batches");
    assert!(initialized_but_missing_archive.is_empty());
    assert!(!paths.archive_database_path.exists());
}

#[test]
fn import_preview_revert_and_restore_batch_are_reversible() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_takeout_fixture(dir.path());
    let inspection = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import");

    let batch = inspection.import_batch.expect("import batch");
    assert_eq!(batch.candidate_items, 2);
    assert_eq!(batch.imported_items, 2);
    assert_eq!(batch.duplicate_items, 0);
    assert_eq!(inspection.imported_items, 2);
    assert!(inspection.notes.is_empty());
    assert_eq!(inspection.preview_entries.len(), 2);
    assert_eq!(batch.visible_items, 2);
    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs after import");
    assert_eq!(recent_runs[0].run_type, "import");
    assert_eq!(recent_runs[0].profiles_processed, 1);
    assert_eq!(recent_runs[0].new_visits, 2);
    assert_eq!(recent_runs[0].profile_scope, vec!["takeout::browser-history".to_string()]);
    let import_detail =
        load_audit_run_detail(&paths, &config, None, recent_runs[0].id).expect("import run detail");
    assert_eq!(import_detail.run.run_type, "import");
    assert_eq!(import_detail.profile_scope, vec!["takeout::browser-history".to_string()]);

    let preview = preview_import_batch(&paths, &config, None, batch.id).expect("preview batch");
    assert_eq!(preview.preview_entries.len(), 2);
    assert_eq!(preview.batch.status, "imported");
    assert_eq!(preview.batch.candidate_items, 2);
    assert_eq!(preview.batch.imported_items, 2);
    assert_eq!(preview.batch.duplicate_items, 0);

    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    let (profile_name, profile_path, chrome_version): (String, String, String) = archive
        .query_row(
            "SELECT profile_name, profile_path, browser_version
             FROM source_profiles
             WHERE profile_key = 'takeout::browser-history'",
            [],
            |row: &Row<'_>| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("load takeout profile");
    assert_eq!(profile_name, "Imported browser history");
    assert_eq!(profile_path, source.display().to_string());
    assert_eq!(chrome_version, "takeout");

    let reverted = revert_import_batch(&paths, &config, None, batch.id).expect("revert batch");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted.batch.visible_items, 0);
    assert!(reverted.notes.iter().any(|note| note.contains("Soft-hid 2 live history rows")));
    let hidden_rows: i64 = archive
        .query_row(
            "SELECT COUNT(*) FROM visits WHERE import_batch_id = ?1 AND reverted_at IS NOT NULL",
            [batch.id],
            |row| row.get(0),
        )
        .expect("load hidden visit count");
    assert_eq!(hidden_rows, 2);
    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs after revert");
    assert_eq!(recent_runs[0].run_type, "rollback");
    assert_eq!(recent_runs[0].new_visits, 2);

    let restored = restore_import_batch(&paths, &config, None, batch.id).expect("restore batch");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored.batch.visible_items, 2);
    assert!(restored.notes.iter().any(|note| note.contains("Restored at")));
    let visible_rows: i64 = archive
        .query_row(
            "SELECT COUNT(*) FROM visits WHERE import_batch_id = ?1 AND reverted_at IS NULL",
            [batch.id],
            |row| row.get(0),
        )
        .expect("load restored visit count");
    assert_eq!(visible_rows, 2);
    let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs after restore");
    assert_eq!(recent_runs[0].run_type, "restore");
    assert_eq!(recent_runs[0].new_visits, 2);
}

#[test]
fn preview_import_batch_repairs_missing_audit_artifacts() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_takeout_fixture(dir.path());
    let inspection = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import");
    let batch = inspection.import_batch.expect("import batch");
    let audit_path = batch.audit_path.clone().expect("audit path");
    fs::remove_file(&audit_path).expect("remove audit artifact");

    let repaired = preview_import_batch(&paths, &config, None, batch.id).expect("preview batch");
    let repaired_path = repaired.batch.audit_path.expect("repaired audit path");

    assert!(Path::new(&repaired_path).exists());
    assert!(repaired_path.ends_with(".json"));
}

#[test]
fn import_takeout_deduplicates_matching_history_from_different_files() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source_a = write_takeout_fixture_with_name(
        dir.path(),
        "takeout-a",
        &[
            r#"{"url":"https://example.com/one","title":"One","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
        ],
    );
    let source_b = write_takeout_fixture_with_name(
        dir.path(),
        "takeout-b",
        &[
            r#"{"url":"https://example.com/one","title":"One","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
        ],
    );

    let first = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source_a.display().to_string(), dry_run: false },
    )
    .expect("first import");
    let second = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source_b.display().to_string(), dry_run: false },
    )
    .expect("second import");

    assert_eq!(first.imported_items, 1);
    assert_eq!(first.duplicate_items, 0);
    assert_eq!(second.imported_items, 0);
    assert_eq!(second.duplicate_items, 1);

    let history =
        crate::archive::list_history(&paths, &config, None, Default::default()).expect("history");
    assert_eq!(history.total, 1);

    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    let visits: i64 = archive
        .query_row("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL", [], |row: &Row<'_>| {
            row.get(0)
        })
        .expect("visit count");
    assert_eq!(visits, 1);
}

#[test]
fn takeout_records_without_timestamps_are_skipped_with_a_note() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_takeout_fixture_with_name(
        dir.path(),
        "takeout-missing-time",
        &[
            r#"{"url":"https://example.com/valid","title":"Valid","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
            r#"{"url":"https://example.com/missing","title":"Missing"}"#,
        ],
    );

    let dry_run = inspect_takeout(
        &paths,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
    )
    .expect("inspect takeout");
    assert_eq!(dry_run.source_path, source.display().to_string());
    assert!(dry_run.dry_run);
    assert_eq!(dry_run.candidate_items, 1);
    assert_eq!(dry_run.recognized_files.len(), 1);
    assert_eq!(dry_run.recognized_files[0].status, "previewed-with-skips");
    assert_eq!(dry_run.recognized_files[0].records, 1);
    assert_eq!(dry_run.notes.len(), 1);
    assert!(dry_run.notes.iter().any(|note| note.contains("missing a visit timestamp")));

    let imported = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import takeout");
    assert_eq!(imported.imported_items, 1);
    assert_eq!(imported.duplicate_items, 0);
    assert_eq!(
        imported.notes.iter().filter(|note| note.contains("missing a visit timestamp")).count(),
        1
    );
    assert_eq!(imported.recognized_files.len(), 1);
    assert_eq!(imported.recognized_files[0].records, 1);

    let history =
        crate::archive::list_history(&paths, &config, None, Default::default()).expect("history");
    assert_eq!(history.total, 1);
}

#[test]
fn import_takeout_streams_browser_history_json_payloads() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_takeout_browser_json_fixture(dir.path(), "takeout-browser-json");
    let inspection = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import browser history json");

    assert_eq!(inspection.imported_items, 2);
    assert_eq!(inspection.duplicate_items, 0);
    assert_eq!(inspection.recognized_files.len(), 1);
    assert_eq!(inspection.recognized_files[0].kind, "browser-json");
    assert_eq!(inspection.recognized_files[0].records, 2);

    let history =
        crate::archive::list_history(&paths, &config, None, Default::default()).expect("history");
    assert_eq!(history.total, 2);
}

#[test]
fn inspect_takeout_reports_parse_errors_for_recognized_files() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("malformed");
    fs::create_dir_all(source.join("Chrome")).expect("create malformed source");
    fs::write(source.join("Chrome").join("BrowserHistory.json"), "{not-json")
        .expect("write malformed history");

    let inspection = inspect_takeout(
        &sample_paths(dir.path()),
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
    )
    .expect("inspect malformed takeout");

    assert_eq!(inspection.recognized_files.len(), 1);
    assert_eq!(inspection.recognized_files[0].status, "parse-error");
    assert!(inspection.notes.iter().any(|note| note.contains("Could not parse")));
}

#[test]
fn my_activity_exports_stay_in_review_and_do_not_create_empty_batches() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = dir.path().join("takeout-my-activity");
    fs::create_dir_all(source.join("我的活動").join("Chrome")).expect("create activity source");
    fs::write(source.join(".DS_Store"), "noise").expect("write ds_store");
    fs::write(
        source.join("我的活動").join("Chrome").join("我的活動.json"),
        r#"[{"header":"google.com","title":"Visited Example","titleUrl":"https://example.com","time":"2026-04-22T18:30:56.385Z","products":["Chrome"]}]"#,
    )
    .expect("write my activity json");

    let inspection = inspect_takeout(
        &paths,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
    )
    .expect("inspect my activity");
    assert!(inspection.recognized_files.is_empty());
    assert_eq!(inspection.quarantined_files.len(), 1);
    assert_eq!(inspection.quarantined_files[0].reason_code.as_deref(), Some("chrome-my-activity-json"));
    assert_eq!(inspection.quarantined_files[0].detected_locale.as_deref(), Some("zh-tw"));
    assert_eq!(inspection.detected_locale.as_deref(), Some("zh-tw"));

    let imported = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import my activity");
    assert!(imported.import_batch.is_none());
    assert_eq!(imported.imported_items, 0);
    assert_eq!(imported.quarantined_files.len(), 1);
    assert_eq!(load_import_batches(&paths, &config, None).expect("load batches").len(), 0);
}

#[test]
fn recognize_and_parse_takeout_payloads() {
    assert_eq!(recognize_takeout_file("BrowserHistory.json"), Some("browser-json".to_string()));
    assert_eq!(recognize_takeout_file("Chrome/History.json"), Some("browser-json".to_string()));
    assert_eq!(recognize_takeout_file("Chrome/Verlauf.json"), Some("browser-json".to_string()));
    assert_eq!(recognize_takeout_file("entries.jsonl"), Some("jsonl".to_string()));
    assert_eq!(recognize_takeout_file("archive_browser.html"), Some("takeout-index".to_string()));
    assert_eq!(recognize_takeout_file("notes.txt"), None);

    let records = collect_records_from_payload(
        "fixture.json",
        "browser-json",
        chrome_browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000}"#,
        ])
        .as_bytes(),
    )
    .expect("collect");
    assert_eq!(records.records.len(), 1);
    assert_eq!(records.records[0].title.as_deref(), Some("Example"));

    let report = parse_payload_report(
        "fixture.json",
        "browser-json",
        chrome_browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000}"#,
        ])
        .as_bytes(),
    )
    .expect("parse report");
    assert_eq!(report.record_count, 1);
    assert_eq!(report.history.visits.len(), 1);
}

#[test]
fn takeout_helpers_cover_unknown_files_zip_sources_and_quarantine() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");

    let unknown_source = dir.path().join("unknown-only");
    fs::create_dir_all(&unknown_source).expect("create unknown source");
    let unknown_file = unknown_source.join("notes.txt");
    fs::write(&unknown_file, "notes").expect("write unknown file");

    let unknown_inspection = inspect_takeout(
        &paths,
        &TakeoutRequest { source_path: unknown_source.display().to_string(), dry_run: true },
    )
    .expect("inspect unknown");
    assert_eq!(unknown_inspection.recognized_files.len(), 1);
    assert!(unknown_inspection.quarantined_files.is_empty());
    assert!(unknown_inspection.notes.iter().any(|note| note.contains("No directly importable")));

    quarantine_file(&paths, &unknown_source, &unknown_file.display().to_string())
        .expect("quarantine file");
    assert!(paths.quarantine_dir.join("unknown-only").join("notes.txt").exists());

    let zip_source = write_takeout_zip(
        dir.path(),
        &[
            (
                "Chrome/BrowserHistory.json",
                &chrome_browser_history_payload(&[
                    r#"{"url":"https://example.com/zip","title":"Zip","time_usec":1711965600000000}"#,
                ]),
            ),
            ("archive_browser.html", "<html></html>"),
            ("nested/notes.txt", "ignore me"),
        ],
    );
    let files = gather_takeout_files(&zip_source).expect("gather zip files");
    assert_eq!(files.len(), 3);
    let zip_bytes =
        read_zip_entry(&zip_source, "Chrome/BrowserHistory.json").expect("read zip entry");
    assert!(String::from_utf8(zip_bytes).expect("zip utf8").contains("example.com/zip"));

    let zip_inspection = inspect_takeout(
        &paths,
        &TakeoutRequest { source_path: zip_source.display().to_string(), dry_run: true },
    )
    .expect("inspect zip");
    assert_eq!(zip_inspection.candidate_items, 1);
    assert_eq!(zip_inspection.recognized_files.len(), 3);
    assert!(zip_inspection.quarantined_files.is_empty());

    quarantine_takeout_file(
        &paths,
        &zip_source,
        &TakeoutFile { path: "nested/notes.txt".to_string(), from_zip: true },
    )
    .expect("quarantine zip entry");
    assert_eq!(
        fs::read_to_string(paths.quarantine_dir.join("takeout").join("nested").join("notes.txt"))
            .expect("read quarantined zip entry"),
        "ignore me"
    );
}

#[test]
fn takeout_import_guards_and_idempotent_revert_cover_batch_edges() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let archive = open_archive_connection(&paths, &initialized_plaintext_config(), None)
        .expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    assert!(
        load_import_batches(&paths, &initialized_plaintext_config(), None)
            .expect("load batches")
            .is_empty()
    );

    let source = write_takeout_fixture(dir.path());
    let dry_run = import_takeout(
        &paths,
        &initialized_plaintext_config(),
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
    )
    .expect("dry run import");
    assert!(dry_run.import_batch.is_none());
    assert_eq!(dry_run.imported_items, 0);

    let uninitialized_error = import_takeout(
        &paths,
        &AppConfig::default(),
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect_err("uninitialized import should fail");
    assert!(uninitialized_error.to_string().contains("archive must be initialized"));

    let mut git_config = initialized_plaintext_config();
    git_config.git_enabled = true;
    let imported = import_takeout(
        &paths,
        &git_config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import with git");
    let batch_id = imported.import_batch.expect("batch").id;
    let batches = load_import_batches(&paths, &git_config, None).expect("load populated batches");
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].id, batch_id);
    assert_eq!(batches[0].candidate_items, 2);
    assert_eq!(batches[0].imported_items, 2);
    assert_eq!(batches[0].duplicate_items, 0);
    let imported_audit_path = batches[0].audit_path.as_deref().expect("import audit path");
    assert!(!imported_audit_path.is_empty());
    assert!(Path::new(imported_audit_path).exists());
    let imported_git_commit = batches[0].git_commit.as_deref().expect("import git commit");
    assert_eq!(imported_git_commit.len(), 40);
    let reverted = revert_import_batch(&paths, &git_config, None, batch_id).expect("revert");
    let reverted_again =
        revert_import_batch(&paths, &git_config, None, batch_id).expect("revert again");
    assert_eq!(reverted.batch.status, "reverted");
    assert_eq!(reverted_again.batch.status, "reverted");
    let reverted_audit_path =
        reverted_again.batch.audit_path.as_deref().expect("revert audit path");
    assert!(!reverted_audit_path.is_empty());
    assert!(Path::new(reverted_audit_path).exists());
    let reverted_git_commit =
        reverted_again.batch.git_commit.as_deref().expect("revert git commit");
    assert_eq!(reverted_git_commit.len(), 40);

    let restored =
        restore_import_batch(&paths, &git_config, None, batch_id).expect("restore batch");
    let restored_again =
        restore_import_batch(&paths, &git_config, None, batch_id).expect("restore again");
    assert_eq!(restored.batch.status, "imported");
    assert_eq!(restored_again.batch.status, "imported");
    let restored_audit_path =
        restored_again.batch.audit_path.as_deref().expect("restore audit path");
    assert!(!restored_audit_path.is_empty());
    assert!(Path::new(restored_audit_path).exists());
    let restored_git_commit =
        restored_again.batch.git_commit.as_deref().expect("restore git commit");
    assert_eq!(restored_git_commit.len(), 40);

    let connection = open_archive_connection(&paths, &git_config, None).expect("open archive");
    assert!(load_import_batch_record(&connection, batch_id).expect("load batch record").is_some());
    assert!(load_import_batch_record(&connection, 9_999).expect("missing batch").is_none());
}

#[test]
fn takeout_parsers_cover_blank_lines_arrays_and_unusable_payloads() {
    let payload = collect_records_from_payload(
        "fixture.jsonl",
        "jsonl",
        br#"
{"url":"https://example.com/ok","title":"OK","visitedAt":"2026-04-01T10:00:00+00:00"}
{"url":"https://example.com/missing","title":"Missing"}
{"title":"Ignored"}
"#,
    )
    .expect("collect jsonl");
    assert_eq!(payload.records.len(), 1);
    assert_eq!(payload.skipped_missing_visit_time, 1);

    let array_payload = collect_records_from_payload(
        "array.json",
        "browser-json",
        br#"[{"titleUrl":"https://example.com/array","pageTitle":"Array","visitedAt":"2026-04-01T10:00:00+00:00"}]"#,
    )
    .expect("collect array");
    assert_eq!(array_payload.records.len(), 1);

    let empty_payload =
        collect_records_from_payload("empty.json", "browser-json", br#"{"notHistory":[]}"#)
            .expect("collect empty");
    assert!(empty_payload.records.is_empty());

    let unusable_error =
        preview_entry_from_payload("source", "browser-json", br#"{"title":"ignored"}"#)
            .expect_err("unusable");
    assert!(unusable_error.to_string().contains("usable history record"));
}

#[test]
fn import_takeout_quarantines_unknown_files_and_skips_index_entries() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = dir.path().join("mixed-takeout");
    fs::create_dir_all(&source).expect("create mixed source");
    fs::write(
        source.join("entries.jsonl"),
        r#"{"url":"https://example.com/import","title":"Import","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
    )
    .expect("write importable payload");
    fs::write(source.join("archive_browser.html"), "<html></html>").expect("write index");
    fs::write(source.join("notes.txt"), "quarantine me").expect("write unknown file");

    let inspection = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import mixed takeout");

    assert_eq!(inspection.imported_items, 1);
    assert!(inspection.notes.is_empty());
    assert!(!paths.quarantine_dir.join("mixed-takeout").join("notes.txt").exists());
    assert!(
        inspection
            .recognized_files
            .iter()
            .any(|file| file.path.ends_with("archive_browser.html") && file.status == "ignored")
    );
}

#[test]
fn import_takeout_persists_source_evidence_for_all_recognized_payloads() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = dir.path().join("rich-takeout");
    let chrome_dir = source.join("Chrome");
    fs::create_dir_all(&chrome_dir).expect("create rich source");
    fs::write(
        chrome_dir.join("BrowserHistory.json"),
        chrome_browser_history_payload(&[
            r#"{"url":"https://example.com/imported","title":"Imported","time_usec":1711965600000000,"client_id":"client-1"}"#,
        ]),
    )
    .expect("write browser history");
    fs::write(
        chrome_dir.join("TypedUrl.json"),
        r#"{"TypedUrl":[{"url":"https://example.com/imported","title":"Imported","visits":[1711965600000000]}]}"#,
    )
    .expect("write typed urls");
    fs::write(
        chrome_dir.join("Session.json"),
        r#"{"Session":[{"sessionTag":"device-1","tab":[{"tabId":1,"navigation":[{"virtualUrl":"https://example.com/imported"}]}]}]}"#,
    )
    .expect("write session");

    let inspection = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import rich takeout");
    assert_eq!(inspection.imported_items, 1);
    assert_eq!(inspection.recognized_files.len(), 3);

    let source_evidence =
        open_source_evidence_connection(&paths, &config, None).expect("open source evidence");
    let source_batch_count: i64 = source_evidence
        .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get(0))
        .expect("count source batches");
    assert_eq!(source_batch_count, 3);

    let native_entity_kinds = {
        let mut statement = source_evidence
            .prepare("SELECT entity_kind FROM native_entities ORDER BY entity_kind")
            .expect("prepare native entity query");
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query native kinds")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect native kinds")
    };
    assert!(native_entity_kinds.contains(&"takeout-browser-history".to_string()));
    assert!(native_entity_kinds.contains(&"takeout-typed-url".to_string()));
    assert!(native_entity_kinds.contains(&"takeout-session".to_string()));

    let context_rows: i64 = source_evidence
        .query_row(
            "SELECT COUNT(*) FROM visit_context_evidence WHERE context_key = 'context.takeout.client_id'",
            [],
            |row| row.get(0),
        )
        .expect("count takeout context rows");
    assert_eq!(context_rows, 1);

    let last_source_batch_id: Option<i64> = open_archive_connection(&paths, &config, None)
        .expect("reopen archive")
        .query_row(
            "SELECT last_source_batch_id FROM profile_watermarks WHERE profile_id = 'takeout::browser-history'",
            [],
            |row| row.get(0),
        )
        .expect("load watermark");
    assert!(last_source_batch_id.is_some());
}

#[test]
fn import_takeout_spools_large_streamed_source_evidence_chunks() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let large_blob = "x".repeat(300_000);
    let source = dir.path().join("spooled-takeout");
    fs::create_dir_all(source.join("Chrome")).expect("create spooled source");
    fs::write(
        source.join("Chrome").join("BrowserHistory.json"),
        chrome_browser_history_payload(&[&format!(
            r#"{{"url":"https://example.com/huge","title":"Huge","time_usec":1711965600000000,"client_id":"{}"}}"#,
            large_blob
        )]),
    )
    .expect("write large browser history");

    let inspection = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import spooled takeout");
    assert_eq!(inspection.imported_items, 1);

    let source_evidence =
        open_source_evidence_connection(&paths, &config, None).expect("open source evidence");
    let context_rows: i64 = source_evidence
        .query_row(
            "SELECT COUNT(*) FROM visit_context_evidence WHERE context_key = 'context.takeout.client_id'",
            [],
            |row| row.get(0),
        )
        .expect("count takeout context rows");
    assert_eq!(context_rows, 1);
    let payload_bytes: i64 = source_evidence
        .query_row(
            "SELECT LENGTH(payload_json) FROM native_entities WHERE entity_kind = 'takeout-browser-history'",
            [],
            |row| row.get(0),
        )
        .expect("load native entity payload length");
    assert!(payload_bytes > 300_000);

    let spool_dir = paths.staging_dir.join("source-evidence-spool");
    assert_eq!(fs::read_dir(&spool_dir).expect("read spool dir").count(), 0);
}

#[test]
fn import_takeout_quarantines_unknown_zip_entries() {
    let dir = tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = initialized_plaintext_config();
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    drop(archive);

    let source = write_takeout_zip(
        dir.path(),
        &[
            (
                "Chrome/BrowserHistory.json",
                &chrome_browser_history_payload(&[
                    r#"{"url":"https://example.com/imported","title":"Imported","time_usec":1711965600000000}"#,
                ]),
            ),
            ("nested/notes.txt", "quarantine me"),
        ],
    );

    let inspection = import_takeout(
        &paths,
        &config,
        None,
        &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
    )
    .expect("import zipped takeout");

    assert_eq!(inspection.imported_items, 1);
    assert!(!paths.quarantine_dir.join("takeout").join("nested").join("notes.txt").exists());
}

#[test]
fn browser_json_payloads_ignore_missing_and_valid_records() {
    let payload = collect_records_from_payload(
        "BrowserHistory.json",
        "browser-json",
        chrome_browser_history_payload(&[
            r#"{"title":"Ignored"}"#,
            r#"{"url":"https://example.com/missing","title":"Missing"}"#,
            r#"{"url":"https://example.com/ok","title":"OK","time_usec":1711965600000000}"#,
        ])
        .as_bytes(),
    )
    .expect("collect browser history payload");

    assert_eq!(payload.records.len(), 1);
    assert_eq!(payload.records[0].title.as_deref(), Some("OK"));
    assert_eq!(payload.skipped_missing_visit_time, 1);
}
