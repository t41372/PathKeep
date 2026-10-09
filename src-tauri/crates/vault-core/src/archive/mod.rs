//! Failure modes: native entry points bypassed, legacy recovery contracts lost or shared archive state
//! diverges.
//!
//! Canonical archive domain boundary.
//!
//! Responsibilities:
//! - Re-export the archive entrypoints used by `vault-core`, the worker, and
//!   Tauri commands.
//! - Keep shared SQL fragments and small cross-module helpers in one stable
//!   place while delegating heavier flows to focused submodules.
//! - Preserve the accepted archive contracts around run-ledger truth,
//!   recoverability, and immutable canonical facts.
//!
//! Not responsible for:
//! - Browser discovery or live-file staging.
//! - Parser-specific row extraction logic.
//! - Frontend transport shaping or Tauri command orchestration.
//!
//! ## Dependencies
//! - `crate::chrome` for staged browser snapshots and discovery metadata.
//! - `browser_history_parser` for deterministic source parsing.
//! - SQLite archives for canonical facts, search recall, and source evidence.
//!
//! ## Performance notes
//! - This module sits on both recall-heavy read paths and long-running
//!   import/backup paths, so helpers here must avoid unnecessary full-memory
//!   copies and keep expensive work off the UI thread.

mod artifacts;
mod at_rest;
mod backup;
mod backup_manifest;
mod browser_schema;
mod checkpoint_restore;
mod doctor;
mod doctor_derived;
mod doctor_facts;
mod favicon_metadata;
pub use doctor_derived::derived_health;
mod history;
// Re-export the SSRF guard module so the W-ENRICH-1 enrichment plane can reach
// `crate::archive::history::net_guard::url_target_is_blocked` through the same chokepoint og:image
// fetching uses (history itself is a private module, so the path needs a crate-visible re-export).
pub(crate) use self::history::net_guard;
pub(crate) mod ingest;
mod read_models;
pub(crate) mod totals;
pub(crate) use self::read_models::recent_runs_from;
mod run_support;
pub(crate) use run_support::write_manifest_artifact;
pub(crate) mod search_documents;
pub(crate) mod write_runs;
pub use self::write_runs::append_run_warnings;
pub use search_documents::set_enrichment_text;
pub(crate) mod search_lexical;
mod search_opencc;
pub(crate) mod search_query;
pub(crate) mod source_evidence;
mod source_stats;
mod url_detail;
mod wipe;
mod write_lock;

pub(crate) use self::artifacts::SnapshotArtifact;
#[cfg(test)]
pub(crate) use self::at_rest::check_config_disk_consistency;
pub use self::at_rest::{
    ARCHIVE_RECOVERY_REQUIRED_PREFIX, ArchiveRecoveryKind, ArchiveRecoveryReport,
    DiskEncryptionMode, LaunchRecovery, ReconcileReport, detect_disk_encryption_mode,
    recover_archive_on_launch,
};
#[cfg(test)]
pub(crate) use self::backup::run_backup_profiles;
pub use self::backup::{run_backup, run_backup_with_control, run_backup_with_progress};
pub(crate) use self::history::cap_enrichment_excerpt;
use self::read_models::{decode_profile_scope, directory_size, file_size};
pub(crate) use self::run_support::current_timezone_name;
pub(crate) use self::search_documents::enrichment_text_for_index;
pub(crate) use self::source_evidence::{
    SourceBatchInput, SourceEvidencePayload, record_schema_observation, upsert_source_batch,
};
pub(crate) use browser_schema::collect_schema_payload;

