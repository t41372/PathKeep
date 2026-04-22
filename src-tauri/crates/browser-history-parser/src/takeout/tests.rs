//! Regression tests for the Takeout parser boundary.

use super::*;
use crate::types::{HistoryBatchConsumer, ParsedUrl, ParsedVisit};
use std::{fs, io::Write};
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

fn browser_history_payload(records: &[&str]) -> String {
    format!(r#"{{"Browser History":[{}]}}"#, records.join(","))
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
fn classify_payload_path_handles_localized_history_and_review_only_paths() {
    let english = classify_payload_path("Chrome/History.json");
    assert_eq!(english.recognized_kind, Some(KIND_BROWSER_JSON));
    assert_eq!(english.locale, Some("en"));
    assert_eq!(english.disposition, TakeoutPathDisposition::WillImport);

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
            r#"{"url":"https://example.com","title":"Example","time_usec":1711965600000000,"client_id":"abc"}"#,
        ])
        .as_bytes(),
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
        browser_history_payload(&[
            r#"{"url":"https://example.com/one","title":"One","time_usec":1711965600000000}"#,
            r#"{"url":"https://example.com/two","title":"Two","time_usec":1711969200000000}"#,
        ])
        .as_bytes(),
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

    assert!(typed.history.visits.is_empty());
    assert_eq!(typed.history.native_entities.len(), 1);
    assert_eq!(session.history.native_entities.len(), 1);
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
