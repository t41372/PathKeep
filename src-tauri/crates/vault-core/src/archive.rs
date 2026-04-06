use crate::{
    ai::ensure_ai_schema,
    chrome::{ProfileSnapshot, discover_profiles, stage_profile_snapshot},
    config::{ProjectPaths, ensure_paths, save_config},
    git_audit,
    insights::ensure_insight_schema,
    models::{
        AppConfig, ArchiveMode, ArchiveStatus, BackupProfileSummary, BackupReport,
        BackupRunOverview, ExportFormat, ExportRequest, ExportResult, HealthCheck, HealthReport,
        HistoryEntry, HistoryQuery, HistoryQueryResponse,
    },
    utils::{
        chrome_time_to_rfc3339, now_rfc3339, sha256_hex, sqlite_row_to_json,
        unix_micros_to_chrome_time, url_domain,
    },
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, params};
use serde::Serialize;
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path, time::Duration as StdDuration};

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    created_at: String,
    previous_manifest_hash: Option<String>,
    run_id: i64,
    due_only: bool,
    database_path: String,
    summary: BackupRunOverview,
    profiles: Vec<BackupProfileSummary>,
    source_hashes: BTreeMap<String, BTreeMap<String, String>>,
    warnings: Vec<String>,
}

const MIGRATION_0001_SQL: &str = include_str!("migrations/0001_initial_archive.sql");
const MIGRATION_0002_SQL: &str = include_str!("migrations/0002_import_recoverability.sql");
const RECENT_RUNS_SQL: &str = "SELECT id, started_at, finished_at, status, manifest_hash, summary_json FROM backup_runs ORDER BY id DESC LIMIT 12";
const LIST_HISTORY_SQL: &str = "SELECT id, profile_id, url, title, visit_time, visit_duration, transition, source_visit_id, app_id FROM visit_events WHERE (?1 IS NULL OR profile_id = ?1) AND (?2 IS NULL OR url LIKE '%' || ?2 || '%' OR IFNULL(title, '') LIKE '%' || ?2 || '%') AND (?3 IS NULL OR url LIKE ?3) ORDER BY visit_time DESC LIMIT ?4";
const INGEST_URLS_SQL: &str = "SELECT id, url, title, visit_count, typed_count, last_visit_time, hidden FROM urls WHERE last_visit_time >= ?1 ORDER BY last_visit_time ASC";
const INGEST_VISITS_SQL: &str = "SELECT visits.id, visits.url, urls.url, urls.title, visits.visit_time, visits.from_visit, visits.transition, visits.visit_duration, visits.is_known_to_sync, visits.visited_link_id, visits.external_referrer_url, visits.app_id FROM visits JOIN urls ON urls.id = visits.url WHERE visits.id > ?1 ORDER BY visits.id ASC";
const FIREFOX_HISTORY_SQL: &str = "SELECT h.id, p.id, p.url, p.title, p.visit_count, IFNULL(p.hidden, 0), h.from_visit, h.visit_type, h.visit_date FROM moz_historyvisits h JOIN moz_places p ON h.place_id = p.id WHERE h.id > ?1 ORDER BY h.id ASC";
const SAFARI_HISTORY_SQL: &str = "SELECT hv.id, hi.id, hi.url, hv.title, hv.visit_time FROM history_visits hv JOIN history_items hi ON hi.id = hv.history_item WHERE hv.id > ?1 ORDER BY hv.id ASC";
const DOWNLOADS_SQL: &str = "SELECT id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type FROM downloads WHERE id > ?1 ORDER BY id ASC";
const SEARCH_TERMS_SQL: &str = "SELECT keyword_id, url_id, term, normalized_term FROM keyword_search_terms WHERE url_id IN (SELECT id FROM urls WHERE last_visit_time >= ?1)";
const FAVICONS_SQL: &str = "SELECT icon_mapping.page_url, favicons.url, favicons.icon_type, IFNULL(favicon_bitmaps.width, 0), IFNULL(favicon_bitmaps.height, 0), IFNULL(favicon_bitmaps.last_updated, 0), favicon_bitmaps.image_data FROM icon_mapping JOIN favicons ON favicons.id = icon_mapping.icon_id LEFT JOIN favicon_bitmaps ON favicon_bitmaps.icon_id = favicons.id WHERE IFNULL(favicon_bitmaps.last_updated, 0) >= ?1 ORDER BY IFNULL(favicon_bitmaps.last_updated, 0) ASC";

