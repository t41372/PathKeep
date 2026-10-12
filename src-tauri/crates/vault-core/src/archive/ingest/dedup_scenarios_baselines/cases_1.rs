//! Responsibilities: verify dedup scenarios in both Store key modes.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: each named case preserves the original scenario and assertions.
use super::*;

/// F1 — One Firefox profile, one ingest pass. Asserts every fixture row
/// lands in the canonical archive with correct URL count, visit count,
/// timestamps, and field values matching fixture input. This is the
/// Firefox analog of C1 (Chromium baseline).
#[test]
fn f1_firefox_baseline_import() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

        // 2026-05-01 00:00, 2026-05-02 12:00, 2026-05-03 08:15:30,
        // 2026-05-04 10:00, 2026-05-05 14:30
        let t1 = 1_777_680_000_000_i64;
        let t2 = 1_777_809_600_000_i64;
        let t3 = 1_777_872_930_000_i64;
        let t4 = 1_777_939_200_000_i64;
        let t5 = 1_778_041_800_000_i64;

        let fixture = FirefoxPlacesFixture::new()
            .add_place(FirefoxPlaceRow {
                id: 1,
                url: "https://example.com/firefox-article-one".to_string(),
                title: Some("Firefox Article One".to_string()),
                visit_count: 2,
                hidden: false,
                last_visit_unix_ms: t2,
            })
            .add_place(FirefoxPlaceRow {
                id: 2,
                url: "https://example.org/firefox-article-two".to_string(),
                title: Some("Firefox Article Two".to_string()),
                visit_count: 2,
                hidden: false,
                last_visit_unix_ms: t4,
            })
            .add_place(FirefoxPlaceRow {
                id: 3,
                url: "https://example.net/firefox-article-three".to_string(),
                title: Some("Firefox Article Three".to_string()),
                visit_count: 1,
                hidden: false,
                last_visit_unix_ms: t5,
            })
            // 5 visits across 3 URLs
            .add_visit(FirefoxVisitRow {
                id: 10,
                place_id: 1,
                visit_time_unix_ms: t1,
                from_visit: None,
                visit_type: Some(1),
            })
            .add_visit(FirefoxVisitRow {
                id: 11,
                place_id: 1,
                visit_time_unix_ms: t2,
                from_visit: Some(10),
                visit_type: Some(2),
            })
            .add_visit(FirefoxVisitRow {
                id: 12,
                place_id: 2,
                visit_time_unix_ms: t3,
                from_visit: None,
                visit_type: Some(1),
            })
            .add_visit(FirefoxVisitRow {
                id: 13,
                place_id: 2,
                visit_time_unix_ms: t4,
                from_visit: Some(12),
                visit_type: Some(1),
            })
            .add_visit(FirefoxVisitRow {
                id: 14,
                place_id: 3,
                visit_time_unix_ms: t5,
                from_visit: None,
                visit_type: Some(5),
            });

        let snapshot = firefox_snapshot(&fixture, "firefox:Default");
        let summary = run_one_ingest(&env, 1, &snapshot, false);

        // Summary must report exactly what the fixture contained.
        assert_eq!(summary.new_urls, 3, "summary reports 3 new urls");
        assert_eq!(summary.new_visits, 5, "summary reports 5 new visits");

        // Archive row counts match fixture.
        assert_eq!(count_archive_rows(&env, "urls"), 3);
        assert_eq!(count_archive_rows(&env, "visits"), 5);
        assert_eq!(count_urls_for_profile(&env, "firefox:Default"), 3);
        assert_eq!(count_visits_for_profile(&env, "firefox:Default"), 5);

        // Source visit IDs flow through unmodified.
        let visit_ids = collect_visit_source_ids(&env, "firefox:Default");
        assert_eq!(visit_ids, vec!["10", "11", "12", "13", "14"]);

        // Spot-check visit timestamps round-tripped correctly.
        let archive = env.open_archive();
        let first_visit_ms: i64 = archive
            .query_row(
                "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'firefox:Default'
               AND visits.source_visit_id = '10'",
                (),
                |row| Ok(row.get(0)?),
            )
            .expect("query first visit time");
        assert_eq!(first_visit_ms, t1, "first visit timestamp must match fixture");

        let last_visit_ms: i64 = archive
            .query_row(
                "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'firefox:Default'
               AND visits.source_visit_id = '14'",
                (),
                |row| Ok(row.get(0)?),
            )
            .expect("query last visit time");
        assert_eq!(last_visit_ms, t5, "last visit timestamp must match fixture");

        // URL title landed correctly.
        let title: Option<String> = archive
            .query_row(
                "SELECT title FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'firefox:Default'
               AND urls.source_url_id = 1",
                (),
                |row| Ok(row.get(0)?),
            )
            .expect("query url title");
        assert_eq!(title.as_deref(), Some("Firefox Article One"));
    }
}

