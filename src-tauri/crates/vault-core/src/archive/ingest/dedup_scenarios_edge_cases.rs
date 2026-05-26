//! Edge-case and contract-pinning ingest scenarios.
//!
//! These tests complement `dedup_scenarios.rs` (main Chromium dedup paths)
//! and `dedup_scenarios_baselines.rs` (Firefox/Safari baselines) by covering:
//! - **C_SUB_MS (E5)**: Sub-millisecond Chrome visit collision
//! - **E6**: URL canonicalization — no normalization applied
//! - **Empty DB**: Zero-row fixtures for all browser families
//! - **R1**: Corrupt / malformed source database resilience
//! - **E1-E4**: Time boundary edge cases (epoch, year-2038, far-future, negative)
//! - **E7**: NULL title handling
//! - **E8**: Unicode (CJK, percent-encoded, emoji) byte-identical round-trip
//! - **E9**: `hidden = true` URL flag round-trip

use super::*;
use browser_history_fixtures::{
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, FirefoxPlacesFixture,
    SafariHistoryFixture,
};
use std::io::Write;
use tempfile::tempdir;

// ── Shared helpers (mirror dedup_scenarios.rs patterns) ─────────────

fn test_config() -> AppConfig {
    AppConfig { initialized: true, ..AppConfig::default() }
}

fn test_paths(root: &Path) -> ProjectPaths {
    crate::config::project_paths_with_root(root)
}

/// Holds the long-lived resources one scenario needs across multiple
/// imports (same as dedup_scenarios::ScenarioEnv).
struct ScenarioEnv {
    _root: tempfile::TempDir,
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

