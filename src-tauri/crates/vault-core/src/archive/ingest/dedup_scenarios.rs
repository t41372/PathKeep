//! Responsibilities: verify parser-to-archive dedup contracts on real Turso.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: duplicate visits, changed URL metadata, cursor loss, timestamp
//! truncation, lost optional fields or source evidence. Scenarios below pin each.

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
use tempfile::tempdir;

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

fn seed_run(archive: &crate::test_support::store::Probe, run_id: i64) {
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
    vault: crate::Vault,
}

impl ScenarioEnv {
    fn new(key: Option<[u8; 32]>) -> Self {
        let vault = crate::Vault::open_temp_with_key(key).expect("temporary vault");
        let paths = vault.paths().clone();
        crate::config::ensure_paths(&paths).expect("ensure paths");
        Self { vault }
    }

    fn open_archive(&self) -> crate::test_support::store::Probe {
        crate::test_support::store::Probe::new(&self.vault)
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
    seed_run(&env.open_archive(), run_id);
    process_profile_snapshot(
        &env.vault,
        run_id,
        snapshot,
        false,
        use_watermark,
        &vault_store::CancelToken::new(),
        None,
    )
    .expect("process profile snapshot")
}

fn count_archive_rows(env: &ScenarioEnv, table: &str) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), (), |row| Ok(row.get(0)?))
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
            |row| Ok(row.get(0)?),
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
            |row| Ok(row.get(0)?),
        )
        .expect("count visits for profile")
}

fn collect_visit_source_ids(env: &ScenarioEnv, profile_key: &str) -> Vec<String> {
    let archive = env.open_archive();
    archive
        .query(
            "SELECT visits.source_visit_id FROM visits
             JOIN source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE source_profiles.profile_key = ?1
             ORDER BY visits.source_visit_id ASC",
            [profile_key],
            |row| Ok(row.get::<String>(0)?),
        )
        .expect("query rows")
}

/// Reads the saved watermark row for a profile_id directly. Returns
/// `None` if no row exists yet. Used by watermark-isolation and
/// incremental-import scenarios that need to prove the parser's cursor
/// actually advanced (the row-count assertions alone cannot — the
/// canonical-layer dedup masks any watermark regression).
fn read_profile_watermark(env: &ScenarioEnv, profile_id: &str) -> Option<i64> {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT last_visit_id FROM profile_watermarks WHERE profile_id = ?1",
            [profile_id],
            |row| Ok(row.get::<i64>(0)?),
        )
        .ok()
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

// ----------------------------------------------------------------------
// C2: Chromium incremental no-new-data — watermark prevents re-import
// ----------------------------------------------------------------------

// ----------------------------------------------------------------------
// C3: Chromium incremental revisit of an old URL
// ----------------------------------------------------------------------

// ----------------------------------------------------------------------
// X1: Edge imports Chrome history, then both diverge
// ----------------------------------------------------------------------

// T1, T2, T2b moved to dedup_scenarios_takeout.rs.

// ----------------------------------------------------------------------
// X2: Chromium-family product identity for Atlas and Comet
// ----------------------------------------------------------------------

// ----------------------------------------------------------------------
// C5: Chromium incremental growth — pure append-new-rows
// ----------------------------------------------------------------------

// ----------------------------------------------------------------------
// X3: Multi-profile per browser — Chrome Default vs Chrome Profile 1
// ----------------------------------------------------------------------

// ----------------------------------------------------------------------
// C6: Chromium source DB schema tolerance — extra columns must not break ingest
// ----------------------------------------------------------------------

// ----------------------------------------------------------------------
// C7: Tied last_visit_ms must NOT overwrite title / hidden / payload_hash
// ----------------------------------------------------------------------

// ----------------------------------------------------------------------
// C4: URL upsert must not regress metadata on re-import (B1 — FIXED)
// ----------------------------------------------------------------------

fn stored_visit_count(env: &ScenarioEnv, profile_key: &str, source_url_id: i64) -> i64 {
    let archive = env.open_archive();
    archive
        .query_row(
            "SELECT visit_count FROM urls
             JOIN source_profiles ON source_profiles.id = urls.source_profile_id
             WHERE source_profiles.profile_key = ?1 AND urls.source_url_id = ?2",
            vault_store::params![profile_key, source_url_id],
            |row| Ok(row.get(0)?),
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
            vault_store::params![profile_key, source_url_id],
            |row| Ok(row.get(0)?),
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
            vault_store::params![profile_key, source_url_id],
            |row| Ok(row.get(0)?),
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
            vault_store::params![profile_key, source_url_id],
            |row| Ok(row.get(0)?),
        )
        .expect("query hidden");
    hidden_int != 0
}

// F2, S2 moved to dedup_scenarios_baselines.rs.
// T3, T5 moved to dedup_scenarios_takeout.rs.
// C_SUB_MS implemented in dedup_scenarios_edge_cases.rs.

#[path = "dedup_scenarios/cases_1.rs"]
mod cases_1;

#[path = "dedup_scenarios/cases_2.rs"]
mod cases_2;
