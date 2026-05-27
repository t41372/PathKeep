//! Canonical backup ingest orchestration.
//!
//! ## Responsibilities
//! - Select readable browser profiles for a backup run.
//! - Translate staged parser output into canonical archive rows plus deferred
//!   source-evidence plans.
//! - Coordinate checkpoint creation and watermark advancement after a staged
//!   snapshot has been processed successfully.
//!
//! ## Not responsible for
//! - Driving the top-level backup run ledger or manifest lifecycle.
//! - Rendering read models for Explorer, Dashboard, or Audit.
//! - Retention, restore, or other recoverability flows after rows are written.
//!
//! ## Dependencies
//! - Parser contracts from `browser_history_parser`.
//! - Browser staging metadata from `crate::chrome`.
//! - Canonical archive/schema helpers defined in the parent `archive` module.
//!
//! ## Performance notes
//! - This module sits on the hottest large-data ingest path in the repo.
//! - All row writes happen inside caller-owned SQLite transactions to avoid
//!   partial commits and to keep archive/source-evidence updates aligned.

mod parser;
mod writes;

#[cfg(test)]
mod concurrency_tests;
#[cfg(test)]
mod dedup_scenarios;
#[cfg(test)]
mod dedup_scenarios_baselines;
#[cfg(test)]
mod dedup_scenarios_chromium_sidecars;
#[cfg(test)]
mod dedup_scenarios_edge_cases;
#[cfg(test)]
mod dedup_scenarios_takeout;

use self::{
    parser::{Watermark, load_watermark, save_watermark, should_checkpoint},
    writes::{
        UrlVisitBounds, canonical_url_exists, insert_download, insert_favicon, insert_search_term,
        insert_visit, sync_url_bounds, track_url_visit_bounds, upsert_source_profile, upsert_url,
    },
};
use super::*;
use browser_history_parser::{
    ChromiumReadCursor, HistoryBatchConsumer, HistoryDatabaseSet, ParsedDownload, ParsedFavicon,
    ParsedSearchTerm, ParsedUrl, ParsedVisit, StreamHistoryError, StreamedHistory, chromium,
    firefox, safari,
};
use std::collections::{BTreeMap, HashMap};

/// Parses a saved source checkpoint without incremental cursors so restore preview can size the replay.
pub(super) fn preview_snapshot_counts(
    snapshot: &ProfileSnapshot,
    config: &AppConfig,
) -> Result<(usize, usize, usize)> {
    parser::preview_snapshot_counts(snapshot, config)
}

/// Defers source-evidence persistence until canonical archive writes have committed.
#[derive(Debug)]
pub(super) struct SourceEvidencePlan {
    profile_id: String,
    source_profile_id: i64,
    source_batch: SourceBatchInput,
    schema_observation: browser_history_parser::SchemaObservation,
    source_evidence_payload: DeferredSourceEvidencePayload,
}

const ARCHIVE_STREAM_CHUNK_SIZE: usize = 10_000;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct ArchiveIngestProgress {
    pub(super) processed_records: usize,
    pub(super) imported_records: usize,
    pub(super) duplicate_records: usize,
    pub(super) skipped_records: usize,
}

#[derive(Debug, Default)]
struct ArchiveStreamProgress {
    url_id_map: HashMap<i64, i64>,
    url_bounds: HashMap<i64, UrlVisitBounds>,
    new_urls: usize,
    new_visits: usize,
    new_downloads: usize,
    skipped_visits: usize,
    inserted_search_terms: usize,
    url_count: usize,
    visit_count: usize,
    download_count: usize,
    search_term_count: usize,
    last_visit_id: i64,
    last_url_marker: Option<i64>,
    last_download_id: Option<i64>,
    last_favicon_marker: Option<i64>,
}

struct ArchiveChunkConsumer<'a> {
    archive: &'a Transaction<'a>,
    run_id: i64,
    source_profile_id: i64,
    profile: &'a crate::models::BrowserProfile,
    progress: ArchiveStreamProgress,
    report_progress: Option<Box<dyn FnMut(ArchiveIngestProgress) + 'a>>,
}

impl<'a> ArchiveChunkConsumer<'a> {
    fn new(
        archive: &'a Transaction<'a>,
        run_id: i64,
        source_profile_id: i64,
        profile: &'a crate::models::BrowserProfile,
        report_progress: Option<Box<dyn FnMut(ArchiveIngestProgress) + 'a>>,
    ) -> Self {
        Self {
            archive,
            run_id,
            source_profile_id,
            profile,
            progress: ArchiveStreamProgress::default(),
            report_progress,
        }
    }

