//! Responsibilities: verify dedup scenarios in both Store key modes.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: each named case preserves the original scenario and assertions.
use super::*;

/// C1 — One profile, one ingest pass, asserts every fixture row landed.
#[test]
fn c1_chromium_baseline_import() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        let snapshot = snapshot_for_fixture(
            &baseline_chromium_fixture(),
            chromium_profile("chrome:Default", "Google Chrome"),
        );

        let summary = run_one_ingest(&env, 1, &snapshot, false);

        assert_eq!(summary.new_urls, 2, "summary reports 2 new urls");
        assert_eq!(summary.new_visits, 3, "summary reports 3 new visits");

        assert_eq!(count_archive_rows(&env, "urls"), 2);
        assert_eq!(count_archive_rows(&env, "visits"), 3);
        assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 2);
        assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 3);

        let visit_ids = collect_visit_source_ids(&env, "chrome:Default");
        assert_eq!(visit_ids, vec!["10".to_string(), "11".to_string(), "12".to_string()]);
    }
}

/// C2 — Re-importing the same fixture with `use_watermark = true` must
/// produce zero new rows. The watermark advance after the first import
/// should make the second import a no-op at the parser level.
///
/// The new-rows assertion alone does NOT prove the watermark works —
/// the fingerprint partial index would catch identical re-imports even
/// if the watermark always returned zero. We additionally query
/// `profile_watermarks` directly to assert the cursor advanced to the
/// maximum source_visit_id observed in pass 1, then stayed there after
/// the no-op pass 2.
#[test]
fn c2_chromium_incremental_no_new_data() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        let first_snapshot = snapshot_for_fixture(
            &baseline_chromium_fixture(),
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        run_one_ingest(&env, 1, &first_snapshot, false);
        drop(first_snapshot);

        // Direct watermark assertion — proves the parser actually saved the
        // cursor. baseline_chromium_fixture's max source_visit_id is 12.
        let watermark_after_pass1 = read_profile_watermark(&env, "chrome:Default");
        assert_eq!(
            watermark_after_pass1,
            Some(12),
            "C2 watermark contract: pass 1 must save the max source_visit_id observed (12)"
        );

        let second_snapshot = snapshot_for_fixture(
            &baseline_chromium_fixture(),
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let summary = run_one_ingest(&env, 2, &second_snapshot, true);

        assert_eq!(summary.new_urls, 0, "second import must add no new URL rows");
        assert_eq!(summary.new_visits, 0, "second import must add no new visit rows");

        assert_eq!(count_archive_rows(&env, "urls"), 2);
        assert_eq!(count_archive_rows(&env, "visits"), 3);

        // Watermark must not regress on the no-op pass.
        let watermark_after_pass2 = read_profile_watermark(&env, "chrome:Default");
        assert_eq!(
            watermark_after_pass2,
            Some(12),
            "C2 watermark contract: no-op pass 2 must not regress the cursor"
        );
    }
}

/// C3 — A URL whose `last_visit_time` is older than the watermark gets a
/// new visit. Without the `OR id IN (SELECT DISTINCT url FROM visits ...)`
/// fallback in `INGEST_URLS_SQL`, the URL would not be re-streamed in
/// pass 2; the new visit's `url_id_map` lookup would fail and the visit
/// would be silently dropped. This scenario asserts the fix is intact.
#[test]
fn c3_chromium_incremental_revisit_of_old_url() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

        // Initial state: one URL with a single old visit. After import, the
        // watermark sits at visit_id=10 and url_last_visit_time=visit_one.
        let visit_one_ms = 1_777_680_000_000_i64; // 2026-05-01T00:00:00Z
        let visit_two_ms = 1_777_872_930_000_i64; // 2026-05-03T08:15:30Z

        let first_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/long-tail".to_string(),
                title: Some("Long Tail Article".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: visit_one_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, visit_one_ms));

        let first_snapshot = snapshot_for_fixture(
            &first_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let first_summary = run_one_ingest(&env, 1, &first_snapshot, false);
        assert_eq!(first_summary.new_urls, 1);
        assert_eq!(first_summary.new_visits, 1);
        drop(first_snapshot);

        // Adversarial pass-2 fixture: same URL row with its last_visit_time
        // intentionally left at the OLD value (visit_one_ms), but a new
        // visit row with id > visit watermark and time > url watermark. The
        // visit cursor moves past 10; the URL cursor does not. Only the OR
        // fallback can rescue this URL into the second stream.
        let second_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/long-tail".to_string(),
                title: Some("Long Tail Article".to_string()),
                visit_count: 2,
                typed_count: 0,
                last_visit_unix_ms: visit_one_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, visit_one_ms))
            .add_visit(visit_row(11, 1, visit_two_ms));

        let second_snapshot = snapshot_for_fixture(
            &second_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let second_summary = run_one_ingest(&env, 2, &second_snapshot, true);

        assert_eq!(
            second_summary.new_visits, 1,
            "long-tail revisit captured by the OR fallback in INGEST_URLS_SQL"
        );
        assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 2);
    }
}

