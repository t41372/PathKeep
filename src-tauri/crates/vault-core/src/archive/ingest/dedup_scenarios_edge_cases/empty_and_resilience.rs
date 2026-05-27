//! Empty-source and malformed-source ingest contract tests.
//!
//! ## Responsibilities
//! - Verify empty browser databases import as successful zero-work runs.
//! - Verify corrupt or malformed source databases return errors instead of panics.
//!
//! ## Not responsible for
//! - Transaction rollback or partial-write crash recovery tests.
//! - Parser schema self-validation tests from the fixture crate.
//!
//! ## Dependencies
//! - Uses shared edge-case fixture helpers from the parent module.
//! - Uses temporary files to create corrupt and missing-table source databases.
//!
//! ## Performance notes
//! - Exercises only minimal files and zero-row fixtures.

use super::*;

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
