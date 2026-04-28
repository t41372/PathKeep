//! Browser-direct history import boundary.
//!
//! ## Responsibilities
//! - Stage local browser SQLite databases before parser access.
//! - Inspect and import Chrome/Safari history databases through the browser
//!   parser contract instead of the Takeout payload contract.
//! - Reuse import-batch review, source evidence, rollback, and search refresh
//!   surfaces so Browser Direct has the same trust workflow as Takeout.
//!
//! ## Not responsible for
//! - Discovering installed browser profiles; discovery stays in `chrome`.
//! - Inventing browser-specific artifacts that a source does not provide.
//! - Importing non-history sidecars as part of this local database flow.
//!
//! ## Dependencies
//! - Browser parser crate for already-staged SQLite databases.
//! - Archive/source-evidence helpers from `crate::archive`.
//! - Import batch helpers from the existing import review boundary.
//!
//! ## Performance notes
//! - Canonical URL/visit rows stream into the archive transaction in parser
//!   batches, while preview keeps only the first bounded set of rows.

use super::{
    batches::{self, ImportBatchSource},
    import_flow::{create_import_run, finalize_failed_import_run, finalize_successful_import_run},
    *,
};
use browser_history_parser::{
    HistoryBatchConsumer, ParsedUrl, ParsedVisit, SourceEvidenceChunk, StreamedHistory,
};
use staging::{
    StagedBrowserHistorySource, browser_file_report, stage_browser_history_source,
    stream_browser_history,
};

mod staging;

const BROWSER_DIRECT_SOURCE_KIND: &str = "browser-history";

#[derive(Debug, Default)]
struct BrowserPreviewCollector {
    preview_entries: Vec<TakeoutPreviewEntry>,
    candidate_items: usize,
    url_count: usize,
    preview_range: PreviewRangeSummary,
    source_path: String,
}

impl HistoryBatchConsumer for BrowserPreviewCollector {
    type Error = anyhow::Error;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        self.url_count += batch.len();
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.candidate_items += batch.len();
        for visit in batch {
            merge_preview_range(
                &mut self.preview_range,
                Some(&visit.visit_time_iso),
                Some(&visit.visit_time_iso),
            );
            if self.preview_entries.len() >= PREVIEW_LIMIT {
                continue;
            }
            self.preview_entries.push(TakeoutPreviewEntry {
                source_path: self.source_path.clone(),
                url: visit.url,
                title: visit.title,
                visited_at: visit.visit_time_iso,
                source_visit_id: visit.source_visit_id,
                status: "candidate".to_string(),
            });
        }
        Ok(())
    }

    fn retain_source_evidence_in_report(&self) -> bool {
        false
    }
}

#[derive(Debug, Default)]
struct BrowserImportCounts {
    urls: usize,
    visits: usize,
}

struct BrowserImportProgress {
    processed_records: usize,
    imported_records: usize,
    duplicate_records: usize,
    skipped_records: usize,
}

struct BrowserEvidencePersistInput<'a> {
    paths: &'a ProjectPaths,
    config: &'a AppConfig,
    key: Option<&'a str>,
    profile_id: &'a str,
    source_profile_id: i64,
    run_id: i64,
    staged: &'a StagedBrowserHistorySource,
    counts: &'a BrowserImportCounts,
}

struct BrowserHistoryArchiveConsumer<'a> {
    archive: &'a Transaction<'a>,
    run_id: i64,
    batch_id: i64,
    source_profile_id: i64,
    source_kind: &'static str,
    url_id_map: std::collections::BTreeMap<i64, i64>,
    source_evidence: DeferredSourceEvidenceBuilder,
    preview_range: PreviewRangeSummary,
    stats: ImportStats,
    counts: BrowserImportCounts,
    progress: Option<Box<dyn FnMut(BrowserImportProgress) + 'a>>,
}

