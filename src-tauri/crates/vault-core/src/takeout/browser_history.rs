//! Responsibilities: preview and import staged browser files through the shared writer.
//! Not responsible for: browser SQLite staging, parser internals or search indexing.
//! Failure modes: source profiles merge, reimports duplicate, parse/cancel leaves
//! a running run, committed evidence/cursors diverge, preview counts omit duplicates.
//! Tests: tests::browser_history and browser_history::consumer_tests.

use super::*;
use crate::{
    Vault,
    archive::ingest::{
        ArchiveChunkConsumer, ArchiveIngestProgress, ImportOrigin, WriteContext, parser::Watermark,
    },
};
use browser_history_parser::{HistoryBatchConsumer, ParsedUrl, ParsedVisit};
use preview::{ArchiveDuplicateProbe, BrowserPreviewCollector};
use progress::*;
use staging::{
    StagedBrowserHistorySource, browser_file_report, stage_browser_history_source,
    stream_browser_history,
};
use vault_store::{CancelToken, Db, Priority, Session, params};

#[cfg(test)]
mod consumer_tests;
mod preview;
mod progress;
mod review;
mod staging;
const BROWSER_DIRECT_SOURCE_KIND: &str = "browser-history";
type BrowserImportProgress = ArchiveIngestProgress;

/// Preview uses the same source identity and dedup keys as execution.
pub fn inspect_browser_history(
    vault: &Vault,
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    ensure_paths(vault.paths())?;
    let staged = stage_browser_history_source(vault.paths(), request)?;
    let mut collector = BrowserPreviewCollector {
        source_path: staged.requested_path.display().to_string(),
        duplicates: ArchiveDuplicateProbe::open(vault, &staged)?,
        ..BrowserPreviewCollector::default()
    };
    let streamed = stream_browser_history(&staged, &mut collector)?;
    let notes = parser_warning_notes(&streamed.warnings);
    Ok(TakeoutInspection {
        source_path: request.source_path.clone(),
        dry_run: true,
        recognized_files: vec![browser_file_report(
            &staged,
            "previewed",
            collector.candidate_items,
        )],
        candidate_items: collector.candidate_items,
        duplicate_items: collector.duplicate_items,
        preview_entries: collector.preview_entries,
        preview_range_start: collector.preview_range.start,
        preview_range_end: collector.preview_range.end,
        notes: notes.iter().map(|n| n.message.clone()).collect(),
        note_details: notes,
        ..TakeoutInspection::default()
    })
}

/// Keeps direct browser imports on the same canonical writer as backup.
pub fn import_browser_history(
    vault: &Vault,
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    import_browser_history_with_progress(vault, request, |_| {})
}
/// Reports committed record progress without holding the whole import transaction.
pub fn import_browser_history_with_progress(
    vault: &Vault,
    request: &BrowserHistoryImportRequest,
    progress: impl FnMut(ImportProgressEvent),
) -> Result<TakeoutInspection> {
    import_browser_history_with_control(vault, request, &CancelToken::new(), progress)
}

