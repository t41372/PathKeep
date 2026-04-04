use crate::{
    chrome::{ProfileSnapshot, discover_profiles, stage_profile_snapshot},
    config::{ProjectPaths, ensure_paths, save_config},
    git_audit,
    models::{
        AppConfig, ArchiveMode, ArchiveStatus, BackupProfileSummary, BackupReport,
        BackupRunOverview, ExportFormat, ExportRequest, ExportResult, HealthCheck, HealthReport,
        HistoryEntry, HistoryQuery, HistoryQueryResponse,
    },
    utils::{chrome_time_to_rfc3339, now_rfc3339, sha256_hex, sqlite_row_to_json, url_domain},
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
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
    let mut statement = connection.prepare(
        "SELECT id, started_at, finished_at, status, manifest_hash, summary_json FROM backup_runs ORDER BY id DESC LIMIT 12",
    )?;
    let rows = statement.query_map([], |row| {
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
            profiles_processed: summary
                .get("profilesProcessed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize,
            new_visits: summary.get("newVisits").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            new_urls: summary.get("newUrls").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            new_downloads: summary.get("newDownloads").and_then(|v| v.as_u64()).unwrap_or(0)
                as usize,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
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

    if due_only {
        if let Some(last_backup_at) = latest_successful_backup_at(&archive)? {
            let elapsed = Utc::now() - last_backup_at;
            if elapsed < Duration::hours(config.due_after_hours as i64) {
                return Ok(BackupReport {
                    due_skipped: true,
                    reason: Some(format!(
                        "last successful backup is only {} hours old",
                        elapsed.num_hours()
                    )),
                    ..BackupReport::default()
                });
            }
        }
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
    archive.execute(
        "INSERT INTO backup_runs (started_at, status, due_only, profiles_json) VALUES (?1, 'running', ?2, ?3)",
        params![started_at, due_only as i64, serde_json::to_string(&selected_profiles)?],
    )?;
    let run_id = archive.last_insert_rowid();
    let previous_manifest_hash = latest_manifest_hash(&archive)?;

    let mut profile_summaries = Vec::new();
    let mut source_hashes = BTreeMap::<String, BTreeMap<String, String>>::new();
    let warnings = Vec::new();

    for profile in &selected_profiles {
        let snapshot = stage_profile_snapshot(paths, profile)?;
        let profile_summary = process_profile_snapshot(&archive, run_id, paths, config, &snapshot)
            .with_context(|| format!("processing profile {}", profile.profile_id))?;
        let mut hashes = BTreeMap::new();
        for fingerprint in &snapshot.source_hashes {
            hashes.insert(fingerprint.path.clone(), fingerprint.sha256.clone());
        }
        source_hashes.insert(profile.profile_id.clone(), hashes);
        profile_summaries.push(profile_summary);
    }

    let finished_at = now_rfc3339();
    let summary = BackupRunOverview {
        id: run_id,
        started_at: started_at.clone(),
        finished_at: Some(finished_at.clone()),
        status: "success".to_string(),
        manifest_hash: None,
        profiles_processed: profile_summaries.len(),
        new_visits: profile_summaries.iter().map(|item| item.new_visits).sum(),
        new_urls: profile_summaries.iter().map(|item| item.new_urls).sum(),
        new_downloads: profile_summaries.iter().map(|item| item.new_downloads).sum(),
    };

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
    let manifest_path = git_audit::write_audit_file(
        &paths.audit_repo_path,
        &manifest_relative_path,
        &manifest_json,
    )?;

    let summary_json = serde_json::to_string(&json!({
        "profilesProcessed": summary.profiles_processed,
        "newVisits": summary.new_visits,
        "newUrls": summary.new_urls,
        "newDownloads": summary.new_downloads,
    }))?;

    archive.execute(
        "UPDATE backup_runs
         SET finished_at = ?1, status = 'success', manifest_path = ?2, manifest_hash = ?3, previous_manifest_hash = ?4, summary_json = ?5
         WHERE id = ?6",
        params![
            finished_at,
            manifest_path.display().to_string(),
            manifest_hash,
            previous_manifest_hash,
            summary_json,
            run_id
        ],
    )?;
    archive.execute(
        "INSERT INTO manifests (run_id, manifest_hash, previous_manifest_hash, path, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            run_id,
            manifest_hash,
            manifest.previous_manifest_hash,
            manifest_path.display().to_string(),
            manifest.created_at
        ],
    )?;

    let git_commit = if config.git_enabled {
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
    let limit = query.limit.unwrap_or(150).clamp(1, 1000);
    let q = query.q.clone().filter(|value| !value.trim().is_empty());
    let domain_pattern = query
        .domain
        .clone()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{value}%"));

    let mut statement = connection.prepare(
        "SELECT id, profile_id, url, title, visit_time, visit_duration, transition, source_visit_id, app_id
         FROM visit_events
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR url LIKE '%' || ?2 || '%' OR IFNULL(title, '') LIKE '%' || ?2 || '%')
           AND (?3 IS NULL OR url LIKE ?3)
         ORDER BY visit_time DESC
         LIMIT ?4",
    )?;
    let rows = statement.query_map(params![query.profile_id, q, domain_pattern, limit], |row| {
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
    })?;
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
    let extension = match request.format {
        ExportFormat::Html => "html",
        ExportFormat::Markdown => "md",
        ExportFormat::Text => "txt",
        ExportFormat::Jsonl => "jsonl",
    };
    let file_name = format!("export-{}.{}", now_rfc3339().replace(':', "-"), extension);
    let target_path = paths.exports_dir.join(file_name);
    let content = match request.format {
        ExportFormat::Html => render_html_export(&results),
        ExportFormat::Markdown => render_markdown_export(&results),
        ExportFormat::Text => render_text_export(&results),
        ExportFormat::Jsonl => results
            .items
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n"),
    };
    fs::write(&target_path, content)
        .with_context(|| format!("writing {}", target_path.display()))?;
    Ok(ExportResult {
        format: request.format,
        path: target_path.display().to_string(),
        count: results.items.len(),
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
    let chrome_root = crate::chrome::chrome_user_data_dir().ok();
    let mut checks = Vec::new();
    checks.push(HealthCheck {
        name: "Config".to_string(),
        ok: paths.config_path.exists(),
        detail: paths.config_path.display().to_string(),
    });
    checks.push(HealthCheck {
        name: "Chrome User Data".to_string(),
        ok: chrome_root.as_ref().is_some_and(|path| path.exists()),
        detail: chrome_root
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Chrome user data path unavailable".to_string()),
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
    connection.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS profiles (
          profile_id TEXT PRIMARY KEY,
          profile_name TEXT NOT NULL,
          user_name TEXT,
          profile_path TEXT NOT NULL,
          chrome_version TEXT,
          updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS source_schemas (
          schema_hash TEXT PRIMARY KEY,
          source_kind TEXT NOT NULL,
          chrome_version TEXT,
          payload_json TEXT NOT NULL,
          seen_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS backup_runs (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          started_at TEXT NOT NULL,
          finished_at TEXT,
          status TEXT NOT NULL,
          due_only INTEGER NOT NULL,
          profiles_json TEXT NOT NULL,
          manifest_path TEXT,
          manifest_hash TEXT,
          previous_manifest_hash TEXT,
          summary_json TEXT
        );
        CREATE TABLE IF NOT EXISTS manifests (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          run_id INTEGER NOT NULL UNIQUE,
          manifest_hash TEXT NOT NULL UNIQUE,
          previous_manifest_hash TEXT,
          path TEXT NOT NULL,
          created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS import_batches (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          source_kind TEXT NOT NULL,
          source_path TEXT NOT NULL,
          profile_id TEXT NOT NULL,
          created_at TEXT NOT NULL,
          imported_at TEXT,
          reverted_at TEXT,
          status TEXT NOT NULL,
          summary_json TEXT NOT NULL,
          audit_path TEXT,
          git_commit TEXT
        );
        CREATE TABLE IF NOT EXISTS raw_row_versions (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          run_id INTEGER NOT NULL,
          profile_id TEXT NOT NULL,
          source_kind TEXT NOT NULL,
          table_name TEXT NOT NULL,
          source_pk TEXT NOT NULL,
          payload_hash TEXT NOT NULL,
          payload_json TEXT NOT NULL,
          schema_hash TEXT NOT NULL,
          chrome_version TEXT,
          recorded_at TEXT NOT NULL,
          UNIQUE(profile_id, source_kind, table_name, source_pk, payload_hash)
        );
        CREATE TABLE IF NOT EXISTS url_versions (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          profile_id TEXT NOT NULL,
          source_url_id INTEGER NOT NULL,
          url TEXT NOT NULL,
          title TEXT,
          visit_count INTEGER,
          typed_count INTEGER,
          last_visit_time INTEGER NOT NULL,
          hidden INTEGER NOT NULL,
          payload_hash TEXT NOT NULL,
          recorded_at TEXT NOT NULL,
          UNIQUE(profile_id, source_url_id, payload_hash)
        );
        CREATE TABLE IF NOT EXISTS visit_events (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          profile_id TEXT NOT NULL,
          source_visit_id INTEGER NOT NULL,
          source_url_id INTEGER NOT NULL,
          url TEXT NOT NULL,
          title TEXT,
          visit_time INTEGER NOT NULL,
          from_visit INTEGER,
          transition INTEGER,
          visit_duration INTEGER,
          is_known_to_sync INTEGER,
          visited_link_id INTEGER,
          external_referrer_url TEXT,
          app_id TEXT,
          payload_hash TEXT NOT NULL,
          recorded_at TEXT NOT NULL,
          UNIQUE(profile_id, source_visit_id)
        );
        CREATE TABLE IF NOT EXISTS download_versions (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          profile_id TEXT NOT NULL,
          source_download_id INTEGER NOT NULL,
          guid TEXT,
          current_path TEXT,
          target_path TEXT,
          start_time INTEGER,
          total_bytes INTEGER,
          received_bytes INTEGER,
          state INTEGER,
          mime_type TEXT,
          original_mime_type TEXT,
          payload_hash TEXT NOT NULL,
          recorded_at TEXT NOT NULL,
          UNIQUE(profile_id, source_download_id, payload_hash)
        );
        CREATE TABLE IF NOT EXISTS search_terms (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          profile_id TEXT NOT NULL,
          keyword_id INTEGER NOT NULL,
          url_id INTEGER NOT NULL,
          term TEXT NOT NULL,
          normalized_term TEXT NOT NULL,
          recorded_at TEXT NOT NULL,
          UNIQUE(profile_id, keyword_id, url_id, term, normalized_term)
        );
        CREATE TABLE IF NOT EXISTS favicons (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          profile_id TEXT NOT NULL,
          page_url TEXT NOT NULL,
          icon_url TEXT NOT NULL,
          icon_type INTEGER,
          width INTEGER,
          height INTEGER,
          last_updated INTEGER,
          image_data BLOB,
          payload_hash TEXT NOT NULL,
          recorded_at TEXT NOT NULL,
          UNIQUE(profile_id, page_url, icon_url, payload_hash)
        );
        CREATE TABLE IF NOT EXISTS profile_watermarks (
          profile_id TEXT PRIMARY KEY,
          last_visit_id INTEGER NOT NULL DEFAULT 0,
          last_url_last_visit_time INTEGER NOT NULL DEFAULT 0,
          last_download_id INTEGER NOT NULL DEFAULT 0,
          last_favicon_last_updated INTEGER NOT NULL DEFAULT 0,
          last_checkpoint_at TEXT,
          last_schema_hash TEXT,
          updated_at TEXT NOT NULL
        );",
    )?;
    ensure_column(connection, "visit_events", "import_batch_id", "INTEGER")?;
    ensure_column(connection, "raw_row_versions", "import_batch_id", "INTEGER")?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_visit_events_import_batch_id ON visit_events(import_batch_id)",
        [],
    )?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_raw_row_versions_import_batch_id ON raw_row_versions(import_batch_id)",
        [],
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
        connection.execute(
            &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}"),
            [],
        )?;
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

fn latest_manifest_hash(connection: &Connection) -> Result<Option<String>> {
    Ok(connection
        .query_row("SELECT manifest_hash FROM manifests ORDER BY id DESC LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()?)
}

fn open_readonly_source(path: &Path) -> Result<Connection> {
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?)
}

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
    archive.execute(
        "INSERT OR IGNORE INTO source_schemas (schema_hash, source_kind, chrome_version, payload_json, seen_at)
         VALUES (?1, 'chrome-history', ?2, ?3, ?4)",
        params![
            schema_hash,
            snapshot.profile.chrome_version,
            schema_string,
            now_rfc3339()
        ],
    )?;
    archive.execute(
        "INSERT INTO profiles (profile_id, profile_name, user_name, profile_path, chrome_version, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(profile_id) DO UPDATE SET
           profile_name = excluded.profile_name,
           user_name = excluded.user_name,
           profile_path = excluded.profile_path,
           chrome_version = excluded.chrome_version,
           updated_at = excluded.updated_at",
        params![
            snapshot.profile.profile_id,
            snapshot.profile.profile_name,
            snapshot.profile.user_name,
            snapshot.profile.profile_path,
            snapshot.profile.chrome_version,
            now_rfc3339()
        ],
    )?;

    let mut watermark = load_watermark(archive, &snapshot.profile.profile_id)?;
    let mut summary = BackupProfileSummary {
        profile_id: snapshot.profile.profile_id.clone(),
        ..BackupProfileSummary::default()
    };

    let max_url_last_visit_time = ingest_urls(
        archive,
        run_id,
        &snapshot.profile.profile_id,
        &history,
        &schema_hash,
        &mut summary,
        watermark.last_url_last_visit_time,
    )?;
    let max_visit_id = ingest_visits(
        archive,
        run_id,
        &snapshot.profile.profile_id,
        &history,
        &schema_hash,
        &mut summary,
        watermark.last_visit_id,
    )?;
    let max_download_id = ingest_downloads(
        archive,
        run_id,
        &snapshot.profile.profile_id,
        &history,
        &schema_hash,
        &mut summary,
        watermark.last_download_id,
    )?;
    ingest_search_terms(archive, &snapshot.profile.profile_id, &history, max_url_last_visit_time)?;

    let mut max_favicon_last_updated = watermark.last_favicon_last_updated;
    if config.capture_favicons {
        if let Some(favicons_path) = &snapshot.favicons_path {
            let favicons = open_readonly_source(favicons_path)?;
            max_favicon_last_updated = ingest_favicons(
                archive,
                &snapshot.profile.profile_id,
                &favicons,
                watermark.last_favicon_last_updated,
            )?;
        }
    }

    let checkpoint_due = should_checkpoint(&watermark, &schema_hash, config.checkpoint_days);
    if checkpoint_due {
        checkpoint_snapshot(paths, snapshot)?;
        summary.checkpoint_created = true;
    }

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
    let mut statement = source.prepare(
        "SELECT id, url, title, visit_count, typed_count, last_visit_time, hidden
         FROM urls
         WHERE last_visit_time >= ?1
         ORDER BY last_visit_time ASC",
    )?;
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
        let inserted = archive.execute(
            "INSERT OR IGNORE INTO url_versions
             (profile_id, source_url_id, url, title, visit_count, typed_count, last_visit_time, hidden, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                profile_id,
                source_url_id,
                url,
                title,
                visit_count,
                typed_count,
                last_visit_time,
                hidden,
                payload_hash,
                now_rfc3339()
            ],
        )?;
        summary.new_urls += inserted as usize;
        summary.raw_rows += insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                profile_id,
                source_kind: "chrome-history",
                table_name: "urls",
                source_pk: &source_url_id.to_string(),
                payload_hash: &payload_hash,
                payload_json: &payload_string,
                schema_hash,
            },
        )? as usize;
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
    let mut statement = source.prepare(
        "SELECT visits.id, visits.url, urls.url, urls.title, visits.visit_time, visits.from_visit,
                visits.transition, visits.visit_duration, visits.is_known_to_sync,
                visits.visited_link_id, visits.external_referrer_url, visits.app_id
         FROM visits
         JOIN urls ON urls.id = visits.url
         WHERE visits.id > ?1
         ORDER BY visits.id ASC",
    )?;
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
        let inserted = archive.execute(
            "INSERT OR IGNORE INTO visit_events
             (profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition,
              visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                profile_id,
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
                payload_hash,
                now_rfc3339()
            ],
        )?;
        summary.new_visits += inserted as usize;
        summary.raw_rows += insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                profile_id,
                source_kind: "chrome-history",
                table_name: "visits",
                source_pk: &source_visit_id.to_string(),
                payload_hash: &payload_hash,
                payload_json: &payload_string,
                schema_hash,
            },
        )? as usize;
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
    let mut statement = source.prepare(
        "SELECT id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type
         FROM downloads
         WHERE id > ?1
         ORDER BY id ASC",
    )?;
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
        let inserted = archive.execute(
            "INSERT OR IGNORE INTO download_versions
             (profile_id, source_download_id, guid, current_path, target_path, start_time, total_bytes, received_bytes, state, mime_type, original_mime_type, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                profile_id,
                source_download_id,
                guid,
                current_path,
                target_path,
                start_time,
                total_bytes,
                received_bytes,
                state,
                mime_type,
                original_mime_type,
                payload_hash,
                now_rfc3339()
            ],
        )?;
        summary.new_downloads += inserted as usize;
        summary.raw_rows += insert_raw_row(
            archive,
            RawRowInsert {
                run_id,
                profile_id,
                source_kind: "chrome-history",
                table_name: "downloads",
                source_pk: &source_download_id.to_string(),
                payload_hash: &payload_hash,
                payload_json: &payload_string,
                schema_hash,
            },
        )? as usize;
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
    let mut statement = source.prepare(
        "SELECT keyword_id, url_id, term, normalized_term
         FROM keyword_search_terms
         WHERE url_id IN (SELECT id FROM urls WHERE last_visit_time >= ?1)",
    )?;
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
        archive.execute(
            "INSERT OR IGNORE INTO search_terms (profile_id, keyword_id, url_id, term, normalized_term, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![profile_id, keyword_id, url_id, term, normalized_term, now_rfc3339()],
        )?;
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
    let mut statement = source.prepare(
        "SELECT icon_mapping.page_url, favicons.url, favicons.icon_type,
                IFNULL(favicon_bitmaps.width, 0), IFNULL(favicon_bitmaps.height, 0),
                IFNULL(favicon_bitmaps.last_updated, 0), favicon_bitmaps.image_data
         FROM icon_mapping
         JOIN favicons ON favicons.id = icon_mapping.icon_id
         LEFT JOIN favicon_bitmaps ON favicon_bitmaps.icon_id = favicons.id
         WHERE IFNULL(favicon_bitmaps.last_updated, 0) >= ?1
         ORDER BY IFNULL(favicon_bitmaps.last_updated, 0) ASC",
    )?;
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
        archive.execute(
            "INSERT OR IGNORE INTO favicons
             (profile_id, page_url, icon_url, icon_type, width, height, last_updated, image_data, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                profile_id,
                page_url,
                icon_url,
                icon_type,
                width,
                height,
                last_updated,
                image_data,
                payload_hash,
                now_rfc3339()
            ],
        )?;
        max_last_updated = max_last_updated.max(last_updated);
    }
    Ok(max_last_updated)
}

fn insert_raw_row(archive: &Connection, row: RawRowInsert<'_>) -> Result<usize> {
    Ok(archive.execute(
        "INSERT OR IGNORE INTO raw_row_versions
         (run_id, profile_id, source_kind, table_name, source_pk, payload_hash, payload_json, schema_hash, recorded_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.run_id,
            row.profile_id,
            row.source_kind,
            row.table_name,
            row.source_pk,
            row.payload_hash,
            row.payload_json,
            row.schema_hash,
            now_rfc3339()
        ],
    )?)
}

struct RawRowInsert<'a> {
    run_id: i64,
    profile_id: &'a str,
    source_kind: &'a str,
    table_name: &'a str,
    source_pk: &'a str,
    payload_hash: &'a str,
    payload_json: &'a str,
    schema_hash: &'a str,
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
    archive.execute(
        "INSERT INTO profile_watermarks
         (profile_id, last_visit_id, last_url_last_visit_time, last_download_id, last_favicon_last_updated, last_checkpoint_at, last_schema_hash, updated_at)
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
            watermark.updated_at
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
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Chrome History Backup Export</title><style>body{{font-family:ui-sans-serif,system-ui;padding:32px;background:#f4efe6;color:#1e1b16}}ol{{display:grid;gap:16px}}li{{padding:16px;border-top:1px solid rgba(30,27,22,.15)}}a{{color:#0a6c74;text-decoration:none}}p{{margin:6px 0 0;color:#5d5548}}</style></head><body><h1>Chrome History Backup Export</h1><ol>{rows}</ol></body></html>"
    )
}

fn render_markdown_export(results: &HistoryQueryResponse) -> String {
    let mut output = String::from("# Chrome History Backup Export\n\n");
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
        config::{ensure_paths, save_config},
        models::{AppConfig, ArchiveMode},
        utils::iso_to_chrome_time_micros,
    };
    use std::path::PathBuf;
    use tempfile::tempdir;

    const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
    const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";

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

    fn initialized_config(mode: ArchiveMode) -> AppConfig {
        AppConfig {
            initialized: true,
            archive_mode: mode,
            git_enabled: false,
            due_after_hours: 72,
            checkpoint_days: 1,
            ..AppConfig::default()
        }
    }

    fn chrome_user_data_fixture(root: &Path) -> PathBuf {
        let chrome_root = root.join("chrome-user-data");
        let profile_dir = chrome_root.join("Default");
        fs::create_dir_all(&profile_dir).expect("create chrome profile dir");
        fs::write(chrome_root.join("Last Version"), "135.0.0.0").expect("write version");
        fs::write(
            chrome_root.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"tester@example.com"}}}}"#,
        )
        .expect("write local state");

        let history = Connection::open(profile_dir.join("History")).expect("open source history");
        history
            .execute_batch(
                "
                CREATE TABLE urls (
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
            .expect("create history schema");

        let visit_time =
            iso_to_chrome_time_micros("2026-04-01T10:00:00+00:00").expect("chrome time");
        history
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (1, 'https://example.com/<item>?q=1', 'Title <One>', 1, 1, ?1, 0)",
                [visit_time],
            )
            .expect("insert url");
        history
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES (1, 1, ?1, NULL, 805306368, 24000, 1, 3, 'https://ref.example', 'com.example.app')",
                [visit_time],
            )
            .expect("insert visit");
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

    #[test]
    fn backup_history_export_and_rekey_work_end_to_end() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let config = initialized_config(ArchiveMode::Plaintext);
        save_config(&paths, &config).expect("save config");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        }

        let report = run_backup(&paths, &config, None, false).expect("run backup");
        assert!(!report.due_skipped);
        assert_eq!(report.run.as_ref().expect("run").new_visits, 1);
        assert_eq!(report.run.as_ref().expect("run").new_urls, 1);
        assert_eq!(report.run.as_ref().expect("run").new_downloads, 1);
        assert!(paths.audit_repo_path.join(".git").exists());

        let history = list_history(
            &paths,
            &config,
            None,
            HistoryQuery {
                q: Some("Title".to_string()),
                domain: Some("example.com".to_string()),
                profile_id: Some("Default".to_string()),
                limit: Some(10),
            },
        )
        .expect("list history");
        assert_eq!(history.total, 1);
        assert_eq!(history.items[0].domain, "example.com");

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
        assert!(html.contains("Chrome History Backup Export"));

        let due_report = run_backup(&paths, &config, None, true).expect("due backup");
        assert!(due_report.due_skipped);

        let health = doctor(&paths, &config, None).expect("doctor");
        assert!(health.checks.iter().any(|check| check.name == "Archive DB" && check.ok));

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

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        }
    }

    #[test]
    fn schema_migration_helpers_and_text_renderers_are_stable() {
        let connection = Connection::open_in_memory().expect("db");
        connection
            .execute("CREATE TABLE visit_events (id INTEGER PRIMARY KEY)", [])
            .expect("create visit_events");
        ensure_column(&connection, "visit_events", "import_batch_id", "INTEGER")
            .expect("add column");
        ensure_column(&connection, "visit_events", "import_batch_id", "INTEGER")
            .expect("idempotent");

        let results = HistoryQueryResponse {
            total: 1,
            items: vec![HistoryEntry {
                id: 1,
                profile_id: "Default".to_string(),
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
    }
}
