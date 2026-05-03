//! Canonical archive domain boundary.
//!
//! ## Responsibilities
//! - Re-export the archive entrypoints used by `vault-core`, the worker, and
//!   Tauri commands.
//! - Keep shared SQL fragments and small cross-module helpers in one stable
//!   place while delegating heavier flows to focused submodules.
//! - Preserve the accepted archive contracts around run-ledger truth,
//!   recoverability, and immutable canonical facts.
//!
//! ## Not responsible for
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
mod backup;
mod doctor;
mod history;
mod ingest;
mod intelligence_projection;
mod maintenance;
mod read_models;
mod run_support;
mod schema;
mod search_lexical;
mod search_projection;
mod source_evidence;
mod source_evidence_builder;

pub(crate) use self::artifacts::{
    SnapshotArtifact, collect_schema_payload, create_snapshot_artifact,
    load_checkpoint_profile_snapshot, load_snapshot_record, record_snapshot_reference,
    serialize_payload,
};
pub use self::backup::{run_backup, run_backup_with_progress};
pub use self::intelligence_projection::open_intelligence_connection;
#[cfg(test)]
pub(crate) use self::intelligence_projection::{
    open_intelligence_connection_call_count, open_intelligence_connection_call_sites,
    reset_open_intelligence_connection_call_count,
};
use self::read_models::{decode_profile_scope, directory_size, file_size};
pub(crate) use self::run_support::{
    ArchiveVisibleTotals, BackupManifest, archive_row_counts, backup_run_summary,
    count_visible_archive_totals, current_timezone_name, finalize_failed_run,
    finalize_successful_run, latest_manifest_row, persist_manifest_row,
    persist_structured_manifest, stats_with_archive_totals, write_manifest_artifact,
};
pub(crate) use self::schema::apply_cipher_key;
pub(crate) use self::schema::export_archive_database;
pub use self::schema::{create_schema, open_archive_connection};
pub use self::schema::{current_version, run_migrations};
pub(crate) use self::search_projection::{
    rebuild_search_projection, refresh_search_projection_for_import_batch,
};
pub use self::source_evidence::open_source_evidence_connection;
pub(crate) use self::source_evidence::{
    DeferredSourceEvidencePayload, SourceBatchInput, SourceEvidencePayload,
    defer_source_evidence_payload, record_schema_observation, upsert_source_batch,
};
pub(crate) use self::source_evidence_builder::{
    DeferredSourceEvidenceBuilder, SourceEvidenceCounts, coverage_stats_json_from_counts,
    coverage_stats_json_from_parts,
};
pub use self::{
    doctor::{doctor, repair_health_issues},
    history::{export_history, list_history, load_history_favicons},
    maintenance::{
        preview_retention, preview_snapshot_restore, rekey_archive, run_retention_prune,
        run_snapshot_restore,
    },
    read_models::{
        archive_status, ensure_archive_initialized, load_audit_run_detail, load_dashboard_snapshot,
        load_recent_runs,
    },
};
use crate::{
    chrome::{FileFingerprint, ProfileSnapshot, discover_profiles},
    config::{ProjectPaths, ensure_paths, save_config},
    git_audit,
    models::{
        AppConfig, ArchiveMode, ArchiveStatus, AuditArtifact, AuditRunDetail, BackupProfileSummary,
        BackupReport, BackupRunOverview, DashboardSnapshot, ExportFormat, ExportRequest,
        ExportResult, HealthCheck, HealthRepairReport, HealthReport, HistoryEntry, HistoryFavicon,
        HistoryFaviconLookupEntry, HistoryFaviconLookupResult, HistoryQuery, HistoryQueryResponse,
        RetentionBucket, RetentionPreview, RetentionPruneRequest, RetentionPruneResult,
        SnapshotRestorePreview, SnapshotRestoreRequest, StorageSummary,
    },
    utils::{
        file_sha256_hex, filesystem_safe_path_segment, identifier_from_filesystem_segment,
        image_data_to_data_url, now_rfc3339, sha256_hex, unix_micros_to_chrome_time, url_domain,
    },
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::RegexBuilder;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, named_params, params};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration as StdDuration,
};
use tempfile::tempdir;

