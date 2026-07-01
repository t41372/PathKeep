//! Archive read models and status helpers.
//!
//! This module serves the non-mutating archive-facing reads used by the desktop
//! shell: initialization status, recent runs, dashboard summary, and audit
//! detail. These reads must stay honest about locked/uninitialized states and
//! must not hide the canonical run ledger behind page-specific shortcuts.

use super::*;
use crate::models::ArchiveUpgradeProgress;

/// Initializes the archive schema and returns the resulting status snapshot.
pub fn ensure_archive_initialized(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ArchiveStatus> {
    ensure_archive_initialized_with_progress(paths, config, key, |_| {})
}

/// Initializes the archive schema, streaming first-run upgrade progress as the
/// heavy phases advance, and returns the resulting status snapshot.
///
/// This is the observable twin of [`ensure_archive_initialized`], which
/// delegates here with a no-op callback so the no-callback path is byte-for-byte
/// the original behavior. The heavy work (index-build migrations, the `015`
/// registrable-domain backfill, and the v2→v4 search reprojection) all happens
/// inside the single [`open_archive_connection_reporting`] call — the SAME
/// bootstrap the plain open runs, now threaded so a large first-launch upgrade
/// can drive a calm "Upgrading your archive…" screen instead of an opaque
/// multi-minute busy overlay. A terminal [`ArchiveUpgradeProgress::finished`] is
/// emitted once the archive is fully open.
///
/// Progress is OBSERVATION ONLY: no migration SQL, backfill algorithm, or
/// reprojection logic changes, so the backfill stays keyset-paged + idempotent
/// and a user who quits mid-upgrade still resumes cleanly on the next launch.
pub fn ensure_archive_initialized_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    mut report: F,
) -> Result<ArchiveStatus>
where
    F: FnMut(ArchiveUpgradeProgress),
{
    ensure_paths(paths)?;
    let mut next_config = config.clone();
    next_config.initialized = true;
    save_config(paths, &next_config)?;
    let _connection = open_archive_connection_reporting(paths, &next_config, key, &mut report)?;
    report(ArchiveUpgradeProgress::finished());
    archive_status(paths, &next_config, key)
}

/// Reports whether the canonical archive exists and is currently readable.
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

/// Loads the recent run overview list used by Dashboard and Audit.
pub fn load_recent_runs(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Vec<BackupRunOverview>> {
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(Vec::new());
    }

    let connection = open_archive_connection(paths, config, key)?;
    let mut statement = connection.prepare(RECENT_RUNS_SQL)?;
    let rows = statement.query_map([], backup_run_overview_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Builds the dashboard snapshot read model from the canonical archive.
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
    // Dashboard "Span" reads `earliest_visit_at`/`latest_visit_at` so the
    // user sees "1y 2m of archive" instead of the misleading "today" you
    // used to get when the last backup completed minutes ago but the
    // imported data spans a year. Both bounds live alongside the totals
    // in the cached stats payload — at 14.4M visits, paying a fresh
    // MIN/MAX scan on every dashboard render would dominate page load.
    //
    // Backfill defence: pre-bounds-cache backups wrote stats_json with
    // counts but no `earliestVisitAt` / `latestVisitAt`. Upgrading users
    // would otherwise see a blank Span on every dashboard load until the
    // next backup overwrites the cache. When the cached totals say
    // "visits exist but bounds are None", treat it as an incomplete
    // cache hit and fall through to the live MIN/MAX (one-time cost
    // until the next backup writes the new shape).
    let totals = match load_cached_archive_totals(&connection)? {
        Some(cached) if cache_bounds_present_or_archive_empty(&cached) => cached,
        _ => count_visible_archive_totals(&connection)?,
    };
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
        earliest_visit_at: totals.earliest_visit_at,
        latest_visit_at: totals.latest_visit_at,
        recent_runs,
        storage: storage_summary(paths),
        next_action,
    })
}

/// Loads the full audit detail for one run, including manifest and snapshot artifacts.
pub fn load_audit_run_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    run_id: i64,
) -> Result<AuditRunDetail> {
    let connection = open_archive_connection(paths, config, key)?;
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
            error_message: row.11.clone(),
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

/// Converts one `runs` row into the shared dashboard/audit overview shape.
pub(super) fn backup_run_overview_from_row(row: &Row<'_>) -> rusqlite::Result<BackupRunOverview> {
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
        error_message: row.get(9)?,
    })
}