impl<'a> BrowserHistoryArchiveConsumer<'a> {
    fn new(
        paths: &ProjectPaths,
        archive: &'a Transaction<'a>,
        run_id: i64,
        batch_id: i64,
        source_profile_id: i64,
        source_kind: &'static str,
        source_label: &str,
        progress: Option<Box<dyn FnMut(BrowserImportProgress) + 'a>>,
    ) -> Self {
        Self {
            archive,
            run_id,
            batch_id,
            source_profile_id,
            source_kind,
            url_id_map: std::collections::BTreeMap::new(),
            source_evidence: DeferredSourceEvidenceBuilder::new(paths, source_label),
            preview_range: PreviewRangeSummary::default(),
            stats: ImportStats::default(),
            counts: BrowserImportCounts::default(),
            progress,
        }
    }

    fn finish(
        self,
    ) -> Result<(
        BrowserImportCounts,
        ImportStats,
        PreviewRangeSummary,
        DeferredSourceEvidencePayload,
        SourceEvidenceCounts,
    )> {
        let source_evidence_counts = self.source_evidence.counts();
        let source_evidence_payload = self.source_evidence.finish()?;
        Ok((
            self.counts,
            self.stats,
            self.preview_range,
            source_evidence_payload,
            source_evidence_counts,
        ))
    }
}

impl HistoryBatchConsumer for BrowserHistoryArchiveConsumer<'_> {
    type Error = anyhow::Error;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        self.counts.urls += batch.len();
        for url in batch {
            let payload_hash = sha256_hex(serde_json::to_string(&url)?.as_bytes());
            let url_id = self.archive.query_row(
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
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(source_profile_id, source_url_id) DO UPDATE SET
                   url = excluded.url,
                   title = excluded.title,
                   visit_count = excluded.visit_count,
                   typed_count = excluded.typed_count,
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
                    url.visit_count.max(1),
                    url.typed_count.max(0),
                    url.last_visit_ms,
                    url.last_visit_iso,
                    self.source_profile_id,
                    self.run_id,
                    url.source_url_id.to_string(),
                    i64::from(url.hidden),
                    payload_hash,
                    now_rfc3339(),
                ],
                |row| row.get::<_, i64>(0),
            )?;
            self.url_id_map.insert(url.source_url_id, url_id);
        }
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        self.counts.visits += batch.len();
        for visit in batch {
            merge_preview_range(
                &mut self.preview_range,
                Some(&visit.visit_time_iso),
                Some(&visit.visit_time_iso),
            );
            let Some(&url_id) = self.url_id_map.get(&visit.source_url_id) else {
                self.stats.skipped_items += 1;
                continue;
            };
            let payload_hash = sha256_hex(serde_json::to_string(&visit)?.as_bytes());
            let inserted = self.archive.execute(
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
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    url_id,
                    visit.source_visit_id.to_string(),
                    visit.visit_time_ms,
                    visit.visit_time_iso,
                    visit.transition,
                    visit.visit_duration_ms,
                    self.source_profile_id,
                    self.run_id,
                    visit.from_visit,
                    visit.is_known_to_sync as i64,
                    visit.visited_link_id,
                    visit.external_referrer_url,
                    visit.app_id,
                    visit_event_fingerprint(
                        self.source_kind,
                        &visit.url,
                        visit.visit_time_ms,
                        visit.title.as_deref(),
                        visit.transition,
                        visit.app_id.as_deref(),
                    ),
                    payload_hash,
                    now_rfc3339(),
                    self.batch_id,
                ],
            )?;
            if inserted > 0 {
                self.stats.imported_items += 1;
            } else {
                self.stats.duplicate_items += 1;
            }
        }
        if let Some(progress) = self.progress.as_mut() {
            progress(BrowserImportProgress {
                processed_records: self.counts.visits,
                imported_records: self.stats.imported_items,
                duplicate_records: self.stats.duplicate_items,
                skipped_records: self.stats.skipped_items,
            });
        }
        Ok(())
    }

    fn source_evidence(&mut self, chunk: SourceEvidenceChunk) -> Result<(), Self::Error> {
        self.source_evidence.push(SourceEvidencePayload {
            typed_evidence: chunk.typed_evidence,
            native_entities: chunk.native_entities,
        })
    }

    fn retain_source_evidence_in_report(&self) -> bool {
        false
    }
}

