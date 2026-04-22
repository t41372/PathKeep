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

use self::{
    parser::{
        ParsedProfileSnapshot, Watermark, load_watermark, parse_profile_snapshot, save_watermark,
        should_checkpoint,
    },
    writes::{
        UrlVisitBounds, canonical_url_exists, insert_download, insert_favicon, insert_search_term,
        insert_visit, sync_url_bounds, track_url_visit_bounds, upsert_source_profile, upsert_url,
    },
};
use super::*;
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
    source_evidence_payload: SourceEvidencePayload,
}

/// Keeps only the selected profiles that are currently readable on this host.
pub(super) fn select_supported_profiles<'a>(
    discovered: &'a [crate::models::BrowserProfile],
    selected_profile_ids: &[String],
) -> Vec<&'a crate::models::BrowserProfile> {
    discovered
        .iter()
        .filter(|profile| profile.history_exists)
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
        .filter(|profile| !profile.history_exists)
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
    let source_profile_id = upsert_source_profile(archive, &snapshot.profile)?;
    let schema_payload = collect_schema_payload(&snapshot.history_path)?;
    let schema_string = serde_json::to_string(&schema_payload)?;
    let schema_hash = sha256_hex(schema_string.as_bytes());
    let watermark = if use_watermark {
        load_watermark(archive, &snapshot.profile.profile_id)?
    } else {
        Watermark::default()
    };
    let parsed_snapshot = parse_profile_snapshot(snapshot, config, &watermark)
        .with_context(|| format!("parsing {} staging copy", snapshot.profile.browser_name))?;
    let ParsedProfileSnapshot {
        mut history,
        last_visit_id,
        last_url_marker,
        last_download_id,
        last_favicon_marker,
    } = parsed_snapshot;

    let mut summary = BackupProfileSummary {
        profile_id: snapshot.profile.profile_id.clone(),
        notes: history.warnings.iter().map(|warning| warning.message.clone()).collect(),
        ..BackupProfileSummary::default()
    };

    let mut url_id_map = HashMap::new();
    for url in &history.urls {
        let payload = serialize_payload(url)?;
        let existing_url = canonical_url_exists(archive, source_profile_id, url.source_url_id)?;
        let canonical_url_id =
            upsert_url(archive, run_id, source_profile_id, &snapshot.profile, url, &payload.hash)?;
        url_id_map.insert(url.source_url_id, canonical_url_id);
        if !existing_url {
            summary.new_urls += 1;
        }
    }

    let mut url_bounds = HashMap::<i64, UrlVisitBounds>::new();
    for visit in &history.visits {
        let Some(&url_id) = url_id_map.get(&visit.source_url_id) else {
            continue;
        };
        let payload = serialize_payload(visit)?;
        let inserted = insert_visit(
            archive,
            run_id,
            source_profile_id,
            &snapshot.profile.profile_id,
            url_id,
            visit,
            &payload.hash,
        )?;
        if inserted > 0 {
            summary.new_visits += 1;
        }
        track_url_visit_bounds(&mut url_bounds, url_id, visit);
    }

    for (url_id, bounds) in url_bounds {
        sync_url_bounds(archive, url_id, &bounds)?;
    }

    for download in &history.downloads {
        let payload = serialize_payload(download)?;
        let inserted =
            insert_download(archive, run_id, source_profile_id, download, &payload.hash)?;
        if inserted > 0 {
            summary.new_downloads += 1;
        }
    }

    let mut inserted_search_terms = 0usize;
    for term in &history.search_terms {
        let Some(&url_id) = url_id_map.get(&term.url_id) else {
            continue;
        };
        inserted_search_terms += insert_search_term(
            archive,
            run_id,
            source_profile_id,
            &snapshot.profile.profile_id,
            url_id,
            term,
        )?;
    }
    if inserted_search_terms > 0 {
        summary.notes.push(format!(
            "Captured {inserted_search_terms} {} search term rows.",
            snapshot.profile.browser_name
        ));
    }

    for favicon in &history.favicons {
        let payload = serialize_payload(favicon)?;
        insert_favicon(archive, run_id, source_profile_id, favicon, &payload.hash)?;
    }

    if allow_checkpoint && should_checkpoint(&watermark, &schema_hash, config.checkpoint_days) {
        let artifact = super::create_snapshot_artifact(
            archive,
            run_id,
            paths,
            snapshot,
            if watermark.last_schema_hash.as_deref() != Some(&schema_hash) {
                "source-schema-changed"
            } else {
                "periodic-checkpoint"
            },
        )?;
        snapshot_artifacts.push(artifact);
        summary.checkpoint_created = true;
    }

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
            schema_fingerprint: schema_hash.clone(),
            capability_snapshot: history.capability_snapshot.clone(),
            coverage_stats_json: coverage_stats_json(&history),
            artifact_refs_json: Some(
                json!({
                    "historyPath": snapshot.history_path.display().to_string(),
                    "faviconsPath": snapshot.favicons_path.as_ref().map(|path| path.display().to_string()),
                    "sourceHashes": snapshot_source_hashes(snapshot),
                })
                .to_string(),
            ),
            notes_json: Some(serde_json::to_string(&history.warnings)?),
        },
        schema_observation: history.schema_observation.clone(),
        source_evidence_payload: take_source_evidence_payload(&mut history),
    });

    save_watermark(
        archive,
        &snapshot.profile.profile_id,
        &Watermark {
            last_visit_id: last_visit_id.max(watermark.last_visit_id),
            last_url_last_visit_time: last_url_marker
                .unwrap_or(watermark.last_url_last_visit_time)
                .max(watermark.last_url_last_visit_time),
            last_download_id: last_download_id
                .unwrap_or(watermark.last_download_id)
                .max(watermark.last_download_id),
            last_favicon_last_updated: last_favicon_marker
                .unwrap_or(watermark.last_favicon_last_updated)
                .max(watermark.last_favicon_last_updated),
            last_checkpoint_at: if summary.checkpoint_created {
                Some(now_rfc3339())
            } else {
                watermark.last_checkpoint_at.clone()
            },
            last_schema_hash: Some(schema_hash),
            last_source_batch_id: watermark.last_source_batch_id,
            updated_at: now_rfc3339(),
        },
    )?;

    Ok(summary)
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
        persist_source_evidence(
            &transaction,
            source_batch_id,
            plan.source_profile_id,
            &plan.source_evidence_payload,
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

/// Converts staged source-file fingerprints into the manifest-friendly map shape.
pub(super) fn snapshot_source_hashes(snapshot: &ProfileSnapshot) -> BTreeMap<String, String> {
    snapshot
        .source_hashes
        .iter()
        .map(|fingerprint| (fingerprint.path.clone(), fingerprint.sha256.clone()))
        .collect()
}