    fn open_archive(&self) -> rusqlite::Connection {
        open_archive_connection(&self.paths, &self.config, None).expect("open archive")
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
        false,
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

// ── Chromium helpers ────────────────────────────────────────────────

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

fn chromium_visit_row(id: i64, url_id: i64, visit_time_unix_ms: i64) -> ChromiumVisitRow {
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

fn snapshot_for_chromium_fixture(
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

// ── Firefox helpers ─────────────────────────────────────────────────

fn firefox_profile(profile_id: &str) -> crate::models::BrowserProfile {
    crate::models::BrowserProfile {
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
        history_bytes: 128,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    }
}

fn firefox_snapshot(fixture: &FirefoxPlacesFixture, profile_id: &str) -> ProfileSnapshot {
    let temp_dir = tempdir().expect("firefox snapshot tempdir");
    let history_path = temp_dir.path().join("places.sqlite");
    fixture.write(&history_path).expect("write firefox fixture");
    let history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
    let mut profile = firefox_profile(profile_id);
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

// ── Safari helpers ──────────────────────────────────────────────────

fn safari_profile(profile_id: &str) -> crate::models::BrowserProfile {
    crate::models::BrowserProfile {
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
        history_bytes: 128,
        favicons_bytes: 0,
        supporting_bytes: 0,
        retention_boundary: crate::models::BrowserRetentionBoundary::default(),
    }
}

fn safari_snapshot(fixture: &SafariHistoryFixture, profile_id: &str) -> ProfileSnapshot {
    let temp_dir = tempdir().expect("safari snapshot tempdir");
    let history_path = temp_dir.path().join("History.db");
    fixture.write(&history_path).expect("write safari fixture");
    let history_bytes = std::fs::metadata(&history_path).map(|meta| meta.len()).unwrap_or(0);
    let mut profile = safari_profile(profile_id);
    profile.history_bytes = history_bytes;
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

// ======================================================================
// C_SUB_MS (E5) — Sub-millisecond Chrome visit collision contract
// ======================================================================

/// C_SUB_MS (E5) — Sub-millisecond Chrome visit collision contract.
///
/// Chrome stores visit times at microsecond precision; our parser truncates
/// to milliseconds. Two visits to the same URL within the same millisecond
/// produce identical `event_fingerprint` values. The partial unique index
/// deduplicates the second visit even though source_visit_ids differ.
///
/// This is a known acceptable limitation, not a bug. This test pins the
/// behavior so that any future precision change is caught.
#[test]
fn c_sub_ms_same_millisecond_visits_collapsed_by_fingerprint() {
    let env = ScenarioEnv::new();

    // Two visits to the same URL with different source_visit_ids but
    // identical visit_time_unix_ms. The fingerprint computation uses
    // unix_micros_to_chrome_time(visit_time_ms * 1000), so both visits
    // produce the same Chrome time → same fingerprint → INSERT OR IGNORE
    // silently skips the second.
    let same_ms = 1_777_680_000_000_i64; // 2026-05-01T00:00:00Z

    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/sub-ms-collision".to_string(),
            title: Some("Sub-ms Collision".to_string()),
            visit_count: 2,
            typed_count: 0,
            last_visit_unix_ms: same_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(20, 1, same_ms))
        .add_visit(chromium_visit_row(21, 1, same_ms));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:Default", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);

    // The parser delivers both visits, but only one survives archive insert:
    // - Visit 20 inserted successfully (new source_visit_id, new fingerprint).
    // - Visit 21 has a DIFFERENT source_visit_id (so UNIQUE(source_profile_id,
    //   source_visit_id) does not fire) but the SAME event_fingerprint (same
    //   url, same Chrome time, same title, same transition, same app_id).
    //   The partial unique index on (source_profile_id, event_fingerprint)
    //   triggers → INSERT OR IGNORE silently skips.
    assert_eq!(
        summary.new_visits, 1,
        "only one of two same-millisecond visits should survive fingerprint dedup"
    );
    assert_eq!(summary.new_urls, 1);
    assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 1);
    assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 1);
}

// ======================================================================
// E6 — URL canonicalization contract: no normalization applied
// ======================================================================

/// E6 — URL canonicalization contract pins.
///
/// PathKeep stores URL strings as-is with NO normalization. Different URL
/// strings with different source_url_ids must be preserved as separate URL
/// rows even when they point to semantically "the same" resource. This
/// pins the contract so a future normalization change is caught.
#[test]
fn e6_url_strings_stored_verbatim_no_normalization() {
    let env = ScenarioEnv::new();

    let t1 = 1_777_680_000_000_i64;
    let t2 = 1_777_809_600_000_i64;
    let t3 = 1_777_872_930_000_i64;
    let t4 = 1_777_939_200_000_i64;

    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/path".to_string(),
            title: Some("Base URL".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: t1,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/path/".to_string(),
            title: Some("Trailing Slash".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: t2,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 3,
            url: "https://example.com/page#section".to_string(),
            title: Some("Fragment Preserved".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: t3,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 4,
            url: "https://Example.COM/Path".to_string(),
            title: Some("Mixed Case".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: t4,
            hidden: false,
        })
        .add_visit(chromium_visit_row(10, 1, t1))
        .add_visit(chromium_visit_row(11, 2, t2))
        .add_visit(chromium_visit_row(12, 3, t3))
        .add_visit(chromium_visit_row(13, 4, t4));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:Default", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);

    // All four URLs must be preserved as distinct rows.
    assert_eq!(summary.new_urls, 4, "all URL variants must be separate rows");
    assert_eq!(summary.new_visits, 4);
    assert_eq!(count_urls_for_profile(&env, "chrome:Default"), 4);
    assert_eq!(count_visits_for_profile(&env, "chrome:Default"), 4);

    // Query back every URL string and assert verbatim storage.
    let archive = env.open_archive();
    let expected_urls = [
        (1_i64, "https://example.com/path"),
        (2, "https://example.com/path/"),
        (3, "https://example.com/page#section"),
        (4, "https://Example.COM/Path"),
    ];
    for (source_url_id, expected_url) in expected_urls {
        let stored_url: String = archive
            .query_row(
                "SELECT url FROM urls
                 JOIN source_profiles ON source_profiles.id = urls.source_profile_id
                 WHERE source_profiles.profile_key = 'chrome:Default'
                   AND urls.source_url_id = ?1",
                [source_url_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| panic!("query URL for source_url_id={source_url_id}"));
        assert_eq!(
            stored_url, expected_url,
            "URL with source_url_id={source_url_id} must be stored verbatim"
        );
    }
}

// ======================================================================
// Empty DB — Zero-row fixtures for all browser families
// ======================================================================

/// Empty Chromium fixture: import completes without error, summary is zero.
#[test]
fn empty_chromium_fixture_imports_without_error() {
    let env = ScenarioEnv::new();
    let fixture = ChromiumHistoryFixture::new();
    let snapshot =
        snapshot_for_chromium_fixture(&fixture, chromium_profile("chrome:Empty", "Google Chrome"));
    let summary = run_one_ingest(&env, 1, &snapshot, false);

    assert_eq!(summary.new_urls, 0, "empty fixture must produce 0 new URLs");
    assert_eq!(summary.new_visits, 0, "empty fixture must produce 0 new visits");
    assert_eq!(count_archive_rows(&env, "urls"), 0);
    assert_eq!(count_archive_rows(&env, "visits"), 0);
}

/// Empty Firefox fixture: import completes without error, summary is zero.
#[test]
fn empty_firefox_fixture_imports_without_error() {
    let env = ScenarioEnv::new();
    let fixture = FirefoxPlacesFixture::new();
    let snapshot = firefox_snapshot(&fixture, "firefox:Empty");
    let summary = run_one_ingest(&env, 1, &snapshot, false);

    assert_eq!(summary.new_urls, 0, "empty fixture must produce 0 new URLs");
    assert_eq!(summary.new_visits, 0, "empty fixture must produce 0 new visits");
    assert_eq!(count_archive_rows(&env, "urls"), 0);
    assert_eq!(count_archive_rows(&env, "visits"), 0);
}

/// Empty Safari fixture: import completes without error, summary is zero.
#[test]
fn empty_safari_fixture_imports_without_error() {
    let env = ScenarioEnv::new();
    let fixture = SafariHistoryFixture::new();
    let snapshot = safari_snapshot(&fixture, "safari:Empty");
    let summary = run_one_ingest(&env, 1, &snapshot, false);

    assert_eq!(summary.new_urls, 0, "empty fixture must produce 0 new URLs");
    assert_eq!(summary.new_visits, 0, "empty fixture must produce 0 new visits");
    assert_eq!(count_archive_rows(&env, "urls"), 0);
    assert_eq!(count_archive_rows(&env, "visits"), 0);
}

// ======================================================================
// R1 — Corrupt / malformed source database resilience
// ======================================================================

/// R1a — A file containing random bytes (not a valid SQLite database) must
/// cause `process_profile_snapshot` to return `Err`, not panic.
#[test]
fn r1a_corrupt_random_bytes_returns_error_not_panic() {
    let env = ScenarioEnv::new();
    let snapshot_dir = tempdir().expect("corrupt snapshot tempdir");
    let corrupt_path = snapshot_dir.path().join("History");
    {
        let mut file = std::fs::File::create(&corrupt_path).expect("create corrupt file");
        file.write_all(b"not a database at all, just random garbage bytes 0xDEADBEEF")
            .expect("write corrupt bytes");
    }

    let profile = chromium_profile("chrome:Corrupt", "Google Chrome");
    let snapshot = ProfileSnapshot {
        profile,
        temp_dir: snapshot_dir,
        history_path: corrupt_path,
        favicons_path: None,
        source_hashes: vec![FileFingerprint {
            path: "History".to_string(),
            sha256: "corrupt-hash".to_string(),
        }],
    };

    let mut archive = env.open_archive();
    let transaction = archive.transaction().expect("transaction");
    seed_run(&transaction, 1);
    let mut snapshot_artifacts = Vec::new();
    let mut source_evidence_plans = Vec::new();
    let result = process_profile_snapshot(
        &transaction,
        1,
        &env.paths,
        &env.config,
        &snapshot,
        &mut snapshot_artifacts,
        &mut source_evidence_plans,
        false,
        false,
    );

    assert!(result.is_err(), "corrupt random-bytes file must return Err, not panic");
}

/// R1b — A valid SQLite database but missing required browser tables must
/// cause `process_profile_snapshot` to return `Err`, not panic.
#[test]
fn r1b_valid_sqlite_missing_tables_returns_error_not_panic() {
    let env = ScenarioEnv::new();
    let snapshot_dir = tempdir().expect("missing-tables snapshot tempdir");
    let db_path = snapshot_dir.path().join("History");
    {
        let conn = rusqlite::Connection::open(&db_path).expect("create empty sqlite");
        conn.execute_batch("CREATE TABLE dummy (id INTEGER PRIMARY KEY)")
            .expect("create dummy table");
    }

    let profile = chromium_profile("chrome:MissingTables", "Google Chrome");
    let snapshot = ProfileSnapshot {
        profile,
        temp_dir: snapshot_dir,
        history_path: db_path,
        favicons_path: None,
        source_hashes: vec![FileFingerprint {
            path: "History".to_string(),
            sha256: "missing-tables-hash".to_string(),
        }],
    };

    let mut archive = env.open_archive();
    let transaction = archive.transaction().expect("transaction");
    seed_run(&transaction, 1);
    let mut snapshot_artifacts = Vec::new();
    let mut source_evidence_plans = Vec::new();
    let result = process_profile_snapshot(
        &transaction,
        1,
        &env.paths,
        &env.config,
        &snapshot,
        &mut snapshot_artifacts,
        &mut source_evidence_plans,
        false,
        false,
    );

    assert!(result.is_err(), "valid SQLite with missing browser tables must return Err, not panic");
}

// ======================================================================
// E1-E4 — Time boundary edge cases
// ======================================================================

/// E1 — Epoch timestamp boundary: visit_time_ms = 0 (1970-01-01T00:00:00Z).
/// A zero timestamp is legal in the archive schema and must round-trip
/// without error. This pins the lower bound of the time domain.
#[test]
fn e1_epoch_timestamp_imports_without_error() {
    let env = ScenarioEnv::new();
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/epoch".to_string(),
            title: Some("Epoch Boundary".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: 0,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, 0));
    let snapshot =
        snapshot_for_chromium_fixture(&fixture, chromium_profile("chrome:Epoch", "Google Chrome"));
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 1);
    assert_eq!(summary.new_visits, 1);
    // Verify the timestamp is stored as 0.
    let archive = env.open_archive();
    let visit_time: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Epoch'",
            [],
            |row| row.get(0),
        )
        .expect("query epoch visit time");
    assert_eq!(visit_time, 0, "epoch timestamp must round-trip as 0");
}

/// E2 — Year-2038 boundary (2038-01-19T03:14:07Z = 2_147_483_647_000 ms).
/// PathKeep uses i64 for timestamps, so the 32-bit overflow must be
/// transparent. This pins the contract.
#[test]
fn e2_year_2038_boundary_imports_without_error() {
    let env = ScenarioEnv::new();
    let y2038_ms = 2_147_483_647_000_i64; // 2038-01-19T03:14:07Z
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/y2038".to_string(),
            title: Some("Year 2038".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: y2038_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, y2038_ms));
    let snapshot =
        snapshot_for_chromium_fixture(&fixture, chromium_profile("chrome:Y2038", "Google Chrome"));
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 1);
    assert_eq!(summary.new_visits, 1);
    let archive = env.open_archive();
    let visit_time: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:Y2038'",
            [],
            |row| row.get(0),
        )
        .expect("query y2038 visit time");
    assert_eq!(visit_time, y2038_ms, "year-2038 timestamp must round-trip correctly");
}

