//! Responsibilities: verify dedup scenarios in both Store key modes.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: each named case preserves the original scenario and assertions.
use super::*;

/// C4 — Reading an older snapshot must not regress URL metadata. The URL
/// upsert in `writes.rs` uses `MAX()` for `visit_count` / `typed_count`
/// and `CASE WHEN excluded.last_visit_ms >= urls.last_visit_ms` for `title`
/// / `hidden`, so an older snapshot cannot overwrite newer metadata.
/// This test asserts all four fields survive a re-import of an older
/// snapshot.
#[test]
fn c4_chromium_reimport_of_an_older_snapshot_keeps_newer_counts_title_and_hidden() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        let visit_two_ms = 1_777_809_600_000_i64;

        // Snapshot 1: URL with lifetime visit_count=10.
        let first_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/long-tracked".to_string(),
                title: Some("Long Tracked Page".to_string()),
                visit_count: 10,
                typed_count: 4,
                last_visit_unix_ms: visit_two_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, visit_two_ms));
        let first_snapshot = snapshot_for_fixture(
            &first_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        run_one_ingest(&env, 1, &first_snapshot, false);
        drop(first_snapshot);
        assert_eq!(stored_visit_count(&env, "chrome:Default", 1), 10);

        // Snapshot 2: same URL but visit_count=5, as an older snapshot reports it.
        // last_visit_ms is identical, so the title / hidden guard does not fire
        // and only the MAX() on the counts protects them.
        let second_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/long-tracked".to_string(),
                title: Some("Regressed Title".to_string()),
                visit_count: 5,
                typed_count: 1,
                last_visit_unix_ms: visit_two_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, visit_two_ms));
        let second_snapshot = snapshot_for_fixture(
            &second_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        run_one_ingest(&env, 2, &second_snapshot, false);

        let final_count = stored_visit_count(&env, "chrome:Default", 1);
        assert!(
            final_count >= 10,
            "urls.visit_count must not regress on re-import (got {final_count}, was 10)"
        );

        // typed_count uses MAX semantics and must keep the higher value.
        let final_typed = stored_typed_count(&env, "chrome:Default", 1);
        assert!(final_typed >= 4, "typed_count must use MAX semantics (got {final_typed}, was 4)");

        // Title and hidden use CASE WHEN excluded.last_visit_ms >=
        // urls.last_visit_ms — at equal timestamps the second import "wins",
        // which is acceptable. The important contract: a strictly OLDER
        // snapshot cannot overwrite. Re-import with an older last_visit_ms
        // to verify.
        drop(second_snapshot);
        let visit_one_ms = 1_777_680_000_000_i64; // strictly older
        let third_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/long-tracked".to_string(),
                title: Some("Ancient Title".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: visit_one_ms,
                hidden: true,
            })
            .add_visit(visit_row(10, 1, visit_one_ms));
        let third_snapshot = snapshot_for_fixture(
            &third_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        run_one_ingest(&env, 3, &third_snapshot, false);

        let final_title = stored_title(&env, "chrome:Default", 1);
        assert_ne!(
            final_title.as_deref(),
            Some("Ancient Title"),
            "title from strictly older snapshot must not overwrite newer"
        );

        let final_hidden = stored_hidden(&env, "chrome:Default", 1);
        assert!(!final_hidden, "hidden must not regress to older snapshot's value");
    }
}
