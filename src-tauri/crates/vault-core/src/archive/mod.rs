//! Canonical archive domain.
//!
//! This module owns the source-of-truth archive behavior for PathKeep:
//! initialization, backup ingest, recall/export, recoverability, and doctor
//! repair. The accepted archive contracts from the docs matter here:
//!
//! - `runs` is the unified ledger for backup/import/rollback/doctor/restore work
//! - rollback hides user-visible facts without deleting immutable audit facts
//! - preview/manual/execute flows must stay explainable and recoverable
//! - derived intelligence can be rebuilt, but canonical facts cannot be faked

mod doctor;
mod history;
mod ingest;
mod intelligence_projection;
mod maintenance;
mod read_models;
mod schema;
mod search_projection;
mod source_evidence;

use self::ingest::{
    collect_skipped_profiles, persist_source_evidence_plans, process_profile_snapshot,
    select_supported_profiles, snapshot_source_hashes,
};
pub use self::intelligence_projection::open_intelligence_connection;
#[cfg(test)]
pub(crate) use self::intelligence_projection::{
    open_intelligence_connection_call_count, open_intelligence_connection_call_sites,
    reset_open_intelligence_connection_call_count,
};
use self::read_models::{decode_profile_scope, directory_size, file_size};
pub(crate) use self::schema::apply_cipher_key;
pub(crate) use self::schema::export_archive_database;
pub use self::schema::{create_schema, open_archive_connection};
pub use self::schema::{current_version, run_migrations};
pub(crate) use self::search_projection::rebuild_search_projection;
pub use self::source_evidence::open_source_evidence_connection;
pub(crate) use self::source_evidence::{
    SourceBatchInput, coverage_stats_json, persist_source_evidence, record_schema_observation,
    upsert_source_batch,
};
pub use self::{
    doctor::{doctor, repair_health_issues},
    history::{export_history, list_history},
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
    chrome::{FileFingerprint, ProfileSnapshot, discover_profiles, stage_profile_snapshot},
    config::{ProjectPaths, ensure_paths, save_config},
    git_audit,
    models::{
        AppConfig, ArchiveMode, ArchiveStatus, AuditArtifact, AuditRunDetail, BackupProfileSummary,
        BackupProgressEvent, BackupReport, BackupRunOverview, DashboardSnapshot, ExportFormat,
        ExportRequest, ExportResult, HealthCheck, HealthRepairReport, HealthReport, HistoryEntry,
        HistoryFavicon, HistoryQuery, HistoryQueryResponse, RetentionBucket, RetentionPreview,
        RetentionPruneRequest, RetentionPruneResult, SnapshotRestorePreview,
        SnapshotRestoreRequest, StorageSummary,
    },
    utils::{
        file_sha256_hex, image_data_to_data_url, now_rfc3339, sha256_hex,
        unix_micros_to_chrome_time, url_domain,
    },
};
use anyhow::{Context, Result};
use browser_history_parser::ParsedHistory;
use chrono::{DateTime, Duration, Utc};
use iana_time_zone::get_timezone;
use regex::RegexBuilder;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, named_params, params};
use serde::Serialize;
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
  visits.app_id,
  (
    SELECT favicons.image_data
    FROM favicons
    WHERE favicons.source_profile_id = source_profiles.id
      AND favicons.page_url = urls.url
      AND favicons.image_data IS NOT NULL
    ORDER BY
      favicons.last_updated_ms DESC,
      favicons.width DESC,
      favicons.height DESC,
      favicons.id DESC
    LIMIT 1
  ) AS favicon_image_data
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

const LIST_HISTORY_FTS_SQL: &str = r#"
SELECT
  visits.id,
  source_profiles.profile_key,
  urls.url,
  urls.title,
  visits.visit_time_ms,
  visits.visit_duration_ms,
  visits.transition_type,
  visits.source_visit_id,
  visits.app_id,
  (
    SELECT favicons.image_data
    FROM favicons
    WHERE favicons.source_profile_id = source_profiles.id
      AND favicons.page_url = urls.url
      AND favicons.image_data IS NOT NULL
    ORDER BY
      favicons.last_updated_ms DESC,
      favicons.width DESC,
      favicons.height DESC,
      favicons.id DESC
    LIMIT 1
  ) AS favicon_image_data
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
JOIN search.history_search AS history_search
  ON history_search.rowid = urls.id
