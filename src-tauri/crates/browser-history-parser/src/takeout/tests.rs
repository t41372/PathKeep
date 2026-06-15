//! Regression tests for the Takeout parser boundary.

use super::*;
use crate::types::{
    CapabilityCoverage, ContextEvidence, DatabaseInspection, HistoryBatchConsumer, NativeEntity,
    NavigationEvidence, ObservedColumn, ObservedTable, ParsedDownload, ParsedFavicon,
    ParsedSearchTerm, ParsedUrl, ParsedVisit, ParserWarning, SchemaObservation, SearchEvidence,
    StreamedHistory, TypedEvidenceBatch,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

fn browser_history_payload(records: &[&str]) -> String {
    format!(r#"{{"Browser History":[{}]}}"#, records.join(","))
}

fn collect_json_stream_records(bytes: &[u8], keys: &[&str]) -> Result<Vec<(Value, usize)>, String> {
    let mut records = Vec::new();
    json_stream::stream_payload_records(bytes, "payload.json", keys, |record, ordinal| {
        records.push((record, ordinal));
        Ok::<(), String>(())
    })
    .map_err(|error| error.to_string())?;
    Ok(records)
}

#[test]
fn json_stream_records_arrays_ordinals_errors_and_shape_rejections() {
    let top_level = collect_json_stream_records(
        br#"[{"url":"https://example.com/one"},{"url":"https://example.com/two"}]"#,
        &[],
    )
    .expect("top-level records");
    assert_eq!(top_level.len(), 2);
    assert_eq!(top_level[1].1, 1);

    let named = collect_json_stream_records(
        br#"{"Ignored":[{"skip":true}],"Rows":[{"id":1},{"id":2}]}"#,
        &["Rows"],
    )
    .expect("named records");
    assert_eq!(named.len(), 2);
    assert_eq!(named[0].0["id"], 1);

    for bytes in [
        br#"true"#.as_slice(),
        br#"-1"#.as_slice(),
        br#"18446744073709551615"#.as_slice(),
        br#"1.5"#.as_slice(),
        br#""literal""#.as_slice(),
        br#"null"#.as_slice(),
    ] {
        let error = collect_json_stream_records(bytes, &["Rows"]).expect_err("primitive root");
        assert!(error.contains("Takeout payload array or object"));
    }

    for bytes in [
        br#"{"Rows":true}"#.as_slice(),
        br#"{"Rows":-1}"#.as_slice(),
        br#"{"Rows":18446744073709551615}"#.as_slice(),
        br#"{"Rows":1.5}"#.as_slice(),
        br#"{"Rows":"literal"}"#.as_slice(),
        br#"{"Rows":null}"#.as_slice(),
        br#"{"Rows":{"id":1}}"#.as_slice(),
    ] {
        let error =
            collect_json_stream_records(bytes, &["Rows"]).expect_err("primitive named value");
        assert!(error.contains("array of Takeout payload records"));
    }

    let callback = json_stream::stream_payload_records(
        br#"[{"id":1},{"id":2}]"#,
        "payload.json",
        &[],
        |_record, ordinal| {
            if ordinal == 1 {
                return Err("stop at second row".to_string());
            }
            Ok(())
        },
    )
    .expect_err("callback error");
    assert!(matches!(
        callback,
        json_stream::JsonRecordStreamError::Callback(ref error) if error == "stop at second row"
    ));

    let parse_error =
        json_stream::stream_payload_records::<_, String>(b"{", "broken.json", &[], |_record, _| {
            Ok(())
        })
        .expect_err("parse error");
    assert!(matches!(
        parse_error,
        json_stream::JsonRecordStreamError::Parse(ref error)
            if error.to_string().contains("broken.json")
    ));
}

#[test]
fn inspect_history_reports_supported_takeout_payloads() {
    let dir = tempdir().expect("tempdir");
    fs::create_dir_all(dir.path().join("Chrome")).expect("create chrome dir");
    fs::write(
        dir.path().join("Chrome").join("BrowserHistory.json"),
        browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000}"#,
        ]),
    )
    .expect("write browser history");
    fs::write(
        dir.path().join("TypedUrl.json"),
        r#"{"TypedUrl":[{"url":"https://example.com","title":"Example"}]}"#,
    )
    .expect("write typed url");
    let inspection = inspect_history(dir.path()).expect("inspect");
    assert!(inspection.table_names.contains(&KIND_BROWSER_JSON.to_string()));
    assert!(inspection.table_names.contains(&KIND_TYPED_URL_JSON.to_string()));
}

#[test]
fn collected_urls_merge_counts_and_keep_the_newest_title() {
    let mut existing = ParsedUrl {
        source_url_id: 7,
        url: "https://example.com".to_string(),
        title: Some("Old title".to_string()),
        visit_count: 2,
        typed_count: 1,
        last_visit_ms: 100,
        last_visit_iso: "1970-01-01T00:00:00Z".to_string(),
        hidden: false,
        source_last_visit_marker: None,
    };
    let newer = ParsedUrl {
        source_url_id: 7,
        url: "https://example.com".to_string(),
        title: Some("Fresh title".to_string()),
        visit_count: 3,
        typed_count: 2,
        last_visit_ms: 200,
        last_visit_iso: "1970-01-01T00:00:01Z".to_string(),
        hidden: false,
        source_last_visit_marker: None,
    };

    merge_collected_url(&mut existing, &newer);

    assert_eq!(existing.visit_count, 5);
    assert_eq!(existing.typed_count, 3);
    assert_eq!(existing.last_visit_ms, 200);
    assert_eq!(existing.last_visit_iso, "1970-01-01T00:00:01Z");
    assert_eq!(existing.title.as_deref(), Some("Fresh title"));
}