    fn finish(mut self) -> Result<ArchiveStreamProgress> {
        for (url_id, bounds) in self.progress.url_bounds.drain() {
            sync_url_bounds(self.archive, url_id, &bounds)?;
        }
        Ok(self.progress)
    }
}

impl HistoryBatchConsumer for ArchiveChunkConsumer<'_> {
    type Error = anyhow::Error;

    fn urls(&mut self, batch: Vec<ParsedUrl>) -> Result<(), Self::Error> {
        for url in batch {
            let payload = serialize_payload(&url)?;
            let existing_url =
                canonical_url_exists(self.archive, self.source_profile_id, url.source_url_id)?;
            let canonical_url_id = upsert_url(
                self.archive,
                self.run_id,
                self.source_profile_id,
                self.profile,
                &url,
                &payload.hash,
            )?;
            self.progress.url_id_map.insert(url.source_url_id, canonical_url_id);
            if !existing_url {
                self.progress.new_urls += 1;
            }
            self.progress.url_count += 1;
            let url_marker = url_last_visit_marker(self.profile, &url);
            self.progress.last_url_marker =
                Some(self.progress.last_url_marker.unwrap_or_default().max(url_marker));
        }
        Ok(())
    }

    fn visits(&mut self, batch: Vec<ParsedVisit>) -> Result<(), Self::Error> {
        for visit in batch {
            let Some(&url_id) = self.progress.url_id_map.get(&visit.source_url_id) else {
                self.progress.skipped_visits += 1;
                continue;
            };
            let payload = serialize_payload(&visit)?;
            let inserted = insert_visit(
                self.archive,
                self.run_id,
                self.source_profile_id,
                &self.profile.profile_id,
                url_id,
                &visit,
                &payload.hash,
            )?;
            if inserted > 0 {
                self.progress.new_visits += 1;
                // Only widen URL bounds from visits that actually landed.
                // INSERT OR IGNORE may drop a visit on either unique-index
                // hit (`(url_id, source_visit_id)` or the fingerprint
                // partial index); in either case the visit row is not in
                // the canonical `visits` table, so widening
                // `urls.first_visit_ms` / `urls.last_visit_ms` from it
                // would leave the URL claiming bounds that no visit row
                // proves — breaking any read model that joins them back.
                track_url_visit_bounds(&mut self.progress.url_bounds, url_id, &visit);
            }
            self.progress.visit_count += 1;
            self.progress.last_visit_id = self.progress.last_visit_id.max(visit.source_visit_id);
        }
        if let Some(report_progress) = self.report_progress.as_mut() {
            report_progress(ArchiveIngestProgress {
                processed_records: self.progress.visit_count + self.progress.skipped_visits,
                imported_records: self.progress.new_visits,
                duplicate_records: self
                    .progress
                    .visit_count
                    .saturating_sub(self.progress.new_visits),
                skipped_records: self.progress.skipped_visits,
            });
        }
        Ok(())
    }

    fn downloads(&mut self, batch: Vec<ParsedDownload>) -> Result<(), Self::Error> {
        for download in batch {
            let payload = serialize_payload(&download)?;
            let inserted = insert_download(
                self.archive,
                self.run_id,
                self.source_profile_id,
                &download,
                &payload.hash,
            )?;
            if inserted > 0 {
                self.progress.new_downloads += 1;
            }
            self.progress.download_count += 1;
            self.progress.last_download_id = Some(
                self.progress.last_download_id.unwrap_or_default().max(download.source_download_id),
            );
        }
        Ok(())
    }

    fn search_terms(&mut self, batch: Vec<ParsedSearchTerm>) -> Result<(), Self::Error> {
        for term in batch {
            let Some(&url_id) = self.progress.url_id_map.get(&term.url_id) else {
                continue;
            };
            self.progress.inserted_search_terms += insert_search_term(
                self.archive,
                self.run_id,
                self.source_profile_id,
                &self.profile.profile_id,
                url_id,
                &term,
            )?;
            self.progress.search_term_count += 1;
        }
        Ok(())
    }

    fn favicons(&mut self, batch: Vec<ParsedFavicon>) -> Result<(), Self::Error> {
        for favicon in batch {
            let payload = serialize_payload(&favicon)?;
            insert_favicon(
                self.archive,
                self.run_id,
                self.source_profile_id,
                &favicon,
                &payload.hash,
            )?;
            let favicon_marker = favicon_last_updated_marker(self.profile, &favicon);
            self.progress.last_favicon_marker =
                Some(self.progress.last_favicon_marker.unwrap_or_default().max(favicon_marker));
        }
        Ok(())
    }
}

