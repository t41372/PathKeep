//! Google Takeout inspection and import pipeline.
//!
//! Takeout import is handled as an explicit archive flow with preview,
//! quarantine, audit detail, and reversible visibility changes. This module
//! owns that pipeline end to end.

use crate::{
    archive::{
        SourceBatchInput, coverage_stats_json, create_schema, open_archive_connection,
        open_source_evidence_connection, persist_source_evidence, rebuild_search_projection,
        record_schema_observation, stats_with_archive_totals, upsert_source_batch,
        visit_event_fingerprint,
    },
    config::{ProjectPaths, ensure_paths},
    git_audit,
    models::{
        AppConfig, ImportBatchDetail, ImportBatchOverview, TakeoutFileReport, TakeoutInspection,
        TakeoutPreviewEntry, TakeoutRequest,
    },
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use browser_history_parser::{
    ParsedHistory,
    takeout::{
        KIND_INDEX, TakeoutPayloadReport, parse_payload as parse_takeout_payload,
        recognize_payload as recognize_takeout_payload,
    },
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
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
    visited_at: String,
    source_visit_id: i64,
}

#[derive(Debug, Default)]
struct ImportStats {
    imported_items: usize,
    duplicate_items: usize,
    skipped_items: usize,
}

#[derive(Debug)]
struct ImportedPayload {
    stats: ImportStats,
    source_evidence_plan: TakeoutSourceEvidencePlan,
}

struct CollectedPayload {
    report: TakeoutPayloadReport,
    records: Vec<ParsedTakeoutRecord>,
    skipped_missing_visit_time: usize,
}

#[derive(Debug, Clone)]
struct TakeoutSourceEvidencePlan {
    source_batch: SourceBatchInput,
    schema_observation: browser_history_parser::SchemaObservation,
    parsed_history: ParsedHistory,
}

#[derive(Debug, Clone)]
struct ImportBatchRecord {
    overview: ImportBatchOverview,
    recognized_files: Vec<TakeoutFileReport>,
    quarantined_files: Vec<TakeoutFileReport>,
    notes: Vec<String>,
}

/// Inspects a Takeout source and builds a preview-only import report.
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
            kind: kind.to_string(),
            status: "recognized".to_string(),
            records: 0,
        };

        if kind == KIND_INDEX {
            inspection.recognized_files.push(report);
            continue;
        }

        let bytes = if file.from_zip {
            read_zip_entry(Path::new(&request.source_path), &file.path)?
        } else {
            fs::read(&file.path)?
        };

        match collect_records_from_payload(&file.path, &kind, &bytes) {
            Ok(payload) => {
                report.records = payload.report.record_count;
                report.status = if payload.skipped_missing_visit_time > 0 {
                    "previewed-with-skips".to_string()
                } else {
                    "previewed".to_string()
                };
                inspection.candidate_items += payload.records.len();
                if payload.skipped_missing_visit_time > 0 {
                    inspection.notes.push(format!(
                        "Skipped {} records from {} because they were missing a visit timestamp.",
                        payload.skipped_missing_visit_time, file.path
                    ));
                }
                for record in payload.records {
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

/// Imports a Takeout source into the canonical archive.
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
    let synthetic_profile = "takeout::browser-history".to_string();
    let started_at = now_rfc3339();

    let mut archive = open_archive_connection(paths, config, key)?;
    create_schema(&archive)?;
    let run_id = create_import_run(&archive, &synthetic_profile, &started_at)?;
    let transaction = archive.transaction()?;
    let source_profile_id = upsert_takeout_profile(&transaction, &synthetic_profile, source)?;

    let batch_id = create_import_batch(&transaction, &synthetic_profile, request, &inspection)?;
    let files = gather_takeout_files(source)?;
    let mut stats = ImportStats::default();
    let mut source_evidence_plans = Vec::new();

    let import_result = (|| -> Result<()> {
        for file in files {
            let Some(kind) = recognize_takeout_file(&file.path) else {
                quarantine_takeout_file(paths, source, &file)?;
                continue;
            };
            if kind == KIND_INDEX {
                continue;
            }

            let bytes = if file.from_zip {
                read_zip_entry(source, &file.path)?
            } else {
                fs::read(&file.path)?
            };
            #[rustfmt::skip]
            let file_stats = import_supported_payload(
                &transaction,
                run_id,
                batch_id,
                source_profile_id,
                &file.path,
                &kind,
                &bytes,
            )?;
            stats.imported_items += file_stats.stats.imported_items;
            stats.duplicate_items += file_stats.stats.duplicate_items;
            source_evidence_plans.push(file_stats.source_evidence_plan);
            if file_stats.stats.skipped_items > 0 {
                inspection.notes.push(format!(
                    "Skipped {} records from {} because they were missing a visit timestamp.",
                    file_stats.stats.skipped_items, file.path
                ));
            }
        }

        transaction.commit()?;
        Ok(())
    })();

    if let Err(error) = import_result {
        finalize_failed_import_run(&archive, run_id, &inspection.notes, &stats, &error)?;
        return Err(error);
    }

    inspection.imported_items = stats.imported_items;
    inspection.duplicate_items = stats.duplicate_items;
    if let Err(error) = persist_takeout_source_evidence_plans(
        paths,
        config,
        key,
        &synthetic_profile,
        &source_evidence_plans,
    ) {
        inspection.notes.push(format!(
            "Canonical Takeout import completed, but the source-evidence archive needs a rebuild: {error}"
        ));
    }
    finalize_import_batch(&archive, batch_id, &inspection)?;
    finalize_successful_import_run(&archive, run_id, batch_id, &inspection, &stats)?;
    if let Err(error) = rebuild_search_projection(paths, config, key) {
        inspection.notes.push(format!(
            "Import completed, but the keyword-recall projection needs a rebuild: {error}"
        ));
    }

    ensure_import_batch_audit_artifact(paths, config, key, batch_id, Some("imported"))?;

    let detail = preview_import_batch(paths, config, key, batch_id)?;
    inspection.import_batch = Some(detail.batch);
    Ok(inspection)
}

/// Loads recent import batches, newest first.
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
    const LOAD_IMPORT_BATCHES_SQL: &str = "SELECT id, source_kind, source_path, profile_id, created_at, imported_at, reverted_at, status, summary_json, audit_path, git_commit, (SELECT COUNT(*) FROM visits WHERE import_batch_id = import_batches.id AND reverted_at IS NULL) AS visible_items FROM import_batches ORDER BY id DESC LIMIT 16";
    #[rustfmt::skip]
    let mut statement = connection.prepare(LOAD_IMPORT_BATCHES_SQL)?;
    let rows = statement.query_map([], |row: &Row<'_>| {
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

/// Loads one import batch detail, repairing its audit artifact if needed.
pub fn preview_import_batch(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let detail = load_import_batch_detail(&connection, batch_id)?;
    let audit_missing = detail
        .batch
        .audit_path
        .as_deref()
        .is_none_or(|path| path.trim().is_empty() || !Path::new(path).exists());
    if !audit_missing {
        return Ok(detail);
    }

    drop(connection);
    ensure_import_batch_audit_artifact(paths, config, key, batch_id, None)?;

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    load_import_batch_detail(&connection, batch_id)
}

/// Reverts one import batch by hiding its visits from the visible archive surface.
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

    let reverted_at = now_rfc3339();
    let removed: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM visits WHERE import_batch_id = ?1 AND reverted_at IS NULL",
        [batch_id],
        |row| row.get(0),
    )?;
    let rollback_run_id = create_import_revert_run(
        &transaction,
        existing.overview.profile_id.as_str(),
        batch_id,
        &reverted_at,
    )?;
    transaction.execute(
        "UPDATE visits
         SET reverted_at = ?1,
             reverted_by_run_id = ?2
         WHERE import_batch_id = ?3
           AND reverted_at IS NULL",
        params![reverted_at, rollback_run_id, batch_id],
    )?;
    let mut notes = existing.notes.clone();
    notes.push(format!(
        "Reverted at {}. Soft-hid {} live history rows from the archive view.",
        reverted_at, removed
    ));
    #[rustfmt::skip]
    update_batch_summary(&transaction, BatchSummaryUpdate { batch_id, status: "reverted", imported_items: existing.overview.imported_items, duplicate_items: existing.overview.duplicate_items, candidate_items: existing.overview.candidate_items, recognized_files: &existing.recognized_files, quarantined_files: &existing.quarantined_files, notes: &notes, reverted_at: Some(reverted_at) })?;
    refresh_completed_run_stats(
        &transaction,
        rollback_run_id,
        json!({
            "importBatchId": batch_id,
            "softHiddenVisits": removed.max(0),
        }),
    )?;
    transaction.commit()?;
    let rebuild_warning = rebuild_search_projection(paths, config, key).err().map(|error| {
        format!("Revert completed, but the keyword-recall projection needs a rebuild: {error}")
    });

    ensure_import_batch_audit_artifact(paths, config, key, batch_id, Some("reverted"))?;
    let mut detail = preview_import_batch(paths, config, key, batch_id)?;
    if let Some(warning) = rebuild_warning {
        detail.notes.push(warning);
    }
    Ok(detail)
}

/// Restores a previously reverted import batch to the visible archive surface.
pub fn restore_import_batch(
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
    if existing.overview.status != "reverted" {
        drop(transaction);
        return preview_import_batch(paths, config, key, batch_id);
    }

    let restored_at = now_rfc3339();
    let restored: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM visits WHERE import_batch_id = ?1 AND reverted_at IS NOT NULL",
        [batch_id],
        |row| row.get(0),
    )?;
    let restore_run_id = create_import_restore_run(
        &transaction,
        existing.overview.profile_id.as_str(),
        batch_id,
        &restored_at,
    )?;
    transaction.execute(
        "UPDATE visits
         SET reverted_at = NULL,
             reverted_by_run_id = NULL
         WHERE import_batch_id = ?1
           AND reverted_at IS NOT NULL",
        [batch_id],
    )?;
    let mut notes = existing.notes.clone();
    notes.push(format!(
        "Restored at {}. Returned {} hidden history rows to the visible archive view via restore run #{}.",
        restored_at, restored, restore_run_id
    ));
    #[rustfmt::skip]
    update_batch_summary(&transaction, BatchSummaryUpdate { batch_id, status: "imported", imported_items: existing.overview.imported_items, duplicate_items: existing.overview.duplicate_items, candidate_items: existing.overview.candidate_items, recognized_files: &existing.recognized_files, quarantined_files: &existing.quarantined_files, notes: &notes, reverted_at: None })?;
    transaction
        .execute("UPDATE import_batches SET reverted_at = NULL WHERE id = ?1", [batch_id])?;
    refresh_completed_run_stats(
        &transaction,
        restore_run_id,
        json!({
            "importBatchId": batch_id,
            "restoredVisits": restored.max(0),
            "action": "restore",
        }),
    )?;
    transaction.commit()?;
    let rebuild_warning = rebuild_search_projection(paths, config, key).err().map(|error| {
        format!("Restore completed, but the keyword-recall projection needs a rebuild: {error}")
    });

    ensure_import_batch_audit_artifact(paths, config, key, batch_id, Some("restored"))?;
    let mut detail = preview_import_batch(paths, config, key, batch_id)?;
    if let Some(warning) = rebuild_warning {
        detail.notes.push(warning);
    }
    Ok(detail)
}