#[test]
fn classify_payload_path_handles_localized_history_and_review_only_paths() {
    let english = classify_payload_path("Chrome/History.json");
    assert_eq!(english.recognized_kind, Some(KIND_BROWSER_JSON));
    assert_eq!(english.locale, Some("en"));
    assert_eq!(english.disposition, TakeoutPathDisposition::WillImport);

    let direct_english = classify_payload_path("History.json");
    assert_eq!(direct_english.recognized_kind, Some(KIND_BROWSER_JSON));
    assert_eq!(direct_english.locale, Some("en"));

    let spaced_backslash = classify_payload_path(" Chrome\\BrowserHistory.json ");
    assert_eq!(spaced_backslash.recognized_kind, Some(KIND_BROWSER_JSON));
    assert_eq!(spaced_backslash.locale, Some("en"));

    let german = classify_payload_path("Chrome/Verlauf.json");
    assert_eq!(german.recognized_kind, Some(KIND_BROWSER_JSON));
    assert_eq!(german.locale, Some("de"));
    assert_eq!(german.disposition, TakeoutPathDisposition::WillImport);

    let activity = classify_payload_path("My Activity/Chrome/MyActivity.json");
    assert_eq!(activity.recognized_kind, None);
    assert_eq!(activity.disposition, TakeoutPathDisposition::NeedsReview);
    assert_eq!(activity.reason_code, "chrome-my-activity-json");

    let activity_zh_tw = classify_payload_path("Takeout/我的活動/Chrome/我的活動.json");
    assert_eq!(activity_zh_tw.recognized_kind, None);
    assert_eq!(activity_zh_tw.disposition, TakeoutPathDisposition::NeedsReview);
    assert_eq!(activity_zh_tw.reason_code, "chrome-my-activity-json");
    assert_eq!(activity_zh_tw.locale, Some("zh-tw"));

    let activity_html = classify_payload_path("Takeout/我的活動/Chrome/我的活動.html");
    assert_eq!(activity_html.recognized_kind, None);
    assert_eq!(activity_html.disposition, TakeoutPathDisposition::NeedsReview);
    assert_eq!(activity_html.reason_code, "chrome-my-activity-html");

    let ignored = classify_payload_path("Google Play Store/Installs.json");
    assert_eq!(ignored.recognized_kind, None);
    assert_eq!(ignored.disposition, TakeoutPathDisposition::KnownIgnored);
}

#[test]
fn classify_payload_path_covers_takeout_scope_matrix() {
    let jsonl = classify_payload_path("entries.jsonl");
    assert_eq!(jsonl.recognized_kind, Some(KIND_JSONL));
    assert_eq!(jsonl.reason_code, "jsonl-history-fixture");

    for typed_url in ["TypedUrl.json", "Typed Url.json"] {
        let matched = classify_payload_path(typed_url);
        assert_eq!(matched.recognized_kind, Some(KIND_TYPED_URL_JSON));
        assert_eq!(matched.disposition, TakeoutPathDisposition::KnownIgnored);
    }

    for session in ["Session.json", "Sessions.json"] {
        let matched = classify_payload_path(session);
        assert_eq!(matched.recognized_kind, Some(KIND_SESSION_JSON));
        assert_eq!(matched.reason_code, "source-evidence-only");
    }

    let index = classify_payload_path("archive_browser.html");
    assert_eq!(index.recognized_kind, Some(KIND_INDEX));
    assert_eq!(index.locale, Some("en"));

    let german_index = classify_payload_path("archiv_übersicht.html");
    assert_eq!(german_index.recognized_kind, Some(KIND_INDEX));
    assert_eq!(german_index.locale, Some("de"));

    let german_activity = classify_payload_path("Meine Aktivitäten/Chrome/Meine Aktivitäten.json");
    assert_eq!(german_activity.reason_code, "chrome-my-activity-json");
    assert_eq!(german_activity.locale, Some("de"));

    let zh_cn_activity = classify_payload_path("我的活动/Chrome/我的活动.html");
    assert_eq!(zh_cn_activity.reason_code, "chrome-my-activity-html");
    assert_eq!(zh_cn_activity.locale, Some("zh-cn"));

    let chrome_support = classify_payload_path("Chrome/Bookmarks.json");
    assert_eq!(chrome_support.family, "chrome-supporting-file");
    assert_eq!(chrome_support.disposition, TakeoutPathDisposition::KnownIgnored);
    assert_eq!(chrome_support.reason_code, "chrome-supporting-file");

    let chrome_history_like = classify_payload_path("Chrome/Browser-Backup.sqlite");
    assert_eq!(chrome_history_like.disposition, TakeoutPathDisposition::NeedsReview);
    assert_eq!(chrome_history_like.reason_code, "unrecognized-history-file");

    let outside_activity = classify_payload_path("Meine Aktivitäten/Suche/MyActivity.json");
    assert_eq!(outside_activity.family, "google-activity");
    assert_eq!(outside_activity.locale, Some("de"));
    assert_eq!(outside_activity.disposition, TakeoutPathDisposition::KnownIgnored);

    let german_outside_scope = classify_payload_path("Google Fotos/metadata.json");
    assert_eq!(german_outside_scope.locale, Some("de"));
    assert_eq!(german_outside_scope.reason_code, "outside-chrome-scope");

    let normalized_spaces = classify_payload_path("Google  Fotos/metadata.json");
    assert_eq!(normalized_spaces.locale, Some("de"));

    let normalized_tab = classify_payload_path("Google\tFotos/metadata.json");
    assert_eq!(normalized_tab.locale, Some("de"));

    let unknown_history = classify_payload_path("some/random/history-export.txt");
    assert_eq!(unknown_history.family, "unknown-history-like");
    assert_eq!(unknown_history.disposition, TakeoutPathDisposition::NeedsReview);

    let unknown_browser = classify_payload_path("some/random/browser-export.txt");
    assert_eq!(unknown_browser.family, "unknown-history-like");
    assert_eq!(unknown_browser.disposition, TakeoutPathDisposition::NeedsReview);

    let unknown_activity = classify_payload_path("some/random/myactivity.txt");
    assert_eq!(unknown_activity.family, "unknown-history-like");
    assert_eq!(unknown_activity.disposition, TakeoutPathDisposition::NeedsReview);

    let outside = classify_payload_path("some/random/file.txt");
    assert_eq!(outside.family, "outside-scope");
    assert_eq!(outside.disposition, TakeoutPathDisposition::KnownIgnored);

    assert_eq!(recognize_payload("Chrome/History.json"), Some(KIND_BROWSER_JSON));
}

