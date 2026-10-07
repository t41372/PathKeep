//! Responsibilities: stream staged browser rows through bounded Turso writes.
//! Not responsible for: source-file staging, run lifecycle or index consumption.
//! Failure modes: duplicate identities, cursor/data split commits, missing cold
//! evidence, archive-sized memory, checkpoint provenance loss, interrupted batches.
//! Tests: core_tests, concurrency_tests and dedup_scenarios*.

mod consumer;
pub(crate) mod parser;
mod selection;
pub(crate) mod writes;

use super::*;
use browser_history_parser::{
    ChromiumReadCursor, HistoryDatabaseSet, StreamHistoryError, chromium, firefox, safari,
};
pub(crate) use consumer::{
    ArchiveChunkConsumer, ArchiveIngestProgress, ArchiveStreamProgress, BATCH_ROWS, ImportOrigin,
    WriteContext,
};
use parser::{Watermark, load_watermark, save_watermark, should_checkpoint};
pub(super) use selection::{collect_skipped_profiles, select_supported_profiles};
use vault_store::{CancelToken, Db, Priority};

/// Preview reads only the browser-owned staging file through rusqlite.
pub(super) fn preview_snapshot_counts(snapshot: &ProfileSnapshot) -> Result<(usize, usize, usize)> {
    parser::preview_snapshot_counts(snapshot)
}

/// Commits parser batches independently, keeping cold evidence ahead of watermarks.
pub(super) fn process_profile_snapshot(
    vault: &crate::Vault,
    run_id: i64,
    snapshot: &ProfileSnapshot,
    allow_checkpoint: bool,
    use_watermark: bool,
    cancel: &CancelToken,
    progress: Option<Box<dyn FnMut(ArchiveIngestProgress) + '_>>,
) -> Result<BackupProfileSummary> {
    let profile = snapshot.profile.clone();
    let (source_profile_id, watermark) = vault.write(Db::Archive, Priority::Batch, move |s| {
        let id = writes::upsert_source_profile(s, &profile)?;
        let cursor = if use_watermark {
            load_watermark(s, &profile.profile_id)?
        } else {
            Watermark::default()
        };
        Ok((id, cursor))
    })?;
    let schema = collect_schema_payload(&snapshot.history_path)?;
    let hash = sha256_hex(serde_json::to_string(&schema)?.as_bytes());
    let source_batch_id = start_source_batch(vault, run_id, source_profile_id, snapshot, &hash)?;
    let context = WriteContext {
        run_id,
        source_profile_id,
        profile_id: snapshot.profile.profile_id.clone(),
        browser_family: snapshot.profile.browser_family.clone(),
        import: None,
    };
    let mut consumer = ArchiveChunkConsumer::new(
        vault,
        context,
        watermark.clone(),
        cancel,
        progress,
        source_batch_id,
    );
    let streamed = match snapshot.profile.browser_family.as_str() {
        "chromium" => chromium::stream_history(
            &chromium_history_database_set(snapshot, vault.config().capture_favicons),
            ChromiumReadCursor {
                after_search_url_time: watermark.last_search_url_time,
                url_scan: watermark.url_scan,
                after_visit_id: watermark.last_visit_id,
                after_url_last_visit_time: watermark.last_url_last_visit_time,
                after_download_id: watermark.last_download_id,
                after_favicon_last_updated: watermark.last_favicon_last_updated,
            },
            BATCH_ROWS,
            &mut consumer,
        ),
        "firefox" => firefox::stream_history(
            &snapshot.history_path,
            watermark.last_visit_id,
            watermark.last_url_last_visit_time,
            watermark.url_scan,
            BATCH_ROWS,
            &mut consumer,
        ),
        "safari" => safari::stream_history(
            &snapshot.history_path,
            watermark.last_visit_id,
            watermark.last_url_last_visit_time,
            watermark.url_scan,
            BATCH_ROWS,
            &mut consumer,
        ),
        family => anyhow::bail!("browser family `{family}` is not supported by the archive engine"),
    }
    .map_err(stream_history_error_to_anyhow)?;
    let progress = consumer.finish()?;
    let mut warnings = streamed
        .warnings
        .iter()
        .map(|w| {
            crate::models::BackupWarning::new(&w.code, w.message.clone())
                .with_profile_id(snapshot.profile.profile_id.clone())
        })
        .collect::<Vec<_>>();
    if progress.inserted_search_terms > 0 {
        warnings.push(
            crate::models::BackupWarning::new(
                "profile-search-terms-captured",
                format!(
                    "Captured {} {} search term rows.",
                    progress.inserted_search_terms, snapshot.profile.browser_name
                ),
            )
            .with_count(progress.inserted_search_terms)
            .with_profile_id(snapshot.profile.profile_id.clone()),
        );
    }
    finish_source_batch(vault, source_batch_id, streamed, &progress)?;
    let checkpoint =
        allow_checkpoint && should_checkpoint(&watermark, &hash, vault.config().checkpoint_days);
    if checkpoint {
        create_checkpoint(vault, run_id, snapshot, &hash, &watermark)?;
    }
    let profile_key = snapshot.profile.profile_id.clone();
    vault.write(Db::Archive, Priority::Batch, move |s| {
        let mut cursor = load_watermark(s, &profile_key)?;
        cursor.last_schema_hash = Some(hash);
        if checkpoint {
            cursor.last_checkpoint_at = Some(now_rfc3339());
        }
        cursor.last_source_batch_id = Some(source_batch_id);
        save_watermark(s, &profile_key, &cursor)?;
        Ok(())
    })?;
    Ok(BackupProfileSummary {
        profile_id: snapshot.profile.profile_id.clone(),
        new_urls: progress.new_urls,
        new_visits: progress.new_visits,
        new_downloads: progress.new_downloads,
        checkpoint_created: checkpoint,
        notes: backup_warning_messages(&warnings),
        note_details: warnings,
    })
}