fn import_supported_payload(
    archive: &Transaction<'_>,
    run_id: i64,
    batch_id: i64,
    source_profile_id: i64,
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<ImportedPayload> {
    let payload = collect_records_from_payload(source_path, kind, bytes)?;
    let mut stats =
        ImportStats { skipped_items: payload.skipped_missing_visit_time, ..ImportStats::default() };
    let mut url_id_map = std::collections::BTreeMap::new();

    for url in &payload.report.history.urls {
        let payload_hash = sha256_hex(
            serde_json::to_string(url)
                .context("serializing Takeout URL for payload hash")?
                .as_bytes(),
        );
        let url_id = archive.query_row(
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
             VALUES (?1, ?2, 1, 0, ?3, ?4, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(source_profile_id, source_url_id) DO UPDATE SET
               url = excluded.url,
               title = excluded.title,
               hidden = excluded.hidden,
               last_visit_ms = CASE
                 WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_ms
                 ELSE urls.last_visit_ms
               END,
               last_visit_iso = CASE
                 WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_iso
                 ELSE urls.last_visit_iso
               END,
               payload_hash = excluded.payload_hash,
               recorded_at = excluded.recorded_at
             RETURNING id",
            params![
                url.url,
                url.title,
                url.last_visit_ms,
                url.last_visit_iso,
                source_profile_id,
                run_id,
                url.source_url_id.to_string(),
                i64::from(url.hidden),
                payload_hash,
                now_rfc3339(),
            ],
            |row| row.get::<_, i64>(0),
        )?;
        url_id_map.insert(url.source_url_id, url_id);
    }

    for visit in &payload.report.history.visits {
        let Some(&url_id) = url_id_map.get(&visit.source_url_id) else {
            continue;
        };
        let payload_hash = sha256_hex(
            serde_json::to_string(visit)
                .context("serializing Takeout visit for payload hash")?
                .as_bytes(),
        );
        let inserted = archive.execute(
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
             VALUES (?1, ?2, ?3, ?4, NULL, NULL, ?5, ?6, NULL, 0, NULL, ?7, 'takeout', ?8, ?9, ?10, ?11)",
            params![
                url_id,
                visit.source_visit_id.to_string(),
                visit.visit_time_ms,
                visit.visit_time_iso,
                source_profile_id,
                run_id,
                source_path,
                visit_event_fingerprint(
                    "takeout",
                    &visit.url,
                    visit.visit_time_ms,
                    visit.title.as_deref(),
                    None,
                    Some("takeout"),
                ),
                payload_hash,
                now_rfc3339(),
                batch_id,
            ],
        )?;

        if inserted > 0 {
            stats.imported_items += 1;
        } else {
            stats.duplicate_items += 1;
        }
    }

    Ok(ImportedPayload {
        stats,
        source_evidence_plan: build_takeout_source_evidence_plan(
            source_profile_id,
            run_id,
            source_path,
            &payload.report,
        )?,
    })
}

fn build_takeout_source_evidence_plan(
    source_profile_id: i64,
    run_id: i64,
    source_path: &str,
    report: &TakeoutPayloadReport,
) -> Result<TakeoutSourceEvidencePlan> {
    let observation_json = serde_json::to_string(&report.history.schema_observation)?;
    Ok(TakeoutSourceEvidencePlan {
        source_batch: SourceBatchInput {
            source_profile_id,
            run_id: Some(run_id),
            source_kind: "takeout".to_string(),
            browser_version: None,
            schema_version_text: Some(report.kind.clone()),
            schema_version_int: None,
            schema_fingerprint: sha256_hex(observation_json.as_bytes()),
            capability_snapshot: report.history.capability_snapshot.clone(),
            coverage_stats_json: coverage_stats_json(&report.history),
            artifact_refs_json: Some(
                json!({
                    "sourcePath": source_path,
                    "payloadKind": report.kind,
                })
                .to_string(),
            ),
            notes_json: Some(serde_json::to_string(&report.history.warnings)?),
        },
        schema_observation: report.history.schema_observation.clone(),
        parsed_history: report.history.clone(),
    })
}

fn persist_takeout_source_evidence_plans(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    profile_id: &str,
    plans: &[TakeoutSourceEvidencePlan],
) -> Result<()> {
    if plans.is_empty() {
        return Ok(());
    }

    let archive = open_archive_connection(paths, config, key)?;
    let mut source_evidence = open_source_evidence_connection(paths, config, key)?;
    let transaction = source_evidence.transaction()?;
    let mut last_source_batch_id = None;

    for plan in plans {
        let source_batch_id = upsert_source_batch(&transaction, &plan.source_batch)?;
        record_schema_observation(
            &transaction,
            source_batch_id,
            "takeout-payload",
            &plan.schema_observation,
        )?;
        persist_source_evidence(
            &transaction,
            source_batch_id,
            plan.source_batch.source_profile_id,
            &plan.parsed_history,
        )?;
        last_source_batch_id = Some(source_batch_id);
    }

    transaction.commit()?;
    if let Some(source_batch_id) = last_source_batch_id {
        touch_takeout_source_batch_watermark(&archive, profile_id, source_batch_id)?;
    }
    Ok(())
}

fn touch_takeout_source_batch_watermark(
    archive: &Connection,
    profile_id: &str,
    source_batch_id: i64,
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
           last_source_batch_id,
           updated_at
         )
         VALUES (?1, 0, 0, 0, 0, NULL, NULL, ?2, ?3)
         ON CONFLICT(profile_id) DO UPDATE SET
           last_source_batch_id = excluded.last_source_batch_id,
           updated_at = excluded.updated_at",
        params![profile_id, source_batch_id, now_rfc3339()],
    )?;
    Ok(())
}

