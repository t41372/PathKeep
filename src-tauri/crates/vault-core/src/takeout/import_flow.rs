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
    TakeoutPayloadImportContext, TakeoutPayloadProgress, import_supported_payload_with_progress,
    persist_takeout_source_evidence_plans, upsert_takeout_profile,
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
    if request.dry_run {
        return inspect::inspect_takeout(paths, request);
    }

    if !config.initialized {
        anyhow::bail!("archive must be initialized before importing takeout data")
    }

    let source = Path::new(&request.source_path);
    let files = inspect::gather_takeout_files(source)?;
    let classified_files = files
        .iter()
        .map(|file| inspect::classify_takeout_file(source, file))
        .collect::<Result<Vec<_>>>()?;
    let planned_files = classified_files
        .iter()
        .filter(|file| {
            file.path_match.disposition == TakeoutPathDisposition::WillImport
                && file.path_match.recognized_kind.is_some_and(|kind| kind != KIND_INDEX)
        })
        .count();
    if planned_files == 0 {
        return inspect::inspect_takeout(paths, request);
    }
    let mut inspection = TakeoutInspection {
        source_path: request.source_path.clone(),
        dry_run: false,
        ..TakeoutInspection::default()
    };
    let mut found_importable_payload = false;
    let synthetic_profile = "takeout::browser-history".to_string();
    let started_at = now_rfc3339();

    let mut archive = open_archive_connection(paths, config, key)?;
    create_schema(&archive)?;
    let run_id = create_import_run(&archive, &synthetic_profile, &started_at)?;
    let transaction = archive.transaction()?;
    let source_profile_id = upsert_takeout_profile(&transaction, &synthetic_profile, source)?;
    let batch_id = batches::create_import_batch(&transaction, &synthetic_profile, request)?;
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
        format!("Scanning {} payload(s) before archive write.", planned_files.max(1)),
        0,
        planned_files.max(1),
        progress_percent_from_counts(0, planned_files.max(1)),
        Some(request.source_path.clone()),
        &progress_log_lines,
    );

    let import_result = (|| -> Result<()> {
        let mut imported_file_count = 0usize;
        for classified_file in classified_files {
            let file = classified_file.file;
            merge_detected_locale(
                &mut inspection.detected_locale,
                classified_file.path_match.locale,
            );

            if classified_file.path_match.disposition == TakeoutPathDisposition::NeedsReview {
                inspect::quarantine_takeout_file(paths, source, file)?;
                inspection.quarantined_files.push(file_report_from_match(
                    classified_file,
                    "needs-review",
                    0,
                    None,
                ));
                progress_log_lines.push(format!("Queued review-needed payload {}.", file.path));
                continue;
            }

            let Some(kind) = classified_file.path_match.recognized_kind else {
                inspection.recognized_files.push(file_report_from_match(
                    classified_file,
                    "ignored",
                    0,
                    None,
                ));
                continue;
            };
            if kind == KIND_INDEX {
                inspection.recognized_files.push(file_report_from_match(
                    classified_file,
                    "ignored",
                    0,
                    None,
                ));
                continue;
            }
            if classified_file.path_match.disposition == TakeoutPathDisposition::WillImport {
                found_importable_payload = true;
            }
            imported_file_count += 1;
            progress_log_lines.push(format!("Importing {kind} from {}.", file.path));
            emit_import_progress(
                &mut report_progress,
                "import-file",
                format!(
                    "Processing {} ({imported_file_count}/{})",
                    file.path,
                    planned_files.max(1)
                ),
                imported_file_count,
                planned_files.max(1),
                None,
                Some(file.path.clone()),
                &progress_log_lines,
            );

            let bytes = if file.from_zip {
                inspect::read_zip_entry(source, &file.path)?
            } else {
                fs::read(&file.path)?
            };
            let mut last_processed_records = 0usize;
            let source_label = file.path.clone();
            let file_stats = match import_supported_payload_with_progress(
                TakeoutPayloadImportContext {
                    paths,
                    archive: &transaction,
                    run_id,
                    batch_id,
                    source_profile_id,
                },
                classified_file,
                kind,
                &bytes,
                Some(Box::new(|progress: TakeoutPayloadProgress| {
                    emit_import_record_progress_if_changed(
                        &mut report_progress,
                        &mut last_processed_records,
                        imported_file_count,
                        planned_files,
                        &source_label,
                        &progress_log_lines,
                        progress,
                    );
                })),
            ) {
                Ok(file_stats) => file_stats,
                Err(error) => {
                    let mut report = file_report_from_match(
                        classified_file,
                        "parse-error",
                        0,
                        Some(error.to_string()),
                    );
                    report.classification = "parse-error".to_string();
                    report.reason_code = Some("parse-error".to_string());
                    inspection.recognized_files.push(report);
                    inspection.notes.push(format!("Could not parse {}: {}", file.path, error));
                    return Err(error);
                }
            };
            inspection.candidate_items += file_stats.record_count;
            inspection.recognized_files.push(file_stats.recognized_file);
            let mut preview_range = PreviewRangeSummary {
                start: inspection.preview_range_start.take(),
                end: inspection.preview_range_end.take(),
            };
            merge_preview_range(
                &mut preview_range,
                file_stats.earliest_visit_iso.as_deref(),
                file_stats.latest_visit_iso.as_deref(),
            );
            inspection.preview_range_start = preview_range.start;
            inspection.preview_range_end = preview_range.end;
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
                format!("Imported {} ({imported_file_count}/{})", file.path, planned_files.max(1)),
                imported_file_count,
                planned_files.max(1),
                progress_percent_from_counts(imported_file_count, planned_files.max(1)),
                Some(file.path.clone()),
                &progress_log_lines,
            );
        }

        debug_assert!(found_importable_payload);

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
        "Refreshing keyword recall and batch review metadata.".to_string(),
        planned_files.max(1),
        planned_files.max(1),
        progress_percent_from_counts(planned_files.max(1), planned_files.max(1)),
        Some(request.source_path.clone()),
        &progress_log_lines,
    );
    inspection.notes.extend(
        persist_takeout_source_evidence_plans(
            paths,
            config,
            key,
            &synthetic_profile,
            &source_evidence_plans,
        )
        .err()
        .map(takeout_source_evidence_rebuild_note),
    );
    batches::finalize_import_batch(&archive, batch_id, &inspection)?;
    finalize_successful_import_run(&archive, run_id, batch_id, &inspection, &stats)?;
    inspection.notes.extend(
        refresh_search_projection_for_import_batch(paths, config, key, batch_id)
            .err()
            .map(takeout_keyword_recall_rebuild_note),
    );

    batches::ensure_import_batch_audit_artifact(paths, config, key, batch_id, Some("imported"))?;

    let detail = preview_import_batch(paths, config, key, batch_id)?;
    inspection.import_batch = Some(detail.batch);
    inspection.preview_entries = detail.preview_entries;
    inspection.recognized_files = detail.recognized_files;
    inspection.quarantined_files = detail.quarantined_files;
    inspection.notes = detail.notes;
    inspection.detected_locale = detail.detected_locale;
    inspection.preview_range_start = detail.preview_range_start;
    inspection.preview_range_end = detail.preview_range_end;
    progress_log_lines.push(format!(
        "Imported {} new record(s); {} duplicate(s) skipped.",
        inspection.imported_items, inspection.duplicate_items
    ));
    emit_import_progress(
        &mut report_progress,
        "complete",
        "Takeout review is ready and follow-up rebuild work can continue in the background."
            .to_string(),
        planned_files.max(1),
        planned_files.max(1),
        progress_percent_from_counts(planned_files.max(1), planned_files.max(1)),
        Some(request.source_path.clone()),
        &progress_log_lines,
    );
    Ok(inspection)
}

