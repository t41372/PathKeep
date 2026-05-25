//! Self-validation for the Chromium History fixture writer.
//!
//! Every scenario test built on `browser-history-fixtures` ultimately relies on
//! one promise: the SQLite file we wrote is byte-faithful enough that the
//! production PathKeep parser reads back exactly the records we declared. If
//! that promise breaks, every downstream scenario is meaningless — a passing
//! assertion could just mean "writer and parser are silently aligned in their
//! shared mistake."
//!
//! This file is the gate. It exercises the smallest meaningful fixture
//! (two URLs, three visits, one revisit) and round-trips it through the real
//! `browser_history_parser::chromium::parse_history` entry point.

use browser_history_fixtures::{
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, chrome_time_to_unix_ms,
    unix_ms_to_chrome_time,
};
use browser_history_parser::{ChromiumReadCursor, HistoryDatabaseSet, chromium};
use tempfile::TempDir;

#[test]
fn chromium_fixture_round_trips_through_production_parser() {
    let temp = TempDir::new().expect("tempdir");
    let history_path = temp.path().join("History");

    // 2026-05-01T00:00:00Z, 2026-05-02T12:00:00Z, 2026-05-03T08:15:30Z
    let visit_one_ms = 1_777_680_000_000;
    let visit_two_ms = 1_777_809_600_000;
    let visit_three_ms = 1_777_872_930_000;

    ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/article-one".to_string(),
            title: Some("Article One".to_string()),
            visit_count: 2,
            typed_count: 1,
            last_visit_unix_ms: visit_two_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.org/article-two".to_string(),
            title: Some("Article Two".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: visit_three_ms,
            hidden: false,
        })
        .add_visit(ChromiumVisitRow {
            id: 10,
            url_id: 1,
            visit_time_unix_ms: visit_one_ms,
            from_visit: Some(0),
            transition: Some(805306368), // PAGE_TRANSITION_TYPED | CHAIN_START | CHAIN_END
            visit_duration_micros: Some(30_000_000),
            is_known_to_sync: true,
            visited_link_id: Some(42),
            external_referrer_url: None,
            app_id: None,
        })
        .add_visit(ChromiumVisitRow {
            id: 11,
            url_id: 1,
            visit_time_unix_ms: visit_two_ms,
            from_visit: Some(10),
            transition: Some(805306369), // PAGE_TRANSITION_LINK | ...
            visit_duration_micros: Some(15_500_000),
            is_known_to_sync: true,
            visited_link_id: Some(42),
            external_referrer_url: Some("https://referrer.example.net/".to_string()),
            app_id: None,
        })
        .add_visit(ChromiumVisitRow {
            id: 12,
            url_id: 2,
            visit_time_unix_ms: visit_three_ms,
            from_visit: Some(11),
            transition: Some(805306369),
            visit_duration_micros: None,
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: Some("app.example".to_string()),
        })
        .write(&history_path)
        .expect("write fixture");

    let parsed = chromium::parse_history(
        &HistoryDatabaseSet { history_path: history_path.clone(), favicons_path: None },
        ChromiumReadCursor::default(),
    )
    .expect("parse fixture");

    assert_eq!(parsed.urls.len(), 2, "parser should see exactly the URLs we wrote");
    assert_eq!(parsed.visits.len(), 3, "parser should see exactly the visits we wrote");

    let url_one = parsed.urls.iter().find(|url| url.source_url_id == 1).expect("url id 1");
    assert_eq!(url_one.url, "https://example.com/article-one");
    assert_eq!(url_one.title.as_deref(), Some("Article One"));
    assert_eq!(url_one.visit_count, 2);
    assert_eq!(url_one.typed_count, 1);
    assert_eq!(url_one.last_visit_ms, visit_two_ms);
    assert!(!url_one.hidden);

    let url_two = parsed.urls.iter().find(|url| url.source_url_id == 2).expect("url id 2");
    assert_eq!(url_two.url, "https://example.org/article-two");
    assert_eq!(url_two.last_visit_ms, visit_three_ms);

    let visit_one =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 10).expect("visit id 10");
    assert_eq!(visit_one.source_url_id, 1);
    assert_eq!(visit_one.visit_time_ms, visit_one_ms);
    assert_eq!(visit_one.transition, Some(805306368));
    // Despite the field name `visit_duration_ms`, the Chromium parser passes
    // the raw `visits.visit_duration` value through, which Chrome itself
    // stores as microseconds. This is a known naming inconsistency in
    // production code (see import-dedup-audit.md); the fixture writes the
    // value in Chrome's native microsecond unit and the round-trip confirms.
    assert_eq!(visit_one.visit_duration_ms, Some(30_000_000));
    assert!(visit_one.is_known_to_sync);
    assert_eq!(visit_one.visited_link_id, Some(42));

    let visit_two =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 11).expect("visit id 11");
    assert_eq!(visit_two.from_visit, Some(10));
    assert_eq!(
        visit_two.external_referrer_url.as_deref(),
        Some("https://referrer.example.net/")
    );

    let visit_three =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 12).expect("visit id 12");
    assert_eq!(visit_three.source_url_id, 2);
    assert_eq!(visit_three.app_id.as_deref(), Some("app.example"));
    assert!(!visit_three.is_known_to_sync);
}

#[test]
fn time_helpers_match_production_offset() {
    let unix_ms = 1_777_809_600_000;
    let chrome = unix_ms_to_chrome_time(unix_ms);
    assert_eq!(chrome_time_to_unix_ms(chrome), unix_ms);

    // Pin the constant: 2026-05-02T12:00:00Z in Unix ms is exactly
    // 13_422_283_200_000_000 in Chrome microseconds-since-1601.
    assert_eq!(chrome, 13_422_283_200_000_000);
}
