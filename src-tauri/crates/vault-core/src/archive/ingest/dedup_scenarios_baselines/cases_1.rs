//! Responsibilities: verify dedup scenarios in both Store key modes.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: each named case preserves the original scenario and assertions.
use super::*;

/// Chromium fingerprint dedup — Imports a Chromium fixture, then
/// re-imports the exact same visits but with DIFFERENT `source_visit_id`
/// values (simulating a database rebuild or ID reassignment). The
/// `(source_profile_id, event_fingerprint)` partial unique index must
/// catch these as duplicates. No duplicate visit rows should be created.
#[test]
fn chromium_fingerprint_dedup_catches_same_visits_with_different_source_ids() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

        let t1 = 1_777_680_000_000_i64;
        let t2 = 1_777_809_600_000_i64;
        let t3 = 1_777_872_930_000_i64;

        // First import: visit IDs 10, 11, 12.
        let first_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/fingerprint-test-one".to_string(),
                title: Some("Fingerprint Test One".to_string()),
                visit_count: 2,
                typed_count: 1,
                last_visit_unix_ms: t2,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 2,
                url: "https://example.org/fingerprint-test-two".to_string(),
                title: Some("Fingerprint Test Two".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: t3,
                hidden: false,
            })
            .add_visit(chromium_visit_row(10, 1, t1))
            .add_visit(chromium_visit_row(11, 1, t2))
            .add_visit(chromium_visit_row(12, 2, t3));

        let first_snapshot = snapshot_for_chromium_fixture(
            &first_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let first_summary = run_one_ingest(&env, 1, &first_snapshot, false);
        assert_eq!(first_summary.new_urls, 2);
        assert_eq!(first_summary.new_visits, 3);
        drop(first_snapshot);

        // Second import: SAME URLs and visit times, but source_visit_ids are
        // different (100, 101, 102 instead of 10, 11, 12). This simulates a
        // Chrome database rebuild where rowids get reassigned but the actual
        // browsing events are identical.
        let second_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/fingerprint-test-one".to_string(),
                title: Some("Fingerprint Test One".to_string()),
                visit_count: 2,
                typed_count: 1,
                last_visit_unix_ms: t2,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 2,
                url: "https://example.org/fingerprint-test-two".to_string(),
                title: Some("Fingerprint Test Two".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: t3,
                hidden: false,
            })
            .add_visit(chromium_visit_row(100, 1, t1))
            .add_visit(chromium_visit_row(101, 1, t2))
            .add_visit(chromium_visit_row(102, 2, t3));

        let second_snapshot = snapshot_for_chromium_fixture(
            &second_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let second_summary = run_one_ingest(&env, 2, &second_snapshot, false);

        // The fingerprint partial index should catch all 3 visits as duplicates.
        assert_eq!(
            second_summary.new_visits, 0,
            "fingerprint dedup must catch same visits with different source_visit_ids"
        );

        // Archive row counts must stay at the first import's values.
        assert_eq!(count_archive_rows(&env, "urls"), 2);
        assert_eq!(
            count_visits_for_profile(&env, "chrome:Default"),
            3,
            "no duplicate visits should be created despite different source_visit_ids"
        );
    }
}