WHERE visits.reverted_at IS NULL
  AND history_search MATCH :ftsQuery
  AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
  AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
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
"#;

const COUNT_HISTORY_FTS_SQL: &str = r#"
SELECT COUNT(*)
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
JOIN search.history_search AS history_search
  ON history_search.rowid = urls.id
WHERE visits.reverted_at IS NULL
  AND history_search MATCH :ftsQuery
  AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
  AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Cheap visible-row counters cached for read models and backup summaries.
pub(crate) struct ArchiveVisibleTotals {
    pub total_profiles: usize,
    pub total_urls: usize,
    pub total_visits: usize,
    pub total_downloads: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotArtifact {
    kind: String,
    path: String,
    checksum: String,
    reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    created_at: String,
    run_id: i64,
    timezone: String,
    due_only: bool,
    database_path: String,
    summary: BackupRunOverview,
    profiles: Vec<BackupProfileSummary>,
    warnings: Vec<String>,
    source_hashes: BTreeMap<String, BTreeMap<String, String>>,
    snapshots: Vec<SnapshotArtifact>,
    row_counts: Value,
    parent_manifest_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct ManifestRow {
    id: i64,
    hash: String,
}

#[derive(Debug, Clone)]
struct SnapshotRecord {
    run_id: i64,
    profile_scope: Vec<String>,
    file_path: String,
    created_at: String,
    reason: Option<String>,
}

#[derive(Debug)]
struct SerializedPayload {
    hash: String,
}

/// Runs one backup using a no-op progress callback.
pub fn run_backup(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    due_only: bool,
) -> Result<BackupReport> {
    run_backup_with_progress(paths, config, key, due_only, |_| {})
}

/// Runs one canonical backup and emits progress events through the supplied callback.
pub fn run_backup_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    due_only: bool,
    mut report_progress: F,
) -> Result<BackupReport>
where
    F: FnMut(BackupProgressEvent),
{
    ensure_paths(paths)?;
    if !config.initialized {
        anyhow::bail!("archive has not been initialized");
    }

    let mut connection = open_archive_connection(paths, config, key)?;
    let mut source_evidence = open_source_evidence_connection(paths, config, key)?;

    if due_only && let Some(reason) = backup_due_skip_reason(&connection, config)? {
        return Ok(BackupReport {
            due_skipped: true,
            reason: Some(reason),
            ..BackupReport::default()
        });
    }

    let discovered = discover_profiles()?;
    if config.selected_profile_ids.is_empty() {
        anyhow::bail!("select at least one readable browser profile before running a backup")
    }
    let selected_profiles = select_supported_profiles(&discovered, &config.selected_profile_ids);
    if selected_profiles.is_empty() {
        anyhow::bail!(
            "the selected profiles are not readable yet; choose at least one detected profile with a readable history database"
        )
    }
    let skipped_profiles = collect_skipped_profiles(&discovered, &config.selected_profile_ids);
    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let trigger = if due_only { "schedule" } else { "manual" };
    let total_profiles = selected_profiles.len();

    report_progress(BackupProgressEvent {
        phase: "prepare".to_string(),
        label: "Inspect selected browser profiles".to_string(),
        detail: format!(
            "Queued {total_profiles} readable profile(s) for the canonical backup run."
        ),
        step: 0,
        total_steps: 3,
        completed_profiles: 0,
        total_profiles,
        profile_id: None,
        progress_current: Some(0),
        progress_total: Some(total_profiles),
        progress_percent: Some(0.0),
        log_lines: vec![format!("Queued {total_profiles} readable profile(s) for backup.")],
    });

    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('backup', ?1, ?2, ?3, 'running', ?4, '[]', '{}', ?5)",
        params![
            trigger,
            started_at,
            timezone,
            serde_json::to_string(
                &selected_profiles
                    .iter()
                    .map(|profile| profile.profile_id.clone())
                    .collect::<Vec<_>>()
            )?,
            due_only as i64,
        ],
    )?;
    let run_id = connection.last_insert_rowid();
    let parent_manifest = latest_manifest_row(&connection)?;

    let mut profile_summaries = Vec::new();
    let mut source_hashes = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut snapshot_artifacts = Vec::new();
    let mut source_evidence_plans = Vec::new();
    let mut warnings = skipped_profiles;

