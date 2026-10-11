//! Responsibilities: retain import provenance and atomically revert/restore visibility.
//! Not responsible for: parsing, full-text indexing or browser staging.
//! Failure modes: shared pages disappear, hidden visits retain memberships,
//! changed-URL walks skip chunk boundaries, active producers race reversal,
//! outbox errors partially commit,
//! batch audit/provenance is lost. Tests: batch_search_tests and takeout::tests.

use super::{batch_review::load_import_batch_detail, *};
use crate::Vault;
use vault_store::{Db, Priority, Session, params};

/// Hides one import in resumable chunks without deleting its provenance.
pub fn revert_import_batch(vault: &Vault, batch_id: i64) -> Result<ImportBatchDetail> {
    super::visibility::change_visibility(
        vault,
        batch_id,
        false,
        &vault_store::CancelToken::new(),
        |_| {},
    )
}
/// Restores the batch's original imported, failed or interrupted state.
pub fn restore_import_batch(vault: &Vault, batch_id: i64) -> Result<ImportBatchDetail> {
    super::visibility::change_visibility(
        vault,
        batch_id,
        true,
        &vault_store::CancelToken::new(),
        |_| {},
    )
}

/// Lets a caller stop between committed URL chunks and later resume the same batch.
pub fn revert_import_batch_with_control(
    vault: &Vault,
    batch_id: i64,
    cancel: &vault_store::CancelToken,
    progress: impl FnMut(usize),
) -> Result<ImportBatchDetail> {
    super::visibility::change_visibility(vault, batch_id, false, cancel, progress)
}

/// Restores a partial visibility operation with cooperative cancellation.
pub fn restore_import_batch_with_control(
    vault: &Vault,
    batch_id: i64,
    cancel: &vault_store::CancelToken,
    progress: impl FnMut(usize),
) -> Result<ImportBatchDetail> {
    super::visibility::change_visibility(vault, batch_id, true, cancel, progress)
}

pub(crate) fn ensure_import_batch_audit_artifact(
    vault: &Vault,
    batch_id: i64,
    action_hint: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    let detail = vault.read(Db::Archive, move |s| load_import_batch_detail(s, batch_id))?;
    if detail.batch.audit_path.as_deref().is_some_and(|p| Path::new(p).exists()) {
        return Ok((detail.batch.audit_path, detail.batch.git_commit));
    }
    let action = action_hint.unwrap_or(if detail.batch.status == "reverted" {
        "reverted"
    } else {
        "imported"
    });
    let (path, commit) =
        write_batch_audit_detail(vault.paths(), vault.config(), batch_id, &detail, action)?;
    let (stored_path, stored_commit) = (path.clone(), commit.clone());
    vault.write(Db::Archive, Priority::Batch, move |s| {
        update_batch_audit(s, batch_id, Some(&stored_path), stored_commit.as_deref())
    })?;
    Ok((Some(path), commit))
}

pub(super) fn continue_or_create_takeout_batch(
    archive: &Session<'_>,
    profile_id: &str,
    request: &TakeoutRequest,
) -> Result<(i64, Option<ContinuedBatch>)> {
    continue_or_create_import_batch(
        archive,
        ImportBatchSource { source_kind: "takeout", source_path: &request.source_path, profile_id },
    )
}

/// Describes the minimal source identity needed to create an import batch.
pub(super) struct ImportBatchSource<'a> {
    pub(super) source_kind: &'a str,
    pub(super) source_path: &'a str,
    pub(super) profile_id: &'a str,
}