fn start_source_batch(
    vault: &crate::Vault,
    run_id: i64,
    source_profile_id: i64,
    snapshot: &ProfileSnapshot,
    hash: &str,
) -> Result<i64> {
    let input = SourceBatchInput {
        source_profile_id,
        run_id: Some(run_id),
        source_kind: "local_db".into(),
        browser_version: snapshot.profile.browser_version.clone(),
        schema_version_text: None,
        schema_version_int: None,
        schema_fingerprint: hash.into(),
        capability_snapshot: browser_history_parser::CapabilitySnapshot::default(),
        coverage_stats_json: "{}".into(),
        artifact_refs_json: Some(
            json!({"sourceHashes":snapshot_source_hashes(snapshot)}).to_string(),
        ),
        notes_json: None,
    };
    vault.write(Db::SourceEvidence, Priority::Batch, move |s| upsert_source_batch(s, &input))
}

fn finish_source_batch(
    vault: &crate::Vault,
    id: i64,
    streamed: browser_history_parser::StreamedHistory,
    progress: &ArchiveStreamProgress,
) -> Result<()> {
    let coverage = progress.evidence_counts.coverage(
        progress.url_count,
        progress.visit_count,
        progress.download_count,
        progress.search_term_count,
    );
    vault.write(Db::SourceEvidence,Priority::Batch,move |s| {
        record_schema_observation(s,id,"primary-source",&streamed.schema_observation)?;
        s.execute("UPDATE source_batches SET capability_snapshot_json=?2,coverage_stats_json=?3,notes_json=?4 WHERE id=?1",
            vault_store::params![id,serde_json::to_string(&streamed.capability_snapshot)?,coverage,serde_json::to_string(&streamed.warnings)?])?;
        Ok(())
    })
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
        StreamHistoryError::Parse(e) => e.into(),
        StreamHistoryError::Consumer(e) => e,
    }
}
pub(super) fn snapshot_source_hashes(snapshot: &ProfileSnapshot) -> BTreeMap<String, String> {
    snapshot.source_hashes.iter().map(|h| (h.path.clone(), h.sha256.clone())).collect()
}

mod checkpoint;
use checkpoint::create_checkpoint;

#[cfg(test)]
mod concurrency_tests;
#[cfg(test)]
mod core_tests;
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

#[cfg(test)]
mod rowid_resume_tests;