/// Inspects one local browser history database without mutating the archive.
pub fn inspect_browser_history(
    paths: &ProjectPaths,
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    ensure_paths(paths)?;
    let staged = stage_browser_history_source(paths, request)?;
    let mut collector = BrowserPreviewCollector {
        source_path: staged.requested_path.display().to_string(),
        ..BrowserPreviewCollector::default()
    };
    let streamed = stream_browser_history(&staged, &mut collector)?;
    let notes = streamed.warnings.iter().map(|warning| warning.message.clone()).collect::<Vec<_>>();

    Ok(TakeoutInspection {
        source_path: request.source_path.clone(),
        dry_run: true,
        recognized_files: vec![browser_file_report(
            &staged,
            "previewed",
            collector.candidate_items,
        )],
        quarantined_files: Vec::new(),
        candidate_items: collector.candidate_items,
        imported_items: 0,
        duplicate_items: 0,
        preview_entries: collector.preview_entries,
        import_batch: None,
        notes,
        detected_locale: None,
        preview_range_start: collector.preview_range.start,
        preview_range_end: collector.preview_range.end,
    })
}

/// Imports one local browser history database into the canonical archive.
pub fn import_browser_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &BrowserHistoryImportRequest,
) -> Result<TakeoutInspection> {
    import_browser_history_with_progress(paths, config, key, request, |_| {})
}