/// Decodes the stored JSON profile scope, defaulting to an empty scope on malformed data.
pub(super) fn decode_profile_scope(value: Option<&str>) -> Vec<String> {
    value.and_then(|content| serde_json::from_str::<Vec<String>>(content).ok()).unwrap_or_default()
}

/// Decodes the stored run stats JSON, defaulting to an empty object when absent or malformed.
pub(super) fn decode_run_stats(value: Option<&str>) -> Value {
    value
        .and_then(|content| serde_json::from_str::<Value>(content).ok())
        .unwrap_or_else(|| json!({}))
}

/// Reads the first matching numeric counter from the stored stats object.
fn run_stat_count(stats: &Value, keys: &[&str]) -> usize {
    keys.iter().find_map(|key| stats.get(*key).and_then(Value::as_u64)).unwrap_or(0) as usize
}

/// Resolves the processed-profile count, falling back to the explicit scope when absent.
pub(super) fn run_profiles_processed(stats: &Value, profile_scope: &[String]) -> usize {
    let explicit = run_stat_count(stats, &["profilesProcessed"]);
    if explicit > 0 { explicit } else { profile_scope.len() }
}

/// Resolves the visit count for several run families that reuse the overview surface.
pub(super) fn run_new_visits(stats: &Value) -> usize {
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

/// Resolves the URL count from stored run stats.
pub(super) fn run_new_urls(stats: &Value) -> usize {
    run_stat_count(stats, &["newUrls"])
}

/// Resolves the download count from stored run stats.
pub(super) fn run_new_downloads(stats: &Value) -> usize {
    run_stat_count(stats, &["newDownloads"])
}

/// Summarizes the on-disk footprint of the archive and nearby artifact directories.
pub(super) fn storage_summary(paths: &ProjectPaths) -> StorageSummary {
    StorageSummary {
        archive_database_bytes: file_size(&paths.archive_database_path),
        source_evidence_database_bytes: file_size(&paths.source_evidence_database_path),
        search_database_bytes: file_size(&paths.search_database_path),
        intelligence_database_bytes: file_size(&paths.intelligence_database_path),
        manifest_bytes: directory_size(&paths.manifests_dir),
        snapshot_bytes: directory_size(&paths.raw_snapshots_dir),
        export_bytes: directory_size(&paths.exports_dir),
        staging_bytes: directory_size(&paths.staging_dir),
        quarantine_bytes: directory_size(&paths.quarantine_dir),
        semantic_sidecar_bytes: directory_size(&paths.semantic_index_dir),
        intelligence_blob_bytes: directory_size(&paths.intelligence_blobs_dir),
    }
}

/// Returns the size of one file, treating missing files as zero bytes.
pub(super) fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or_default()
}

/// Recursively totals directory contents for storage reporting.
pub(super) fn directory_size(path: &Path) -> u64 {
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

/// Extracts cached archive totals from recent successful runs when available.
pub(super) fn load_cached_archive_totals(
    connection: &Connection,
) -> Result<Option<ArchiveVisibleTotals>> {
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

/// Returns `true` when the cached totals can stand in for a live
/// `count_visible_archive_totals` call. The visible-row counts are
/// stable across `count_visible_archive_totals` and the cached stats
/// shape, but the coverage bounds were added later — a pre-bounds-cache
/// payload reports `Some(totals)` with `None` bounds, and using that as-is
/// would silently blank the dashboard's Span stat for every upgrading
/// user until their next backup overwrites the cache. Falling through to
/// the live read in that case repairs the display at the cost of one
/// MIN/MAX scan per dashboard load until the cache is repopulated.
fn cache_bounds_present_or_archive_empty(cached: &ArchiveVisibleTotals) -> bool {
    if cached.total_visits == 0 {
        return true;
    }
    cached.earliest_visit_at.is_some() && cached.latest_visit_at.is_some()
}

/// Rebuilds cached visible totals from a stored stats payload when possible.
fn archive_totals_from_stats(stats: &Value) -> Option<ArchiveVisibleTotals> {
    Some(ArchiveVisibleTotals {
        total_profiles: stats.get("totalProfiles")?.as_u64()? as usize,
        total_urls: stats.get("totalUrls")?.as_u64()? as usize,
        total_visits: stats.get("totalVisits")?.as_u64()? as usize,
        total_downloads: stats.get("totalDownloads")?.as_u64()? as usize,
        earliest_visit_at: stats
            .get("earliestVisitAt")
            .and_then(|value| value.as_str().map(str::to_string)),
        latest_visit_at: stats
            .get("latestVisitAt")
            .and_then(|value| value.as_str().map(str::to_string)),
    })
}