/// An interrupted import retains its batch ID so its committed prefix can be reverted. One that
/// `cancel` stops returns what it saved (`TakeoutInspection::stopped`); the next import of the
/// file is a new batch, where those visits count as already in the archive.
pub fn import_browser_history_with_control(
    vault: &Vault,
    request: &BrowserHistoryImportRequest,
    cancel: &CancelToken,
    mut report: impl FnMut(ImportProgressEvent),
) -> Result<TakeoutInspection> {
    if request.dry_run {
        return inspect_browser_history(vault, request);
    }
    anyhow::ensure!(
        vault.config().initialized,
        "archive must be initialized before importing browser history data"
    );
    ensure_paths(vault.paths())?;
    let staged = stage_browser_history_source(vault.paths(), request)?;
    let source_path = request.source_path.clone();
    let (staged, source_profile_id, batch_id, run_id) =
        vault.write(Db::Archive, Priority::Batch, move |s| {
            let id = upsert_browser_history_profile(s, &staged)?;
            let batch = batches::create_import_batch_for_source(
                s,
                batches::ImportBatchSource {
                    source_kind: BROWSER_DIRECT_SOURCE_KIND,
                    source_path: &source_path,
                    profile_id: &staged.profile_id,
                },
            )?;
            s.execute("UPDATE import_batches SET summary_json=?2 WHERE id=?1",params![batch,json!({
                "candidateItems":0,"importedItems":0,"duplicateItems":0,
                "recognizedFiles":[browser_file_report(&staged,"running",0)],"quarantinedFiles":[],
                "notes":[],"noteDetails":[],"detectedLocale":null,"previewRangeStart":null,"previewRangeEnd":null
            }).to_string()])?;
            let run=crate::archive::write_runs::start_in(s,"import","manual",std::slice::from_ref(&staged.profile_id))?;
            s.execute("UPDATE runs SET stats_json=?2 WHERE id=?1",params![run,json!({"importBatchId":batch}).to_string()])?;
            Ok((staged, id, batch, run))
        })?;
    let guard = super::import_run::ImportRun::new(vault, run_id, batch_id);
    let mut partial_review = None;
    let result=guard.run(|| {
    let source_batch = SourceBatchInput {
        source_profile_id,
        run_id: Some(run_id),
        source_kind: "local_browser_history".into(),
        browser_version: None,
        schema_version_text: Some(staged.family.as_str().into()),
        schema_version_int: None,
        schema_fingerprint: String::new(),
        capability_snapshot: browser_history_parser::CapabilitySnapshot::default(),
        coverage_stats_json: "{}".into(),
        artifact_refs_json: Some(
            json!({"sourcePath":staged.requested_path,"browserFamily":staged.family.as_str()})
                .to_string(),
        ),
        notes_json: None,
    };
    let source_batch_id = vault.write(Db::SourceEvidence, Priority::Batch, move |s| {
        upsert_source_batch(s, &source_batch)
    })?;
    let context = WriteContext {
        run_id,
        source_profile_id,
        profile_id: staged.profile_id.clone(),
        browser_family: staged.family.as_str().into(),
        import: Some(ImportOrigin {
            batch_id,
            source_kind: staged.family.source_kind().into(),
            source_path: None,
            // A browser's history file starts a new batch each time (Takeout carries on).
            continuing: false,
        }),
    };
    let source_label = staged.requested_path.display().to_string();
    let logs = vec![format!("Staged {} for {}.", source_label, staged.browser_name)];
    emit_browser_import_progress(
        &mut report,
        "prepare",
        format!("Processing {source_label}"),
        0,
        1,
        Some(0.0),
        Some(source_label.clone()),
        &logs,
    );
    let mut previous = 0;
    let mut consumer = ArchiveChunkConsumer::new(
        vault,
        context,
        Watermark::default(),
        cancel,
        Some(Box::new(|p| {
            emit_browser_import_progress_if_changed(
                &mut report,
                &mut previous,
                &source_label,
                &staged.browser_name,
                &staged.profile_name,
                &logs,
                p,
            );
        })),
        source_batch_id,
    );
    let streamed=stream_browser_history(&staged,&mut consumer)?;
    let counts=consumer.finish()?;
    let stats = ImportStats {
        imported_items: counts.new_visits,
        duplicate_items: counts.visit_count - counts.new_visits - counts.same_visits,
        same_visit_items: counts.same_visits,
        replaced_profiles: counts.replaced_profiles.clone(),
        skipped_items: counts.skipped_visits,
    };
    let replaced = stats.replaced_profiles.iter().copied().collect::<Vec<_>>();
    let replaced_profiles = vault.read(Db::Archive, move |s| {
        Ok(crate::archive::same_visit::profile_keys(s, &replaced)?)
    })?;
    let notes = parser_warning_notes(&streamed.warnings);
    let mut inspection = TakeoutInspection {
        source_path: request.source_path.clone(),
        dry_run: false,
        candidate_items: counts.visit_count + counts.skipped_visits,
        imported_items: stats.imported_items,
        duplicate_items: stats.duplicate_items,
        same_visit_items: stats.same_visit_items,
        replaced_profiles,
        recognized_files: vec![browser_file_report(&staged, "previewed", counts.visit_count)],
        notes: notes.iter().map(|n| n.message.clone()).collect(),
        note_details: notes,
        preview_range_start: counts.first_visit_iso,
        preview_range_end: counts.last_visit_iso,
        ..TakeoutInspection::default()
    };
    append_browser_import_skipped_note(&mut inspection, stats.skipped_items);
    partial_review=Some(inspection.clone());
    vault.write(Db::SourceEvidence,Priority::Batch,move |s| {
        s.execute("UPDATE source_batches SET schema_fingerprint=?2 WHERE id=?1",params![source_batch_id,sha256_hex(&serde_json::to_vec(&streamed.schema_observation)?)])?;
        record_schema_observation(s,source_batch_id,"browser-history-database",&streamed.schema_observation)?;
        s.execute("UPDATE source_batches SET capability_snapshot_json=?2,coverage_stats_json=?3,notes_json=?4 WHERE id=?1",
            params![source_batch_id,serde_json::to_string(&streamed.capability_snapshot)?,counts.evidence_counts.coverage(counts.url_count,counts.visit_count,counts.download_count,counts.search_term_count),serde_json::to_string(&streamed.warnings)?])?;Ok(())
    })?;
    super::import_flow::finalize_successful_import_run(
        vault,
        run_id,
        batch_id,
        &inspection,
        &stats,
    )?;
    // The records are in; what follows only describes them.
    crate::archive::write_runs::after_success(
        vault,
        run_id,
        "writing its review record",
        batches::ensure_import_batch_audit_artifact(vault, batch_id, Some("imported")),
    );
    if let Some(detail) =
        crate::archive::write_runs::after_success(vault, run_id, "reading it back for review", preview_import_batch(vault, batch_id))
    {
        inspection.import_batch = Some(detail.batch);
        inspection.preview_entries = detail.preview_entries;
    }
    emit_browser_import_progress(
        &mut report,
        "complete",
        "Browser Direct import review is ready.".into(),
        1,
        1,
        Some(100.0),
        Some(source_label),
        &logs,
    );
    Ok(inspection)
    });
    match result {
        // Stopped by the user: what it saved stays, and nothing failed to read.
        Err(error) if super::import_flow::stopped(cancel, &error) => {
            batches::stopped_inspection(vault, batch_id, &request.source_path, None)
        }
        result => result.map_err(|error| {
            let saved =
                review::save_failure(vault, batch_id, &request.source_path, partial_review, &error);
            super::import_run::with_review_context(error, saved)
        }),
    }
}

