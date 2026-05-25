//! Baseline import scenarios for Firefox, Safari, and Chromium fingerprint dedup.
//!
//! These scenarios complement `dedup_scenarios.rs` by covering:
//! - **F1**: Firefox single-import baseline — asserts all URLs and visits
//!   land correctly from a Firefox Places fixture.
//! - **S1**: Safari single-import baseline — asserts all URLs and visits
//!   land correctly from a Safari History fixture.
//! - **Chromium fingerprint dedup**: Re-importing the same visits with
//!   different `source_visit_id` values must not create duplicates because
//!   the `event_fingerprint` partial index catches them.
//!
//! Each scenario reuses the `ScenarioEnv`, `run_one_ingest`, `count_*`
//! helpers from `dedup_scenarios.rs` and the snapshot builders for Firefox
//! and Safari already defined there.

use super::*;
use browser_history_fixtures::{
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, FirefoxPlaceRow,
    FirefoxPlacesFixture, FirefoxVisitRow, SafariHistoryFixture, SafariHistoryItemRow,
    SafariHistoryVisitRow,
};
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

// ======================================================================
// F1: Firefox baseline import — happy path
// ======================================================================

/// F1 — One Firefox profile, one ingest pass. Asserts every fixture row
/// lands in the canonical archive with correct URL count, visit count,
/// timestamps, and field values matching fixture input. This is the
/// Firefox analog of C1 (Chromium baseline).
#[test]
fn f1_firefox_baseline_import() {
    let env = ScenarioEnv::new();

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
            [],
            |row| row.get(0),
        )
        .expect("query first visit time");
    assert_eq!(first_visit_ms, t1, "first visit timestamp must match fixture");

    let last_visit_ms: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'firefox:Default'
               AND visits.source_visit_id = '14'",
            [],
            |row| row.get(0),
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
            [],
            |row| row.get(0),
        )
        .expect("query url title");
    assert_eq!(title.as_deref(), Some("Firefox Article One"));
}

// ======================================================================
// S1: Safari baseline import — happy path
// ======================================================================

/// S1 — One Safari profile, one ingest pass. Asserts every fixture row
/// lands in the canonical archive with correct URL count, visit count,
/// timestamps, and field values matching fixture input. This is the
/// Safari analog of C1 (Chromium baseline).
#[test]
fn s1_safari_baseline_import() {
    let env = ScenarioEnv::new();

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
            [],
            |row| row.get(0),
        )
        .expect("query first visit time");
    assert_eq!(first_visit_ms, t1, "first visit timestamp must match fixture");

    let last_visit_ms: i64 = archive
        .query_row(
            "SELECT visit_time_ms FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = 'safari:Default'
               AND visits.source_visit_id = '14'",
            [],
            |row| row.get(0),
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
            [],
            |row| row.get(0),
        )
        .expect("query url title");
    assert!(title.is_some(), "Safari URL title should be populated from visit title");
}

// ======================================================================
// Chromium fingerprint dedup — same visits, different source_visit_ids
// ======================================================================

/// Chromium fingerprint dedup — Imports a Chromium fixture, then
/// re-imports the exact same visits but with DIFFERENT `source_visit_id`
/// values (simulating a database rebuild or ID reassignment). The
/// `(source_profile_id, event_fingerprint)` partial unique index must
/// catch these as duplicates. No duplicate visit rows should be created.
#[test]
fn chromium_fingerprint_dedup_catches_same_visits_with_different_source_ids() {
    let env = ScenarioEnv::new();

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
