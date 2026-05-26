//! Chromium-family ingest dedup scenarios (C1–C4, X1).
//!
//! These tests drive the real `process_profile_snapshot` pipeline against
//! synthetic `History` databases produced by the `browser-history-fixtures`
//! crate. They live here rather than in `tests/` because
//! `process_profile_snapshot` is `pub(super)` to the `archive` module; an
//! in-module test placement lets them stay end-to-end without widening the
//! public surface for testability alone.
//!
//! Each scenario function is named with the audit-spec ID it maps to (C1,
//! C2, C3, ...) so failures point directly at
//! `docs/plan/program/import-test-harness-spec.md`.
//!
//! Companion modules split by browser family:
//! - `dedup_scenarios_baselines` — Firefox/Safari baselines (F1, S1,
//!   F_C2, S_C2) + long-tail revisit scenarios (F2, S2) + Chromium
//!   fingerprint dedup.
//! - `dedup_scenarios_takeout` — Takeout-family (T1, T2, T2b, T3, T5).
//! - `dedup_scenarios_edge_cases` — cross-family edge cases (E1–E6,
//!   empty DB, R1 corrupt DB).

use super::*;
use browser_history_fixtures::{ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow};
use rusqlite::Connection;
use tempfile::{TempDir, tempdir};

fn test_config() -> AppConfig {
    AppConfig { initialized: true, ..AppConfig::default() }
}

fn test_paths(root: &Path) -> ProjectPaths {
    crate::config::project_paths_with_root(root)
}

fn chromium_profile(profile_id: &str, browser_name: &str) -> crate::models::BrowserProfile {
    crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name: "Default".to_string(),
        browser_family: "chromium".to_string(),
        browser_name: browser_name.to_string(),
        user_name: Some("synthetic-user".to_string()),
        profile_path: format!("/synthetic/{profile_id}"),
        history_path: Some(format!("/synthetic/{profile_id}/History")),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: Some("146.0.0.0".to_string()),
        history_file_name: "History".to_string(),
        history_bytes: 128,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    }
}

fn seed_run(archive: &Transaction<'_>, run_id: i64) {
    archive
        .execute(
            "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (?1, 'backup', 'manual', '2026-05-25T00:00:00+00:00', 'UTC', 'running', '[]', '[]', '{}', 0)",
            [run_id],
        )
        .expect("seed run");
}

/// Wraps one fixture file inside a `ProfileSnapshot` owned by a fresh `TempDir`.
///
/// The temp dir holds the fixture History file so that `ProfileSnapshot`'s
/// lifetime contract (the dir is dropped when the snapshot is dropped) is
/// honored exactly the same way real staging produces a snapshot.
fn snapshot_for_fixture(
    fixture: &ChromiumHistoryFixture,
    profile: crate::models::BrowserProfile,
) -> ProfileSnapshot {
    let temp_dir = tempdir().expect("snapshot tempdir");
    let history_path = temp_dir.path().join("History");
    fixture.write(&history_path).expect("write chromium fixture");
    let history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
    let mut profile = profile;
    profile.history_bytes = history_bytes;
    ProfileSnapshot {
        profile,
        temp_dir,
        history_path,
        favicons_path: None,
        source_hashes: vec![FileFingerprint {
            path: "History".to_string(),
            sha256: "synthetic-fixture-hash".to_string(),
        }],
    }
}

/// Holds the long-lived resources one scenario needs across multiple
/// imports. Owning the `TempDir` here means the project paths stay valid
/// until the scenario asserts archive state at the end.
struct ScenarioEnv {
    _root: TempDir,
    paths: ProjectPaths,
    config: AppConfig,
}

impl ScenarioEnv {
    fn new() -> Self {
        let root = tempdir().expect("scenario root tempdir");
        let paths = test_paths(root.path());
        let config = test_config();
        crate::config::ensure_paths(&paths).expect("ensure paths");
        Self { _root: root, paths, config }
    }