/// Creates the initial running import-batch row for any preview-first import source.
pub(super) fn create_import_batch_for_source(
    archive: &Session<'_>,
    source: ImportBatchSource<'_>,
) -> Result<i64> {
    let summary_json = serde_json::to_string(&json!({
        "candidateItems": 0,
        "importedItems": 0,
        "duplicateItems": 0,
        "recognizedFiles": [],
        "quarantinedFiles": [],
        "notes": [],
        "noteDetails": [],
        "detectedLocale": null,
        "previewRangeStart": null,
        "previewRangeEnd": null,
    }))?;
    let id=archive.query_one(
        "INSERT INTO import_batches (source_kind, source_path, profile_id, created_at, status, summary_json, previous_status)
         VALUES (?1, ?2, ?3, ?4, 'running', ?5, 'running') RETURNING id",
        params![
            source.source_kind,
            source.source_path,
            source.profile_id,
            now_rfc3339(),
            summary_json
        ],
        |r|Ok(r.get::<i64>(0)?),
    )?;
    Ok(id)
}

/// An interrupted import of the same source that a new run carries on (`continue_or_create`).
#[derive(Debug, Clone)]
pub(super) struct ContinuedBatch {
    /// When the interrupted import began.
    pub(super) created_at: String,
    /// What its earlier runs wrote, which this run adds to.
    pub(super) imported_items: usize,
    pub(super) same_visit_items: usize,
}

/// The batch a new import of `source` writes into: the latest interrupted import of the same
/// kind and path, carried on so that one import stays one batch (to undo together), or a new one.
/// An import that finished, or was undone, is never reopened: a run of the same file then adds a
/// new batch, whose records the earlier one already holds count as duplicates.
pub(super) fn continue_or_create_import_batch(
    archive: &Session<'_>,
    source: ImportBatchSource<'_>,
) -> Result<(i64, Option<ContinuedBatch>)> {
    let interrupted = archive.query(
        "SELECT id, created_at, summary_json FROM import_batches
         WHERE source_kind=?1 AND source_path=?2 AND status='interrupted'
         ORDER BY id DESC LIMIT 1",
        params![source.source_kind, source.source_path],
        |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
    )?;
    let Some((id, created_at, summary)) = interrupted.into_iter().next() else {
        return Ok((create_import_batch_for_source(archive, source)?, None));
    };
    let mut summary: serde_json::Value = serde_json::from_str(&summary)?;
    let count =
        |summary: &serde_json::Value, key: &str| summary[key].as_u64().unwrap_or(0) as usize;
    let (imported_items, same_visit_items) =
        (count(&summary, "importedItems"), count(&summary, "sameVisitItems"));
    // This run reads the whole export again, so what it reads and finds elsewhere is counted
    // afresh; what the earlier run wrote stays counted.
    summary["candidateItems"] = json!(0);
    summary["duplicateItems"] = json!(0);
    archive.execute(
        "UPDATE import_batches SET status='running', previous_status='running', summary_json=?2
         WHERE id=?1",
        params![id, summary.to_string()],
    )?;
    Ok((id, Some(ContinuedBatch { created_at, imported_items, same_visit_items })))
}

/// What an import the user stopped reports: the counts its batch holds, which every committed
/// batch of records has added to (`consumer::flush`), since a stop in the middle of a file
/// leaves the run's own tallies short. `continued_from` is set when it carried on an earlier
/// run, whose counts the batch includes.
pub(super) fn stopped_inspection(
    vault: &crate::Vault,
    batch_id: i64,
    source_path: &str,
    continued_from: Option<String>,
) -> Result<TakeoutInspection> {
    let (detail, same_visit_items) = vault.read(Db::Archive, move |s| {
        let detail = load_import_batch_detail(s, batch_id)?;
        let summary =
            s.query_one("SELECT summary_json FROM import_batches WHERE id=?1", [batch_id], |r| {
                Ok(r.get::<String>(0)?)
            })?;
        let summary: serde_json::Value = serde_json::from_str(&summary)?;
        Ok((detail, summary["sameVisitItems"].as_u64().unwrap_or(0) as usize))
    })?;
    Ok(TakeoutInspection {
        source_path: source_path.to_owned(),
        candidate_items: detail.batch.candidate_items,
        imported_items: detail.batch.imported_items,
        duplicate_items: detail.batch.duplicate_items,
        same_visit_items,
        continued_from,
        stopped: true,
        preview_entries: detail.preview_entries,
        recognized_files: detail.recognized_files,
        quarantined_files: detail.quarantined_files,
        notes: detail.notes,
        note_details: detail.note_details,
        detected_locale: detail.detected_locale,
        preview_range_start: detail.preview_range_start,
        preview_range_end: detail.preview_range_end,
        import_batch: Some(detail.batch),
        ..TakeoutInspection::default()
    })
}

