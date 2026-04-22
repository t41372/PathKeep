//! Takeout import execution helpers.
//!
//! ## Responsibilities
//! - Run the archive import transaction for recognized Takeout payloads.
//! - Emit honest progress updates while the import is in flight.
//! - Persist source-evidence plans after canonical archive writes commit.
//!
//! ## Not responsible for
//! - Loading persisted batch review read models.
//! - Reverting or restoring prior import batches.
//! - Quarantining unsupported files beyond delegating to inspection helpers.
//!
//! ## Dependencies
//! - Canonical archive helpers from `crate::archive`.
//! - Batch review helpers from `super::batches`.
//! - Inspection/quarantine helpers from `super::inspect`.
//!
//! ## Performance notes
//! - Import still walks recognized files sequentially inside one transaction.
//! - Source-evidence writes happen after canonical commit so archive visibility
//!   is never half-written if source-evidence persistence fails.

use super::payload_import::{
    import_supported_payload, persist_takeout_source_evidence_plans, upsert_takeout_profile,
};
use super::{batches, inspect, *};

/// Imports a Takeout source into the canonical archive without streaming progress.
pub fn import_takeout(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    import_takeout_with_progress(paths, config, key, request, |_| {})
}

/// Imports a Takeout source and emits progress events for the desktop shell.
pub fn import_takeout_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TakeoutRequest,
    mut report_progress: F,
) -> Result<TakeoutInspection>
where
    F: FnMut(ImportProgressEvent),
{
    ensure_paths(paths)?;
    let mut inspection = inspect::inspect_takeout(paths, request)?;
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

    let batch_id =
        batches::create_import_batch(&transaction, &synthetic_profile, request, &inspection)?;
    let files = inspect::gather_takeout_files(source)?;
    let planned_files =
        inspection.recognized_files.iter().filter(|file| file.kind != KIND_INDEX).count();
    let mut stats = ImportStats::default();
    let mut source_evidence_plans = Vec::new();
    let mut progress_log_lines = vec![format!(
        "Queued {} Takeout payload(s) from {}.",
        planned_files.max(1),
        request.source_path
    )];
    emit_import_progress(
        &mut report_progress,
        "prepare",
        "Preparing import",
        format!("Scanning {} payload(s) before archive write.", planned_files.max(1)),
        0,
        planned_files.max(1),
        Some(request.source_path.clone()),
        &progress_log_lines,
    );

    let import_result = (|| -> Result<()> {
        let mut imported_file_count = 0usize;
        for file in files {
            let Some(kind) = inspect::recognize_takeout_file(&file.path) else {
                inspect::quarantine_takeout_file(paths, source, &file)?;
                progress_log_lines.push(format!("Quarantined unsupported payload {}.", file.path));
                continue;
            };
            if kind == KIND_INDEX {
                continue;
            }
            imported_file_count += 1;
            progress_log_lines.push(format!("Importing {kind} from {}.", file.path));
            emit_import_progress(
                &mut report_progress,
                "import-file",
                "Importing browser history",
                format!(
                    "Processing {} ({imported_file_count}/{})",
                    file.path,
                    planned_files.max(1)
                ),
                imported_file_count.saturating_sub(1),
                planned_files.max(1),
                Some(file.path.clone()),
                &progress_log_lines,
            );

            let bytes = if file.from_zip {
                inspect::read_zip_entry(source, &file.path)?
            } else {
                fs::read(&file.path)?
            };
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
                progress_log_lines.push(format!(
                    "Skipped {} record(s) without visit timestamps in {}.",
                    file_stats.stats.skipped_items, file.path
                ));
            }
            emit_import_progress(
                &mut report_progress,
                "import-file",
                "Importing browser history",
                format!("Imported {} ({imported_file_count}/{})", file.path, planned_files.max(1)),
                imported_file_count,
                planned_files.max(1),
                Some(file.path.clone()),
                &progress_log_lines,
            );
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
    progress_log_lines.push("Refreshing derived import review surfaces.".to_string());
    emit_import_progress(
        &mut report_progress,
        "finalize",
        "Finalizing import",
        "Refreshing keyword recall and batch review metadata.".to_string(),
        planned_files.max(1),
        planned_files.max(1),
        Some(request.source_path.clone()),
        &progress_log_lines,
    );
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
    batches::finalize_import_batch(&archive, batch_id, &inspection)?;
    finalize_successful_import_run(&archive, run_id, batch_id, &inspection, &stats)?;
    if let Err(error) = rebuild_search_projection(paths, config, key) {
        inspection.notes.push(format!(
            "Import completed, but the keyword-recall projection needs a rebuild: {error}"
        ));
    }

    batches::ensure_import_batch_audit_artifact(paths, config, key, batch_id, Some("imported"))?;

    let detail = batches::preview_import_batch(paths, config, key, batch_id)?;
    inspection.import_batch = Some(detail.batch);
    progress_log_lines.push(format!(
        "Imported {} new record(s); {} duplicate(s) skipped.",
        inspection.imported_items, inspection.duplicate_items
    ));
    emit_import_progress(
        &mut report_progress,
        "complete",
        "Import complete",
        "Takeout review is ready and follow-up rebuild work can continue in the background."
            .to_string(),
        planned_files.max(1),
        planned_files.max(1),
        Some(request.source_path.clone()),
        &progress_log_lines,
    );
    Ok(inspection)
}

/// Emits one shell-facing import progress event with a bounded recent log window.
fn emit_import_progress(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    phase: &str,
    label: &str,
    detail: String,
    current: usize,
    total: usize,
    source_path: Option<String>,
    log_lines: &[String],
) {
    report_progress(ImportProgressEvent {
        phase: phase.to_string(),
        label: label.to_string(),
        detail,
        current,
        total,
        progress_percent: if total == 0 {
            None
        } else {
            Some(((current as f32 / total as f32) * 100.0).min(100.0))
        },
        log_lines: log_lines
            .iter()
            .rev()
            .take(4)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect(),
        source_path,
    });
}

/// Creates the running import ledger row before archive writes begin.
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

/// Marks the import run successful and folds import counters into archive totals.
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

/// Marks the import run failed while keeping the partial counters visible for audit/debug.
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