/// S1 — One Safari profile, one ingest pass. Asserts every fixture row
/// lands in the canonical archive with correct URL count, visit count,
/// timestamps, and field values matching fixture input. This is the
/// Safari analog of C1 (Chromium baseline).
#[test]
fn s1_safari_baseline_import() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

        // 2026-05-01 00:00, 2026-05-02 12:00, 2026-05-03 08:15:30,
        // 2026-05-04 10:00, 2026-05-05 14:30
        let t1 = 1_777_680_000_000_i64;
        let t2 = 1_777_809_600_000_i64;
        let t3 = 1_777_872_930_000_i64;
        let t4 = 1_777_939_200_000_i64;
        let t5 = 1_778_041_800_000_i64;

        let fixture = SafariHistoryFixture::new()
            .add_item(SafariHistoryItemRow {
                id: 1,
                url: "https://example.com/safari-article-one".to_string(),
            })
            .add_item(SafariHistoryItemRow {
                id: 2,
                url: "https://example.org/safari-article-two".to_string(),
            })
            .add_item(SafariHistoryItemRow {
                id: 3,
                url: "https://example.net/safari-article-three".to_string(),
            })
            // 5 visits across 3 items
            .add_visit(safari_visit(10, 1, "Safari Article One", t1))
            .add_visit(safari_visit(11, 1, "Safari Article One", t2))
            .add_visit(safari_visit(12, 2, "Safari Article Two", t3))
            .add_visit(safari_visit(13, 2, "Safari Article Two", t4))
            .add_visit(safari_visit(14, 3, "Safari Article Three", t5));

        let snapshot = safari_snapshot(&fixture, "safari:Default");
        let summary = run_one_ingest(&env, 1, &snapshot, false);

        // Summary must report exactly what the fixture contained.
        assert_eq!(summary.new_urls, 3, "summary reports 3 new urls");
        assert_eq!(summary.new_visits, 5, "summary reports 5 new visits");

        // Archive row counts match fixture.
        assert_eq!(count_archive_rows(&env, "urls"), 3);
        assert_eq!(count_archive_rows(&env, "visits"), 5);
        assert_eq!(count_urls_for_profile(&env, "safari:Default"), 3);
        assert_eq!(count_visits_for_profile(&env, "safari:Default"), 5);

        // Source visit IDs flow through unmodified.
        let visit_ids = collect_visit_source_ids(&env, "safari:Default");
        assert_eq!(visit_ids, vec!["10", "11", "12", "13", "14"]);

        // Spot-check visit timestamps round-tripped correctly.
        let archive = env.open_archive();
        let first_visit_ms: i64 = archive
            .query_row(
                "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'safari:Default'
               AND visits.source_visit_id = '10'",
                (),
                |row| Ok(row.get(0)?),
            )
            .expect("query first visit time");
        assert_eq!(first_visit_ms, t1, "first visit timestamp must match fixture");

        let last_visit_ms: i64 = archive
            .query_row(
                "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'safari:Default'
               AND visits.source_visit_id = '14'",
                (),
                |row| Ok(row.get(0)?),
            )
            .expect("query last visit time");
        assert_eq!(last_visit_ms, t5, "last visit timestamp must match fixture");

        // URL title landed correctly (Safari carries title on visits, not items;
        // the parser should populate url.title from the most recent visit title).
        let title: Option<String> = archive
            .query_row(
                "SELECT title FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'safari:Default'
               AND urls.source_url_id = 1",
                (),
                |row| Ok(row.get(0)?),
            )
            .expect("query url title");
        assert!(title.is_some(), "Safari URL title should be populated from visit title");
    }
}

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

