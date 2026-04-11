mod schema;

pub(crate) use self::schema::apply_cipher_key;
pub(crate) use self::schema::export_archive_database;
pub use self::schema::{create_schema, open_archive_connection};
pub use self::schema::{current_version, run_migrations};
use crate::{
    chrome::{FileFingerprint, ProfileSnapshot, discover_profiles, stage_profile_snapshot},
    config::{ProjectPaths, ensure_paths, save_config},
    git_audit,
    models::{
        AppConfig, ArchiveMode, ArchiveStatus, AuditArtifact, AuditRunDetail, BackupProfileSummary,
        BackupProgressEvent, BackupReport, BackupRunOverview, DashboardSnapshot, ExportFormat,
        ExportRequest, ExportResult, HealthCheck, HealthRepairReport, HealthReport, HistoryEntry,
        HistoryQuery, HistoryQueryResponse, RetentionBucket, RetentionPreview,
        RetentionPruneRequest, RetentionPruneResult, SnapshotRestorePreview,
        SnapshotRestoreRequest, StorageSummary,
    },
    utils::{file_sha256_hex, now_rfc3339, sha256_hex, unix_micros_to_chrome_time, url_domain},
};
use anyhow::{Context, Result};
use browser_history_parser::{
    ChromiumReadCursor, HistoryDatabaseSet, ParsedDownload, ParsedFavicon, ParsedHistory,
    ParsedSearchTerm, ParsedUrl, ParsedVisit, chromium, firefox, safari,
};
use chrono::{DateTime, Duration, Utc};
use iana_time_zone::get_timezone;
use regex::RegexBuilder;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, named_params, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
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
  visits.app_id
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
JOIN history_search
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
JOIN history_search
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

#[derive(Debug, Default)]
struct Watermark {
    last_visit_id: i64,
    last_url_last_visit_time: i64,
    last_download_id: i64,
    last_favicon_last_updated: i64,
    last_checkpoint_at: Option<String>,
    last_schema_hash: Option<String>,
    updated_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
struct RawRowInsert<'a> {
    run_id: i64,
    source_profile_id: i64,
    profile_id: &'a str,
    source_kind: &'a str,
    table_name: &'a str,
    source_pk: &'a str,
    payload_hash: &'a str,
    payload_json: &'a str,
    schema_hash: &'a str,
    chrome_version: Option<&'a str>,
    import_batch_id: Option<i64>,
}

#[derive(Debug)]
struct SerializedPayload {
    json: String,
    hash: String,
}

#[derive(Debug, Clone)]
struct UrlVisitBounds {
    first_visit_ms: i64,
    first_visit_iso: String,
    last_visit_ms: i64,
    last_visit_iso: String,
}

#[derive(Debug)]
struct ParsedProfileSnapshot {
    source_kind: &'static str,
    history: ParsedHistory,
    last_visit_id: i64,
    last_url_marker: Option<i64>,
    last_download_id: Option<i64>,
    last_favicon_marker: Option<i64>,
}