pub fn ensure_archive_initialized(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<ArchiveStatus> {
    ensure_paths(paths)?;
    let mut config = config.clone();
    config.initialized = true;
    save_config(paths, &config)?;
    let connection = open_archive_connection(paths, &config, key)?;
    create_schema(&connection)?;
    archive_status(paths, &config, key)
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
            status.unlocked = true;
            status.last_successful_backup_at = connection
                .query_row(
                    "SELECT finished_at FROM backup_runs WHERE status = 'success' ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
        }
        Err(error) => {
            status.warning = Some(error.to_string());
        }
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
    let mut statement = connection.prepare(RECENT_RUNS_SQL)?;
    let rows = statement.query_map([], backup_run_overview_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn backup_run_overview_from_row(row: &Row<'_>) -> rusqlite::Result<BackupRunOverview> {
    let summary_json: Option<String> = row.get(5)?;
    let summary = summary_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .unwrap_or_else(|| json!({}));
    Ok(BackupRunOverview {
        id: row.get(0)?,
        started_at: row.get(1)?,
        finished_at: row.get(2)?,
        status: row.get(3)?,
        manifest_hash: row.get(4)?,
        profiles_processed: summary.get("profilesProcessed").and_then(|v| v.as_u64()).unwrap_or(0)
            as usize,
        new_visits: summary.get("newVisits").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        new_urls: summary.get("newUrls").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
        new_downloads: summary.get("newDownloads").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
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
        anyhow::bail!("archive has not been initialized")
    }

    let archive = open_archive_connection(paths, config, key)?;
    create_schema(&archive)?;

    if due_only && let Some(reason) = backup_due_skip_reason(&archive, config)? {
        return Ok(BackupReport {
            due_skipped: true,
            reason: Some(reason),
            ..BackupReport::default()
        });
    }

    let profiles = discover_profiles()?;
    let selected_profiles = if config.selected_profile_ids.is_empty() {
        profiles.into_iter().filter(|profile| profile.history_exists).collect::<Vec<_>>()
    } else {
        profiles
            .into_iter()
            .filter(|profile| {
                profile.history_exists
                    && config.selected_profile_ids.iter().any(|id| id == &profile.profile_id)
            })
            .collect::<Vec<_>>()
    };

    let started_at = now_rfc3339();
    #[rustfmt::skip]
    archive.execute("INSERT INTO backup_runs (started_at, status, due_only, profiles_json) VALUES (?1, 'running', ?2, ?3)", params![started_at, due_only as i64, serde_json::to_string(&selected_profiles)?])?;
    let run_id = archive.last_insert_rowid();
    let previous_manifest_hash = latest_manifest_hash(&archive)?;

    let mut profile_summaries = Vec::new();
    let mut source_hashes = BTreeMap::<String, BTreeMap<String, String>>::new();
    let warnings = Vec::new();

    let backup_result: Result<()> = (|| {
        for profile in &selected_profiles {
            let snapshot = stage_profile_snapshot(paths, profile)?;
            let profile_summary =
                process_profile_snapshot(&archive, run_id, paths, config, &snapshot)
                    .with_context(|| format!("processing profile {}", profile.profile_id))?;
            source_hashes.insert(profile.profile_id.clone(), snapshot_source_hashes(&snapshot));
            profile_summaries.push(profile_summary);
        }
        Ok(())
    })();

    if let Err(error) = backup_result {
        let finished_at = now_rfc3339();
        let summary_json = failed_backup_summary_json(&profile_summaries, &error)?;
        archive
            .execute(
                "UPDATE backup_runs
                 SET finished_at = ?1, status = 'failed', summary_json = ?2
                 WHERE id = ?3",
                params![finished_at, summary_json, run_id],
            )
            .with_context(|| format!("recording failed backup run {run_id}"))?;
        return Err(error);
    }

    let finished_at = now_rfc3339();
    let summary = backup_run_summary(run_id, &started_at, &finished_at, &profile_summaries);

    let manifest = BackupManifest {
        created_at: finished_at.clone(),
        previous_manifest_hash: previous_manifest_hash.clone(),
        run_id,
        due_only,
        database_path: paths.archive_database_path.display().to_string(),
        summary: summary.clone(),
        profiles: profile_summaries.clone(),
        source_hashes,
        warnings: warnings.clone(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    let manifest_hash = sha256_hex(manifest_json.as_bytes());
    git_audit::ensure_repo(&paths.audit_repo_path)?;
    let manifest_relative_path =
        format!("manifests/{}/run-{}-{}.json", &finished_at[0..10], run_id, &manifest_hash[..12]);
    #[rustfmt::skip]
    let manifest_path = git_audit::write_audit_file(&paths.audit_repo_path, &manifest_relative_path, &manifest_json)?;

    let summary_json = serde_json::to_string(&json!({
        "profilesProcessed": summary.profiles_processed,
        "newVisits": summary.new_visits,
        "newUrls": summary.new_urls,
        "newDownloads": summary.new_downloads,
    }))?;

    #[rustfmt::skip]
    archive.execute("UPDATE backup_runs SET finished_at = ?1, status = 'success', manifest_path = ?2, manifest_hash = ?3, previous_manifest_hash = ?4, summary_json = ?5 WHERE id = ?6", params![finished_at, manifest_path.display().to_string(), manifest_hash, previous_manifest_hash, summary_json, run_id])?;
    #[rustfmt::skip]
    archive.execute("INSERT INTO manifests (run_id, manifest_hash, previous_manifest_hash, path, created_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![run_id, manifest_hash, manifest.previous_manifest_hash, manifest_path.display().to_string(), manifest.created_at])?;

    #[rustfmt::skip]
    let git_commit = if config.git_enabled { git_audit::commit_all(&paths.audit_repo_path, &format!("backup run {run_id}"))? } else { None };

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

fn snapshot_source_hashes(snapshot: &ProfileSnapshot) -> BTreeMap<String, String> {
    snapshot
        .source_hashes
        .iter()
        .map(|fingerprint| (fingerprint.path.clone(), fingerprint.sha256.clone()))
        .collect()
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

pub fn list_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let connection = open_archive_connection(paths, config, key)?;
    let limit = query.limit.unwrap_or(150).clamp(1, 1000);
    let q = query.q.clone().filter(|value| !value.trim().is_empty());
    let domain_pattern = query
        .domain
        .clone()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{value}%"));

    let mut statement = connection.prepare(LIST_HISTORY_SQL)?;
    let rows = statement
        .query_map(params![query.profile_id, q, domain_pattern, limit], history_entry_from_row)?;
    let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(HistoryQueryResponse { total: items.len(), items })
}

fn history_entry_from_row(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let url: String = row.get(2)?;
    let visit_time: i64 = row.get(4)?;
    Ok(HistoryEntry {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        domain: url_domain(&url),
        url,
        title: row.get(3)?,
        visited_at: chrome_time_to_rfc3339(visit_time),
        visit_time,
        duration_ms: row.get(5)?,
        transition: row.get(6)?,
        source_visit_id: row.get(7)?,
        app_id: row.get(8)?,
    })
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

pub fn rekey_archive(
    paths: &ProjectPaths,
    current_config: &AppConfig,
    old_key: Option<&str>,
    new_mode: ArchiveMode,
    new_key: Option<&str>,
) -> Result<ArchiveStatus> {
    ensure_paths(paths)?;
    if !paths.archive_database_path.exists() {
        anyhow::bail!("archive database does not exist")
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
        name: "Audit Repo".to_string(),
        ok: paths.audit_repo_path.join(".git").exists(),
        detail: paths.audit_repo_path.display().to_string(),
    });
    checks.push(HealthCheck {
        name: "Archive Unlock".to_string(),
        ok: archive_status(paths, config, key)?.unlocked,
        detail: if matches!(config.archive_mode, ArchiveMode::Encrypted) {
            "Encrypted archive requires an active session key".to_string()
        } else {
            "Plaintext archive".to_string()
        },
    });
    Ok(HealthReport { generated_at: now_rfc3339(), checks })
}

pub(crate) fn open_archive_connection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Connection> {
    ensure_paths(paths)?;
    let connection = Connection::open(&paths.archive_database_path)
        .with_context(|| format!("opening {}", paths.archive_database_path.display()))?;
    connection.busy_timeout(StdDuration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        let key = key.context("database key is required for encrypted archives")?;
        apply_cipher_key(&connection, key)?;
    }
    Ok(connection)
}

pub(crate) fn apply_cipher_key(connection: &Connection, key: &str) -> Result<()> {
    connection.pragma_update(None, "key", key)?;
    Ok(())
}

pub(crate) fn export_archive_database(
    source: &Connection,
    target_path: &Path,
    target_key: Option<&str>,
) -> Result<()> {
    if target_path.exists() {
        fs::remove_file(target_path)
            .with_context(|| format!("removing {}", target_path.display()))?;
    }

    let target = target_path.display().to_string().replace('\'', "''");
    let key = target_key.unwrap_or("").replace('\'', "''");
    source
        .execute_batch(&format!("ATTACH DATABASE '{target}' AS rekeyed KEY '{key}';"))
        .context("attaching target database for export")?;
    let export_result = source
        .query_row("SELECT sqlcipher_export('rekeyed')", [], |_| Ok(()))
        .context("exporting encrypted database");
    let detach_result = source.execute_batch("DETACH DATABASE rekeyed;");
    export_result?;
    detach_result.context("detaching exported database")?;
    Ok(())
}

pub(crate) fn create_schema(connection: &Connection) -> Result<()> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        )",
        [],
    )?;
    apply_migration(connection, 1, "initial_archive_schema", || {
        connection.execute_batch(MIGRATION_0001_SQL)?;
        Ok(())
    })?;
    apply_migration(connection, 2, "import_recoverability_columns", || {
        ensure_column(connection, "visit_events", "import_batch_id", "INTEGER")?;
        ensure_column(connection, "visit_events", "event_fingerprint", "TEXT")?;
        ensure_column(connection, "raw_row_versions", "import_batch_id", "INTEGER")?;
        connection.execute_batch(MIGRATION_0002_SQL)?;
        Ok(())
    })?;
    ensure_ai_schema(connection)?;
    ensure_insight_schema(connection)?;
    Ok(())
}

fn apply_migration<F>(connection: &Connection, version: i64, name: &str, migration: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let already_applied = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            params![version],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0)
        == 1;
    if already_applied {
        return Ok(());
    }
    migration()?;
    connection.execute(
        "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
        params![version, name, now_rfc3339()],
    )?;
    Ok(())
}

fn ensure_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .any(|name| name == column_name);

    if !exists {
        #[rustfmt::skip]
        let _ = connection.execute(&format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}"), [])?;
    }

    Ok(())
}

fn latest_successful_backup_at(connection: &Connection) -> Result<Option<DateTime<Utc>>> {
    let latest: Option<String> = connection
        .query_row(
            "SELECT finished_at FROM backup_runs WHERE status = 'success' ORDER BY id DESC LIMIT 1",
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

#[rustfmt::skip]
fn failed_backup_summary_json(profile_summaries: &[BackupProfileSummary], error: &anyhow::Error) -> Result<String> { Ok(serde_json::to_string(&json!({ "profilesProcessed": profile_summaries.len(), "newVisits": profile_summaries.iter().map(|item| item.new_visits).sum::<usize>(), "newUrls": profile_summaries.iter().map(|item| item.new_urls).sum::<usize>(), "newDownloads": profile_summaries.iter().map(|item| item.new_downloads).sum::<usize>(), "error": format!("{error:#}"), }))?) }

#[rustfmt::skip]
fn latest_manifest_hash(connection: &Connection) -> Result<Option<String>> { Ok(connection.query_row("SELECT manifest_hash FROM manifests ORDER BY id DESC LIMIT 1", [], |row| row.get(0)).optional()?) }

#[rustfmt::skip]
fn open_readonly_source(path: &Path) -> Result<Connection> { Ok(Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?) }

fn process_profile_snapshot(
    archive: &Connection,
    run_id: i64,
    paths: &ProjectPaths,
    config: &AppConfig,
    snapshot: &ProfileSnapshot,
) -> Result<BackupProfileSummary> {
    let history = open_readonly_source(&snapshot.history_path)?;
    let schema_json = collect_schema_json(&history)?;
    let schema_string = serde_json::to_string(&schema_json)?;
    let schema_hash = sha256_hex(schema_string.as_bytes());
    let source_kind = source_kind_for_profile(&snapshot.profile);
    #[rustfmt::skip]
    archive.execute("INSERT OR IGNORE INTO source_schemas (schema_hash, source_kind, chrome_version, payload_json, seen_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![schema_hash, source_kind, snapshot.profile.browser_version, schema_string, now_rfc3339()])?;
    #[rustfmt::skip]
    archive.execute("INSERT INTO profiles (profile_id, profile_name, user_name, profile_path, chrome_version, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(profile_id) DO UPDATE SET profile_name = excluded.profile_name, user_name = excluded.user_name, profile_path = excluded.profile_path, chrome_version = excluded.chrome_version, updated_at = excluded.updated_at", params![snapshot.profile.profile_id, snapshot.profile.profile_name, snapshot.profile.user_name, snapshot.profile.profile_path, snapshot.profile.browser_version, now_rfc3339()])?;

    let mut watermark = load_watermark(archive, &snapshot.profile.profile_id)?;
    let mut summary = BackupProfileSummary {
        profile_id: snapshot.profile.profile_id.clone(),
        ..BackupProfileSummary::default()
    };

    let (max_url_last_visit_time, max_visit_id, max_download_id, max_favicon_last_updated) =
        match snapshot.profile.browser_family.as_str() {
            "firefox" => {
                #[rustfmt::skip]
                let max_visit_id = ingest_firefox_history(archive, run_id, &snapshot.profile.profile_id, &history, &schema_hash, &mut summary, watermark.last_visit_id)?;
                (
                    watermark.last_url_last_visit_time,
                    max_visit_id,
                    watermark.last_download_id,
                    watermark.last_favicon_last_updated,
                )
            }
            "safari" => {
                #[rustfmt::skip]
                let max_visit_id = ingest_safari_history(archive, run_id, &snapshot.profile.profile_id, &history, &schema_hash, &mut summary, watermark.last_visit_id)?;
                (
                    watermark.last_url_last_visit_time,
                    max_visit_id,
                    watermark.last_download_id,
                    watermark.last_favicon_last_updated,
                )
            }
            _ => {
                #[rustfmt::skip]
                let max_url_last_visit_time = ingest_urls(archive, run_id, &snapshot.profile.profile_id, &history, &schema_hash, &mut summary, watermark.last_url_last_visit_time)?;
                #[rustfmt::skip]
                let max_visit_id = ingest_visits(archive, run_id, &snapshot.profile.profile_id, &history, &schema_hash, &mut summary, watermark.last_visit_id)?;
                #[rustfmt::skip]
                let max_download_id = ingest_downloads(archive, run_id, &snapshot.profile.profile_id, &history, &schema_hash, &mut summary, watermark.last_download_id)?;
                #[rustfmt::skip]
                ingest_search_terms(archive, &snapshot.profile.profile_id, &history, max_url_last_visit_time)?;

                let mut max_favicon_last_updated = watermark.last_favicon_last_updated;
                if config.capture_favicons
                    && let Some(favicons_path) = &snapshot.favicons_path
                {
                    let favicons = open_readonly_source(favicons_path)?;
                    #[rustfmt::skip]
                    let ingested_favicon_last_updated = ingest_favicons(archive, &snapshot.profile.profile_id, &favicons, watermark.last_favicon_last_updated)?;
                    max_favicon_last_updated = ingested_favicon_last_updated;
                }
                (max_url_last_visit_time, max_visit_id, max_download_id, max_favicon_last_updated)
            }
        };

    summary.checkpoint_created =
        should_checkpoint(&watermark, &schema_hash, config.checkpoint_days);
    #[rustfmt::skip]
    let _ = summary.checkpoint_created.then(|| checkpoint_snapshot(paths, snapshot)).transpose()?;

    watermark.last_url_last_visit_time =
        max_url_last_visit_time.max(watermark.last_url_last_visit_time);
    watermark.last_visit_id = max_visit_id.max(watermark.last_visit_id);
    watermark.last_download_id = max_download_id.max(watermark.last_download_id);
    watermark.last_favicon_last_updated =
        max_favicon_last_updated.max(watermark.last_favicon_last_updated);
    watermark.updated_at = now_rfc3339();
    watermark.last_schema_hash = Some(schema_hash);
    if summary.checkpoint_created {
        watermark.last_checkpoint_at = Some(now_rfc3339());
    }
    save_watermark(archive, &snapshot.profile.profile_id, &watermark)?;
    Ok(summary)
}

#[rustfmt::skip]
fn collect_schema_json(connection: &Connection) -> Result<serde_json::Value> {
    let mut statement =
        connection.prepare("SELECT type, name, tbl_name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY type, name")?;
    let rows = statement.query_map([], |row| {
        Ok(json!({
            "type": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "tableName": row.get::<_, String>(2)?,
            "sql": row.get::<_, String>(3)?,
        }))
    })?;
    Ok(serde_json::Value::Array(rows.collect::<rusqlite::Result<Vec<_>>>()?))
}

#[rustfmt::skip]
fn source_kind_for_profile(profile: &crate::models::BrowserProfile) -> &'static str { match profile.browser_family.as_str() { "firefox" => "firefox-history", "safari" => "safari-history", _ => "chromium-history", } }

#[rustfmt::skip]
pub(crate) fn visit_event_fingerprint(source_kind: &str, url: &str, visit_time: i64, title: Option<&str>, transition: Option<i64>, app_id: Option<&str>) -> String { let payload = json!({ "sourceKind": source_kind, "url": url, "visitTime": visit_time, "title": title.unwrap_or_default(), "transition": transition, "appId": app_id.unwrap_or_default(), }); let payload = serde_json::to_string(&payload).unwrap_or_default(); sha256_hex(payload.as_bytes()) }

fn ingest_urls(
    archive: &Connection,
    run_id: i64,
    profile_id: &str,
    source: &Connection,
    schema_hash: &str,
    summary: &mut BackupProfileSummary,
    watermark: i64,
) -> Result<i64> {
    let mut max_last_visit_time = watermark;
    let mut statement = source.prepare(INGEST_URLS_SQL)?;
    let rows = statement.query_map([watermark], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
            sqlite_row_to_json(row)?,
        ))
    })?;

    for row in rows {
        let (source_url_id, url, title, visit_count, typed_count, last_visit_time, hidden, payload) =
            row?;
        let payload_string = serde_json::to_string(&payload)?;
        let payload_hash = sha256_hex(payload_string.as_bytes());
        #[rustfmt::skip]
        let inserted = archive.execute("INSERT OR IGNORE INTO url_versions (profile_id, source_url_id, url, title, visit_count, typed_count, last_visit_time, hidden, payload_hash, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", params![profile_id, source_url_id, url, title, visit_count, typed_count, last_visit_time, hidden, payload_hash, now_rfc3339()])?;
        summary.new_urls += inserted as usize;
        #[rustfmt::skip]
        let inserted_raw_rows = insert_raw_row(archive, RawRowInsert { run_id, profile_id, source_kind: "chromium-history", table_name: "urls", source_pk: &source_url_id.to_string(), payload_hash: &payload_hash, payload_json: &payload_string, schema_hash, chrome_version: None })? as usize;
        summary.raw_rows += inserted_raw_rows;
        max_last_visit_time = max_last_visit_time.max(last_visit_time);
    }

    Ok(max_last_visit_time)
}

fn ingest_visits(
    archive: &Connection,
    run_id: i64,
    profile_id: &str,
    source: &Connection,
    schema_hash: &str,
    summary: &mut BackupProfileSummary,
    watermark: i64,
) -> Result<i64> {
    let mut max_visit_id = watermark;
    let mut statement = source.prepare(INGEST_VISITS_SQL)?;
    let rows = statement.query_map([watermark], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, Option<i64>>(7)?,
            row.get::<_, Option<i64>>(8)?,
            row.get::<_, Option<i64>>(9)?,
            row.get::<_, Option<String>>(10)?,
            row.get::<_, Option<String>>(11)?,
            sqlite_row_to_json(row)?,
        ))
    })?;

    for row in rows {
        let (
            source_visit_id,
            source_url_id,
            url,
            title,
            visit_time,
            from_visit,
            transition,
            visit_duration,
            is_known_to_sync,
            visited_link_id,
            external_referrer_url,
            app_id,
            payload,
        ) = row?;
        let payload_string = serde_json::to_string(&payload)?;
        let payload_hash = sha256_hex(payload_string.as_bytes());
        let event_fingerprint = visit_event_fingerprint(
            "chromium-history",
            &url,
            visit_time,
            title.as_deref(),
            transition,
            app_id.as_deref(),
        );
        #[rustfmt::skip]
        let inserted = archive.execute("INSERT OR IGNORE INTO visit_events (profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)", params![profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, now_rfc3339()])?;
        summary.new_visits += inserted as usize;
        #[rustfmt::skip]
        let inserted_raw_rows = insert_raw_row(archive, RawRowInsert { run_id, profile_id, source_kind: "chromium-history", table_name: "visits", source_pk: &source_visit_id.to_string(), payload_hash: &payload_hash, payload_json: &payload_string, schema_hash, chrome_version: None })? as usize;
        summary.raw_rows += inserted_raw_rows;
        max_visit_id = max_visit_id.max(source_visit_id);
    }

    Ok(max_visit_id)
}

