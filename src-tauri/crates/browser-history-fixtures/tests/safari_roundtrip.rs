//! Self-validation for the Safari `History.db` fixture writer.
//!
//! Covers both the minimal and current macOS Safari schema variants. The
//! current variant exercises the parser's optional-column probing path
//! (`load_successful`, `synthesized`, `redirect_*`, `score`).

use browser_history_fixtures::{
    SafariHistoryFixture, SafariHistoryItemRow, SafariHistoryVisitRow, SafariSchemaVariant,
    safari_time_to_unix_ms, unix_ms_to_safari_time,
};
use browser_history_parser::safari;
use tempfile::TempDir;

#[test]
fn safari_minimal_fixture_round_trips_through_production_parser() {
    let temp = TempDir::new().expect("tempdir");
    let history_path = temp.path().join("History.db");

    let visit_one_ms = 1_777_680_000_000;
    let visit_two_ms = 1_777_809_600_000;

    SafariHistoryFixture::new()
        .with_variant(SafariSchemaVariant::Minimal)
        .add_item(SafariHistoryItemRow {
            id: 5,
            url: "https://example.com/safari".to_string(),
        })
        .add_visit(SafariHistoryVisitRow {
            id: 9,
            history_item: 5,
            title: Some("Safari Example One".to_string()),
            visit_time_unix_ms: visit_one_ms,
            load_successful: None,
            http_non_get: None,
            synthesized: None,
            redirect_source: None,
            redirect_destination: None,
            origin: None,
            generation: None,
            attributes: None,
            score: None,
        })
        .add_visit(SafariHistoryVisitRow {
            id: 10,
            history_item: 5,
            title: Some("Safari Example Two".to_string()),
            visit_time_unix_ms: visit_two_ms,
            load_successful: None,
            http_non_get: None,
            synthesized: None,
            redirect_source: None,
            redirect_destination: None,
            origin: None,
            generation: None,
            attributes: None,
            score: None,
        })
        .write(&history_path)
        .expect("write minimal safari fixture");

    let parsed = safari::parse_history(&history_path, 0, 0).expect("parse minimal safari fixture");

    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.visits.len(), 2);

    let url = &parsed.urls[0];
    assert_eq!(url.url, "https://example.com/safari");
    assert_eq!(url.visit_count, 2);
    assert_eq!(url.last_visit_ms, visit_two_ms);

    let visit_nine =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 9).expect("visit 9");
    assert_eq!(visit_nine.visit_time_ms, visit_one_ms);
    assert_eq!(visit_nine.title.as_deref(), Some("Safari Example One"));
    assert_eq!(visit_nine.app_id.as_deref(), Some("safari"));
}

#[test]
fn safari_current_fixture_round_trips_through_production_parser() {
    let temp = TempDir::new().expect("tempdir");
    let history_path = temp.path().join("History.db");

    let visit_one_ms = 1_777_680_000_000;

    SafariHistoryFixture::new()
        .with_variant(SafariSchemaVariant::Current)
        .add_item(SafariHistoryItemRow {
            id: 5,
            url: "https://example.com/safari-current".to_string(),
        })
        .add_visit(SafariHistoryVisitRow {
            id: 9,
            history_item: 5,
            title: Some("Safari Current Schema".to_string()),
            visit_time_unix_ms: visit_one_ms,
            load_successful: Some(true),
            http_non_get: Some(false),
            synthesized: Some(false),
            redirect_source: None,
            redirect_destination: Some(10),
            origin: Some(1),
            generation: Some(2),
            attributes: Some(4),
            score: Some(0.75),
        })
        .write(&history_path)
        .expect("write current safari fixture");

    let parsed = safari::parse_history(&history_path, 0, 0).expect("parse current safari fixture");

    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(parsed.visits.len(), 1);
    assert_eq!(parsed.urls[0].url, "https://example.com/safari-current");
    assert_eq!(parsed.visits[0].visit_time_ms, visit_one_ms);
}

#[test]
fn safari_time_helpers_match_production_offset() {
    let unix_ms = 1_777_809_600_000;
    let safari = unix_ms_to_safari_time(unix_ms);
    let back = safari_time_to_unix_ms(safari);
    assert_eq!(back, unix_ms);

    // Unix epoch zero maps to a negative CFAbsoluteTime since the Cocoa
    // epoch is in 2001. Production helpers clamp negatives back to zero on
    // the inverse path, so the pinning here is one-way.
    let cocoa_epoch_unix_ms = 978_307_200_000;
    assert!((unix_ms_to_safari_time(cocoa_epoch_unix_ms)).abs() < 0.001);
}