pub fn ensure_archive_initialized(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ArchiveStatus> {
    ensure_paths(paths)?;
    let mut next_config = config.clone();
    next_config.initialized = true;
    save_config(paths, &next_config)?;
    let connection = open_archive_connection(paths, &next_config, key)?;
    create_schema(&connection)?;
    archive_status(paths, &next_config, key)
}

pub fn archive_status(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ArchiveStatus> {
    ensure_paths(paths)?;
    let mut status = ArchiveStatus {
        initialized: config.initialized && paths.archive_database_path.exists(),
        encrypted: matches!(config.archive_mode, ArchiveMode::Encrypted),
        unlocked: false,
        database_path: paths.archive_database_path.display().to_string(),
        last_successful_backup_at: None,
        warning: None,
    };

    if !status.initialized {
        return Ok(status);
    }

    match open_archive_connection(paths, config, key) {
        Ok(connection) => {
            create_schema(&connection)?;
            status.unlocked = true;
            status.last_successful_backup_at = connection
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
        }
        Err(error) => status.warning = Some(error.to_string()),
    }

    Ok(status)
}

pub fn load_recent_runs(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Vec<BackupRunOverview>> {
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(Vec::new());
    }

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let mut statement = connection.prepare(RECENT_RUNS_SQL)?;
    let rows = statement.query_map([], backup_run_overview_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn load_dashboard_snapshot(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<DashboardSnapshot> {
    let recent_runs = load_recent_runs(paths, config, key)?;
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(DashboardSnapshot {
            generated_at: now_rfc3339(),
            recent_runs,
            next_action: Some(
                "Initialize the archive before running your first manual backup.".to_string(),
            ),
            storage: storage_summary(paths),
            ..DashboardSnapshot::default()
        });
    }

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let totals = load_cached_archive_totals(&connection)?
        .unwrap_or(count_visible_archive_totals(&connection)?);
    let last_successful_backup_at = connection
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

    let next_action = if recent_runs.is_empty() {
        Some("Run a manual backup to create the first manifest and snapshot artifacts.".to_string())
    } else {
        None
    };

    Ok(DashboardSnapshot {
        generated_at: now_rfc3339(),
        total_profiles: totals.total_profiles,
        total_urls: totals.total_urls,
        total_visits: totals.total_visits,
        total_downloads: totals.total_downloads,
        last_successful_backup_at,
        recent_runs,
        storage: storage_summary(paths),
        next_action,
    })
}

pub fn load_audit_run_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    run_id: i64,
) -> Result<AuditRunDetail> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let row = connection.query_row(
        "SELECT
           id,
           run_type,
           trigger,
           timezone,
           due_only,
           started_at,
           finished_at,
           status,
           profile_scope_json,
           stats_json,
           warnings_json,
           error_message
         FROM runs
         WHERE id = ?1",
        [run_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        },
    )?;
    let manifest = connection
        .query_row(
            "SELECT file_path, content_hash
             FROM manifests
             WHERE run_id = ?1
             LIMIT 1",
            [run_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    let mut statement = connection.prepare(
        "SELECT file_path, checksum, file_size, created_at, reason
         FROM snapshots
         WHERE run_id = ?1
         ORDER BY id ASC",
    )?;
    let artifacts = statement
        .query_map([run_id], |row| {
            Ok(AuditArtifact {
                kind: "snapshot".to_string(),
                path: row.get(0)?,
                checksum: row.get(1)?,
                size_bytes: row.get::<_, Option<i64>>(2)?.map(|value| value.max(0) as u64),
                created_at: row.get(3)?,
                reason: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let profile_scope = decode_profile_scope(row.8.as_deref());
    let stats = decode_run_stats(row.9.as_deref());
    let warnings = row
        .10
        .as_ref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();

    Ok(AuditRunDetail {
        run: BackupRunOverview {
            id: row.0,
            started_at: row.5,
            finished_at: row.6,
            status: row.7,
            run_type: row.1,
            trigger: row.2.clone(),
            profile_scope: profile_scope.clone(),
            manifest_hash: manifest.as_ref().and_then(|(_, hash)| hash.clone()),
            profiles_processed: run_profiles_processed(&stats, &profile_scope),
            new_visits: run_new_visits(&stats),
            new_urls: run_new_urls(&stats),
            new_downloads: run_new_downloads(&stats),
        },
        trigger: row.2,
        timezone: row.3,
        due_only: row.4 != 0,
        profile_scope,
        warnings,
        error_message: row.11,
        stats,
        manifest_path: manifest.as_ref().and_then(|(path, _)| path.clone()),
        manifest_hash: manifest.and_then(|(_, hash)| hash),
        artifacts,
    })
}

pub fn preview_snapshot_restore(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SnapshotRestoreRequest,
) -> Result<SnapshotRestorePreview> {
    ensure_paths(paths)?;
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let snapshot = load_snapshot_record(&connection, &request.snapshot_path)?
        .with_context(|| format!("snapshot {} was not found", request.snapshot_path))?;
    let snapshot_path = PathBuf::from(&snapshot.file_path);

    if snapshot_path.is_file() {
        return Ok(SnapshotRestorePreview {
            snapshot_path: snapshot.file_path,
            snapshot_kind: "archive-safety-snapshot".to_string(),
            source_run_id: Some(snapshot.run_id),
            source_profile_id: snapshot.profile_scope.first().cloned(),
            source_browser_name: None,
            created_at: Some(snapshot.created_at),
            reason: snapshot.reason,
            execute_supported: false,
            estimated_visits: 0,
            estimated_urls: 0,
            estimated_downloads: 0,
            warnings: vec![
                "This snapshot is a full archive safety copy. PathKeep currently automates restore only for saved browser source checkpoints; keep this file for manual recovery review.".to_string(),
            ],
        });
    }

    let checkpoint = load_checkpoint_profile_snapshot(&connection, &snapshot_path, &snapshot)?;
    let parsed = parse_profile_snapshot(&checkpoint, config, &Watermark::default())?;

    Ok(SnapshotRestorePreview {
        snapshot_path: snapshot.file_path,
        snapshot_kind: "raw-source-checkpoint".to_string(),
        source_run_id: Some(snapshot.run_id),
        source_profile_id: Some(checkpoint.profile.profile_id.clone()),
        source_browser_name: Some(checkpoint.profile.browser_name.clone()),
        created_at: Some(snapshot.created_at),
        reason: snapshot.reason,
        execute_supported: true,
        estimated_visits: parsed.history.visits.len(),
        estimated_urls: parsed.history.urls.len(),
        estimated_downloads: parsed.history.downloads.len(),
        warnings: vec![
            "Snapshot restore replays the saved browser checkpoint into the current archive. Existing visible archive facts stay in place and duplicate rows are skipped.".to_string(),
        ],
    })
}

pub fn run_snapshot_restore(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SnapshotRestoreRequest,
) -> Result<BackupReport> {
    ensure_paths(paths)?;
    if !config.initialized {
        anyhow::bail!("archive has not been initialized");
    }

    let mut connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let snapshot = load_snapshot_record(&connection, &request.snapshot_path)?
        .with_context(|| format!("snapshot {} was not found", request.snapshot_path))?;
    let snapshot_path = PathBuf::from(&snapshot.file_path);
    if snapshot_path.is_file() {
        anyhow::bail!(
            "automatic restore is only supported for saved browser source checkpoints right now"
        );
    }

    let checkpoint = load_checkpoint_profile_snapshot(&connection, &snapshot_path, &snapshot)?;
    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let profile_scope = vec![checkpoint.profile.profile_id.clone()];

    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('snapshot_restore', 'manual', ?1, ?2, 'running', ?3, '[]', '{}', 0)",
        params![started_at, timezone, serde_json::to_string(&profile_scope)?],
    )?;
    let run_id = connection.last_insert_rowid();
    let parent_manifest = latest_manifest_row(&connection)?;

    let mut snapshot_artifacts = Vec::new();
    let restore_result = (|| -> Result<BackupProfileSummary> {
        let transaction = connection.transaction()?;
        let profile_summary = process_profile_snapshot(
            &transaction,
            run_id,
            paths,
            config,
            &checkpoint,
            &mut snapshot_artifacts,
            false,
            false,
        )?;
        transaction.commit()?;
        Ok(profile_summary)
    })();

    let profile_summary = match restore_result {
        Ok(profile_summary) => profile_summary,
        Err(error) => {
            finalize_failed_run(&connection, run_id, &[], &[], &error)?;
            return Err(error);
        }
    };

    record_snapshot_reference(
        &connection,
        run_id,
        &snapshot_path,
        "restored-source-checkpoint",
        &snapshot.created_at,
    )?;

    let finished_at = now_rfc3339();
    let summary = backup_run_summary(
        "snapshot_restore",
        run_id,
        &started_at,
        &finished_at,
        "manual",
        &profile_scope,
        std::slice::from_ref(&profile_summary),
    );
    let row_counts = archive_row_counts(&connection)?;
    let manifest = BackupManifest {
        created_at: finished_at.clone(),
        run_id,
        timezone: timezone.clone(),
        due_only: false,
        database_path: paths.archive_database_path.display().to_string(),
        summary: summary.clone(),
        profiles: vec![profile_summary.clone()],
        warnings: Vec::new(),
        source_hashes: BTreeMap::from([(
            checkpoint.profile.profile_id.clone(),
            snapshot_source_hashes(&checkpoint),
        )]),
        snapshots: snapshot_artifacts,
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
    finalize_successful_run(&connection, run_id, &finished_at, &summary, &[], &manifest_hash)?;

    let git_commit = if config.git_enabled {
        git_audit::ensure_repo(&paths.audit_repo_path)?;
        git_audit::commit_all(&paths.audit_repo_path, &format!("snapshot restore run {run_id}"))?
    } else {
        None
    };

    Ok(BackupReport {
        due_skipped: false,
        reason: None,
        run: Some(BackupRunOverview { manifest_hash: Some(manifest_hash), ..summary }),
        profiles: vec![profile_summary],
        manifest_path: Some(manifest_path.display().to_string()),
        git_commit,
        warnings: Vec::new(),
        remote_backup: None,
    })
}

pub fn preview_retention(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<RetentionPreview> {
    ensure_paths(paths)?;
    let snapshot_bucket = retention_snapshot_bucket(paths, config, key)?;
    let export_bucket = retention_directory_bucket("exports", &paths.exports_dir);
    let staging_bucket = retention_directory_bucket("staging", &paths.staging_dir);
    let quarantine_bucket = retention_directory_bucket("quarantine", &paths.quarantine_dir);

    Ok(RetentionPreview {
        buckets: vec![snapshot_bucket, export_bucket, staging_bucket, quarantine_bucket],
        warnings: vec![
            "Pruning snapshots removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.".to_string(),
            "Export pruning only removes local files under the PathKeep data directory. Remote objects are unchanged.".to_string(),
        ],
    })
}

pub fn run_retention_prune(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &RetentionPruneRequest,
) -> Result<RetentionPruneResult> {
    ensure_paths(paths)?;
    if !config.initialized {
        anyhow::bail!("initialize the archive before pruning retention artifacts");
    }
    if request.bucket_ids.is_empty() {
        return Ok(RetentionPruneResult {
            warnings: vec![
                "Choose at least one retention bucket before executing prune.".to_string(),
            ],
            ..RetentionPruneResult::default()
        });
    }

    let preview = preview_retention(paths, config, key)?;
    let selected = preview
        .buckets
        .iter()
        .filter(|bucket| request.bucket_ids.iter().any(|id| id == &bucket.id))
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Ok(RetentionPruneResult {
            warnings: vec!["No matching retention buckets were selected for prune.".to_string()],
            ..RetentionPruneResult::default()
        });
    }

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('retention_prune', 'manual', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, timezone],
    )?;
    let run_id = connection.last_insert_rowid();

    let mut deleted_bytes = 0u64;
    let mut deleted_files = 0usize;
    for bucket in &selected {
        match bucket.id.as_str() {
            "snapshots" => {
                let (bytes, files) = prune_snapshot_bucket(&connection, paths)?;
                deleted_bytes += bytes;
                deleted_files += files;
            }
            "exports" => {
                let (bytes, files) = remove_directory_contents(&paths.exports_dir)?;
                deleted_bytes += bytes;
                deleted_files += files;
            }
            "staging" => {
                let (bytes, files) = remove_directory_contents(&paths.staging_dir)?;
                deleted_bytes += bytes;
                deleted_files += files;
            }
            "quarantine" => {
                let (bytes, files) = remove_directory_contents(&paths.quarantine_dir)?;
                deleted_bytes += bytes;
                deleted_files += files;
            }
            _ => {}
        }
    }

    let finished_at = now_rfc3339();
    let manifest_payload = json!({
        "runType": "retention_prune",
        "runId": run_id,
        "createdAt": finished_at,
        "deletedBytes": deleted_bytes,
        "deletedFiles": deleted_files,
        "buckets": selected,
    });
    let (manifest_hash, manifest_path) =
        persist_structured_manifest(&connection, paths, run_id, &finished_at, &manifest_payload)?;
    let stats = stats_with_archive_totals(
        &connection,
        json!({
            "deletedBytes": deleted_bytes,
            "deletedFiles": deleted_files,
            "buckets": request.bucket_ids,
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
            serde_json::to_string(&preview.warnings)?,
            run_id,
        ],
    )?;

    if config.git_enabled {
        git_audit::ensure_repo(&paths.audit_repo_path)?;
        let _ = git_audit::commit_all(
            &paths.audit_repo_path,
            &format!("retention prune run {run_id}"),
        )?;
    }

    let _ = manifest_path;

    Ok(RetentionPruneResult {
        run_id: Some(run_id),
        deleted_bytes,
        deleted_files,
        buckets: selected,
        warnings: preview.warnings,
    })
}

pub fn run_backup(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    due_only: bool,
) -> Result<BackupReport> {
    run_backup_with_progress(paths, config, key, due_only, |_| {})
}

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
    create_schema(&connection)?;

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
            });
            let profile_summary = process_profile_snapshot(
                &transaction,
                run_id,
                paths,
                config,
                &snapshot,
                &mut snapshot_artifacts,
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
        });
        transaction.commit()?;
        Ok(())
    })();

    if let Err(error) = backup_result {
        finalize_failed_run(&connection, run_id, &profile_summaries, &warnings, &error)?;
        return Err(error);
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

pub fn list_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    fn parse_history_cursor(cursor: Option<&str>) -> Option<(i64, i64)> {
        let raw = cursor?;
        let (visit_time, id) = raw.split_once('|')?;
        Some((visit_time.parse().ok()?, id.parse().ok()?))
    }

    fn encode_history_cursor(entry: &HistoryEntry) -> String {
        format!("{}|{}", entry.visit_time, entry.id)
    }

    fn page_count(total: usize, page_size: usize) -> usize {
        if total == 0 || page_size == 0 { 1 } else { ((total - 1) / page_size) + 1 }
    }

    fn build_history_response(
        total: usize,
        page_size: usize,
        page: usize,
        start_index: usize,
        items: Vec<HistoryEntry>,
    ) -> HistoryQueryResponse {
        let normalized_page_size = page_size.max(1);
        let normalized_page_count = page_count(total, normalized_page_size);
        let normalized_page = page.clamp(1, normalized_page_count);
        let has_previous = start_index > 0;
        let has_next = start_index + items.len() < total;

        HistoryQueryResponse {
            total,
            page: normalized_page,
            page_size: normalized_page_size,
            page_count: normalized_page_count,
            has_previous,
            has_next,
            next_cursor: has_next.then(|| items.last().map(encode_history_cursor)).flatten(),
            items,
        }
    }

    fn build_fts_query(raw: &str) -> Option<String> {
        let tokens = raw
            .split(|character: char| !character.is_alphanumeric())
            .filter(|token| !token.is_empty())
            .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
            .collect::<Vec<_>>();
        if tokens.is_empty() { None } else { Some(tokens.join(" AND ")) }
    }

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let limit = query.limit.unwrap_or(150).clamp(1, 1_000);
    let limit_usize = limit as usize;
    let requested_page = query.page.map(|page| usize::try_from(page.max(1)).unwrap_or(usize::MAX));
    let profile_id = query.profile_id.clone();
    let browser_kind = query.browser_kind.clone();
    let start_time_ms = query.start_time_ms;
    let end_time_ms = query.end_time_ms;
    let q = query.q.clone().filter(|value| !value.trim().is_empty());
    let fts_query = q.as_deref().and_then(build_fts_query);
    let regex = if query.regex_mode.unwrap_or(false) {
        q.as_ref()
            .map(|value| {
                RegexBuilder::new(value)
                    .case_insensitive(true)
                    .build()
                    .with_context(|| format!("invalid regex pattern `{value}`"))
            })
            .transpose()?
    } else {
        None
    };
    let domain_pattern = query
        .domain
        .clone()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{value}%"));
    let sort = query.sort.clone().unwrap_or_else(|| "newest".to_string());
    let cursor = parse_history_cursor(query.cursor.as_deref());
    let (cursor_visit_time, cursor_id) = cursor.unwrap_or((0, 0));

    if let Some(regex) = regex {
        // Regex search is a manual, post-filtered mode that keeps the canonical
        // archive filters intact without pretending it is the fast default path.
        let mut statement = connection.prepare(LIST_HISTORY_SQL)?;
        let rows = statement.query_map(
            named_params! {
                ":profileId": profile_id,
                ":browserKind": browser_kind,
                ":query": Option::<String>::None,
                ":domainPattern": domain_pattern,
                ":startTimeMs": start_time_ms,
                ":endTimeMs": end_time_ms,
                ":sort": sort,
                ":cursorVisitTime": Option::<i64>::None,
                ":cursorId": Option::<i64>::None,
                ":pageLimit": -1i64,
            },
            history_entry_from_row,
        )?;
        let filtered_items = rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|entry| {
                regex.is_match(&entry.url)
                    || entry.title.as_ref().is_some_and(|title| regex.is_match(title))
            })
            .collect::<Vec<_>>();
        let total = filtered_items.len();
        let normalized_page_count = page_count(total, limit_usize);
        let page = requested_page.unwrap_or(1).min(normalized_page_count);
        let start_index = if requested_page.is_some() {
            page.saturating_sub(1) * limit_usize
        } else if let Some((cursor_visit_time, cursor_id)) = cursor {
            filtered_items
                .iter()
                .position(|entry| {
                    if sort == "oldest" {
                        entry.visit_time > cursor_visit_time
                            || (entry.visit_time == cursor_visit_time && entry.id > cursor_id)
                    } else {
                        entry.visit_time < cursor_visit_time
                            || (entry.visit_time == cursor_visit_time && entry.id < cursor_id)
                    }
                })
                .unwrap_or(total)
        } else {
            0
        };
        let items =
            filtered_items.into_iter().skip(start_index).take(limit_usize).collect::<Vec<_>>();

        return Ok(build_history_response(total, limit_usize, page, start_index, items));
    }

    if q.is_some() && fts_query.is_none() {
        return Ok(HistoryQueryResponse::default());
    }

    if let Some(fts_query) = fts_query {
        let total: usize = connection
            .query_row(
                COUNT_HISTORY_FTS_SQL,
                named_params! {
                    ":ftsQuery": fts_query.clone(),
                    ":profileId": profile_id.clone(),
                    ":browserKind": browser_kind.clone(),
                    ":domainPattern": domain_pattern.clone(),
                    ":startTimeMs": start_time_ms,
                    ":endTimeMs": end_time_ms,
                },
                |row| row.get::<_, i64>(0),
            )?
            .try_into()
            .expect("history count fits in usize");

        let mut statement = connection.prepare(LIST_HISTORY_FTS_SQL)?;
        let normalized_page_count = page_count(total, limit_usize);
        let page = requested_page.unwrap_or(1).min(normalized_page_count);
        let start_index = page.saturating_sub(1) * limit_usize;
        let page_limit = if requested_page.is_some() {
            i64::try_from(page.saturating_mul(limit_usize).saturating_add(1)).unwrap_or(i64::MAX)
        } else {
            i64::from(limit) + 1
        };
        let rows = statement.query_map(
            named_params! {
                ":ftsQuery": fts_query,
                ":profileId": profile_id,
                ":browserKind": browser_kind,
                ":domainPattern": domain_pattern,
                ":startTimeMs": start_time_ms,
                ":endTimeMs": end_time_ms,
                ":sort": sort,
                ":cursorVisitTime": if requested_page.is_some() { Option::<i64>::None } else { cursor.map(|_| cursor_visit_time) },
                ":cursorId": if requested_page.is_some() { Option::<i64>::None } else { cursor.map(|_| cursor_id) },
                ":pageLimit": page_limit,
            },
            history_entry_from_row,
        )?;
        let items = if requested_page.is_some() {
            rows.collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .skip(start_index)
                .take(limit_usize)
                .collect::<Vec<_>>()
        } else {
            let mut window_items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            if window_items.len() > limit_usize {
                window_items.truncate(limit_usize);
            }
            window_items
        };

        return Ok(build_history_response(
            total,
            limit_usize,
            page,
            if requested_page.is_some() {
                start_index
            } else {
                if cursor.is_some() { limit_usize } else { 0 }
            },
            items,
        ));
    }

    let total: usize = connection
        .query_row(
            COUNT_HISTORY_SQL,
            named_params! {
                ":profileId": profile_id.clone(),
                ":browserKind": browser_kind.clone(),
                ":query": q.clone(),
                ":domainPattern": domain_pattern.clone(),
                ":startTimeMs": start_time_ms,
                ":endTimeMs": end_time_ms,
            },
            |row| row.get::<_, i64>(0),
        )?
        .try_into()
        .expect("history count fits in usize");

    let mut statement = connection.prepare(LIST_HISTORY_SQL)?;
    let normalized_page_count = page_count(total, limit_usize);
    let page = requested_page.unwrap_or(1).min(normalized_page_count);
    let start_index = page.saturating_sub(1) * limit_usize;
    let page_limit = if requested_page.is_some() {
        i64::try_from(page.saturating_mul(limit_usize).saturating_add(1)).unwrap_or(i64::MAX)
    } else {
        i64::from(limit) + 1
    };
    let rows = statement.query_map(
        named_params! {
            ":profileId": profile_id,
            ":browserKind": browser_kind,
            ":query": q,
            ":domainPattern": domain_pattern,
            ":startTimeMs": start_time_ms,
            ":endTimeMs": end_time_ms,
            ":sort": sort,
            ":cursorVisitTime": if requested_page.is_some() { Option::<i64>::None } else { cursor.map(|_| cursor_visit_time) },
            ":cursorId": if requested_page.is_some() { Option::<i64>::None } else { cursor.map(|_| cursor_id) },
            ":pageLimit": page_limit,
        },
        history_entry_from_row,
    )?;
    let items = if requested_page.is_some() {
        rows.collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .skip(start_index)
            .take(limit_usize)
            .collect::<Vec<_>>()
    } else {
        let mut window_items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        if window_items.len() > limit_usize {
            window_items.truncate(limit_usize);
        }
        window_items
    };

    Ok(build_history_response(
        total,
        limit_usize,
        page,
        if requested_page.is_some() {
            start_index
        } else {
            if cursor.is_some() { limit_usize } else { 0 }
        },
        items,
    ))
}

fn collect_history_for_export(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let mut export_query = query;
    export_query.cursor = None;
    export_query.limit = Some(1_000);

    let mut items = Vec::new();

    let total = loop {
        let page = list_history(paths, config, key, export_query.clone())?;
        let total = page.total;
        let next_cursor = page.next_cursor.clone();
        items.extend(page.items);

        let Some(next_cursor) = next_cursor else {
            break total;
        };

        export_query.cursor = Some(next_cursor);
    };

    Ok(HistoryQueryResponse {
        total,
        page: 1,
        page_size: items.len(),
        page_count: 1,
        has_previous: false,
        has_next: false,
        items,
        next_cursor: None,
    })
}

pub fn export_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ExportRequest,
) -> Result<ExportResult> {
    let results = collect_history_for_export(paths, config, key, request.query)?;
    fs::create_dir_all(&paths.exports_dir)?;
    let format = request.format;
    let extension = match format {
        ExportFormat::Html => "html",
        ExportFormat::Markdown => "md",
        ExportFormat::Text => "txt",
        ExportFormat::Jsonl => "jsonl",
    };
    let file_name = format!("export-{}.{}", now_rfc3339().replace(':', "-"), extension);
    let target_path = paths.exports_dir.join(file_name);
    let content = render_export_content(&results, &format)?;
    fs::write(&target_path, content)
        .with_context(|| format!("writing {}", target_path.display()))?;
    Ok(ExportResult { format, path: target_path.display().to_string(), count: results.items.len() })
}

pub fn rekey_archive(
    paths: &ProjectPaths,
    current_config: &AppConfig,
    old_key: Option<&str>,
    new_mode: ArchiveMode,
    new_key: Option<&str>,
) -> Result<ArchiveStatus> {
    ensure_paths(paths)?;
    if !paths.archive_database_path.exists() {
        anyhow::bail!("archive database does not exist");
    }

    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let source = open_archive_connection(paths, current_config, old_key)?;
    let snapshot_path = create_rekey_snapshot(paths)?;
    let temp_path = paths.archive_database_path.with_extension("rekey.sqlite");
    let backup_path = paths.archive_database_path.with_extension("backup.sqlite");
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }
    if backup_path.exists() {
        fs::remove_file(&backup_path)?;
    }
    let target_key = match new_mode {
        ArchiveMode::Encrypted => Some(new_key.context("new encryption key is required")?),
        ArchiveMode::Plaintext => None,
    };
    let mut next_config = current_config.clone();
    next_config.initialized = true;
    next_config.archive_mode = new_mode.clone();
    let result = (|| -> Result<ArchiveStatus> {
        export_archive_database(&source, &temp_path, target_key)?;
        fs::rename(&paths.archive_database_path, &backup_path)?;
        if let Err(error) = fs::rename(&temp_path, &paths.archive_database_path) {
            let _ = fs::rename(&backup_path, &paths.archive_database_path);
            let _ = fs::remove_file(&temp_path);
            return Err(error).context("replacing archive database after rekey export");
        }
        let _ = fs::remove_file(&backup_path);
        save_config(paths, &next_config)?;
        archive_status(paths, &next_config, new_key.or(old_key))
    })();

    let (status_label, run_config, run_key, run_error) = match &result {
        Ok(_) => ("success", &next_config, new_key.or(old_key), None),
        Err(error) => ("failed", current_config, old_key, Some(format!("{error:#}"))),
    };

    if let Ok(connection) = open_archive_connection(paths, run_config, run_key) {
        if create_schema(&connection).is_ok() {
            let _ = (|| -> Result<()> {
                connection.execute(
                    "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                     VALUES ('rekey', 'manual', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
                    params![started_at, timezone],
                )?;
                let run_id = connection.last_insert_rowid();
                record_snapshot_reference(
                    &connection,
                    run_id,
                    &snapshot_path,
                    "before-rekey",
                    &started_at,
                )?;
                let finished_at = now_rfc3339();
                let manifest_payload = json!({
                    "runType": "rekey",
                    "runId": run_id,
                    "createdAt": finished_at,
                    "fromMode": current_config.archive_mode,
                    "toMode": new_mode.clone(),
                    "snapshotPath": snapshot_path.display().to_string(),
                    "status": status_label,
                    "error": run_error.clone(),
                });
                let (manifest_hash, _manifest_path) = persist_structured_manifest(
                    &connection,
                    paths,
                    run_id,
                    &finished_at,
                    &manifest_payload,
                )?;
                let stats = stats_with_archive_totals(
                    &connection,
                    json!({
                        "fromMode": current_config.archive_mode,
                        "toMode": new_mode.clone(),
                        "snapshotPath": snapshot_path.display().to_string(),
                        "manifestHash": manifest_hash,
                    }),
                )?;
                connection.execute(
                    "UPDATE runs
                     SET finished_at = ?1,
                         status = ?2,
                         stats_json = ?3,
                         warnings_json = ?4,
                         error_message = ?5
                     WHERE id = ?6",
                    params![
                        finished_at,
                        status_label,
                        serde_json::to_string(&stats)?,
                        serde_json::to_string(&Vec::<String>::new())?,
                        run_error.clone(),
                        run_id,
                    ],
                )?;
                Ok(())
            })();
        }
    }

    result
}

