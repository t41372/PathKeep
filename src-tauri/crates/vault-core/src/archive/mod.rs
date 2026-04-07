mod schema;

pub(crate) use self::schema::{create_schema, export_archive_database, open_archive_connection};
pub use self::schema::{current_version, run_migrations};
use crate::{
    chrome::{ProfileSnapshot, discover_profiles, stage_profile_snapshot},
    config::{ProjectPaths, ensure_paths, save_config},
    git_audit,
    models::{
        AppConfig, ArchiveMode, ArchiveStatus, AuditArtifact, AuditRunDetail, BackupProfileSummary,
        BackupReport, BackupRunOverview, DashboardSnapshot, ExportFormat, ExportRequest,
        ExportResult, HealthCheck, HealthReport, HistoryEntry, HistoryQuery, HistoryQueryResponse,
        StorageSummary,
    },
    utils::{now_rfc3339, sha256_hex, unix_micros_to_chrome_time, url_domain},
};
use anyhow::{Context, Result};
use browser_history_parser::{
    ChromiumReadCursor, HistoryDatabaseSet, ParsedDownload, ParsedFavicon, ParsedSearchTerm,
    ParsedUrl, ParsedVisit, chromium,
};
use chrono::{DateTime, Duration, Utc};
use iana_time_zone::get_timezone;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

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
  AND (?1 IS NULL OR source_profiles.profile_key = ?1)
  AND (?2 IS NULL OR source_profiles.browser_kind = ?2)
  AND (?3 IS NULL OR urls.url LIKE '%' || ?3 || '%' OR IFNULL(urls.title, '') LIKE '%' || ?3 || '%')
  AND (?4 IS NULL OR urls.url LIKE ?4)
  AND (?5 IS NULL OR visits.visit_time_ms >= ?5)
  AND (?6 IS NULL OR visits.visit_time_ms <= ?6)
ORDER BY
  CASE WHEN ?7 = 'oldest' THEN visits.visit_time_ms END ASC,
  CASE WHEN ?7 != 'oldest' THEN visits.visit_time_ms END DESC
LIMIT ?8
"#;

const RECENT_RUNS_SQL: &str = r#"
SELECT
  runs.id,
  runs.started_at,
  runs.finished_at,
  runs.status,
  manifests.content_hash,
  runs.stats_json
FROM runs
LEFT JOIN manifests
  ON manifests.run_id = runs.id
