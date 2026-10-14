//! Responsibilities: expose committed archive status, dashboard counters and run/audit reads.
//! Not responsible for: initialization, passwords, discovery or legacy maintenance.
//! Failure modes: stale totals, blocked readers, hidden running runs, pagination loss,
//! malformed audit data silently discarded. Tests: read_model_tests in both modes.
use super::*;
use crate::{Vault, models::BackupWarning};
use vault_store::{Db, Session};

#[cfg(test)]
#[path = "read_model_tests.rs"]
mod tests;

/// A run page carries the cursor needed to reach older audit records.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPage {
    pub items: Vec<BackupRunOverview>,
    pub next_cursor: Option<i64>,
}

/// The retained Vault proves the archive is unlocked; status never retrieves a secret.
pub fn archive_status(vault: &Vault) -> Result<ArchiveStatus> {
    let mut status = archive_availability(vault.paths(), vault.config());
    status.unlocked = true;
    status.last_successful_backup_at = vault.read(Db::Archive, last_successful_backup)?;
    Ok(status)
}

/// Locked/onboarding status reads metadata only and never opens a database.
pub fn archive_availability(paths: &ProjectPaths, config: &AppConfig) -> ArchiveStatus {
    // An archive still in the v0.3 format counts: the upgrade screen converts it.
    let path = &paths.archive_db;
    ArchiveStatus {
        initialized: config.initialized && (path.exists() || paths.legacy.archive.exists()),
        encrypted: matches!(config.archive_mode, ArchiveMode::Encrypted),
        unlocked: false,
        database_path: path.display().to_string(),
        last_successful_backup_at: None,
        warning: None,
    }
}

fn last_successful_backup(s: &Session<'_>) -> Result<Option<String>> {
    Ok(s.query("SELECT finished_at FROM runs WHERE run_type='backup' AND status='success' ORDER BY id DESC LIMIT 1",(),|r|Ok(r.get::<Option<String>>(0)?))?.pop().flatten())
}

/// Lists a page of audit runs, including running and interrupted backups.
pub fn load_recent_runs(vault: &Vault, cursor: Option<i64>, limit: usize) -> Result<RunPage> {
    vault.read(Db::Archive, move |s| recent_runs_from(s, cursor, limit))
}

pub(crate) fn recent_runs_from(
    s: &Session<'_>,
    cursor: Option<i64>,
    limit: usize,
) -> Result<RunPage> {
    let limit = limit.clamp(1, 1000);
    let sql = RECENT_RUNS_SQL.replace(
        "ORDER BY runs.id DESC\nLIMIT 12",
        "AND runs.id < ?1 ORDER BY runs.id DESC LIMIT ?2",
    );
    let mut items = s.query(
        &sql,
        vault_store::params![cursor.unwrap_or(i64::MAX), limit as i64 + 1],
        backup_run_overview_from_row,
    )?;
    let more = items.len() > limit;
    items.truncate(limit);
    let next_cursor = more.then(|| items.last().map(|r| r.id)).flatten();
    Ok(RunPage { items, next_cursor })
}

/// One request an outside app made through PathKeep's MCP server (an `mcp_query` run): which
/// tool, what it asked and how much came back. Exists so the person can see what other apps read
/// of their history; the runs are kept for that and listed nowhere else.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpQueryRecord {
    pub id: i64,
    pub at: String,
    /// `search-history`, `archive-status` or `usage-guide`.
    pub tool: String,
    pub query: Option<String>,
    pub domain: Option<String>,
    pub profile_id: Option<String>,
    /// How many pages matched a search, across the archive.
    pub results: Option<u64>,
    /// How many of them the app was given: one page of results. Requests recorded before
    /// 2026-10-13 did not keep it.
    pub returned: Option<u64>,
    pub notes: Vec<String>,
}

/// A page of MCP requests, newest first, with the cursor to older ones.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpQueryPage {
    pub items: Vec<McpQueryRecord>,
    pub next_cursor: Option<i64>,
}