fn create_rekey_snapshot(paths: &ProjectPaths) -> Result<PathBuf> {
    let snapshot_dir = paths.raw_snapshots_dir.join("rekey");
    fs::create_dir_all(&snapshot_dir)?;
    let snapshot_path = snapshot_dir
        .join(format!("archive-before-rekey-{}.sqlite", now_rfc3339().replace(':', "-")));
    fs::copy(&paths.archive_database_path, &snapshot_path).with_context(|| {
        format!("creating rekey safety snapshot at {}", snapshot_path.display())
    })?;
    Ok(snapshot_path)
}

pub fn doctor(paths: &ProjectPaths, config: &AppConfig, key: Option<&str>) -> Result<HealthReport> {
    ensure_paths(paths)?;
    let discovered_profiles = discover_profiles().unwrap_or_default();
    let status = archive_status(paths, config, key)?;
    let connection = if status.initialized && status.unlocked {
        Some(open_archive_connection(paths, config, key)?)
    } else {
        None
    };

    let mut checks = Vec::new();
    checks.push(HealthCheck {
        name: "Config".to_string(),
        ok: paths.config_path.exists(),
        detail: paths.config_path.display().to_string(),
    });
    checks.push(HealthCheck {
        name: "Browser sources".to_string(),
        ok: !discovered_profiles.is_empty(),
        detail: if discovered_profiles.is_empty() {
            "No supported browser profiles were detected in the known source locations.".to_string()
        } else {
            format!(
                "{} supported browser profiles detected across local data roots.",
                discovered_profiles.len()
            )
        },
    });
    checks.push(HealthCheck {
        name: "Archive DB".to_string(),
        ok: paths.archive_database_path.exists(),
        detail: paths.archive_database_path.display().to_string(),
    });
    checks.push(HealthCheck {
        name: "Archive Unlock".to_string(),
        ok: status.unlocked,
        detail: if matches!(config.archive_mode, ArchiveMode::Encrypted) {
            "Encrypted archive requires an active session key".to_string()
        } else {
            "Plaintext archive".to_string()
        },
    });

    if let Some(connection) = connection.as_ref() {
        create_schema(connection)?;
        checks.push(HealthCheck {
            name: "Schema version".to_string(),
            ok: current_version(connection)? >= 2,
            detail: format!("current canonical schema version is {}", current_version(connection)?),
        });
        checks.push(check_manifest_chain(connection)?);
        checks.push(check_snapshot_files(connection)?);
        checks.push(check_import_audit_artifacts(connection)?);
        checks.push(check_broken_visibility(connection)?);
        checks.push(check_stale_derived_state(connection)?);
    }

    Ok(HealthReport { generated_at: now_rfc3339(), checks })
}

pub fn repair_health_issues(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<HealthRepairReport> {
    ensure_paths(paths)?;
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;

    let missing_import_audits = missing_import_audit_batches(&connection)?;
    let broken_visibility_rows: usize = connection
        .query_row(
            "SELECT COUNT(*)
             FROM visits
             LEFT JOIN runs
               ON runs.id = visits.reverted_by_run_id
             WHERE visits.reverted_at IS NOT NULL
               AND (visits.reverted_by_run_id IS NULL OR runs.id IS NULL)",
            [],
            |row| row.get::<_, i64>(0),
        )?
        .max(0) as usize;
    let stale_ai_embeddings = if table_exists(&connection, "ai_embeddings")? {
        connection
            .query_row(
                "SELECT COUNT(*)
                 FROM ai_embeddings
                 WHERE history_id NOT IN (SELECT id FROM visit_events)",
                [],
                |row| row.get::<_, i64>(0),
            )?
            .max(0) as usize
    } else {
        0
    };
    let stale_insight_state = if table_exists(&connection, "insight_thread_members")?
        || table_exists(&connection, "visit_insight_features")?
    {
        let stale_members = if table_exists(&connection, "insight_thread_members")? {
            connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM insight_thread_members
                     WHERE history_id NOT IN (SELECT id FROM visit_events)",
                    [],
                    |row| row.get::<_, i64>(0),
                )?
                .max(0) as usize
        } else {
            0
        };
        let stale_features = if table_exists(&connection, "visit_insight_features")? {
            connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM visit_insight_features
                     WHERE history_id NOT IN (SELECT id FROM visit_events)",
                    [],
                    |row| row.get::<_, i64>(0),
                )?
                .max(0) as usize
        } else {
            0
        };
        stale_members + stale_features
    } else {
        0
    };

    if missing_import_audits.is_empty()
        && broken_visibility_rows == 0
        && stale_ai_embeddings == 0
        && stale_insight_state == 0
    {
        return Ok(HealthRepairReport {
            run_id: None,
            notes: vec!["Doctor repair found no actionable damage.".to_string()],
            ..HealthRepairReport::default()
        });
    }

    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    connection.execute(
        "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
         VALUES ('doctor', 'manual', ?1, ?2, 'running', '[]', '[]', '{}', 0)",
        params![started_at, timezone],
    )?;
    let run_id = connection.last_insert_rowid();

    let repair_result = (|| -> Result<HealthRepairReport> {
        let mut notes = Vec::new();
        let repaired_audit_paths =
            rewrite_import_audit_artifacts(paths, config, key, &missing_import_audits)?;
        let repaired_import_audits = repaired_audit_paths.len();
        for (batch_id, audit_path) in &repaired_audit_paths {
            connection.execute(
                "UPDATE import_batches SET audit_path = ?1 WHERE id = ?2",
                params![audit_path, batch_id],
            )?;
        }
        if repaired_import_audits > 0 {
            notes.push(format!(
                "Rebuilt {} missing import audit artifact(s).",
                repaired_import_audits
            ));
        }

        let repaired_visibility_rows = connection.execute(
            "UPDATE visits
             SET reverted_by_run_id = ?1
             WHERE reverted_at IS NOT NULL
               AND (
                 reverted_by_run_id IS NULL
                 OR reverted_by_run_id NOT IN (SELECT id FROM runs)
               )",
            [run_id],
        )?;
        if repaired_visibility_rows > 0 {
            notes.push(format!(
                "Re-linked {} reverted visit rows to doctor repair run #{}.",
                repaired_visibility_rows, run_id
            ));
        }

        let cleared_ai_embeddings = if table_exists(&connection, "ai_embeddings")? {
            connection.execute(
                "DELETE FROM ai_embeddings
                 WHERE history_id NOT IN (SELECT id FROM visit_events)",
                [],
            )?
        } else {
            0
        };
        if cleared_ai_embeddings > 0 {
            notes.push(format!(
                "Removed {} stale AI embedding rows that pointed at hidden or missing visits.",
                cleared_ai_embeddings
            ));
        }

        let cleared_insight_rows =
            if stale_insight_state > 0 { invalidate_insight_state(&connection)? } else { 0 };
        if cleared_insight_rows > 0 {
            notes.push(format!(
                "Cleared {} stale insight rows so the next insight run rebuilds from visible history only.",
                cleared_insight_rows
            ));
        }

        let cleared_derived_rows = cleared_ai_embeddings + cleared_insight_rows;
        let git_commit = if config.git_enabled && repaired_import_audits > 0 {
            git_audit::commit_all(&paths.audit_repo_path, "doctor repair import audit artifacts")?
        } else {
            None
        };
        if let Some(git_commit) = git_commit {
            for batch_id in &missing_import_audits {
                connection.execute(
                    "UPDATE import_batches SET git_commit = ?1 WHERE id = ?2",
                    params![git_commit, batch_id],
                )?;
            }
            notes.push(format!(
                "Recorded repaired import artifacts in audit commit {}.",
                git_commit
            ));
        }

        Ok(HealthRepairReport {
            run_id: Some(run_id),
            repaired_import_audits,
            repaired_visibility_rows,
            cleared_derived_rows,
            notes,
        })
    })();

    match repair_result {
        Ok(report) => {
            connection.execute(
                "UPDATE runs
                 SET finished_at = ?1,
                     status = 'success',
                     stats_json = ?2,
                     warnings_json = ?3
                 WHERE id = ?4",
                params![
                    now_rfc3339(),
                    serde_json::to_string(&json!({
                        "repairedImportAudits": report.repaired_import_audits,
                        "repairedVisibilityRows": report.repaired_visibility_rows,
                        "clearedDerivedRows": report.cleared_derived_rows,
                    }))?,
                    serde_json::to_string(&report.notes)?,
                    run_id,
                ],
            )?;
            Ok(report)
        }
        Err(error) => {
            connection.execute(
                "UPDATE runs
                 SET finished_at = ?1,
                     status = 'failed',
                     error_message = ?2
                 WHERE id = ?3",
                params![now_rfc3339(), error.to_string(), run_id],
            )?;
            Err(error)
        }
    }
}