/// X1 — Per-source-profile contract: even when Edge and Chrome share visit
/// records (because Edge was installed and imported the Chrome history at
/// setup time), the archive must keep them as independent rows under
/// distinct `source_profiles` rows, and Edge's `browser_product` must
/// remain "Microsoft Edge" rather than collapsing to "Google Chrome"
/// (browser-support-and-adapter-playbook.md:107).
#[test]
fn x1_edge_imports_chrome_then_both_diverge() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

        let day_one_ms = 1_777_680_000_000_i64;
        let day_two_ms = 1_777_809_600_000_i64;
        let day_three_ms = 1_777_872_930_000_i64;
        let day_four_ms = 1_777_900_000_000_i64;

        // Chrome: 3 visits across 3 URLs.
        let chrome_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/shared".to_string(),
                title: Some("Shared Article".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_one_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 2,
                url: "https://example.com/chrome-only".to_string(),
                title: Some("Chrome-only Article".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_two_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 3,
                url: "https://example.com/chrome-late".to_string(),
                title: Some("Chrome Late".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_four_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, day_one_ms))
            .add_visit(visit_row(11, 2, day_two_ms))
            .add_visit(visit_row(12, 3, day_four_ms));

        // Edge: imported the shared visit from Chrome (same URL + same time),
        // then made its own visit to the same URL on day three, and finally
        // landed an Edge-only URL on day four.
        let edge_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 100,
                url: "https://example.com/shared".to_string(),
                title: Some("Shared Article".to_string()),
                visit_count: 2,
                typed_count: 0,
                last_visit_unix_ms: day_three_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 101,
                url: "https://example.com/edge-only".to_string(),
                title: Some("Edge-only Article".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_four_ms,
                hidden: false,
            })
            .add_visit(visit_row(200, 100, day_one_ms)) // imported from Chrome
            .add_visit(visit_row(201, 100, day_three_ms)) // genuine Edge visit
            .add_visit(visit_row(202, 101, day_four_ms));

        let chrome_snapshot = snapshot_for_fixture(
            &chrome_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let edge_snapshot =
            snapshot_for_fixture(&edge_fixture, chromium_profile("edge:Default", "Microsoft Edge"));

        run_one_ingest(&env, 1, &chrome_snapshot, false);
        run_one_ingest(&env, 2, &edge_snapshot, false);

        // Per-profile counts: each browser sees its own truth without merging.
        assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 3);
        assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 3);
        assert_eq!(count_urls_for_profile(&env, "edge:Default"), 2);
        assert_eq!(count_visits_for_profile(&env, "edge:Default"), 3);

        // Total archive rows: 3 + 2 url rows = 5; 3 + 3 visit rows = 6.
        // The shared URL exists once per profile (= 2 rows) by design.
        assert_eq!(count_archive_rows(&env, "urls"), 5);
        assert_eq!(count_archive_rows(&env, "visits"), 6);

        // Provenance contract: Edge profile must keep its product identity.
        let archive = env.open_archive();
        let edge_product: String = archive
            .query_row(
                "SELECT browser_product FROM source_profiles WHERE profile_key = ?1",
                ["edge:Default"],
                |row| Ok(row.get(0)?),
            )
            .expect("edge product");
        assert_eq!(
            edge_product, "Microsoft Edge",
            "Edge profile must not collapse to Google Chrome (playbook §107)"
        );

        let chrome_product: String = archive
            .query_row(
                "SELECT browser_product FROM source_profiles WHERE profile_key = ?1",
                ["chrome:Default"],
                |row| Ok(row.get(0)?),
            )
            .expect("chrome product");
        assert_eq!(chrome_product, "Google Chrome");
    }
}