/// Imports one local browser history database and emits desktop progress events.
pub fn import_browser_history_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &BrowserHistoryImportRequest,
    mut report_progress: F,
) -> Result<TakeoutInspection>
where
    F: FnMut(ImportProgressEvent),
{
    ensure_paths(paths)?;
    if request.dry_run {
        return inspect_browser_history(paths, request);
    }
    if !config.initialized {
        anyhow::bail!("archive must be initialized before importing browser history data")
    }

    let staged = stage_browser_history_source(paths, request)?;
    let started_at = now_rfc3339();
    let mut archive = open_archive_connection(paths, config, key)?;
    create_schema(&archive)?;
    let run_id = create_import_run(&archive, &staged.profile_id, &started_at)?;
    let transaction = archive.transaction()?;
    let source_profile_id = upsert_browser_history_profile(&transaction, &staged)?;
    let batch_id = batches::create_import_batch_for_source(
        &transaction,
        ImportBatchSource {
            source_kind: BROWSER_DIRECT_SOURCE_KIND,
            source_path: &request.source_path,
            profile_id: &staged.profile_id,
        },
    )?;

    let mut inspection = TakeoutInspection {
        source_path: request.source_path.clone(),
        dry_run: false,
        recognized_files: vec![browser_file_report(&staged, "importing", 0)],
        ..TakeoutInspection::default()
    };
    let mut progress_log_lines =
        vec![format!("Staged {} for {}.", staged.requested_path.display(), staged.browser_name)];
    emit_browser_import_progress(
        &mut report_progress,
        "prepare",
        format!("Validating {} before archive write.", staged.requested_path.display()),
        0,
        1,
        Some(0.0),
        Some(request.source_path.clone()),
        &progress_log_lines,
    );

    let import_result = (|| -> Result<(
        StreamedHistory,
        BrowserImportCounts,
        ImportStats,
        PreviewRangeSummary,
        DeferredSourceEvidencePayload,
        SourceEvidenceCounts,
    )> {
        progress_log_lines.push(format!("Importing {} rows.", staged.family.as_str()));
        emit_browser_import_progress(
            &mut report_progress,
            "import-file",
            format!("Processing {}", staged.requested_path.display()),
            1,
            1,
            None,
            Some(staged.requested_path.display().to_string()),
            &progress_log_lines,
        );
        let (
            streamed,
            counts,
            stats,
            preview_range,
            source_evidence_payload,
            source_evidence_counts,
        ) = {
            let source_label = staged.requested_path.display().to_string();
            let mut last_processed_records = 0usize;
            let mut consumer = BrowserHistoryArchiveConsumer::new(
                paths,
                &transaction,
                run_id,
                batch_id,
                source_profile_id,
                staged.family.source_kind(),
                &source_label,
                Some(Box::new(|progress: BrowserImportProgress| {
                    emit_browser_import_progress_if_changed(
                        &mut report_progress,
                        &mut last_processed_records,
                        &source_label,
                        &staged.browser_name,
                        &staged.profile_name,
                        &progress_log_lines,
                        progress,
                    );
                })),
            );
            let streamed = stream_browser_history(&staged, &mut consumer)?;
            let (
                counts,
                stats,
                preview_range,
                source_evidence_payload,
                source_evidence_counts,
            ) = consumer.finish()?;
            (
                streamed,
                counts,
                stats,
                preview_range,
                source_evidence_payload,
                source_evidence_counts,
            )
        };
        transaction.commit()?;
        Ok((
            streamed,
            counts,
            stats,
            preview_range,
            source_evidence_payload,
            source_evidence_counts,
        ))
    })();

    match import_result {
        Ok((
            streamed,
            counts,
            stats,
            preview_range,
            source_evidence_payload,
            source_evidence_counts,
        )) => {
            inspection.candidate_items = counts.visits;
            inspection.imported_items = stats.imported_items;
            inspection.duplicate_items = stats.duplicate_items;
            inspection.recognized_files =
                vec![browser_file_report(&staged, "previewed", counts.visits)];
            inspection.notes =
                streamed.warnings.iter().map(|warning| warning.message.clone()).collect::<Vec<_>>();
            append_browser_import_skipped_note(&mut inspection.notes, stats.skipped_items);
            inspection.preview_range_start = preview_range.start;
            inspection.preview_range_end = preview_range.end;

            inspection.notes.extend(
                persist_browser_source_evidence_plan(
                    BrowserEvidencePersistInput {
                        paths,
                        config,
                        key,
                        profile_id: &staged.profile_id,
                        source_profile_id,
                        run_id,
                        staged: &staged,
                        counts: &counts,
                    },
                    streamed,
                    source_evidence_payload,
                    source_evidence_counts,
                )
                .err()
                .map(browser_import_source_evidence_warning),
            );
            batches::finalize_import_batch(&archive, batch_id, &inspection)?;
            finalize_successful_import_run(&archive, run_id, batch_id, &inspection, &stats)?;
            inspection.notes.extend(
                refresh_search_projection_for_import_batch(paths, config, key, batch_id)
                    .err()
                    .map(browser_import_search_projection_warning),
            );
            batches::ensure_import_batch_audit_artifact(
                paths,
                config,
                key,
                batch_id,
                Some("imported"),
            )?;
            let detail = preview_import_batch(paths, config, key, batch_id)?;
            inspection.import_batch = Some(detail.batch);
            inspection.preview_entries = detail.preview_entries;
            inspection.recognized_files = detail.recognized_files;
            inspection.quarantined_files = detail.quarantined_files;
            inspection.notes = detail.notes;
            inspection.preview_range_start = detail.preview_range_start;
            inspection.preview_range_end = detail.preview_range_end;
            progress_log_lines.push(format!(
                "Imported {} new visit(s); {} duplicate(s) skipped.",
                inspection.imported_items, inspection.duplicate_items
            ));
            emit_browser_import_progress(
                &mut report_progress,
                "complete",
                "Browser Direct import review is ready.".to_string(),
                1,
                1,
                Some(100.0),
                Some(request.source_path.clone()),
                &progress_log_lines,
            );
            Ok(inspection)
        }
        Err(error) => {
            finalize_failed_browser_history_import(&archive, run_id, &inspection.notes, &error)?;
            Err(error)
        }
    }
}

