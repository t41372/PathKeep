//! Responsibilities: preserve preview bounds, warnings and missing-parent handling.
//! Not responsible for: browser SQLite internals.
//! Failure modes: uncapped preview, duplicate flags drift, missing URL visits inserted.
use super::*;
use crate::test_support::store::Probe;
use crate::{
    archive::{create_schema, open_archive_connection},
    config::{ensure_paths, project_paths_with_root},
    models::{AppConfig, ArchiveMode},
};
use tempfile::tempdir;

fn parsed_url(source_url_id: i64) -> ParsedUrl {
    ParsedUrl {
        source_url_id,
        url: format!("https://example.com/{source_url_id}"),
        title: Some(format!("Example {source_url_id}")),
        visit_count: 1,
        typed_count: 0,
        last_visit_ms: 1_767_222_000_000 + source_url_id,
        last_visit_iso: "2026-01-01T00:00:00Z".to_string(),
        hidden: false,
        source_last_visit_marker: None,
    }
}

fn parsed_visit(source_visit_id: i64, source_url_id: i64) -> ParsedVisit {
    ParsedVisit {
        source_visit_id,
        source_url_id,
        url: format!("https://example.com/{source_url_id}"),
        title: Some(format!("Visit {source_visit_id}")),
        visit_time_ms: 1_767_222_000_000 + source_visit_id,
        visit_time_iso: "2026-01-01T00:00:00Z".to_string(),
        from_visit: None,
        transition: Some(1),
        visit_duration_ms: None,
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: None,
    }
}

#[test]
fn preview_collector_counts_urls_and_caps_preview_entries() {
    let mut collector = BrowserPreviewCollector {
        source_path: "/tmp/History".to_string(),
        ..BrowserPreviewCollector::default()
    };
    let visits = (0..(PREVIEW_LIMIT + 2))
        .map(|index| parsed_visit(index as i64, index as i64))
        .collect::<Vec<_>>();

    collector.urls(vec![parsed_url(1)]).expect("count urls");
    collector.visits(visits).expect("collect preview visits");

    assert_eq!(collector.url_count, 1);
    assert_eq!(collector.candidate_items, PREVIEW_LIMIT + 2);
    assert_eq!(collector.preview_entries.len(), PREVIEW_LIMIT);
    assert_eq!(collector.preview_entries[0].source_path, "/tmp/History");
    assert_eq!(collector.preview_range.start.as_deref(), Some("2026-01-01T00:00:00Z"));
}

#[test]
fn archive_consumer_reports_visits_without_matching_url_rows_as_skipped() {
    let vault = Vault::open_temp().unwrap();
    let run = crate::archive::write_runs::start(&vault, "import", "manual", &[]).unwrap();
    vault.store().write(Db::Archive,Priority::Batch,|s| {s.execute("INSERT INTO import_batches(id,source_kind,source_path,profile_id,created_at,status,summary_json) VALUES(2,'test','test','test','now','running','{}')",())?;Ok(())}).unwrap();
    let context = WriteContext {
        run_id: run,
        source_profile_id: 3,
        profile_id: "test".into(),
        browser_family: "chromium".into(),
        import: Some(ImportOrigin {
            batch_id: 2,
            source_kind: "chromium-history-db".into(),
            source_path: None,
        }),
    };
    let cancel = CancelToken::new();
    let mut progress_events = Vec::new();
    let mut consumer = ArchiveChunkConsumer::new(
        &vault,
        context,
        Watermark::default(),
        &cancel,
        Some(Box::new(|p| progress_events.push(p))),
        1,
    );
    consumer.visits(vec![parsed_visit(10, 999)]).expect("consume visit");

    let counts = consumer.finish().unwrap();
    assert_eq!(counts.visit_count + counts.skipped_visits, 1);
    assert_eq!(counts.skipped_visits, 1);
    assert_eq!(progress_events.len(), 1);
    assert_eq!(progress_events[0].processed_records, 1);
    assert_eq!(progress_events[0].skipped_records, 1);
}

#[test]
fn browser_history_import_requires_initialized_archive_before_staging() {
    let root = tempdir().expect("tempdir");
    let request = BrowserHistoryImportRequest {
        source_path: root.path().join("missing-history").display().to_string(),
        dry_run: false,
        browser_family: Some("chromium".to_string()),
        profile_id: Some("chrome:Default".to_string()),
        browser_name: Some("Google Chrome".to_string()),
        profile_name: Some("Default".to_string()),
    };

    let error = import_browser_history(
        &Vault::open_temp().unwrap().with_config(AppConfig::default()),
        &request,
    )
    .expect_err("uninitialized archive should fail before staging");

    assert!(error.to_string().contains("archive must be initialized"));
}

