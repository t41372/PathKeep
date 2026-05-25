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
    let url_one = urls_by_url.get("https://example.com/page-one").expect("page-one parsed url");
    assert_eq!(url_one.title.as_deref(), Some("Example Page One"));
    assert_eq!(url_one.last_visit_ms, visit_one);

    let url_two = urls_by_url.get("https://example.org/page-two").expect("page-two parsed url");
    assert_eq!(url_two.title.as_deref(), Some("Example Page Two"));
    assert_eq!(url_two.last_visit_ms, visit_two);
    // Takeout parser hardcodes typed_count to 0 and hidden to false.
    assert_eq!(url_two.typed_count, 0);
    assert!(!url_two.hidden);

    let visits_by_url: std::collections::HashMap<_, _> =
        parsed.visits.iter().map(|visit| (visit.url.clone(), visit)).collect();

    let visit_one_record =
        visits_by_url.get("https://example.com/page-one").expect("page-one parsed visit");
    assert_eq!(visit_one_record.visit_time_ms, visit_one);
    assert_eq!(visit_one_record.app_id.as_deref(), Some("takeout"));
    assert_eq!(visit_one_record.title.as_deref(), Some("Example Page One"));
    assert_eq!(visit_one_record.url, "https://example.com/page-one");
    // Takeout parser hardcodes these visit-level fields.
    assert_eq!(visit_one_record.transition, None);
    assert_eq!(visit_one_record.from_visit, None);
    assert_eq!(visit_one_record.visit_duration_ms, None);
    assert!(!visit_one_record.is_known_to_sync);
    assert_eq!(visit_one_record.visited_link_id, None);
    assert_eq!(visit_one_record.external_referrer_url, None);
    assert!(!visit_one_record.visit_time_iso.is_empty(), "visit_time_iso should be populated");

    let visit_two_record =
        visits_by_url.get("https://example.org/page-two").expect("page-two parsed visit");
    assert_eq!(visit_two_record.visit_time_ms, visit_two);
    assert_eq!(visit_two_record.app_id.as_deref(), Some("takeout"));
    assert_eq!(visit_two_record.transition, None);

    // --- client_id and favicon_url surface as context evidence ---

    // client_id → ContextEvidence with key "context.takeout.client_id"
    let client_id_evidence: Vec<_> = parsed
        .typed_evidence
        .context
        .iter()
        .filter(|ctx| ctx.context_key == "context.takeout.client_id")
        .collect();
    assert_eq!(
        client_id_evidence.len(),
        2,
        "each record with client_id should produce one context evidence row"
    );
    assert!(
        client_id_evidence.iter().all(|ctx| ctx.value_json.contains("synthetic-client-id")),
        "client_id evidence should contain the fixture value"
    );

    // favicon_url → ContextEvidence with key "context.takeout.favicon_url"
    let favicon_evidence: Vec<_> = parsed
        .typed_evidence
        .context
        .iter()
        .filter(|ctx| ctx.context_key == "context.takeout.favicon_url")
        .collect();
    assert_eq!(
        favicon_evidence.len(),
        2,
        "each record with favicon_url should produce one context evidence row"
    );
    assert!(
        favicon_evidence.iter().any(|ctx| ctx.value_json.contains("page-one/favicon.ico")),
        "favicon evidence should contain the page-one favicon URL"
    );
    assert!(
        favicon_evidence.iter().any(|ctx| ctx.value_json.contains("page-two/favicon.ico")),
        "favicon evidence should contain the page-two favicon URL"
    );

    // page_transition → ContextEvidence with key "context.takeout.page_transition"
    let transition_evidence: Vec<_> = parsed
        .typed_evidence
        .context
        .iter()
        .filter(|ctx| ctx.context_key == "context.takeout.page_transition")
        .collect();
    assert_eq!(
        transition_evidence.len(),
        2,
        "each record with page_transition should produce one context evidence row"
    );
    assert!(
        transition_evidence.iter().all(|ctx| ctx.value_json.contains("LINK")),
        "page_transition evidence should contain the LINK value"
    );
}