fn select_supported_profiles<'a>(
    discovered: &'a [crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<&'a crate::models::BrowserProfile> {
    discovered
        .iter()
        .filter(|profile| profile.history_exists)
        .filter(|profile| {
            selected_profile_ids.iter().any(|selected| selected == &profile.profile_id)
        })
        .collect()
}

fn collect_skipped_profiles(
    discovered: &[crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<String> {
    let mut warnings = discovered
        .iter()
        .filter(|profile| !profile.history_exists)
        .filter(|profile| {
            selected_profile_ids.iter().any(|selected| selected == &profile.profile_id)
        })
        .map(|profile| {
            if profile.browser_family == "safari" {
                format!(
                    "Skipped `{}` because Safari History.db is not readable yet. On macOS, grant Full Disk Access before the next backup.",
                    profile.profile_id
                )
            } else {
                format!(
                    "Skipped `{}` because {} is missing or unreadable at {}.",
                    profile.profile_id, profile.history_file_name, profile.profile_path
                )
            }
        })
        .collect::<Vec<_>>();

    for selected_profile_id in selected_profile_ids {
        if !discovered.iter().any(|profile| profile.profile_id == *selected_profile_id) {
            warnings.push(format!(
                "Skipped `{selected_profile_id}` because it is no longer detected on this device."
            ));
        }
    }

    warnings
}

fn process_profile_snapshot(
    archive: &Transaction<'_>,
    run_id: i64,
    paths: &ProjectPaths,
    config: &AppConfig,
    snapshot: &ProfileSnapshot,
    snapshot_artifacts: &mut Vec<SnapshotArtifact>,
    allow_checkpoint: bool,
    use_watermark: bool,
) -> Result<BackupProfileSummary> {
    let source_profile_id = upsert_source_profile(archive, &snapshot.profile)?;
    let schema_payload = collect_schema_payload(&snapshot.history_path)?;
    let schema_string = serde_json::to_string(&schema_payload)?;
    let schema_hash = sha256_hex(schema_string.as_bytes());
    let watermark = if use_watermark {
        load_watermark(archive, &snapshot.profile.profile_id)?
    } else {
        Watermark::default()
    };
    let parsed_snapshot = parse_profile_snapshot(snapshot, config, &watermark)
        .with_context(|| format!("parsing {} staging copy", snapshot.profile.browser_name))?;

    let mut summary = BackupProfileSummary {
        profile_id: snapshot.profile.profile_id.clone(),
        notes: parsed_snapshot
            .history
            .warnings
            .iter()
            .map(|warning| warning.message.clone())
            .collect(),
        ..BackupProfileSummary::default()
    };

    let mut url_id_map = HashMap::new();
    for url in &parsed_snapshot.history.urls {
        let payload = serialize_payload(url)?;
        let canonical_url_id =
            upsert_url(archive, run_id, source_profile_id, &snapshot.profile, url, &payload.hash)?;
        url_id_map.insert(url.source_url_id, canonical_url_id);
        insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                source_profile_id,
                profile_id: &snapshot.profile.profile_id,
                source_kind: parsed_snapshot.source_kind,
                table_name: "urls",
                source_pk: &url.source_url_id.to_string(),
                payload_hash: &payload.hash,
                payload_json: &payload.json,
                schema_hash: &schema_hash,
                chrome_version: snapshot.profile.browser_version.as_deref(),
                import_batch_id: None,
            },
        )?;
        summary.new_urls += 1;
        summary.raw_rows += 1;
    }

    let mut url_bounds = HashMap::<i64, UrlVisitBounds>::new();
    for visit in &parsed_snapshot.history.visits {
        let Some(&url_id) = url_id_map.get(&visit.source_url_id) else {
            continue;
        };
        let payload = serialize_payload(visit)?;
        let inserted = insert_visit(
            archive,
            run_id,
            source_profile_id,
            &snapshot.profile.profile_id,
            url_id,
            visit,
            &payload.hash,
        )?;
        if inserted > 0 {
            summary.new_visits += 1;
        }
        track_url_visit_bounds(&mut url_bounds, url_id, visit);
        insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                source_profile_id,
                profile_id: &snapshot.profile.profile_id,
                source_kind: parsed_snapshot.source_kind,
                table_name: "visits",
                source_pk: &visit.source_visit_id.to_string(),
                payload_hash: &payload.hash,
                payload_json: &payload.json,
                schema_hash: &schema_hash,
                chrome_version: snapshot.profile.browser_version.as_deref(),
                import_batch_id: None,
            },
        )?;
        summary.raw_rows += 1;
    }

    for (url_id, bounds) in url_bounds {
        sync_url_bounds(archive, url_id, &bounds)?;
    }

    for download in &parsed_snapshot.history.downloads {
        let payload = serialize_payload(download)?;
        let inserted =
            insert_download(archive, run_id, source_profile_id, download, &payload.hash)?;
        if inserted > 0 {
            summary.new_downloads += 1;
        }
        insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                source_profile_id,
                profile_id: &snapshot.profile.profile_id,
                source_kind: parsed_snapshot.source_kind,
                table_name: "downloads",
                source_pk: &download.source_download_id.to_string(),
                payload_hash: &payload.hash,
                payload_json: &payload.json,
                schema_hash: &schema_hash,
                chrome_version: snapshot.profile.browser_version.as_deref(),
                import_batch_id: None,
            },
        )?;
        summary.raw_rows += 1;
    }

    let mut inserted_search_terms = 0usize;
    for term in &parsed_snapshot.history.search_terms {
        let Some(&url_id) = url_id_map.get(&term.url_id) else {
            continue;
        };
        inserted_search_terms += insert_search_term(
            archive,
            run_id,
            source_profile_id,
            &snapshot.profile.profile_id,
            url_id,
            term,
        )?;
    }
    if inserted_search_terms > 0 {
        summary.notes.push(format!(
            "Captured {inserted_search_terms} {} search term rows.",
            snapshot.profile.browser_name
        ));
    }

    for favicon in &parsed_snapshot.history.favicons {
        let payload = serialize_payload(favicon)?;
        insert_favicon(archive, run_id, source_profile_id, favicon, &payload.hash)?;
    }

    if allow_checkpoint && should_checkpoint(&watermark, &schema_hash, config.checkpoint_days) {
        let artifact = create_snapshot_artifact(
            archive,
            run_id,
            paths,
            snapshot,
            if watermark.last_schema_hash.as_deref() != Some(&schema_hash) {
                "source-schema-changed"
            } else {
                "periodic-checkpoint"
            },
        )?;
        snapshot_artifacts.push(artifact);
        summary.checkpoint_created = true;
    }

    save_watermark(
        archive,
        &snapshot.profile.profile_id,
        &Watermark {
            last_visit_id: parsed_snapshot.last_visit_id.max(watermark.last_visit_id),
            last_url_last_visit_time: parsed_snapshot
                .last_url_marker
                .unwrap_or(watermark.last_url_last_visit_time)
                .max(watermark.last_url_last_visit_time),
            last_download_id: parsed_snapshot
                .last_download_id
                .unwrap_or(watermark.last_download_id)
                .max(watermark.last_download_id),
            last_favicon_last_updated: parsed_snapshot
                .last_favicon_marker
                .unwrap_or(watermark.last_favicon_last_updated)
                .max(watermark.last_favicon_last_updated),
            last_checkpoint_at: if summary.checkpoint_created {
                Some(now_rfc3339())
            } else {
                watermark.last_checkpoint_at.clone()
            },
            last_schema_hash: Some(schema_hash),
            updated_at: now_rfc3339(),
        },
    )?;

    Ok(summary)
}

fn parse_profile_snapshot(
    snapshot: &ProfileSnapshot,
    config: &AppConfig,
    watermark: &Watermark,
) -> Result<ParsedProfileSnapshot> {
    match snapshot.profile.browser_family.as_str() {
        "chromium" => {
            let history = chromium::parse_history(
                &HistoryDatabaseSet {
                    history_path: snapshot.history_path.clone(),
                    favicons_path: if config.capture_favicons {
                        snapshot.favicons_path.clone()
                    } else {
                        None
                    },
                },
                ChromiumReadCursor {
                    after_visit_id: watermark.last_visit_id,
                    after_url_last_visit_time: watermark.last_url_last_visit_time,
                    after_download_id: watermark.last_download_id,
                    after_favicon_last_updated: watermark.last_favicon_last_updated,
                },
            )?;
            let last_visit_id =
                history.visits.iter().map(|visit| visit.source_visit_id).max().unwrap_or_default();
            let last_url_marker =
                history.urls.iter().map(|url| ms_to_chromium_time(url.last_visit_ms)).max();
            let last_download_id =
                history.downloads.iter().map(|download| download.source_download_id).max();
            let last_favicon_marker = history
                .favicons
                .iter()
                .map(|favicon| ms_to_chromium_time(favicon.last_updated_ms))
                .max();

            Ok(ParsedProfileSnapshot {
                source_kind: "chromium-history",
                history,
                last_visit_id,
                last_url_marker,
                last_download_id,
                last_favicon_marker,
            })
        }
        "firefox" => {
            let history = firefox::parse_history(
                &snapshot.history_path,
                watermark.last_visit_id,
                watermark.last_url_last_visit_time,
            )?;
            let last_visit_id =
                history.visits.iter().map(|visit| visit.source_visit_id).max().unwrap_or_default();
            let last_url_marker = history.urls.iter().map(|url| url.last_visit_ms).max();
            Ok(ParsedProfileSnapshot {
                source_kind: "firefox-history",
                history,
                last_visit_id,
                last_url_marker,
                last_download_id: None,
                last_favicon_marker: None,
            })
        }
        "safari" => {
            let history = safari::parse_history(
                &snapshot.history_path,
                watermark.last_visit_id,
                watermark.last_url_last_visit_time,
            )?;
            let last_visit_id =
                history.visits.iter().map(|visit| visit.source_visit_id).max().unwrap_or_default();
            let last_url_marker = history.urls.iter().map(|url| url.last_visit_ms).max();
            Ok(ParsedProfileSnapshot {
                source_kind: "safari-history",
                history,
                last_visit_id,
                last_url_marker,
                last_download_id: None,
                last_favicon_marker: None,
            })
        }
        family => anyhow::bail!("browser family `{family}` is not supported by the archive engine"),
    }
}