#[derive(Default)]
struct ImportProgressRecordState {
    source_label: Option<String>,
    processed_records: Option<usize>,
    total_records: Option<usize>,
    imported_records: Option<usize>,
    duplicate_records: Option<usize>,
    skipped_records: Option<usize>,
}

struct ImportProgressEventInput<'a> {
    phase: &'a str,
    detail: String,
    current: usize,
    total: usize,
    progress_percent: Option<f32>,
    source_path: Option<String>,
    log_lines: &'a [String],
    record_state: ImportProgressRecordState,
}

/// Emits one shell-facing import progress event with a bounded recent log window.
fn emit_import_progress(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    phase: &str,
    detail: String,
    current: usize,
    total: usize,
    progress_percent: Option<f32>,
    source_path: Option<String>,
    log_lines: &[String],
) {
    emit_import_progress_with_records(
        report_progress,
        ImportProgressEventInput {
            phase,
            detail,
            current,
            total,
            progress_percent,
            source_path,
            log_lines,
            record_state: ImportProgressRecordState::default(),
        },
    );
}

fn emit_import_progress_with_records(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    input: ImportProgressEventInput<'_>,
) {
    let ImportProgressEventInput {
        phase,
        detail,
        current,
        total,
        progress_percent,
        source_path,
        log_lines,
        record_state,
    } = input;
    report_progress(
        ImportProgressEvent {
            phase: phase.to_string(),
            label: progress_label_for_phase(phase).to_string(),
            detail,
            current,
            total,
            progress_percent,
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
            source_label: record_state.source_label,
            processed_records: record_state.processed_records,
            total_records: record_state.total_records,
            imported_records: record_state.imported_records,
            duplicate_records: record_state.duplicate_records,
            skipped_records: record_state.skipped_records,
            log_events: Vec::new(),
        }
        .with_log_event(progress_log_level_for_phase(phase), &format!("import.{phase}")),
    );
}