fn should_emit_browser_import_progress(last_processed_records: &mut usize, current: usize) -> bool {
    if current == *last_processed_records {
        return false;
    }
    *last_processed_records = current;
    true
}

fn emit_browser_import_progress_if_changed(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    last_processed_records: &mut usize,
    source_label: &str,
    browser_name: &str,
    profile_name: &str,
    progress_log_lines: &[String],
    progress: BrowserImportProgress,
) {
    if !should_emit_browser_import_progress(last_processed_records, progress.processed_records) {
        return;
    }
    emit_browser_import_progress_with_records(
        report_progress,
        BrowserImportProgressEventInput {
            phase: "import-file",
            detail: format!("Processing {source_label}"),
            current: 1,
            total: 1,
            progress_percent: None,
            source_path: Some(source_label.to_string()),
            log_lines: progress_log_lines,
            record_state: BrowserImportProgressState {
                source_label: Some(format!("{browser_name} / {profile_name}")),
                processed_records: Some(progress.processed_records),
                total_records: None,
                imported_records: Some(progress.imported_records),
                duplicate_records: Some(progress.duplicate_records),
                skipped_records: Some(progress.skipped_records),
            },
        },
    );
}

fn append_browser_import_skipped_note(notes: &mut Vec<String>, skipped_items: usize) {
    if skipped_items > 0 {
        notes.push(format!(
            "Skipped {} visit row(s) because their URL row was not present in the source.",
            skipped_items
        ));
    }
}

#[cfg(test)]
fn append_browser_import_source_evidence_warning(notes: &mut Vec<String>, error: &anyhow::Error) {
    notes.push(browser_import_source_evidence_warning(anyhow::anyhow!("{error}")));
}

#[cfg(test)]
fn append_browser_import_search_projection_warning(notes: &mut Vec<String>, error: &anyhow::Error) {
    notes.push(browser_import_search_projection_warning(anyhow::anyhow!("{error}")));
}

fn browser_import_source_evidence_warning(error: anyhow::Error) -> String {
    format!(
        "Canonical Browser Direct import completed, but the source-evidence archive needs a rebuild: {error}"
    )
}

fn browser_import_search_projection_warning(error: anyhow::Error) -> String {
    format!("Import completed, but the keyword-recall projection needs a rebuild: {error}")
}

fn finalize_failed_browser_history_import(
    archive: &Connection,
    run_id: i64,
    notes: &[String],
    error: &anyhow::Error,
) -> Result<()> {
    finalize_failed_import_run(archive, run_id, notes, &ImportStats::default(), error)
}