/// F_C2 — Re-importing the same Firefox fixture with `use_watermark = true`
/// must produce zero new rows. The watermark advance after the first import
/// should make the second import a no-op at the parser level. This is the
/// Firefox analog of C2 (Chromium incremental no-new-data).
#[test]
fn f_c2_firefox_incremental_no_new_data() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        let (t1, t2, t3, t4, t5) = (
            1_777_680_000_000_i64,
            1_777_809_600_000_i64,
            1_777_872_930_000_i64,
            1_777_939_200_000_i64,
            1_778_041_800_000_i64,
        );

        let build_fixture = || {
            FirefoxPlacesFixture::new()
                .add_place(FirefoxPlaceRow {
                    id: 1,
                    url: "https://example.com/firefox-article-one".to_string(),
                    title: Some("Firefox Article One".to_string()),
                    visit_count: 2,
                    hidden: false,
                    last_visit_unix_ms: t2,
                })
                .add_place(FirefoxPlaceRow {
                    id: 2,
                    url: "https://example.org/firefox-article-two".to_string(),
                    title: Some("Firefox Article Two".to_string()),
                    visit_count: 2,
                    hidden: false,
                    last_visit_unix_ms: t4,
                })
                .add_place(FirefoxPlaceRow {
                    id: 3,
                    url: "https://example.net/firefox-article-three".to_string(),
                    title: Some("Firefox Article Three".to_string()),
                    visit_count: 1,
                    hidden: false,
                    last_visit_unix_ms: t5,
                })
                .add_visit(FirefoxVisitRow {
                    id: 10,
                    place_id: 1,
                    visit_time_unix_ms: t1,
                    from_visit: None,
                    visit_type: Some(1),
                })
                .add_visit(FirefoxVisitRow {
                    id: 11,
                    place_id: 1,
                    visit_time_unix_ms: t2,
                    from_visit: Some(10),
                    visit_type: Some(2),
                })
                .add_visit(FirefoxVisitRow {
                    id: 12,
                    place_id: 2,
                    visit_time_unix_ms: t3,
                    from_visit: None,
                    visit_type: Some(1),
                })
                .add_visit(FirefoxVisitRow {
                    id: 13,
                    place_id: 2,
                    visit_time_unix_ms: t4,
                    from_visit: Some(12),
                    visit_type: Some(1),
                })
                .add_visit(FirefoxVisitRow {
                    id: 14,
                    place_id: 3,
                    visit_time_unix_ms: t5,
                    from_visit: None,
                    visit_type: Some(5),
                })
        };

        // First import: baseline — no watermark.
        let first_snapshot = firefox_snapshot(&build_fixture(), "firefox:Default");
        run_one_ingest(&env, 1, &first_snapshot, false);
        drop(first_snapshot);

        // Second import: identical data — watermark should skip everything.
        let second_snapshot = firefox_snapshot(&build_fixture(), "firefox:Default");
        let summary = run_one_ingest(&env, 2, &second_snapshot, true);

        assert_eq!(summary.new_urls, 0, "second import must add no new URL rows");
        assert_eq!(summary.new_visits, 0, "second import must add no new visit rows");

        // Archive row counts must stay at the first import's values.
        assert_eq!(count_archive_rows(&env, "urls"), 3);
        assert_eq!(count_archive_rows(&env, "visits"), 5);
        assert_eq!(count_urls_for_profile(&env, "firefox:Default"), 3);
        assert_eq!(count_visits_for_profile(&env, "firefox:Default"), 5);
    }
}