fn upsert_browser_history_profile(
    archive: &Session<'_>,
    staged: &StagedBrowserHistorySource,
) -> Result<i64> {
    let enabled = archive
        .query(
            "SELECT enabled FROM source_profiles WHERE profile_key=?1",
            [staged.profile_id.as_str()],
            |r| Ok(r.get::<i64>(0)?),
        )?
        .pop();
    if enabled != Some(1) {
        crate::archive::totals::add(archive, "sourceProfiles", 1)?;
    }
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
         VALUES (?1, ?2, ?3, 'browser-direct', ?4, ?5, ?6, 1, ?7, NULL, ?6)
         ON CONFLICT(profile_key) DO UPDATE SET
           profile_name = excluded.profile_name,
           profile_path = excluded.profile_path,
           browser_family = excluded.browser_family,
           browser_product = excluded.browser_product,
           browser_version = excluded.browser_version,
           updated_at = excluded.updated_at,
           enabled = 1",
        params![
            staged.family.as_str(),
            staged.family.as_str(),
            staged.browser_name.as_str(),
            staged.profile_name.as_str(),
            staged.requested_path.display().to_string(),
            now_rfc3339(),
            staged.profile_id.as_str(),
        ],
    )?;
    archive
        .query_one(
            "SELECT id FROM source_profiles WHERE profile_key = ?1",
            [staged.profile_id.as_str()],
            |row| Ok(row.get(0)?),
        )
        .map_err(Into::into)
}
