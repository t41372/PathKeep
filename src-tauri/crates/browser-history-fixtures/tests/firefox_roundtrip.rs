//! Self-validation for the Firefox `places.sqlite` fixture writer.
//!
//! Mirrors the Chromium round-trip pattern: build a small fixture, parse it
//! back through `browser_history_parser::firefox::parse_history`, and assert
//! every emitted field matches what the fixture promised.

use browser_history_fixtures::{
    FirefoxPlaceRow, FirefoxPlacesFixture, FirefoxVisitRow, firefox_time_to_unix_ms,
    unix_ms_to_firefox_time,
};
use browser_history_parser::firefox;
use tempfile::TempDir;

#[test]
fn firefox_fixture_round_trips_through_production_parser() {
    let temp = TempDir::new().expect("tempdir");
    let history_path = temp.path().join("places.sqlite");

    let visit_one_ms = 1_777_680_000_000;
    let visit_two_ms = 1_777_809_600_000;
    let visit_three_ms = 1_777_872_930_000;

    FirefoxPlacesFixture::new()
        .add_place(FirefoxPlaceRow {
            id: 7,
            url: "https://example.com/firefox-one".to_string(),
            title: Some("Firefox Example One".to_string()),
            visit_count: 2,
            hidden: false,
            last_visit_unix_ms: visit_two_ms,
        })
        .add_place(FirefoxPlaceRow {
            id: 8,
            url: "https://example.org/firefox-two".to_string(),
            title: Some("Firefox Example Two".to_string()),
            visit_count: 1,
            hidden: false,
            last_visit_unix_ms: visit_three_ms,
        })
        .add_visit(FirefoxVisitRow {
            id: 11,
            place_id: 7,
            visit_time_unix_ms: visit_one_ms,
            from_visit: None,
            visit_type: Some(1),
        })
        .add_visit(FirefoxVisitRow {
            id: 12,
            place_id: 7,
            visit_time_unix_ms: visit_two_ms,
            from_visit: Some(11),
            visit_type: Some(1),
        })
        .add_visit(FirefoxVisitRow {
            id: 13,
            place_id: 8,
            visit_time_unix_ms: visit_three_ms,
            from_visit: Some(12),
            visit_type: Some(2),
        })
        .write(&history_path)
        .expect("write firefox fixture");

    let parsed = firefox::parse_history(&history_path, 0, 0).expect("parse firefox fixture");

    assert_eq!(parsed.urls.len(), 2);
    assert_eq!(parsed.visits.len(), 3);

    let url_seven = parsed.urls.iter().find(|url| url.source_url_id == 7).expect("place 7");
    assert_eq!(url_seven.url, "https://example.com/firefox-one");
    assert_eq!(url_seven.title.as_deref(), Some("Firefox Example One"));
    assert_eq!(url_seven.visit_count, 2);
    assert_eq!(url_seven.last_visit_ms, visit_two_ms);
    assert!(!url_seven.hidden);

    let visit_eleven =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 11).expect("visit 11");
    assert_eq!(visit_eleven.source_url_id, 7);
    assert_eq!(visit_eleven.visit_time_ms, visit_one_ms);
    assert_eq!(visit_eleven.transition, Some(1));
    assert_eq!(visit_eleven.from_visit, None);
    assert_eq!(visit_eleven.app_id.as_deref(), Some("firefox"));

    let visit_twelve =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 12).expect("visit 12");
    assert_eq!(visit_twelve.from_visit, Some(11));
    assert_eq!(visit_twelve.visit_time_ms, visit_two_ms);

    let visit_thirteen =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 13).expect("visit 13");
    assert_eq!(visit_thirteen.source_url_id, 8);
    assert_eq!(visit_thirteen.from_visit, Some(12));
}

#[test]
fn firefox_time_helpers_match_production_offset() {
    let unix_ms = 1_777_809_600_000;
    let firefox = unix_ms_to_firefox_time(unix_ms);
    assert_eq!(firefox_time_to_unix_ms(firefox), unix_ms);
    assert_eq!(firefox, 1_777_809_600_000_000);
}