#[test]
fn gather_takeout_files_skips_common_system_noise() {
    let dir = tempdir().expect("tempdir");
    let chrome_dir = dir.path().join("Chrome");
    fs::create_dir_all(&chrome_dir).expect("create chrome dir");
    fs::write(chrome_dir.join("BrowserHistory.json"), browser_history_payload(&[]))
        .expect("write browser history");
    fs::write(dir.path().join(".DS_Store"), "noise").expect("write ds_store");
    fs::create_dir_all(dir.path().join("__MACOSX")).expect("create macosx dir");
    fs::write(dir.path().join("__MACOSX").join("ignored.json"), "{}").expect("write ignored");

    let files = source::gather_takeout_files(dir.path()).expect("gather files");
    assert_eq!(files.len(), 1);
    assert!(files[0].path.ends_with("Chrome/BrowserHistory.json"));
}

#[test]
fn source_discovery_handles_empty_sources_direct_noise_and_zip_sniffing() {
    let empty_dir = tempdir().expect("empty tempdir");
    let inspection = inspect_history(empty_dir.path()).expect("inspect empty dir");
    assert!(inspection.table_names.is_empty());
    assert!(inspection.warnings.iter().any(|warning| warning.code == "no-recognized-payload"));

    let noise = empty_dir.path().join(".DS_Store");
    fs::write(&noise, "noise").expect("write noise");
    let noise_files = source::gather_takeout_files(&noise).expect("gather direct noise");
    assert!(noise_files.is_empty());

    let direct = empty_dir.path().join("BrowserHistory.json");
    fs::write(&direct, browser_history_payload(&[])).expect("write direct history");
    let direct_files = source::gather_takeout_files(&direct).expect("gather direct file");
    assert_eq!(direct_files.len(), 1);
    let direct_bytes =
        source::read_takeout_file(&direct, &direct_files[0]).expect("read direct file");
    assert!(String::from_utf8_lossy(&direct_bytes).contains("Browser History"));

    let zip_path = empty_dir.path().join("takeout.zip");
    let file = fs::File::create(&zip_path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("__MACOSX/ignored.json", options).expect("start ignored entry");
    zip.write_all(b"{}").expect("write ignored entry");
    zip.start_file("Chrome/anything.json", options).expect("start sniff entry");
    zip.write_all(
        browser_history_payload(&[
            r#"{"url":"https://example.com/zip","title":"Zip","time_usec":1711965600000000}"#,
        ])
        .as_bytes(),
    )
    .expect("write sniff entry");
    zip.finish().expect("finish zip");

    let files = source::gather_takeout_files(&zip_path).expect("gather zip");
    assert_eq!(files.len(), 1);
    assert!(files[0].from_zip);
    let path_match =
        classify_payload_path_with_sniff(&zip_path, &files[0].path, true).expect("sniff zip");
    assert_eq!(path_match.recognized_kind, Some(KIND_BROWSER_JSON));
    let zip_bytes = source::read_takeout_file(&zip_path, &files[0]).expect("read zip entry");
    assert!(String::from_utf8_lossy(&zip_bytes).contains("Browser History"));

    let recognized_missing_file =
        classify_payload_path_with_sniff(empty_dir.path(), "Chrome/History.json", false)
            .expect("recognized history paths do not need sniffing");
    assert_eq!(recognized_missing_file.recognized_kind, Some(KIND_BROWSER_JSON));

    let late_marker = empty_dir.path().join("late-marker.json");
    fs::write(
        &late_marker,
        format!(
            "{}{}",
            " ".repeat(2_048),
            browser_history_payload(&[
                r#"{"url":"https://example.com/late","title":"Late","time_usec":1711965600000000}"#,
            ])
        ),
    )
    .expect("write late marker history");
    let late_marker_match =
        classify_payload_path_with_sniff(&late_marker, &late_marker.display().to_string(), false)
            .expect("sniff late marker");
    assert_eq!(late_marker_match.recognized_kind, Some(KIND_BROWSER_JSON));

    let incomplete_marker = empty_dir.path().join("incomplete-marker.json");
    fs::write(&incomplete_marker, r#"{"Browser History":[{"url":"https://example.com"}]}"#)
        .expect("write incomplete marker");
    let incomplete_marker_match = classify_payload_path_with_sniff(
        &incomplete_marker,
        &incomplete_marker.display().to_string(),
        false,
    )
    .expect("sniff incomplete marker");
    assert_eq!(incomplete_marker_match.recognized_kind, None);

    let non_history = empty_dir.path().join("whatever.json");
    fs::write(&non_history, r#"{"not":"browser history"}"#).expect("write non-history");
    let no_sniff_match =
        classify_payload_path_with_sniff(&non_history, &non_history.display().to_string(), false)
            .expect("classify non-history direct file");
    assert_eq!(no_sniff_match.recognized_kind, None);

    let non_json_match =
        classify_payload_path_with_sniff(empty_dir.path(), "Chrome/notes.txt", false)
            .expect("classify non-json");
    assert_eq!(non_json_match.recognized_kind, None);

    let outside_chrome_json =
        classify_payload_path_with_sniff(empty_dir.path(), "Drive/anything.json", false)
            .expect("outside-chrome json does not sniff");
    assert_eq!(outside_chrome_json.recognized_kind, None);

    let missing_zip = empty_dir.path().join("missing.zip");
    let missing_zip_error =
        source::gather_takeout_files(&missing_zip).expect_err("missing zip source");
    assert!(missing_zip_error.to_string().contains("missing.zip"));

    let missing_zip_sniff_error =
        classify_payload_path_with_sniff(&missing_zip, "Chrome/anything.json", true)
            .expect_err("missing zip sniff source");
    assert!(missing_zip_sniff_error.to_string().contains("missing.zip"));

    let missing_zip_entry =
        source::TakeoutFile { path: "Chrome/BrowserHistory.json".to_string(), from_zip: true };
    let missing_entry_error =
        source::read_takeout_file(&missing_zip, &missing_zip_entry).expect_err("missing zip file");
    assert!(missing_entry_error.to_string().contains("missing.zip"));

    let missing_direct = empty_dir.path().join("Chrome").join("missing-direct.json");
    let missing_direct_error = classify_payload_path_with_sniff(
        empty_dir.path(),
        &missing_direct.display().to_string(),
        false,
    )
    .expect_err("missing direct sniff source");
    assert!(missing_direct_error.to_string().contains("missing-direct.json"));
}

#[test]
fn source_zip_reader_preserves_read_error_path_context() {
    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("zip reader failed"))
        }
    }

    let error = source::read_takeout_zip_reader(
        FailingReader,
        PathBuf::from("/tmp/takeout.zip::Chrome/BrowserHistory.json"),
    )
    .expect_err("failing zip reader");

    assert!(error.to_string().contains("takeout.zip::Chrome/BrowserHistory.json"));
}