/// Keeps only the selected profiles that are currently readable on this host.
pub(super) fn select_supported_profiles<'a>(
    discovered: &'a [crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<&'a crate::models::BrowserProfile> {
    discovered
        .iter()
        .filter(|profile| profile.history_exists)
        .filter(|profile| profile.history_readable)
        .filter(|profile| {
            selected_profile_ids.iter().any(|selected| selected == &profile.profile_id)
        })
        .collect()
}

/// Explains why selected profiles were skipped before staging or ingest began.
pub(super) fn collect_skipped_profiles(
    discovered: &[crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<String> {
    let mut warnings = discovered
        .iter()
        .filter(|profile| !profile.history_exists || !profile.history_readable)
        .filter(|profile| {
            selected_profile_ids
                .iter()
                .any(|selected| selected == &profile.profile_id)
        })
        .map(|profile| {
            if profile.browser_family == "safari" {
                format!(
                    "Skipped `{}` because Safari History.db is not readable yet. On macOS, grant Full Disk Access before the next backup.",
                    profile.profile_id
                )
            } else {
                format!(
                    "Skipped `{}` because {} is missing or unreadable at {}.",
                    profile.profile_id, profile.history_file_name, profile.profile_path
                )
            }
        })
        .collect::<Vec<_>>();

    for selected_profile_id in selected_profile_ids {
        if !discovered.iter().any(|profile| profile.profile_id == *selected_profile_id) {
            warnings.push(format!(
                "Skipped `{selected_profile_id}` because it is no longer detected on this device."
            ));
        }
    }

    warnings
}

fn url_last_visit_marker(profile: &crate::models::BrowserProfile, url: &ParsedUrl) -> i64 {
    if profile.browser_family == "chromium" {
        unix_micros_to_chrome_time(url.last_visit_ms.saturating_mul(1_000))
    } else {
        url.last_visit_ms
    }
}

fn favicon_last_updated_marker(
    profile: &crate::models::BrowserProfile,
    favicon: &ParsedFavicon,
) -> i64 {
    if profile.browser_family == "chromium" {
        unix_micros_to_chrome_time(favicon.last_updated_ms.saturating_mul(1_000))
    } else {
        favicon.last_updated_ms
    }
}

#[allow(clippy::too_many_arguments)]
fn process_streamed_profile_snapshot(
    archive: &Transaction<'_>,
    run_id: i64,
    paths: &ProjectPaths,
    config: &AppConfig,
    snapshot: &ProfileSnapshot,
    source_profile_id: i64,
    schema_hash: &str,
    watermark: &Watermark,
    snapshot_artifacts: &mut Vec<SnapshotArtifact>,
    source_evidence_plans: &mut Vec<SourceEvidencePlan>,
    allow_checkpoint: bool,
    streamed: StreamedHistory,
    progress: ArchiveStreamProgress,
) -> Result<BackupProfileSummary> {
    let mut summary = BackupProfileSummary {
        profile_id: snapshot.profile.profile_id.clone(),
        notes: streamed.warnings.iter().map(|warning| warning.message.clone()).collect(),
        new_urls: progress.new_urls,
        new_visits: progress.new_visits,
        new_downloads: progress.new_downloads,
        ..BackupProfileSummary::default()
    };

    if progress.inserted_search_terms > 0 {
        summary.notes.push(format!(
            "Captured {} {} search term rows.",
            progress.inserted_search_terms, snapshot.profile.browser_name
        ));
    }

    if allow_checkpoint && should_checkpoint(watermark, schema_hash, config.checkpoint_days) {
        let artifact = super::create_snapshot_artifact(
            archive,
            run_id,
            paths,
            snapshot,
            if watermark.last_schema_hash.as_deref() != Some(schema_hash) {
                "source-schema-changed"
            } else {
                "periodic-checkpoint"
            },
        )?;
        snapshot_artifacts.push(artifact);
        summary.checkpoint_created = true;
    }

    let source_evidence_payload = SourceEvidencePayload {
        typed_evidence: streamed.typed_evidence,
        native_entities: streamed.native_entities,
    };
    let coverage_stats_json = coverage_stats_json_from_parts(
        progress.url_count,
        progress.visit_count,
        progress.download_count,
        progress.search_term_count,
        &source_evidence_payload,
    );
    let deferred_source_evidence_payload = defer_source_evidence_payload(
        paths,
        &snapshot.profile.profile_id,
        source_evidence_payload,
    )?;
    source_evidence_plans.push(SourceEvidencePlan {
        profile_id: snapshot.profile.profile_id.clone(),
        source_profile_id,
        source_batch: SourceBatchInput {
            source_profile_id,
            run_id: Some(run_id),
            source_kind: "local_db".to_string(),
            browser_version: snapshot.profile.browser_version.clone(),
            schema_version_text: None,
            schema_version_int: None,
            schema_fingerprint: schema_hash.to_string(),
            capability_snapshot: streamed.capability_snapshot,
            coverage_stats_json,
            artifact_refs_json: Some(
                json!({
                    "historyPath": snapshot.history_path.display().to_string(),
                    "faviconsPath": snapshot.favicons_path.as_ref().map(|path| path.display().to_string()),
                    "sourceHashes": snapshot_source_hashes(snapshot),
                })
                .to_string(),
            ),
            notes_json: Some(serde_json::to_string(&streamed.warnings)?),
        },
        schema_observation: streamed.schema_observation,
        source_evidence_payload: deferred_source_evidence_payload,
    });

    save_watermark(
        archive,
        &snapshot.profile.profile_id,
        &Watermark {
            last_visit_id: progress.last_visit_id.max(watermark.last_visit_id),
            last_url_last_visit_time: progress
                .last_url_marker
                .unwrap_or(watermark.last_url_last_visit_time)
                .max(watermark.last_url_last_visit_time),
            last_download_id: progress
                .last_download_id
                .unwrap_or(watermark.last_download_id)
                .max(watermark.last_download_id),
            last_favicon_last_updated: progress
                .last_favicon_marker
                .unwrap_or(watermark.last_favicon_last_updated)
                .max(watermark.last_favicon_last_updated),
            last_checkpoint_at: if summary.checkpoint_created {
                Some(now_rfc3339())
            } else {
                watermark.last_checkpoint_at.clone()
            },
            last_schema_hash: Some(schema_hash.to_string()),
            last_source_batch_id: watermark.last_source_batch_id,
            updated_at: now_rfc3339(),
        },
    )?;

    Ok(summary)
}

/// Ingests one staged profile snapshot into canonical archive rows and deferred evidence plans.
#[allow(clippy::too_many_arguments)]
pub(super) fn process_profile_snapshot(
    archive: &Transaction<'_>,
    run_id: i64,
    paths: &ProjectPaths,
    config: &AppConfig,
    snapshot: &ProfileSnapshot,
    snapshot_artifacts: &mut Vec<SnapshotArtifact>,
    source_evidence_plans: &mut Vec<SourceEvidencePlan>,
    allow_checkpoint: bool,
    use_watermark: bool,
) -> Result<BackupProfileSummary> {
    process_profile_snapshot_with_progress(
        archive,
        run_id,
        paths,
        config,
        snapshot,
        snapshot_artifacts,
        source_evidence_plans,
        allow_checkpoint,
        use_watermark,
        None,
    )
}

/// Ingests one staged profile snapshot and reports parser-batch record progress to the caller.
#[allow(clippy::too_many_arguments)]
pub(super) fn process_profile_snapshot_with_progress(
    archive: &Transaction<'_>,
    run_id: i64,
    paths: &ProjectPaths,
    config: &AppConfig,
    snapshot: &ProfileSnapshot,
    snapshot_artifacts: &mut Vec<SnapshotArtifact>,
    source_evidence_plans: &mut Vec<SourceEvidencePlan>,
    allow_checkpoint: bool,
    use_watermark: bool,
    report_progress: Option<Box<dyn FnMut(ArchiveIngestProgress) + '_>>,
) -> Result<BackupProfileSummary> {
    let source_profile_id = upsert_source_profile(archive, &snapshot.profile)?;
    let schema_payload = collect_schema_payload(&snapshot.history_path)?;
    let schema_string = serde_json::to_string(&schema_payload)?;
    let schema_hash = sha256_hex(schema_string.as_bytes());
    let watermark = if use_watermark {
        load_watermark(archive, &snapshot.profile.profile_id)?
    } else {
        Watermark::default()
    };
    match snapshot.profile.browser_family.as_str() {
        "chromium" | "firefox" | "safari" => {
            let mut consumer = ArchiveChunkConsumer::new(
                archive,
                run_id,
                source_profile_id,
                &snapshot.profile,
                report_progress,
            );
            let browser_family = snapshot.profile.browser_family.as_str();
            let streamed = if browser_family == "chromium" {
                chromium::stream_history(
                    &chromium_history_database_set(snapshot, config.capture_favicons),
                    ChromiumReadCursor {
                        after_visit_id: watermark.last_visit_id,
                        after_url_last_visit_time: watermark.last_url_last_visit_time,
                        after_download_id: watermark.last_download_id,
                        after_favicon_last_updated: watermark.last_favicon_last_updated,
                    },
                    ARCHIVE_STREAM_CHUNK_SIZE,
                    &mut consumer,
                )
            } else if browser_family == "firefox" {
                firefox::stream_history(
                    &snapshot.history_path,
                    watermark.last_visit_id,
                    watermark.last_url_last_visit_time,
                    ARCHIVE_STREAM_CHUNK_SIZE,
                    &mut consumer,
                )
            } else {
                safari::stream_history(
                    &snapshot.history_path,
                    watermark.last_visit_id,
                    watermark.last_url_last_visit_time,
                    ARCHIVE_STREAM_CHUNK_SIZE,
                    &mut consumer,
                )
            }
            .map_err(stream_history_error_to_anyhow)
            .with_context(|| format!("parsing {} staging copy", snapshot.profile.browser_name))?;
            let progress = consumer.finish()?;
            process_streamed_profile_snapshot(
                archive,
                run_id,
                paths,
                config,
                snapshot,
                source_profile_id,
                &schema_hash,
                &watermark,
                snapshot_artifacts,
                source_evidence_plans,
                allow_checkpoint,
                streamed,
                progress,
            )
        }
        family => anyhow::bail!("browser family `{family}` is not supported by the archive engine"),
    }
}

/// Persists deferred source-evidence plans only after canonical archive writes have committed.
pub(super) fn persist_source_evidence_plans(
    source_evidence: &mut Connection,
    archive: &Connection,
    plans: &[SourceEvidencePlan],
) -> Result<()> {
    let transaction = source_evidence.transaction()?;
    let mut committed_batch_ids = Vec::new();
    for plan in plans {
        let source_batch_id = upsert_source_batch(&transaction, &plan.source_batch)?;
        record_schema_observation(
            &transaction,
            source_batch_id,
            "primary-source",
            &plan.schema_observation,
        )?;
        plan.source_evidence_payload.persist(
            &transaction,
            source_batch_id,
            plan.source_profile_id,
        )?;
        committed_batch_ids.push((plan.profile_id.clone(), source_batch_id));
    }
    transaction.commit()?;
    for (profile_id, source_batch_id) in committed_batch_ids {
        archive.execute(
            "UPDATE profile_watermarks
             SET last_source_batch_id = ?1,
                 updated_at = ?2
             WHERE profile_id = ?3",
            params![source_batch_id, now_rfc3339(), profile_id],
        )?;
    }
    Ok(())
}

fn chromium_history_database_set(
    snapshot: &ProfileSnapshot,
    capture_favicons: bool,
) -> HistoryDatabaseSet {
    HistoryDatabaseSet {
        history_path: snapshot.history_path.clone(),
        favicons_path: if capture_favicons { snapshot.favicons_path.clone() } else { None },
    }
}

fn stream_history_error_to_anyhow(error: StreamHistoryError<anyhow::Error>) -> anyhow::Error {
    match error {
        StreamHistoryError::Parse(error) => anyhow::Error::new(error),
        StreamHistoryError::Consumer(error) => error,
    }
}

/// Converts staged source-file fingerprints into the manifest-friendly map shape.
pub(super) fn snapshot_source_hashes(snapshot: &ProfileSnapshot) -> BTreeMap<String, String> {
    snapshot
        .source_hashes
        .iter()
        .map(|fingerprint| (fingerprint.path.clone(), fingerprint.sha256.clone()))
        .collect()
}
#[cfg(test)]
mod core_tests;