fn upsert_source_profile(
    archive: &Transaction<'_>,
    profile: &crate::models::BrowserProfile,
) -> Result<i64> {
    let browser_kind = profile.profile_id.split(':').next().unwrap_or(&profile.browser_family);
    archive
        .query_row(
            "INSERT INTO source_profiles (
           browser_kind,
           browser_version,
           profile_name,
           profile_path,
           discovered_at,
           enabled,
           profile_key,
           user_name,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8)
         ON CONFLICT(profile_key) DO UPDATE SET
           browser_kind = excluded.browser_kind,
           browser_version = excluded.browser_version,
           profile_name = excluded.profile_name,
           profile_path = excluded.profile_path,
           user_name = excluded.user_name,
           updated_at = excluded.updated_at,
           enabled = 1
         RETURNING id",
            params![
                browser_kind,
                profile.browser_version,
                profile.profile_name,
                profile.profile_path,
                now_rfc3339(),
                profile.profile_id,
                profile.user_name,
                now_rfc3339(),
            ],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn upsert_url(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    profile: &crate::models::BrowserProfile,
    url: &ParsedUrl,
    payload_hash: &str,
) -> Result<i64> {
    let recorded_at = now_rfc3339();
    archive
        .query_row(
            "INSERT INTO urls (
           url,
           title,
           visit_count,
           typed_count,
           first_visit_ms,
           first_visit_iso,
           last_visit_ms,
           last_visit_iso,
           source_profile_id,
           created_by_run_id,
           source_url_id,
           hidden,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(source_profile_id, source_url_id) DO UPDATE SET
           url = excluded.url,
           title = excluded.title,
           visit_count = excluded.visit_count,
           typed_count = excluded.typed_count,
           hidden = excluded.hidden,
           payload_hash = excluded.payload_hash,
           recorded_at = excluded.recorded_at,
           last_visit_ms = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_ms
             ELSE urls.last_visit_ms
           END,
           last_visit_iso = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_iso
             ELSE urls.last_visit_iso
           END
         RETURNING id",
            params![
                url.url,
                url.title,
                url.visit_count,
                url.typed_count,
                url.last_visit_ms,
                url.last_visit_iso,
                source_profile_id,
                run_id,
                url.source_url_id,
                url.hidden as i64,
                payload_hash,
                recorded_at,
            ],
            |row| row.get(0),
        )
        .with_context(|| format!("loading canonical url id for {}", profile.profile_id))
}

fn insert_visit(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    profile_id: &str,
    url_id: i64,
    visit: &ParsedVisit,
    payload_hash: &str,
) -> Result<usize> {
    archive
        .execute(
            "INSERT OR IGNORE INTO visits (
               url_id,
               source_visit_id,
               visit_time_ms,
               visit_time_iso,
               transition_type,
               visit_duration_ms,
               source_profile_id,
               created_by_run_id,
               from_visit,
               is_known_to_sync,
               visited_link_id,
               external_referrer_url,
               app_id,
               event_fingerprint,
               payload_hash,
               recorded_at,
               import_batch_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, NULL)",
            params![
                url_id,
                visit.source_visit_id.to_string(),
                visit.visit_time_ms,
                visit.visit_time_iso,
                visit.transition,
                visit.visit_duration_ms,
                source_profile_id,
                run_id,
                visit.from_visit,
                visit.is_known_to_sync as i64,
                visit.visited_link_id,
                visit.external_referrer_url,
                visit.app_id,
                visit_event_fingerprint(
                    "chromium-history",
                    &visit.url,
                    ms_to_chromium_time(visit.visit_time_ms),
                    visit.title.as_deref(),
                    visit.transition,
                    visit.app_id.as_deref(),
                ),
                payload_hash,
                now_rfc3339(),
            ],
        )
        .with_context(|| format!("inserting visit for {profile_id}"))
}

fn insert_download(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    download: &ParsedDownload,
    payload_hash: &str,
) -> Result<usize> {
    archive
        .execute(
            "INSERT OR IGNORE INTO downloads (
           source_download_id,
           guid,
           current_path,
           target_path,
           start_time_ms,
           start_time_iso,
           total_bytes,
           received_bytes,
           state,
           mime_type,
           original_mime_type,
           source_profile_id,
           created_by_run_id,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                download.source_download_id.to_string(),
                download.guid,
                download.current_path,
                download.target_path,
                download.start_time_ms,
                download.start_time_iso,
                download.total_bytes,
                download.received_bytes,
                download.state,
                download.mime_type,
                download.original_mime_type,
                source_profile_id,
                run_id,
                payload_hash,
                now_rfc3339(),
            ],
        )
        .map_err(Into::into)
}

fn insert_search_term(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    profile_id: &str,
    url_id: i64,
    term: &ParsedSearchTerm,
) -> Result<usize> {
    archive
        .execute(
            "INSERT OR IGNORE INTO search_terms (
               url_id,
               term,
               normalized_term,
               source_profile_id,
               created_by_run_id,
               profile_id,
               keyword_id,
               recorded_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                url_id,
                term.term,
                term.normalized_term,
                source_profile_id,
                run_id,
                profile_id,
                term.keyword_id,
                now_rfc3339(),
            ],
        )
        .map_err(Into::into)
}

fn insert_favicon(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    favicon: &ParsedFavicon,
    payload_hash: &str,
) -> Result<usize> {
    archive
        .execute(
            "INSERT OR IGNORE INTO favicons (
           page_url,
           icon_url,
           icon_type,
           width,
           height,
           last_updated_ms,
           last_updated_iso,
           image_data,
           source_profile_id,
           created_by_run_id,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                favicon.page_url,
                favicon.icon_url,
                favicon.icon_type,
                favicon.width,
                favicon.height,
                favicon.last_updated_ms,
                favicon.last_updated_iso,
                favicon.image_data,
                source_profile_id,
                run_id,
                payload_hash,
                now_rfc3339(),
            ],
        )
        .map_err(Into::into)
}

fn insert_raw_row(archive: &Transaction<'_>, row: RawRowInsert<'_>) -> Result<usize> {
    archive
        .execute(
            "INSERT OR IGNORE INTO raw_row_versions (
               source_profile_id,
               source_kind,
               table_name,
               source_pk,
               payload_hash,
               schema_fingerprint,
               browser_version,
               payload_json,
               recorded_at,
               run_id,
               profile_id,
               schema_hash,
               chrome_version,
               import_batch_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                row.source_profile_id,
                row.source_kind,
                row.table_name,
                row.source_pk,
                row.payload_hash,
                row.schema_hash,
                row.chrome_version,
                row.payload_json,
                now_rfc3339(),
                row.run_id,
                row.profile_id,
                row.schema_hash,
                row.chrome_version,
                row.import_batch_id,
            ],
        )
        .map_err(Into::into)
}

fn sync_url_bounds(archive: &Transaction<'_>, url_id: i64, bounds: &UrlVisitBounds) -> Result<()> {
    archive.execute(
        "UPDATE urls
         SET first_visit_ms = CASE
               WHEN ?2 < first_visit_ms THEN ?2
               ELSE first_visit_ms
             END,
         first_visit_iso = CASE
               WHEN ?2 < first_visit_ms THEN ?3
               ELSE first_visit_iso
             END,
         last_visit_ms = CASE
               WHEN ?4 > last_visit_ms THEN ?4
               ELSE last_visit_ms
             END,
         last_visit_iso = CASE
               WHEN ?4 > last_visit_ms THEN ?5
               ELSE last_visit_iso
             END
         WHERE id = ?1",
        params![
            url_id,
            bounds.first_visit_ms,
            bounds.first_visit_iso,
            bounds.last_visit_ms,
            bounds.last_visit_iso
        ],
    )?;
    Ok(())
}

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

fn load_watermark(archive: &Transaction<'_>, profile_id: &str) -> Result<Watermark> {
    archive
        .query_row(
            "SELECT
               last_visit_id,
               last_url_last_visit_time,
               last_download_id,
               last_favicon_last_updated,
               last_checkpoint_at,
               last_schema_hash,
               updated_at
             FROM profile_watermarks
             WHERE profile_id = ?1",
            [profile_id],
            |row| {
                Ok(Watermark {
                    last_visit_id: row.get(0)?,
                    last_url_last_visit_time: row.get(1)?,
                    last_download_id: row.get(2)?,
                    last_favicon_last_updated: row.get(3)?,
                    last_checkpoint_at: row.get(4)?,
                    last_schema_hash: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map(|value| {
            value.unwrap_or_else(|| Watermark { updated_at: now_rfc3339(), ..Watermark::default() })
        })
        .map_err(Into::into)
}

fn save_watermark(
    archive: &Transaction<'_>,
    profile_id: &str,
    watermark: &Watermark,
) -> Result<()> {
    archive.execute(
        "INSERT INTO profile_watermarks (
           profile_id,
           last_visit_id,
           last_url_last_visit_time,
           last_download_id,
           last_favicon_last_updated,
           last_checkpoint_at,
           last_schema_hash,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(profile_id) DO UPDATE SET
           last_visit_id = excluded.last_visit_id,
           last_url_last_visit_time = excluded.last_url_last_visit_time,
           last_download_id = excluded.last_download_id,
           last_favicon_last_updated = excluded.last_favicon_last_updated,
           last_checkpoint_at = excluded.last_checkpoint_at,
           last_schema_hash = excluded.last_schema_hash,
           updated_at = excluded.updated_at",
        params![
            profile_id,
            watermark.last_visit_id,
            watermark.last_url_last_visit_time,
            watermark.last_download_id,
            watermark.last_favicon_last_updated,
            watermark.last_checkpoint_at,
            watermark.last_schema_hash,
            watermark.updated_at,
        ],
    )?;
    Ok(())
}

fn should_checkpoint(watermark: &Watermark, schema_hash: &str, checkpoint_days: u64) -> bool {
    if watermark.last_schema_hash.as_deref() != Some(schema_hash) {
        return true;
    }
    let Some(last_checkpoint_at) = &watermark.last_checkpoint_at else {
        return true;
    };
    let Ok(last_checkpoint_at) = DateTime::parse_from_rfc3339(last_checkpoint_at) else {
        return true;
    };
    Utc::now() - last_checkpoint_at.with_timezone(&Utc) > Duration::days(checkpoint_days as i64)
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
            Ok(connection) => {
                create_schema(&connection)?;
                connection
                    .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get::<_, i64>(0))
                    .unwrap_or_default()
                    .max(0) as usize
            }
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
    let raw_rows: i64 =
        connection.query_row("SELECT COUNT(*) FROM raw_row_versions", [], |row| row.get(0))?;
    let manifests: i64 =
        connection.query_row("SELECT COUNT(*) FROM manifests", [], |row| row.get(0))?;
    let snapshots: i64 =
        connection.query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))?;
    Ok(json!({
        "urls": urls,
        "visits": visits,
        "downloads": downloads,
        "rawRows": raw_rows,
        "manifests": manifests,
        "snapshots": snapshots,
    }))
}

fn storage_summary(paths: &ProjectPaths) -> StorageSummary {
    StorageSummary {
        archive_database_bytes: file_size(&paths.archive_database_path),
        manifest_bytes: directory_size(&paths.manifests_dir),
        snapshot_bytes: directory_size(&paths.raw_snapshots_dir),
        export_bytes: directory_size(&paths.exports_dir),
        staging_bytes: directory_size(&paths.staging_dir),
        quarantine_bytes: directory_size(&paths.quarantine_dir),
    }
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or_default()
}

fn directory_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    let mut total = 0;
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += directory_size(&path);
        } else {
            total += file_size(&path);
        }
    }
    total
}

fn backup_run_overview_from_row(row: &Row<'_>) -> rusqlite::Result<BackupRunOverview> {
    let profile_scope = decode_profile_scope(row.get::<_, Option<String>>(6)?.as_deref());
    let summary = decode_run_stats(row.get::<_, Option<String>>(8)?.as_deref());
    Ok(BackupRunOverview {
        id: row.get(0)?,
        started_at: row.get(1)?,
        finished_at: row.get(2)?,
        status: row.get(3)?,
        run_type: row.get(4)?,
        trigger: row.get(5)?,
        profile_scope: profile_scope.clone(),
        manifest_hash: row.get(7)?,
        profiles_processed: run_profiles_processed(&summary, &profile_scope),
        new_visits: run_new_visits(&summary),
        new_urls: run_new_urls(&summary),
        new_downloads: run_new_downloads(&summary),
    })
}

fn decode_profile_scope(value: Option<&str>) -> Vec<String> {
    value.and_then(|content| serde_json::from_str::<Vec<String>>(content).ok()).unwrap_or_default()
}

fn decode_run_stats(value: Option<&str>) -> Value {
    value
        .and_then(|content| serde_json::from_str::<Value>(content).ok())
        .unwrap_or_else(|| json!({}))
}

fn run_stat_count(stats: &Value, keys: &[&str]) -> usize {
    keys.iter().find_map(|key| stats.get(*key).and_then(Value::as_u64)).unwrap_or(0) as usize
}

fn run_profiles_processed(stats: &Value, profile_scope: &[String]) -> usize {
    let explicit = run_stat_count(stats, &["profilesProcessed"]);
    if explicit > 0 { explicit } else { profile_scope.len() }
}

fn run_new_visits(stats: &Value) -> usize {
    run_stat_count(
        stats,
        &[
            "newVisits",
            "importedItems",
            "softHiddenVisits",
            "restoredVisits",
            "repairedVisibilityRows",
        ],
    )
}

fn run_new_urls(stats: &Value) -> usize {
    run_stat_count(stats, &["newUrls"])
}

