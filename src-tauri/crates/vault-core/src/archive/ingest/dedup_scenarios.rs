//! End-to-end ingest dedup scenarios.
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

use super::*;
use browser_history_fixtures::{
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, FirefoxPlaceRow,
    FirefoxPlacesFixture, FirefoxVisitRow, SafariHistoryFixture, SafariHistoryItemRow,
    SafariHistoryVisitRow, TakeoutBrowserHistoryFixture, TakeoutBrowserRecord,
};
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

// ----------------------------------------------------------------------
// T1: Takeout baseline import — happy path through import_takeout
// ----------------------------------------------------------------------

/// T1 — A Takeout BrowserHistory JSON gets imported via the public
/// `import_takeout` flow. Asserts row counts under the synthetic profile
/// the Takeout flow upserts (`takeout::browser-history`) and that visit
/// `app_id` lands as `"takeout"`.
#[test]
fn t1_takeout_baseline_import() {
    let env = ScenarioEnv::new();
    let source_root = tempdir().expect("takeout source root");
    let payload_path = source_root.path().join("Chrome/BrowserHistory.json");

    TakeoutBrowserHistoryFixture::new()
        .add_record(takeout_record("https://example.com/page-one", "Page One", 1_777_680_000_000))
        .add_record(takeout_record("https://example.com/page-two", "Page Two", 1_777_809_600_000))
        .add_record(takeout_record(
            "https://example.org/page-three",
            "Page Three",
            1_777_872_930_000,
        ))
        .write(&payload_path)
        .expect("write takeout fixture");

    let request = crate::models::TakeoutRequest {
        source_path: source_root.path().display().to_string(),
        dry_run: false,
    };

    let inspection = crate::takeout::import_takeout(&env.paths, &env.config, None, &request)
        .expect("import takeout");

    assert!(!inspection.dry_run);
    assert_eq!(inspection.imported_items + inspection.duplicate_items, 3);

    let profile_key = "takeout::browser-history";
    assert_eq!(count_urls_for_profile(&env, profile_key), 3);
    assert_eq!(count_visits_for_profile(&env, profile_key), 3);

    // Takeout-sourced visits must carry app_id="takeout"; this is the same
    // hardcoded marker that contributes to B4's fingerprint mismatch.
    let archive = env.open_archive();
    let takeout_visit_count: i64 = archive
        .query_row(
            "SELECT COUNT(*) FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = ?1 AND visits.app_id = 'takeout'",
            [profile_key],
            |row| row.get(0),
        )
        .expect("takeout app_id count");
    assert_eq!(takeout_visit_count, 3);
}

// ----------------------------------------------------------------------
// T2: Takeout file rename re-import — refines B3 framing
// ----------------------------------------------------------------------

/// T2 — Re-importing the same Takeout records from a different on-disk
/// path. The audit's first cut of **B3** ("path-bound source_visit_id
/// causes a full duplicate set on every re-import") turned out to overstate
/// the practical risk: while it is true that the path change does produce
/// completely different `source_visit_id` values for every record, the
/// `(source_profile_id, event_fingerprint)` partial unique index catches
/// the duplicates because the fingerprint inputs (url, visit_time_ms,
/// title, transition=None, app_id="takeout") are identical across the two
/// imports.
///
/// This scenario pins the **actual current behavior**: rename-only
/// re-import of unchanged Takeout records is correctly de-duplicated by
/// the fingerprint partial index, ending at 3 visit rows. The B3 design
/// concern (poor robustness — the path-bound id provides zero useful
/// signal, so the system relies on the fingerprint as a single layer)
/// stays documented in the audit; [`t2b_takeout_rename_with_title_change_demonstrates_b3_when_fingerprint_diverges`]
/// covers the case where the fingerprint can't save B3 anymore.
#[test]
fn t2_takeout_rename_file_reimport_dedups_via_fingerprint_partial_index() {
    let env = ScenarioEnv::new();

    let records: Vec<TakeoutBrowserRecord> = (0..3)
        .map(|index| {
            let visit_time = 1_777_680_000_000 + (index as i64 * 86_400_000);
            takeout_record(
                &format!("https://example.com/article-{index}"),
                &format!("Article {index}"),
                visit_time,
            )
        })
        .collect();

    import_takeout_fixture(&env, &records, "first");
    let profile_key = "takeout::browser-history";
    assert_eq!(count_visits_for_profile(&env, profile_key), 3);

    import_takeout_fixture(&env, &records, "second");

    // The fingerprint partial index catches the duplicates even though
    // every source_visit_id differs from the first pass.
    assert_eq!(
        count_visits_for_profile(&env, profile_key),
        3,
        "fingerprint partial index dedups the renamed-source re-import"
    );
}

