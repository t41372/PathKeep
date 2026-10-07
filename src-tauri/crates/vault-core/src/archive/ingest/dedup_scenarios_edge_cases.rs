//! Responsibilities: verify parser-to-archive dedup contracts on real Turso.
//! Not responsible for: browser-owned SQLite engine behavior.
//! Failure modes: duplicate visits, changed URL metadata, cursor loss, timestamp
//! truncation, lost optional fields or source evidence. Scenarios below pin each.

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
//! - **E10-E14**: Minor data-integrity pins for visit counts, referential
//!   fields, duration values, Safari context evidence, and Firefox visit types.

use super::*;
use browser_history_fixtures::{
    ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow, FirefoxPlaceRow,
    FirefoxPlacesFixture, FirefoxVisitRow, SafariHistoryFixture, SafariHistoryItemRow,
    SafariHistoryVisitRow,
};
use std::io::Write;
use tempfile::tempdir;

// ── Shared helpers (mirror dedup_scenarios.rs patterns) ─────────────

/// Holds the long-lived resources one scenario needs across multiple
/// imports (same as dedup_scenarios::ScenarioEnv).
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

fn seed_run(archive: &crate::test_support::store::Probe, run_id: i64) {
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

fn run_one_ingest_with_persisted_source_evidence(
    env: &ScenarioEnv,
    run_id: i64,
    snapshot: &ProfileSnapshot,
) -> BackupProfileSummary {
    seed_run(&env.open_archive(), run_id);
    process_profile_snapshot(
        &env.vault,
        run_id,
        snapshot,
        false,
        false,
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

// Test bodies live in focused child modules so this file stays a small
// shared fixture harness instead of another oversized scenario owner.
mod chromium_contracts;
mod empty_and_resilience;
mod minor_data_integrity;
mod time_and_nullable;
mod unicode_and_flags;
