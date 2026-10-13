//! Responsibilities: verify dedup scenarios in both Store key modes.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: each named case preserves the original scenario and assertions.
use super::*;

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
/// distinct `source_profiles` rows; the fingerprint index must not drop
/// Edge's copy of the shared visit.
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
    }
}

/// C6 — Chrome's `History` schema grows over time (real Chrome adds
/// columns like `favicon_id` on `urls`, plus `segment_id`,
/// `opener_visit`, and the `originator_*` sync metadata fields on
/// `visits`). PathKeep's parser uses **explicit column lists** in
/// SELECTs (see `INGEST_URLS_SQL`, `INGEST_VISITS_SQL`), so extra
/// columns in the source DB must not break ingest. A fixture DB with
/// `ALTER TABLE`-added columns must import without error and produce the
/// same canonical rows. Whether the extra values are also kept as source
/// evidence (ADR-011 §5) is not asserted here.
///
/// If a future refactor switches to `SELECT *` or otherwise becomes
/// column-count-sensitive, this test fails immediately.
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
        // canonical rows come only from the columns the parser names.
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
            // just a NULL column suffix.
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

        // Canonical row counts must match what a normal fixture without
        // ALTER TABLE produces.
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