/// T2b — When the fingerprint cannot rescue B3, the path-bound
/// `source_visit_id` produces a real duplicate set. Two re-imports of the
/// "same" record but with even one fingerprint input changed (title
/// here) defeat the fingerprint partial index, leaving the broken
/// path-bound primary key as the only defense. The result is the full
/// duplicate set the audit warned about.
///
/// This is a `should_panic` failing test today: the assertion below is
/// what the system should provide after B3 is fixed (e.g. by deriving
/// `source_visit_id` from `(url, visit_time_micros)` so the primary key
/// is stable across re-imports regardless of path or fingerprint input
/// drift). Today the count grows to 6 and the assertion fires.
#[test]
#[should_panic(expected = "B3 fix required")]
fn t2b_takeout_rename_with_title_change_demonstrates_b3_when_fingerprint_diverges() {
    let env = ScenarioEnv::new();

    let first_records: Vec<TakeoutBrowserRecord> = (0..3)
        .map(|index| {
            let visit_time = 1_777_680_000_000 + (index as i64 * 86_400_000);
            takeout_record(
                &format!("https://example.com/article-{index}"),
                &format!("Original title {index}"),
                visit_time,
            )
        })
        .collect();
    import_takeout_fixture(&env, &first_records, "first");

    // Real-world equivalent: user re-exports Takeout months later; Google
    // captured an updated page title in the meantime. Same URL, same
    // visit time, different title → fingerprint differs.
    let second_records: Vec<TakeoutBrowserRecord> = first_records
        .iter()
        .map(|record| {
            let mut next = record.clone();
            next.title = Some(format!(
                "Updated title for {}",
                record.url.rsplit('/').next().unwrap_or("page")
            ));
            next
        })
        .collect();
    import_takeout_fixture(&env, &second_records, "second");

    let profile_key = "takeout::browser-history";
    let visit_count = count_visits_for_profile(&env, profile_key);

    // Expected post-fix: 3 visits (treated as the same logical event with
    // an updated title). Today: 6 (because both source_visit_id and
    // event_fingerprint differ across the two imports).
    assert_eq!(
        visit_count, 3,
        "B3 fix required: rename + title drift duplicates rows (got {visit_count})"
    );
}

fn import_takeout_fixture(env: &ScenarioEnv, records: &[TakeoutBrowserRecord], label: &str) {
    let root = tempdir().unwrap_or_else(|_| panic!("{label} takeout root"));
    let payload = root.path().join("Chrome/BrowserHistory.json");
    let mut fixture = TakeoutBrowserHistoryFixture::new();
    for record in records {
        fixture = fixture.add_record(record.clone());
    }
    fixture.write(&payload).expect("write takeout fixture");
    crate::takeout::import_takeout(
        &env.paths,
        &env.config,
        None,
        &crate::models::TakeoutRequest {
            source_path: root.path().display().to_string(),
            dry_run: false,
        },
    )
    .unwrap_or_else(|err| panic!("{label} import_takeout failed: {err}"));
    // Keep root alive until the import returns; drops here once import has
    // finished walking the directory.
    drop(root);
}