/// X2 — Per the browser-support-and-adapter-playbook §156-161, ChatGPT
/// Atlas and Perplexity Comet are Chromium-family products that must
/// preserve their product identity in `source_profiles.browser_product`
/// rather than collapsing into a generic "Google Chrome". This scenario
/// pins that contract: each profile's `browser_product` column must
/// match its source `browser_name` verbatim after ingest. If a future
/// refactor accidentally normalizes all Chromium-family browsers to
/// "Google Chrome" (or strips the product distinction in any other
/// way), this test fails immediately.
#[test]
fn x2_chromium_family_products_preserve_browser_product_identity() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);
        let day_one_ms = 1_777_680_000_000_i64;

        // Each browser gets its own synthetic 1-URL, 1-visit fixture. The
        // fixture format is the same Chromium History schema for all three
        // products — what differs is the profile metadata.
        let make_fixture = |url: &str, title: &str| {
            ChromiumHistoryFixture::new()
                .add_url(ChromiumUrlRow {
                    id: 1,
                    url: url.to_string(),
                    title: Some(title.to_string()),
                    visit_count: 1,
                    typed_count: 0,
                    last_visit_unix_ms: day_one_ms,
                    hidden: false,
                })
                .add_visit(visit_row(10, 1, day_one_ms))
        };

        let atlas_snapshot = snapshot_for_fixture(
            &make_fixture("https://example.com/atlas-page", "Atlas Page"),
            chromium_profile("chatgpt-atlas:Default", "ChatGPT Atlas"),
        );
        let comet_snapshot = snapshot_for_fixture(
            &make_fixture("https://example.com/comet-page", "Comet Page"),
            chromium_profile("comet:Default", "Perplexity Comet"),
        );
        let chrome_snapshot = snapshot_for_fixture(
            &make_fixture("https://example.com/chrome-page", "Chrome Page"),
            chromium_profile("chrome:Default", "Google Chrome"),
        );

        run_one_ingest(&env, 1, &atlas_snapshot, false);
        run_one_ingest(&env, 2, &comet_snapshot, false);
        run_one_ingest(&env, 3, &chrome_snapshot, false);

        // Each profile lands as an independent source_profile with its own
        // canonical row counts.
        assert_eq!(count_urls_for_profile(&env, "chatgpt-atlas:Default"), 1);
        assert_eq!(count_visits_for_profile(&env, "chatgpt-atlas:Default"), 1);
        assert_eq!(count_urls_for_profile(&env, "comet:Default"), 1);
        assert_eq!(count_visits_for_profile(&env, "comet:Default"), 1);
        assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 1);
        assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 1);

        // Provenance contract: each browser_product must stay verbatim.
        let archive = env.open_archive();
        let product_for = |profile_key: &str| -> String {
            archive
                .query_row(
                    "SELECT browser_product FROM source_profiles WHERE profile_key = ?1",
                    [profile_key],
                    |row| Ok(row.get(0)?),
                )
                .expect("query browser_product")
        };

        assert_eq!(
            product_for("chatgpt-atlas:Default"),
            "ChatGPT Atlas",
            "ChatGPT Atlas must not collapse to Google Chrome (playbook §156)"
        );
        assert_eq!(
            product_for("comet:Default"),
            "Perplexity Comet",
            "Perplexity Comet must not collapse to Google Chrome (playbook §158)"
        );
        assert_eq!(product_for("chrome:Default"), "Google Chrome");

        // browser_kind (derived from profile_id prefix) must also distinguish them.
        let kind_for = |profile_key: &str| -> String {
            archive
                .query_row(
                    "SELECT browser_kind FROM source_profiles WHERE profile_key = ?1",
                    [profile_key],
                    |row| Ok(row.get(0)?),
                )
                .expect("query browser_kind")
        };

        assert_eq!(kind_for("chatgpt-atlas:Default"), "chatgpt-atlas");
        assert_eq!(kind_for("comet:Default"), "comet");
        assert_eq!(kind_for("chrome:Default"), "chrome");
    }
}