fn create_import_run(archive: &Connection, profile_id: &str, started_at: &str) -> Result<i64> {
    archive.execute(
        "INSERT INTO runs (
           run_type,
           trigger,
           started_at,
           timezone,
           status,
           profile_scope_json,
           warnings_json,
           stats_json,
           due_only
         )
         VALUES (
           'import',
           'manual',
           ?1,
           'UTC',
           'running',
           ?2,
           '[]',
           '{}',
           0
         )",
        params![started_at, serde_json::to_string(&vec![profile_id.to_string()])?],
    )?;
    Ok(archive.last_insert_rowid())
}

fn finalize_successful_import_run(
    archive: &Connection,
    run_id: i64,
    batch_id: i64,
    inspection: &TakeoutInspection,
    stats: &ImportStats,
) -> Result<()> {
    let stats_json = stats_with_archive_totals(
        archive,
        json!({
            "profilesProcessed": 1,
            "newVisits": stats.imported_items,
            "newUrls": 0,
            "newDownloads": 0,
            "candidateItems": inspection.candidate_items,
            "importedItems": stats.imported_items,
            "duplicateItems": stats.duplicate_items,
            "importBatchId": batch_id,
        }),
    )?;
    archive.execute(
        "UPDATE runs
         SET finished_at = ?1,
             status = 'success',
             stats_json = ?2,
             warnings_json = ?3,
             error_message = NULL
         WHERE id = ?4",
        params![
            now_rfc3339(),
            serde_json::to_string(&stats_json)?,
            serde_json::to_string(&inspection.notes)?,
            run_id,
        ],
    )?;
    Ok(())
}

fn finalize_failed_import_run(
    archive: &Connection,
    run_id: i64,
    notes: &[String],
    stats: &ImportStats,
    error: &anyhow::Error,
) -> Result<()> {
    archive.execute(
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
                "profilesProcessed": 1,
                "newVisits": stats.imported_items,
                "newUrls": 0,
                "newDownloads": 0,
                "duplicateItems": stats.duplicate_items,
            }))?,
            serde_json::to_string(notes)?,
            format!("{error:#}"),
            run_id,
        ],
    )?;
    Ok(())
}

fn create_import_revert_run(
    archive: &Transaction<'_>,
    profile_id: &str,
    batch_id: i64,
    started_at: &str,
) -> Result<i64> {
    archive.execute(
        "INSERT INTO runs (
           run_type,
           trigger,
           started_at,
           finished_at,
           timezone,
           status,
           profile_scope_json,
           stats_json,
           warnings_json,
           error_message,
           due_only
         )
         VALUES (
           'rollback',
           'manual',
           ?1,
           ?1,
           'UTC',
           'success',
           ?2,
           ?3,
           '[]',
           NULL,
           0
         )",
        params![
            started_at,
            serde_json::to_string(&vec![profile_id])?,
            serde_json::to_string(&json!({ "importBatchId": batch_id }))?,
        ],
    )?;
    Ok(archive.last_insert_rowid())
}

fn create_import_restore_run(
    archive: &Transaction<'_>,
    profile_id: &str,
    batch_id: i64,
    started_at: &str,
) -> Result<i64> {
    archive.execute(
        "INSERT INTO runs (
           run_type,
           trigger,
           started_at,
           finished_at,
           timezone,
           status,
           profile_scope_json,
           stats_json,
           warnings_json,
           error_message,
           due_only
         )
         VALUES (
           'restore',
           'manual',
           ?1,
           ?1,
           'UTC',
           'success',
           ?2,
           ?3,
           '[]',
           NULL,
           0
         )",
        params![
            started_at,
            serde_json::to_string(&vec![profile_id])?,
            serde_json::to_string(&json!({ "importBatchId": batch_id, "action": "restore" }))?,
        ],
    )?;
    Ok(archive.last_insert_rowid())
}

