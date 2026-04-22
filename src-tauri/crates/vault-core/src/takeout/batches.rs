//! Takeout batch review and rollback helpers.
//!
//! ## Responsibilities
//! - Load import-batch review read models and repair missing audit artifacts.
//! - Revert/restore imported visibility without deleting canonical audit facts.
//! - Maintain batch summary JSON and review artifact pointers.
//!
//! ## Not responsible for
//! - Parsing Takeout payloads.
//! - Executing the main import transaction.
//! - Persisting source-evidence payload rows.
//!
//! ## Dependencies
//! - Canonical archive helpers from `crate::archive`.
//! - Audit-repo helpers from `crate::git_audit`.
//!
//! ## Performance notes
//! - Batch review queries are bounded and preview-limited.
//! - Revert/restore rebuild the search projection after visibility changes so
//!   recall stays honest.

use super::batch_review::{load_import_batch_detail, load_import_batch_record};
use super::*;

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
            detected_locale: existing.detected_locale.as_deref(),
            preview_range_start: existing.preview_range_start.as_deref(),
            preview_range_end: existing.preview_range_end.as_deref(),
            reverted_at: Some(reverted_at),
        },
    )?;
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
    update_batch_summary(
        &transaction,
        BatchSummaryUpdate {
            batch_id,
            status: "imported",
            imported_items: existing.overview.imported_items,
            duplicate_items: existing.overview.duplicate_items,
            candidate_items: existing.overview.candidate_items,
            recognized_files: &existing.recognized_files,
            quarantined_files: &existing.quarantined_files,
            notes: &notes,
            detected_locale: existing.detected_locale.as_deref(),
            preview_range_start: existing.preview_range_start.as_deref(),
            preview_range_end: existing.preview_range_end.as_deref(),
            reverted_at: None,
        },
    )?;
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

/// Creates the initial running import-batch row before any payload writes begin.
pub(super) fn create_import_batch(
    archive: &Transaction<'_>,
    profile_id: &str,
    request: &TakeoutRequest,
) -> Result<i64> {
    let summary_json = serde_json::to_string(&json!({
        "candidateItems": 0,
        "importedItems": 0,
        "duplicateItems": 0,
        "recognizedFiles": [],
        "quarantinedFiles": [],
        "notes": [],
        "detectedLocale": null,
        "previewRangeStart": null,
        "previewRangeEnd": null,
    }))?;
    archive.execute(
        "INSERT INTO import_batches (source_kind, source_path, profile_id, created_at, status, summary_json)
         VALUES ('takeout', ?1, ?2, ?3, 'running', ?4)",
        params![request.source_path, profile_id, now_rfc3339(), summary_json],
    )?;
    Ok(archive.last_insert_rowid())
}

/// Finalizes the import-batch summary after canonical writes complete.
pub(super) fn finalize_import_batch(
    archive: &Connection,
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
            detected_locale: inspection.detected_locale.as_deref(),
            preview_range_start: inspection.preview_range_start.as_deref(),
            preview_range_end: inspection.preview_range_end.as_deref(),
            reverted_at: None,
        },
    )?;
    archive.execute(
        "UPDATE import_batches SET imported_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), batch_id],
    )?;
    Ok(())
}

/// Creates the rollback ledger row used when an import batch is reverted.
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

/// Creates the restore ledger row used when a reverted batch becomes visible again.
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

/// Recomputes completed run stats after a revert/restore action mutates visibility.
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

/// Updates the import-batch summary JSON and status in one place.
fn update_batch_summary(archive: &Connection, update: BatchSummaryUpdate<'_>) -> Result<()> {
    let summary_json = serde_json::to_string(&json!({
        "candidateItems": update.candidate_items,
        "importedItems": update.imported_items,
        "duplicateItems": update.duplicate_items,
        "recognizedFiles": update.recognized_files,
        "quarantinedFiles": update.quarantined_files,
        "notes": update.notes,
        "detectedLocale": update.detected_locale,
        "previewRangeStart": update.preview_range_start,
        "previewRangeEnd": update.preview_range_end,
    }))?;
    archive.execute(
        "UPDATE import_batches SET status = ?1, summary_json = ?2, reverted_at = COALESCE(?3, reverted_at) WHERE id = ?4",
        params![update.status, summary_json, update.reverted_at, update.batch_id],
    )?;
    Ok(())
}

/// Writes a review artifact for one batch and optionally commits it into the audit repo.
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

/// Stores the latest audit artifact pointer for one batch.
pub(super) fn update_batch_audit(
    connection: &Connection,
    batch_id: i64,
    audit_path: Option<&str>,
    git_commit: Option<&str>,
) -> Result<()> {
    connection.execute(
        "UPDATE import_batches SET audit_path = ?1, git_commit = ?2 WHERE id = ?3",
        params![audit_path, git_commit, batch_id],
    )?;
    Ok(())
}