fn ingest_firefox_history(
    archive: &Connection,
    run_id: i64,
    profile_id: &str,
    source: &Connection,
    schema_hash: &str,
    summary: &mut BackupProfileSummary,
    watermark: i64,
) -> Result<i64> {
    let mut max_visit_id = watermark;
    let mut statement = source.prepare(FIREFOX_HISTORY_SQL)?;
    let rows = statement.query_map([watermark], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, Option<i64>>(7)?,
            row.get::<_, i64>(8)?,
            sqlite_row_to_json(row)?,
        ))
    })?;

    for row in rows {
        let (
            source_visit_id,
            source_url_id,
            url,
            title,
            visit_count,
            hidden,
            from_visit,
            transition,
            raw_visit_date,
            payload,
        ) = row?;
        let visit_date_unix_micros = firefox_visit_date_to_unix_micros(raw_visit_date);
        let visit_time = unix_micros_to_chrome_time(visit_date_unix_micros);
        let url_payload = json!({
            "id": source_url_id,
            "url": url,
            "title": title,
            "visitCount": visit_count,
            "hidden": hidden,
            "lastVisitTime": visit_time,
        });
        let url_payload_string = serde_json::to_string(&url_payload)?;
        let url_payload_hash = sha256_hex(url_payload_string.as_bytes());
        #[rustfmt::skip]
        let inserted_url = archive.execute("INSERT OR IGNORE INTO url_versions (profile_id, source_url_id, url, title, visit_count, typed_count, last_visit_time, hidden, payload_hash, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9)", params![profile_id, source_url_id, url, title, visit_count, visit_time, hidden, url_payload_hash, now_rfc3339()])?;
        summary.new_urls += inserted_url as usize;
        #[rustfmt::skip]
        let inserted_raw_rows = insert_raw_row(archive, RawRowInsert { run_id, profile_id, source_kind: "firefox-history", table_name: "moz_places", source_pk: &source_url_id.to_string(), payload_hash: &url_payload_hash, payload_json: &url_payload_string, schema_hash, chrome_version: None })? as usize;
        summary.raw_rows += inserted_raw_rows;

        let payload_string = serde_json::to_string(&payload)?;
        let payload_hash = sha256_hex(payload_string.as_bytes());
        let event_fingerprint = visit_event_fingerprint(
            "firefox-history",
            &url,
            visit_time,
            title.as_deref(),
            transition,
            None,
        );
        #[rustfmt::skip]
        let inserted_visit = archive.execute("INSERT OR IGNORE INTO visit_events (profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, 0, NULL, NULL, 'firefox', ?9, ?10, ?11)", params![profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, event_fingerprint, payload_hash, now_rfc3339()])?;
        summary.new_visits += inserted_visit as usize;
        #[rustfmt::skip]
        let inserted_raw_rows = insert_raw_row(archive, RawRowInsert { run_id, profile_id, source_kind: "firefox-history", table_name: "moz_historyvisits", source_pk: &source_visit_id.to_string(), payload_hash: &payload_hash, payload_json: &payload_string, schema_hash, chrome_version: None })? as usize;
        summary.raw_rows += inserted_raw_rows;
        max_visit_id = max_visit_id.max(source_visit_id);
    }

    Ok(max_visit_id)
}