/// S_C2 — Re-importing the same Safari fixture with `use_watermark = true`
/// must produce zero new rows. The watermark advance after the first import
/// should make the second import a no-op at the parser level. This is the
/// Safari analog of C2 (Chromium incremental no-new-data).
#[test]
fn s_c2_safari_incremental_no_new_data() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        let (t1, t2, t3, t4, t5) = (
            1_777_680_000_000_i64,
            1_777_809_600_000_i64,
            1_777_872_930_000_i64,
            1_777_939_200_000_i64,
            1_778_041_800_000_i64,
        );

        let build_fixture = || {
            SafariHistoryFixture::new()
                .add_item(SafariHistoryItemRow {
                    id: 1,
                    url: "https://example.com/safari-article-one".to_string(),
                })
                .add_item(SafariHistoryItemRow {
                    id: 2,
                    url: "https://example.org/safari-article-two".to_string(),
                })
                .add_item(SafariHistoryItemRow {
                    id: 3,
                    url: "https://example.net/safari-article-three".to_string(),
                })
                .add_visit(safari_visit(10, 1, "Safari Article One", t1))
                .add_visit(safari_visit(11, 1, "Safari Article One", t2))
                .add_visit(safari_visit(12, 2, "Safari Article Two", t3))
                .add_visit(safari_visit(13, 2, "Safari Article Two", t4))
                .add_visit(safari_visit(14, 3, "Safari Article Three", t5))
        };

        // First import: baseline — no watermark.
        let first_snapshot = safari_snapshot(&build_fixture(), "safari:Default");
        run_one_ingest(&env, 1, &first_snapshot, false);
        drop(first_snapshot);

        // Second import: identical data — watermark should skip everything.
        let second_snapshot = safari_snapshot(&build_fixture(), "safari:Default");
        let summary = run_one_ingest(&env, 2, &second_snapshot, true);

        assert_eq!(summary.new_urls, 0, "second import must add no new URL rows");
        assert_eq!(summary.new_visits, 0, "second import must add no new visit rows");

        // Archive row counts must stay at the first import's values.
        assert_eq!(count_archive_rows(&env, "urls"), 3);
        assert_eq!(count_archive_rows(&env, "visits"), 5);
        assert_eq!(count_urls_for_profile(&env, "safari:Default"), 3);
        assert_eq!(count_visits_for_profile(&env, "safari:Default"), 5);
    }
}

/// F2 — Firefox equivalent of C3, regression test for audit bug B2.
/// The Chromium parser's `INGEST_URLS_SQL` has an
/// `OR id IN (SELECT DISTINCT url FROM visits WHERE id > ?2)` fallback
/// to catch URLs whose `last_visit_time` is below the URL watermark but
/// which received a new visit anyway. Firefox grew the equivalent OR
/// fallback in `firefox/mod.rs:32-44` as part of the B2 fix; this
/// scenario pins that fix in place. If the Firefox URL stream loses the
/// OR-subquery in a future refactor, the new visit's `url_id_map.get` will
/// fail and `ArchiveChunkConsumer::visits`
/// will silently drop the row — the assertion below would then fail.
#[test]
fn f2_firefox_incremental_revisit_of_old_url_drops_visit_demonstrates_b2() {
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
        assert_eq!(
            visits, 3,
            "B2 fix required for Firefox: long-tail revisit silently dropped (got {visits})"
        );
    }
}

/// S2 — Audit **B2** lumped Firefox and Safari together as both missing
/// the Chromium OR-fallback. The harness proved that Safari does not
/// actually have the bug: the Safari URL query at `safari/mod.rs:42-56`
/// computes `MAX(history_visits.visit_time)` *on the fly* from the
/// visits table (Safari's `history_items` table has no cached
/// `last_visit_time` column), so any new visit row immediately raises
/// the item's effective last-visit time and the URL gets re-streamed
/// without needing an OR fallback. This contract scenario pins that
/// correct behavior — if a future refactor introduces a stored
/// `last_visit_time` cache on `history_items` without the OR fallback,
/// the same long-tail revisit bug would emerge and this test would
/// flip from passing to failing.
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