/// E3 — Far-future timestamp (year 3000 ≈ 32_503_680_000_000 ms).
/// Clock skew or data corruption can produce far-future timestamps.
/// The archive must accept them without error.
#[test]
fn e3_far_future_timestamp_imports_without_error() {
    let env = ScenarioEnv::new();
    let far_future_ms = 32_503_680_000_000_i64; // ~3000-01-01
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/future".to_string(),
            title: Some("Far Future".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: far_future_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, far_future_ms));
    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:FarFuture", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 1);
    assert_eq!(summary.new_visits, 1);
    let archive = env.open_archive();
    let visit_time: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:FarFuture'",
            [],
            |row| row.get(0),
        )
        .expect("query far-future visit time");
    assert_eq!(visit_time, far_future_ms, "far-future timestamp must round-trip correctly");
}

/// E4 — Negative timestamp (before Unix epoch, e.g. 1969-12-31).
///
/// All browser parsers (Chromium, Firefox, Safari) clamp visit times to
/// `max(0)` when converting from browser-native format back to Unix ms.
/// A negative source timestamp therefore survives the fixture writer
/// (Chromium maps it to a valid Chrome-epoch microsecond value) but the
/// parser clamps the result to 0 on read-back. The archive must accept
/// the row without error; the stored `visit_time_ms` will be 0.
#[test]
fn e4_negative_timestamp_clamped_to_zero_without_error() {
    let env = ScenarioEnv::new();
    // -86_400_000 ms = 1969-12-31T00:00:00Z (one day before epoch).
    // The Chromium fixture writer converts this to a valid Chrome-epoch
    // microsecond (11_558_073_600_000_000), but the production parser's
    // `chrome_time_to_unix_ms` applies `.max(0)`, so it becomes 0.
    let negative_ms = -86_400_000_i64;
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/pre-epoch".to_string(),
            title: Some("Pre-Epoch".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: negative_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, negative_ms));
    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:PreEpoch", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 1);
    assert_eq!(summary.new_visits, 1);
    let archive = env.open_archive();
    let visit_time: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:PreEpoch'",
            [],
            |row| row.get(0),
        )
        .expect("query pre-epoch visit time");
    assert_eq!(visit_time, 0, "negative timestamp must be clamped to 0 by parser's max(0)");
}