/// F2 — Firefox equivalent of C3.
/// The Chromium parser's `INGEST_URLS_SQL` has an
/// `OR id IN (SELECT DISTINCT url FROM visits WHERE id > ?2)` fallback
/// to catch URLs whose `last_visit_time` is below the URL watermark but
/// which received a new visit anyway. The Firefox URL stream has the
/// same OR fallback. Without it the new visit's `url_id_map.get` fails
/// and `ArchiveChunkConsumer::visits` silently drops the row.
#[test]
fn f2_firefox_incremental_revisit_of_old_url_keeps_the_new_visit() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        // Long-tail URL (T1) + anchor URL (T2) so the URL watermark
        // advances past T1 after the first import; the second-pass URL
        // query then excludes the long-tail URL.
        let visit_long_tail_ms = 1_777_680_000_000_i64;
        let visit_anchor_ms = 1_777_809_600_000_i64;
        let visit_revisit_ms = 1_777_872_930_000_i64;

        let first_fixture = FirefoxPlacesFixture::new()
            .add_place(FirefoxPlaceRow {
                id: 1,
                url: "https://example.com/firefox-long-tail".to_string(),
                title: Some("Firefox Long Tail".to_string()),
                visit_count: 1,
                hidden: false,
                last_visit_unix_ms: visit_long_tail_ms,
            })
            .add_place(FirefoxPlaceRow {
                id: 2,
                url: "https://example.com/firefox-anchor".to_string(),
                title: Some("Firefox Anchor".to_string()),
                visit_count: 1,
                hidden: false,
                last_visit_unix_ms: visit_anchor_ms,
            })
            .add_visit(FirefoxVisitRow {
                id: 10,
                place_id: 1,
                visit_time_unix_ms: visit_long_tail_ms,
                from_visit: None,
                visit_type: Some(1),
            })
            .add_visit(FirefoxVisitRow {
                id: 20,
                place_id: 2,
                visit_time_unix_ms: visit_anchor_ms,
                from_visit: None,
                visit_type: Some(1),
            });
        let first_snapshot = firefox_snapshot(&first_fixture, "firefox:Default");
        run_one_ingest(&env, 1, &first_snapshot, false);
        drop(first_snapshot);

        // Pass 2: URL 1's last_visit_date stays at T1 (below the watermark);
        // its new visit (id=30, time > T2) only appears in moz_historyvisits.
        // Without the OR fallback the URL is filtered out and the visit's
        // url_id_map lookup fails.
        let second_fixture = FirefoxPlacesFixture::new()
            .add_place(FirefoxPlaceRow {
                id: 1,
                url: "https://example.com/firefox-long-tail".to_string(),
                title: Some("Firefox Long Tail".to_string()),
                visit_count: 2,
                hidden: false,
                last_visit_unix_ms: visit_long_tail_ms,
            })
            .add_place(FirefoxPlaceRow {
                id: 2,
                url: "https://example.com/firefox-anchor".to_string(),
                title: Some("Firefox Anchor".to_string()),
                visit_count: 1,
                hidden: false,
                last_visit_unix_ms: visit_anchor_ms,
            })
            .add_visit(FirefoxVisitRow {
                id: 10,
                place_id: 1,
                visit_time_unix_ms: visit_long_tail_ms,
                from_visit: None,
                visit_type: Some(1),
            })
            .add_visit(FirefoxVisitRow {
                id: 20,
                place_id: 2,
                visit_time_unix_ms: visit_anchor_ms,
                from_visit: None,
                visit_type: Some(1),
            })
            .add_visit(FirefoxVisitRow {
                id: 30,
                place_id: 1,
                visit_time_unix_ms: visit_revisit_ms,
                from_visit: Some(20),
                visit_type: Some(1),
            });
        let second_snapshot = firefox_snapshot(&second_fixture, "firefox:Default");
        run_one_ingest(&env, 2, &second_snapshot, true);

        let visits = count_visits_for_profile(&env, "firefox:Default");
        assert_eq!(visits, 3, "Firefox long-tail revisit silently dropped (got {visits})");
    }
}

/// S2 — Safari needs no OR fallback for the same long-tail revisit: the
/// Safari URL query computes `MAX(history_visits.visit_time)` on the fly
/// from the visits table (Safari's `history_items` table has no cached
/// `last_visit_time` column), so any new visit row immediately raises
/// the item's effective last-visit time and the URL gets re-streamed.
/// If a stored `last_visit_time` cache on `history_items` is ever added
/// without the OR fallback, the new visit is dropped and this test fails.
#[test]
fn s2_safari_long_tail_revisit_captured_without_or_fallback() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        // Long-tail item (T1) + anchor item (T2). The anchor pushes the URL
        // watermark past T1; the second-pass Safari URL query (which
        // computes per-item MAX(visit_time) on the fly) excludes the
        // long-tail item; the new visit references it and gets dropped.
        let visit_long_tail_ms = 1_777_680_000_000_i64;
        let visit_anchor_ms = 1_777_809_600_000_i64;
        let visit_revisit_ms = 1_777_872_930_000_i64;

        let first_fixture = SafariHistoryFixture::new()
            .add_item(SafariHistoryItemRow {
                id: 1,
                url: "https://example.com/safari-long-tail".to_string(),
            })
            .add_item(SafariHistoryItemRow {
                id: 2,
                url: "https://example.com/safari-anchor".to_string(),
            })
            .add_visit(safari_visit(9, 1, "Safari Long Tail", visit_long_tail_ms))
            .add_visit(safari_visit(19, 2, "Safari Anchor", visit_anchor_ms));
        let first_snapshot = safari_snapshot(&first_fixture, "safari:Default");
        run_one_ingest(&env, 1, &first_snapshot, false);
        drop(first_snapshot);

        let second_fixture = SafariHistoryFixture::new()
            .add_item(SafariHistoryItemRow {
                id: 1,
                url: "https://example.com/safari-long-tail".to_string(),
            })
            .add_item(SafariHistoryItemRow {
                id: 2,
                url: "https://example.com/safari-anchor".to_string(),
            })
            .add_visit(safari_visit(9, 1, "Safari Long Tail", visit_long_tail_ms))
            .add_visit(safari_visit(19, 2, "Safari Anchor", visit_anchor_ms))
            .add_visit(safari_visit(29, 1, "Safari Long Tail Revisited", visit_revisit_ms));
        let second_snapshot = safari_snapshot(&second_fixture, "safari:Default");
        run_one_ingest(&env, 2, &second_snapshot, true);

        let visits = count_visits_for_profile(&env, "safari:Default");
        assert_eq!(
            visits, 3,
            "Safari MAX(visit_time)-computed URL query already handles long-tail revisits without an OR fallback"
        );
    }
}