fn run_new_downloads(stats: &Value) -> usize {
    run_stat_count(stats, &["newDownloads"])
}

fn history_entry_from_row(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let url: String = row.get(2)?;
    let source_visit_id = row
        .get::<_, Option<String>>(7)?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    Ok(HistoryEntry {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        domain: url_domain(&url),
        url,
        title: row.get(3)?,
        visited_at: row.get(4).map(|ms: i64| {
            DateTime::<Utc>::from_timestamp_millis(ms).unwrap_or_else(Utc::now).to_rfc3339()
        })?,
        visit_time: row.get(4)?,
        duration_ms: row.get(5)?,
        transition: row.get(6)?,
        source_visit_id,
        app_id: row.get(8)?,
    })
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
    Ok(SerializedPayload { json, hash })
}

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

fn load_cached_archive_totals(connection: &Connection) -> Result<Option<ArchiveVisibleTotals>> {
    let mut statement = connection.prepare(
        "SELECT stats_json
         FROM runs
         WHERE status = 'success'
         ORDER BY id DESC
         LIMIT 24",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    for row in rows {
        let stats_json = row?;
        let Ok(stats) = serde_json::from_str::<Value>(&stats_json) else {
            continue;
        };
        if let Some(totals) = archive_totals_from_stats(&stats) {
            return Ok(Some(totals));
        }
    }
    Ok(None)
}

fn archive_totals_from_stats(stats: &Value) -> Option<ArchiveVisibleTotals> {
    Some(ArchiveVisibleTotals {
        total_profiles: stats.get("totalProfiles")?.as_u64()? as usize,
        total_urls: stats.get("totalUrls")?.as_u64()? as usize,
        total_visits: stats.get("totalVisits")?.as_u64()? as usize,
        total_downloads: stats.get("totalDownloads")?.as_u64()? as usize,
    })
}

fn track_url_visit_bounds(
    url_bounds: &mut HashMap<i64, UrlVisitBounds>,
    url_id: i64,
    visit: &ParsedVisit,
) {
    url_bounds
        .entry(url_id)
        .and_modify(|bounds| {
            if visit.visit_time_ms < bounds.first_visit_ms {
                bounds.first_visit_ms = visit.visit_time_ms;
                bounds.first_visit_iso = visit.visit_time_iso.clone();
            }
            if visit.visit_time_ms > bounds.last_visit_ms {
                bounds.last_visit_ms = visit.visit_time_ms;
                bounds.last_visit_iso = visit.visit_time_iso.clone();
            }
        })
        .or_insert_with(|| UrlVisitBounds {
            first_visit_ms: visit.visit_time_ms,
            first_visit_iso: visit.visit_time_iso.clone(),
            last_visit_ms: visit.visit_time_ms,
            last_visit_iso: visit.visit_time_iso.clone(),
        });
}

fn snapshot_source_hashes(snapshot: &ProfileSnapshot) -> BTreeMap<String, String> {
    snapshot
        .source_hashes
        .iter()
        .map(|fingerprint| (fingerprint.path.clone(), fingerprint.sha256.clone()))
        .collect()
}

fn current_timezone_name() -> String {
    get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

fn check_manifest_chain(connection: &Connection) -> Result<HealthCheck> {
    let mut statement = connection.prepare(
        "SELECT id, parent_manifest_id, content_hash, file_path
         FROM manifests
         ORDER BY id ASC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<i64>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    let mut previous_id = None;
    let mut previous_hash = None::<String>;
    for row in rows {
        let (id, parent_id, hash, file_path) = row?;
        if previous_id.is_some() && parent_id != previous_id {
            return Ok(HealthCheck {
                name: "Manifest chain".to_string(),
                ok: false,
                detail: format!("manifest {id} does not point to the previous manifest id"),
            });
        }
        if let Some(path) = file_path {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("reading manifest artifact {}", path))?;
            let recalculated = sha256_hex(content.as_bytes());
            if recalculated != hash {
                return Ok(HealthCheck {
                    name: "Manifest chain".to_string(),
                    ok: false,
                    detail: format!("manifest hash mismatch at run artifact {}", path),
                });
            }
        }
        previous_id = Some(id);
        previous_hash = Some(hash);
    }

    Ok(HealthCheck {
        name: "Manifest chain".to_string(),
        ok: true,
        detail: previous_hash.unwrap_or_else(|| "No manifest artifacts recorded yet.".to_string()),
    })
}

fn check_snapshot_files(connection: &Connection) -> Result<HealthCheck> {
    let missing = connection
        .query_row(
            "SELECT file_path
             FROM snapshots
             WHERE file_path IS NOT NULL
             ORDER BY id DESC",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .filter(|path| !Path::new(path).exists());

    Ok(match missing {
        Some(path) => HealthCheck {
            name: "Snapshot artifacts".to_string(),
            ok: false,
            detail: format!("missing snapshot artifact {}", path),
        },
        None => HealthCheck {
            name: "Snapshot artifacts".to_string(),
            ok: true,
            detail: "All recorded snapshot artifacts are present.".to_string(),
        },
    })
}

fn check_import_audit_artifacts(connection: &Connection) -> Result<HealthCheck> {
    let mut statement = connection.prepare(
        "SELECT id, audit_path
         FROM import_batches
         ORDER BY id DESC",
    )?;
    let mut rows = statement.query([])?;
    let mut missing = None;
    while let Some(row) = rows.next()? {
        let batch_id = row.get::<_, i64>(0)?;
        let audit_path = row.get::<_, Option<String>>(1)?;
        match audit_path {
            Some(path) if !path.is_empty() && Path::new(&path).exists() => continue,
            other => {
                missing = Some((batch_id, other));
                break;
            }
        }
    }

    Ok(match missing {
        Some((batch_id, Some(path))) => HealthCheck {
            name: "Import audit artifacts".to_string(),
            ok: false,
            detail: format!("import batch {batch_id} points to a missing audit artifact at {path}"),
        },
        Some((batch_id, None)) => HealthCheck {
            name: "Import audit artifacts".to_string(),
            ok: false,
            detail: format!("import batch {batch_id} does not have an audit artifact yet"),
        },
        None => HealthCheck {
            name: "Import audit artifacts".to_string(),
            ok: true,
            detail: "All recorded import batches have readable audit artifacts.".to_string(),
        },
    })
}

fn check_broken_visibility(connection: &Connection) -> Result<HealthCheck> {
    let broken_visibility: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM visits
         LEFT JOIN runs
           ON runs.id = visits.reverted_by_run_id
         WHERE visits.reverted_at IS NOT NULL
           AND (visits.reverted_by_run_id IS NULL OR runs.id IS NULL)",
        [],
        |row| row.get(0),
    )?;

    Ok(if broken_visibility > 0 {
        HealthCheck {
            name: "Broken visibility references".to_string(),
            ok: false,
            detail: format!(
                "{broken_visibility} reverted visit rows are missing the rollback run that should explain their hidden state"
            ),
        }
    } else {
        HealthCheck {
            name: "Broken visibility references".to_string(),
            ok: true,
            detail: "All hidden visit rows still point at a valid rollback run.".to_string(),
        }
    })
}

fn check_stale_derived_state(connection: &Connection) -> Result<HealthCheck> {
    let mut stale_details = Vec::new();

    if table_exists(connection, "ai_embeddings")? {
        let stale_embeddings: i64 = connection.query_row(
            "SELECT COUNT(*)
             FROM ai_embeddings
             WHERE history_id NOT IN (SELECT id FROM visit_events)",
            [],
            |row| row.get(0),
        )?;
        if stale_embeddings > 0 {
            stale_details.push(format!("{stale_embeddings} stale AI embeddings"));
        }
    }

    if table_exists(connection, "insight_thread_members")? {
        let stale_members: i64 = connection.query_row(
            "SELECT COUNT(*)
             FROM insight_thread_members
             WHERE history_id NOT IN (SELECT id FROM visit_events)",
            [],
            |row| row.get(0),
        )?;
        if stale_members > 0 {
            stale_details.push(format!("{stale_members} stale insight thread members"));
        }
    }

    if table_exists(connection, "visit_insight_features")? {
        let stale_features: i64 = connection.query_row(
            "SELECT COUNT(*)
             FROM visit_insight_features
             WHERE history_id NOT IN (SELECT id FROM visit_events)",
            [],
            |row| row.get(0),
        )?;
        if stale_features > 0 {
            stale_details.push(format!("{stale_features} stale insight feature rows"));
        }
    }

    Ok(if stale_details.is_empty() {
        HealthCheck {
            name: "Derived state freshness".to_string(),
            ok: true,
            detail: "Derived AI and insight tables match the visible visit set.".to_string(),
        }
    } else {
        HealthCheck {
            name: "Derived state freshness".to_string(),
            ok: false,
            detail: stale_details.join(", "),
        }
    })
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table_name],
        |row| row.get::<_, i64>(0),
    )? > 0)
}

fn missing_import_audit_batches(connection: &Connection) -> Result<Vec<i64>> {
    let mut statement = connection.prepare(
        "SELECT id, audit_path
         FROM import_batches
         ORDER BY id ASC",
    )?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)))?;

    let mut batch_ids = Vec::new();
    for row in rows {
        let (batch_id, audit_path) = row?;
        match audit_path {
            Some(path) if !path.is_empty() && Path::new(&path).exists() => {}
            _ => batch_ids.push(batch_id),
        }
    }
    Ok(batch_ids)
}

fn rewrite_import_audit_artifacts(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    batch_ids: &[i64],
) -> Result<Vec<(i64, String)>> {
    if batch_ids.is_empty() {
        return Ok(Vec::new());
    }

    git_audit::ensure_repo(&paths.audit_repo_path)?;
    let mut rewritten = Vec::new();
    for batch_id in batch_ids {
        let detail = crate::takeout::preview_import_batch(paths, config, key, *batch_id)?;
        let action = match detail.batch.status.as_str() {
            "reverted" => "reverted",
            _ => "imported",
        };
        let file_name = format!(
            "imports/{}/batch-{}-{}.json",
            &detail.batch.created_at[0..10],
            detail.batch.id,
            action
        );
        let contents = serde_json::to_string_pretty(&detail)?;
        let audit_path =
            git_audit::write_audit_file(&paths.audit_repo_path, &file_name, &contents)?;
        rewritten.push((*batch_id, audit_path.display().to_string()));
    }
    Ok(rewritten)
}