    fn open_archive(&self) -> Connection {
        open_archive_connection(&self.paths, &self.config, None).expect("open archive")
    }
}

/// Runs one ingest pass for a given snapshot, committing the transaction
/// before returning so subsequent asserts and re-imports observe a stable
/// canonical archive.
fn run_one_ingest(
    env: &ScenarioEnv,
    run_id: i64,
    snapshot: &ProfileSnapshot,
    use_watermark: bool,
) -> BackupProfileSummary {
    let mut archive = env.open_archive();
    let transaction = archive.transaction().expect("scenario transaction");
    seed_run(&transaction, run_id);
    let mut snapshot_artifacts = Vec::new();
    let mut source_evidence_plans = Vec::new();
    let summary = process_profile_snapshot(
        &transaction,
        run_id,
        &env.paths,
        &env.config,
        snapshot,
        &mut snapshot_artifacts,
        &mut source_evidence_plans,
        false, // allow_checkpoint
        use_watermark,
    )
    .expect("process profile snapshot");
    transaction.commit().expect("commit scenario transaction");
    summary
}

fn count_archive_rows(env: &ScenarioEnv, table: &str) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
        .expect("count rows")
}

fn count_urls_for_profile(env: &ScenarioEnv, profile_key: &str) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT COUNT(*) FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1",
            [profile_key],
            |row| row.get(0),
        )
        .expect("count urls for profile")
}

fn count_visits_for_profile(env: &ScenarioEnv, profile_key: &str) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT COUNT(*) FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = ?1",
            [profile_key],
            |row| row.get(0),
        )
        .expect("count visits for profile")
}

fn collect_visit_source_ids(env: &ScenarioEnv, profile_key: &str) -> Vec<String> {
    let archive = env.open_archive();
    let mut statement = archive
        .prepare(
            "SELECT visits.source_visit_id FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = ?1
             ORDER BY visits.source_visit_id ASC",
        )
        .expect("prepare visit ids");
    statement
        .query_map([profile_key], |row| row.get::<_, String>(0))
        .expect("query visit ids")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect visit ids")
}

/// Build a fixture with two URLs and three visits, all within one week.
fn baseline_chromium_fixture() -> ChromiumHistoryFixture {
    // 2026-05-01 00:00, 2026-05-02 12:00, 2026-05-03 08:15:30
    let visit_one_ms = 1_777_680_000_000_i64;
    let visit_two_ms = 1_777_809_600_000_i64;
    let visit_three_ms = 1_777_872_930_000_i64;

    ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/article-one".to_string(),
            title: Some("Article One".to_string()),
            visit_count: 2,
            typed_count: 1,
            last_visit_unix_ms: visit_two_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.org/article-two".to_string(),
            title: Some("Article Two".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: visit_three_ms,
            hidden: false,
        })
        .add_visit(visit_row(10, 1, visit_one_ms))
        .add_visit(visit_row(11, 1, visit_two_ms))
        .add_visit(visit_row(12, 2, visit_three_ms))
}

fn visit_row(id: i64, url_id: i64, visit_time_unix_ms: i64) -> ChromiumVisitRow {
    ChromiumVisitRow {
        id,
        url_id,
        visit_time_unix_ms,
        from_visit: Some(0),
        transition: Some(805306368),
        visit_duration_micros: Some(5_000_000),
        is_known_to_sync: false,
        visited_link_id: None,
        external_referrer_url: None,
        app_id: None,
    }
}

// ----------------------------------------------------------------------
// C1: Chromium baseline import — happy path
// ----------------------------------------------------------------------