/// Finalizes the import-batch summary after canonical writes complete.
pub(super) fn finalize_import_batch(
    archive: &Session<'_>,
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
            note_details: &inspection.note_details,
            detected_locale: inspection.detected_locale.as_deref(),
            preview_range_start: inspection.preview_range_start.as_deref(),
            preview_range_end: inspection.preview_range_end.as_deref(),
            reverted_at: None,
        },
    )?;
    archive.execute(
        "UPDATE import_batches SET imported_at = ?1, audit_path=NULL, git_commit=NULL WHERE id = ?2",
        params![now_rfc3339(), batch_id],
    )?;
    Ok(())
}

/// Updates the import-batch summary JSON and status in one place.
fn update_batch_summary(archive: &Session<'_>, update: BatchSummaryUpdate<'_>) -> Result<()> {
    let summary_json = serde_json::to_string(&json!({
        "candidateItems": update.candidate_items,
        "importedItems": update.imported_items,
        "duplicateItems": update.duplicate_items,
        "recognizedFiles": update.recognized_files,
        "quarantinedFiles": update.quarantined_files,
        "notes": update.notes,
        "noteDetails": update.note_details,
        "detectedLocale": update.detected_locale,
        "previewRangeStart": update.preview_range_start,
        "previewRangeEnd": update.preview_range_end,
    }))?;
    archive.execute(
        "UPDATE import_batches SET status = ?1, previous_status = ?1, summary_json = ?2, reverted_at = COALESCE(?3, reverted_at) WHERE id = ?4",
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
    let file_name =
        format!("imports/{}/batch-{}-{}.json", &detail.batch.created_at[0..10], batch_id, action);
    let contents = serde_json::to_string_pretty(detail)?;
    let audit_path = git_audit::write_audit_file(&paths.audit_repo_path, &file_name, &contents)?;
    let git_commit = if config.git_enabled {
        let (git_commit, _) = git_audit::commit_all_optional(
            &paths.audit_repo_path,
            &format!("import batch {batch_id} {action}"),
        );
        git_commit
    } else {
        None
    };
    Ok((audit_path.display().to_string(), git_commit))
}

/// Stores the latest audit artifact pointer for one batch.
pub(super) fn update_batch_audit(
    connection: &Session<'_>,
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

#[cfg(test)]
#[path = "batch_search_tests.rs"]
mod search_tests;

/// Preserve review evidence on an unsuccessful import without overwriting committed counts.
pub(super) fn save_partial_review(
    s: &Session<'_>,
    batch: i64,
    inspection: &TakeoutInspection,
) -> Result<()> {
    let text =
        s.query_one("SELECT summary_json FROM import_batches WHERE id=?1", [batch], |r| {
            Ok(r.get::<String>(0)?)
        })?;
    let mut summary: Value = serde_json::from_str(&text)?;
    summary["recognizedFiles"] = serde_json::to_value(&inspection.recognized_files)?;
    summary["quarantinedFiles"] = serde_json::to_value(&inspection.quarantined_files)?;
    summary["notes"] = serde_json::to_value(&inspection.notes)?;
    summary["noteDetails"] = serde_json::to_value(&inspection.note_details)?;
    summary["previewRangeStart"] = json!(inspection.preview_range_start);
    summary["previewRangeEnd"] = json!(inspection.preview_range_end);
    s.execute(
        "UPDATE import_batches SET summary_json=?2 WHERE id=?1",
        params![batch, summary.to_string()],
    )?;
    Ok(())
}
