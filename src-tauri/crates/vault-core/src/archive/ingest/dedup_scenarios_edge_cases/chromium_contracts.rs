//! Chromium edge-case ingest contract tests.
//!
//! ## Responsibilities
//! - Pin verbatim storage of the observed URL string in `urls.url`.
//!
//! ## Not responsible for
//! - Browser-family baseline scenarios covered by sibling modules.
//!
//! ## Dependencies
//! - Uses shared edge-case fixture helpers from the parent module.
//! - Uses synthetic Chromium History fixtures only.
//!
//! ## Performance notes
//! - Keeps fixtures tiny; scale-collision tests live in a blocked benchmark block.

use super::*;

// ======================================================================
// E6 — `urls.url` keeps the observed string
// ======================================================================

/// E6 — `urls.url` stores the URL string exactly as the browser recorded it.
/// Normalization belongs to `pages.canonical_url` (data-model.md), not to
/// the stored URL row. Different URL strings with different source_url_ids
/// must be preserved as separate URL rows even when they point to
/// semantically "the same" resource, so no spelling is rewritten or lost.
#[test]
fn e6_url_strings_stored_verbatim_no_normalization() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

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
                    |row| Ok(row.get(0)?),
                )
                .unwrap_or_else(|_| panic!("query URL for source_url_id={source_url_id}"));
            assert_eq!(
                stored_url, expected_url,
                "URL with source_url_id={source_url_id} must be stored verbatim"
            );
        }
    }
}