/// C1 — One profile, one ingest pass, asserts every fixture row landed.
#[test]
fn c1_chromium_baseline_import() {
    let env = ScenarioEnv::new();
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

// ----------------------------------------------------------------------
// C2: Chromium incremental no-new-data — watermark prevents re-import
// ----------------------------------------------------------------------

/// C2 — Re-importing the same fixture with `use_watermark = true` must
/// produce zero new rows. The watermark advance after the first import
/// should make the second import a no-op at the parser level.
#[test]
fn c2_chromium_incremental_no_new_data() {
    let env = ScenarioEnv::new();
    let first_snapshot = snapshot_for_fixture(
        &baseline_chromium_fixture(),
        chromium_profile("chrome:Default", "Google Chrome"),
    );
    run_one_ingest(&env, 1, &first_snapshot, false);
    drop(first_snapshot);

    let second_snapshot = snapshot_for_fixture(
        &baseline_chromium_fixture(),
        chromium_profile("chrome:Default", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 2, &second_snapshot, true);

    assert_eq!(summary.new_urls, 0, "second import must add no new URL rows");
    assert_eq!(summary.new_visits, 0, "second import must add no new visit rows");

    assert_eq!(count_archive_rows(&env, "urls"), 2);
    assert_eq!(count_archive_rows(&env, "visits"), 3);
}

// ----------------------------------------------------------------------
// C3: Chromium incremental revisit of an old URL
// ----------------------------------------------------------------------

/// C3 — A URL whose `last_visit_time` is older than the watermark gets a
/// new visit. Without the `OR id IN (SELECT DISTINCT url FROM visits ...)`
/// fallback in `INGEST_URLS_SQL`, the URL would not be re-streamed in
/// pass 2; the new visit's `url_id_map` lookup would fail and the visit
/// would be silently dropped. This scenario asserts the fix is intact.
#[test]
fn c3_chromium_incremental_revisit_of_old_url() {
    let env = ScenarioEnv::new();

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

    let first_snapshot =
        snapshot_for_fixture(&first_fixture, chromium_profile("chrome:Default", "Google Chrome"));
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

    let second_snapshot =
        snapshot_for_fixture(&second_fixture, chromium_profile("chrome:Default", "Google Chrome"));
    let second_summary = run_one_ingest(&env, 2, &second_snapshot, true);

    assert_eq!(
        second_summary.new_visits, 1,
        "long-tail revisit captured by the OR fallback in INGEST_URLS_SQL"
    );
    assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 2);
}

// ----------------------------------------------------------------------
// X1: Edge imports Chrome history, then both diverge
// ----------------------------------------------------------------------

/// X1 — Per-source-profile contract: even when Edge and Chrome share visit
/// records (because Edge was installed and imported the Chrome history at
/// setup time), the archive must keep them as independent rows under
/// distinct `source_profiles` rows, and Edge's `browser_product` must
/// remain "Microsoft Edge" rather than collapsing to "Google Chrome"
/// (browser-support-and-adapter-playbook.md:107).
#[test]
fn x1_edge_imports_chrome_then_both_diverge() {
    let env = ScenarioEnv::new();

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

    let chrome_snapshot =
        snapshot_for_fixture(&chrome_fixture, chromium_profile("chrome:Default", "Google Chrome"));
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
            |row| row.get(0),
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
            |row| row.get(0),
        )
        .expect("chrome product");
    assert_eq!(chrome_product, "Google Chrome");
}

// T1, T2, T2b moved to dedup_scenarios_takeout.rs.

// ----------------------------------------------------------------------
// X2: Chromium-family product identity for Atlas and Comet
// ----------------------------------------------------------------------

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
    let env = ScenarioEnv::new();
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
                |row| row.get(0),
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
                |row| row.get(0),
            )
            .expect("query browser_kind")
    };

    assert_eq!(kind_for("chatgpt-atlas:Default"), "chatgpt-atlas");
    assert_eq!(kind_for("comet:Default"), "comet");
    assert_eq!(kind_for("chrome:Default"), "chrome");
}

// ----------------------------------------------------------------------
// C5: Chromium incremental growth — pure append-new-rows
// ----------------------------------------------------------------------