WHERE runs.run_type = 'backup'
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
        Some(
            "Run a manual Chromium backup to create the first manifest and snapshot artifacts."
                .to_string(),
        )
    } else {
        None
    };

    Ok(DashboardSnapshot {
        generated_at: now_rfc3339(),
        total_profiles: total_profiles.max(0) as usize,
        total_urls: total_urls.max(0) as usize,
        total_visits: total_visits.max(0) as usize,
        total_downloads: total_downloads.max(0) as usize,
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
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
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

    let profile_scope = row
        .7
        .as_ref()
        .map(|value| serde_json::from_str::<Vec<String>>(value).unwrap_or_default())
        .unwrap_or_default();
    let stats = row
        .8
        .as_ref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .unwrap_or_else(|| json!({}));
    let warnings = row
        .9
        .as_ref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();

    Ok(AuditRunDetail {
        run: BackupRunOverview {
            id: row.0,
            started_at: row.4,
            finished_at: row.5,
            status: row.6,
            manifest_hash: manifest.as_ref().and_then(|(_, hash)| hash.clone()),
            profiles_processed: stats.get("profilesProcessed").and_then(Value::as_u64).unwrap_or(0)
                as usize,
            new_visits: stats.get("newVisits").and_then(Value::as_u64).unwrap_or(0) as usize,
            new_urls: stats.get("newUrls").and_then(Value::as_u64).unwrap_or(0) as usize,
            new_downloads: stats.get("newDownloads").and_then(Value::as_u64).unwrap_or(0) as usize,
        },
        trigger: row.1,
        timezone: row.2,
        due_only: row.3 != 0,
        profile_scope,
        warnings,
        error_message: row.10,
        stats,
        manifest_path: manifest.as_ref().and_then(|(path, _)| path.clone()),
        manifest_hash: manifest.and_then(|(_, hash)| hash),
        artifacts,
    })
}

pub fn run_backup(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    due_only: bool,
) -> Result<BackupReport> {
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
        anyhow::bail!("select at least one Chromium profile before running a backup")
    }
    let selected_profiles = select_supported_profiles(&discovered, &config.selected_profile_ids);
    if selected_profiles.is_empty() {
        anyhow::bail!(
            "the selected profiles are not supported yet; choose at least one Chromium profile with a readable History database"
        )
    }
    let skipped_profiles = collect_skipped_profiles(&discovered, &config.selected_profile_ids);
    let started_at = now_rfc3339();
    let timezone = current_timezone_name();
    let trigger = if due_only { "schedule" } else { "manual" };

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
    let mut warnings = skipped_profiles
        .into_iter()
        .map(|profile_id| format!("Skipped unsupported non-Chromium profile `{profile_id}` during M1 archive foundation."))
        .collect::<Vec<_>>();

    let backup_result = (|| -> Result<()> {
        let transaction = connection.transaction()?;
        for profile in &selected_profiles {
            let snapshot = stage_profile_snapshot(paths, profile)?;
            let profile_summary = process_profile_snapshot(
                &transaction,
                run_id,
                paths,
                config,
                &snapshot,
                &mut snapshot_artifacts,
            )
            .with_context(|| format!("processing profile {}", profile.profile_id))?;
            source_hashes.insert(profile.profile_id.clone(), snapshot_source_hashes(&snapshot));
            warnings.extend(profile_summary.notes.clone());
            profile_summaries.push(profile_summary);
        }
        transaction.commit()?;
        Ok(())
    })();

    if let Err(error) = backup_result {
        finalize_failed_run(&connection, run_id, &profile_summaries, &warnings, &error)?;
        return Err(error);
    }

    let finished_at = now_rfc3339();
    let summary = backup_run_summary(run_id, &started_at, &finished_at, &profile_summaries);
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
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let limit = query.limit.unwrap_or(150).clamp(1, 1_000);
    let q = query.q.clone().filter(|value| !value.trim().is_empty());
    let domain_pattern = query
        .domain
        .clone()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{value}%"));
    let sort = query.sort.clone().unwrap_or_else(|| "newest".to_string());

    let mut statement = connection.prepare(LIST_HISTORY_SQL)?;
    let rows = statement.query_map(
        params![
            query.profile_id,
            query.browser_kind,
            q,
            domain_pattern,
            query.start_time_ms,
            query.end_time_ms,
            sort,
            limit,
        ],
        history_entry_from_row,
    )?;
    let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(HistoryQueryResponse { total: items.len(), items })
}

pub fn export_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ExportRequest,
) -> Result<ExportResult> {
    let results = list_history(paths, config, key, request.query)?;
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

    let source = open_archive_connection(paths, current_config, old_key)?;
    let temp_path = paths.archive_database_path.with_extension("rekey.sqlite");
    let backup_path = paths.archive_database_path.with_extension("backup.sqlite");
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }
    let target_key = match new_mode {
        ArchiveMode::Encrypted => Some(new_key.context("new encryption key is required")?),
        ArchiveMode::Plaintext => None,
    };
    export_archive_database(&source, &temp_path, target_key)?;
    fs::rename(&paths.archive_database_path, &backup_path)?;
    fs::rename(&temp_path, &paths.archive_database_path)?;
    let _ = fs::remove_file(&backup_path);

    let mut next_config = current_config.clone();
    next_config.initialized = true;
    next_config.archive_mode = new_mode;
    save_config(paths, &next_config)?;
    archive_status(paths, &next_config, new_key.or(old_key))
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
    }

    Ok(HealthReport { generated_at: now_rfc3339(), checks })
}

fn select_supported_profiles<'a>(
    discovered: &'a [crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<&'a crate::models::BrowserProfile> {
    discovered
        .iter()
        .filter(|profile| profile.history_exists && profile.browser_family == "chromium")
        .filter(|profile| {
            selected_profile_ids.iter().any(|selected| selected == &profile.profile_id)
        })
        .collect()
}