#[test]
fn takeout_alternate_key_round_trips() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("Chrome/BrowserHistory.json");

    let visit_ms = 1_777_680_000_000;

    TakeoutBrowserHistoryFixture::new()
        .with_format(TakeoutPayloadFormat::AlternateBrowserHistoryJson)
        .add_record(record("https://example.com/alt", "Alt", visit_ms))
        .write(&path)
        .expect("write alternate-key takeout fixture");

    let parsed = takeout::parse_history(&path).expect("parse alternate-key payload");
    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.visits.len(), 1);
    assert_eq!(parsed.urls[0].url, "https://example.com/alt");
    assert_eq!(parsed.urls[0].title.as_deref(), Some("Alt"));
    assert_eq!(parsed.urls[0].last_visit_ms, visit_ms);
    assert_eq!(parsed.urls[0].visit_count, 1);

    assert_eq!(parsed.visits[0].url, "https://example.com/alt");
    assert_eq!(parsed.visits[0].title.as_deref(), Some("Alt"));
    assert_eq!(parsed.visits[0].visit_time_ms, visit_ms);
    assert_eq!(parsed.visits[0].app_id.as_deref(), Some("takeout"));

    // Context evidence for the alternate-key format should contain client_id.
    assert!(
        parsed
            .typed_evidence
            .context
            .iter()
            .any(|ctx| ctx.context_key == "context.takeout.client_id"),
        "alternate-key format should preserve client_id evidence"
    );
}

#[test]
fn takeout_jsonl_round_trips() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("BrowserHistory.jsonl");

    let visit_one_ms = 1_777_680_000_000;
    let visit_two_ms = 1_777_809_600_000;

    TakeoutBrowserHistoryFixture::new()
        .with_format(TakeoutPayloadFormat::JsonLines)
        .add_record(record("https://example.com/jsonl-one", "One", visit_one_ms))
        .add_record(record("https://example.com/jsonl-two", "Two", visit_two_ms))
        .write(&path)
        .expect("write jsonl takeout fixture");

    let parsed = takeout::parse_history(&path).expect("parse jsonl payload");
    assert_eq!(parsed.urls.len(), 2);
    assert_eq!(parsed.visits.len(), 2);

    let urls_by_url: std::collections::HashMap<_, _> =
        parsed.urls.iter().map(|url| (url.url.clone(), url)).collect();
    let jsonl_one = urls_by_url.get("https://example.com/jsonl-one").expect("jsonl-one url");
    assert_eq!(jsonl_one.title.as_deref(), Some("One"));
    assert_eq!(jsonl_one.last_visit_ms, visit_one_ms);
    assert_eq!(jsonl_one.visit_count, 1);

    let jsonl_two = urls_by_url.get("https://example.com/jsonl-two").expect("jsonl-two url");
    assert_eq!(jsonl_two.title.as_deref(), Some("Two"));
    assert_eq!(jsonl_two.last_visit_ms, visit_two_ms);

    let visits_by_url: std::collections::HashMap<_, _> =
        parsed.visits.iter().map(|visit| (visit.url.clone(), visit)).collect();
    let visit_one =
        visits_by_url.get("https://example.com/jsonl-one").expect("jsonl-one parsed visit");
    assert_eq!(visit_one.visit_time_ms, visit_one_ms);
    assert_eq!(visit_one.app_id.as_deref(), Some("takeout"));
    assert_eq!(visit_one.title.as_deref(), Some("One"));

    let visit_two =
        visits_by_url.get("https://example.com/jsonl-two").expect("jsonl-two parsed visit");
    assert_eq!(visit_two.visit_time_ms, visit_two_ms);
    assert_eq!(visit_two.app_id.as_deref(), Some("takeout"));

    // JSONL format should also capture context evidence (client_id, favicon_url).
    assert!(
        parsed
            .typed_evidence
            .context
            .iter()
            .any(|ctx| ctx.context_key == "context.takeout.client_id"),
        "JSONL format should preserve client_id evidence"
    );
    assert!(
        parsed
            .typed_evidence
            .context
            .iter()
            .any(|ctx| ctx.context_key == "context.takeout.favicon_url"),
        "JSONL format should preserve favicon_url evidence"
    );
}