fn takeout_record(url: &str, title: &str, visit_time_unix_ms: i64) -> TakeoutBrowserRecord {
    TakeoutBrowserRecord {
        url: url.to_string(),
        title: Some(title.to_string()),
        visit_time_unix_ms,
        page_transition: Some("LINK".to_string()),
        client_id: None,
        favicon_url: None,
    }
}

// ----------------------------------------------------------------------
// C4: URL upsert silently regresses counts on re-import (B1)
// ----------------------------------------------------------------------

/// C4 — Demonstrates audit bug **B1**. The URL upsert in
/// `writes.rs:123-138` unconditionally overwrites `visit_count`, `title`,
/// `typed_count`, and `hidden`; only `last_visit_ms` has a "keep newer"
/// guard. Re-importing an older snapshot (e.g. restoring a checkpoint or
/// re-ingesting an older Takeout export through the chromium adapter)
/// therefore rolls archive counts BACKWARDS even though no visit row was
/// deleted. This `#[should_panic]` test pins the broken behavior — flip
/// to plain `#[test]` once each affected field is gated on
/// `excluded.last_visit_ms >= urls.last_visit_ms`.
#[test]
#[should_panic(expected = "B1 fix required")]
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

// ----------------------------------------------------------------------
// F2: Firefox incremental revisit of an old URL drops the new visit (B2)
// ----------------------------------------------------------------------

/// F2 — Firefox equivalent of C3. The Chromium parser's
/// `INGEST_URLS_SQL` has an `OR id IN (SELECT DISTINCT url FROM visits WHERE id > ?2)`
/// fallback to catch URLs whose `last_visit_time` is below the watermark
/// but which received a new visit anyway. The Firefox parser at
/// `firefox/mod.rs:22-33` lacks that fallback: its URL stream uses
/// `WHERE COALESCE(moz_places.last_visit_date, 0) >= ?1` only. A
/// long-tail revisit therefore falls through `url_id_map` and is
/// silently dropped by `ArchiveChunkConsumer::visits`. `#[should_panic]`
/// today; flip to plain `#[test]` after Firefox grows the OR fallback.
#[test]
#[should_panic(expected = "B2 fix required for Firefox")]
fn f2_firefox_incremental_revisit_of_old_url_drops_visit_demonstrates_b2() {
    let env = ScenarioEnv::new();
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

fn firefox_snapshot(fixture: &FirefoxPlacesFixture, profile_id: &str) -> ProfileSnapshot {
    let temp_dir = tempdir().expect("firefox snapshot tempdir");
    let history_path = temp_dir.path().join("places.sqlite");
    fixture.write(&history_path).expect("write firefox fixture");
    let history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
    let mut profile = crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name: "Default".to_string(),
        browser_family: "firefox".to_string(),
        browser_name: "Firefox".to_string(),
        user_name: Some("synthetic-user".to_string()),
        profile_path: format!("/synthetic/{profile_id}"),
        history_path: Some(format!("/synthetic/{profile_id}/places.sqlite")),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: Some("125.0".to_string()),
        history_file_name: "places.sqlite".to_string(),
        history_bytes,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    };
    profile.history_bytes = history_bytes;
    ProfileSnapshot {
        profile,
        temp_dir,
        history_path,
        favicons_path: None,
        source_hashes: vec![FileFingerprint {
            path: "places.sqlite".to_string(),
            sha256: "synthetic-firefox-hash".to_string(),
        }],
    }
}

// ----------------------------------------------------------------------
// S2: Safari long-tail revisit correctly handled — refutes B2 for Safari
// ----------------------------------------------------------------------

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
    let env = ScenarioEnv::new();
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

fn safari_visit(
    id: i64,
    history_item: i64,
    title: &str,
    visit_time_unix_ms: i64,
) -> SafariHistoryVisitRow {
    SafariHistoryVisitRow {
        id,
        history_item,
        title: Some(title.to_string()),
        visit_time_unix_ms,
        load_successful: Some(true),
        http_non_get: Some(false),
        synthesized: Some(false),
        redirect_source: None,
        redirect_destination: None,
        origin: Some(0),
        generation: Some(1),
        attributes: Some(0),
        score: Some(0.5),
    }
}

