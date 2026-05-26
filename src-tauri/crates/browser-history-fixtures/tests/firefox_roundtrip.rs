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

    // --- URL-level assertions: all ParsedUrl fields ---

    let url_seven = parsed.urls.iter().find(|url| url.source_url_id == 7).expect("place 7");
    assert_eq!(url_seven.url, "https://example.com/firefox-one");
    assert_eq!(url_seven.title.as_deref(), Some("Firefox Example One"));
    assert_eq!(url_seven.visit_count, 2);
    assert_eq!(url_seven.last_visit_ms, visit_two_ms);
    assert!(!url_seven.hidden);
    // Firefox parser hardcodes typed_count to 0 (Firefox stores typed counts
    // differently than Chromium — the parser does not extract them).
    assert_eq!(url_seven.typed_count, 0);
    // last_visit_iso is derived from the Firefox microsecond timestamp.
    assert!(!url_seven.last_visit_iso.is_empty(), "last_visit_iso should be populated");

    let url_eight = parsed.urls.iter().find(|url| url.source_url_id == 8).expect("place 8");
    assert_eq!(url_eight.url, "https://example.org/firefox-two");
    assert_eq!(url_eight.title.as_deref(), Some("Firefox Example Two"));
    assert_eq!(url_eight.visit_count, 1);
    assert_eq!(url_eight.last_visit_ms, visit_three_ms);
    assert!(!url_eight.hidden);
    assert_eq!(url_eight.typed_count, 0);

    // --- Visit-level assertions: all ParsedVisit fields ---

    let visit_eleven =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 11).expect("visit 11");
    assert_eq!(visit_eleven.source_url_id, 7);
    assert_eq!(visit_eleven.visit_time_ms, visit_one_ms);
    // visit_time_iso is derived from the Firefox microsecond timestamp.
    assert!(
        !visit_eleven.visit_time_iso.is_empty(),
        "visit_time_iso should be populated for visit 11"
    );
    assert_eq!(visit_eleven.transition, Some(1));
    assert_eq!(visit_eleven.from_visit, None);
    assert_eq!(visit_eleven.app_id.as_deref(), Some("firefox"));
    // url field on visits is populated from the JOIN with moz_places.
    assert_eq!(visit_eleven.url, "https://example.com/firefox-one");
    assert_eq!(visit_eleven.title.as_deref(), Some("Firefox Example One"));
    // Firefox parser hardcodes these fields — verify the contract.
    assert_eq!(visit_eleven.visit_duration_ms, None);
    assert!(!visit_eleven.is_known_to_sync);
    assert_eq!(visit_eleven.visited_link_id, None);
    assert_eq!(visit_eleven.external_referrer_url, None);

    let visit_twelve =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 12).expect("visit 12");
    assert_eq!(visit_twelve.source_url_id, 7);
    assert_eq!(visit_twelve.from_visit, Some(11));
    assert_eq!(visit_twelve.visit_time_ms, visit_two_ms);
    assert!(
        !visit_twelve.visit_time_iso.is_empty(),
        "visit_time_iso should be populated for visit 12"
    );
    assert_eq!(visit_twelve.transition, Some(1));
    assert_eq!(visit_twelve.url, "https://example.com/firefox-one");
    assert_eq!(visit_twelve.app_id.as_deref(), Some("firefox"));
    assert_eq!(visit_twelve.visit_duration_ms, None);
    assert!(!visit_twelve.is_known_to_sync);
    assert_eq!(visit_twelve.visited_link_id, None);
    assert_eq!(visit_twelve.external_referrer_url, None);

    let visit_thirteen =
        parsed.visits.iter().find(|visit| visit.source_visit_id == 13).expect("visit 13");
    assert_eq!(visit_thirteen.source_url_id, 8);
    assert_eq!(visit_thirteen.from_visit, Some(12));
    assert_eq!(visit_thirteen.visit_time_ms, visit_three_ms);
    assert!(
        !visit_thirteen.visit_time_iso.is_empty(),
        "visit_time_iso should be populated for visit 13"
    );
    assert_eq!(visit_thirteen.transition, Some(2));
    assert_eq!(visit_thirteen.url, "https://example.org/firefox-two");
    assert_eq!(visit_thirteen.title.as_deref(), Some("Firefox Example Two"));
    assert_eq!(visit_thirteen.app_id.as_deref(), Some("firefox"));
    assert_eq!(visit_thirteen.visit_duration_ms, None);
    assert!(!visit_thirteen.is_known_to_sync);
    assert_eq!(visit_thirteen.visited_link_id, None);
    assert_eq!(visit_thirteen.external_referrer_url, None);
}