fn invalidate_insight_state(connection: &Connection) -> Result<usize> {
    let mut cleared_rows = 0usize;
    for table_name in [
        "insight_cards",
        "insight_reference_pages",
        "insight_source_effectiveness",
        "insight_query_group_members",
        "insight_query_groups",
        "insight_bursts",
        "insight_thread_members",
        "insight_threads",
        "insight_topics",
        "visit_insight_features",
        "insight_runs",
    ] {
        if table_exists(connection, table_name)? {
            cleared_rows += connection
                .execute(&format!("DELETE FROM {table_name}"), [])
                .with_context(|| format!("clearing stale derived table {table_name}"))?;
        }
    }
    crate::intelligence_runtime::ensure_intelligence_runtime_schema(connection)?;
    crate::intelligence_runtime::mark_all_deterministic_modules_stale(
        connection,
        "Archive visibility or rollback state changed after the last deterministic rebuild.",
    )?;
    Ok(cleared_rows)
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

fn ms_to_chromium_time(value_ms: i64) -> i64 {
    unix_micros_to_chrome_time(value_ms.saturating_mul(1_000))
}

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

fn render_export_content(results: &HistoryQueryResponse, format: &ExportFormat) -> Result<String> {
    Ok(match format {
        ExportFormat::Html => render_html_export(results),
        ExportFormat::Markdown => render_markdown_export(results),
        ExportFormat::Text => render_text_export(results),
        ExportFormat::Jsonl => results
            .items
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n"),
    })
}

fn render_html_export(results: &HistoryQueryResponse) -> String {
    let body = results
        .items
        .iter()
        .map(|item| {
            format!(
                "<article><h2>{}</h2><p><a href=\"{url}\">{url}</a></p><p>{}</p></article>",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.visited_at,
                url = item.url,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("<html><body>{body}</body></html>")
}

fn render_markdown_export(results: &HistoryQueryResponse) -> String {
    results
        .items
        .iter()
        .map(|item| {
            format!(
                "- [{}]({}) — {}",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.url,
                item.visited_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_text_export(results: &HistoryQueryResponse) -> String {
    results
        .items
        .iter()
        .map(|item| {
            format!(
                "{}\n{}\n{}\n",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.url,
                item.visited_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{ProjectPaths, project_paths_with_root},
        models::{ArchiveMode, RetentionPruneRequest, SnapshotRestoreRequest, TakeoutRequest},
        utils::{restore_test_env_var, test_env_lock},
    };
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn sample_paths(root: &Path) -> ProjectPaths {
        project_paths_with_root(root)
    }

    fn seed_chrome_fixture(root: &Path) -> PathBuf {
        let chrome_root = root.join("chrome-user-data");
        let profile_dir = chrome_root.join("Default");
        fs::create_dir_all(&profile_dir).expect("create profile dir");
        fs::write(chrome_root.join("Last Version"), "146.0.0.0").expect("write version");
        fs::write(
            chrome_root.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"tim@example.com"}}}}"#,
        )
        .expect("write local state");

        let history = Connection::open(profile_dir.join("History")).expect("open history");
        history
            .execute_batch(
                "CREATE TABLE urls (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER NOT NULL,
                   typed_count INTEGER NOT NULL,
                   last_visit_time INTEGER NOT NULL,
                   hidden INTEGER NOT NULL
                 );
                 CREATE TABLE visits (
                   id INTEGER PRIMARY KEY,
                   url INTEGER NOT NULL,
                   visit_time INTEGER NOT NULL,
                   from_visit INTEGER,
                   transition INTEGER,
                   visit_duration INTEGER,
                   is_known_to_sync INTEGER,
                   visited_link_id INTEGER,
                   external_referrer_url TEXT,
                   app_id TEXT
                 );
                 CREATE TABLE downloads (
                   id INTEGER PRIMARY KEY,
                   guid TEXT,
                   current_path TEXT,
                   target_path TEXT,
                   start_time INTEGER,
                   received_bytes INTEGER,
                   total_bytes INTEGER,
                   state INTEGER,
                   mime_type TEXT,
                   original_mime_type TEXT
                 );
                 CREATE TABLE keyword_search_terms (
                   keyword_id INTEGER,
                   url_id INTEGER,
                   term TEXT,
                   normalized_term TEXT
                 );",
            )
            .expect("create history tables");
        let first_visit = crate::utils::iso_to_chrome_time_micros("2026-04-05T10:00:00+00:00")
            .expect("first visit time");
        let second_visit = crate::utils::iso_to_chrome_time_micros("2026-04-05T11:00:00+00:00")
            .expect("second visit time");
        history
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (1, 'https://example.com/archive', 'Archive docs', 2, 0, ?1, 0)",
                [second_visit],
            )
            .expect("insert url");
        history
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES
                 (1, 1, ?1, NULL, 805306368, 24000, 1, NULL, 'https://google.com', NULL),
                 (2, 1, ?2, 1, 805306368, 12000, 1, NULL, NULL, NULL)",
                params![first_visit, second_visit],
            )
            .expect("insert visits");
        history
            .execute(
                "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
                 VALUES (9, 'guid-9', '/tmp/archive.pdf', '/tmp/archive.pdf', ?1, 10, 10, 1, 'application/pdf', 'application/pdf')",
                [second_visit],
            )
            .expect("insert download");
        history
            .execute(
                "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
                 VALUES (1, 1, 'deep recall token', 'deep recall token')",
                [],
            )
            .expect("insert search term");

        let favicons = Connection::open(profile_dir.join("Favicons")).expect("open favicons");
        favicons
            .execute_batch(
                "CREATE TABLE favicons (id INTEGER PRIMARY KEY, url TEXT NOT NULL, icon_type INTEGER);
                 CREATE TABLE icon_mapping (page_url TEXT NOT NULL, icon_id INTEGER NOT NULL);
                 CREATE TABLE favicon_bitmaps (icon_id INTEGER NOT NULL, width INTEGER, height INTEGER, last_updated INTEGER, image_data BLOB);",
            )
            .expect("create favicons tables");
        favicons
            .execute(
                "INSERT INTO favicons (id, url, icon_type) VALUES (1, 'https://example.com/favicon.ico', 1)",
                [],
            )
            .expect("insert favicon");
        favicons
            .execute(
                "INSERT INTO icon_mapping (page_url, icon_id) VALUES ('https://example.com/archive', 1)",
                [],
            )
            .expect("insert icon mapping");
        favicons
            .execute(
                "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
                 VALUES (1, 16, 16, ?1, X'0102')",
                [second_visit],
            )
            .expect("insert favicon bitmap");

        chrome_root
    }

    fn seed_firefox_fixture(root: &Path) -> PathBuf {
        let firefox_root = root.join("firefox");
        let profiles_dir = firefox_root.join("Profiles");
        let profile_dir = profiles_dir.join("abcd.default-release");
        fs::create_dir_all(&profile_dir).expect("create firefox profile dir");
        fs::write(
            firefox_root.join("profiles.ini"),
            "[Profile0]\nName=Work Firefox\nPath=abcd.default-release\nIsRelative=1\n",
        )
        .expect("write firefox profiles.ini");

        let history = Connection::open(profile_dir.join("places.sqlite")).expect("open firefox db");
        history
            .execute_batch(
                "CREATE TABLE moz_places (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL,
                   title TEXT,
                   visit_count INTEGER,
                   hidden INTEGER,
                   last_visit_date INTEGER
                 );
                 CREATE TABLE moz_historyvisits (
                   id INTEGER PRIMARY KEY,
                   place_id INTEGER NOT NULL,
                   visit_date INTEGER NOT NULL,
                   from_visit INTEGER,
                   visit_type INTEGER
                 );",
            )
            .expect("create firefox tables");
        history
            .execute(
                "INSERT INTO moz_places (id, url, title, visit_count, hidden, last_visit_date)
                 VALUES (1, 'https://example.com/firefox', 'Firefox docs', 1, 0, 1744146000000000)",
                [],
            )
            .expect("insert firefox place");
        history
            .execute(
                "INSERT INTO moz_historyvisits (id, place_id, visit_date, from_visit, visit_type)
                 VALUES (1, 1, 1744146000000000, NULL, 1)",
                [],
            )
            .expect("insert firefox visit");

        profiles_dir
    }

    fn seed_safari_fixture(root: &Path) -> PathBuf {
        let safari_root = root.join("Safari");
        fs::create_dir_all(&safari_root).expect("create safari root");
        let history = Connection::open(safari_root.join("History.db")).expect("open safari db");
        history
            .execute_batch(
                "CREATE TABLE history_items (
                   id INTEGER PRIMARY KEY,
                   url TEXT NOT NULL
                 );
                 CREATE TABLE history_visits (
                   id INTEGER PRIMARY KEY,
                   history_item INTEGER NOT NULL,
                   title TEXT,
                   visit_time REAL NOT NULL
                 );",
            )
            .expect("create safari tables");
        history
            .execute(
                "INSERT INTO history_items (id, url) VALUES (1, 'https://example.com/safari')",
                [],
            )
            .expect("insert safari item");
        history
            .execute(
                "INSERT INTO history_visits (id, history_item, title, visit_time)
                 VALUES (1, 1, 'Safari docs', 765838800.0)",
                [],
            )
            .expect("insert safari visit");
        safari_root
    }

    fn seed_takeout_fixture(root: &Path) -> PathBuf {
        let source_dir = root.join("takeout-source");
        fs::create_dir_all(&source_dir).expect("create takeout dir");
        fs::write(
            source_dir.join("entries.jsonl"),
            r#"{"url":"https://example.com/import","title":"Imported","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
        )
        .expect("write takeout fixture");
        source_dir
    }

    #[test]
    fn canonical_backup_pipeline_writes_runs_manifests_snapshots_and_queries() {
        let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempdir().expect("tempdir");
        let chrome_root = seed_chrome_fixture(dir.path());
        let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
        unsafe {
            std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
        }

        let paths = sample_paths(dir.path());
        let config = AppConfig {
            initialized: true,
            selected_profile_ids: vec!["chrome:Default".to_string()],
            ..AppConfig::default()
        };

        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let mut progress_events = Vec::new();
        let report = run_backup_with_progress(&paths, &config, None, false, |event| {
            progress_events.push(event);
        })
        .expect("run backup");
        assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
        assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
        assert_eq!(report.run.as_ref().expect("run").new_downloads, 1);
        assert!(report.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));
        assert!(report.profiles[0].checkpoint_created);
        assert!(progress_events.iter().any(|event| event.phase == "prepare"));
        assert!(progress_events.iter().any(|event| event.phase == "stage-profile"));
        assert!(progress_events.iter().any(|event| event.phase == "ingest-profile"));
        assert!(progress_events.iter().any(|event| event.phase == "finalize"));

        let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
        assert!(!recent_runs.is_empty());
        assert!(recent_runs.iter().any(|run| run.run_type == "backup" && run.status == "success"));

        let history = list_history(
            &paths,
            &config,
            None,
            HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
        )
        .expect("list history");
        assert_eq!(history.total, 2);

        let search_term_history = list_history(
            &paths,
            &config,
            None,
            HistoryQuery { q: Some("deep recall".to_string()), ..HistoryQuery::default() },
        )
        .expect("list search term history");
        assert_eq!(search_term_history.total, 2);

        let url_fragment_history = list_history(
            &paths,
            &config,
            None,
            HistoryQuery { q: Some("example.com/archive".to_string()), ..HistoryQuery::default() },
        )
        .expect("list url fragment history");
        assert_eq!(url_fragment_history.total, 2);

        let regex_history = list_history(
            &paths,
            &config,
            None,
            HistoryQuery {
                q: Some("archive\\sdocs".to_string()),
                regex_mode: Some(true),
                ..HistoryQuery::default()
            },
        )
        .expect("regex history");
        assert_eq!(regex_history.total, 2);

        let invalid_regex = list_history(
            &paths,
            &config,
            None,
            HistoryQuery {
                q: Some("archive(".to_string()),
                regex_mode: Some(true),
                ..HistoryQuery::default()
            },
        )
        .expect_err("invalid regex");
        assert!(
            format!("{invalid_regex:#}").contains("invalid regex pattern"),
            "unexpected error: {invalid_regex:#}"
        );

        let first_page = list_history(
            &paths,
            &config,
            None,
            HistoryQuery { limit: Some(1), ..HistoryQuery::default() },
        )
        .expect("first history page");
        assert_eq!(first_page.total, 2);
        assert_eq!(first_page.items.len(), 1);
        assert!(first_page.next_cursor.is_some());

        let second_page = list_history(
            &paths,
            &config,
            None,
            HistoryQuery {
                limit: Some(1),
                cursor: first_page.next_cursor.clone(),
                ..HistoryQuery::default()
            },
        )
        .expect("second history page");
        assert_eq!(second_page.total, 2);
        assert_eq!(second_page.items.len(), 1);
        assert!(second_page.next_cursor.is_none());

        let report_again = run_backup(&paths, &config, None, false).expect("rerun backup");
        assert_eq!(report_again.run.as_ref().expect("run").new_visits, 0);

        let connection = Connection::open(&paths.archive_database_path).expect("open archive");
        let mut statement = connection
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT visits.id
                 FROM visits
                 JOIN urls ON urls.id = visits.url_id
                 JOIN source_profiles ON source_profiles.id = visits.source_profile_id
                 JOIN history_search ON history_search.rowid = urls.id
                 WHERE visits.reverted_at IS NULL
                   AND history_search MATCH ?1",
            )
            .expect("prepare query plan");
        let plan = statement
            .query_map(["\"deep\"*"], |row| row.get::<_, String>(3))
            .expect("query plan rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect query plan");
        assert!(
            plan.iter().any(|detail| detail.contains("VIRTUAL TABLE INDEX")),
            "unexpected query plan: {plan:?}"
        );

        restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
    }

    #[test]
    fn multi_browser_backup_ingests_firefox_and_safari_history() {
        let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempdir().expect("tempdir");
        let firefox_profiles = seed_firefox_fixture(dir.path());
        let safari_root = seed_safari_fixture(dir.path());
        let original_firefox = std::env::var_os("CHB_FIREFOX_PROFILES_DIR");
        let original_safari = std::env::var_os("CHB_SAFARI_ROOT");
        unsafe {
            std::env::set_var("CHB_FIREFOX_PROFILES_DIR", &firefox_profiles);
            std::env::set_var("CHB_SAFARI_ROOT", &safari_root);
        }

        let paths = sample_paths(dir.path());
        let config = AppConfig {
            initialized: true,
            selected_profile_ids: vec![
                "firefox:abcd.default-release".to_string(),
                "safari:default".to_string(),
            ],
            ..AppConfig::default()
        };

        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let report = run_backup(&paths, &config, None, false).expect("run multi-browser backup");
        assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
        assert_eq!(report.run.as_ref().expect("run").new_urls, 2);
        assert_eq!(report.profiles.len(), 2);
        assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("firefox:")));
        assert!(report.profiles.iter().any(|profile| profile.profile_id.starts_with("safari:")));
        assert!(report.warnings.iter().any(|warning| warning.contains("Firefox baseline ingest")));
        assert!(report.warnings.iter().any(|warning| warning.contains("Safari baseline ingest")));

        let history =
            list_history(&paths, &config, None, HistoryQuery::default()).expect("history");
        assert_eq!(history.total, 2);
        assert!(history.items.iter().any(|entry| entry.profile_id.starts_with("firefox:")));
        assert!(history.items.iter().any(|entry| entry.profile_id.starts_with("safari:")));

        let rerun = run_backup(&paths, &config, None, false).expect("rerun multi-browser backup");
        assert_eq!(rerun.run.as_ref().expect("rerun").new_visits, 0);
        assert_eq!(rerun.run.as_ref().expect("rerun").new_urls, 0);

        restore_test_env_var("CHB_FIREFOX_PROFILES_DIR", original_firefox.as_deref());
        restore_test_env_var("CHB_SAFARI_ROOT", original_safari.as_deref());
    }

    #[test]
    fn doctor_detects_missing_snapshot_artifacts() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = AppConfig { initialized: true, ..AppConfig::default() };
        ensure_paths(&paths).expect("ensure paths");
        let connection = Connection::open(&paths.archive_database_path).expect("open archive");
        create_schema(&connection).expect("schema");
        connection
            .execute(
                "INSERT INTO snapshots (run_id, file_path, file_size, checksum, reason, created_at)
                 VALUES (0, ?1, 0, 'missing', 'test', ?2)",
                params![dir.path().join("missing").display().to_string(), now_rfc3339()],
            )
            .expect("insert missing snapshot");

        let report = doctor(&paths, &config, None).expect("doctor");
        assert!(report.checks.iter().any(|check| check.name == "Snapshot artifacts" && !check.ok));
    }

    #[test]
    fn doctor_repair_restores_missing_import_artifacts_visibility_and_derived_state() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = AppConfig { initialized: true, git_enabled: false, ..AppConfig::default() };
        ensure_archive_initialized(&paths, &config, None).expect("init archive");

        let takeout_source = seed_takeout_fixture(dir.path());
        let inspection = crate::takeout::import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
        )
        .expect("import takeout");
        let batch = inspection.import_batch.expect("batch");
        let audit_path = batch.audit_path.expect("audit path");
        fs::remove_file(&audit_path).expect("remove import audit artifact");

        let connection = Connection::open(&paths.archive_database_path).expect("open archive");
        create_schema(&connection).expect("schema");
        connection
            .execute(
                "UPDATE visits SET reverted_at = ?1, reverted_by_run_id = NULL WHERE import_batch_id = ?2",
                params![now_rfc3339(), batch.id],
            )
            .expect("break visibility");
        connection
            .execute(
                "INSERT INTO ai_embeddings
                 (history_id, profile_id, url, title, domain, visited_at, content, content_hash, provider_id, model, embedding_json, dimensions, indexed_at)
                 VALUES (999, 'takeout::browser-history', 'https://example.com/import', 'Imported', 'example.com', ?1, 'Imported', 'hash', 'provider', 'model', '[0.1]', 1, ?1)",
                [now_rfc3339()],
            )
            .expect("insert stale ai embedding");
        connection
            .execute(
                "INSERT INTO insight_thread_members (thread_id, history_id, ordinal, visited_at)
                 VALUES ('thread-1', 999, 0, ?1)",
                [now_rfc3339()],
            )
            .expect("insert stale insight member");
        connection
            .execute(
                "INSERT INTO visit_insight_features
                 (history_id, profile_id, topic_id, thread_id, page_type, source_role, query_term, query_stage, novelty_score, importance_score, explore_score, keywords_json, entities_json, updated_at, pipeline_version)
                 VALUES (999, 'takeout::browser-history', 'topic', 'thread-1', 'doc', 'research', NULL, NULL, 0.1, 0.2, 0.3, '[]', '[]', ?1, 'test-pipeline')",
                [now_rfc3339()],
            )
            .expect("insert stale insight feature");

        let report = doctor(&paths, &config, None).expect("doctor before repair");
        assert!(
            report.checks.iter().any(|check| check.name == "Import audit artifacts" && !check.ok)
        );
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.name == "Broken visibility references" && !check.ok)
        );
        assert!(
            report.checks.iter().any(|check| check.name == "Derived state freshness" && !check.ok)
        );

        let repair = repair_health_issues(&paths, &config, None).expect("repair health");
        assert!(repair.run_id.is_some());
        assert_eq!(repair.repaired_import_audits, 1);
        assert_eq!(repair.repaired_visibility_rows, 1);
        assert!(repair.cleared_derived_rows >= 2);

        let repaired_report = doctor(&paths, &config, None).expect("doctor after repair");
        assert!(repaired_report.checks.iter().all(|check| check.ok));
    }

    #[test]
    fn dashboard_snapshot_tracks_cached_totals_across_import_visibility_changes() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = AppConfig { initialized: true, ..AppConfig::default() };
        ensure_archive_initialized(&paths, &config, None).expect("init archive");

        let takeout_source = seed_takeout_fixture(dir.path());
        let inspection = crate::takeout::import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: takeout_source.display().to_string(), dry_run: false },
        )
        .expect("import takeout");
        let batch_id = inspection.import_batch.expect("batch").id;

        let dashboard_after_import =
            load_dashboard_snapshot(&paths, &config, None).expect("dashboard after import");
        assert_eq!(dashboard_after_import.total_visits, 1);
        assert_eq!(dashboard_after_import.total_urls, 1);
        let visible_after_import = list_history(
            &paths,
            &config,
            None,
            HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
        )
        .expect("query after import");
        assert_eq!(visible_after_import.total, 1);

        let after_import_stats: Value = Connection::open(&paths.archive_database_path)
            .expect("open archive")
            .query_row(
                "SELECT stats_json
                 FROM runs
                 WHERE run_type = 'import'
                 ORDER BY id DESC
                 LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .map(|value| serde_json::from_str(&value).expect("parse import stats"))
            .expect("load import stats");
        assert_eq!(after_import_stats["totalVisits"], 1);

        crate::takeout::revert_import_batch(&paths, &config, None, batch_id)
            .expect("revert import batch");
        let dashboard_after_revert =
            load_dashboard_snapshot(&paths, &config, None).expect("dashboard after revert");
        assert_eq!(dashboard_after_revert.total_visits, 0);
        assert_eq!(dashboard_after_revert.total_urls, 1);
        let hidden_after_revert = list_history(
            &paths,
            &config,
            None,
            HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
        )
        .expect("query after revert");
        assert_eq!(hidden_after_revert.total, 0);

        let after_revert_stats: Value = Connection::open(&paths.archive_database_path)
            .expect("open archive")
            .query_row(
                "SELECT stats_json
                 FROM runs
                 WHERE run_type = 'rollback'
                 ORDER BY id DESC
                 LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .map(|value| serde_json::from_str(&value).expect("parse revert stats"))
            .expect("load revert stats");
        assert_eq!(after_revert_stats["totalVisits"], 0);

        crate::takeout::restore_import_batch(&paths, &config, None, batch_id)
            .expect("restore import batch");
        let dashboard_after_restore =
            load_dashboard_snapshot(&paths, &config, None).expect("dashboard after restore");
        assert_eq!(dashboard_after_restore.total_visits, 1);
        assert_eq!(dashboard_after_restore.total_urls, 1);
        let visible_after_restore = list_history(
            &paths,
            &config,
            None,
            HistoryQuery { q: Some("imported".to_string()), ..HistoryQuery::default() },
        )
        .expect("query after restore");
        assert_eq!(visible_after_restore.total, 1);
    }

    #[test]
    fn snapshot_restore_preview_and_run_record_the_saved_checkpoint() {
        let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempdir().expect("tempdir");
        let chrome_root = seed_chrome_fixture(dir.path());
        let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
        unsafe {
            std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
        }

        let paths = sample_paths(dir.path());
        let config = AppConfig {
            initialized: true,
            selected_profile_ids: vec!["chrome:Default".to_string()],
            ..AppConfig::default()
        };

        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let backup = run_backup(&paths, &config, None, false).expect("run backup");
        let snapshot_path: String = Connection::open(&paths.archive_database_path)
            .expect("open archive")
            .query_row(
                "SELECT file_path
                 FROM snapshots
                 ORDER BY id DESC
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("latest snapshot path");

        let preview = preview_snapshot_restore(
            &paths,
            &config,
            None,
            &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
        )
        .expect("preview snapshot restore");
        assert!(preview.execute_supported);
        assert_eq!(preview.snapshot_kind, "raw-source-checkpoint");
        assert_eq!(preview.estimated_visits, 2);
        assert_eq!(preview.estimated_urls, 1);

        let restored = run_snapshot_restore(
            &paths,
            &config,
            None,
            &SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
        )
        .expect("run snapshot restore");
        let restore_run = restored.run.expect("restore run");
        assert_eq!(restore_run.run_type, "snapshot_restore");
        assert_eq!(restore_run.status, "success");
        assert!(backup.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));

        let detail =
            load_audit_run_detail(&paths, &config, None, restore_run.id).expect("restore detail");
        assert!(
            detail
                .artifacts
                .iter()
                .any(|artifact| artifact.reason.as_deref() == Some("restored-source-checkpoint"))
        );

        restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
    }

    #[test]
    fn retention_preview_and_prune_clear_local_artifacts_and_record_a_run() {
        let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempdir().expect("tempdir");
        let chrome_root = seed_chrome_fixture(dir.path());
        let original_override = std::env::var_os("CHB_CHROME_USER_DATA_DIR");
        unsafe {
            std::env::set_var("CHB_CHROME_USER_DATA_DIR", &chrome_root);
        }

        let paths = sample_paths(dir.path());
        let config = AppConfig {
            initialized: true,
            selected_profile_ids: vec!["chrome:Default".to_string()],
            ..AppConfig::default()
        };

        ensure_archive_initialized(&paths, &config, None).expect("init archive");
        run_backup(&paths, &config, None, false).expect("run backup");
        fs::create_dir_all(&paths.exports_dir).expect("create exports dir");
        fs::write(paths.exports_dir.join("export.jsonl"), "[]").expect("write export fixture");

        let preview = preview_retention(&paths, &config, None).expect("preview retention");
        assert!(preview.buckets.iter().any(|bucket| bucket.id == "snapshots" && bucket.bytes > 0));
        assert!(preview.buckets.iter().any(|bucket| bucket.id == "exports" && bucket.bytes > 0));

        let result = run_retention_prune(
            &paths,
            &config,
            None,
            &RetentionPruneRequest {
                bucket_ids: vec!["snapshots".to_string(), "exports".to_string()],
            },
        )
        .expect("run retention prune");
        assert!(result.run_id.is_some());
        assert!(result.deleted_bytes > 0);
        assert_eq!(directory_size(&paths.raw_snapshots_dir), 0);
        assert_eq!(directory_size(&paths.exports_dir), 0);

        let connection = Connection::open(&paths.archive_database_path).expect("open archive");
        let snapshot_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .expect("snapshot count");
        assert_eq!(snapshot_count, 0);
        let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
        assert!(recent_runs.iter().any(|run| run.run_type == "retention_prune"));

        restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
    }

    #[test]
    fn rekey_archive_keeps_a_safety_snapshot() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = AppConfig { initialized: true, ..AppConfig::default() };
        ensure_archive_initialized(&paths, &config, None).expect("init archive");

        let status =
            rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, Some("vault-passphrase"))
                .expect("rekey archive");

        let rekey_dir = paths.raw_snapshots_dir.join("rekey");
        let snapshots = fs::read_dir(&rekey_dir)
            .expect("read rekey snapshot dir")
            .filter_map(|entry| entry.ok())
            .collect::<Vec<_>>();

        assert!(status.encrypted);
        assert_eq!(snapshots.len(), 1);
        assert!(snapshots[0].path().is_file());

        let encrypted_config = AppConfig { archive_mode: ArchiveMode::Encrypted, ..config.clone() };
        let recent_runs = load_recent_runs(&paths, &encrypted_config, Some("vault-passphrase"))
            .expect("recent runs after rekey");
        let rekey_run =
            recent_runs.iter().find(|run| run.run_type == "rekey").expect("rekey run in ledger");
        let detail = load_audit_run_detail(
            &paths,
            &encrypted_config,
            Some("vault-passphrase"),
            rekey_run.id,
        )
        .expect("rekey audit detail");
        assert!(detail.manifest_path.is_some());
        assert!(
            detail
                .artifacts
                .iter()
                .any(|artifact| artifact.reason.as_deref() == Some("before-rekey"))
        );
    }

    #[test]
    fn visit_event_fingerprint_is_stable() {
        let fingerprint = visit_event_fingerprint(
            "chromium-history",
            "https://example.com",
            1,
            Some("Title"),
            Some(805306368),
            None,
        );
        assert_eq!(fingerprint, "da53df0772e36b09afd187a0454da559fe451c828a40353f4e5c7514d17ecc59");
    }
}
