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
//! Why: shared identity helpers keep archive keys stable across dependency feature changes.
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
pub(crate) mod source_error;
pub use doctor_derived::derived_health;
mod history;
// Re-export the SSRF guard module so the W-ENRICH-1 enrichment plane can reach
// `crate::archive::history::net_guard::url_target_is_blocked` through the same chokepoint og:image
// fetching uses (history itself is a private module, so the path needs a crate-visible re-export).
pub(crate) use self::history::net_guard;
pub(crate) mod ingest;
mod read_models;
pub(crate) mod totals;
pub use self::read_models::browsing_archive_bytes;
pub(crate) use self::read_models::recent_runs_from;
mod run_support;
mod schedule;
pub use schedule::{backup_due_anchor, next_scheduled_backup_at};
#[cfg(test)]
mod schedule_estimate_tests;
pub(crate) use run_support::write_manifest_artifact;
pub(crate) mod search_documents;
pub(crate) mod write_runs;
pub use self::write_runs::append_run_warnings;
pub use search_documents::set_enrichment_text;
pub(crate) mod same_visit;
pub(crate) mod search_lexical;
mod search_opencc;
pub(crate) mod search_query;
pub mod sites;
pub(crate) mod source_evidence;
mod source_stats;
mod url_detail;
pub(crate) mod wipe;
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
use self::read_models::{decode_profile_scope, file_size};
pub(crate) use self::run_support::current_timezone_name;
pub(crate) use self::search_documents::enrichment_text_for_index;
pub(crate) use self::source_evidence::{
    SourceBatchInput, SourceEvidencePayload, record_schema_observation, upsert_source_batch,
};
pub(crate) use browser_schema::collect_schema_payload;

pub use self::source_stats::load_source_stats;
pub use self::url_detail::{get_url_detail, is_visited_url};
pub use self::wipe::{
    SettledWipe, WipeOutsideState, data_wipe_interrupted, finish_interrupted_data_wipe,
    preview_data_wipe, request_running_jobs_stop, wipe_all_data,
};
// The cross-process archive write lock and the in-process gate that serializes top-level
// destructive operations in one process (the flock is reentrant, so it can't). Public so the
// converter's launch recovery for old archives takes the same pair.
pub use self::checkpoint_restore::{preview_snapshot_restore, run_snapshot_restore};
pub use self::write_lock::{ArchiveOpGate, ArchiveWriteLock};
pub use self::{
    doctor::{doctor, repair_health_issues},
    history::{
        ExportCancelled, cancel_export, export_history, get_export_progress, list_history,
        load_history_favicons, load_site_favicons, og_images, og_images_fetch,
        record_export_failure, search_history, suggested_export_name,
    },
    read_models::{
        McpQueryPage, McpQueryRecord, RunPage, archive_availability, archive_status,
        last_finished_backup, load_audit_run_detail, load_dashboard_snapshot, load_mcp_queries,
        load_recent_runs,
    },
};
use crate::{
    chrome::{FileFingerprint, ProfileSnapshot},
    config::ProjectPaths,
    git_audit,
    models::{
        AppConfig, ArchiveMode, ArchiveStatus, AuditArtifact, AuditRunDetail, BackupProfileSummary,
        BackupReport, BackupRunOverview, DashboardSnapshot, HealthCheck, HealthRepairReport,
        HealthReport, HistoryQuery, HistoryQueryResponse, SnapshotRestorePreview,
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

/// The archive's runs, newest first, for Backup's "Recent runs" and its audit sheet: backups,
/// imports and their undo, restores, prunes, repairs, encryption changes. The AI's runs (an index
/// build, an answer, each MCP query) are kept in the same table but are not archive operations;
/// listed here they pushed the backups out of the twelve rows, under a backup's summary.
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
  AND runs.run_type NOT IN ('ai_index', 'assistant', 'mcp_query')
ORDER BY runs.id DESC
LIMIT 12
"#;

/// Computes the stable fingerprint used to deduplicate canonical visit events.
pub(crate) fn visit_event_fingerprint(
    source_kind: &str,
    url: &str,
    visit_time: i64,
    title: Option<&str>,
    transition: Option<i64>,
    app_id: Option<&str>,
) -> String {
    let mut payload = json!({
        "sourceKind": source_kind,
        "url": url,
        "visitTime": visit_time,
        "title": title.unwrap_or_default(),
        "transition": transition,
        "appId": app_id.unwrap_or_default(),
    });
    browser_history_parser::canonicalize_json(&mut payload);
    sha256_hex(payload.to_string().as_bytes())
}

#[cfg(test)]
mod tests;