pub use self::source_stats::load_source_stats;
pub use self::url_detail::{get_url_detail, is_visited_url};
pub use self::wipe::{
    WipeOutsideState, data_wipe_interrupted, finish_interrupted_data_wipe, preview_data_wipe,
    request_running_jobs_stop, wipe_all_data,
};
// The cross-process archive write lock and the in-process gate that serializes top-level
// destructive operations in one process (the flock is reentrant, so it can't). Public so the
// converter's launch recovery for old archives takes the same pair.
pub use self::checkpoint_restore::{preview_snapshot_restore, run_snapshot_restore};
pub use self::write_lock::{ArchiveOpGate, ArchiveWriteLock};
pub use self::{
    doctor::{doctor, repair_health_issues},
    history::{
        BrowseDayInsights, BrowseDayInsightsRequest, BrowseDaySearchQuery, BrowseDayTopDomain,
        BrowseDayTopUrl, ExportCancelled, cancel_export, export_history, export_search_history,
        get_browse_day_insights, get_export_progress, list_history, load_history_favicons,
        load_site_favicons, og_images, og_images_fetch, record_export_failure, search_history,
    },
    read_models::{
        RunPage, archive_availability, archive_status, load_audit_run_detail,
        load_dashboard_snapshot, load_recent_runs,
    },
};
use crate::{
    chrome::{FileFingerprint, ProfileSnapshot},
    config::ProjectPaths,
    git_audit,
    models::{
        AppConfig, ArchiveMode, ArchiveStatus, AuditArtifact, AuditRunDetail, BackupProfileSummary,
        BackupReport, BackupRunOverview, DashboardSnapshot, HealthCheck, HealthRepairReport,
        HealthReport, HistoryQuery, HistoryQueryResponse, RetentionBucket, SnapshotRestorePreview,
        SnapshotRestoreRequest, StorageSummary, backup_warning_messages,
    },
    utils::{
        file_sha256_hex, filesystem_safe_path_segment, identifier_from_filesystem_segment,
        now_rfc3339, sha256_hex, unix_micros_to_chrome_time,
    },
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use tempfile::tempdir;

/// The History list query in one sort direction.
///
/// Time and cursor bounds are plain comparisons, never `:x IS NULL OR ...`, so SQLite can seek
/// `idx_visits_visible_time_id` (migration 016) and stop after one page instead of sorting every
/// visible visit. Callers bind sentinels for "no bound": see [`list_history_bounds`]. The two
/// directions share one body so the filters cannot drift apart.
const RECENT_RUNS_SQL: &str = r#"
SELECT
  runs.id,
  runs.started_at,
  runs.finished_at,
  runs.status,
  runs.run_type,
  runs.trigger,
  runs.profile_scope_json,
  (
    SELECT manifests.content_hash
    FROM manifests
    WHERE manifests.run_id = runs.id
    ORDER BY manifests.id DESC
    LIMIT 1
  ) AS manifest_hash,
  runs.stats_json,
  runs.error_message
FROM runs
-- The 002 migration seeds a synthetic id=0/run_type='system'/trigger='compat'
-- run so legacy foreign keys can resolve. It has a 1970-01-01 timestamp and
-- never represents real backup activity, so the audit ledger and dashboard
-- must not surface it as if a user triggered it.
WHERE NOT (runs.id = 0 AND runs.run_type = 'system' AND runs.trigger = 'compat')
ORDER BY runs.id DESC
LIMIT 12
"#;

/// Builds one retention bucket backed directly by a local directory.
///
/// These buckets are rebuildable local artifacts, so the preview only needs
/// bytes, item counts, and the filesystem paths that will be pruned.
pub(crate) fn retention_directory_bucket(id: &str, path: &Path) -> RetentionBucket {
    RetentionBucket {
        id: id.to_string(),
        bytes: directory_size(path),
        item_count: count_path_entries(path),
        paths: vec![path.display().to_string()],
    }
}

/// Counts files under one retention path using the same recursive semantics as prune.
///
/// The retention preview uses this to stay honest about how many artifacts a
/// prune operation would actually remove from nested directories.
fn count_path_entries(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let child_path = entry.path();
            if child_path.is_dir() { 1 + count_path_entries(&child_path) } else { 1 }
        })
        .sum()
}