pub(super) fn emit_import_record_progress_if_changed(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    last_processed_records: &mut usize,
    imported_file_count: usize,
    planned_files: usize,
    source_label: &str,
    progress_log_lines: &[String],
    progress: TakeoutPayloadProgress,
) {
    if progress.processed_records == *last_processed_records {
        return;
    }
    *last_processed_records = progress.processed_records;
    emit_import_progress_with_records(
        report_progress,
        ImportProgressEventInput {
            phase: "import-file",
            detail: format!(
                "Processing {} ({imported_file_count}/{})",
                source_label,
                planned_files.max(1)
            ),
            current: imported_file_count,
            total: planned_files.max(1),
            progress_percent: None,
            source_path: Some(source_label.to_string()),
            log_lines: progress_log_lines,
            record_state: ImportProgressRecordState {
                source_label: Some(source_label.to_string()),
                processed_records: Some(progress.processed_records),
                total_records: None,
                imported_records: Some(progress.imported_records),
                duplicate_records: Some(progress.duplicate_records),
                skipped_records: Some(progress.skipped_records),
            },
        },
    );
}

pub(super) fn progress_percent_from_counts(current: usize, total: usize) -> Option<f32> {
    if total == 0 { None } else { Some(((current as f32 / total as f32) * 100.0).min(100.0)) }
}

pub(super) fn progress_label_for_phase(phase: &str) -> &'static str {
    match phase {
        "prepare" => "Preparing import",
        "import-file" => "Importing browser history",
        "finalize" => "Finalizing import",
        "complete" => "Import complete",
        _ => "Importing browser history",
    }
}

fn progress_log_level_for_phase(phase: &str) -> &'static str {
    match phase {
        "complete" => "success",
        _ => "info",
    }
}

pub(super) fn takeout_source_evidence_rebuild_note(error: anyhow::Error) -> String {
    format!(
        "Canonical Takeout import completed, but the source-evidence archive needs a rebuild: {error}"
    )
}

pub(super) fn takeout_keyword_recall_rebuild_note(error: anyhow::Error) -> String {
    format!("Import completed, but the keyword-recall projection needs a rebuild: {error}")
}

/// Creates the running import ledger row before archive writes begin.
pub(super) fn create_import_run(
    archive: &Connection,
    profile_id: &str,
    started_at: &str,
) -> Result<i64> {
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
pub(super) fn finalize_successful_import_run(
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
pub(super) fn finalize_failed_import_run(
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