fn refresh_completed_run_stats(archive: &Connection, run_id: i64, stats: Value) -> Result<()> {
    let stats_json = stats_with_archive_totals(archive, stats)?;
    archive.execute(
        "UPDATE runs
         SET stats_json = ?1
         WHERE id = ?2",
        params![serde_json::to_string(&stats_json)?, run_id],
    )?;
    Ok(())
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
    archive: &Connection,
    batch_id: i64,
    inspection: &TakeoutInspection,
) -> Result<()> {
    #[rustfmt::skip]
    update_batch_summary(archive, BatchSummaryUpdate { batch_id, status: "imported", imported_items: inspection.imported_items, duplicate_items: inspection.duplicate_items, candidate_items: inspection.candidate_items, recognized_files: &inspection.recognized_files, quarantined_files: &inspection.quarantined_files, notes: &inspection.notes, reverted_at: None })?;
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
    #[rustfmt::skip]
    archive.execute("UPDATE import_batches SET status = ?1, summary_json = ?2, reverted_at = COALESCE(?3, reverted_at) WHERE id = ?4", params![update.status, summary_json, update.reverted_at, update.batch_id])?;
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
) -> Result<i64> {
    archive.execute(
        "INSERT INTO source_profiles (
           browser_kind,
           browser_family,
           browser_product,
           browser_version,
           profile_name,
           profile_path,
           discovered_at,
           enabled,
           profile_key,
           user_name,
           updated_at
         )
         VALUES ('takeout', 'takeout', 'takeout', 'takeout', ?1, ?2, ?3, 1, ?4, NULL, ?3)
         ON CONFLICT(profile_key) DO UPDATE SET
           profile_name = excluded.profile_name,
           profile_path = excluded.profile_path,
           browser_family = excluded.browser_family,
           browser_product = excluded.browser_product,
           browser_version = excluded.browser_version,
           updated_at = excluded.updated_at,
           enabled = 1",
        params![
            "Imported browser history".to_string(),
            source.display().to_string(),
            now_rfc3339(),
            profile_id,
        ],
    )?;
    archive
        .query_row("SELECT id FROM source_profiles WHERE profile_key = ?1", [profile_id], |row| {
            row.get(0)
        })
        .map_err(Into::into)
}

/// Ensures an import batch has an audit artifact that matches its current state.
pub(crate) fn ensure_import_batch_audit_artifact(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    batch_id: i64,
    action_hint: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    let detail = load_import_batch_detail(&connection, batch_id)?;
    if detail
        .batch
        .audit_path
        .as_deref()
        .is_some_and(|path| !path.trim().is_empty() && Path::new(path).exists())
    {
        return Ok((detail.batch.audit_path.clone(), detail.batch.git_commit.clone()));
    }

    let action = action_hint.unwrap_or(match detail.batch.status.as_str() {
        "reverted" => "reverted",
        _ => "imported",
    });
    let (audit_path, git_commit) =
        write_batch_audit_detail(paths, config, batch_id, &detail, action)?;
    update_batch_audit(&connection, batch_id, Some(audit_path.as_str()), git_commit.as_deref())?;
    Ok((Some(audit_path), git_commit))
}

fn write_batch_audit_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    batch_id: i64,
    detail: &ImportBatchDetail,
    action: &str,
) -> Result<(String, Option<String>)> {
    git_audit::ensure_repo(&paths.audit_repo_path)?;
    let file_name =
        format!("imports/{}/batch-{}-{}.json", &detail.batch.created_at[0..10], batch_id, action);
    let contents = serde_json::to_string_pretty(detail)?;
    let audit_path = git_audit::write_audit_file(&paths.audit_repo_path, &file_name, &contents)?;
    let git_commit = if config.git_enabled {
        git_audit::commit_all(&paths.audit_repo_path, &format!("import batch {batch_id} {action}"))?
    } else {
        None
    };
    Ok((audit_path.display().to_string(), git_commit))
}

fn update_batch_audit(
    connection: &Connection,
    batch_id: i64,
    audit_path: Option<&str>,
    git_commit: Option<&str>,
) -> Result<()> {
    #[rustfmt::skip]
    connection.execute("UPDATE import_batches SET audit_path = ?1, git_commit = ?2 WHERE id = ?3", params![audit_path, git_commit, batch_id])?;
    Ok(())
}

fn load_import_batch_detail(connection: &Connection, batch_id: i64) -> Result<ImportBatchDetail> {
    let batch = load_import_batch_record(connection, batch_id)?
        .with_context(|| format!("import batch {batch_id} was not found"))?;

    const PREVIEW_IMPORT_BATCH_SQL: &str = r#"
        SELECT urls.url, urls.title, visits.visit_time_iso, visits.source_visit_id
        FROM visits
        JOIN urls ON urls.id = visits.url_id
        WHERE visits.import_batch_id = ?1
          AND visits.reverted_at IS NULL
        ORDER BY visits.visit_time_ms DESC, visits.id DESC
        LIMIT ?2
    "#;
    let mut statement = connection.prepare(PREVIEW_IMPORT_BATCH_SQL)?;
    let rows = statement.query_map(params![batch_id, PREVIEW_LIMIT as i64], |row: &Row<'_>| {
        Ok(TakeoutPreviewEntry {
            source_path: batch.overview.source_path.clone(),
            url: row.get(0)?,
            title: row.get(1)?,
            visited_at: row.get(2)?,
            source_visit_id: row
                .get::<_, Option<String>>(3)?
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or_default(),
            status: "imported".to_string(),
        })
    })?;
    let preview_entries = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(ImportBatchDetail {
        batch: batch.overview,
        preview_entries,
        recognized_files: batch.recognized_files,
        quarantined_files: batch.quarantined_files,
        notes: batch.notes,
    })
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
                (SELECT COUNT(*) FROM visits WHERE import_batch_id = import_batches.id AND reverted_at IS NULL) AS visible_items
             FROM import_batches
             WHERE id = ?1",
            [batch_id],
            |row: &Row<'_>| {
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

#[cfg(test)]
fn preview_entry_from_payload(
    source_path: &str,
    kind: &str,
    payload_json: &[u8],
) -> Result<TakeoutPreviewEntry> {
    let payload = collect_records_from_payload(source_path, kind, payload_json)?;
    let Some(record) = payload.records.first() else {
        anyhow::bail!("payload did not include a usable history record")
    };
    Ok(preview_entry(record, "imported"))
}

fn preview_entry(record: &ParsedTakeoutRecord, status: &str) -> TakeoutPreviewEntry {
    TakeoutPreviewEntry {
        source_path: record.source_path.clone(),
        url: record.url.clone(),
        title: record.title.clone(),
        visited_at: record.visited_at.clone(),
        source_visit_id: record.source_visit_id,
        status: status.to_string(),
    }
}

