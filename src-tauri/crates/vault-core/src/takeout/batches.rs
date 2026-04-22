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

use super::*;

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

/// Loads one import batch detail and recreates its audit artifact if it is missing.
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
fn update_batch_audit(
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

/// Loads the full review detail for one import batch.
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

/// Loads the persisted batch row and reconstructs its review metadata.
pub(super) fn load_import_batch_record(
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

/// Rebuilds the lightweight overview row consumed by recent-import surfaces.
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

/// Extracts one integer summary field from the stored summary JSON.
fn summary_count(summary: &Value, key: &str) -> usize {
    summary.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
}
