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
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, TakeoutBrowserHistoryFixture,
    TakeoutBrowserRecord,
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
        .add_record(takeout_record("https://example.org/page-three", "Page Three", 1_777_872_930_000))
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
    assert_eq!(visit_count, 3, "B3 fix required: rename + title drift duplicates rows (got {visit_count})");
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