#[test]
fn inspect_history_accepts_direct_localized_browser_history_files() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("歷史記錄.json");
    fs::write(
        &source,
        browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000}"#,
        ]),
    )
    .expect("write browser history");

    let inspection = inspect_history(&source).expect("inspect direct file");
    assert_eq!(inspection.table_names, vec![KIND_BROWSER_JSON.to_string()]);
}

#[test]
fn classify_payload_path_with_sniff_accepts_unknown_named_chrome_history_files() {
    let dir = tempdir().expect("tempdir");
    let source = dir.path().join("任何名字.json");
    fs::write(
        &source,
        browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000}"#,
        ]),
    )
    .expect("write browser history");

    let path_match =
        classify_payload_path_with_sniff(&source, &source.display().to_string(), false)
            .expect("classify direct file");
    assert_eq!(path_match.recognized_kind, Some(KIND_BROWSER_JSON));
    assert_eq!(path_match.disposition, TakeoutPathDisposition::WillImport);
}

#[test]
fn parse_payload_extracts_browser_history_records() {
    let report = parse_payload(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000,"client_id":"abc","favicon_url":"https://example.com/favicon.ico","page_transition":"LINK","ptoken":"token"}"#,
        ])
        .as_bytes(),
    )
    .expect("parse payload");
    assert_eq!(report.record_count, 1);
    assert_eq!(report.history.visits.len(), 1);
    assert_eq!(report.history.urls.len(), 1);
    assert_eq!(report.history.native_entities.len(), 1);
    assert!(report.history.warnings.is_empty());
    assert!(
        report
            .history
            .capability_snapshot
            .items
            .iter()
            .any(|item| item.key == "canonical.visits" && item.available)
    );
}

#[test]
fn browser_history_payload_covers_jsonl_skips_duplicates_and_error_paths() {
    let jsonl = parse_payload(
        "entries.jsonl",
        KIND_JSONL,
        b"\n{\"url\":\"\",\"time_usec\":1711965600000000}\n{\"url\":\"https://example.com/repeat\",\"title\":\"Old\",\"time_usec\":\"1711965600000000\"}\n{\"url\":\"https://example.com/repeat\",\"title\":\"New\",\"visitedAt\":\"2024-04-01T11:00:00+00:00\"}\n{\"url\":\"https://example.com/missing-time\"}\n",
    )
    .expect("jsonl payload");
    assert_eq!(jsonl.record_count, 2);
    assert_eq!(jsonl.skipped_missing_visit_time, 1);
    assert_eq!(jsonl.history.urls.len(), 1);
    assert_eq!(jsonl.history.urls[0].visit_count, 2);
    assert_eq!(jsonl.history.urls[0].title.as_deref(), Some("New"));
    assert!(jsonl.history.warnings.iter().any(|warning| warning.code == "missing-visit-time"));

    let bad_jsonl =
        parse_payload("entries.jsonl", KIND_JSONL, b"{").expect_err("bad jsonl payload");
    assert!(bad_jsonl.to_string().contains("entries.jsonl line 1"));

    // Regression: a non-UTF-8 byte in a JSONL line must surface as a graceful
    // ReadSource error, never panic the import worker (BufRead::lines yields
    // Err(InvalidData) on invalid UTF-8 regardless of I/O success).
    let non_utf8_jsonl = parse_payload("entries.jsonl", KIND_JSONL, b"\xff\xfe garbage\n")
        .expect_err("non-utf8 jsonl payload");
    assert!(non_utf8_jsonl.to_string().contains("entries.jsonl"));

    let bad_browser_json =
        parse_payload("BrowserHistory.json", KIND_BROWSER_JSON, b"{").expect_err("bad json");
    assert!(bad_browser_json.to_string().contains("BrowserHistory.json"));

    #[derive(Default)]
    struct FailingConsumer;

    impl HistoryBatchConsumer for FailingConsumer {
        type Error = String;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Err("url sink offline".to_string())
        }

        fn visits(&mut self, _batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl TakeoutSourceEvidenceConsumer<String> for FailingConsumer {
        fn source_evidence(&mut self, _chunk: TakeoutSourceEvidenceChunk) -> Result<(), String> {
            Ok(())
        }
    }

    let mut failing = FailingConsumer;
    let sink_error = stream_payload(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000}"#,
        ])
        .as_bytes(),
        1,
        &mut failing,
    )
    .expect_err("canonical sink error");
    assert!(sink_error.to_string().contains("url sink offline"));

    let unsupported = browser_history::stream_browser_history_payload(
        "Unknown.json",
        "unknown-kind",
        b"[]",
        1,
        TakeoutStreamOptions::default(),
        &mut failing,
    )
    .expect_err("unsupported browser-history payload");
    assert!(unsupported.to_string().contains("google-takeout-browser-history"));
}