const LIST_HISTORY_SQL: &str = r#"
SELECT
  visits.id,
  source_profiles.profile_key,
  urls.url,
  urls.title,
  visits.visit_time_ms,
  visits.visit_duration_ms,
  visits.transition_type,
  visits.source_visit_id,
  visits.app_id
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
WHERE visits.reverted_at IS NULL
  AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
  AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
  AND (:query IS NULL OR urls.url LIKE '%' || :query || '%' OR IFNULL(urls.title, '') LIKE '%' || :query || '%')
  AND (:domainPattern IS NULL OR urls.url LIKE :domainPattern)
  AND (:startTimeMs IS NULL OR visits.visit_time_ms >= :startTimeMs)
  AND (:endTimeMs IS NULL OR visits.visit_time_ms <= :endTimeMs)
  AND (
    :cursorVisitTime IS NULL
    OR (
      :sort = 'oldest'
      AND (
        visits.visit_time_ms > :cursorVisitTime
        OR (visits.visit_time_ms = :cursorVisitTime AND visits.id > :cursorId)
      )
    )
    OR (
      :sort != 'oldest'
      AND (
        visits.visit_time_ms < :cursorVisitTime
        OR (visits.visit_time_ms = :cursorVisitTime AND visits.id < :cursorId)
      )
    )
  )
ORDER BY
  CASE WHEN :sort = 'oldest' THEN visits.visit_time_ms END ASC,
  CASE WHEN :sort = 'oldest' THEN visits.id END ASC,
  CASE WHEN :sort != 'oldest' THEN visits.visit_time_ms END DESC,
  CASE WHEN :sort != 'oldest' THEN visits.id END DESC
LIMIT :pageLimit
OFFSET :pageOffset
"#;

const COUNT_HISTORY_SQL: &str = r#"
SELECT COUNT(*)
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
WHERE visits.reverted_at IS NULL
  AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
  AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
  AND (:query IS NULL OR urls.url LIKE '%' || :query || '%' OR IFNULL(urls.title, '') LIKE '%' || :query || '%')
  AND (:domainPattern IS NULL OR urls.url LIKE :domainPattern)
  AND (:startTimeMs IS NULL OR visits.visit_time_ms >= :startTimeMs)
  AND (:endTimeMs IS NULL OR visits.visit_time_ms <= :endTimeMs)
"#;

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
  runs.stats_json
FROM runs
ORDER BY runs.id DESC
LIMIT 12
"#;

/// Builds the retention bucket for saved raw checkpoints and safety snapshots.
///
/// The preview prefers authoritative ledger counts when the archive is readable
/// and falls back to a filesystem walk when the archive cannot be opened.
fn retention_snapshot_bucket(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<RetentionBucket> {
    let item_count = if config.initialized {
        match open_archive_connection(paths, config, key) {
            Ok(connection) => connection
                .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get::<_, i64>(0))
                .unwrap_or_default()
                .max(0) as usize,
            Err(_) => count_path_entries(&paths.raw_snapshots_dir),
        }
    } else {
        count_path_entries(&paths.raw_snapshots_dir)
    };

    Ok(RetentionBucket {
        id: "snapshots".to_string(),
        bytes: directory_size(&paths.raw_snapshots_dir),
        item_count,
        paths: vec![paths.raw_snapshots_dir.display().to_string()],
    })
}

/// Builds one retention bucket backed directly by a local directory.
///
/// These buckets are rebuildable local artifacts, so the preview only needs
/// bytes, item counts, and the filesystem paths that will be pruned.
fn retention_directory_bucket(id: &str, path: &Path) -> RetentionBucket {
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
fn remove_directory_contents(path: &Path) -> Result<(u64, usize)> {
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

/// Prunes the snapshot bucket and clears its corresponding ledger rows.
///
/// This keeps retention prune truthful: once the files are gone, the snapshot
/// table should no longer advertise restore checkpoints that cannot exist.
fn prune_snapshot_bucket(connection: &Connection, paths: &ProjectPaths) -> Result<(u64, usize)> {
    let deleted = remove_directory_contents(&paths.raw_snapshots_dir)?;
    connection.execute("DELETE FROM snapshots", [])?;
    Ok(deleted)
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