/// Lists what outside apps asked through the MCP server, newest first (`idx_runs_type_id`).
pub fn load_mcp_queries(vault: &Vault, cursor: Option<i64>, limit: usize) -> Result<McpQueryPage> {
    let limit = limit.clamp(1, 1000);
    let mut items = vault
        .read(Db::Archive, move |s| {
            Ok(s.query(
                "SELECT id, started_at, stats_json, warnings_json FROM runs
             WHERE run_type = 'mcp_query' AND id < ?1 ORDER BY id DESC LIMIT ?2",
                vault_store::params![cursor.unwrap_or(i64::MAX), limit as i64 + 1],
                |r| {
                    Ok((
                        r.get::<i64>(0)?,
                        r.get::<String>(1)?,
                        r.get::<Option<String>>(2)?,
                        r.get::<Option<String>>(3)?,
                    ))
                },
            )?)
        })?
        .into_iter()
        .map(|(id, at, stats, warnings)| {
            let stats: serde_json::Value =
                stats.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default();
            let text = |key: &str| {
                stats
                    .get(key)
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
                    .filter(|v| !v.is_empty())
            };
            McpQueryRecord {
                id,
                at,
                tool: text("tool").unwrap_or_default(),
                query: text("query"),
                domain: text("domain"),
                profile_id: text("profileId"),
                results: stats.get("total").and_then(|value| value.as_u64()),
                returned: stats.get("returned").and_then(|value| value.as_u64()),
                notes: warnings
                    .and_then(|json| serde_json::from_str(&json).ok())
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    let more = items.len() > limit;
    items.truncate(limit);
    let next_cursor = more.then(|| items.last().map(|record| record.id)).flatten();
    Ok(McpQueryPage { items, next_cursor })
}

/// Dashboard totals come from the counters committed with the facts.
pub fn load_dashboard_snapshot(vault: &Vault) -> Result<DashboardSnapshot> {
    let storage = storage_summary(vault.paths());
    vault.read(Db::Archive, move |s| {
        let totals = super::run_support::visible_totals(s)?;
        let runs = recent_runs_from(s, None, 12)?;
        let next_action = runs.items.is_empty().then(|| {
            "Run a manual backup to create the first manifest and snapshot artifacts.".to_owned()
        });
        Ok(DashboardSnapshot {
            generated_at: now_rfc3339(),
            total_profiles: totals.total_profiles,
            total_urls: totals.total_urls,
            total_visits: totals.total_visits,
            total_downloads: totals.total_downloads,
            earliest_visit_at: totals.earliest_visit_at,
            latest_visit_at: totals.latest_visit_at,
            last_successful_backup_at: last_successful_backup(s)?,
            recent_runs: runs.items,
            recent_runs_next_cursor: runs.next_cursor,
            storage,
            next_action,
        })
    })
}

/// Loads the full audit detail for one run, including manifest and snapshot artifacts.
pub fn load_audit_run_detail(vault: &crate::Vault, run_id: i64) -> Result<AuditRunDetail> {
    vault.read(vault_store::Db::Archive, move |connection| {
    let row = connection.query_one(
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
                row.get::< i64>(0)?,
                row.get::< String>(1)?,
                row.get::< String>(2)?,
                row.get::< Option<String>>(3)?,
                row.get::< i64>(4)?,
                row.get::< String>(5)?,
                row.get::< Option<String>>(6)?,
                row.get::< String>(7)?,
                row.get::< Option<String>>(8)?,
                row.get::< Option<String>>(9)?,
                row.get::< Option<String>>(10)?,
                row.get::< Option<String>>(11)?,
            ))
        },
    )?;
    let manifest = connection.query(
        "SELECT file_path,content_hash FROM manifests WHERE run_id=?1 ORDER BY id DESC LIMIT 1",[run_id],
        |r|Ok((r.get::<Option<String>>(0)?,r.get::<Option<String>>(1)?))
    )?.pop();
    let artifacts=connection.query("SELECT file_path,checksum,file_size,created_at,reason FROM snapshots WHERE run_id=?1 ORDER BY id",[run_id],|r|Ok(AuditArtifact {
        kind:"snapshot".into(),path:r.get(0)?,checksum:r.get(1)?,size_bytes:r.get::<Option<i64>>(2)?.map(|v|v as u64),created_at:r.get(3)?,reason:r.get(4)?,
    }))?;

    let profile_scope = decode_profile_scope(row.8.as_deref());
    let stats = decode_run_stats(row.9.as_deref());
    let warning_details = decode_run_warnings(row.10.as_deref());
    let warnings = crate::models::backup_warning_messages(&warning_details);

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
        warning_details,
        error_message: row.11,
        stats,
        manifest_path: manifest.as_ref().and_then(|(path, _)| path.clone()),
        manifest_hash: manifest.and_then(|(_, hash)| hash),
        artifacts,
    })
    })
}

/// Converts one `runs` row into the shared dashboard/audit overview shape.
pub(super) fn backup_run_overview_from_row(
    row: &vault_store::Row,
) -> vault_store::Result<BackupRunOverview> {
    let profile_scope = decode_profile_scope(row.get::<Option<String>>(6)?.as_deref());
    let summary = decode_run_stats(row.get::<Option<String>>(8)?.as_deref());
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

/// Decodes the stored run warnings, accepting both wire shapes.
///
/// `warnings_json` used to be a plain `["message", …]` array and backup runs now
/// write `[{ code, message, … }, …]`. Both shapes stay readable so an archive
/// written by an older build keeps its warnings visible in Audit — a run whose
/// warnings silently vanished would be worse than an unlocalized one. Non-backup
/// run types (maintenance, doctor, projection) still write plain strings.
pub(super) fn decode_run_warnings(value: Option<&str>) -> Vec<BackupWarning> {
    let Some(parsed) = value.and_then(|content| serde_json::from_str::<Value>(content).ok()) else {
        return Vec::new();
    };
    let Some(entries) = parsed.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| match entry {
            Value::String(message) => Some(BackupWarning::opaque(message.clone())),
            other => serde_json::from_value::<BackupWarning>(other.clone()).ok(),
        })
        .collect()
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
    stats
        .get("profilesProcessed")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(profile_scope.len())
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
///
/// The databases are measured where the store keeps them (`vault_store::Paths`) with their
/// write-ahead logs, which can hold as much as the file itself between checkpoints; the
/// `.sqlite` paths in `ProjectPaths` are the layout before v0.4.
pub(super) fn storage_summary(paths: &ProjectPaths) -> StorageSummary {
    let db = vault_store::Paths::under(&paths.app_root);
    let database = |path: &Path| {
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        file_size(path) + file_size(Path::new(&wal))
    };
    StorageSummary {
        archive_database_bytes: database(&db.archive),
        source_evidence_database_bytes: database(&db.source_evidence),
        search_database_bytes: directory_size(&paths.search_index_dir),
        intelligence_database_bytes: database(&db.intelligence) + database(&db.scratch),
        manifest_bytes: directory_size(&paths.manifests_dir),
        snapshot_bytes: directory_size(&paths.raw_snapshots_dir),
        export_bytes: directory_size(&paths.exports_dir),
        staging_bytes: directory_size(&paths.staging_dir),
        quarantine_bytes: directory_size(&paths.quarantine_dir),
        semantic_sidecar_bytes: crate::ai_sidecar::sidecar_storage_bytes(paths),
        intelligence_blob_bytes: directory_size(&paths.intelligence_blobs_dir),
        agent_database_bytes: database(&db.agent),
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
