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
