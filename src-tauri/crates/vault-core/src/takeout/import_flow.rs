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

/// Imports one recognized payload into canonical archive rows plus source-evidence plans.
fn import_supported_payload(
    archive: &Transaction<'_>,
    run_id: i64,
    batch_id: i64,
    source_profile_id: i64,
    source_path: &str,
    kind: &str,
    bytes: &[u8],
) -> Result<ImportedPayload> {
    let payload = inspect::collect_records_from_payload(source_path, kind, bytes)?;
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

/// Builds the cold source-evidence plan corresponding to one imported payload.
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

/// Persists all deferred source-evidence writes after canonical import success.
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

/// Advances the Takeout watermark so later provenance reads can find the newest source batch.
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

/// Ensures the synthetic Takeout profile exists before import writes start.
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
