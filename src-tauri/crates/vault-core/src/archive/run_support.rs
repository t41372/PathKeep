//! Failure modes: totals recount facts, bounds include reverted rows, run metadata loses provenance.
//! Tests: read_model_tests and the backup suites.
//! Archive run-ledger and manifest helpers.
//!
//! ## Responsibilities
//! - Own the structs and helper functions that describe completed archive runs.
//! - Persist manifest metadata and visible row counts in a consistent format.
//! - Keep scheduler-facing due-window logic and archive-total snapshots in one
//!   shared location for backup, maintenance, and takeout flows.
//!
//! ## Not responsible for
//! - Parsing staged browser rows.
//! - Running backup/restore/rekey control flow.
//! - Mutating source-evidence batches or derived projections directly.
//!
//! ## Dependencies
//! - The session-owned Turso archive and transactionally maintained totals.
//! - `git_audit` for durable manifest artifact writes.
//! - `chrono` and `iana_time_zone` for scheduler-aware timestamps.
//!
//! ## Performance notes
//! - These helpers are called at the tail of long-running jobs. They keep
//!   their SQL narrowly scoped so finalization does not re-scan more archive
//!   state than needed.

use super::SnapshotArtifact;
use crate::{
    config::ProjectPaths,
    git_audit,
    models::{AppConfig, BackupProfileSummary, BackupRunOverview},
};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use iana_time_zone::get_timezone;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use vault_store::Session;

pub(super) const EARLIEST_VISIT_SQL: &str = "SELECT visit_time_iso FROM visits INDEXED BY idx_visits_visible_time_id WHERE reverted_at IS NULL AND visit_time_ms>=-9223372036854775808 ORDER BY visit_time_ms,id LIMIT 1";
pub(super) const LATEST_VISIT_SQL: &str = "SELECT visit_time_iso FROM visits INDEXED BY idx_visits_visible_time_id WHERE reverted_at IS NULL AND visit_time_ms>=-9223372036854775808 ORDER BY visit_time_ms DESC,id DESC LIMIT 1";

/// Visible-row counters + coverage bounds cached for read models and run
/// summaries. The bounds let the dashboard's "Span" stat avoid a per-render
/// MIN/MAX scan of the visits table on large archives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ArchiveVisibleTotals {
    pub(crate) total_profiles: usize,
    pub(crate) total_urls: usize,
    pub(crate) total_visits: usize,
    pub(crate) total_downloads: usize,
    pub(crate) earliest_visit_at: Option<String>,
    pub(crate) latest_visit_at: Option<String>,
}

/// Serialized manifest payload written to the audit repo after a successful run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupManifest {
    pub(crate) created_at: String,
    pub(crate) run_id: i64,
    pub(crate) timezone: String,
    pub(crate) due_only: bool,
    pub(crate) database_path: String,
    pub(crate) summary: BackupRunOverview,
    pub(crate) profiles: Vec<BackupProfileSummary>,
    pub(crate) warnings: Vec<String>,
    pub(crate) source_hashes: BTreeMap<String, BTreeMap<String, String>>,
    pub(crate) snapshots: Vec<SnapshotArtifact>,
    pub(crate) row_counts: Value,
    pub(crate) parent_manifest_hash: Option<String>,
}

pub(crate) fn backup_run_summary(
    run_type: &str,
    run_id: i64,
    started_at: &str,
    finished_at: &str,
    trigger: &str,
    profile_scope: &[String],
    profile_summaries: &[BackupProfileSummary],
) -> BackupRunOverview {
    BackupRunOverview {
        id: run_id,
        started_at: started_at.to_string(),
        finished_at: Some(finished_at.to_string()),
        status: "success".to_string(),
        run_type: run_type.to_string(),
        trigger: trigger.to_string(),
        profile_scope: profile_scope.to_vec(),
        manifest_hash: None,
        profiles_processed: profile_summaries.len(),
        new_visits: profile_summaries.iter().map(|item| item.new_visits).sum(),
        new_urls: profile_summaries.iter().map(|item| item.new_urls).sum(),
        new_downloads: profile_summaries.iter().map(|item| item.new_downloads).sum(),
        error_message: None,
    }
}

/// Writes the manifest JSON into the audit repo using a stable date-based path.
///
/// The path format is part of PathKeep's audit review ergonomics, so this
/// helper keeps that filename grammar in one place.
pub(crate) fn write_manifest_artifact(
    paths: &ProjectPaths,
    run_id: i64,
    finished_at: &str,
    manifest_hash: &str,
    manifest_json: &str,
) -> Result<PathBuf> {
    let relative_path =
        format!("manifests/{}/run-{}-{}.json", &finished_at[0..10], run_id, &manifest_hash[..12]);
    git_audit::write_audit_file(&paths.audit_repo_path, &relative_path, manifest_json)
}

/// Keeps render totals independent of archive size; bounds are two index seeks.
pub(crate) fn visible_totals(s: &Session<'_>) -> Result<ArchiveVisibleTotals> {
    let totals = super::totals::load_cached_archive_totals(s)?;
    Ok(ArchiveVisibleTotals {
        total_profiles: totals["sourceProfiles"] as usize,
        total_urls: totals["urls"] as usize,
        total_visits: totals["visits"] as usize,
        total_downloads: totals["downloads"] as usize,
        earliest_visit_at: s.query(EARLIEST_VISIT_SQL, (), |r| Ok(r.get::<String>(0)?))?.pop(),
        latest_visit_at: s.query(LATEST_VISIT_SQL, (), |r| Ok(r.get::<String>(0)?))?.pop(),
    })
}

/// Resolves the current local timezone name for run metadata.
///
/// Falling back to `UTC` keeps manifests deterministic even on hosts whose
/// timezone lookup is unavailable.
pub(crate) fn current_timezone_name() -> String {
    get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// Computes the concrete skip reason for one due-window comparison.
///
/// Tests and other helpers can call this with a controlled `now` value to keep
/// scheduler behavior deterministic.
pub(super) fn backup_due_skip_reason_at(
    last_backup_at: DateTime<Utc>,
    config: &AppConfig,
    now: DateTime<Utc>,
) -> Option<String> {
    let elapsed = now - last_backup_at;
    let due_after_minutes = due_after_minutes(config.due_after_hours);
    (elapsed < Duration::minutes(due_after_minutes as i64))
        .then(|| format!("last successful backup is only {} minutes old", elapsed.num_minutes()))
}

fn due_after_minutes(due_after_hours: f64) -> u64 {
    let minutes = (due_after_hours * 60.0).round();
    if minutes.is_finite() && minutes >= 1.0 && minutes <= u64::MAX as f64 {
        minutes as u64
    } else {
        1
    }
}
