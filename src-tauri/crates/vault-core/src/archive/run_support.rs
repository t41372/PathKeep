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
//! - SQLite run-ledger tables in the canonical archive.
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
    utils::now_rfc3339,
};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use iana_time_zone::get_timezone;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Cheap visible-row counters cached for read models and run summaries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ArchiveVisibleTotals {
    pub(crate) total_profiles: usize,
    pub(crate) total_urls: usize,
    pub(crate) total_visits: usize,
    pub(crate) total_downloads: usize,
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

/// Parent-manifest pointer used to keep the manifest chain append-only.
#[derive(Debug, Clone)]
pub(crate) struct ManifestRow {
    pub(crate) id: i64,
    pub(crate) hash: String,
}

/// Persists one structured manifest payload and links it into the manifest chain.
///
/// Maintenance flows use this when they need manifest semantics identical to a
/// backup run but already have the final JSON payload assembled.
pub(crate) fn persist_structured_manifest(
    connection: &Connection,
    paths: &ProjectPaths,
    run_id: i64,
    finished_at: &str,
    payload: &Value,
) -> Result<(String, PathBuf)> {
    let manifest_json = serde_json::to_string_pretty(payload)?;
    let manifest_hash = crate::utils::sha256_hex(manifest_json.as_bytes());
    let row_counts = archive_row_counts(connection)?;
    let parent_manifest = latest_manifest_row(connection)?;
    let manifest_path =
        write_manifest_artifact(paths, run_id, finished_at, &manifest_hash, &manifest_json)?;
    persist_manifest_row(
        connection,
        run_id,
        parent_manifest.as_ref(),
        &manifest_hash,
        &manifest_path,
        finished_at,
        &row_counts,
    )?;
    Ok((manifest_hash, manifest_path))
}

/// Loads the most recent manifest row so the next run can chain to it.
pub(crate) fn latest_manifest_row(connection: &Connection) -> Result<Option<ManifestRow>> {
    connection
        .query_row(
            "SELECT id, content_hash
             FROM manifests
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| Ok(ManifestRow { id: row.get(0)?, hash: row.get(1)? }),
        )
        .optional()
        .map_err(Into::into)
}

/// Records one manifest row after its artifact has been written successfully.
///
/// Keeping the file write ahead of the SQL row avoids ledger entries that point
/// at audit artifacts which were never created.
pub(crate) fn persist_manifest_row(
    connection: &Connection,
    run_id: i64,
    parent: Option<&ManifestRow>,
    content_hash: &str,
    file_path: &Path,
    created_at: &str,
    row_counts: &Value,
) -> Result<()> {
    connection.execute(
        "INSERT INTO manifests (run_id, parent_manifest_id, content_hash, row_counts_json, created_at, file_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            run_id,
            parent.map(|manifest| manifest.id),
            content_hash,
            serde_json::to_string(row_counts)?,
            created_at,
            file_path.display().to_string(),
        ],
    )?;
    Ok(())
}

/// Marks a run successful and snapshots the visible archive totals alongside it.
///
/// The extra totals let Dashboard and Audit surfaces render recent activity
/// without immediately recomputing global counts from scratch.
pub(crate) fn finalize_successful_run(
    connection: &Connection,
    run_id: i64,
    finished_at: &str,
    summary: &BackupRunOverview,
    warnings: &[String],
    manifest_hash: &str,
) -> Result<()> {
    let stats = stats_with_archive_totals(
        connection,
        json!({
            "profilesProcessed": summary.profiles_processed,
            "newVisits": summary.new_visits,
            "newUrls": summary.new_urls,
            "newDownloads": summary.new_downloads,
            "manifestHash": manifest_hash,
        }),
    )?;
    connection.execute(
        "UPDATE runs
         SET finished_at = ?1,
             status = 'success',
             stats_json = ?2,
             warnings_json = ?3,
             error_message = NULL
         WHERE id = ?4",
        params![
            finished_at,
            serde_json::to_string(&stats)?,
            serde_json::to_string(warnings)?,
            run_id,
        ],
    )?;
    Ok(())
}

/// Marks a run failed without discarding the partial work summary gathered so far.
///
/// This keeps Audit truthful: the user can still see how far the job got
/// before the error aborted the flow.
pub(crate) fn finalize_failed_run(
    connection: &Connection,
    run_id: i64,
    profile_summaries: &[BackupProfileSummary],
    warnings: &[String],
    error: &anyhow::Error,
) -> Result<()> {
    connection.execute(
        "UPDATE runs
         SET finished_at = ?1,
             status = 'failed',
             stats_json = ?2,
             warnings_json = ?3,
             error_message = ?4
         WHERE id = ?5",
        params![
            now_rfc3339(),
            serde_json::to_string(&json!({
                "profilesProcessed": profile_summaries.len(),
                "newVisits": profile_summaries.iter().map(|item| item.new_visits).sum::<usize>(),
                "newUrls": profile_summaries.iter().map(|item| item.new_urls).sum::<usize>(),
                "newDownloads": profile_summaries.iter().map(|item| item.new_downloads).sum::<usize>(),
            }))?,
            serde_json::to_string(warnings)?,
            format!("{error:#}"),
            run_id,
        ],
    )?;
    Ok(())
}

