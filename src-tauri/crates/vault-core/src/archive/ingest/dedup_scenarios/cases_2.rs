//! Responsibilities: verify dedup scenarios in both Store key modes.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: each named case preserves the original scenario and assertions.
use super::*;

/// C7 — Tie-break contract for the B1 fix in `writes.rs::upsert_url`.
/// When two snapshots report the same `last_visit_ms` for a URL, the
/// upsert must NOT overwrite `title`, `hidden`, `payload_hash`, or
/// `recorded_at` — only strictly newer timestamps win. This prevents
/// two real-world data losses:
///
/// 1. A re-import where Chrome's title hadn't been hydrated yet
///    (ParsedUrl.title = None) shouldn't silently destroy a captured
///    title at the same `last_visit_ms`.
/// 2. Firefox bookmark-only URLs (last_visit_date IS NULL → 0) tie at
///    `last_visit_ms = 0` on every re-import; the original B1 fix's
///    `>=` comparison meant title/hidden flipped to the second snapshot
///    every sync.
#[test]
fn c7_tied_last_visit_ms_does_not_overwrite_title_hidden_or_payload_hash() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        let visit_time_ms = 1_777_809_600_000_i64;

        // Snapshot 1: URL with real title, hidden=false, captured at T.
        let first_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/tied-time".to_string(),
                title: Some("Captured Title".to_string()),
                visit_count: 3,
                typed_count: 1,
                last_visit_unix_ms: visit_time_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, visit_time_ms));
        let first_snapshot =
            snapshot_for_fixture(&first_fixture, chromium_profile("chrome:Tied", "Google Chrome"));
        run_one_ingest(&env, 1, &first_snapshot, false);
        drop(first_snapshot);

        let initial_payload_hash: String = {
            let archive = env.open_archive();
            archive
                .query_row(
                    "SELECT payload_hash FROM urls
                 JOIN source_profiles ON source_profiles.id = urls.source_profile_id
                 WHERE source_profiles.profile_key = 'chrome:Tied'
                   AND urls.source_url_id = 1",
                    (),
                    |row| Ok(row.get(0)?),
                )
                .expect("query initial payload_hash")
        };

        // Snapshot 2: same last_visit_ms (tie), but everything else is
        // worse — title is NULL, hidden flipped to true, lower counts.
        // The B1 fix must preserve snapshot 1's values across this tie.
        let second_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/tied-time".to_string(),
                title: None,
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: visit_time_ms,
                hidden: true,
            })
            .add_visit(visit_row(11, 1, visit_time_ms));
        let second_snapshot =
            snapshot_for_fixture(&second_fixture, chromium_profile("chrome:Tied", "Google Chrome"));
        run_one_ingest(&env, 2, &second_snapshot, false);

        let archive = env.open_archive();
        let (title, hidden, payload_hash, visit_count, typed_count): (
            Option<String>,
            i64,
            String,
            i64,
            i64,
        ) = archive
            .query_row(
                "SELECT title, hidden, payload_hash, visit_count, typed_count FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Tied'
               AND urls.source_url_id = 1",
                (),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("query url state after tied re-import");

        assert_eq!(
            title.as_deref(),
            Some("Captured Title"),
            "tied last_visit_ms must NOT overwrite title with NULL from later snapshot",
        );
        assert_eq!(
            hidden, 0,
            "tied last_visit_ms must NOT flip hidden to true from later snapshot"
        );
        assert_eq!(
            payload_hash, initial_payload_hash,
            "tied last_visit_ms must preserve original payload_hash (audit-trail integrity)",
        );
        assert_eq!(
            visit_count, 3,
            "visit_count must use MAX semantics, preserving the higher value"
        );
        assert_eq!(
            typed_count, 1,
            "typed_count must use MAX semantics, preserving the higher value"
        );
    }
}

/// C4 — Regression test for audit bug **B1** (now fixed). The URL
/// upsert in `writes.rs` now uses `MAX()` for `visit_count` / `typed_count`
/// and `CASE WHEN excluded.last_visit_ms >= urls.last_visit_ms` for `title`
/// / `hidden`, preventing older snapshots from overwriting newer metadata.
/// This test asserts all four fields survive a re-import of an older
/// snapshot without regression.
#[test]
fn c4_chromium_reimport_older_snapshot_regresses_visit_count_demonstrates_b1() {
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

        // Snapshot 2: same URL but visit_count=5 (the older snapshot regression).
        // last_visit_ms is identical, so the existing guard does not fire and
        // the unconditional overwrite path runs.
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
            "B1 fix required: urls.visit_count must not regress on re-import (got {final_count}, was 10)"
        );

        // B1 fix: typed_count uses MAX semantics — must keep the higher value.
        let final_typed = stored_typed_count(&env, "chrome:Default", 1);
        assert!(
            final_typed >= 4,
            "B1 fix: typed_count must use MAX semantics (got {final_typed}, was 4)"
        );

        // B1 fix: title and hidden use CASE WHEN excluded.last_visit_ms >=
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
            "B1 fix: title from strictly older snapshot must not overwrite newer"
        );

        let final_hidden = stored_hidden(&env, "chrome:Default", 1);
        assert!(!final_hidden, "B1 fix: hidden must not regress to older snapshot's value");
    }
}