fn upsert_browser_history_profile(
    archive: &Transaction<'_>,
    staged: &StagedBrowserHistorySource,
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
        .query_row(
            "SELECT id FROM source_profiles WHERE profile_key = ?1",
            [&staged.profile_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn persist_browser_source_evidence_plan(
    input: BrowserEvidencePersistInput<'_>,
    streamed: StreamedHistory,
    source_evidence_payload: DeferredSourceEvidencePayload,
    source_evidence_counts: SourceEvidenceCounts,
) -> Result<()> {
    let BrowserEvidencePersistInput {
        paths,
        config,
        key,
        profile_id,
        source_profile_id,
        run_id,
        staged,
        counts,
    } = input;
    let StreamedHistory { schema_observation, capability_snapshot, warnings, .. } = streamed;
    let observation_json = serde_json::to_string(&schema_observation)?;
    let source_batch = SourceBatchInput {
        source_profile_id,
        run_id: Some(run_id),
        source_kind: "local_browser_history".to_string(),
        browser_version: None,
        schema_version_text: Some(staged.family.as_str().to_string()),
        schema_version_int: None,
        schema_fingerprint: sha256_hex(observation_json.as_bytes()),
        capability_snapshot,
        coverage_stats_json: coverage_stats_json_from_counts(
            counts.urls,
            counts.visits,
            0,
            0,
            &source_evidence_counts,
        ),
        artifact_refs_json: Some(
            json!({
                "sourcePath": staged.requested_path.display().to_string(),
                "stagedHistoryPath": staged.history_path.display().to_string(),
                "browserFamily": staged.family.as_str(),
            })
            .to_string(),
        ),
        notes_json: Some(serde_json::to_string(&warnings)?),
    };
    let mut source_evidence = open_source_evidence_connection(paths, config, key)?;
    let transaction = source_evidence.transaction()?;
    let source_batch_id = upsert_source_batch(&transaction, &source_batch)?;
    record_schema_observation(
        &transaction,
        source_batch_id,
        "browser-history-database",
        &schema_observation,
    )?;
    source_evidence_payload.persist(&transaction, source_batch_id, source_profile_id)?;
    transaction.commit()?;

    let archive = open_archive_connection(paths, config, key)?;
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

#[derive(Default)]
struct BrowserImportProgressState {
    source_label: Option<String>,
    processed_records: Option<usize>,
    total_records: Option<usize>,
    imported_records: Option<usize>,
    duplicate_records: Option<usize>,
    skipped_records: Option<usize>,
}

struct BrowserImportProgressEventInput<'a> {
    phase: &'a str,
    detail: String,
    current: usize,
    total: usize,
    progress_percent: Option<f32>,
    source_path: Option<String>,
    log_lines: &'a [String],
    record_state: BrowserImportProgressState,
}

fn emit_browser_import_progress(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    phase: &str,
    detail: String,
    current: usize,
    total: usize,
    progress_percent: Option<f32>,
    source_path: Option<String>,
    log_lines: &[String],
) {
    emit_browser_import_progress_with_records(
        report_progress,
        BrowserImportProgressEventInput {
            phase,
            detail,
            current,
            total,
            progress_percent,
            source_path,
            log_lines,
            record_state: BrowserImportProgressState::default(),
        },
    );
}

fn emit_browser_import_progress_with_records(
    report_progress: &mut impl FnMut(ImportProgressEvent),
    input: BrowserImportProgressEventInput<'_>,
) {
    let BrowserImportProgressEventInput {
        phase,
        detail,
        current,
        total,
        progress_percent,
        source_path,
        log_lines,
        record_state,
    } = input;
    let label = match phase {
        "prepare" => "Preparing import",
        "import-file" => "Importing browser history",
        "finalize" => "Finalizing import",
        "complete" => "Import complete",
        _ => "Importing browser history",
    };
    report_progress(ImportProgressEvent {
        phase: phase.to_string(),
        label: label.to_string(),
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
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        archive::{create_schema, open_archive_connection},
        config::{ensure_paths, project_paths_with_root},
        models::{AppConfig, ArchiveMode},
        utils::now_rfc3339,
    };
    use tempfile::tempdir;

    fn parsed_url(source_url_id: i64) -> ParsedUrl {
        ParsedUrl {
            source_url_id,
            url: format!("https://example.com/{source_url_id}"),
            title: Some(format!("Example {source_url_id}")),
            visit_count: 1,
            typed_count: 0,
            last_visit_ms: 1_767_222_000_000 + source_url_id,
            last_visit_iso: "2026-01-01T00:00:00Z".to_string(),
            hidden: false,
        }
    }

    fn parsed_visit(source_visit_id: i64, source_url_id: i64) -> ParsedVisit {
        ParsedVisit {
            source_visit_id,
            source_url_id,
            url: format!("https://example.com/{source_url_id}"),
            title: Some(format!("Visit {source_visit_id}")),
            visit_time_ms: 1_767_222_000_000 + source_visit_id,
            visit_time_iso: "2026-01-01T00:00:00Z".to_string(),
            from_visit: None,
            transition: Some(1),
            visit_duration_ms: None,
            is_known_to_sync: false,
            visited_link_id: None,
            external_referrer_url: None,
            app_id: None,
        }
    }

    #[test]
    fn preview_collector_counts_urls_and_caps_preview_entries() {
        let mut collector = BrowserPreviewCollector {
            source_path: "/tmp/History".to_string(),
            ..BrowserPreviewCollector::default()
        };
        let visits = (0..(PREVIEW_LIMIT + 2))
            .map(|index| parsed_visit(index as i64, index as i64))
            .collect::<Vec<_>>();

        collector.urls(vec![parsed_url(1)]).expect("count urls");
        collector.visits(visits).expect("collect preview visits");

        assert_eq!(collector.url_count, 1);
        assert_eq!(collector.candidate_items, PREVIEW_LIMIT + 2);
        assert_eq!(collector.preview_entries.len(), PREVIEW_LIMIT);
        assert_eq!(collector.preview_entries[0].source_path, "/tmp/History");
        assert_eq!(collector.preview_range.start.as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    #[test]
    fn archive_consumer_reports_visits_without_matching_url_rows_as_skipped() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let mut archive = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&archive).expect("schema");
        let transaction = archive.transaction().expect("transaction");
        let mut progress_events = Vec::new();
        let mut consumer = BrowserHistoryArchiveConsumer::new(
            &paths,
            &transaction,
            1,
            2,
            3,
            "chromium-history-db",
            "Google Chrome / Primary",
            Some(Box::new(|progress| progress_events.push(progress))),
        );

        consumer.visits(vec![parsed_visit(10, 999)]).expect("consume visit");

        assert_eq!(consumer.counts.visits, 1);
        assert_eq!(consumer.stats.skipped_items, 1);
        drop(consumer);
        assert_eq!(progress_events.len(), 1);
        assert_eq!(progress_events[0].processed_records, 1);
        assert_eq!(progress_events[0].skipped_records, 1);
    }

    #[test]
    fn browser_history_import_requires_initialized_archive_before_staging() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let request = BrowserHistoryImportRequest {
            source_path: root.path().join("missing-history").display().to_string(),
            dry_run: false,
            browser_family: Some("chromium".to_string()),
            profile_id: Some("chrome:Default".to_string()),
            browser_name: Some("Google Chrome".to_string()),
            profile_name: Some("Default".to_string()),
        };

        let error = import_browser_history(&paths, &AppConfig::default(), None, &request)
            .expect_err("uninitialized archive should fail before staging");

        assert!(error.to_string().contains("archive must be initialized"));
    }

    #[test]
    fn browser_history_import_marks_run_failed_when_streaming_breaks_after_staging() {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("paths");
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        create_schema(&archive).expect("schema");
        let history_path = root.path().join("BrokenHistory");
        Connection::open(&history_path)
            .expect("broken history")
            .execute_batch(
                "CREATE TABLE urls (id INTEGER PRIMARY KEY);
                 CREATE TABLE visits (id INTEGER PRIMARY KEY);",
            )
            .expect("minimal broken chromium schema");
        let request = BrowserHistoryImportRequest {
            source_path: history_path.display().to_string(),
            dry_run: false,
            browser_family: Some("chromium".to_string()),
            profile_id: Some("chrome:Broken".to_string()),
            browser_name: Some("Google Chrome".to_string()),
            profile_name: Some("Broken".to_string()),
        };

        let error = import_browser_history(&paths, &config, None, &request)
            .expect_err("broken staged history should fail");
        let status = archive
            .query_row("SELECT status FROM runs ORDER BY id DESC LIMIT 1", [], |row| {
                row.get::<_, String>(0)
            })
            .expect("failed run status");

        assert!(!format!("{error:#}").is_empty());
        assert_eq!(status, "failed");
    }

    #[test]
    fn browser_import_progress_uses_fallback_label_and_log_tail() {
        let log_lines = vec![
            "one".to_string(),
            "two".to_string(),
            "three".to_string(),
            "four".to_string(),
            "five".to_string(),
        ];
        let mut events = Vec::new();

        emit_browser_import_progress_with_records(
            &mut |event| events.push(event),
            BrowserImportProgressEventInput {
                phase: "unexpected-phase",
                detail: "Working".to_string(),
                current: 2,
                total: 3,
                progress_percent: Some(66.0),
                source_path: Some("/tmp/History".to_string()),
                log_lines: &log_lines,
                record_state: BrowserImportProgressState {
                    source_label: Some("Google Chrome / Primary".to_string()),
                    processed_records: Some(5),
                    total_records: Some(9),
                    imported_records: Some(4),
                    duplicate_records: Some(1),
                    skipped_records: Some(0),
                },
            },
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].label, "Importing browser history");
        assert_eq!(events[0].phase, "unexpected-phase");
        assert_eq!(events[0].log_lines, vec!["two", "three", "four", "five"]);
        assert_eq!(events[0].processed_records, Some(5));
        assert_eq!(events[0].source_label.as_deref(), Some("Google Chrome / Primary"));
    }

    #[test]
    fn browser_import_note_and_failure_helpers_keep_review_contract() {
        let mut last_processed_records = 0;
        assert!(!should_emit_browser_import_progress(&mut last_processed_records, 0));
        assert!(should_emit_browser_import_progress(&mut last_processed_records, 3));
        assert_eq!(last_processed_records, 3);
        let mut progress_events = Vec::new();
        emit_browser_import_progress_if_changed(
            &mut |event| progress_events.push(event),
            &mut last_processed_records,
            "/tmp/History",
            "Google Chrome",
            "Primary",
            &["importing".to_string()],
            BrowserImportProgress {
                processed_records: 3,
                imported_records: 2,
                duplicate_records: 1,
                skipped_records: 0,
            },
        );
        assert!(progress_events.is_empty());
        emit_browser_import_progress_if_changed(
            &mut |event| progress_events.push(event),
            &mut last_processed_records,
            "/tmp/History",
            "Google Chrome",
            "Primary",
            &["importing".to_string()],
            BrowserImportProgress {
                processed_records: 4,
                imported_records: 3,
                duplicate_records: 1,
                skipped_records: 0,
            },
        );
        assert_eq!(progress_events.len(), 1);
        assert_eq!(progress_events[0].processed_records, Some(4));
        assert_eq!(progress_events[0].source_label.as_deref(), Some("Google Chrome / Primary"));

        let mut notes = Vec::new();
        append_browser_import_skipped_note(&mut notes, 2);
        append_browser_import_source_evidence_warning(
            &mut notes,
            &anyhow::anyhow!("source evidence offline"),
        );
        append_browser_import_search_projection_warning(
            &mut notes,
            &anyhow::anyhow!("projection offline"),
        );
        assert!(notes.iter().any(|note| note.contains("Skipped 2 visit row")));
        assert!(notes.iter().any(|note| note.contains("source-evidence archive")));
        assert!(notes.iter().any(|note| note.contains("keyword-recall projection")));

        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("open archive");
        create_schema(&archive).expect("schema");
        let now = now_rfc3339();
        archive
            .execute(
                "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES ('import', 'manual', ?1, 'UTC', 'running', '[]', '[]', '{}', 0)",
                [&now],
            )
            .expect("insert running import run");
        let run_id = archive.last_insert_rowid();

        finalize_failed_browser_history_import(
            &archive,
            run_id,
            &notes,
            &anyhow::anyhow!("stream failed"),
        )
        .expect("finalize failed import");
        let (status, error_message): (String, Option<String>) = archive
            .query_row("SELECT status, error_message FROM runs WHERE id = ?1", [run_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("failed run");
        assert_eq!(status, "failed");
        assert!(error_message.as_deref().is_some_and(|message| message.contains("stream failed")));
    }
}