/// C5 — The most common real-world re-import: the user has new browsing
/// activity since last backup. Distinct from C2 (zero new rows) and C3
/// (new visit on an OLD URL exposing watermark fallback). Here the
/// second pass adds wholly new URLs and visits that did not exist in
/// the first import. The watermark advance must let only the new rows
/// land while the original rows stay deduplicated. Pins the audit §5.1
/// "re-import after appending new rows" contract.
#[test]
fn c5_chromium_incremental_append_new_urls_and_visits() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

        let day_one_ms = 1_777_680_000_000_i64;
        let day_two_ms = 1_777_809_600_000_i64;
        let day_three_ms = 1_777_872_930_000_i64;
        let day_four_ms = 1_777_939_200_000_i64;

        // Pass 1: 2 URLs, 2 visits.
        let first_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/original-one".to_string(),
                title: Some("Original One".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_one_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 2,
                url: "https://example.com/original-two".to_string(),
                title: Some("Original Two".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_two_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, day_one_ms))
            .add_visit(visit_row(11, 2, day_two_ms));
        let first_snapshot = snapshot_for_fixture(
            &first_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let first_summary = run_one_ingest(&env, 1, &first_snapshot, false);
        assert_eq!(first_summary.new_urls, 2);
        assert_eq!(first_summary.new_visits, 2);
        drop(first_snapshot);

        // Direct watermark assertion — pins that the parser saved cursor=11
        // after pass 1, otherwise pass 2's new_visits=2 below could be
        // satisfied by a broken watermark that re-streams everything and
        // relies on fingerprint dedup to drop the originals.
        assert_eq!(
            read_profile_watermark(&env, "chrome:Default"),
            Some(11),
            "C5 watermark contract: pass 1 must save cursor at max source_visit_id (11)"
        );

        // Pass 2: same 2 URLs + 2 NEW URLs + 2 NEW visits (one per new URL).
        // The originals must stay deduplicated; only the 2 new URLs / 2 new
        // visits should land.
        let second_fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/original-one".to_string(),
                title: Some("Original One".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_one_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 2,
                url: "https://example.com/original-two".to_string(),
                title: Some("Original Two".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_two_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 3,
                url: "https://example.com/new-three".to_string(),
                title: Some("New Three".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_three_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 4,
                url: "https://example.com/new-four".to_string(),
                title: Some("New Four".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_four_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, day_one_ms))
            .add_visit(visit_row(11, 2, day_two_ms))
            .add_visit(visit_row(12, 3, day_three_ms))
            .add_visit(visit_row(13, 4, day_four_ms));
        let second_snapshot = snapshot_for_fixture(
            &second_fixture,
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let second_summary = run_one_ingest(&env, 2, &second_snapshot, true);

        // Summary must report exactly the new content.
        assert_eq!(second_summary.new_urls, 2, "second import should report 2 new URLs");
        assert_eq!(second_summary.new_visits, 2, "second import should report 2 new visits");

        // Archive totals: 4 URLs, 4 visits.
        assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 4);
        assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 4);
        assert_eq!(count_archive_rows(&env, "urls"), 4);
        assert_eq!(count_archive_rows(&env, "visits"), 4);

        // Source visit IDs flow through unmodified (sorted lexically: 10, 11, 12, 13).
        let visit_ids = collect_visit_source_ids(&env, "chrome:Default");
        assert_eq!(visit_ids, vec!["10", "11", "12", "13"]);

        // Confirm the new visit timestamps round-tripped, not just the row count.
        let archive = env.open_archive();
        let new_visit_three_ms: i64 = archive
            .query_row(
                "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Default'
               AND visits.source_visit_id = '12'",
                (),
                |row| Ok(row.get(0)?),
            )
            .expect("query new visit three time");
        assert_eq!(new_visit_three_ms, day_three_ms);

        // Direct watermark assertion: pass 2's parser ran with cursor=11
        // (saved by pass 1) and observed visits 12, 13. The cursor must
        // have advanced to 13 after pass 2 commits. If a future regression
        // breaks the watermark save and pass 2 silently re-streamed every
        // visit (with fingerprint dedup masking the row counts), this
        // assertion catches it.
        assert_eq!(
            read_profile_watermark(&env, "chrome:Default"),
            Some(13),
            "C5 watermark contract: pass 2 must advance the cursor to the new max (13)"
        );
    }
}