fn collect_records_from_payload(
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<CollectedPayload> {
    let report = parse_takeout_payload(source_path, kind, bytes)
        .with_context(|| format!("parsing Takeout payload {source_path} ({kind})"))?;
    let records = report
        .history
        .visits
        .iter()
        .map(|visit| ParsedTakeoutRecord {
            source_path: source_path.to_string(),
            url: visit.url.clone(),
            title: visit.title.clone(),
            visited_at: visit.visit_time_iso.clone(),
            source_visit_id: visit.source_visit_id,
        })
        .collect();
    Ok(CollectedPayload {
        skipped_missing_visit_time: report.skipped_missing_visit_time,
        records,
        report,
    })
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
    recognize_takeout_payload(path).map(ToString::to_string)
}

fn read_zip_entry(source_zip: &Path, entry_name: &str) -> Result<Vec<u8>> {
    let file = fs::File::open(source_zip)?;
    let mut archive = ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_name)?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn quarantine_takeout_file(
    paths: &ProjectPaths,
    source_root: &Path,
    file: &TakeoutFile,
) -> Result<()> {
    if file.from_zip {
        let bytes = read_zip_entry(source_root, &file.path)?;
        quarantine_bytes(paths, source_root, &file.path, &bytes)?;
    } else {
        quarantine_file(paths, source_root, &file.path)?;
    }
    Ok(())
}

fn quarantine_file(paths: &ProjectPaths, source_root: &Path, path: &str) -> Result<()> {
    let destination = quarantine_destination(paths, source_root, path);
    ensure_parent_dir(&destination)?;
    copy_if_exists(path, &destination)?;
    Ok(())
}

fn quarantine_bytes(
    paths: &ProjectPaths,
    source_root: &Path,
    path: &str,
    bytes: &[u8],
) -> Result<()> {
    let destination = quarantine_destination(paths, source_root, path);
    ensure_parent_dir(&destination)?;
    fs::write(&destination, bytes).with_context(|| format!("writing {}", destination.display()))?;
    Ok(())
}

fn quarantine_destination(paths: &ProjectPaths, source_root: &Path, path: &str) -> PathBuf {
    paths
        .quarantine_dir
        .join(quarantine_source_name(source_root))
        .join(quarantine_relative_path(source_root, path))
}

fn quarantine_source_name(source_root: &Path) -> &str {
    source_root.file_stem().and_then(|name| name.to_str()).unwrap_or("takeout")
}

fn quarantine_relative_path(source_root: &Path, path: &str) -> PathBuf {
    let candidate = Path::new(path)
        .strip_prefix(source_root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| PathBuf::from(path));
    let sanitized = sanitized_relative_path(&candidate);
    if sanitized.as_os_str().is_empty() { PathBuf::from("unknown") } else { sanitized }
}

fn sanitized_relative_path(path: &Path) -> PathBuf {
    let mut sanitized = PathBuf::new();
    for component in path.components() {
        if let std::path::Component::Normal(part) = component {
            sanitized.push(part);
        }
    }
    sanitized
}

#[rustfmt::skip]
fn ensure_parent_dir(path: &Path) -> Result<()> { if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; } Ok(()) }

