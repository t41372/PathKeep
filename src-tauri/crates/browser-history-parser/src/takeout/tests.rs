//! Regression tests for the Takeout parser boundary.

use super::*;
use crate::types::{HistoryBatchConsumer, ParsedUrl, ParsedVisit};
use std::{fs, io::Write};
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

#[test]
fn inspect_history_reports_supported_takeout_payloads() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("BrowserHistory.json"),
        r#"{"BrowserHistory":[{"titleUrl":"https://example.com","pageTitle":"Example","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#,
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
fn parse_payload_extracts_browser_history_records() {
    let report = parse_payload(
        "BrowserHistory.json",
        KIND_BROWSER_JSON,
        br#"{"BrowserHistory":[{"titleUrl":"https://example.com","pageTitle":"Example","visitedAt":"2026-04-01T10:00:00+00:00","client_id":"abc"}]}"#,
    )
    .expect("parse payload");
    assert_eq!(report.record_count, 1);
    assert_eq!(report.history.visits.len(), 1);
    assert_eq!(report.history.urls.len(), 1);
    assert_eq!(report.history.native_entities.len(), 1);
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
        br#"{"BrowserHistory":[
            {"titleUrl":"https://example.com/one","pageTitle":"One","visitedAt":"2026-04-01T10:00:00+00:00"},
            {"titleUrl":"https://example.com/two","pageTitle":"Two","visitedAt":"2026-04-01T11:00:00+00:00"}
        ]}"#,
        1,
        &mut consumer,
    )
    .expect("stream payload");

    assert_eq!(report.counts.urls, 2);
    assert_eq!(report.counts.visits, 2);
    assert_eq!(report.record_count, 2);
    assert_eq!(consumer.urls.len(), 2);
    assert_eq!(consumer.visits.len(), 2);
    assert_eq!(consumer.visits.iter().map(Vec::len).sum::<usize>(), 2);
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

    assert!(typed.history.visits.is_empty());
    assert_eq!(typed.history.native_entities.len(), 1);
    assert_eq!(session.history.native_entities.len(), 1);
}

#[test]
fn parse_history_reads_directory_and_zip_sources() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("BrowserHistory.json"),
        r#"{"BrowserHistory":[{"titleUrl":"https://example.com","pageTitle":"Example","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#,
    )
    .expect("write browser history");
    fs::write(dir.path().join("Session.json"), r#"{"Session":[{"sessionTag":"device-1"}]}"#)
        .expect("write session");
    let parsed = parse_history(dir.path()).expect("parse directory");
    assert_eq!(parsed.visits.len(), 1);
    assert_eq!(parsed.native_entities.len(), 2);

    let zip_path = dir.path().join("takeout.zip");
    let file = fs::File::create(&zip_path).expect("create zip");
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("BrowserHistory.json", options).expect("start browser entry");
    zip.write_all(br#"{"BrowserHistory":[{"titleUrl":"https://example.com/zip","pageTitle":"Zip","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#)
        .expect("write browser entry");
    zip.start_file("TypedUrl.json", options).expect("start typed entry");
    zip.write_all(br#"{"TypedUrl":[{"url":"https://example.com/zip","title":"Zip"}]}"#)
        .expect("write typed entry");
    zip.finish().expect("finish zip");

    let parsed_zip = parse_history(&zip_path).expect("parse zip");
    assert_eq!(parsed_zip.visits.len(), 1);
    assert_eq!(parsed_zip.native_entities.len(), 2);
}
