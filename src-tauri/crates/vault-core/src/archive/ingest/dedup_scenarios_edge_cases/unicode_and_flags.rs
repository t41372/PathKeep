//! Unicode and URL-flag ingest contract tests.
//!
//! ## Responsibilities
//! - Pin byte-identical Unicode URL/title round-trips.
//! - Pin Chromium hidden URL flag preservation.
//!
//! ## Not responsible for
//! - Search indexing or display-time Unicode folding.
//! - URL normalization policy covered by the Chromium contracts module.
//!
//! ## Dependencies
//! - Uses shared edge-case fixture helpers from the parent module.
//! - Uses synthetic Chromium History fixtures only.
//!
//! ## Performance notes
//! - Uses tiny fixtures with a few UTF-8 strings only.

use super::*;

// ======================================================================
// E8 — Unicode in URLs and titles (CJK + emoji + IDN)
// ======================================================================

/// E8 — International users routinely have Unicode in browsing history:
/// CJK characters in titles, internationalized domain names (IDN /
/// Punycode), percent-encoded paths, and emoji. SQLite stores all of
/// these as UTF-8 TEXT natively, but the contract must be pinned:
/// every character must round-trip byte-identically through the parser,
/// the fingerprint hash, and the archive storage. If a future refactor
/// accidentally normalizes Unicode (NFC vs NFD, case folding, IDN
/// decoding) or truncates non-ASCII, this test fails immediately.
#[test]
fn e8_unicode_urls_and_titles_round_trip_byte_identical() {
    let env = ScenarioEnv::new();
    let day_one_ms = 1_777_680_000_000_i64;
    let day_two_ms = 1_777_809_600_000_i64;
    let day_three_ms = 1_777_872_930_000_i64;

    // Three diverse Unicode shapes that must NOT be normalized:
    // 1. CJK title (Traditional Chinese) on plain ASCII URL
    // 2. Percent-encoded path with mixed case (verbatim per E6)
    // 3. Emoji in title
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/article".to_string(),
            title: Some("臺灣公開資料平臺 — 開放資料的全球趨勢".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/path/%E6%B8%AC%E8%A9%A6".to_string(),
            title: Some("Percent-Encoded Path".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_two_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 3,
            url: "https://example.com/celebration".to_string(),
            title: Some("Launch Day 🚀 — Ship It!".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_three_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(10, 1, day_one_ms))
        .add_visit(chromium_visit_row(20, 2, day_two_ms))
        .add_visit(chromium_visit_row(30, 3, day_three_ms));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:Unicode", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 3);
    assert_eq!(summary.new_visits, 3);

    let archive = env.open_archive();
    let read_url_and_title = |source_url_id: i64| -> (String, Option<String>) {
        archive
            .query_row(
                "SELECT url, title FROM urls
                 JOIN source_profiles ON source_profiles.id = urls.source_profile_id
                 WHERE source_profiles.profile_key = 'chrome:Unicode'
                   AND urls.source_url_id = ?1",
                [source_url_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query unicode row")
    };

    let (url1, title1) = read_url_and_title(1);
    assert_eq!(url1, "https://example.com/article");
    assert_eq!(
        title1.as_deref(),
        Some("臺灣公開資料平臺 — 開放資料的全球趨勢"),
        "CJK title must round-trip byte-identical (no NFC/NFD normalization)"
    );

    let (url2, title2) = read_url_and_title(2);
    assert_eq!(
        url2, "https://example.com/path/%E6%B8%AC%E8%A9%A6",
        "percent-encoded path must NOT be decoded — stored verbatim"
    );
    assert_eq!(title2.as_deref(), Some("Percent-Encoded Path"));

    let (url3, title3) = read_url_and_title(3);
    assert_eq!(url3, "https://example.com/celebration");
    assert_eq!(
        title3.as_deref(),
        Some("Launch Day 🚀 — Ship It!"),
        "emoji + em-dash must round-trip verbatim"
    );
}

// ======================================================================
// E9 — `hidden = true` URL flag round-trip
// ======================================================================

/// E9 — Real Chrome `History` databases routinely store URLs with
/// `hidden = 1` (Chrome marks redirect intermediates, certain extension
/// URLs, and explicitly-hidden items this way). The PathKeep parser
/// must preserve this flag verbatim: `hidden = true` on the source URL
/// must produce `hidden != 0` on the canonical archive URL, and
/// `hidden = false` must produce `hidden = 0`.
///
/// This pins the `hidden` bit contract — sibling to E7 (NULL title)
/// and E8 (Unicode round-trip). Existing C-series tests only exercise
/// `hidden: false`; the C4 B1-fix test exercises `hidden: true` but
/// only in the context of preventing older-snapshot regressions. No
/// test had asserted that a first-time import of a `hidden = true` URL
/// actually preserves the flag.
#[test]
fn e9_hidden_url_flag_round_trips_for_both_true_and_false() {
    let env = ScenarioEnv::new();
    let day_one_ms = 1_777_680_000_000_i64;
    let day_two_ms = 1_777_809_600_000_i64;

    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/visible".to_string(),
            title: Some("Visible Page".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/hidden-redirect-intermediate".to_string(),
            title: Some("Hidden Redirect".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_two_ms,
            hidden: true,
        })
        .add_visit(chromium_visit_row(1, 1, day_one_ms))
        .add_visit(chromium_visit_row(2, 2, day_two_ms));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:HiddenFlag", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 2);
    assert_eq!(summary.new_visits, 2);

    let archive = env.open_archive();
    let read_hidden = |source_url_id: i64| -> i64 {
        archive
            .query_row(
                "SELECT hidden FROM urls
                 JOIN source_profiles ON source_profiles.id = urls.source_profile_id
                 WHERE source_profiles.profile_key = 'chrome:HiddenFlag'
                   AND urls.source_url_id = ?1",
                [source_url_id],
                |row| row.get(0),
            )
            .expect("query hidden flag")
    };

    assert_eq!(read_hidden(1), 0, "hidden=false source must land as 0 in archive");
    assert!(
        read_hidden(2) != 0,
        "hidden=true source must land as non-zero in archive (not silently dropped)"
    );
}