fn safari_snapshot(fixture: &SafariHistoryFixture, profile_id: &str) -> ProfileSnapshot {
    let temp_dir = tempdir().expect("safari snapshot tempdir");
    let history_path = temp_dir.path().join("History.db");
    fixture.write(&history_path).expect("write safari fixture");
    let history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
    let profile = crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name: "Default".to_string(),
        browser_family: "safari".to_string(),
        browser_name: "Safari".to_string(),
        user_name: Some("synthetic-user".to_string()),
        profile_path: format!("/synthetic/{profile_id}"),
        history_path: Some(format!("/synthetic/{profile_id}/History.db")),
        favicons_path: None,
        history_exists: true,
        history_readable: true,
        access_issue: None,
        browser_version: Some("18.4".to_string()),
        history_file_name: "History.db".to_string(),
        history_bytes,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    };
    ProfileSnapshot {
        profile,
        temp_dir,
        history_path,
        favicons_path: None,
        source_hashes: vec![FileFingerprint {
            path: "History.db".to_string(),
            sha256: "synthetic-safari-hash".to_string(),
        }],
    }
}

// ----------------------------------------------------------------------
// T3: Takeout × local Chrome same-period overlap — B4 contract
// ----------------------------------------------------------------------

/// T3 — Same-period overlap between a local Chrome profile and the
/// Takeout JSON of the same Chrome installation. The audit's **B4**
/// observation: even when records describe literally the same browsing
/// event, the fingerprint inputs differ between the two source paths
/// (local Chrome has a real `transition` and the browser's real
/// `app_id`; Takeout hardcodes `app_id = "takeout"` and `transition =
/// None`), so even a hypothetical cross-source-profile fingerprint
/// dedup would not match. This contract scenario pins the current
/// storage truth — 3 + 3 = 6 visits across two profiles — and
/// documents the input divergence so any future "merge across sources"
/// proposal must address the fingerprint normalization gap first.
#[test]
fn t3_takeout_and_local_chrome_same_period_b4_contract() {
    let env = ScenarioEnv::new();
    let day_one = 1_777_680_000_000_i64;
    let day_two = 1_777_809_600_000_i64;
    let day_three = 1_777_872_930_000_i64;

    let chrome_fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/shared-one".to_string(),
            title: Some("Shared One".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/shared-two".to_string(),
            title: Some("Shared Two".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_two,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 3,
            url: "https://example.com/shared-three".to_string(),
            title: Some("Shared Three".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_three,
            hidden: false,
        })
        .add_visit(visit_row(10, 1, day_one))
        .add_visit(visit_row(11, 2, day_two))
        .add_visit(visit_row(12, 3, day_three));
    let chrome_snapshot =
        snapshot_for_fixture(&chrome_fixture, chromium_profile("chrome:Default", "Google Chrome"));
    run_one_ingest(&env, 1, &chrome_snapshot, false);

    let takeout_source = tempdir().expect("takeout source root");
    let takeout_payload = takeout_source.path().join("Chrome/BrowserHistory.json");
    TakeoutBrowserHistoryFixture::new()
        .add_record(takeout_record("https://example.com/shared-one", "Shared One", day_one))
        .add_record(takeout_record("https://example.com/shared-two", "Shared Two", day_two))
        .add_record(takeout_record("https://example.com/shared-three", "Shared Three", day_three))
        .write(&takeout_payload)
        .expect("write takeout fixture");
    crate::takeout::import_takeout(
        &env.paths,
        &env.config,
        None,
        &crate::models::TakeoutRequest {
            source_path: takeout_source.path().display().to_string(),
            dry_run: false,
        },
    )
    .expect("import takeout");

    // Each source kept independent rows under its own source_profile.
    assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 3);
    assert_eq!(count_visits_for_profile(&env, "takeout::browser-history"), 3);
    assert_eq!(count_archive_rows(&env, "visits"), 6);

    // Fingerprint divergence: a future cross-source dedup design has to
    // normalize app_id (and likely also project transition to None) before
    // any pair of these visits could share a fingerprint.
    let archive = env.open_archive();
    let chrome_app_ids: Vec<Option<String>> = archive
        .prepare(
            "SELECT app_id FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Default'",
        )
        .expect("prepare chrome")
        .query_map([], |row| row.get(0))
        .expect("query chrome")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect chrome");
    let takeout_app_ids: Vec<Option<String>> = archive
        .prepare(
            "SELECT app_id FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'takeout::browser-history'",
        )
        .expect("prepare takeout")
        .query_map([], |row| row.get(0))
        .expect("query takeout")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect takeout");
    assert!(chrome_app_ids.iter().all(|app_id| app_id.is_none()));
    assert!(takeout_app_ids.iter().all(|app_id| app_id.as_deref() == Some("takeout")));
}

