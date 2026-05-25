//! Self-validation for the Google Takeout payload writer.
//!
//! Exercises all three on-disk formats the parser accepts: the standard
//! `Browser History` key, the alternate `BrowserHistory` (no space) key,
//! and JSONL. Records flow through `browser_history_parser::takeout` so
//! the test pins the field-name contract Google ships today.

use browser_history_fixtures::{
    TakeoutBrowserHistoryFixture, TakeoutBrowserRecord, TakeoutPayloadFormat,
};
use browser_history_parser::takeout;
use tempfile::TempDir;

fn record(url: &str, title: &str, visit_time_unix_ms: i64) -> TakeoutBrowserRecord {
    TakeoutBrowserRecord {
        url: url.to_string(),
        title: Some(title.to_string()),
        visit_time_unix_ms,
        page_transition: Some("LINK".to_string()),
        client_id: Some("synthetic-client-id".to_string()),
        favicon_url: Some(format!("{url}/favicon.ico")),
    }
}

#[test]
fn takeout_standard_json_round_trips_through_production_parser() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("Chrome/BrowserHistory.json");

    let visit_one = 1_777_680_000_000;
    let visit_two = 1_777_809_600_000;

    TakeoutBrowserHistoryFixture::new()
        .add_record(record("https://example.com/page-one", "Example Page One", visit_one))
        .add_record(record("https://example.org/page-two", "Example Page Two", visit_two))
        .write(&path)
        .expect("write standard takeout fixture");

    let parsed = takeout::parse_history(&path).expect("parse takeout payload");

    // Takeout dedups URL rows by URL identity; two records to two URLs = 2.
    assert_eq!(parsed.urls.len(), 2);
    assert_eq!(parsed.visits.len(), 2);

    let urls_by_url: std::collections::HashMap<_, _> =
        parsed.urls.iter().map(|url| (url.url.clone(), url)).collect();
    let url_one = urls_by_url
        .get("https://example.com/page-one")
        .expect("page-one parsed url");
    assert_eq!(url_one.title.as_deref(), Some("Example Page One"));
    assert_eq!(url_one.last_visit_ms, visit_one);

    let visits_by_url: std::collections::HashMap<_, _> =
        parsed.visits.iter().map(|visit| (visit.url.clone(), visit)).collect();
    let visit_two_record = visits_by_url
        .get("https://example.org/page-two")
        .expect("page-two parsed visit");
    assert_eq!(visit_two_record.visit_time_ms, visit_two);
    assert_eq!(visit_two_record.app_id.as_deref(), Some("takeout"));
    assert_eq!(visit_two_record.transition, None);
}

#[test]
fn takeout_alternate_key_round_trips() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("Chrome/BrowserHistory.json");

    TakeoutBrowserHistoryFixture::new()
        .with_format(TakeoutPayloadFormat::AlternateBrowserHistoryJson)
        .add_record(record("https://example.com/alt", "Alt", 1_777_680_000_000))
        .write(&path)
        .expect("write alternate-key takeout fixture");

    let parsed = takeout::parse_history(&path).expect("parse alternate-key payload");
    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.visits.len(), 1);
    assert_eq!(parsed.urls[0].url, "https://example.com/alt");
}

#[test]
fn takeout_jsonl_round_trips() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("BrowserHistory.jsonl");

    TakeoutBrowserHistoryFixture::new()
        .with_format(TakeoutPayloadFormat::JsonLines)
        .add_record(record("https://example.com/jsonl-one", "One", 1_777_680_000_000))
        .add_record(record("https://example.com/jsonl-two", "Two", 1_777_809_600_000))
        .write(&path)
        .expect("write jsonl takeout fixture");

    let parsed = takeout::parse_history(&path).expect("parse jsonl payload");
    assert_eq!(parsed.urls.len(), 2);
    assert_eq!(parsed.visits.len(), 2);
}