/// X3 — Real users almost always have multiple Chrome profiles
/// (`Default`, `Profile 1`, sometimes more). Each profile is a separate
/// `~/Library/Application Support/Google/Chrome/<Profile>/History`
/// file, discovered as an independent `BrowserProfile`. The dedup
/// contract requires:
///
/// 1. **Independent source_profiles**: `profile_key = "chrome:Default"`
///    and `profile_key = "chrome:Profile 1"` must produce two distinct
///    rows in `source_profiles` (no collision under same `browser_kind`).
/// 2. **Per-profile dedup scope**: identical visits across the two
///    profiles must not deduplicate. The `event_fingerprint` partial
///    unique index is scoped by `source_profile_id`, so each profile
///    keeps its own copy.
/// 3. **Per-profile watermark isolation**: a re-import of Profile 1
///    after Default has been ingested must not be affected by Default's
///    watermark advance — both profiles get independent incremental
///    state.
///
/// This is the multi-profile mirror of X1's cross-browser test. If a
/// future refactor accidentally key the watermark by `browser_kind` only
/// (instead of by `source_profile_id`), or merges identical visits
/// across profiles, this scenario fails.
#[test]
fn x3_multiple_profiles_within_same_browser_stay_independent() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

        let day_one_ms = 1_777_680_000_000_i64;
        let day_two_ms = 1_777_809_600_000_i64;
        let day_three_ms = 1_777_872_930_000_i64;

        // Both profiles share the same URL + visit time (e.g. the user
        // visited the same article from both work and personal profiles).
        let shared_fixture = |source_url_id: i64, source_visit_id: i64| {
            ChromiumHistoryFixture::new()
                .add_url(ChromiumUrlRow {
                    id: source_url_id,
                    url: "https://example.com/cross-profile".to_string(),
                    title: Some("Cross Profile".to_string()),
                    visit_count: 1,
                    typed_count: 0,
                    last_visit_unix_ms: day_one_ms,
                    hidden: false,
                })
                .add_visit(visit_row(source_visit_id, source_url_id, day_one_ms))
        };

        // Default: pass 1 — single shared URL + visit.
        let default_snap_1 = snapshot_for_fixture(
            &shared_fixture(1, 10),
            chromium_profile("chrome:Default", "Google Chrome"),
        );
        let default_summary_1 = run_one_ingest(&env, 1, &default_snap_1, false);
        assert_eq!(default_summary_1.new_urls, 1);
        assert_eq!(default_summary_1.new_visits, 1);
        drop(default_snap_1);

        // Profile 1: pass 1 — same URL + visit time but DIFFERENT
        // source_visit_id (each Chrome profile has its own rowid sequence).
        // The fingerprint inputs (url, visit_time_ms, title, transition,
        // app_id) match Default's, but the fingerprint partial index is
        // scoped per source_profile_id, so this visit must NOT dedup.
        let profile1_snap_1 = snapshot_for_fixture(
            &shared_fixture(1, 99),
            chromium_profile("chrome:Profile 1", "Google Chrome"),
        );
        let profile1_summary_1 = run_one_ingest(&env, 2, &profile1_snap_1, false);
        assert_eq!(
            profile1_summary_1.new_urls, 1,
            "Profile 1's URL must land independently of Default's"
        );
        assert_eq!(
            profile1_summary_1.new_visits, 1,
            "identical visit across profiles must not dedup (per-profile fingerprint scope)"
        );

        // Per-profile counts confirm the two profiles each hold one URL +
        // one visit, even though the visit content is identical.
        assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 1);
        assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 1);
        assert_eq!(count_urls_for_profile(&env, "chrome:Profile 1"), 1);
        assert_eq!(count_visits_for_profile(&env, "chrome:Profile 1"), 1);
        assert_eq!(count_archive_rows(&env, "urls"), 2);
        assert_eq!(count_archive_rows(&env, "visits"), 2);

        // Direct per-profile watermark assertion — pins that the two
        // profiles each have their own profile_watermarks row keyed by
        // their distinct profile_id. If a regression keyed watermarks by
        // browser_kind only (cross-profile bleed), these two queries would
        // return the same value or one of them would be missing.
        assert_eq!(
            read_profile_watermark(&env, "chrome:Default"),
            Some(10),
            "Default's watermark must be saved at its own max source_visit_id (10)"
        );
        assert_eq!(
            read_profile_watermark(&env, "chrome:Profile 1"),
            Some(99),
            "Profile 1's watermark must be saved at its own max source_visit_id (99), \
         independently of Default's"
        );

        // Per-profile watermark isolation: now re-import Profile 1 with
        // NEW activity (the user kept browsing on Profile 1). Default's
        // watermark advance from pass 1 must not affect Profile 1's
        // incremental cursor. Profile 1's new content must be detected.
        let profile1_fixture_2 = ChromiumHistoryFixture::new()
            // Same URL+visit as Profile 1's pass 1 — must dedup at Profile 1's
            // partial fingerprint index.
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/cross-profile".to_string(),
                title: Some("Cross Profile".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_one_ms,
                hidden: false,
            })
            // New URL only seen on Profile 1.
            .add_url(ChromiumUrlRow {
                id: 2,
                url: "https://example.com/profile-one-only".to_string(),
                title: Some("Profile One Only".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_two_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 3,
                url: "https://example.com/profile-one-late".to_string(),
                title: Some("Profile One Late".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_three_ms,
                hidden: false,
            })
            .add_visit(visit_row(99, 1, day_one_ms))
            .add_visit(visit_row(100, 2, day_two_ms))
            .add_visit(visit_row(101, 3, day_three_ms));
        let profile1_snap_2 = snapshot_for_fixture(
            &profile1_fixture_2,
            chromium_profile("chrome:Profile 1", "Google Chrome"),
        );
        let profile1_summary_2 = run_one_ingest(&env, 3, &profile1_snap_2, true);

        // Watermark must have been read from Profile 1's own state (not
        // Default's). Profile 1 sees 2 new URLs and 2 new visits.
        assert_eq!(
            profile1_summary_2.new_urls, 2,
            "Profile 1's incremental import must pick up its own 2 new URLs"
        );
        assert_eq!(
            profile1_summary_2.new_visits, 2,
            "Profile 1's incremental import must pick up its own 2 new visits"
        );

        // Final per-profile counts.
        assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 1, "Default untouched");
        assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 1, "Default untouched");
        assert_eq!(count_urls_for_profile(&env, "chrome:Profile 1"), 3);
        assert_eq!(count_visits_for_profile(&env, "chrome:Profile 1"), 3);
        assert_eq!(count_archive_rows(&env, "urls"), 4);
        assert_eq!(count_archive_rows(&env, "visits"), 4);

        // Direct watermark assertion after Profile 1's incremental pass:
        // Default's cursor must remain frozen at 10, Profile 1's must have
        // advanced to 101 (the new max). If a regression made the two
        // profiles share a single watermark, Default's cursor would have
        // jumped to 101 too — which this assertion catches.
        assert_eq!(
            read_profile_watermark(&env, "chrome:Default"),
            Some(10),
            "Default's watermark must NOT be touched by Profile 1's incremental import"
        );
        assert_eq!(
            read_profile_watermark(&env, "chrome:Profile 1"),
            Some(101),
            "Profile 1's watermark must have advanced to the new max source_visit_id (101)"
        );

        // Provenance: both share `browser_kind = chrome` and
        // `browser_product = Google Chrome` but have distinct `profile_key`
        // and `profile_name`.
        let archive = env.open_archive();
        let collect_profile_meta = |profile_key: &str| -> (String, String, String) {
            archive
                .query_row(
                    "SELECT browser_kind, browser_product, profile_name
                 FROM source_profiles WHERE profile_key = ?1",
                    [profile_key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("profile meta")
        };
        let (default_kind, default_product, default_name) = collect_profile_meta("chrome:Default");
        let (profile1_kind, profile1_product, profile1_name) =
            collect_profile_meta("chrome:Profile 1");
        assert_eq!(default_kind, "chrome");
        assert_eq!(profile1_kind, "chrome");
        assert_eq!(default_product, "Google Chrome");
        assert_eq!(profile1_product, "Google Chrome");
        assert_eq!(default_name, "Default");
        // profile_name comes from chromium_profile helper which hardcodes
        // "Default"; in real PathKeep it would be the OS-discovered name.
        // Both still produce distinct profile_keys via the profile_id input.
        assert_eq!(profile1_name, "Default");
    }
}