    let backup_result = (|| -> Result<()> {
        let transaction = connection.transaction()?;
        for (index, profile) in selected_profiles.iter().enumerate() {
            report_progress(BackupProgressEvent {
                phase: "stage-profile".to_string(),
                label: "Stage source profile".to_string(),
                detail: format!(
                    "Copying {} into the staging area ({}/{total_profiles}).",
                    profile.profile_id,
                    index + 1,
                ),
                step: 1,
                total_steps: 3,
                completed_profiles: index,
                total_profiles,
                profile_id: Some(profile.profile_id.clone()),
                progress_current: Some(index),
                progress_total: Some(total_profiles),
                progress_percent: if total_profiles == 0 {
                    None
                } else {
                    Some((index as f32 / total_profiles as f32) * 100.0)
                },
                log_lines: vec![format!("Staging {}.", profile.profile_id)],
            });
            let snapshot = stage_profile_snapshot(paths, profile)?;
            report_progress(BackupProgressEvent {
                phase: "ingest-profile".to_string(),
                label: "Write canonical archive facts".to_string(),
                detail: format!(
                    "Processing {} and writing archive rows ({}/{total_profiles}).",
                    profile.profile_id,
                    index + 1,
                ),
                step: 1,
                total_steps: 3,
                completed_profiles: index,
                total_profiles,
                profile_id: Some(profile.profile_id.clone()),
                progress_current: Some(index + 1),
                progress_total: Some(total_profiles),
                progress_percent: if total_profiles == 0 {
                    None
                } else {
                    Some((((index + 1) as f32) / total_profiles as f32) * 100.0)
                },
                log_lines: vec![format!("Writing canonical facts for {}.", profile.profile_id)],
            });
            let profile_summary = process_profile_snapshot(
                &transaction,
                run_id,
                paths,
                config,
                &snapshot,
                &mut snapshot_artifacts,
                &mut source_evidence_plans,
                true,
                true,
            )
            .with_context(|| format!("processing profile {}", profile.profile_id))?;
            source_hashes.insert(profile.profile_id.clone(), snapshot_source_hashes(&snapshot));
            warnings.extend(profile_summary.notes.clone());
            profile_summaries.push(profile_summary);
        }
        report_progress(BackupProgressEvent {
            phase: "finalize".to_string(),
            label: "Finalize manifest and cached totals".to_string(),
            detail: format!(
                "Committing run artifacts after {total_profiles} processed profile(s)."
            ),
            step: 2,
            total_steps: 3,
            completed_profiles: total_profiles,
            total_profiles,
            profile_id: None,
            progress_current: Some(total_profiles),
            progress_total: Some(total_profiles),
            progress_percent: Some(100.0),
            log_lines: vec!["Refreshing run ledger and cached summaries.".to_string()],
        });
        transaction.commit()?;
        Ok(())
    })();

    if let Err(error) = backup_result {
        finalize_failed_run(&connection, run_id, &profile_summaries, &warnings, &error)?;
        return Err(error);
    }

    if let Err(error) =
        persist_source_evidence_plans(&mut source_evidence, &connection, &source_evidence_plans)
    {
        warnings.push(format!(
            "Canonical backup completed, but the source-evidence archive needs a rebuild: {error}"
        ));
    }