/// C5 — The most common real-world re-import: the user has new browsing
/// activity since last backup. Distinct from C2 (zero new rows) and C3
/// (new visit on an OLD URL exposing watermark fallback). Here the
/// second pass adds wholly new URLs and visits that did not exist in
/// the first import. The watermark advance must let only the new rows
/// land while the original rows stay deduplicated. Pins the audit §5.1
/// "re-import after appending new rows" contract.
#[test]
fn c5_chromium_incremental_append_new_urls_and_visits() {
    let env = ScenarioEnv::new();

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
    let first_snapshot =
        snapshot_for_fixture(&first_fixture, chromium_profile("chrome:Default", "Google Chrome"));
    let first_summary = run_one_ingest(&env, 1, &first_snapshot, false);
    assert_eq!(first_summary.new_urls, 2);
    assert_eq!(first_summary.new_visits, 2);
    drop(first_snapshot);

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
    let second_snapshot =
        snapshot_for_fixture(&second_fixture, chromium_profile("chrome:Default", "Google Chrome"));
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
            [],
            |row| row.get(0),
        )
        .expect("query new visit three time");
    assert_eq!(new_visit_three_ms, day_three_ms);
}

// ----------------------------------------------------------------------
// C6: Chromium source DB schema tolerance — extra columns must not break ingest
// ----------------------------------------------------------------------

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
    let env = ScenarioEnv::new();

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
                [],
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
            [],
            |row| row.get(0),
        )
        .expect("query url title after ALTER");
    assert_eq!(title.as_deref(), Some("Schema Tolerant"));
}

// ----------------------------------------------------------------------
// C4: URL upsert must not regress metadata on re-import (B1 — FIXED)
// ----------------------------------------------------------------------

/// C4 — Regression test for audit bug **B1** (fixed in 6884c10d). The URL
/// upsert in `writes.rs` now uses `MAX()` for `visit_count` / `typed_count`
/// and `CASE WHEN excluded.last_visit_ms >= urls.last_visit_ms` for `title`
/// / `hidden`, preventing older snapshots from overwriting newer metadata.
/// This test asserts all four fields survive a re-import of an older
/// snapshot without regression.
#[test]
fn c4_chromium_reimport_older_snapshot_regresses_visit_count_demonstrates_b1() {
    let env = ScenarioEnv::new();
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
    let first_snapshot =
        snapshot_for_fixture(&first_fixture, chromium_profile("chrome:Default", "Google Chrome"));
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
    let second_snapshot =
        snapshot_for_fixture(&second_fixture, chromium_profile("chrome:Default", "Google Chrome"));
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
    let third_snapshot =
        snapshot_for_fixture(&third_fixture, chromium_profile("chrome:Default", "Google Chrome"));
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

fn stored_visit_count(env: &ScenarioEnv, profile_key: &str, source_url_id: i64) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT visit_count FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1 AND urls.source_url_id = ?2",
            rusqlite::params![profile_key, source_url_id],
            |row| row.get(0),
        )
        .expect("query visit_count")
}

fn stored_title(env: &ScenarioEnv, profile_key: &str, source_url_id: i64) -> Option<String> {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT title FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1 AND urls.source_url_id = ?2",
            rusqlite::params![profile_key, source_url_id],
            |row| row.get(0),
        )
        .expect("query title")
}

fn stored_typed_count(env: &ScenarioEnv, profile_key: &str, source_url_id: i64) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT typed_count FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1 AND urls.source_url_id = ?2",
            rusqlite::params![profile_key, source_url_id],
            |row| row.get(0),
        )
        .expect("query typed_count")
}

fn stored_hidden(env: &ScenarioEnv, profile_key: &str, source_url_id: i64) -> bool {
    let archive = env.open_archive();
    let hidden_int: i64 = archive
        .query_row(
            "SELECT hidden FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1 AND urls.source_url_id = ?2",
            rusqlite::params![profile_key, source_url_id],
            |row| row.get(0),
        )
        .expect("query hidden");
    hidden_int != 0
}

// F2, S2 moved to dedup_scenarios_baselines.rs.
// T3, T5 moved to dedup_scenarios_takeout.rs.
// C_SUB_MS implemented in dedup_scenarios_edge_cases.rs.