#[test]
fn firefox_null_visit_count_defaults_to_zero() {
    // Firefox's `moz_places.visit_count` can be NULL in corrupted or very old
    // databases. The production parser uses `unwrap_or_default()` on the
    // `Option<i64>` read from SQLite, which coerces NULL to 0.
    //
    // The fixture builder's `FirefoxPlaceRow.visit_count` is non-optional to
    // stay backward-compatible with downstream callers, so this test writes
    // the NULL value directly via SQL.

    let temp = TempDir::new().expect("tempdir");
    let history_path = temp.path().join("places.sqlite");

    let visit_ms = 1_777_680_000_000;

    // Write a minimal fixture, then overwrite visit_count with NULL.
    FirefoxPlacesFixture::new()
        .add_place(FirefoxPlaceRow {
            id: 20,
            url: "https://example.com/null-visit-count".to_string(),
            title: Some("Null Visit Count".to_string()),
            visit_count: 0,
            hidden: false,
            last_visit_unix_ms: visit_ms,
        })
        .add_visit(FirefoxVisitRow {
            id: 30,
            place_id: 20,
            visit_time_unix_ms: visit_ms,
            from_visit: None,
            visit_type: Some(1),
        })
        .write(&history_path)
        .expect("write firefox fixture for null-visit-count test");

    // Patch visit_count to NULL directly so the parser's unwrap_or_default()
    // path is exercised.
    {
        let connection = rusqlite::Connection::open(&history_path).expect("open for null patching");
        connection
            .execute("UPDATE moz_places SET visit_count = NULL WHERE id = 20", [])
            .expect("set visit_count to NULL");
    }

    let parsed = firefox::parse_history(&history_path, 0, 0)
        .expect("parse null-visit-count firefox fixture");

    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(
        parsed.urls[0].visit_count, 0,
        "NULL visit_count should default to 0 via unwrap_or_default()"
    );
    assert_eq!(parsed.urls[0].url, "https://example.com/null-visit-count");
}

#[test]
fn firefox_null_last_visit_date_defaults_to_zero() {
    // Firefox's `moz_places.last_visit_date` can be NULL for places that
    // Firefox created but never actually visited (e.g. bookmarks without visits).
    // The production parser uses `COALESCE(last_visit_date, 0)` in the SQL
    // query, so NULL becomes 0 microseconds, which maps to Unix ms 0.
    //
    // Same approach as null-visit-count: write via the builder, patch to NULL.

    let temp = TempDir::new().expect("tempdir");
    let history_path = temp.path().join("places.sqlite");

    FirefoxPlacesFixture::new()
        .add_place(FirefoxPlaceRow {
            id: 21,
            url: "https://example.com/null-last-visit".to_string(),
            title: Some("Null Last Visit".to_string()),
            visit_count: 0,
            hidden: false,
            last_visit_unix_ms: 0,
        })
        .add_visit(FirefoxVisitRow {
            id: 31,
            place_id: 21,
            visit_time_unix_ms: 1_777_680_000_000,
            from_visit: None,
            visit_type: Some(1),
        })
        .write(&history_path)
        .expect("write firefox fixture for null-last-visit test");

    // Patch last_visit_date to NULL so the parser's COALESCE path is exercised.
    {
        let connection = rusqlite::Connection::open(&history_path).expect("open for null patching");
        connection
            .execute("UPDATE moz_places SET last_visit_date = NULL WHERE id = 21", [])
            .expect("set last_visit_date to NULL");
    }

    // Use after_url_last_visit_ms=0 so the NULL-coalesced row qualifies.
    let parsed =
        firefox::parse_history(&history_path, 0, 0).expect("parse null-last-visit firefox fixture");

    assert_eq!(parsed.urls.len(), 1);
    assert_eq!(
        parsed.urls[0].last_visit_ms, 0,
        "NULL last_visit_date should coalesce to 0 via COALESCE"
    );
    assert_eq!(parsed.urls[0].url, "https://example.com/null-last-visit");
    // Visit should still parse correctly despite the NULL on the URL row.
    assert_eq!(parsed.visits.len(), 1);
    assert_eq!(parsed.visits[0].source_url_id, 21);
}

#[test]
fn firefox_time_helpers_match_production_offset() {
    let unix_ms = 1_777_809_600_000;
    let firefox = unix_ms_to_firefox_time(unix_ms);
    assert_eq!(firefox_time_to_unix_ms(firefox), unix_ms);
    assert_eq!(firefox, 1_777_809_600_000_000);
}
