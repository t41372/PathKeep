use crate::{
    archive::{create_schema, open_archive_connection},
    config::{ProjectPaths, ensure_paths},
    git_audit,
    models::{
        AppConfig, ImportBatchDetail, ImportBatchOverview, TakeoutFileReport, TakeoutInspection,
        TakeoutPreviewEntry, TakeoutRequest,
    },
    utils::{chrome_time_to_rfc3339, iso_to_chrome_time_micros, now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;
use zip::ZipArchive;

const PREVIEW_LIMIT: usize = 24;

#[derive(Debug, Clone)]
struct TakeoutFile {
    path: String,
    from_zip: bool,
}

#[derive(Debug, Clone)]
struct ParsedTakeoutRecord {
    source_path: String,
    url: String,
    title: Option<String>,
    visit_time: i64,
    payload_hash: String,
    payload_json: String,
    source_visit_id: i64,
}

#[derive(Debug, Default)]
struct ImportStats {
    imported_items: usize,
    duplicate_items: usize,
}

#[derive(Debug, Clone)]
struct ImportBatchRecord {
    overview: ImportBatchOverview,
    recognized_files: Vec<TakeoutFileReport>,
    quarantined_files: Vec<TakeoutFileReport>,
    notes: Vec<String>,
}

pub fn inspect_takeout(
    _paths: &ProjectPaths,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    let files = gather_takeout_files(Path::new(&request.source_path))?;
    let mut inspection = TakeoutInspection {
        source_path: request.source_path.clone(),
        dry_run: request.dry_run,
        ..TakeoutInspection::default()
    };

    for file in files {
        let Some(kind) = recognize_takeout_file(&file.path) else {
            inspection.quarantined_files.push(TakeoutFileReport {
                path: file.path,
                kind: "unknown".to_string(),
                status: "quarantine".to_string(),
                records: 0,
            });
            continue;
        };

        let mut report = TakeoutFileReport {
            path: file.path.clone(),
            kind: kind.clone(),
            status: "recognized".to_string(),
            records: 0,
        };

        if kind == "takeout-index" {
            inspection.recognized_files.push(report);
            continue;
        }

        let bytes = if file.from_zip {
            read_zip_entry(Path::new(&request.source_path), &file.path)?
        } else {
            fs::read(&file.path)?
        };

        match collect_records_from_payload(&file.path, &kind, &bytes) {
            Ok(records) => {
                report.records = records.len();
                report.status = "previewed".to_string();
                inspection.candidate_items += records.len();
                for record in records {
                    if inspection.preview_entries.len() < PREVIEW_LIMIT {
                        inspection.preview_entries.push(preview_entry(&record, "candidate"));
                    }
                }
            }
            Err(error) => {
                report.status = "parse-error".to_string();
                inspection.notes.push(format!("Could not parse {}: {}", file.path, error));
            }
        }

        inspection.recognized_files.push(report);
    }

    if inspection.recognized_files.is_empty() {
        inspection.notes.push(
            "No directly importable history files were detected. Dry-run still captured the archive structure."
                .to_string(),
        );
    }

    Ok(inspection)
}

pub fn import_takeout(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    ensure_paths(paths)?;
    let mut inspection = inspect_takeout(paths, request)?;
    if request.dry_run {
        return Ok(inspection);
    }

    if !config.initialized {
        anyhow::bail!("archive must be initialized before importing takeout data")
    }

    let source = Path::new(&request.source_path);
    let synthetic_profile = format!(
        "takeout::{}",
        source.file_stem().and_then(|name| name.to_str()).unwrap_or("archive")
    );

    let mut archive = open_archive_connection(paths, config, key)?;
    create_schema(&archive)?;
    let transaction = archive.transaction()?;
    upsert_takeout_profile(&transaction, &synthetic_profile, source)?;

    let batch_id = create_import_batch(&transaction, &synthetic_profile, request, &inspection)?;
    let files = gather_takeout_files(source)?;
    let mut stats = ImportStats::default();

    for file in files {
        let Some(kind) = recognize_takeout_file(&file.path) else {
            quarantine_file(paths, source, &file.path)?;
            continue;
        };
        if kind == "takeout-index" {
            continue;
        }

        let bytes =
            if file.from_zip { read_zip_entry(source, &file.path)? } else { fs::read(&file.path)? };
        let file_stats = import_supported_payload(
            &transaction,
            batch_id,
            &synthetic_profile,
            &file.path,
            &kind,
            &bytes,
        )?;
        stats.imported_items += file_stats.imported_items;
        stats.duplicate_items += file_stats.duplicate_items;
    }

    inspection.imported_items = stats.imported_items;
    inspection.duplicate_items = stats.duplicate_items;
    finalize_import_batch(&transaction, batch_id, &inspection)?;
    transaction.commit()?;

    let (audit_path, git_commit) = write_batch_audit(paths, config, batch_id, key, "imported")?;
    update_batch_audit(paths, config, key, batch_id, audit_path.as_deref(), git_commit.as_deref())?;

    let detail = preview_import_batch(paths, config, key, batch_id)?;
    inspection.import_batch = Some(detail.batch);
    Ok(inspection)
}

pub fn load_import_batches(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Vec<ImportBatchOverview>> {
    if !config.initialized || !paths.archive_database_path.exists() {
        return Ok(Vec::new());
    }

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let mut statement = connection.prepare(
        "SELECT
            id,
            source_kind,
            source_path,
            profile_id,
            created_at,
            imported_at,
            reverted_at,
            status,
            summary_json,
            audit_path,
            git_commit,
            (SELECT COUNT(*) FROM visit_events WHERE import_batch_id = import_batches.id) AS visible_items
         FROM import_batches
         ORDER BY id DESC
         LIMIT 16",
    )?;
    let rows = statement.query_map([], |row| {
        let summary_json: String = row.get(8)?;
        let visible_items: i64 = row.get(11)?;
        Ok(import_batch_overview_from_summary(ImportBatchOverviewRow {
            id: row.get(0)?,
            source_kind: row.get(1)?,
            source_path: row.get(2)?,
            profile_id: row.get(3)?,
            created_at: row.get(4)?,
            imported_at: row.get(5)?,
            reverted_at: row.get(6)?,
            status: row.get(7)?,
            summary_json,
            visible_items: visible_items.max(0) as usize,
            audit_path: row.get(9)?,
            git_commit: row.get(10)?,
        }))
    })?;

    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn preview_import_batch(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let batch = load_import_batch_record(&connection, batch_id)?
        .with_context(|| format!("import batch {batch_id} was not found"))?;

    let mut statement = connection.prepare(
        "SELECT payload_json
         FROM raw_row_versions
         WHERE import_batch_id = ?1
         ORDER BY id DESC
         LIMIT ?2",
    )?;
    let rows = statement
        .query_map(params![batch_id, PREVIEW_LIMIT as i64], |row| row.get::<_, String>(0))?;
    let preview_entries = rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|payload_json| {
            preview_entry_from_payload(&batch.overview.source_path, &payload_json).ok()
        })
        .collect::<Vec<_>>();

    Ok(ImportBatchDetail {
        batch: batch.overview,
        preview_entries,
        recognized_files: batch.recognized_files,
        quarantined_files: batch.quarantined_files,
        notes: batch.notes,
    })
}

pub fn revert_import_batch(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let mut connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let transaction = connection.transaction()?;

    let existing = load_import_batch_record(&transaction, batch_id)?
        .with_context(|| format!("import batch {batch_id} was not found"))?;
    if existing.overview.status == "reverted" {
        drop(transaction);
        return preview_import_batch(paths, config, key, batch_id);
    }

    let removed =
        transaction.execute("DELETE FROM visit_events WHERE import_batch_id = ?1", [batch_id])?;
    let mut notes = existing.notes.clone();
    notes.push(format!(
        "Reverted at {}. Removed {} live history rows from the archive view.",
        now_rfc3339(),
        removed
    ));
    update_batch_summary(
        &transaction,
        BatchSummaryUpdate {
            batch_id,
            status: "reverted",
            imported_items: existing.overview.imported_items,
            duplicate_items: existing.overview.duplicate_items,
            candidate_items: existing.overview.candidate_items,
            recognized_files: &existing.recognized_files,
            quarantined_files: &existing.quarantined_files,
            notes: &notes,
            reverted_at: Some(now_rfc3339()),
        },
    )?;
    transaction.commit()?;

    let (audit_path, git_commit) = write_batch_audit(paths, config, batch_id, key, "reverted")?;
    update_batch_audit(paths, config, key, batch_id, audit_path.as_deref(), git_commit.as_deref())?;

    preview_import_batch(paths, config, key, batch_id)
}

fn import_supported_payload(
    archive: &Transaction<'_>,
    batch_id: i64,
    profile_id: &str,
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<ImportStats> {
    let records = collect_records_from_payload(source_path, kind, bytes)?;
    let mut stats = ImportStats::default();

    for record in records {
        let inserted = archive.execute(
            "INSERT OR IGNORE INTO visit_events
             (profile_id, source_visit_id, source_url_id, url, title, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id, payload_hash, recorded_at, import_batch_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, NULL, 0, NULL, ?7, 'takeout', ?8, ?9, ?10)",
            params![
                profile_id,
                record.source_visit_id,
                record.source_visit_id,
                record.url,
                record.title,
                record.visit_time,
                record.source_path,
                record.payload_hash,
                now_rfc3339(),
                batch_id
            ],
        )?;

        if inserted > 0 {
            archive.execute(
                "INSERT OR IGNORE INTO raw_row_versions
                 (run_id, profile_id, source_kind, table_name, source_pk, payload_hash, payload_json, schema_hash, chrome_version, recorded_at, import_batch_id)
                 VALUES (0, ?1, 'takeout', 'records', ?2, ?3, ?4, 'takeout', 'takeout', ?5, ?6)",
                params![
                    profile_id,
                    record.source_visit_id.to_string(),
                    record.payload_hash,
                    record.payload_json,
                    now_rfc3339(),
                    batch_id
                ],
            )?;
            stats.imported_items += 1;
        } else {
            stats.duplicate_items += 1;
        }
    }

    Ok(stats)
}

fn create_import_batch(
    archive: &Transaction<'_>,
    profile_id: &str,
    request: &TakeoutRequest,
    inspection: &TakeoutInspection,
) -> Result<i64> {
    let summary_json = serde_json::to_string(&json!({
        "candidateItems": inspection.candidate_items,
        "importedItems": 0,
        "duplicateItems": 0,
        "recognizedFiles": inspection.recognized_files,
        "quarantinedFiles": inspection.quarantined_files,
        "notes": inspection.notes,
    }))?;
    archive.execute(
        "INSERT INTO import_batches (source_kind, source_path, profile_id, created_at, status, summary_json)
         VALUES ('takeout', ?1, ?2, ?3, 'running', ?4)",
        params![request.source_path, profile_id, now_rfc3339(), summary_json],
    )?;
    Ok(archive.last_insert_rowid())
}

fn finalize_import_batch(
    archive: &Transaction<'_>,
    batch_id: i64,
    inspection: &TakeoutInspection,
) -> Result<()> {
    update_batch_summary(
        archive,
        BatchSummaryUpdate {
            batch_id,
            status: "imported",
            imported_items: inspection.imported_items,
            duplicate_items: inspection.duplicate_items,
            candidate_items: inspection.candidate_items,
            recognized_files: &inspection.recognized_files,
            quarantined_files: &inspection.quarantined_files,
            notes: &inspection.notes,
            reverted_at: None,
        },
    )?;
    archive.execute(
        "UPDATE import_batches SET imported_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), batch_id],
    )?;
    Ok(())
}

fn update_batch_summary(archive: &Connection, update: BatchSummaryUpdate<'_>) -> Result<()> {
    let summary_json = serde_json::to_string(&json!({
        "candidateItems": update.candidate_items,
        "importedItems": update.imported_items,
        "duplicateItems": update.duplicate_items,
        "recognizedFiles": update.recognized_files,
        "quarantinedFiles": update.quarantined_files,
        "notes": update.notes,
    }))?;
    archive.execute(
        "UPDATE import_batches
         SET status = ?1, summary_json = ?2, reverted_at = COALESCE(?3, reverted_at)
         WHERE id = ?4",
        params![update.status, summary_json, update.reverted_at, update.batch_id],
    )?;
    Ok(())
}

struct BatchSummaryUpdate<'a> {
    batch_id: i64,
    status: &'a str,
    imported_items: usize,
    duplicate_items: usize,
    candidate_items: usize,
    recognized_files: &'a [TakeoutFileReport],
    quarantined_files: &'a [TakeoutFileReport],
    notes: &'a [String],
    reverted_at: Option<String>,
}

fn upsert_takeout_profile(
    archive: &Transaction<'_>,
    profile_id: &str,
    source: &Path,
) -> Result<()> {
    archive.execute(
        "INSERT INTO profiles (profile_id, profile_name, user_name, profile_path, chrome_version, updated_at)
         VALUES (?1, ?2, NULL, ?3, 'takeout', ?4)
         ON CONFLICT(profile_id) DO UPDATE SET
           profile_name = excluded.profile_name,
           profile_path = excluded.profile_path,
           chrome_version = excluded.chrome_version,
           updated_at = excluded.updated_at",
        params![
            profile_id,
            format!("Takeout import {}", source.display()),
            source.display().to_string(),
            now_rfc3339()
        ],
    )?;
    Ok(())
}

fn write_batch_audit(
    paths: &ProjectPaths,
    config: &AppConfig,
    batch_id: i64,
    key: Option<&str>,
    action: &str,
) -> Result<(Option<String>, Option<String>)> {
    let detail = preview_import_batch(paths, config, key, batch_id)?;
    git_audit::ensure_repo(&paths.audit_repo_path)?;
    let file_name =
        format!("imports/{}/batch-{}-{}.json", &detail.batch.created_at[0..10], batch_id, action);
    let contents = serde_json::to_string_pretty(&detail)?;
    let audit_path = git_audit::write_audit_file(&paths.audit_repo_path, &file_name, &contents)?;
    let git_commit = if config.git_enabled {
        git_audit::commit_all(&paths.audit_repo_path, &format!("import batch {batch_id} {action}"))?
    } else {
        None
    };
    Ok((Some(audit_path.display().to_string()), git_commit))
}

fn update_batch_audit(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    batch_id: i64,
    audit_path: Option<&str>,
    git_commit: Option<&str>,
) -> Result<()> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    connection.execute(
        "UPDATE import_batches SET audit_path = ?1, git_commit = ?2 WHERE id = ?3",
        params![audit_path, git_commit, batch_id],
    )?;
    Ok(())
}

fn load_import_batch_record(
    connection: &Connection,
    batch_id: i64,
) -> Result<Option<ImportBatchRecord>> {
    let row = connection
        .query_row(
            "SELECT
                id,
                source_kind,
                source_path,
                profile_id,
                created_at,
                imported_at,
                reverted_at,
                status,
                summary_json,
                audit_path,
                git_commit,
                (SELECT COUNT(*) FROM visit_events WHERE import_batch_id = import_batches.id) AS visible_items
             FROM import_batches
             WHERE id = ?1",
            [batch_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, i64>(11)?,
                ))
            },
        )
        .optional()?;

    let Some((
        id,
        source_kind,
        source_path,
        profile_id,
        created_at,
        imported_at,
        reverted_at,
        status,
        summary_json,
        audit_path,
        git_commit,
        visible_items,
    )) = row
    else {
        return Ok(None);
    };

    let summary: Value = serde_json::from_str(&summary_json).unwrap_or_else(|_| json!({}));
    let overview = import_batch_overview_from_summary(ImportBatchOverviewRow {
        id,
        source_kind,
        source_path,
        profile_id,
        created_at,
        imported_at,
        reverted_at,
        status,
        summary_json,
        visible_items: visible_items.max(0) as usize,
        audit_path,
        git_commit,
    });
    Ok(Some(ImportBatchRecord {
        overview,
        recognized_files: serde_json::from_value(
            summary.get("recognizedFiles").cloned().unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
        quarantined_files: serde_json::from_value(
            summary.get("quarantinedFiles").cloned().unwrap_or_else(|| json!([])),
        )
        .unwrap_or_default(),
        notes: serde_json::from_value(summary.get("notes").cloned().unwrap_or_else(|| json!([])))
            .unwrap_or_default(),
    }))
}

fn import_batch_overview_from_summary(row: ImportBatchOverviewRow) -> ImportBatchOverview {
    let summary: Value = serde_json::from_str(&row.summary_json).unwrap_or_else(|_| json!({}));
    ImportBatchOverview {
        id: row.id,
        source_kind: row.source_kind,
        source_path: row.source_path,
        profile_id: row.profile_id,
        created_at: row.created_at,
        imported_at: row.imported_at,
        reverted_at: row.reverted_at,
        status: row.status,
        candidate_items: summary_count(&summary, "candidateItems"),
        imported_items: summary_count(&summary, "importedItems"),
        duplicate_items: summary_count(&summary, "duplicateItems"),
        visible_items: row.visible_items,
        audit_path: row.audit_path,
        git_commit: row.git_commit,
    }
}

struct ImportBatchOverviewRow {
    id: i64,
    source_kind: String,
    source_path: String,
    profile_id: String,
    created_at: String,
    imported_at: Option<String>,
    reverted_at: Option<String>,
    status: String,
    summary_json: String,
    visible_items: usize,
    audit_path: Option<String>,
    git_commit: Option<String>,
}

fn summary_count(summary: &Value, key: &str) -> usize {
    summary.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
}

fn preview_entry_from_payload(
    source_path: &str,
    payload_json: &str,
) -> Result<TakeoutPreviewEntry> {
    let record: Value = serde_json::from_str(payload_json)?;
    let parsed = parse_record(source_path, 0, &record)?
        .context("payload did not include a usable history record")?;
    Ok(preview_entry(&parsed, "imported"))
}

fn preview_entry(record: &ParsedTakeoutRecord, status: &str) -> TakeoutPreviewEntry {
    TakeoutPreviewEntry {
        source_path: record.source_path.clone(),
        url: record.url.clone(),
        title: record.title.clone(),
        visited_at: chrome_time_to_rfc3339(record.visit_time),
        source_visit_id: record.source_visit_id,
        status: status.to_string(),
    }
}

fn collect_records_from_payload(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<Vec<ParsedTakeoutRecord>> {
    if kind == "jsonl" {
        let reader = BufReader::new(bytes);
        let mut records = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: Value = serde_json::from_str(&line)
                .with_context(|| format!("parsing {source_path} line {}", index + 1))?;
            if let Some(parsed) = parse_record(source_path, index as i64, &record)? {
                records.push(parsed);
            }
        }
        return Ok(records);
    }

    let value: Value = serde_json::from_slice(bytes)?;
    let records = if let Some(array) = value.as_array() {
        array.iter().enumerate().collect::<Vec<_>>()
    } else if let Some(array) = value.get("BrowserHistory").and_then(Value::as_array) {
        array.iter().enumerate().collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut parsed_records = Vec::new();
    for (index, record) in records {
        if let Some(parsed) = parse_record(source_path, index as i64, record)? {
            parsed_records.push(parsed);
        }
    }
    Ok(parsed_records)
}

fn parse_record(
    source_path: &str,
    ordinal: i64,
    record: &Value,
) -> Result<Option<ParsedTakeoutRecord>> {
    let url = record
        .get("url")
        .or_else(|| record.get("titleUrl"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if url.is_empty() {
        return Ok(None);
    }

    let title = record
        .get("title")
        .or_else(|| record.get("pageTitle"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let visit_time = record
        .get("visitTime")
        .and_then(Value::as_i64)
        .or_else(|| record.get("timeUsec").and_then(Value::as_i64))
        .or_else(|| {
            record.get("visitedAt").and_then(Value::as_str).and_then(iso_to_chrome_time_micros)
        })
        .unwrap_or_else(|| chrono::Utc::now().timestamp_micros() + 11_644_473_600_000_000);

    let payload_json = serde_json::to_string(record)?;
    let payload_hash = sha256_hex(payload_json.as_bytes());
    let source_visit_id = ((sha256_hex(format!("{source_path}:{ordinal}:{url}").as_bytes())
        [0..16])
        .bytes()
        .fold(0_i64, |acc, byte| acc.wrapping_mul(31).wrapping_add(byte as i64)))
    .abs();

    Ok(Some(ParsedTakeoutRecord {
        source_path: source_path.to_string(),
        url,
        title,
        visit_time,
        payload_hash,
        payload_json,
        source_visit_id,
    }))
}

fn gather_takeout_files(source: &Path) -> Result<Vec<TakeoutFile>> {
    if source.is_dir() {
        return Ok(WalkDir::new(source)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| TakeoutFile { path: entry.path().display().to_string(), from_zip: false })
            .collect());
    }

    let file = fs::File::open(source)?;
    let mut archive = ZipArchive::new(file)?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_file() {
            files.push(TakeoutFile { path: entry.name().to_string(), from_zip: true });
        }
    }
    Ok(files)
}

fn recognize_takeout_file(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".jsonl") {
        Some("jsonl".to_string())
    } else if lower.ends_with(".json") && (lower.contains("browser") || lower.contains("history")) {
        Some("browser-json".to_string())
    } else if lower.ends_with("archive_browser.html") {
        Some("takeout-index".to_string())
    } else {
        None
    }
}

fn read_zip_entry(source_zip: &Path, entry_name: &str) -> Result<Vec<u8>> {
    let file = fs::File::open(source_zip)?;
    let mut archive = ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn quarantine_file(paths: &ProjectPaths, source_root: &Path, path: &str) -> Result<()> {
    let destination = paths
        .quarantine_dir
        .join(source_root.file_stem().and_then(|name| name.to_str()).unwrap_or("takeout"))
        .join(PathBuf::from(path).file_name().unwrap_or_default());
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    if Path::new(path).exists() {
        fs::copy(path, destination)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        archive::create_schema,
        config::ensure_paths,
        models::{AppConfig, ArchiveMode},
    };
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

    fn initialized_plaintext_config() -> AppConfig {
        AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            ..AppConfig::default()
        }
    }

    fn write_takeout_fixture(dir: &Path) -> PathBuf {
        let source_dir = dir.join("takeout-source");
        fs::create_dir_all(&source_dir).expect("create takeout source dir");
        let source = source_dir.join("takeout.jsonl");
        fs::write(
            &source,
            [
                r#"{"url":"https://example.com/one","title":"One","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
                r#"{"url":"https://example.com/two","title":"Two","visitedAt":"2026-04-01T11:00:00+00:00"}"#,
            ]
            .join("\n"),
        )
        .expect("write takeout fixture");
        source_dir
    }

    #[test]
    fn inspect_takeout_collects_preview_rows() {
        let dir = tempdir().expect("tempdir");
        let source = write_takeout_fixture(dir.path());
        let inspection = inspect_takeout(
            &sample_paths(dir.path()),
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
        )
        .expect("inspect");

        assert_eq!(inspection.candidate_items, 2);
        assert_eq!(inspection.preview_entries.len(), 2);
        assert_eq!(inspection.recognized_files.len(), 1);
        assert!(inspection.quarantined_files.is_empty());
    }

    #[test]
    fn import_preview_and_revert_batch_are_reversible() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = initialized_plaintext_config();
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&archive).expect("schema");
        drop(archive);

        let source = write_takeout_fixture(dir.path());
        let inspection = import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect("import");

        let batch = inspection.import_batch.expect("import batch");
        assert_eq!(inspection.imported_items, 2);
        assert_eq!(batch.visible_items, 2);

        let preview = preview_import_batch(&paths, &config, None, batch.id).expect("preview batch");
        assert_eq!(preview.preview_entries.len(), 2);
        assert_eq!(preview.batch.status, "imported");

        let reverted = revert_import_batch(&paths, &config, None, batch.id).expect("revert batch");
        assert_eq!(reverted.batch.status, "reverted");
        assert_eq!(reverted.batch.visible_items, 0);
        assert!(reverted.notes.iter().any(|note| note.contains("Removed 2 live history rows")));
    }

    #[test]
    fn recognize_and_parse_takeout_payloads() {
        assert_eq!(recognize_takeout_file("BrowserHistory.json"), Some("browser-json".to_string()));
        assert_eq!(recognize_takeout_file("entries.jsonl"), Some("jsonl".to_string()));
        assert_eq!(
            recognize_takeout_file("archive_browser.html"),
            Some("takeout-index".to_string())
        );
        assert_eq!(recognize_takeout_file("notes.txt"), None);

        let records = collect_records_from_payload(
            "fixture.json",
            "browser-json",
            br#"{"BrowserHistory":[{"titleUrl":"https://example.com","pageTitle":"Example","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#,
        )
        .expect("collect");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].title.as_deref(), Some("Example"));
    }
}