fn collect_skipped_profiles(
    discovered: &[crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<String> {
    discovered
        .iter()
        .filter(|profile| profile.history_exists && profile.browser_family != "chromium")
        .filter(|profile| {
            selected_profile_ids.iter().any(|selected| selected == &profile.profile_id)
        })
        .map(|profile| profile.profile_id.clone())
        .collect()
}

fn process_profile_snapshot(
    archive: &Transaction<'_>,
    run_id: i64,
    paths: &ProjectPaths,
    config: &AppConfig,
    snapshot: &ProfileSnapshot,
    snapshot_artifacts: &mut Vec<SnapshotArtifact>,
) -> Result<BackupProfileSummary> {
    let source_profile_id = upsert_source_profile(archive, &snapshot.profile)?;
    let schema_payload = collect_schema_payload(&snapshot.history_path)?;
    let schema_string = serde_json::to_string(&schema_payload)?;
    let schema_hash = sha256_hex(schema_string.as_bytes());
    let watermark = load_watermark(archive, &snapshot.profile.profile_id)?;
    let parsed = chromium::parse_history(
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
    )
    .context("parsing Chromium staging copy")?;

    let mut summary = BackupProfileSummary {
        profile_id: snapshot.profile.profile_id.clone(),
        notes: parsed.warnings.into_iter().map(|warning| warning.message).collect(),
        ..BackupProfileSummary::default()
    };

    let mut url_id_map = HashMap::new();
    let mut max_url_last_visit_time = watermark.last_url_last_visit_time;
    for url in &parsed.urls {
        let canonical_url_id =
            upsert_url(archive, run_id, source_profile_id, &snapshot.profile, url)?;
        url_id_map.insert(url.source_url_id, canonical_url_id);
        insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                source_profile_id,
                profile_id: &snapshot.profile.profile_id,
                source_kind: "chromium-history",
                table_name: "urls",
                source_pk: &url.source_url_id.to_string(),
                payload_hash: &payload_hash(url)?,
                payload_json: &serde_json::to_string(url)?,
                schema_hash: &schema_hash,
                chrome_version: snapshot.profile.browser_version.as_deref(),
                import_batch_id: None,
            },
        )?;
        max_url_last_visit_time =
            max_url_last_visit_time.max(ms_to_chromium_time(url.last_visit_ms));
        summary.new_urls += 1;
        summary.raw_rows += 1;
    }

    let mut max_visit_id = watermark.last_visit_id;
    for visit in &parsed.visits {
        let Some(&url_id) = url_id_map.get(&visit.source_url_id) else {
            continue;
        };
        let inserted = insert_visit(
            archive,
            run_id,
            source_profile_id,
            &snapshot.profile.profile_id,
            url_id,
            visit,
        )?;
        if inserted > 0 {
            summary.new_visits += 1;
        }
        insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                source_profile_id,
                profile_id: &snapshot.profile.profile_id,
                source_kind: "chromium-history",
                table_name: "visits",
                source_pk: &visit.source_visit_id.to_string(),
                payload_hash: &payload_hash(visit)?,
                payload_json: &serde_json::to_string(visit)?,
                schema_hash: &schema_hash,
                chrome_version: snapshot.profile.browser_version.as_deref(),
                import_batch_id: None,
            },
        )?;
        max_visit_id = max_visit_id.max(visit.source_visit_id);
        summary.raw_rows += 1;
        sync_url_bounds(archive, url_id, visit.visit_time_ms, &visit.visit_time_iso)?;
    }

    let mut max_download_id = watermark.last_download_id;
    for download in &parsed.downloads {
        let inserted = insert_download(archive, run_id, source_profile_id, download)?;
        if inserted > 0 {
            summary.new_downloads += 1;
        }
        insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                source_profile_id,
                profile_id: &snapshot.profile.profile_id,
                source_kind: "chromium-history",
                table_name: "downloads",
                source_pk: &download.source_download_id.to_string(),
                payload_hash: &payload_hash(download)?,
                payload_json: &serde_json::to_string(download)?,
                schema_hash: &schema_hash,
                chrome_version: snapshot.profile.browser_version.as_deref(),
                import_batch_id: None,
            },
        )?;
        max_download_id = max_download_id.max(download.source_download_id);
        summary.raw_rows += 1;
    }

    let mut inserted_search_terms = 0usize;
    for term in &parsed.search_terms {
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
        summary.notes.push(format!("Captured {inserted_search_terms} Chromium search term rows."));
    }

    let mut max_favicon_last_updated = watermark.last_favicon_last_updated;
    for favicon in &parsed.favicons {
        insert_favicon(archive, run_id, source_profile_id, favicon)?;
        max_favicon_last_updated =
            max_favicon_last_updated.max(ms_to_chromium_time(favicon.last_updated_ms));
    }

    if should_checkpoint(&watermark, &schema_hash, config.checkpoint_days) {
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
            last_visit_id: max_visit_id.max(watermark.last_visit_id),
            last_url_last_visit_time: max_url_last_visit_time
                .max(watermark.last_url_last_visit_time),
            last_download_id: max_download_id.max(watermark.last_download_id),
            last_favicon_last_updated: max_favicon_last_updated
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

fn upsert_source_profile(
    archive: &Transaction<'_>,
    profile: &crate::models::BrowserProfile,
) -> Result<i64> {
    let browser_kind = profile.profile_id.split(':').next().unwrap_or(&profile.browser_family);
    archive.execute(
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
           enabled = 1",
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
    )?;
    archive
        .query_row(
            "SELECT id
             FROM source_profiles
             WHERE profile_key = ?1",
            [profile.profile_id.as_str()],
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
) -> Result<i64> {
    let recorded_at = now_rfc3339();
    let payload_hash = payload_hash(url)?;
    archive.execute(
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
           END",
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
    )?;
    archive
        .query_row(
            "SELECT id
             FROM urls
             WHERE source_profile_id = ?1
               AND source_url_id = ?2",
            params![source_profile_id, url.source_url_id],
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
                payload_hash(visit)?,
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
                payload_hash(download)?,
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
                payload_hash(favicon)?,
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

fn sync_url_bounds(
    archive: &Transaction<'_>,
    url_id: i64,
    visit_time_ms: i64,
    visit_time_iso: &str,
) -> Result<()> {
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
               WHEN ?2 > last_visit_ms THEN ?2
               ELSE last_visit_ms
             END,
             last_visit_iso = CASE
               WHEN ?2 > last_visit_ms THEN ?3
               ELSE last_visit_iso
             END
         WHERE id = ?1",
        params![url_id, visit_time_ms, visit_time_iso],
    )?;
    Ok(())
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
            serde_json::to_string(&json!({
                "profilesProcessed": summary.profiles_processed,
                "newVisits": summary.new_visits,
                "newUrls": summary.new_urls,
                "newDownloads": summary.new_downloads,
                "manifestHash": manifest_hash,
            }))?,
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
    run_id: i64,
    started_at: &str,
    finished_at: &str,
    profile_summaries: &[BackupProfileSummary],
) -> BackupRunOverview {
    BackupRunOverview {
        id: run_id,
        started_at: started_at.to_string(),
        finished_at: Some(finished_at.to_string()),
        status: "success".to_string(),
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
    let summary_json: Option<String> = row.get(5)?;
    let summary = summary_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .unwrap_or_else(|| json!({}));
    Ok(BackupRunOverview {
        id: row.get(0)?,
        started_at: row.get(1)?,
        finished_at: row.get(2)?,
        status: row.get(3)?,
        manifest_hash: row.get(4)?,
        profiles_processed: summary.get("profilesProcessed").and_then(Value::as_u64).unwrap_or(0)
            as usize,
        new_visits: summary.get("newVisits").and_then(Value::as_u64).unwrap_or(0) as usize,
        new_urls: summary.get("newUrls").and_then(Value::as_u64).unwrap_or(0) as usize,
        new_downloads: summary.get("newDownloads").and_then(Value::as_u64).unwrap_or(0) as usize,
    })
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

fn payload_hash<T: Serialize>(value: &T) -> Result<String> {
    Ok(sha256_hex(serde_json::to_string(value)?.as_bytes()))
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
        config::ProjectPaths,
        utils::{restore_test_env_var, test_env_lock},
    };
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn sample_paths(root: &Path) -> ProjectPaths {
        ProjectPaths {
            app_root: root.to_path_buf(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
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
                 VALUES (1, 1, 'archive docs', 'archive docs')",
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
        let report = run_backup(&paths, &config, None, false).expect("run backup");
        assert_eq!(report.run.as_ref().expect("run").new_visits, 2);
        assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
        assert_eq!(report.run.as_ref().expect("run").new_downloads, 1);
        assert!(report.manifest_path.as_ref().is_some_and(|path| Path::new(path).exists()));
        assert!(report.profiles[0].checkpoint_created);

        let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
        assert_eq!(recent_runs.len(), 1);
        assert_eq!(recent_runs[0].status, "success");

        let history = list_history(
            &paths,
            &config,
            None,
            HistoryQuery { q: Some("archive".to_string()), ..HistoryQuery::default() },
        )
        .expect("list history");
        assert_eq!(history.total, 2);

        let report_again = run_backup(&paths, &config, None, false).expect("rerun backup");
        assert_eq!(report_again.run.as_ref().expect("run").new_visits, 0);

        restore_test_env_var("CHB_CHROME_USER_DATA_DIR", original_override.as_deref());
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