/// Collapses per-profile summaries into the stable run-overview contract.
///
/// Backup and maintenance flows both use this helper so their Audit rows stay
/// structurally aligned even when their execution paths differ.
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
    }
}

/// Counts the row families surfaced in manifest metadata.
///
/// These counts deliberately exclude reverted visits/downloads because the
/// manifest is meant to describe the user-visible archive state.
pub(crate) fn archive_row_counts(connection: &Connection) -> Result<Value> {
    let urls: i64 = connection.query_row("SELECT COUNT(*) FROM urls", [], |row| row.get(0))?;
    let visits: i64 = connection.query_row(
        "SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let downloads: i64 = connection.query_row(
        "SELECT COUNT(*) FROM downloads WHERE reverted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let manifests: i64 =
        connection.query_row("SELECT COUNT(*) FROM manifests", [], |row| row.get(0))?;
    let snapshots: i64 =
        connection.query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))?;
    Ok(json!({
        "urls": urls,
        "visits": visits,
        "downloads": downloads,
        "manifests": manifests,
        "snapshots": snapshots,
    }))
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
    git_audit::ensure_repo(&paths.audit_repo_path)?;
    let relative_path =
        format!("manifests/{}/run-{}-{}.json", &finished_at[0..10], run_id, &manifest_hash[..12]);
    git_audit::write_audit_file(&paths.audit_repo_path, &relative_path, manifest_json)
}

/// Merges run-local counters with the current visible archive totals.
///
/// This helper avoids a second ad-hoc shape for stats JSON, which keeps Audit,
/// Dashboard, and takeout batch summaries aligned.
pub(crate) fn stats_with_archive_totals(connection: &Connection, stats: Value) -> Result<Value> {
    let totals = count_visible_archive_totals(connection)?;
    let mut object = match stats {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    object.insert("totalProfiles".to_string(), json!(totals.total_profiles));
    object.insert("totalUrls".to_string(), json!(totals.total_urls));
    object.insert("totalVisits".to_string(), json!(totals.total_visits));
    object.insert("totalDownloads".to_string(), json!(totals.total_downloads));
    Ok(Value::Object(object))
}

/// Counts visible archive totals without including reverted history rows.
///
/// Read models use this as a fallback when cached totals are missing or stale.
pub(crate) fn count_visible_archive_totals(
    connection: &Connection,
) -> Result<ArchiveVisibleTotals> {
    let total_profiles: i64 = connection.query_row(
        "SELECT COUNT(*) FROM source_profiles WHERE enabled = 1",
        [],
        |row| row.get(0),
    )?;
    let total_urls: i64 =
        connection.query_row("SELECT COUNT(*) FROM urls", [], |row| row.get(0))?;
    let total_visits: i64 = connection.query_row(
        "SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let total_downloads: i64 = connection.query_row(
        "SELECT COUNT(*) FROM downloads WHERE reverted_at IS NULL",
        [],
        |row| row.get(0),
    )?;

    Ok(ArchiveVisibleTotals {
        total_profiles: total_profiles.max(0) as usize,
        total_urls: total_urls.max(0) as usize,
        total_visits: total_visits.max(0) as usize,
        total_downloads: total_downloads.max(0) as usize,
    })
}

/// Resolves the current local timezone name for run metadata.
///
/// Falling back to `UTC` keeps manifests deterministic even on hosts whose
/// timezone lookup is unavailable.
pub(crate) fn current_timezone_name() -> String {
    get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// Explains why a due-only backup should be skipped right now.
///
/// Scheduled runs use this to stay honest: if the archive was backed up
/// recently enough, the scheduler records a skip instead of silently running
/// redundant work.
pub(super) fn backup_due_skip_reason(
    connection: &Connection,
    config: &AppConfig,
) -> Result<Option<String>> {
    Ok(latest_successful_backup_at(connection)?
        .and_then(|last_backup_at| backup_due_skip_reason_at(last_backup_at, config, Utc::now())))
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
    (elapsed < Duration::hours(config.due_after_hours as i64))
        .then(|| format!("last successful backup is only {} hours old", elapsed.num_hours()))
}

/// Loads the finish time of the most recent successful backup run.
///
/// This stays private because only the due-window logic should care about raw
/// timestamps; callers above this module only need the skip reason.
fn latest_successful_backup_at(connection: &Connection) -> Result<Option<DateTime<Utc>>> {
    let latest: Option<String> = connection
        .query_row(
            "SELECT finished_at
             FROM runs
             WHERE run_type = 'backup'
               AND status = 'success'
             ORDER BY id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(latest
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|value| value.with_timezone(&Utc)))
}