/// Deletes all children of a retention directory while leaving the root in place.
///
/// The prune flow keeps bucket roots intact so later exports or staging work do
/// not have to recreate the whole directory layout from scratch.
pub(crate) fn remove_directory_contents(path: &Path) -> Result<(u64, usize)> {
    if !path.exists() {
        return Ok((0, 0));
    }
    let mut deleted_bytes = 0u64;
    let mut deleted_files = 0usize;
    for entry in fs::read_dir(path)?.flatten() {
        let (bytes, files) = remove_path(&entry.path())?;
        deleted_bytes += bytes;
        deleted_files += files;
    }
    fs::create_dir_all(path)?;
    Ok((deleted_bytes, deleted_files))
}

/// Removes one file-or-directory path and returns the deleted byte/file totals.
///
/// This recursive helper is shared by retention buckets so all prune results
/// count nested files the same way.
fn remove_path(path: &Path) -> Result<(u64, usize)> {
    if !path.exists() {
        return Ok((0, 0));
    }
    if path.is_file() {
        let bytes = file_size(path);
        fs::remove_file(path)?;
        return Ok((bytes, 1));
    }

    let mut deleted_bytes = 0u64;
    let mut deleted_files = 0usize;
    for entry in fs::read_dir(path)?.flatten() {
        let (bytes, files) = remove_path(&entry.path())?;
        deleted_bytes += bytes;
        deleted_files += files;
    }
    fs::remove_dir_all(path)?;
    Ok((deleted_bytes, deleted_files))
}

/// Deletes every child under `root` EXCEPT the `keep` file (and the directories on its path), then
/// recreates `root`, returning the deleted byte/file totals.
///
/// Backs the D3 retention last-good guard: the kept path comes from [`list_recovery_snapshots`],
/// which builds paths from `read_dir` of the SAME `root`, so an `==` comparison matches the entries
/// here without canonicalize. `keep == None` reproduces the original delete-everything behaviour.
pub(crate) fn remove_directory_contents_except(
    root: &Path,
    keep: Option<&Path>,
) -> Result<(u64, usize)> {
    if !root.exists() {
        return Ok((0, 0));
    }
    let mut deleted_bytes = 0u64;
    let mut deleted_files = 0usize;
    for entry in fs::read_dir(root)?.flatten() {
        let (bytes, files) = remove_path_except(&entry.path(), keep)?;
        deleted_bytes += bytes;
        deleted_files += files;
    }
    fs::create_dir_all(root)?;
    Ok((deleted_bytes, deleted_files))
}

/// Recursive helper for [`remove_directory_contents_except`]: removes `path` (counting it) unless it
/// IS the kept file or a directory on the kept file's path (which is recursed into and preserved).
fn remove_path_except(path: &Path, keep: Option<&Path>) -> Result<(u64, usize)> {
    if !path.exists() {
        return Ok((0, 0));
    }
    // The protected file itself — never delete it.
    if keep == Some(path) {
        return Ok((0, 0));
    }
    // A directory that CONTAINS the kept file is recursed into and left in place; everything else
    // (plain files, and subtrees with nothing protected) is removed wholesale via `remove_path`.
    if path.is_dir() && keep.is_some_and(|kept| kept.starts_with(path)) {
        let mut deleted_bytes = 0u64;
        let mut deleted_files = 0usize;
        for entry in fs::read_dir(path)?.flatten() {
            let (bytes, files) = remove_path_except(&entry.path(), keep)?;
            deleted_bytes += bytes;
            deleted_files += files;
        }
        return Ok((deleted_bytes, deleted_files));
    }
    remove_path(path)
}

/// Computes the stable fingerprint used to deduplicate canonical visit events.
pub(crate) fn visit_event_fingerprint(
    source_kind: &str,
    url: &str,
    visit_time: i64,
    title: Option<&str>,
    transition: Option<i64>,
    app_id: Option<&str>,
) -> String {
    let payload = json!({
        "sourceKind": source_kind,
        "url": url,
        "visitTime": visit_time,
        "title": title.unwrap_or_default(),
        "transition": transition,
        "appId": app_id.unwrap_or_default(),
    });
    sha256_hex(payload.to_string().as_bytes())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use source_stats::tests::seeded_archive as seed_read_archive;