// ======================================================================
// E7 — NULL title handling
// ======================================================================

/// E7 — Real Chrome `History` databases routinely have URLs with NULL
/// `title` columns (the user navigated to a URL but the page never
/// finished loading, or it was a binary download). The PathKeep parser
/// must tolerate this and produce a canonical URL row with `title =
/// NULL` rather than failing or storing an empty string. This pins the
/// contract that nullable source columns project as NULL in the archive.
#[test]
fn e7_null_title_imports_with_null_archive_title() {
    let env = ScenarioEnv::new();
    let day_one_ms = 1_777_680_000_000_i64;

    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/no-title".to_string(),
            title: None,
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/with-title".to_string(),
            title: Some("Has Title".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(1, 1, day_one_ms))
        .add_visit(chromium_visit_row(2, 2, day_one_ms));
    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:NullTitle", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 2);
    assert_eq!(summary.new_visits, 2);

    let archive = env.open_archive();
    let no_title: Option<String> = archive
        .query_row(
            "SELECT title FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:NullTitle'
               AND urls.source_url_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("query null-title url");
    assert!(
        no_title.is_none(),
        "NULL source title must project as NULL in archive, not empty string"
    );

    let with_title: Option<String> = archive
        .query_row(
            "SELECT title FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = 'chrome:NullTitle'
               AND urls.source_url_id = 2",
            [],
            |row| row.get(0),
        )
        .expect("query with-title url");
    assert_eq!(with_title.as_deref(), Some("Has Title"));
}

// ======================================================================
// E8 — Unicode in URLs and titles (CJK + emoji + IDN)
// ======================================================================

/// E8 — International users routinely have Unicode in browsing history:
/// CJK characters in titles, internationalized domain names (IDN /
/// Punycode), percent-encoded paths, and emoji. SQLite stores all of
/// these as UTF-8 TEXT natively, but the contract must be pinned:
/// every character must round-trip byte-identically through the parser,
/// the fingerprint hash, and the archive storage. If a future refactor
/// accidentally normalizes Unicode (NFC vs NFD, case folding, IDN
/// decoding) or truncates non-ASCII, this test fails immediately.
#[test]
fn e8_unicode_urls_and_titles_round_trip_byte_identical() {
    let env = ScenarioEnv::new();
    let day_one_ms = 1_777_680_000_000_i64;
    let day_two_ms = 1_777_809_600_000_i64;
    let day_three_ms = 1_777_872_930_000_i64;

    // Three diverse Unicode shapes that must NOT be normalized:
    // 1. CJK title (Traditional Chinese) on plain ASCII URL
    // 2. Percent-encoded path with mixed case (verbatim per E6)
    // 3. Emoji in title
    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/article".to_string(),
            title: Some("臺灣公開資料平臺 — 開放資料的全球趨勢".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/path/%E6%B8%AC%E8%A9%A6".to_string(),
            title: Some("Percent-Encoded Path".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_two_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 3,
            url: "https://example.com/celebration".to_string(),
            title: Some("Launch Day 🚀 — Ship It!".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_three_ms,
            hidden: false,
        })
        .add_visit(chromium_visit_row(10, 1, day_one_ms))
        .add_visit(chromium_visit_row(20, 2, day_two_ms))
        .add_visit(chromium_visit_row(30, 3, day_three_ms));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:Unicode", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 3);
    assert_eq!(summary.new_visits, 3);

    let archive = env.open_archive();
    let read_url_and_title = |source_url_id: i64| -> (String, Option<String>) {
        archive
            .query_row(
                "SELECT url, title FROM urls
                 JOIN source_profiles ON source_profiles.id = urls.source_profile_id
                 WHERE source_profiles.profile_key = 'chrome:Unicode'
                   AND urls.source_url_id = ?1",
                [source_url_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query unicode row")
    };

    let (url1, title1) = read_url_and_title(1);
    assert_eq!(url1, "https://example.com/article");
    assert_eq!(
        title1.as_deref(),
        Some("臺灣公開資料平臺 — 開放資料的全球趨勢"),
        "CJK title must round-trip byte-identical (no NFC/NFD normalization)"
    );

    let (url2, title2) = read_url_and_title(2);
    assert_eq!(
        url2, "https://example.com/path/%E6%B8%AC%E8%A9%A6",
        "percent-encoded path must NOT be decoded — stored verbatim"
    );
    assert_eq!(title2.as_deref(), Some("Percent-Encoded Path"));

    let (url3, title3) = read_url_and_title(3);
    assert_eq!(url3, "https://example.com/celebration");
    assert_eq!(
        title3.as_deref(),
        Some("Launch Day 🚀 — Ship It!"),
        "emoji + em-dash must round-trip verbatim"
    );
}

// ======================================================================
// E9 — `hidden = true` URL flag round-trip
// ======================================================================

/// E9 — Real Chrome `History` databases routinely store URLs with
/// `hidden = 1` (Chrome marks redirect intermediates, certain extension
/// URLs, and explicitly-hidden items this way). The PathKeep parser
/// must preserve this flag verbatim: `hidden = true` on the source URL
/// must produce `hidden != 0` on the canonical archive URL, and
/// `hidden = false` must produce `hidden = 0`.
///
/// This pins the `hidden` bit contract — sibling to E7 (NULL title)
/// and E8 (Unicode round-trip). Existing C-series tests only exercise
/// `hidden: false`; the C4 B1-fix test exercises `hidden: true` but
/// only in the context of preventing older-snapshot regressions. No
/// test had asserted that a first-time import of a `hidden = true` URL
/// actually preserves the flag.
#[test]
fn e9_hidden_url_flag_round_trips_for_both_true_and_false() {
    let env = ScenarioEnv::new();
    let day_one_ms = 1_777_680_000_000_i64;
    let day_two_ms = 1_777_809_600_000_i64;

    let fixture = ChromiumHistoryFixture::new()
        .add_url(ChromiumUrlRow {
            id: 1,
            url: "https://example.com/visible".to_string(),
            title: Some("Visible Page".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_one_ms,
            hidden: false,
        })
        .add_url(ChromiumUrlRow {
            id: 2,
            url: "https://example.com/hidden-redirect-intermediate".to_string(),
            title: Some("Hidden Redirect".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_unix_ms: day_two_ms,
            hidden: true,
        })
        .add_visit(chromium_visit_row(1, 1, day_one_ms))
        .add_visit(chromium_visit_row(2, 2, day_two_ms));

    let snapshot = snapshot_for_chromium_fixture(
        &fixture,
        chromium_profile("chrome:HiddenFlag", "Google Chrome"),
    );
    let summary = run_one_ingest(&env, 1, &snapshot, false);
    assert_eq!(summary.new_urls, 2);
    assert_eq!(summary.new_visits, 2);

    let archive = env.open_archive();
    let read_hidden = |source_url_id: i64| -> i64 {
        archive
            .query_row(
                "SELECT hidden FROM urls
                 JOIN source_profiles ON source_profiles.id = urls.source_profile_id
                 WHERE source_profiles.profile_key = 'chrome:HiddenFlag'
                   AND urls.source_url_id = ?1",
                [source_url_id],
                |row| row.get(0),
            )
            .expect("query hidden flag")
    };

    assert_eq!(read_hidden(1), 0, "hidden=false source must land as 0 in archive");
    assert!(
        read_hidden(2) != 0,
        "hidden=true source must land as non-zero in archive (not silently dropped)"
    );
}