#[test]
fn browser_history_import_marks_run_failed_when_streaming_breaks_after_staging() {
    let root = tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    ensure_paths(&paths).expect("paths");
    let vault = Vault::open_temp().unwrap();
    let archive = Probe::new(&vault);
    let history_path = root.path().join("BrokenHistory");
    Connection::open(&history_path)
        .expect("broken history")
        .execute_batch(
            "CREATE TABLE urls (id INTEGER PRIMARY KEY);
                 CREATE TABLE visits (id INTEGER PRIMARY KEY);",
        )
        .expect("minimal broken chromium schema");
    let request = BrowserHistoryImportRequest {
        source_path: history_path.display().to_string(),
        dry_run: false,
        browser_family: Some("chromium".to_string()),
        profile_id: Some("chrome:Broken".to_string()),
        browser_name: Some("Google Chrome".to_string()),
        profile_name: Some("Broken".to_string()),
    };

    let error =
        import_browser_history(&vault, &request).expect_err("broken staged history should fail");
    let status = archive
        .query_row("SELECT status FROM runs ORDER BY id DESC LIMIT 1", (), |row| {
            Ok(row.get::<String>(0)?)
        })
        .expect("failed run status");

    assert!(!format!("{error:#}").is_empty());
    assert_eq!(status, "failed");
}

#[test]
fn browser_import_progress_uses_fallback_label_and_log_tail() {
    let log_lines = vec![
        "one".to_string(),
        "two".to_string(),
        "three".to_string(),
        "four".to_string(),
        "five".to_string(),
    ];
    let mut events = Vec::new();

    emit_browser_import_progress_with_records(
        &mut |event| events.push(event),
        BrowserImportProgressEventInput {
            phase: "unexpected-phase",
            detail: "Working".to_string(),
            current: 2,
            total: 3,
            progress_percent: Some(66.0),
            source_path: Some("/tmp/History".to_string()),
            log_lines: &log_lines,
            record_state: BrowserImportProgressState {
                source_label: Some("Google Chrome / Primary".to_string()),
                processed_records: Some(5),
                total_records: Some(9),
                imported_records: Some(4),
                duplicate_records: Some(1),
                skipped_records: Some(0),
            },
        },
    );

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].label, "Importing browser history");
    assert_eq!(events[0].phase, "unexpected-phase");
    assert_eq!(events[0].log_lines, vec!["two", "three", "four", "five"]);
    assert_eq!(events[0].processed_records, Some(5));
    assert_eq!(events[0].source_label.as_deref(), Some("Google Chrome / Primary"));
    assert_eq!(events[0].log_events[0].code, "import.unexpected-phase");
    assert_eq!(events[0].log_events[0].processed_records, Some(5));
}

#[test]
fn browser_import_note_and_failure_helpers_keep_review_contract() {
    let mut last_processed_records = 0;
    assert!(!should_emit_browser_import_progress(&mut last_processed_records, 0));
    assert!(should_emit_browser_import_progress(&mut last_processed_records, 3));
    assert_eq!(last_processed_records, 3);
    let mut progress_events = Vec::new();
    emit_browser_import_progress_if_changed(
        &mut |event| progress_events.push(event),
        &mut last_processed_records,
        "/tmp/History",
        "Google Chrome",
        "Primary",
        &["importing".to_string()],
        BrowserImportProgress {
            processed_records: 3,
            imported_records: 2,
            duplicate_records: 1,
            skipped_records: 0,
        },
    );
    assert!(progress_events.is_empty());
    emit_browser_import_progress_if_changed(
        &mut |event| progress_events.push(event),
        &mut last_processed_records,
        "/tmp/History",
        "Google Chrome",
        "Primary",
        &["importing".to_string()],
        BrowserImportProgress {
            processed_records: 4,
            imported_records: 3,
            duplicate_records: 1,
            skipped_records: 0,
        },
    );
    assert_eq!(progress_events.len(), 1);
    assert_eq!(progress_events[0].processed_records, Some(4));
    assert_eq!(progress_events[0].source_label.as_deref(), Some("Google Chrome / Primary"));
    assert_eq!(
        progress_events[0].log_events[0].source_label.as_deref(),
        Some("Google Chrome / Primary")
    );

    let mut note_inspection = TakeoutInspection::default();
    append_browser_import_skipped_note(&mut note_inspection, 0);
    assert!(note_inspection.notes.is_empty());
    assert!(note_inspection.note_details.is_empty());
    append_browser_import_skipped_note(&mut note_inspection, 2);
    assert!(note_inspection.notes.iter().any(|n| n.contains("Skipped 2 visit row")));
    assert_eq!(note_inspection.note_details[0].code, "browser-skipped-missing-url-row");
    assert_eq!(note_inspection.note_details[0].count, Some(2));

    let parser_notes = parser_warning_notes(&[browser_history_parser::ParserWarning {
        code: "missing-table".to_string(),
        message: "required Chromium table `urls` is missing".to_string(),
    }]);
    assert_eq!(parser_notes.len(), 1);
    assert_eq!(parser_notes[0].code, "parser-missing-table");
    assert_eq!(
        parser_notes[0].diagnostic.as_deref(),
        Some("required Chromium table `urls` is missing")
    );

    let root = tempdir().expect("tempdir");
    let paths = project_paths_with_root(root.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("schema");
    let vault = Vault::open_temp().unwrap();
    let archive = Probe::new(&vault);
    let run_id = crate::archive::write_runs::start(&vault, "import", "manual", &[]).unwrap();
    crate::archive::write_runs::stopped(
        &vault,
        run_id,
        &anyhow::anyhow!("stream failed"),
        json!({}),
        None,
    )
    .unwrap();
    let (status, error_message): (String, Option<String>) = archive
        .query_row("SELECT status, error_message FROM runs WHERE id = ?1", [run_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("failed run");
    assert_eq!(status, "failed");
    assert!(error_message.as_deref().is_some_and(|message| message.contains("stream failed")));
}