/// C6 — Chrome's `History` schema grows over time (real Chrome adds
/// columns like `favicon_id` on `urls`, plus `segment_id`,
/// `opener_visit`, and the `originator_*` sync metadata fields on
/// `visits`). PathKeep's parser uses **explicit column lists** in
/// SELECTs (see `INGEST_URLS_SQL`, `INGEST_VISITS_SQL`), so extra
/// columns in the source DB must be silently tolerated. This scenario
/// pins that contract: a fixture DB with `ALTER TABLE`-added columns
/// must import without error and produce identical canonical rows.
///
/// If a future refactor switches to `SELECT *` or otherwise becomes
/// column-count-sensitive, this test fails immediately. This is the
/// §5.1 "re-import after schema migration in the source DB" contract.
#[test]
fn c6_chromium_extra_columns_on_source_db_do_not_break_ingest() {
    for key in [None, Some([83; 32])] {
        let env = ScenarioEnv::new(key);

        let day_one_ms = 1_777_680_000_000_i64;
        let day_two_ms = 1_777_809_600_000_i64;

        let fixture = ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 1,
                url: "https://example.com/schema-tolerant".to_string(),
                title: Some("Schema Tolerant".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_one_ms,
                hidden: false,
            })
            .add_url(ChromiumUrlRow {
                id: 2,
                url: "https://example.com/schema-tolerant-two".to_string(),
                title: Some("Schema Tolerant Two".to_string()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: day_two_ms,
                hidden: false,
            })
            .add_visit(visit_row(10, 1, day_one_ms))
            .add_visit(visit_row(11, 2, day_two_ms));

        let temp_dir = tempdir().expect("snapshot tempdir");
        let history_path = temp_dir.path().join("History");
        fixture.write(&history_path).expect("write chromium fixture");

        // Simulate Chrome adding new columns in a later release. The
        // PathKeep parser must continue to project only the columns it
        // explicitly names; the extras must be ignored entirely.
        {
            let connection = Connection::open(&history_path).expect("open fixture for ALTER");
            // Real Chrome additions over time:
            connection
                .execute("ALTER TABLE urls ADD COLUMN favicon_id INTEGER", [])
                .expect("add favicon_id");
            connection
                .execute("ALTER TABLE visits ADD COLUMN segment_id INTEGER", [])
                .expect("add segment_id");
            connection
                .execute("ALTER TABLE visits ADD COLUMN opener_visit INTEGER", [])
                .expect("add opener_visit");
            connection
                .execute("ALTER TABLE visits ADD COLUMN originator_cache_guid TEXT", [])
                .expect("add originator_cache_guid");
            // Populate the new columns with synthetic data so the schema isn't
            // just a NULL column suffix — proves the parser truly ignores them.
            connection
                .execute("UPDATE urls SET favicon_id = 42 WHERE id = 1", [])
                .expect("populate favicon_id");
            connection
            .execute(
                "UPDATE visits SET segment_id = 7, opener_visit = 0, originator_cache_guid = 'synthetic-originator' WHERE id = 10",
                (),
            )
            .expect("populate visit extras");
        }

        let history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
        let mut profile = chromium_profile("chrome:Default", "Google Chrome");
        profile.history_bytes = history_bytes;
        let snapshot = ProfileSnapshot {
            profile,
            temp_dir,
            history_path,
            favicons_path: None,
            source_hashes: vec![FileFingerprint {
                path: "History".to_string(),
                sha256: "synthetic-fixture-hash".to_string(),
            }],
        };

        let summary = run_one_ingest(&env, 1, &snapshot, false);

        // The extra columns must be silently ignored — canonical row counts
        // must match what a normal fixture without ALTER TABLE produces.
        assert_eq!(
            summary.new_urls, 2,
            "schema-tolerance: URL count must match minimal-schema fixture"
        );
        assert_eq!(
            summary.new_visits, 2,
            "schema-tolerance: visit count must match minimal-schema fixture"
        );
        assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 2);
        assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 2);

        // Spot-check that the columns the parser DOES project still landed.
        let archive = env.open_archive();
        let title: Option<String> = archive
            .query_row(
                "SELECT title FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Default'
               AND urls.source_url_id = 1",
                (),
                |row| Ok(row.get(0)?),
            )
            .expect("query url title after ALTER");
        assert_eq!(title.as_deref(), Some("Schema Tolerant"));
    }
}
