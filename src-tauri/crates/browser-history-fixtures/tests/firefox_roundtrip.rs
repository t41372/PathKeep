//! A NULL `visit_count` or `last_visit_date` in an old or corrupt `places.sqlite` must not
//! drop the place or its visits: the fixture writes a place, the test sets the column to NULL,
//! and `browser_history_parser::firefox::parse_history` reads it back.

use browser_history_fixtures::{FirefoxPlaceRow, FirefoxPlacesFixture, FirefoxVisitRow};
use browser_history_parser::firefox;
use tempfile::TempDir;

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