#[test]
fn stream_payload_batches_browser_history_rows() {
    #[derive(Default)]
    struct RecordingConsumer {
        urls: Vec<Vec<ParsedUrl>>,
        visits: Vec<Vec<ParsedVisit>>,
    }

    impl HistoryBatchConsumer for RecordingConsumer {
        type Error = std::convert::Infallible;

        fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            self.urls.push(batch);
            Ok(())
        }

        fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            self.visits.push(batch);
            Ok(())
        }
    }

    let mut consumer = RecordingConsumer::default();
    let report = stream_payload(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        browser_history_payload(&[
            r#"{"url":"https://example.com/one","title":"One","time_usec":1711965600000000,"client_id":"alpha"}"#,
            r#"{"url":"https://example.com/two","title":"Two","time_usec":1711969200000000,"page_transition":"LINK"}"#,
            r#"{"url":"https://example.com/missing-time","title":"Missing"}"#,
        ])
        .as_bytes(),
        1,
        &mut consumer,
    )
    .expect("stream payload");

    assert_eq!(report.counts.urls, 2);
    assert_eq!(report.counts.visits, 2);
    assert_eq!(report.record_count, 2);
    assert_eq!(report.skipped_missing_visit_time, 1);
    assert_eq!(report.earliest_visit_iso.as_deref(), Some("2024-04-01T10:00:00+00:00"));
    assert_eq!(report.latest_visit_iso.as_deref(), Some("2024-04-01T11:00:00+00:00"));
    assert_eq!(report.history.schema_observation.tables[0].row_count, Some(3));
    assert!(
        report.history.schema_observation.tables[0]
            .columns
            .iter()
            .any(|column| column.name == "client_id")
    );
    let context_capability = report
        .history
        .capability_snapshot
        .items
        .iter()
        .find(|item| item.key == "context.takeout.browser_history")
        .expect("context capability");
    assert_eq!(context_capability.populated_rows, 2);
    assert_eq!(context_capability.total_rows, 2);
    assert_eq!(consumer.urls.len(), 2);
    assert_eq!(consumer.visits.len(), 2);
    assert_eq!(consumer.visits.iter().map(Vec::len).sum::<usize>(), 2);
}

#[test]
fn stream_payload_with_options_can_skip_source_evidence_accumulation() {
    #[derive(Default)]
    struct CountingConsumer {
        visits: usize,
    }

    impl HistoryBatchConsumer for CountingConsumer {
        type Error = std::convert::Infallible;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            self.visits += batch.len();
            Ok(())
        }
    }

    let mut consumer = CountingConsumer::default();
    let report = stream_payload_with_options(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        browser_history_payload(&[
            r#"{"url":"https://example.com/one","title":"One","time_usec":1711965600000000,"client_id":"alpha"}"#,
            r#"{"url":"https://example.com/two","title":"Two","time_usec":1711969200000000,"client_id":"beta"}"#,
        ])
        .as_bytes(),
        10,
        TakeoutStreamOptions {
            collect_source_evidence: false,
            retain_report_source_evidence: false,
        },
        &mut consumer,
    )
    .expect("stream payload without evidence");

    assert_eq!(consumer.visits, 2);
    assert!(report.history.typed_evidence.context.is_empty());
    assert!(report.history.native_entities.is_empty());
}