#[rustfmt::skip]
fn copy_if_exists(source: &str, destination: &Path) -> Result<()> { if Path::new(source).exists() { fs::copy(source, destination)?; } Ok(()) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        archive::{
            create_schema, load_audit_run_detail, load_recent_runs, open_source_evidence_connection,
        },
        config::{ensure_paths, project_paths_with_root},
        models::{AppConfig, ArchiveMode},
    };
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    fn sample_paths(root: &Path) -> ProjectPaths {
        project_paths_with_root(root)
    }

    fn initialized_plaintext_config() -> AppConfig {
        AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            ..AppConfig::default()
        }
    }

    fn write_takeout_fixture_with_name(dir: &Path, name: &str, lines: &[&str]) -> PathBuf {
        let source_dir = dir.join(name);
        fs::create_dir_all(&source_dir).expect("create takeout source dir");
        let source = source_dir.join("takeout.jsonl");
        fs::write(&source, lines.join("\n")).expect("write takeout fixture");
        source_dir
    }

    fn write_takeout_fixture(dir: &Path) -> PathBuf {
        write_takeout_fixture_with_name(
            dir,
            "takeout-source",
            &[
                r#"{"url":"https://example.com/one","title":"One","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
                r#"{"url":"https://example.com/two","title":"Two","visitedAt":"2026-04-01T11:00:00+00:00"}"#,
            ],
        )
    }

    fn write_takeout_zip(dir: &Path, entries: &[(&str, &str)]) -> PathBuf {
        let zip_path = dir.join("takeout.zip");
        let file = fs::File::create(&zip_path).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, contents) in entries {
            zip.start_file(name, options).expect("start zip entry");
            zip.write_all(contents.as_bytes()).expect("write zip entry");
        }
        zip.finish().expect("finish zip");
        zip_path
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

        assert_eq!(inspection.source_path, source.display().to_string());
        assert!(inspection.dry_run);
        assert_eq!(inspection.candidate_items, 2);
        assert_eq!(inspection.preview_entries.len(), 2);
        assert_eq!(inspection.recognized_files.len(), 1);
        assert_eq!(inspection.recognized_files[0].status, "previewed");
        assert_eq!(inspection.recognized_files[0].records, 2);
        assert!(inspection.quarantined_files.is_empty());
        assert!(inspection.notes.is_empty());
    }

    #[test]
    fn inspect_takeout_caps_preview_entries_at_preview_limit() {
        let dir = tempdir().expect("tempdir");
        let lines = (0..=PREVIEW_LIMIT)
            .map(|index| {
                format!(
                    "{{\"url\":\"https://example.com/{index}\",\"title\":\"Item {index}\",\"visitedAt\":\"2026-04-01T10:{index:02}:00+00:00\"}}"
                )
            })
            .collect::<Vec<_>>();
        let line_refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
        let source = write_takeout_fixture_with_name(dir.path(), "takeout-many", &line_refs);

        let inspection = inspect_takeout(
            &sample_paths(dir.path()),
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
        )
        .expect("inspect");

        assert_eq!(inspection.candidate_items, PREVIEW_LIMIT + 1);
        assert_eq!(inspection.preview_entries.len(), PREVIEW_LIMIT);
        assert_eq!(inspection.preview_entries[0].url, "https://example.com/0");
        assert_eq!(
            inspection.preview_entries.last().expect("last preview entry").url,
            format!("https://example.com/{}", PREVIEW_LIMIT - 1)
        );
        assert!(
            inspection
                .preview_entries
                .iter()
                .all(|entry| entry.url != format!("https://example.com/{PREVIEW_LIMIT}"))
        );
    }

    #[test]
    fn load_import_batches_returns_empty_for_uninitialized_archives() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = AppConfig::default();
        assert!(
            load_import_batches(&paths, &config, None).expect("empty import batches").is_empty()
        );

        assert!(!paths.archive_database_path.exists());
        let initialized_but_missing_archive =
            load_import_batches(&paths, &initialized_plaintext_config(), None)
                .expect("missing archive batches");
        assert!(initialized_but_missing_archive.is_empty());
        assert!(!paths.archive_database_path.exists());
    }

    #[test]
    fn import_preview_revert_and_restore_batch_are_reversible() {
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
        assert_eq!(batch.candidate_items, 2);
        assert_eq!(batch.imported_items, 2);
        assert_eq!(batch.duplicate_items, 0);
        assert_eq!(inspection.imported_items, 2);
        assert!(inspection.notes.is_empty());
        assert_eq!(batch.visible_items, 2);
        let recent_runs =
            load_recent_runs(&paths, &config, None).expect("recent runs after import");
        assert_eq!(recent_runs[0].run_type, "import");
        assert_eq!(recent_runs[0].profiles_processed, 1);
        assert_eq!(recent_runs[0].new_visits, 2);
        assert_eq!(recent_runs[0].profile_scope, vec!["takeout::browser-history".to_string()]);
        let import_detail = load_audit_run_detail(&paths, &config, None, recent_runs[0].id)
            .expect("import run detail");
        assert_eq!(import_detail.run.run_type, "import");
        assert_eq!(import_detail.profile_scope, vec!["takeout::browser-history".to_string()]);

        let preview = preview_import_batch(&paths, &config, None, batch.id).expect("preview batch");
        assert_eq!(preview.preview_entries.len(), 2);
        assert_eq!(preview.batch.status, "imported");
        assert_eq!(preview.batch.candidate_items, 2);
        assert_eq!(preview.batch.imported_items, 2);
        assert_eq!(preview.batch.duplicate_items, 0);

        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        let (profile_name, profile_path, chrome_version): (String, String, String) = archive
            .query_row(
                "SELECT profile_name, profile_path, browser_version
                 FROM source_profiles
                 WHERE profile_key = 'takeout::browser-history'",
                [],
                |row: &Row<'_>| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load takeout profile");
        assert_eq!(profile_name, "Imported browser history");
        assert_eq!(profile_path, source.display().to_string());
        assert_eq!(chrome_version, "takeout");

        let reverted = revert_import_batch(&paths, &config, None, batch.id).expect("revert batch");
        assert_eq!(reverted.batch.status, "reverted");
        assert_eq!(reverted.batch.visible_items, 0);
        assert!(reverted.notes.iter().any(|note| note.contains("Soft-hid 2 live history rows")));
        let hidden_rows: i64 = archive
            .query_row(
                "SELECT COUNT(*) FROM visits WHERE import_batch_id = ?1 AND reverted_at IS NOT NULL",
                [batch.id],
                |row| row.get(0),
            )
            .expect("load hidden visit count");
        assert_eq!(hidden_rows, 2);
        let recent_runs =
            load_recent_runs(&paths, &config, None).expect("recent runs after revert");
        assert_eq!(recent_runs[0].run_type, "rollback");
        assert_eq!(recent_runs[0].new_visits, 2);

        let restored =
            restore_import_batch(&paths, &config, None, batch.id).expect("restore batch");
        assert_eq!(restored.batch.status, "imported");
        assert_eq!(restored.batch.visible_items, 2);
        assert!(restored.notes.iter().any(|note| note.contains("Restored at")));
        let visible_rows: i64 = archive
            .query_row(
                "SELECT COUNT(*) FROM visits WHERE import_batch_id = ?1 AND reverted_at IS NULL",
                [batch.id],
                |row| row.get(0),
            )
            .expect("load restored visit count");
        assert_eq!(visible_rows, 2);
        let recent_runs =
            load_recent_runs(&paths, &config, None).expect("recent runs after restore");
        assert_eq!(recent_runs[0].run_type, "restore");
        assert_eq!(recent_runs[0].new_visits, 2);
    }

    #[test]
    fn preview_import_batch_repairs_missing_audit_artifacts() {
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
        let audit_path = batch.audit_path.clone().expect("audit path");
        fs::remove_file(&audit_path).expect("remove audit artifact");

        let repaired =
            preview_import_batch(&paths, &config, None, batch.id).expect("preview batch");
        let repaired_path = repaired.batch.audit_path.expect("repaired audit path");

        assert!(Path::new(&repaired_path).exists());
        assert!(repaired_path.ends_with(".json"));
    }

    #[test]
    fn import_takeout_deduplicates_matching_history_from_different_files() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = initialized_plaintext_config();
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&archive).expect("schema");
        drop(archive);

        let source_a = write_takeout_fixture_with_name(
            dir.path(),
            "takeout-a",
            &[
                r#"{"url":"https://example.com/one","title":"One","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
            ],
        );
        let source_b = write_takeout_fixture_with_name(
            dir.path(),
            "takeout-b",
            &[
                r#"{"url":"https://example.com/one","title":"One","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
            ],
        );

        let first = import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: source_a.display().to_string(), dry_run: false },
        )
        .expect("first import");
        let second = import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: source_b.display().to_string(), dry_run: false },
        )
        .expect("second import");

        assert_eq!(first.imported_items, 1);
        assert_eq!(first.duplicate_items, 0);
        assert_eq!(second.imported_items, 0);
        assert_eq!(second.duplicate_items, 1);

        let history = crate::archive::list_history(&paths, &config, None, Default::default())
            .expect("history");
        assert_eq!(history.total, 1);

        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        let visits: i64 = archive
            .query_row(
                "SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",
                [],
                |row: &Row<'_>| row.get(0),
            )
            .expect("visit count");
        assert_eq!(visits, 1);
    }

    #[test]
    fn takeout_records_without_timestamps_are_skipped_with_a_note() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = initialized_plaintext_config();
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&archive).expect("schema");
        drop(archive);

        let source = write_takeout_fixture_with_name(
            dir.path(),
            "takeout-missing-time",
            &[
                r#"{"url":"https://example.com/valid","title":"Valid","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
                r#"{"url":"https://example.com/missing","title":"Missing"}"#,
            ],
        );

        let dry_run = inspect_takeout(
            &paths,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
        )
        .expect("inspect takeout");
        assert_eq!(dry_run.source_path, source.display().to_string());
        assert!(dry_run.dry_run);
        assert_eq!(dry_run.candidate_items, 1);
        assert_eq!(dry_run.recognized_files.len(), 1);
        assert_eq!(dry_run.recognized_files[0].status, "previewed-with-skips");
        assert_eq!(dry_run.recognized_files[0].records, 1);
        assert_eq!(dry_run.notes.len(), 1);
        assert!(dry_run.notes.iter().any(|note| note.contains("missing a visit timestamp")));

        let imported = import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect("import takeout");
        assert_eq!(imported.imported_items, 1);
        assert_eq!(imported.duplicate_items, 0);
        assert_eq!(
            imported.notes.iter().filter(|note| note.contains("missing a visit timestamp")).count(),
            2
        );
        assert_eq!(imported.recognized_files.len(), 1);
        assert_eq!(imported.recognized_files[0].records, 1);

        let history = crate::archive::list_history(&paths, &config, None, Default::default())
            .expect("history");
        assert_eq!(history.total, 1);
    }

    #[test]
    fn inspect_takeout_reports_parse_errors_for_recognized_files() {
        let dir = tempdir().expect("tempdir");
        let source = dir.path().join("malformed");
        fs::create_dir_all(&source).expect("create malformed source");
        fs::write(source.join("BrowserHistory.json"), "{not-json")
            .expect("write malformed history");

        let inspection = inspect_takeout(
            &sample_paths(dir.path()),
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
        )
        .expect("inspect malformed takeout");

        assert_eq!(inspection.recognized_files.len(), 1);
        assert_eq!(inspection.recognized_files[0].status, "parse-error");
        assert!(inspection.notes.iter().any(|note| note.contains("Could not parse")));
    }

    #[test]
    fn recognize_and_parse_takeout_payloads() {
        assert_eq!(recognize_takeout_file("BrowserHistory.json"), Some("browser-json".to_string()));
        assert_eq!(recognize_takeout_file("browser-export.json"), Some("browser-json".to_string()));
        assert_eq!(recognize_takeout_file("watch-history.json"), Some("browser-json".to_string()));
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
        assert_eq!(records.records.len(), 1);
        assert_eq!(records.records[0].title.as_deref(), Some("Example"));
    }

    #[test]
    fn takeout_helpers_cover_unknown_files_zip_sources_and_quarantine() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");

        let unknown_source = dir.path().join("unknown-only");
        fs::create_dir_all(&unknown_source).expect("create unknown source");
        let unknown_file = unknown_source.join("notes.txt");
        fs::write(&unknown_file, "notes").expect("write unknown file");

        let unknown_inspection = inspect_takeout(
            &paths,
            &TakeoutRequest { source_path: unknown_source.display().to_string(), dry_run: true },
        )
        .expect("inspect unknown");
        assert!(unknown_inspection.recognized_files.is_empty());
        assert_eq!(unknown_inspection.quarantined_files.len(), 1);
        assert!(
            unknown_inspection.notes.iter().any(|note| note.contains("No directly importable"))
        );

        quarantine_file(&paths, &unknown_source, &unknown_file.display().to_string())
            .expect("quarantine file");
        assert!(
            paths.quarantine_dir.join("unknown-only").join("notes.txt").exists(),
            "quarantined file should be copied"
        );

        let zip_source = write_takeout_zip(
            dir.path(),
            &[
                (
                    "BrowserHistory.json",
                    r#"{"BrowserHistory":[{"titleUrl":"https://example.com/zip","pageTitle":"Zip","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#,
                ),
                ("archive_browser.html", "<html></html>"),
                ("nested/notes.txt", "ignore me"),
            ],
        );
        let files = gather_takeout_files(&zip_source).expect("gather zip files");
        assert_eq!(files.len(), 3);
        let zip_bytes = read_zip_entry(&zip_source, "BrowserHistory.json").expect("read zip entry");
        assert!(String::from_utf8(zip_bytes).expect("zip utf8").contains("example.com/zip"));

        let zip_inspection = inspect_takeout(
            &paths,
            &TakeoutRequest { source_path: zip_source.display().to_string(), dry_run: true },
        )
        .expect("inspect zip");
        assert_eq!(zip_inspection.candidate_items, 1);
        assert_eq!(zip_inspection.recognized_files.len(), 2);
        assert_eq!(zip_inspection.quarantined_files.len(), 1);

        quarantine_takeout_file(
            &paths,
            &zip_source,
            &TakeoutFile { path: "nested/notes.txt".to_string(), from_zip: true },
        )
        .expect("quarantine zip entry");
        assert_eq!(
            fs::read_to_string(
                paths.quarantine_dir.join("takeout").join("nested").join("notes.txt")
            )
            .expect("read quarantined zip entry"),
            "ignore me"
        );
    }

    #[test]
    fn takeout_import_guards_and_idempotent_revert_cover_batch_edges() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let archive = open_archive_connection(&paths, &initialized_plaintext_config(), None)
            .expect("open archive");
        create_schema(&archive).expect("schema");
        drop(archive);

        assert!(
            load_import_batches(&paths, &initialized_plaintext_config(), None)
                .expect("load batches")
                .is_empty()
        );

        let source = write_takeout_fixture(dir.path());
        let dry_run = import_takeout(
            &paths,
            &initialized_plaintext_config(),
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: true },
        )
        .expect("dry run import");
        assert!(dry_run.import_batch.is_none());
        assert_eq!(dry_run.imported_items, 0);

        let uninitialized_error = import_takeout(
            &paths,
            &AppConfig::default(),
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect_err("uninitialized import should fail");
        assert!(uninitialized_error.to_string().contains("archive must be initialized"));

        let mut git_config = initialized_plaintext_config();
        git_config.git_enabled = true;
        let imported = import_takeout(
            &paths,
            &git_config,
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect("import with git");
        let batch_id = imported.import_batch.expect("batch").id;
        let batches =
            load_import_batches(&paths, &git_config, None).expect("load populated batches");
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].id, batch_id);
        assert_eq!(batches[0].candidate_items, 2);
        assert_eq!(batches[0].imported_items, 2);
        assert_eq!(batches[0].duplicate_items, 0);
        let imported_audit_path = batches[0].audit_path.as_deref().expect("import audit path");
        assert!(!imported_audit_path.is_empty());
        assert!(Path::new(imported_audit_path).exists());
        let imported_git_commit = batches[0].git_commit.as_deref().expect("import git commit");
        assert_eq!(imported_git_commit.len(), 40);
        let reverted = revert_import_batch(&paths, &git_config, None, batch_id).expect("revert");
        let reverted_again =
            revert_import_batch(&paths, &git_config, None, batch_id).expect("revert again");
        assert_eq!(reverted.batch.status, "reverted");
        assert_eq!(reverted_again.batch.status, "reverted");
        let reverted_audit_path =
            reverted_again.batch.audit_path.as_deref().expect("revert audit path");
        assert!(!reverted_audit_path.is_empty());
        assert!(Path::new(reverted_audit_path).exists());
        let reverted_git_commit =
            reverted_again.batch.git_commit.as_deref().expect("revert git commit");
        assert_eq!(reverted_git_commit.len(), 40);

        let restored =
            restore_import_batch(&paths, &git_config, None, batch_id).expect("restore batch");
        let restored_again =
            restore_import_batch(&paths, &git_config, None, batch_id).expect("restore again");
        assert_eq!(restored.batch.status, "imported");
        assert_eq!(restored_again.batch.status, "imported");
        let restored_audit_path =
            restored_again.batch.audit_path.as_deref().expect("restore audit path");
        assert!(!restored_audit_path.is_empty());
        assert!(Path::new(restored_audit_path).exists());
        let restored_git_commit =
            restored_again.batch.git_commit.as_deref().expect("restore git commit");
        assert_eq!(restored_git_commit.len(), 40);

        let connection = open_archive_connection(&paths, &git_config, None).expect("open archive");
        assert!(
            load_import_batch_record(&connection, batch_id).expect("load batch record").is_some()
        );
        assert!(load_import_batch_record(&connection, 9_999).expect("missing batch").is_none());
    }

    #[test]
    fn takeout_parsers_cover_blank_lines_arrays_and_unusable_payloads() {
        let payload = collect_records_from_payload(
            "fixture.jsonl",
            "jsonl",
            br#"
{"url":"https://example.com/ok","title":"OK","visitedAt":"2026-04-01T10:00:00+00:00"}
{"url":"https://example.com/missing","title":"Missing"}
{"title":"Ignored"}
"#,
        )
        .expect("collect jsonl");
        assert_eq!(payload.records.len(), 1);
        assert_eq!(payload.skipped_missing_visit_time, 1);

        let array_payload = collect_records_from_payload(
            "array.json",
            "browser-json",
            br#"[{"titleUrl":"https://example.com/array","pageTitle":"Array","visitedAt":"2026-04-01T10:00:00+00:00"}]"#,
        )
        .expect("collect array");
        assert_eq!(array_payload.records.len(), 1);

        let empty_payload =
            collect_records_from_payload("empty.json", "browser-json", br#"{"notHistory":[]}"#)
                .expect("collect empty");
        assert!(empty_payload.records.is_empty());

        let unusable_error =
            preview_entry_from_payload("source", "browser-json", br#"{"title":"ignored"}"#)
                .expect_err("unusable");
        assert!(unusable_error.to_string().contains("usable history record"));
    }

    #[test]
    fn import_takeout_quarantines_unknown_files_and_skips_index_entries() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = initialized_plaintext_config();
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&archive).expect("schema");
        drop(archive);

        let source = dir.path().join("mixed-takeout");
        fs::create_dir_all(&source).expect("create mixed source");
        fs::write(
            source.join("entries.jsonl"),
            r#"{"url":"https://example.com/import","title":"Import","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
        )
        .expect("write importable payload");
        fs::write(source.join("archive_browser.html"), "<html></html>").expect("write index");
        fs::write(source.join("notes.txt"), "quarantine me").expect("write unknown file");

        let inspection = import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect("import mixed takeout");

        assert_eq!(inspection.imported_items, 1);
        assert!(inspection.notes.is_empty());
        assert!(paths.quarantine_dir.join("mixed-takeout").join("notes.txt").exists());
        assert!(
            inspection
                .recognized_files
                .iter()
                .any(|file| file.path.ends_with("archive_browser.html")
                    && file.status == "recognized")
        );
    }

    #[test]
    fn import_takeout_persists_source_evidence_for_all_recognized_payloads() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = initialized_plaintext_config();
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&archive).expect("schema");
        drop(archive);

        let source = dir.path().join("rich-takeout");
        fs::create_dir_all(&source).expect("create rich source");
        fs::write(
            source.join("BrowserHistory.json"),
            r#"{"BrowserHistory":[{"titleUrl":"https://example.com/imported","pageTitle":"Imported","visitedAt":"2026-04-01T10:00:00+00:00","client_id":"client-1"}]}"#,
        )
        .expect("write browser history");
        fs::write(
            source.join("TypedUrl.json"),
            r#"{"TypedUrl":[{"url":"https://example.com/imported","title":"Imported","visits":[1711965600000000]}]}"#,
        )
        .expect("write typed urls");
        fs::write(
            source.join("Session.json"),
            r#"{"Session":[{"sessionTag":"device-1","tab":[{"tabId":1,"navigation":[{"virtualUrl":"https://example.com/imported"}]}]}]}"#,
        )
        .expect("write session");

        let inspection = import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect("import rich takeout");
        assert_eq!(inspection.imported_items, 1);
        assert_eq!(inspection.recognized_files.len(), 3);

        let source_evidence =
            open_source_evidence_connection(&paths, &config, None).expect("open source evidence");
        let source_batch_count: i64 = source_evidence
            .query_row("SELECT COUNT(*) FROM source_batches", [], |row| row.get(0))
            .expect("count source batches");
        assert_eq!(source_batch_count, 3);

        let native_entity_kinds = {
            let mut statement = source_evidence
                .prepare("SELECT entity_kind FROM native_entities ORDER BY entity_kind")
                .expect("prepare native entity query");
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .expect("query native kinds")
                .collect::<rusqlite::Result<Vec<_>>>()
                .expect("collect native kinds")
        };
        assert!(native_entity_kinds.contains(&"takeout-browser-history".to_string()));
        assert!(native_entity_kinds.contains(&"takeout-typed-url".to_string()));
        assert!(native_entity_kinds.contains(&"takeout-session".to_string()));

        let context_rows: i64 = source_evidence
            .query_row(
                "SELECT COUNT(*) FROM visit_context_evidence WHERE context_key = 'context.takeout.client_id'",
                [],
                |row| row.get(0),
            )
            .expect("count takeout context rows");
        assert_eq!(context_rows, 1);

        let last_source_batch_id: Option<i64> = open_archive_connection(&paths, &config, None)
            .expect("reopen archive")
            .query_row(
                "SELECT last_source_batch_id FROM profile_watermarks WHERE profile_id = 'takeout::browser-history'",
                [],
                |row| row.get(0),
            )
            .expect("load watermark");
        assert!(last_source_batch_id.is_some());
    }

    #[test]
    fn import_takeout_quarantines_unknown_zip_entries() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = initialized_plaintext_config();
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&archive).expect("schema");
        drop(archive);

        let source = write_takeout_zip(
            dir.path(),
            &[
                (
                    "BrowserHistory.json",
                    r#"{"BrowserHistory":[{"titleUrl":"https://example.com/imported","pageTitle":"Imported","visitedAt":"2026-04-01T10:00:00+00:00"}]}"#,
                ),
                ("nested/notes.txt", "quarantine me"),
            ],
        );

        let inspection = import_takeout(
            &paths,
            &config,
            None,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect("import zipped takeout");

        assert_eq!(inspection.imported_items, 1);
        assert_eq!(
            fs::read_to_string(
                paths.quarantine_dir.join("takeout").join("nested").join("notes.txt")
            )
            .expect("read quarantined zip entry"),
            "quarantine me"
        );
    }

    #[test]
    fn browser_json_payloads_ignore_missing_and_valid_records() {
        let payload = collect_records_from_payload(
            "BrowserHistory.json",
            "browser-json",
            br#"{"BrowserHistory":[
                {"title":"Ignored"},
                {"titleUrl":"https://example.com/missing","pageTitle":"Missing"},
                {"titleUrl":"https://example.com/ok","pageTitle":"OK","visitedAt":"2026-04-01T10:00:00+00:00"}
            ]}"#,
        )
        .expect("collect browser history payload");

        assert_eq!(payload.records.len(), 1);
        assert_eq!(payload.records[0].title.as_deref(), Some("OK"));
        assert_eq!(payload.skipped_missing_visit_time, 1);
    }
}