fn firefox_visit_date_to_unix_micros(raw_visit_date: i64) -> i64 {
    if raw_visit_date > 300_000_000 * 1_000_000 {
        raw_visit_date
    } else {
        raw_visit_date.saturating_mul(1_000)
    }
}

fn ingest_safari_history(
    archive: &Connection,
    run_id: i64,
    profile_id: &str,
    source: &Connection,
    schema_hash: &str,
    summary: &mut BackupProfileSummary,
    watermark: i64,
) -> Result<i64> {
    let mut max_visit_id = watermark;
    let mut statement = source.prepare(SAFARI_HISTORY_SQL)?;
    let rows = statement.query_map([watermark], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, f64>(4)?,
            sqlite_row_to_json(row)?,
        ))
    })?;

    for row in rows {
        let (source_visit_id, source_url_id, url, title, safari_visit_time, payload) = row?;
        let unix_micros = ((safari_visit_time + 978_307_200.0) * 1_000_000.0) as i64;
        let visit_time = unix_micros_to_chrome_time(unix_micros);
        let url_payload = json!({
            "id": source_url_id,
            "url": url,
            "title": title,
            "lastVisitTime": visit_time,
        });
        let url_payload_string = serde_json::to_string(&url_payload)?;
        let url_payload_hash = sha256_hex(url_payload_string.as_bytes());
        #[rustfmt::skip]
        let inserted_url = archive.execute("INSERT OR IGNORE INTO url_versions (profile_id, source_url_id, url, title, visit_count, typed_count, last_visit_time, hidden, payload_hash, recorded_at) VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, 0, ?6, ?7)", params![profile_id, source_url_id, url, title, visit_time, url_payload_hash, now_rfc3339()])?;
        summary.new_urls += inserted_url as usize;
        #[rustfmt::skip]
        let inserted_raw_rows = insert_raw_row(archive, RawRowInsert { run_id, profile_id, source_kind: "safari-history", table_name: "history_items", source_pk: &source_url_id.to_string(), payload_hash: &url_payload_hash, payload_json: &url_payload_string, schema_hash, chrome_version: None })? as usize;
        summary.raw_rows += inserted_raw_rows;

        let payload_string = serde_json::to_string(&payload)?;
        let payload_hash = sha256_hex(payload_string.as_bytes());
        let event_fingerprint = visit_event_fingerprint(
            "safari-history",
            &url,
            visit_time,
            title.as_deref(),
            None,
            None,
        );
        #[rustfmt::skip]
        let inserted_visit = archive.execute("INSERT OR IGNORE INTO visit_events (profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, NULL, 0, NULL, NULL, 'safari', ?7, ?8, ?9)", params![profile_id, source_visit_id, source_url_id, url, title, visit_time, event_fingerprint, payload_hash, now_rfc3339()])?;
        summary.new_visits += inserted_visit as usize;
        #[rustfmt::skip]
        let inserted_raw_rows = insert_raw_row(archive, RawRowInsert { run_id, profile_id, source_kind: "safari-history", table_name: "history_visits", source_pk: &source_visit_id.to_string(), payload_hash: &payload_hash, payload_json: &payload_string, schema_hash, chrome_version: None })? as usize;
        summary.raw_rows += inserted_raw_rows;
        max_visit_id = max_visit_id.max(source_visit_id);
    }

    Ok(max_visit_id)
}

fn ingest_downloads(
    archive: &Connection,
    run_id: i64,
    profile_id: &str,
    source: &Connection,
    schema_hash: &str,
    summary: &mut BackupProfileSummary,
    watermark: i64,
) -> Result<i64> {
    let mut max_download_id = watermark;
    let mut statement = source.prepare(DOWNLOADS_SQL)?;
    let rows = statement.query_map([watermark], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<i64>>(5)?,
            row.get::<_, Option<i64>>(6)?,
            row.get::<_, Option<i64>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
            sqlite_row_to_json(row)?,
        ))
    })?;

    for row in rows {
        let (
            source_download_id,
            guid,
            current_path,
            target_path,
            start_time,
            received_bytes,
            total_bytes,
            state,
            mime_type,
            original_mime_type,
            payload,
        ) = row?;
        let payload_string = serde_json::to_string(&payload)?;
        let payload_hash = sha256_hex(payload_string.as_bytes());
        #[rustfmt::skip]
        let inserted = archive.execute("INSERT OR IGNORE INTO download_versions (profile_id, source_download_id, guid, current_path, target_path, start_time, total_bytes, received_bytes, state, mime_type, original_mime_type, payload_hash, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)", params![profile_id, source_download_id, guid, current_path, target_path, start_time, total_bytes, received_bytes, state, mime_type, original_mime_type, payload_hash, now_rfc3339()])?;
        summary.new_downloads += inserted as usize;
        #[rustfmt::skip]
        let inserted_raw_rows = insert_raw_row(archive, RawRowInsert { run_id, profile_id, source_kind: "chromium-history", table_name: "downloads", source_pk: &source_download_id.to_string(), payload_hash: &payload_hash, payload_json: &payload_string, schema_hash, chrome_version: None })? as usize;
        summary.raw_rows += inserted_raw_rows;
        max_download_id = max_download_id.max(source_download_id);
    }

    Ok(max_download_id)
}

fn ingest_search_terms(
    archive: &Connection,
    profile_id: &str,
    source: &Connection,
    watermark: i64,
) -> Result<()> {
    let mut statement = source.prepare(SEARCH_TERMS_SQL)?;
    let rows = statement.query_map([watermark], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for row in rows {
        let (keyword_id, url_id, term, normalized_term) = row?;
        #[rustfmt::skip]
        archive.execute("INSERT OR IGNORE INTO search_terms (profile_id, keyword_id, url_id, term, normalized_term, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![profile_id, keyword_id, url_id, term, normalized_term, now_rfc3339()])?;
    }
    Ok(())
}

fn ingest_favicons(
    archive: &Connection,
    profile_id: &str,
    source: &Connection,
    watermark: i64,
) -> Result<i64> {
    let mut max_last_updated = watermark;
    let mut statement = source.prepare(FAVICONS_SQL)?;
    let rows = statement.query_map([watermark], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, Option<Vec<u8>>>(6)?,
        ))
    })?;
    for row in rows {
        let (page_url, icon_url, icon_type, width, height, last_updated, image_data) = row?;
        let payload = json!({
            "pageUrl": page_url,
            "iconUrl": icon_url,
            "iconType": icon_type,
            "width": width,
            "height": height,
            "lastUpdated": last_updated,
            "hasImageData": image_data.is_some(),
        });
        let payload_string = serde_json::to_string(&payload)?;
        let payload_hash = sha256_hex(payload_string.as_bytes());
        #[rustfmt::skip]
        archive.execute("INSERT OR IGNORE INTO favicons (profile_id, page_url, icon_url, icon_type, width, height, last_updated, image_data, payload_hash, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", params![profile_id, page_url, icon_url, icon_type, width, height, last_updated, image_data, payload_hash, now_rfc3339()])?;
        max_last_updated = max_last_updated.max(last_updated);
    }
    Ok(max_last_updated)
}

#[rustfmt::skip]
fn insert_raw_row(archive: &Connection, row: RawRowInsert<'_>) -> Result<usize> { Ok(archive.execute("INSERT OR IGNORE INTO raw_row_versions (run_id, profile_id, source_kind, table_name, source_pk, payload_hash, payload_json, schema_hash, chrome_version, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", params![row.run_id, row.profile_id, row.source_kind, row.table_name, row.source_pk, row.payload_hash, row.payload_json, row.schema_hash, row.chrome_version, now_rfc3339()])?) }

struct RawRowInsert<'a> {
    run_id: i64,
    profile_id: &'a str,
    source_kind: &'a str,
    table_name: &'a str,
    source_pk: &'a str,
    payload_hash: &'a str,
    payload_json: &'a str,
    schema_hash: &'a str,
    chrome_version: Option<&'a str>,
}

fn load_watermark(archive: &Connection, profile_id: &str) -> Result<Watermark> {
    Ok(archive
        .query_row(
            "SELECT last_visit_id, last_url_last_visit_time, last_download_id, last_favicon_last_updated, last_checkpoint_at, last_schema_hash
             FROM profile_watermarks WHERE profile_id = ?1",
            [profile_id],
            |row| {
                Ok(Watermark {
                    last_visit_id: row.get(0)?,
                    last_url_last_visit_time: row.get(1)?,
                    last_download_id: row.get(2)?,
                    last_favicon_last_updated: row.get(3)?,
                    last_checkpoint_at: row.get(4)?,
                    last_schema_hash: row.get(5)?,
                    updated_at: String::new(),
                })
            },
        )
        .optional()?
        .unwrap_or_default())
}