#[test]
fn stream_payload_with_sink_moves_source_evidence_out_of_the_report() {
    #[derive(Default)]
    struct RecordingSinkConsumer {
        visits: usize,
        source_chunks: usize,
        context_rows: usize,
        native_entities: usize,
    }

    impl HistoryBatchConsumer for RecordingSinkConsumer {
        type Error = std::convert::Infallible;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            self.visits += batch.len();
            Ok(())
        }
    }

    impl TakeoutSourceEvidenceConsumer<std::convert::Infallible> for RecordingSinkConsumer {
        fn source_evidence(
            &mut self,
            chunk: TakeoutSourceEvidenceChunk,
        ) -> Result<(), std::convert::Infallible> {
            self.source_chunks += 1;
            self.context_rows += chunk.typed_evidence.context.len();
            self.native_entities += chunk.native_entities.len();
            Ok(())
        }
    }

    let mut consumer = RecordingSinkConsumer::default();
    let report = stream_payload_with_sink(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        browser_history_payload(&[
            r#"{"url":"https://example.com/one","title":"One","time_usec":1711965600000000,"client_id":"alpha"}"#,
            r#"{"url":"https://example.com/two","title":"Two","time_usec":1711969200000000,"client_id":"beta"}"#,
        ])
        .as_bytes(),
        1,
        TakeoutStreamOptions {
            collect_source_evidence: true,
            retain_report_source_evidence: false,
        },
        &mut consumer,
    )
    .expect("stream payload with source-evidence sink");

    assert_eq!(consumer.visits, 2);
    assert_eq!(consumer.context_rows, 2);
    assert_eq!(consumer.native_entities, 2);
    assert_eq!(consumer.source_chunks, 2);
    assert!(report.history.typed_evidence.context.is_empty());
    assert!(report.history.native_entities.is_empty());
}