    let finished_at = now_rfc3339();
    let summary = backup_run_summary(
        "backup",
        run_id,
        &started_at,
        &finished_at,
        trigger,
        &selected_profiles.iter().map(|profile| profile.profile_id.clone()).collect::<Vec<_>>(),
        &profile_summaries,
    );
    let row_counts = archive_row_counts(&connection)?;
    let manifest = BackupManifest {
        created_at: finished_at.clone(),
        run_id,
        timezone: timezone.clone(),
        due_only,
        database_path: paths.archive_database_path.display().to_string(),
        summary: summary.clone(),
        profiles: profile_summaries.clone(),
        warnings: warnings.clone(),
        source_hashes,
        snapshots: snapshot_artifacts.clone(),
        row_counts: row_counts.clone(),
        parent_manifest_hash: parent_manifest.as_ref().map(|row| row.hash.clone()),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    let manifest_hash = sha256_hex(manifest_json.as_bytes());
    let manifest_path =
        write_manifest_artifact(paths, run_id, &finished_at, &manifest_hash, &manifest_json)?;
    persist_manifest_row(
        &connection,
        run_id,
        parent_manifest.as_ref(),
        &manifest_hash,
        &manifest_path,
        &finished_at,
        &row_counts,
    )?;
    finalize_successful_run(
        &connection,
        run_id,
        &finished_at,
        &summary,
        &warnings,
        &manifest_hash,
    )?;

    if let Err(error) = rebuild_search_projection(paths, config, key) {
        warnings.push(format!(
            "Canonical backup completed, but the keyword-recall projection needs a rebuild: {error}"
        ));
    }

    let git_commit = if config.git_enabled {
        git_audit::ensure_repo(&paths.audit_repo_path)?;
        git_audit::commit_all(&paths.audit_repo_path, &format!("backup run {run_id}"))?
    } else {
        None
    };

    Ok(BackupReport {
        due_skipped: false,
        reason: None,
        run: Some(BackupRunOverview { manifest_hash: Some(manifest_hash), ..summary }),
        profiles: profile_summaries,
        manifest_path: Some(manifest_path.display().to_string()),
        git_commit,
        warnings,
        remote_backup: None,
    })
}

/// Counts visible archive totals without including reverted visits.
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

fn create_snapshot_artifact(
    archive: &Transaction<'_>,
    run_id: i64,
    paths: &ProjectPaths,
    snapshot: &ProfileSnapshot,
    reason: &str,
) -> Result<SnapshotArtifact> {
    let checkpoint_dir = paths
        .raw_snapshots_dir
        .join(&snapshot.profile.profile_id)
        .join(now_rfc3339().replace(':', "-"));
    fs::create_dir_all(&checkpoint_dir)?;

    let mut copied = Vec::<(String, String)>::new();
    let history_target = checkpoint_dir.join("History");
    fs::copy(&snapshot.history_path, &history_target)?;
    copied.push((
        history_target.display().to_string(),
        crate::utils::file_sha256_hex(&history_target)?,
    ));
    if let Some(favicons_path) = &snapshot.favicons_path {
        let target = checkpoint_dir.join("Favicons");
        fs::copy(favicons_path, &target)?;
        copied.push((target.display().to_string(), crate::utils::file_sha256_hex(&target)?));
    }

    let metadata_json = serde_json::to_string(&copied)?;
    let checksum = sha256_hex(metadata_json.as_bytes());
    let file_path = checkpoint_dir.display().to_string();
    let file_size = copied
        .iter()
        .map(|(path, _)| fs::metadata(path).map(|meta| meta.len()).unwrap_or_default())
        .sum::<u64>() as i64;

    archive.execute(
        "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![run_id, file_path, file_size, checksum, reason, now_rfc3339()],
    )?;

    Ok(SnapshotArtifact {
        kind: "raw-source-checkpoint".to_string(),
        path: checkpoint_dir.display().to_string(),
        checksum,
        reason: reason.to_string(),
    })
}

fn load_snapshot_record(
    connection: &Connection,
    snapshot_path: &str,
) -> Result<Option<SnapshotRecord>> {
    connection
        .query_row(
            "SELECT snapshots.run_id, runs.profile_scope_json, snapshots.file_path, snapshots.created_at, snapshots.reason
             FROM snapshots
             JOIN runs
               ON runs.id = snapshots.run_id
             WHERE snapshots.file_path = ?1
             LIMIT 1",
            [snapshot_path],
            |row| {
                Ok(SnapshotRecord {
                    run_id: row.get(0)?,
                    profile_scope: decode_profile_scope(row.get::<_, Option<String>>(1)?.as_deref()),
                    file_path: row.get(2)?,
                    created_at: row.get(3)?,
                    reason: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn load_checkpoint_profile_snapshot(
    connection: &Connection,
    snapshot_path: &Path,
    snapshot: &SnapshotRecord,
) -> Result<ProfileSnapshot> {
    let profile_id = snapshot
        .profile_scope
        .first()
        .cloned()
        .or_else(|| checkpoint_profile_id_from_path(snapshot_path))
        .context("snapshot restore requires a recorded profile scope")?;
    let history_path = snapshot_path.join("History");
    if !history_path.exists() {
        anyhow::bail!(
            "snapshot {} is not a saved browser source checkpoint",
            snapshot_path.display()
        );
    }
    let favicons_path = snapshot_path.join("Favicons");
    let profile = load_snapshot_browser_profile(
        connection,
        &profile_id,
        &history_path,
        favicons_path.exists(),
    )?;
    let source_hashes = snapshot_file_fingerprints(
        &history_path,
        favicons_path.exists().then_some(favicons_path.as_path()),
    )?;
    Ok(ProfileSnapshot {
        profile,
        temp_dir: tempdir().context("allocating restore snapshot tempdir")?,
        history_path,
        favicons_path: favicons_path.exists().then_some(favicons_path),
        source_hashes,
    })
}

fn checkpoint_profile_id_from_path(snapshot_path: &Path) -> Option<String> {
    snapshot_path.parent()?.file_name()?.to_str().map(str::to_string)
}

fn load_snapshot_browser_profile(
    connection: &Connection,
    profile_id: &str,
    history_path: &Path,
    has_favicons: bool,
) -> Result<crate::models::BrowserProfile> {
    let row = connection
        .query_row(
            "SELECT browser_kind, browser_version, profile_name, profile_path, user_name
             FROM source_profiles
             WHERE profile_key = ?1
             LIMIT 1",
            [profile_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;
    let (browser_kind, browser_version, profile_name, profile_path, user_name) = row
        .unwrap_or_else(|| {
            let browser_kind = profile_id.split(':').next().unwrap_or("archive").to_string();
            (
                browser_kind.clone(),
                None,
                profile_id.to_string(),
                snapshot_path_parent_display(history_path),
                None,
            )
        });
    let history_bytes = file_size(history_path);
    let favicons_path = history_path.parent().map(|parent| parent.join("Favicons"));
    let favicons_bytes = favicons_path
        .as_ref()
        .filter(|path| path.exists())
        .map(|path| file_size(path))
        .unwrap_or_default();
    Ok(crate::models::BrowserProfile {
        profile_id: profile_id.to_string(),
        profile_name,
        browser_family: browser_family_for_profile(&browser_kind),
        browser_name: browser_name_for_profile(&browser_kind),
        user_name,
        profile_path,
        history_path: Some(history_path.display().to_string()),
        favicons_path: has_favicons.then(|| {
            history_path.parent().unwrap_or(history_path).join("Favicons").display().to_string()
        }),
        history_exists: true,
        browser_version,
        history_file_name: history_file_name_for_profile(&browser_kind),
        history_bytes,
        favicons_bytes,
        supporting_bytes: 0,
        retention_boundary: crate::browser_retention::retention_boundary_for_browser(&browser_kind),
    })
}

fn snapshot_path_parent_display(path: &Path) -> String {
    path.parent().unwrap_or(path).display().to_string()
}

fn browser_family_for_profile(browser_kind: &str) -> String {
    match browser_kind {
        "firefox" | "librewolf" | "floorp" | "waterfox" => "firefox".to_string(),
        "safari" => "safari".to_string(),
        _ => "chromium".to_string(),
    }
}

fn browser_name_for_profile(browser_kind: &str) -> String {
    match browser_kind {
        "edge" => "Microsoft Edge".to_string(),
        "brave" => "Brave".to_string(),
        "vivaldi" => "Vivaldi".to_string(),
        "arc" => "Arc".to_string(),
        "firefox" => "Firefox".to_string(),
        "librewolf" => "LibreWolf".to_string(),
        "floorp" => "Floorp".to_string(),
        "waterfox" => "Waterfox".to_string(),
        "safari" => "Safari".to_string(),
        _ => "Google Chrome".to_string(),
    }
}

fn history_file_name_for_profile(browser_kind: &str) -> String {
    if matches!(browser_kind, "firefox" | "librewolf" | "floorp" | "waterfox") {
        "places.sqlite".to_string()
    } else if browser_kind == "safari" {
        "History.db".to_string()
    } else {
        "History".to_string()
    }
}

fn snapshot_file_fingerprints(
    history_path: &Path,
    favicons_path: Option<&Path>,
) -> Result<Vec<FileFingerprint>> {
    let mut fingerprints = vec![FileFingerprint {
        path: history_path.display().to_string(),
        sha256: file_sha256_hex(history_path)?,
    }];
    if let Some(favicons_path) = favicons_path
        && favicons_path.exists()
    {
        fingerprints.push(FileFingerprint {
            path: favicons_path.display().to_string(),
            sha256: file_sha256_hex(favicons_path)?,
        });
    }
    Ok(fingerprints)
}

fn record_snapshot_reference(
    connection: &Connection,
    run_id: i64,
    path: &Path,
    reason: &str,
    created_at: &str,
) -> Result<()> {
    let (file_size, checksum) = snapshot_artifact_bytes_and_checksum(path)?;
    connection.execute(
        "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            run_id,
            path.display().to_string(),
            file_size as i64,
            checksum,
            reason,
            created_at,
        ],
    )?;
    Ok(())
}

fn snapshot_artifact_bytes_and_checksum(path: &Path) -> Result<(u64, Option<String>)> {
    if !path.exists() {
        return Ok((0, None));
    }
    if path.is_file() {
        return Ok((file_size(path), Some(file_sha256_hex(path)?)));
    }
    let file_hashes = collect_path_file_hashes(path, path)?;
    let checksum = sha256_hex(serde_json::to_string(&file_hashes)?.as_bytes());
    let size = file_hashes.iter().map(|(_, _, bytes)| *bytes).sum();
    Ok((size, Some(checksum)))
}

fn collect_path_file_hashes(root: &Path, path: &Path) -> Result<Vec<(String, String, u64)>> {
    let mut entries = Vec::new();
    let Ok(children) = fs::read_dir(path) else {
        return Ok(entries);
    };
    for child in children.flatten() {
        let child_path = child.path();
        if child_path.is_dir() {
            entries.extend(collect_path_file_hashes(root, &child_path)?);
            continue;
        }
        let relative_path =
            child_path.strip_prefix(root).unwrap_or(&child_path).display().to_string();
        let bytes = file_size(&child_path);
        entries.push((relative_path, file_sha256_hex(&child_path)?, bytes));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

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

fn retention_directory_bucket(id: &str, path: &Path) -> RetentionBucket {
    RetentionBucket {
        id: id.to_string(),
        bytes: directory_size(path),
        item_count: count_path_entries(path),
        paths: vec![path.display().to_string()],
    }
}

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

fn prune_snapshot_bucket(connection: &Connection, paths: &ProjectPaths) -> Result<(u64, usize)> {
    let deleted = remove_directory_contents(&paths.raw_snapshots_dir)?;
    connection.execute("DELETE FROM snapshots", [])?;
    Ok(deleted)
}

fn persist_structured_manifest(
    connection: &Connection,
    paths: &ProjectPaths,
    run_id: i64,
    finished_at: &str,
    payload: &Value,
) -> Result<(String, PathBuf)> {
    let manifest_json = serde_json::to_string_pretty(payload)?;
    let manifest_hash = sha256_hex(manifest_json.as_bytes());
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

fn latest_manifest_row(connection: &Connection) -> Result<Option<ManifestRow>> {
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

fn persist_manifest_row(
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

fn finalize_successful_run(
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

fn finalize_failed_run(
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

fn backup_run_summary(
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

fn archive_row_counts(connection: &Connection) -> Result<Value> {
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

fn write_manifest_artifact(
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

fn collect_schema_payload(path: &Path) -> Result<Value> {
    let connection = open_readonly_source(path)?;
    let mut statement = connection.prepare(
        "SELECT type, name, tbl_name, sql
         FROM sqlite_master
         WHERE sql IS NOT NULL
         ORDER BY type, name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(json!({
            "type": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "tableName": row.get::<_, String>(2)?,
            "sql": row.get::<_, String>(3)?,
        }))
    })?;
    Ok(Value::Array(rows.collect::<rusqlite::Result<Vec<_>>>()?))
}

fn open_readonly_source(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening source {}", path.display()))
}

fn serialize_payload<T: Serialize>(value: &T) -> Result<SerializedPayload> {
    let json = serde_json::to_string(value)?;
    let hash = sha256_hex(json.as_bytes());
    Ok(SerializedPayload { hash })
}

/// Merges run-local stats with current visible archive totals.
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

fn current_timezone_name() -> String {
    get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

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

fn backup_due_skip_reason(connection: &Connection, config: &AppConfig) -> Result<Option<String>> {
    Ok(latest_successful_backup_at(connection)?
        .and_then(|last_backup_at| backup_due_skip_reason_at(last_backup_at, config, Utc::now())))
}

fn backup_due_skip_reason_at(
    last_backup_at: DateTime<Utc>,
    config: &AppConfig,
    now: DateTime<Utc>,
) -> Option<String> {
    let elapsed = now - last_backup_at;
    (elapsed < Duration::hours(config.due_after_hours as i64))
        .then(|| format!("last successful backup is only {} hours old", elapsed.num_hours()))
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