fn save_watermark(archive: &Connection, profile_id: &str, watermark: &Watermark) -> Result<()> {
    #[rustfmt::skip]
    archive.execute("INSERT INTO profile_watermarks (profile_id, last_visit_id, last_url_last_visit_time, last_download_id, last_favicon_last_updated, last_checkpoint_at, last_schema_hash, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(profile_id) DO UPDATE SET last_visit_id = excluded.last_visit_id, last_url_last_visit_time = excluded.last_url_last_visit_time, last_download_id = excluded.last_download_id, last_favicon_last_updated = excluded.last_favicon_last_updated, last_checkpoint_at = excluded.last_checkpoint_at, last_schema_hash = excluded.last_schema_hash, updated_at = excluded.updated_at", params![profile_id, watermark.last_visit_id, watermark.last_url_last_visit_time, watermark.last_download_id, watermark.last_favicon_last_updated, watermark.last_checkpoint_at, watermark.last_schema_hash, watermark.updated_at])?;
    Ok(())
}

#[rustfmt::skip]
fn should_checkpoint(watermark: &Watermark, schema_hash: &str, checkpoint_days: u64) -> bool { should_checkpoint_at(watermark, schema_hash, checkpoint_days, Utc::now()) }

#[rustfmt::skip]
fn should_checkpoint_at(watermark: &Watermark, schema_hash: &str, checkpoint_days: u64, now: DateTime<Utc>) -> bool { if watermark.last_schema_hash.as_deref() != Some(schema_hash) { return true; } let Some(last_checkpoint_at) = &watermark.last_checkpoint_at else { return true; }; let Ok(last_checkpoint_at) = DateTime::parse_from_rfc3339(last_checkpoint_at) else { return true; }; now - last_checkpoint_at.with_timezone(&Utc) > Duration::days(checkpoint_days as i64) }

fn checkpoint_snapshot(paths: &ProjectPaths, snapshot: &ProfileSnapshot) -> Result<()> {
    let checkpoint_dir = paths
        .raw_snapshots_dir
        .join(&snapshot.profile.profile_id)
        .join(now_rfc3339().replace(':', "-"));
    fs::create_dir_all(&checkpoint_dir)?;
    fs::copy(&snapshot.history_path, checkpoint_dir.join("History"))?;
    if let Some(favicons_path) = &snapshot.favicons_path {
        fs::copy(favicons_path, checkpoint_dir.join("Favicons"))?;
    }
    Ok(())
}