#[test]
fn parse_payload_preserves_typed_url_and_session_payloads_as_native_entities() {
    let typed = parse_payload(
        "TypedUrl.json",
        KIND_TYPED_URL_JSON,
        br#"{"TypedUrl":[{"url":"https://example.com","title":"Example","hidden":false}]}"#,
    )
    .expect("parse typed url");
    let session = parse_payload(
        "Session.json",
        KIND_SESSION_JSON,
        br#"{"Session":[{"sessionTag":"device-1","tab":[{"navigation":[{"virtual_url":"https://example.com"}]}]}]}"#,
    )
    .expect("parse session");
    let fallback =
        parse_payload("TypedUrl.json", KIND_TYPED_URL_JSON, br#"{"TypedUrl":[{"other":true}]}"#)
            .expect("parse fallback native key");

    assert!(typed.history.visits.is_empty());
    assert_eq!(typed.history.native_entities.len(), 1);
    assert_eq!(typed.history.native_entities[0].native_primary_key, "https://example.com");
    assert_eq!(session.history.native_entities.len(), 1);
    assert_eq!(session.history.native_entities[0].native_primary_key, "device-1");
    assert_eq!(fallback.history.native_entities[0].native_primary_key, "row-0");
}

#[test]
fn stream_payload_routes_index_unsupported_and_native_only_edges() {
    #[derive(Default)]
    struct RecordingConsumer {
        source_chunks: usize,
        native_entities: usize,
    }

    impl HistoryBatchConsumer for RecordingConsumer {
        type Error = std::convert::Infallible;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn visits(&mut self, _batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl TakeoutSourceEvidenceConsumer<std::convert::Infallible> for RecordingConsumer {
        fn source_evidence(
            &mut self,
            chunk: TakeoutSourceEvidenceChunk,
        ) -> Result<(), std::convert::Infallible> {
            self.source_chunks += 1;
            self.native_entities += chunk.native_entities.len();
            Ok(())
        }
    }

    let mut consumer = RecordingConsumer::default();
    let index = stream_payload_with_sink(
        "archive_browser.html",
        KIND_INDEX,
        b"<html></html>",
        10,
        TakeoutStreamOptions::default(),
        &mut consumer,
    )
    .expect("index report");
    assert_eq!(index.kind, KIND_INDEX);
    assert_eq!(index.record_count, 0);
    assert!(index.history.warnings.iter().any(|warning| warning.code == "index-only"));

    let unsupported = stream_payload_with_sink(
        "Unknown.json",
        "unknown-kind",
        b"[]",
        10,
        TakeoutStreamOptions::default(),
        &mut consumer,
    )
    .expect_err("unsupported kind");
    assert!(unsupported.to_string().contains("google-takeout-payload"));

    let typed = stream_payload_with_sink(
        "TypedUrl.json",
        KIND_TYPED_URL_JSON,
        br#"{"Typed Url":[{"url":"https://example.com/a"},{"titleUrl":"https://example.com/b"},{"other":true}]}"#,
        2,
        TakeoutStreamOptions {
            collect_source_evidence: true,
            retain_report_source_evidence: true,
        },
        &mut consumer,
    )
    .expect("typed native payload");
    assert_eq!(typed.record_count, 3);
    assert_eq!(typed.history.native_entities.len(), 3);
    assert_eq!(consumer.source_chunks, 2);
    assert_eq!(consumer.native_entities, 3);
    assert!(
        typed
            .history
            .capability_snapshot
            .items
            .iter()
            .any(|item| item.key == "context.takeout.typed_url" && item.available)
    );

    let mut metadata_keys =
        typed.history.native_entities[0].metadata.keys().cloned().collect::<Vec<_>>();
    metadata_keys.sort();
    assert_eq!(metadata_keys, vec!["payloadKind".to_string(), "sourcePath".to_string()]);

    let no_evidence = stream_payload_with_sink(
        "Session.json",
        KIND_SESSION_JSON,
        br#"{"Session":[{"sessionTag":"device-1"}]}"#,
        10,
        TakeoutStreamOptions {
            collect_source_evidence: false,
            retain_report_source_evidence: false,
        },
        &mut consumer,
    )
    .expect("session without evidence");
    assert_eq!(no_evidence.record_count, 1);
    assert!(no_evidence.history.native_entities.is_empty());

    let parse_error = stream_payload_with_sink(
        "TypedUrl.json",
        KIND_TYPED_URL_JSON,
        b"{",
        10,
        TakeoutStreamOptions::default(),
        &mut consumer,
    )
    .expect_err("native-only parse error");
    assert!(parse_error.to_string().contains("TypedUrl.json"));

    #[derive(Default)]
    struct FailingConsumer;

    impl HistoryBatchConsumer for FailingConsumer {
        type Error = String;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn visits(&mut self, _batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl TakeoutSourceEvidenceConsumer<String> for FailingConsumer {
        fn source_evidence(&mut self, _chunk: TakeoutSourceEvidenceChunk) -> Result<(), String> {
            Err("sink offline".to_string())
        }
    }

    let mut failing = FailingConsumer;
    let sink_error = stream_payload_with_sink(
        "TypedUrl.json",
        KIND_TYPED_URL_JSON,
        br#"{"TypedUrl":[{"url":"https://example.com"}]}"#,
        1,
        TakeoutStreamOptions::default(),
        &mut failing,
    )
    .expect_err("source evidence sink error");
    assert!(sink_error.to_string().contains("sink offline"));

    let mut failing_at_final_flush = FailingConsumer;
    let final_flush_error = stream_payload_with_sink(
        "TypedUrl.json",
        KIND_TYPED_URL_JSON,
        br#"{"TypedUrl":[{"url":"https://example.com/final"}]}"#,
        10,
        TakeoutStreamOptions::default(),
        &mut failing_at_final_flush,
    )
    .expect_err("final source evidence sink error");
    assert!(final_flush_error.to_string().contains("sink offline"));
}

#[test]
fn parse_history_reads_directory_and_zip_sources() {
    let dir = tempdir().expect("tempdir");
    let chrome_dir = dir.path().join("Chrome");
    fs::create_dir_all(&chrome_dir).expect("create chrome dir");
    fs::write(
        chrome_dir.join("BrowserHistory.json"),
        browser_history_payload(&[
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000}"#,
        ]),
    )
    .expect("write browser history");
    fs::write(chrome_dir.join("Session.json"), r#"{"Session":[{"sessionTag":"device-1"}]}"#)
        .expect("write session");
    let parsed = parse_history(dir.path()).expect("parse directory");
    assert_eq!(parsed.visits.len(), 1);
    assert_eq!(parsed.native_entities.len(), 2);

    let zip_path = dir.path().join("takeout.zip");
    let file = fs::File::create(&zip_path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("Chrome/BrowserHistory.json", options).expect("start browser entry");
    zip.write_all(
        browser_history_payload(&[
            r#"{"url":"https://example.com/zip","title":"Zip","time_usec":1711965600000000}"#,
        ])
        .as_bytes(),
    )
    .expect("write browser entry");
    zip.start_file("Chrome/TypedUrl.json", options).expect("start typed entry");
    zip.write_all(br#"{"TypedUrl":[{"url":"https://example.com/zip","title":"Zip"}]}"#)
        .expect("write typed entry");
    zip.finish().expect("finish zip");

    let parsed_zip = parse_history(&zip_path).expect("parse zip");
    assert_eq!(parsed_zip.visits.len(), 1);
    assert_eq!(parsed_zip.native_entities.len(), 2);
}

#[test]
fn parse_history_returns_payload_parse_errors_from_streaming_path() {
    let dir = tempdir().expect("tempdir");
    let chrome_dir = dir.path().join("Chrome");
    fs::create_dir_all(&chrome_dir).expect("create chrome dir");
    fs::write(chrome_dir.join("BrowserHistory.json"), "{").expect("write malformed history");

    let error = parse_history(dir.path()).expect_err("malformed takeout history");

    assert!(error.to_string().contains("BrowserHistory.json"));
}

#[test]
fn merge_stream_reports_sums_counts_and_preserves_evidence_metadata() {
    fn payload_report(index: usize, available: bool) -> TakeoutPayloadStreamReport {
        TakeoutPayloadStreamReport {
            kind: format!("kind-{index}"),
            history: StreamedHistory {
                inspection: DatabaseInspection {
                    table_names: vec![format!("kind-{index}")],
                    warnings: Vec::new(),
                },
                schema_observation: SchemaObservation {
                    tables: vec![ObservedTable {
                        name: format!("table-{index}"),
                        present: true,
                        required: false,
                        row_count: Some(index as i64),
                        columns: vec![ObservedColumn {
                            name: format!("column-{index}"),
                            data_type: Some("TEXT".to_string()),
                            not_null: false,
                            primary_key_ordinal: 0,
                        }],
                    }],
                },
                capability_snapshot: crate::types::CapabilitySnapshot {
                    items: vec![CapabilityCoverage {
                        key: "shared.capability".to_string(),
                        available,
                        populated_rows: index,
                        total_rows: index + 10,
                        notes: vec![format!("note-{index}")],
                    }],
                },
                typed_evidence: TypedEvidenceBatch {
                    search: vec![SearchEvidence {
                        source_visit_id: Some(index as i64),
                        source_url_id: Some((index * 10) as i64),
                        evidence_key: format!("search-{index}"),
                        evidence_value: "pathkeep".to_string(),
                        normalized_value: Some("pathkeep".to_string()),
                        source_field: "term".to_string(),
                    }],
                    navigation: vec![NavigationEvidence {
                        source_visit_id: index as i64,
                        edge_kind: "from_visit".to_string(),
                        target_visit_id: Some((index + 1) as i64),
                        target_url: None,
                        transition: Some(index as i64),
                        source_field: "from_visit".to_string(),
                    }],
                    engagement: Vec::new(),
                    context: vec![ContextEvidence {
                        source_visit_id: Some(index as i64),
                        source_url_id: Some((index * 10) as i64),
                        context_key: format!("context-{index}"),
                        value_json: "true".to_string(),
                        source_field: "field".to_string(),
                    }],
                },
                native_entities: vec![NativeEntity {
                    entity_kind: "takeout-native".to_string(),
                    native_primary_key: format!("native-{index}"),
                    parent_native_primary_key: None,
                    payload_json: "{}".to_string(),
                    metadata: BTreeMap::new(),
                }],
                warnings: vec![ParserWarning {
                    code: format!("warning-{index}"),
                    message: format!("warning {index}"),
                }],
            },
            counts: TakeoutPayloadCounts {
                urls: index,
                visits: index + 1,
                downloads: index + 2,
                search_terms: index + 3,
                favicons: index + 4,
            },
            record_count: index + 5,
            skipped_missing_visit_time: index,
            earliest_visit_iso: Some(format!("earliest-{index}")),
            latest_visit_iso: Some(format!("latest-{index}")),
        }
    }

    let inspection = DatabaseInspection {
        table_names: vec!["takeout".to_string()],
        warnings: vec![ParserWarning {
            code: "source-warning".to_string(),
            message: "source warning".to_string(),
        }],
    };

    let merged = merge_stream_reports(
        inspection.clone(),
        vec![payload_report(1, false), payload_report(2, true)],
    );

    assert_eq!(merged.history.inspection, inspection);
    assert_eq!(merged.history.schema_observation.tables.len(), 2);
    assert_eq!(merged.history.typed_evidence.search.len(), 2);
    assert_eq!(merged.history.typed_evidence.navigation.len(), 2);
    assert_eq!(merged.history.typed_evidence.context.len(), 2);
    assert_eq!(merged.history.native_entities.len(), 2);
    assert_eq!(merged.history.warnings.len(), 2);
    assert_eq!(merged.counts.urls, 3);
    assert_eq!(merged.counts.visits, 5);
    assert_eq!(merged.counts.downloads, 7);
    assert_eq!(merged.counts.search_terms, 9);
    assert_eq!(merged.counts.favicons, 11);
    assert_eq!(merged.record_count, 13);
    assert_eq!(merged.skipped_missing_visit_time, 3);

    let capability = merged
        .history
        .capability_snapshot
        .items
        .iter()
        .find(|item| item.key == "shared.capability")
        .expect("merged capability");
    assert!(capability.available);
    assert_eq!(capability.populated_rows, 3);
    assert_eq!(capability.total_rows, 23);
    assert_eq!(capability.notes, vec!["note-1".to_string(), "note-2".to_string()]);
}

#[test]
fn parse_history_merges_reports_and_adapter_passthrough_edges() {
    let dir = tempdir().expect("tempdir");
    let chrome_dir = dir.path().join("Chrome");
    fs::create_dir_all(&chrome_dir).expect("create chrome dir");
    fs::write(chrome_dir.join("archive_browser.html"), "<html></html>").expect("write index");
    fs::write(chrome_dir.join("Bookmarks.json"), "{}").expect("write ignored chrome file");
    fs::write(
        chrome_dir.join("BrowserHistory.json"),
        browser_history_payload(&[
            r#"{"url":"https://example.com/merge","title":"First","time_usec":1711965600000000}"#,
        ]),
    )
    .expect("write first history");
    fs::write(
        chrome_dir.join("History.json"),
        browser_history_payload(&[
            r#"{"url":"https://example.com/merge","title":"Second","time_usec":1711969200000000}"#,
        ]),
    )
    .expect("write second history");

    let parsed = parse_history(dir.path()).expect("parse merged directory");
    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.urls[0].visit_count, 2);
    assert_eq!(parsed.urls[0].title.as_deref(), Some("Second"));
    assert_eq!(parsed.visits.len(), 2);

    #[derive(Default)]
    struct PassthroughRecorder {
        downloads: usize,
        search_terms: usize,
        favicons: usize,
    }

    impl HistoryBatchConsumer for PassthroughRecorder {
        type Error = std::convert::Infallible;

        fn urls(&mut self, _batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn visits(&mut self, _batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn downloads(&mut self, batch: Vec<ParsedDownload>) -> Result<(), Self::Error> {
            self.downloads += batch.len();
            Ok(())
        }

        fn search_terms(&mut self, batch: Vec<ParsedSearchTerm>) -> Result<(), Self::Error> {
            self.search_terms += batch.len();
            Ok(())
        }

        fn favicons(&mut self, batch: Vec<ParsedFavicon>) -> Result<(), Self::Error> {
            self.favicons += batch.len();
            Ok(())
        }
    }

    let mut recorder = PassthroughRecorder::default();
    let mut adapter = CanonicalOnlyTakeoutConsumer { inner: &mut recorder };
    adapter
        .downloads(vec![ParsedDownload {
            source_download_id: 1,
            guid: None,
            current_path: None,
            target_path: None,
            start_time_ms: None,
            start_time_iso: None,
            received_bytes: None,
            total_bytes: None,
            state: None,
            mime_type: None,
            original_mime_type: None,
        }])
        .expect("download passthrough");
    adapter
        .search_terms(vec![ParsedSearchTerm {
            keyword_id: 1,
            url_id: 1,
            term: "pathkeep".to_string(),
            normalized_term: "pathkeep".to_string(),
        }])
        .expect("search term passthrough");
    adapter
        .favicons(vec![ParsedFavicon {
            page_url: "https://example.com".to_string(),
            icon_url: "https://example.com/favicon.ico".to_string(),
            icon_type: None,
            width: 16,
            height: 16,
            last_updated_ms: 1,
            last_updated_iso: "1970-01-01T00:00:00+00:00".to_string(),
            image_data: None,
        }])
        .expect("favicon passthrough");
    assert_eq!(recorder.downloads, 1);
    assert_eq!(recorder.search_terms, 1);
    assert_eq!(recorder.favicons, 1);
}