// ----------------------------------------------------------------------
// T5: Takeout time_usec unit contract — B6 pinning
// ----------------------------------------------------------------------

/// T5 — Pins the current interpretation of Takeout's `time_usec` field
/// as **Unix-epoch microseconds**. The audit raised **B6** because the
/// helper `micros_to_unix_ms` (parser side) name asserts Unix
/// microseconds but Google's Takeout dumps historically used Chrome
/// epoch microseconds (since 1601). The harness writer emits Unix
/// microseconds; the parser reads Unix microseconds; this test pins
/// that contract end-to-end. If anyone later flips the parser to assume
/// Chrome epoch, T5 fails immediately. If a future real-world Takeout
/// sample disagrees with this interpretation, the writer + this test
/// must be updated together — the audit B6 note documents the open
/// question.
#[test]
fn t5_takeout_time_usec_pinned_as_unix_microseconds_b6_contract() {
    let env = ScenarioEnv::new();
    let source_root = tempdir().expect("takeout source root");
    let payload_path = source_root.path().join("Chrome/BrowserHistory.json");

    // 2026-05-02T00:00:00Z = 1_777_680_000_000 Unix ms = 1_777_680_000_000_000 Unix μs.
    // If the parser treated this as Chrome μs the resulting Unix ms would
    // be (1_777_680_000_000_000 - 11_644_473_600_000_000) / 1000, which
    // produces a negative or wildly different timestamp the assertion
    // below catches.
    let visit_one = 1_777_680_000_000_i64;

    TakeoutBrowserHistoryFixture::new()
        .add_record(takeout_record("https://example.com/time-pin", "Time Pin", visit_one))
        .write(&payload_path)
        .expect("write takeout fixture");

    crate::takeout::import_takeout(
        &env.paths,
        &env.config,
        None,
        &crate::models::TakeoutRequest {
            source_path: source_root.path().display().to_string(),
            dry_run: false,
        },
    )
    .expect("import takeout");

    let archive = env.open_archive();
    let (visit_time_ms, visit_time_iso): (i64, String) = archive
        .query_row(
            "SELECT visits.visit_time_ms, visits.visit_time_iso FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'takeout::browser-history'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query takeout visit time");

    assert_eq!(visit_time_ms, visit_one, "Takeout time_usec must round-trip as Unix milliseconds");
    assert!(
        visit_time_iso.starts_with("2026-05-02"),
        "Takeout ISO must reflect 2026-05-02, got {visit_time_iso}"
    );
}

// TODO: C_SUB_MS — Sub-millisecond Chrome visit collision scenario.
// Chrome stores visit times at microsecond precision; ingest truncates to
// milliseconds. Two visits to the same URL within the same ms produce
// identical fingerprints. The primary index (source_visit_id) keeps them
// apart, but any fingerprint-only dedup path (e.g. Takeout) would drop
// the second visit. Write a scenario with two Chrome visits 500μs apart
// to the same URL and assert both survive.
