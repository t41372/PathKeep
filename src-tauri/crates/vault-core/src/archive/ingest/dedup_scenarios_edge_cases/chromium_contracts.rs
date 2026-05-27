//! Chromium edge-case ingest contract tests.
//!
//! ## Responsibilities
//! - Pin same-millisecond visit fingerprint behavior.
//! - Pin verbatim URL storage with no normalization.
//!
//! ## Not responsible for
//! - Browser-family baseline scenarios covered by sibling modules.
//! - Repairing known precision limitations.
//!
//! ## Dependencies
//! - Uses shared edge-case fixture helpers from the parent module.
//! - Uses synthetic Chromium History fixtures only.
//!
//! ## Performance notes
//! - Keeps fixtures tiny; scale-collision tests live in a blocked benchmark block.

use super::*;

// ======================================================================
// C_SUB_MS (E5) — Sub-millisecond Chrome visit collision contract
// ======================================================================

/// C_SUB_MS (E5) — Sub-millisecond Chrome visit collision contract.
///
/// Chrome stores visit times at microsecond precision; our parser truncates
/// to milliseconds. Two visits to the same URL within the same millisecond
/// produce identical `event_fingerprint` values. The partial unique index
/// deduplicates the second visit even though source_visit_ids differ.
///
/// This is a known acceptable limitation, not a bug. This test pins the
/// behavior so that any future precision change is caught.
#[test]
fn c_sub_ms_same_millisecond_visits_collapsed_by_fingerprint() {
    let env = ScenarioEnv::new();

    // Two visits to the same URL with different source_visit_ids but
    // identical visit_time_unix_ms. The fingerprint computation uses
    // unix_micros_to_chrome_time(visit_time_ms * 1000), so both visits
    // produce the same Chrome time → same fingerprint → INSERT OR IGNORE
    // silently skips the second.
    let same_ms = 1_777_680_000_000_i64; // 2026-05-01T00:00:00Z

    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/sub-ms-collision".to_string(),
            title: Some("Sub-ms Collision".to_string()),
            visit_count: 2,
            typed_count: 0,
            last_visit_unix_ms: same_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(20, 1, same_ms))
        .add_visit(chromium_visit_row(21, 1, same_ms));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:Default", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);

    // The parser delivers both visits, but only one survives archive insert:
    // - Visit 20 inserted successfully (new source_visit_id, new fingerprint).
    // - Visit 21 has a DIFFERENT source_visit_id (so UNIQUE(source_profile_id,
    //   source_visit_id) does not fire) but the SAME event_fingerprint (same
    //   url, same Chrome time, same title, same transition, same app_id).
    //   The partial unique index on (source_profile_id, event_fingerprint)
    //   triggers → INSERT OR IGNORE silently skips.
    assert_eq!(
        summary.new_visits, 1,
        "only one of two same-millisecond visits should survive fingerprint dedup"
    );
    assert_eq!(summary.new_urls, 1);
    assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 1);
    assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 1);
}

// ======================================================================
// E6 — URL canonicalization contract: no normalization applied
// ======================================================================

/// E6 — URL canonicalization contract pins.
///
/// PathKeep stores URL strings as-is with NO normalization. Different URL
/// strings with different source_url_ids must be preserved as separate URL
/// rows even when they point to semantically "the same" resource. This
/// pins the contract so a future normalization change is caught.
#[test]
fn e6_url_strings_stored_verbatim_no_normalization() {
    let env = ScenarioEnv::new();

    let t1 = 1_777_680_000_000_i64;
    let t2 = 1_777_809_600_000_i64;
    let t3 = 1_777_872_930_000_i64;
    let t4 = 1_777_939_200_000_i64;

    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/path".to_string(),
            title: Some("Base URL".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: t1,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/path/".to_string(),
            title: Some("Trailing Slash".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: t2,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 3,
            url: "https://example.com/page#section".to_string(),
            title: Some("Fragment Preserved".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: t3,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 4,
            url: "https://Example.COM/Path".to_string(),
            title: Some("Mixed Case".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: t4,
            hidden: false,
        })
        .add_visit(chromium_visit_row(10, 1, t1))
        .add_visit(chromium_visit_row(11, 2, t2))
        .add_visit(chromium_visit_row(12, 3, t3))
        .add_visit(chromium_visit_row(13, 4, t4));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:Default", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);

    // All four URLs must be preserved as distinct rows.
    assert_eq!(summary.new_urls, 4, "all URL variants must be separate rows");
    assert_eq!(summary.new_visits, 4);
    assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 4);
    assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 4);

    // Query back every URL string and assert verbatim storage.
    let archive = env.open_archive();
    let expected_urls = [
        (1_i64, "https://example.com/path"),
        (2, "https://example.com/path/"),
        (3, "https://example.com/page#section"),
        (4, "https://Example.COM/Path"),
    ];
    for (source_url_id, expected_url) in expected_urls {
        let stored_url: String = archive
            .query_row(
                "SELECT url FROM urls
                 JOIN source_profiles ON source_profiles.id = urls.source_profile_id
                 WHERE source_profiles.profile_key = 'chrome:Default'
                   AND urls.source_url_id = ?1",
                [source_url_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| panic!("query URL for source_url_id={source_url_id}"));
        assert_eq!(
            stored_url, expected_url,
            "URL with source_url_id={source_url_id} must be stored verbatim"
        );
    }
}