fn render_html_export(results: &HistoryQueryResponse) -> String {
    let rows = results
        .items
        .iter()
        .map(|item| {
            format!(
                "<li><time>{}</time> <a href=\"{}\">{}</a><p>{}</p></li>",
                item.visited_at,
                html_escape(&item.url),
                html_escape(item.title.as_deref().unwrap_or(&item.url)),
                html_escape(&item.url)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Browser History Backup Export</title><style>body{{font-family:ui-sans-serif,system-ui;padding:32px;background:#f4efe6;color:#1e1b16}}ol{{display:grid;gap:16px}}li{{padding:16px;border-top:1px solid rgba(30,27,22,.15)}}a{{color:#0a6c74;text-decoration:none}}p{{margin:6px 0 0;color:#5d5548}}</style></head><body><h1>Browser History Backup Export</h1><ol>{rows}</ol></body></html>"
    )
}

fn render_markdown_export(results: &HistoryQueryResponse) -> String {
    let mut output = String::from("# Browser History Backup Export\n\n");
    for item in &results.items {
        output.push_str(&format!(
            "- {} [{}]({})\n",
            item.visited_at,
            item.title.as_deref().unwrap_or(&item.url),
            item.url
        ));
    }
    output
}

fn render_text_export(results: &HistoryQueryResponse) -> String {
    let mut output = String::new();
    for item in &results.items {
        output.push_str(&format!(
            "{} | {} | {}\n",
            item.visited_at,
            item.title.as_deref().unwrap_or("Untitled"),
            item.url
        ));
    }
    output
}

fn html_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        chrome::FileFingerprint,
        config::{ensure_paths, save_config},
        models::{AppConfig, ArchiveMode, BrowserProfile},
        utils::{iso_to_chrome_time_micros, test_env_lock},
    };
    use std::path::PathBuf;
    use tempfile::tempdir;

    const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
    const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[rustfmt::skip]
    fn sample_paths(root: &Path) -> ProjectPaths { ProjectPaths { app_root: root.to_path_buf(), config_path: root.join("config.json"), archive_database_path: root.join("archive/history-vault.sqlite"), audit_repo_path: root.join("audit"), manifests_dir: root.join("audit/manifests"), exports_dir: root.join("exports"), raw_snapshots_dir: root.join("raw-snapshots"), staging_dir: root.join("staging"), quarantine_dir: root.join("quarantine"), schedule_dir: root.join("schedule"), stronghold_path: root.join("vault.hold"), stronghold_salt_path: root.join("stronghold-salt.txt"), } }

    #[rustfmt::skip]
    fn initialized_config(mode: ArchiveMode) -> AppConfig { AppConfig { initialized: true, archive_mode: mode, git_enabled: false, due_after_hours: 72, checkpoint_days: 1, ..AppConfig::default() } }

    fn chrome_user_data_fixture(root: &Path) -> PathBuf {
        let chrome_root = root.join("chrome-user-data");
        let profile_dir = chrome_root.join("Default");
        fs::create_dir_all(&profile_dir).expect("create chrome profile dir");
        fs::write(chrome_root.join("Last Version"), "135.0.0.0").expect("write version");
        fs::write(
            chrome_root.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"fixture@example.test"}}}}"#,
        )
        .expect("write local state");

        let history = Connection::open(profile_dir.join("History")).expect("open source history");
        #[rustfmt::skip]
        history.execute_batch("CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT NOT NULL, title TEXT, visit_count INTEGER NOT NULL, typed_count INTEGER NOT NULL, last_visit_time INTEGER NOT NULL, hidden INTEGER NOT NULL); CREATE TABLE visits (id INTEGER PRIMARY KEY, url INTEGER NOT NULL, visit_time INTEGER NOT NULL, from_visit INTEGER, transition INTEGER, visit_duration INTEGER, is_known_to_sync INTEGER, visited_link_id INTEGER, external_referrer_url TEXT, app_id TEXT); CREATE TABLE downloads (id INTEGER PRIMARY KEY, guid TEXT, current_path TEXT, target_path TEXT, start_time INTEGER, received_bytes INTEGER, total_bytes INTEGER, state INTEGER, mime_type TEXT, original_mime_type TEXT); CREATE TABLE keyword_search_terms (keyword_id INTEGER, url_id INTEGER, term TEXT, normalized_term TEXT);").expect("create history schema");

        let visit_time =
            iso_to_chrome_time_micros("2026-04-01T10:00:00+00:00").expect("chrome time");
        #[rustfmt::skip]
        history.execute("INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden) VALUES (1, 'https://example.com/<item>?q=1', 'Title <One>', 1, 1, ?1, 0)", [visit_time]).expect("insert url");
        #[rustfmt::skip]
        history.execute("INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id) VALUES (1, 1, ?1, NULL, 805306368, 24000, 1, 3, 'https://ref.example', 'com.example.app')", [visit_time]).expect("insert visit");
        history
            .execute(
                "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
                 VALUES (1, 'guid-1', '/tmp/current', '/tmp/target', ?1, 1, 2, 3, 'text/html', 'text/plain')",
                [visit_time],
            )
            .expect("insert download");
        history
            .execute(
                "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
                 VALUES (1, 1, 'chrome history', 'chrome history')",
                [],
            )
            .expect("insert search term");
        drop(history);

        let favicons = Connection::open(profile_dir.join("Favicons")).expect("open favicons");
        favicons
            .execute_batch(
                "
                CREATE TABLE favicons (id INTEGER PRIMARY KEY, url TEXT NOT NULL, icon_type INTEGER);
                CREATE TABLE icon_mapping (page_url TEXT NOT NULL, icon_id INTEGER NOT NULL);
                CREATE TABLE favicon_bitmaps (
                  icon_id INTEGER NOT NULL,
                  width INTEGER,
                  height INTEGER,
                  last_updated INTEGER,
                  image_data BLOB
                );",
            )
            .expect("create favicon schema");
        favicons
            .execute(
                "INSERT INTO favicons (id, url, icon_type) VALUES (1, 'https://example.com/favicon.ico', 1)",
                [],
            )
            .expect("insert favicon");
        favicons
            .execute(
                "INSERT INTO icon_mapping (page_url, icon_id) VALUES ('https://example.com/<item>?q=1', 1)",
                [],
            )
            .expect("insert icon mapping");
        favicons
            .execute(
                "INSERT INTO favicon_bitmaps (icon_id, width, height, last_updated, image_data)
                 VALUES (1, 16, 16, ?1, X'0102')",
                [visit_time],
            )
            .expect("insert bitmap");
        drop(favicons);

        chrome_root
    }

    fn broken_chrome_user_data_fixture(root: &Path) -> PathBuf {
        let chrome_root = root.join("chrome-user-data-broken");
        let profile_dir = chrome_root.join("Default");
        fs::create_dir_all(&profile_dir).expect("create broken profile dir");
        fs::write(profile_dir.join("History"), b"not-a-sqlite-database")
            .expect("write broken history");
        fs::write(chrome_root.join("Last Version"), "135.0.0.0").expect("write version");
        fs::write(
            chrome_root.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Broken","user_name":"fixture@example.test"}}}}"#,
        )
        .expect("write local state");
        chrome_root
    }

    fn firefox_history_fixture(root: &Path) -> PathBuf {
        let profile_dir = root.join("firefox-profile");
        fs::create_dir_all(&profile_dir).expect("create firefox profile");
        let db_path = profile_dir.join("places.sqlite");
        let connection = Connection::open(&db_path).expect("open firefox history");
        connection
            .execute_batch(
                "
                CREATE TABLE moz_places (
                  id INTEGER PRIMARY KEY,
                  url TEXT NOT NULL,
                  title TEXT,
                  visit_count INTEGER,
                  hidden INTEGER
                );
                CREATE TABLE moz_historyvisits (
                  id INTEGER PRIMARY KEY,
                  place_id INTEGER NOT NULL,
                  from_visit INTEGER,
                  visit_type INTEGER,
                  visit_date INTEGER NOT NULL
                );",
            )
            .expect("create firefox schema");
        connection
            .execute(
                "INSERT INTO moz_places (id, url, title, visit_count, hidden)
                 VALUES (1, 'https://example.com/firefox', 'Firefox Page', 3, 0)",
                [],
            )
            .expect("insert firefox place");
        connection
            .execute(
                "INSERT INTO moz_historyvisits (id, place_id, from_visit, visit_type, visit_date)
                 VALUES (11, 1, NULL, 1, 1710000000000000)",
                [],
            )
            .expect("insert firefox visit");
        db_path
    }

    fn safari_history_fixture(root: &Path) -> PathBuf {
        let profile_dir = root.join("safari-profile");
        fs::create_dir_all(&profile_dir).expect("create safari profile");
        let db_path = profile_dir.join("History.db");
        let connection = Connection::open(&db_path).expect("open safari history");
        connection
            .execute_batch(
                "
                CREATE TABLE history_items (
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
            .expect("create safari schema");
        connection
            .execute(
                "INSERT INTO history_items (id, url) VALUES (1, 'https://example.com/safari')",
                [],
            )
            .expect("insert safari item");
        connection
            .execute(
                "INSERT INTO history_visits (id, history_item, title, visit_time)
                 VALUES (21, 1, 'Safari Page', 800000000.0)",
                [],
            )
            .expect("insert safari visit");
        db_path
    }

    #[test]
    fn backup_history_export_and_rekey_work_end_to_end() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let mut config = initialized_config(ArchiveMode::Plaintext);
        config.git_enabled = true;
        config.selected_profile_ids = vec!["chrome:Default".to_string()];
        save_config(&paths, &config).expect("save config");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        }

        let report = run_backup(&paths, &config, None, false).expect("run backup");
        assert!(!report.due_skipped);
        assert!(report.reason.is_none());
        assert_eq!(report.run.as_ref().expect("run").new_visits, 1);
        assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
        assert_eq!(report.run.as_ref().expect("run").new_downloads, 1);
        assert!(report.run.as_ref().and_then(|run| run.manifest_hash.clone()).is_some());
        assert!(report.git_commit.is_some());
        let manifest_path = report.manifest_path.as_ref().expect("manifest path");
        let manifest = serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(manifest_path).expect("read manifest"),
        )
        .expect("parse manifest");
        let profile_hashes =
            manifest["sourceHashes"]["chrome:Default"].as_object().expect("profile source hashes");
        assert!(profile_hashes.keys().any(|path| path.ends_with("History")));
        assert!(profile_hashes.keys().any(|path| path.ends_with("Favicons")));
        assert!(
            profile_hashes
                .values()
                .all(|value| value.as_str().is_some_and(|hash| hash.len() == 64))
        );
        let recent_runs = load_recent_runs(&paths, &config, None).expect("recent runs");
        assert_eq!(recent_runs.len(), 1);
        assert_eq!(recent_runs[0].status, "success");
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        assert_eq!(
            latest_manifest_hash(&archive).expect("latest manifest hash"),
            report.run.as_ref().and_then(|run| run.manifest_hash.clone())
        );
        assert!(paths.audit_repo_path.join(".git").exists());

        let history = list_history(
            &paths,
            &config,
            None,
            HistoryQuery {
                q: Some("Title".to_string()),
                domain: Some("example.com".to_string()),
                profile_id: Some("chrome:Default".to_string()),
                limit: Some(10),
            },
        )
        .expect("list history");
        assert_eq!(history.total, 1);
        assert_eq!(history.items[0].domain, "example.com");
        let blank_filtered_history = list_history(
            &paths,
            &config,
            None,
            HistoryQuery {
                q: Some("   ".to_string()),
                domain: Some("   ".to_string()),
                profile_id: Some("chrome:Default".to_string()),
                limit: Some(10),
            },
        )
        .expect("list history with blank filters");
        assert_eq!(blank_filtered_history.total, 1);

        let html_export = export_history(
            &paths,
            &config,
            None,
            ExportRequest {
                format: ExportFormat::Html,
                query: HistoryQuery { q: None, domain: None, profile_id: None, limit: Some(10) },
            },
        )
        .expect("export html");
        let html = fs::read_to_string(&html_export.path).expect("read html export");
        assert!(html.contains("&lt;item&gt;"));
        assert!(html.contains("Browser History Backup Export"));

        let due_report = run_backup(&paths, &config, None, true).expect("due backup");
        assert!(due_report.due_skipped);
        assert!(due_report.reason.as_deref().is_some_and(|reason| reason.contains("hours old")));
        assert!(due_report.run.is_none());

        let health = doctor(&paths, &config, None).expect("doctor");
        assert!(health.checks.iter().any(|check| check.name == "Archive DB" && check.ok));

        fs::write(paths.archive_database_path.with_extension("rekey.sqlite"), "stale rekey")
            .expect("write stale rekey");
        let encrypted_status =
            rekey_archive(&paths, &config, None, ArchiveMode::Encrypted, Some("secret-key"))
                .expect("encrypt");
        assert!(encrypted_status.unlocked);

        let encrypted_config = initialized_config(ArchiveMode::Encrypted);
        let encrypted_history =
            list_history(&paths, &encrypted_config, Some("secret-key"), HistoryQuery::default())
                .expect("list encrypted");
        assert_eq!(encrypted_history.total, 1);

        let plaintext_status = rekey_archive(
            &paths,
            &encrypted_config,
            Some("secret-key"),
            ArchiveMode::Plaintext,
            None,
        )
        .expect("decrypt");
        assert!(plaintext_status.unlocked);

        let source_archive = open_archive_connection(&paths, &config, None).expect("open archive");
        let export_path = dir.path().join("manual-export.sqlite");
        fs::write(&export_path, "stale export").expect("write stale export");
        export_archive_database(&source_archive, &export_path, None).expect("export archive");
        assert!(export_path.exists());

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        }
    }

    #[test]
    fn run_backup_respects_selected_profile_ids() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let mut config = initialized_config(ArchiveMode::Plaintext);
        config.selected_profile_ids = vec!["chrome:Missing".to_string()];
        save_config(&paths, &config).expect("save config");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        }

        let report = run_backup(&paths, &config, None, false).expect("run backup");
        assert!(!report.due_skipped);
        assert_eq!(report.profiles.len(), 0);
        assert_eq!(report.run.as_ref().expect("run").profiles_processed, 0);
        assert_eq!(report.run.as_ref().expect("run").new_visits, 0);

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        }
    }

    #[test]
    fn failed_backup_marks_run_as_failed() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let chrome_root = broken_chrome_user_data_fixture(dir.path());
        let config = initialized_config(ArchiveMode::Plaintext);
        save_config(&paths, &config).expect("save config");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        }

        let error = run_backup(&paths, &config, None, false).expect_err("run backup should fail");
        let message = error.to_string();
        assert!(!message.trim().is_empty());

        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        let (status, finished_at, summary_json): (String, Option<String>, String) = archive
            .query_row(
                "SELECT status, finished_at, summary_json FROM backup_runs ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load backup run");
        assert_eq!(status, "failed");
        assert!(finished_at.is_some());
        assert!(!summary_json.trim().is_empty());
        let summary =
            serde_json::from_str::<serde_json::Value>(&summary_json).expect("parse summary");
        assert_eq!(summary["profilesProcessed"], 0);
        assert_eq!(summary["newVisits"], 0);
        assert!(
            summary["error"].as_str().is_some_and(|value| value.contains("processing profile"))
        );

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        }
    }

    #[test]
    fn schema_migration_helpers_and_text_renderers_are_stable() {
        let connection = Connection::open_in_memory().expect("db");
        create_schema(&connection).expect("create schema");
        create_schema(&connection).expect("idempotent");
        let applied_versions = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get::<_, i64>(0))
            .expect("load migration count");
        assert_eq!(applied_versions, 2);

        let results = HistoryQueryResponse {
            total: 1,
            items: vec![HistoryEntry {
                id: 1,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/?q=1&v=<x>".to_string(),
                title: Some("Item <One>".to_string()),
                domain: "example.com".to_string(),
                visited_at: "2026-04-01T10:00:00+00:00".to_string(),
                visit_time: 1,
                duration_ms: Some(10),
                transition: Some(1),
                source_visit_id: 1,
                app_id: None,
            }],
        };
        assert!(
            render_markdown_export(&results)
                .contains("[Item <One>](https://example.com/?q=1&v=<x>)")
        );
        assert!(render_text_export(&results).contains("Item <One>"));
        assert_eq!(html_escape("<>&\""), "&lt;&gt;&amp;&quot;");
        let schema = collect_schema_json(&connection).expect("schema json");
        assert!(schema.as_array().is_some_and(|items| {
            items.iter().any(|item| item["name"].as_str() == Some("visit_events"))
        }));
    }

    #[test]
    fn create_schema_upgrades_legacy_archives_without_baseline_stamping() {
        let connection = Connection::open_in_memory().expect("db");
        connection
            .execute_batch(
                "
                CREATE TABLE profiles (
                  profile_id TEXT PRIMARY KEY,
                  profile_name TEXT NOT NULL,
                  user_name TEXT,
                  profile_path TEXT NOT NULL,
                  chrome_version TEXT,
                  updated_at TEXT NOT NULL
                );
                CREATE TABLE visit_events (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  profile_id TEXT NOT NULL,
                  source_visit_id INTEGER NOT NULL,
                  source_url_id INTEGER NOT NULL,
                  url TEXT NOT NULL,
                  title TEXT,
                  visit_time INTEGER NOT NULL,
                  payload_hash TEXT NOT NULL,
                  recorded_at TEXT NOT NULL,
                  UNIQUE(profile_id, source_visit_id)
                );
                CREATE TABLE raw_row_versions (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  run_id INTEGER NOT NULL,
                  profile_id TEXT NOT NULL,
                  source_kind TEXT NOT NULL,
                  table_name TEXT NOT NULL,
                  source_pk TEXT NOT NULL,
                  payload_hash TEXT NOT NULL,
                  payload_json TEXT NOT NULL,
                  schema_hash TEXT NOT NULL,
                  recorded_at TEXT NOT NULL
                );
                ",
            )
            .expect("legacy schema");

        create_schema(&connection).expect("upgrade legacy schema");

        let migration_names = {
            let mut statement = connection
                .prepare("SELECT name FROM schema_migrations ORDER BY version ASC")
                .expect("prepare migration list");
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .expect("query migrations")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("collect migrations")
        };
        assert_eq!(
            migration_names,
            vec!["initial_archive_schema".to_string(), "import_recoverability_columns".to_string()]
        );

        let visit_has_import_batch = connection
            .prepare("PRAGMA table_info(visit_events)")
            .expect("visit info")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("visit columns")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect visit columns")
            .into_iter()
            .any(|column| column == "import_batch_id");
        assert!(visit_has_import_batch);

        let raw_row_has_import_batch = connection
            .prepare("PRAGMA table_info(raw_row_versions)")
            .expect("raw_row info")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("raw_row columns")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect raw_row columns")
            .into_iter()
            .any(|column| column == "import_batch_id");
        assert!(raw_row_has_import_batch);
    }

    #[test]
    fn watermark_checkpoint_and_snapshot_helpers_cover_edge_cases() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let archive = Connection::open_in_memory().expect("memory db");
        create_schema(&archive).expect("schema");

        let missing = load_watermark(&archive, "chrome:Default").expect("load missing watermark");
        assert_eq!(missing.last_visit_id, 0);

        let watermark = Watermark {
            last_visit_id: 7,
            last_url_last_visit_time: 11,
            last_download_id: 13,
            last_favicon_last_updated: 17,
            last_checkpoint_at: Some("2026-04-01T00:00:00+00:00".to_string()),
            last_schema_hash: Some("schema-1".to_string()),
            updated_at: "2026-04-01T00:00:00+00:00".to_string(),
        };
        save_watermark(&archive, "chrome:Default", &watermark).expect("save watermark");
        let loaded = load_watermark(&archive, "chrome:Default").expect("load watermark");
        assert_eq!(loaded.last_visit_id, 7);
        assert!(!should_checkpoint(&loaded, "schema-1", 365));
        assert!(should_checkpoint(&loaded, "schema-2", 365));
        assert!(should_checkpoint(
            &Watermark { last_checkpoint_at: Some("invalid".to_string()), ..Watermark::default() },
            "schema-1",
            1
        ));
        let exact_boundary = DateTime::parse_from_rfc3339("2027-04-01T00:00:00+00:00")
            .expect("parse boundary")
            .with_timezone(&Utc);
        assert!(!should_checkpoint_at(&loaded, "schema-1", 365, exact_boundary));
        assert!(should_checkpoint_at(
            &loaded,
            "schema-1",
            365,
            exact_boundary + Duration::seconds(1)
        ));

        let snapshot_dir = dir.path().join("snapshot-source");
        fs::create_dir_all(&snapshot_dir).expect("create snapshot dir");
        let history_path = snapshot_dir.join("History");
        let favicons_path = snapshot_dir.join("Favicons");
        fs::write(&history_path, b"history").expect("write history");
        fs::write(&favicons_path, b"favicons").expect("write favicons");
        let temp_dir = tempdir().expect("snapshot tempdir");
        let snapshot = ProfileSnapshot {
            profile: BrowserProfile {
                profile_id: "chrome:Default".to_string(),
                profile_name: "Default".to_string(),
                browser_family: "chromium".to_string(),
                browser_name: "Google Chrome".to_string(),
                user_name: None,
                profile_path: snapshot_dir.display().to_string(),
                history_path: Some(history_path.display().to_string()),
                favicons_path: Some(favicons_path.display().to_string()),
                history_exists: true,
                browser_version: Some("146.0.0.0".to_string()),
                history_file_name: "History".to_string(),
            },
            history_path: history_path.clone(),
            favicons_path: Some(favicons_path.clone()),
            source_hashes: vec![FileFingerprint {
                path: history_path.display().to_string(),
                sha256: "hash".to_string(),
            }],
            temp_dir,
        };
        checkpoint_snapshot(&paths, &snapshot).expect("checkpoint snapshot");
        let checkpoint_root = paths.raw_snapshots_dir.join("chrome:Default");
        let mut checkpoint_entries = fs::read_dir(&checkpoint_root)
            .expect("checkpoint dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        checkpoint_entries.sort();
        let latest = checkpoint_entries.last().expect("checkpoint entry");
        assert!(latest.join("History").exists());
        assert!(latest.join("Favicons").exists());
        assert_eq!(firefox_visit_date_to_unix_micros(1_710_000_000_000), 1_710_000_000_000_000);
        assert_eq!(firefox_visit_date_to_unix_micros(1_710_000_000_000_000), 1_710_000_000_000_000);
        assert_eq!(
            firefox_visit_date_to_unix_micros(300_000_000 * 1_000_000),
            300_000_000_000_000_000
        );

        let inserted = insert_raw_row(
            &archive,
            RawRowInsert {
                run_id: 1,
                profile_id: "chrome:Default",
                source_kind: "chromium-history",
                table_name: "urls",
                source_pk: "1",
                payload_hash: "payload-hash",
                payload_json: "{\"id\":1}",
                schema_hash: "schema-1",
                chrome_version: Some("146.0.0.0"),
            },
        )
        .expect("insert raw row");
        let duplicate = insert_raw_row(
            &archive,
            RawRowInsert {
                run_id: 1,
                profile_id: "chrome:Default",
                source_kind: "chromium-history",
                table_name: "urls",
                source_pk: "1",
                payload_hash: "payload-hash",
                payload_json: "{\"id\":1}",
                schema_hash: "schema-1",
                chrome_version: Some("146.0.0.0"),
            },
        )
        .expect("insert duplicate raw row");
        assert_eq!(inserted, 1);
        assert_eq!(duplicate, 0);
    }

    #[test]
    fn process_profile_snapshot_supports_firefox_and_safari_histories() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let archive = Connection::open_in_memory().expect("memory db");
        create_schema(&archive).expect("schema");
        let config = initialized_config(ArchiveMode::Plaintext);

        let firefox_history = firefox_history_fixture(dir.path());
        let firefox_seed = Connection::open(&firefox_history).expect("seed firefox history");
        firefox_seed
            .execute(
                "INSERT INTO moz_places (id, url, title, visit_count, hidden)
                 VALUES (2, 'https://example.com/firefox-legacy', 'Firefox Legacy', 1, 0)",
                [],
            )
            .expect("insert legacy firefox place");
        firefox_seed
            .execute(
                "INSERT INTO moz_historyvisits (id, place_id, from_visit, visit_type, visit_date)
                 VALUES (12, 2, 11, 2, 1710000005000)",
                [],
            )
            .expect("insert legacy firefox visit");
        drop(firefox_seed);
        let firefox_snapshot = ProfileSnapshot {
            profile: BrowserProfile {
                profile_id: "firefox:default-release".to_string(),
                profile_name: "Firefox".to_string(),
                browser_family: "firefox".to_string(),
                browser_name: "Firefox".to_string(),
                user_name: None,
                profile_path: firefox_history
                    .parent()
                    .expect("firefox parent")
                    .display()
                    .to_string(),
                history_path: Some(firefox_history.display().to_string()),
                favicons_path: None,
                history_exists: true,
                browser_version: None,
                history_file_name: "places.sqlite".to_string(),
            },
            temp_dir: tempdir().expect("firefox tempdir"),
            history_path: firefox_history,
            favicons_path: None,
            source_hashes: Vec::new(),
        };
        let firefox_summary =
            process_profile_snapshot(&archive, 1, &paths, &config, &firefox_snapshot)
                .expect("process firefox");
        assert_eq!(firefox_summary.profile_id, "firefox:default-release");
        assert_eq!(firefox_summary.new_urls, 2);
        assert_eq!(firefox_summary.new_visits, 2);
        assert_eq!(firefox_summary.raw_rows, 4);
        assert!(firefox_summary.checkpoint_created);
        let firefox_times = archive
            .prepare(
                "SELECT source_visit_id, visit_time
                 FROM visit_events
                 WHERE profile_id = 'firefox:default-release'
                 ORDER BY source_visit_id",
            )
            .expect("prepare firefox events")
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .expect("query firefox events")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect firefox events");
        assert_eq!(
            firefox_times,
            vec![
                (11, unix_micros_to_chrome_time(1_710_000_000_000_000)),
                (12, unix_micros_to_chrome_time(1_710_000_005_000_000)),
            ]
        );

        let safari_history = safari_history_fixture(dir.path());
        let safari_snapshot = ProfileSnapshot {
            profile: BrowserProfile {
                profile_id: "safari:default".to_string(),
                profile_name: "Default".to_string(),
                browser_family: "safari".to_string(),
                browser_name: "Safari".to_string(),
                user_name: None,
                profile_path: safari_history.parent().expect("safari parent").display().to_string(),
                history_path: Some(safari_history.display().to_string()),
                favicons_path: None,
                history_exists: true,
                browser_version: None,
                history_file_name: "History.db".to_string(),
            },
            temp_dir: tempdir().expect("safari tempdir"),
            history_path: safari_history,
            favicons_path: None,
            source_hashes: Vec::new(),
        };
        let safari_summary =
            process_profile_snapshot(&archive, 2, &paths, &config, &safari_snapshot)
                .expect("process safari");
        assert_eq!(safari_summary.profile_id, "safari:default");
        assert_eq!(safari_summary.new_urls, 1);
        assert_eq!(safari_summary.new_visits, 1);
        assert_eq!(safari_summary.raw_rows, 2);
        assert!(safari_summary.checkpoint_created);
        let safari_visit_time: i64 = archive
            .query_row(
                "SELECT visit_time FROM visit_events WHERE profile_id = 'safari:default' AND source_visit_id = 21",
                [],
                |row| row.get(0),
            )
            .expect("load safari visit time");
        assert_eq!(
            safari_visit_time,
            unix_micros_to_chrome_time(((800_000_000.0 + 978_307_200.0) * 1_000_000.0) as i64)
        );
    }

    #[test]
    fn chromium_ingest_helpers_cover_urls_visits_downloads_search_terms_and_favicons() {
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let history = Connection::open(chrome_root.join("Default/History")).expect("open history");
        let favicons =
            Connection::open(chrome_root.join("Default/Favicons")).expect("open favicons");
        let archive = Connection::open_in_memory().expect("memory db");
        create_schema(&archive).expect("schema");
        let mut summary = BackupProfileSummary::default();
        let second_visit_time =
            iso_to_chrome_time_micros("2026-04-01T11:00:00+00:00").expect("second chrome time");
        history
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (2, 'https://example.com/extra', 'Extra', 1, 0, ?1, 0)",
                [second_visit_time],
            )
            .expect("insert second url");
        history
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES (2, 2, ?1, 1, 268435456, 12000, 1, 4, 'https://example.com/docs', NULL)",
                [second_visit_time],
            )
            .expect("insert second visit");
        history
            .execute(
                "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
                 VALUES (2, 'guid-2', '/tmp/current-2', '/tmp/target-2', ?1, 2, 3, 4, 'application/json', 'application/octet-stream')",
                [second_visit_time],
            )
            .expect("insert second download");

        let max_url_last_visit_time =
            ingest_urls(&archive, 1, "chrome:Default", &history, "schema-1", &mut summary, 0)
                .expect("ingest urls");
        assert_eq!(max_url_last_visit_time, second_visit_time);
        assert_eq!(summary.new_urls, 2);
        assert_eq!(summary.raw_rows, 2);

        let max_visit_id =
            ingest_visits(&archive, 1, "chrome:Default", &history, "schema-1", &mut summary, 0)
                .expect("ingest visits");
        assert_eq!(max_visit_id, 2);
        assert_eq!(summary.new_visits, 2);
        assert_eq!(summary.raw_rows, 4);

        let max_download_id =
            ingest_downloads(&archive, 1, "chrome:Default", &history, "schema-1", &mut summary, 0)
                .expect("ingest downloads");
        assert_eq!(max_download_id, 2);
        assert_eq!(summary.new_downloads, 2);
        assert_eq!(summary.raw_rows, 6);

        ingest_search_terms(&archive, "chrome:Default", &history, 0).expect("ingest search terms");
        let search_terms: i64 = archive
            .query_row("SELECT COUNT(*) FROM search_terms", [], |row| row.get(0))
            .expect("search term count");
        assert_eq!(search_terms, 1);

        let max_favicon_updated =
            ingest_favicons(&archive, "chrome:Default", &favicons, 0).expect("ingest favicons");
        assert!(max_favicon_updated > 0);
        let favicon_rows: i64 = archive
            .query_row("SELECT COUNT(*) FROM favicons", [], |row| row.get(0))
            .expect("favicon count");
        assert_eq!(favicon_rows, 1);

        assert_eq!(
            source_kind_for_profile(&BrowserProfile {
                profile_id: "chrome:Default".to_string(),
                profile_name: "Default".to_string(),
                browser_family: "chromium".to_string(),
                browser_name: "Google Chrome".to_string(),
                user_name: None,
                profile_path: String::new(),
                history_path: None,
                favicons_path: None,
                history_exists: true,
                browser_version: None,
                history_file_name: "History".to_string(),
            }),
            "chromium-history"
        );
        assert_eq!(
            source_kind_for_profile(&BrowserProfile {
                profile_id: "firefox:default-release".to_string(),
                profile_name: "Firefox".to_string(),
                browser_family: "firefox".to_string(),
                browser_name: "Firefox".to_string(),
                user_name: None,
                profile_path: String::new(),
                history_path: None,
                favicons_path: None,
                history_exists: true,
                browser_version: None,
                history_file_name: "places.sqlite".to_string(),
            }),
            "firefox-history"
        );
        assert_eq!(
            source_kind_for_profile(&BrowserProfile {
                profile_id: "safari:default".to_string(),
                profile_name: "Default".to_string(),
                browser_family: "safari".to_string(),
                browser_name: "Safari".to_string(),
                user_name: None,
                profile_path: String::new(),
                history_path: None,
                favicons_path: None,
                history_exists: true,
                browser_version: None,
                history_file_name: "History.db".to_string(),
            }),
            "safari-history"
        );
    }

    #[test]
    fn archive_helper_edges_cover_uninitialized_status_and_additional_exports() {
        let _guard = lock_env();
        let empty_dir = tempdir().expect("tempdir");
        let empty_paths = sample_paths(empty_dir.path());
        let empty_config = AppConfig::default();
        let status = archive_status(&empty_paths, &empty_config, None).expect("archive status");
        assert!(!status.initialized);
        assert!(!status.unlocked);
        assert!(status.warning.is_none());
        assert!(
            load_recent_runs(&empty_paths, &empty_config, None).expect("recent runs").is_empty()
        );
        let backup_error = run_backup(&empty_paths, &empty_config, None, false)
            .expect_err("uninitialized backup should fail");
        assert!(backup_error.to_string().contains("has not been initialized"));
        let rekey_error = rekey_archive(
            &empty_paths,
            &initialized_config(ArchiveMode::Plaintext),
            None,
            ArchiveMode::Encrypted,
            Some("secret"),
        )
        .expect_err("rekey without archive should fail");
        assert!(rekey_error.to_string().contains("does not exist"));

        let empty_override = empty_dir.path().join("empty-chrome");
        fs::create_dir_all(&empty_override).expect("create empty override");
        unsafe {
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &empty_override);
        }
        let doctor_report = doctor(&empty_paths, &empty_config, None).expect("doctor");
        unsafe {
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        }
        assert!(!doctor_report.checks.is_empty());
        let browser_sources = doctor_report
            .checks
            .iter()
            .find(|check| check.name == "Browser sources")
            .expect("browser sources check");
        assert!(!browser_sources.ok);
        assert!(browser_sources.detail.contains("No supported browser profiles"));

        let missing_archive_config = initialized_config(ArchiveMode::Plaintext);
        let missing_archive_status =
            archive_status(&empty_paths, &missing_archive_config, None).expect("missing status");
        assert!(!missing_archive_status.initialized);
        let now = Utc::now();
        assert!(
            backup_due_skip_reason_at(
                now - Duration::hours(missing_archive_config.due_after_hours as i64),
                &missing_archive_config,
                now
            )
            .is_none()
        );

        ensure_paths(&empty_paths).expect("ensure empty paths");
        let archive =
            Connection::open(&empty_paths.archive_database_path).expect("open empty archive");
        create_schema(&archive).expect("create schema");
        archive
            .execute(
                "INSERT INTO backup_runs (started_at, finished_at, status, due_only, profiles_json, summary_json)
                 VALUES ('2026-04-01T00:00:00Z', '2026-04-01T00:05:00Z', 'success', 0, '[]', '{\"newVisits\":1}')",
                [],
            )
            .expect("insert backup run");
        drop(archive);
        assert!(
            load_recent_runs(&empty_paths, &empty_config, None)
                .expect("recent runs with uninitialized config")
                .is_empty()
        );

        let run_dir = tempdir().expect("run dir");
        let run_paths = sample_paths(run_dir.path());
        ensure_paths(&run_paths).expect("ensure paths");
        let chrome_root = chrome_user_data_fixture(run_dir.path());
        let config = initialized_config(ArchiveMode::Plaintext);
        save_config(&run_paths, &config).expect("save config");
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, run_dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        }
        run_backup(&run_paths, &config, None, false).expect("run backup");
        let markdown_export = export_history(
            &run_paths,
            &config,
            None,
            ExportRequest { format: ExportFormat::Markdown, query: HistoryQuery::default() },
        )
        .expect("markdown export");
        let text_export = export_history(
            &run_paths,
            &config,
            None,
            ExportRequest { format: ExportFormat::Text, query: HistoryQuery::default() },
        )
        .expect("text export");
        let jsonl_export = export_history(
            &run_paths,
            &config,
            None,
            ExportRequest { format: ExportFormat::Jsonl, query: HistoryQuery::default() },
        )
        .expect("jsonl export");
        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        }
        assert!(markdown_export.path.ends_with(".md"));
        assert!(text_export.path.ends_with(".txt"));
        assert!(jsonl_export.path.ends_with(".jsonl"));
        assert!(
            fs::read_to_string(&jsonl_export.path).expect("read jsonl").contains("example.com")
        );
    }
}
