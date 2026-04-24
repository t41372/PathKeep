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
use browser_history_parser::{HistoryBatchConsumer, ParsedUrl, ParsedVisit, StreamedHistory};
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
}

#[derive(Debug, Default)]
struct BrowserImportCounts {
    urls: usize,
    visits: usize,
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
    stats: ImportStats,
    counts: BrowserImportCounts,
}

impl<'a> BrowserHistoryArchiveConsumer<'a> {
    fn new(
        archive: &'a Transaction<'a>,
        run_id: i64,
        batch_id: i64,
        source_profile_id: i64,
        source_kind: &'static str,
    ) -> Self {
        Self {
            archive,
            run_id,
            batch_id,
            source_profile_id,
            source_kind,
            url_id_map: std::collections::BTreeMap::new(),
            stats: ImportStats::default(),
            counts: BrowserImportCounts::default(),
        }
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
        Ok(())
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

    let import_result = (|| -> Result<(StreamedHistory, BrowserImportCounts, ImportStats)> {
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
        let (streamed, counts, stats) = {
            let mut consumer = BrowserHistoryArchiveConsumer::new(
                &transaction,
                run_id,
                batch_id,
                source_profile_id,
                staged.family.source_kind(),
            );
            let streamed = stream_browser_history(&staged, &mut consumer)?;
            (streamed, consumer.counts, consumer.stats)
        };
        transaction.commit()?;
        Ok((streamed, counts, stats))
    })();

    match import_result {
        Ok((streamed, counts, stats)) => {
            inspection.candidate_items = counts.visits;
            inspection.imported_items = stats.imported_items;
            inspection.duplicate_items = stats.duplicate_items;
            inspection.recognized_files =
                vec![browser_file_report(&staged, "previewed", counts.visits)];
            inspection.notes =
                streamed.warnings.iter().map(|warning| warning.message.clone()).collect::<Vec<_>>();
            if stats.skipped_items > 0 {
                inspection.notes.push(format!(
                    "Skipped {} visit row(s) because their URL row was not present in the source.",
                    stats.skipped_items
                ));
            }
            let mut preview_range = PreviewRangeSummary::default();
            let mut preview_collector = BrowserPreviewCollector {
                source_path: staged.requested_path.display().to_string(),
                ..BrowserPreviewCollector::default()
            };
            stream_browser_history(&staged, &mut preview_collector)?;
            merge_preview_range(
                &mut preview_range,
                preview_collector.preview_range.start.as_deref(),
                preview_collector.preview_range.end.as_deref(),
            );
            inspection.preview_range_start = preview_range.start;
            inspection.preview_range_end = preview_range.end;

            if let Err(error) = persist_browser_source_evidence_plan(
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
            ) {
                inspection.notes.push(format!(
                    "Canonical Browser Direct import completed, but the source-evidence archive needs a rebuild: {error}"
                ));
            }
            batches::finalize_import_batch(&archive, batch_id, &inspection)?;
            finalize_successful_import_run(&archive, run_id, batch_id, &inspection, &stats)?;
            if let Err(error) = rebuild_search_projection(paths, config, key) {
                inspection.notes.push(format!(
                    "Import completed, but the keyword-recall projection needs a rebuild: {error}"
                ));
            }
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
            finalize_failed_import_run(
                &archive,
                run_id,
                &inspection.notes,
                &ImportStats::default(),
                &error,
            )?;
            Err(error)
        }
    }
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
    let mut builder =
        DeferredSourceEvidenceBuilder::new(paths, &staged.requested_path.display().to_string());
    let StreamedHistory {
        schema_observation,
        capability_snapshot,
        typed_evidence,
        native_entities,
        warnings,
        ..
    } = streamed;
    builder.push(SourceEvidencePayload { typed_evidence, native_entities })?;
    let evidence_counts = builder.counts();
    let source_evidence_payload = builder.finish()?;
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
            &evidence_counts,
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
    });
}
