//! Responsibilities: stream staged browser rows through bounded Turso writes.
//! Not responsible for: source-file staging, run lifecycle or index consumption.
//! Failure modes: duplicate identities, cursor/data split commits, missing cold
//! evidence, archive-sized memory, checkpoint provenance loss, interrupted batches,
//! a renumbered browser file read from an id it no longer reaches.
//! Tests: concurrency_tests, rowid_resume_tests, dedup_scenarios* and backup-integrity.spec.

mod consumer;
pub(crate) mod identity;
pub(crate) mod parser;
mod read_plan;
mod selection;
pub(crate) mod writes;

use super::*;
use crate::archive::source_error::SourceError;
use browser_history_parser::{
    ChromiumReadCursor, HistoryDatabaseSet, StreamHistoryError, chromium, firefox, safari,
};
pub(crate) use consumer::{
    ArchiveChunkConsumer, ArchiveIngestProgress, ArchiveStreamProgress, BATCH_ROWS, ImportOrigin,
    WriteContext,
};
use parser::{Watermark, load_watermark, save_watermark, should_checkpoint};
use read_plan::{ReadPlan, Why};
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
    let (source_profile_id, mut watermark) =
        vault.write(Db::Archive, Priority::Batch, move |s| {
            let id = writes::upsert_source_profile(s, &profile)?;
            let cursor = if use_watermark {
                load_watermark(s, &profile.profile_id)?
            } else {
                Watermark::default()
            };
            Ok((id, cursor))
        })?;
    let table = read_plan::table(&snapshot.profile.browser_family);
    let plan = match table {
        Some(table) if use_watermark => {
            read_plan::plan(&snapshot.history_path, table, &watermark, chrono::Utc::now())
                .map_err(|error| error.context(SourceError::Failed("source-read-failed")))?
        }
        _ => ReadPlan::Everything(Why::First),
    };
    if use_watermark && plan.starts_full_read() {
        let (profile_key, started) = (snapshot.profile.profile_id.clone(), now_rfc3339());
        let at = started.clone();
        vault.write(Db::Archive, Priority::Batch, move |s| {
            Ok(parser::begin_full_read(s, &profile_key, &at)?)
        })?;
        watermark = Watermark {
            last_visit_id: 0,
            last_visit_key: None,
            last_visit_url: None,
            last_url_last_visit_time: 0,
            last_search_url_time: 0,
            url_scan: None,
            full_scan_started_at: Some(started),
            newest_visit_ms: 0,
            ..watermark
        };
    }
    let window = browser_history_parser::VisitWindow {
        after_id: if plan == ReadPlan::Continue { 0 } else { watermark.last_visit_id },
        recent_since_ms: plan.recent_since_ms(),
    };
    let schema = collect_schema_payload(&snapshot.history_path)
        .map_err(|error| error.context(SourceError::Failed("source-read-failed")))?;
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
                after_visit_id: window.after_id,
                recent_visits_since_ms: window.recent_since_ms,
                after_url_last_visit_time: watermark.last_url_last_visit_time,
                // Downloads are few and their ids come back (max+1): all of them, every time.
                after_download_id: 0,
                after_favicon_last_updated: watermark.last_favicon_last_updated,
            },
            BATCH_ROWS,
            &mut consumer,
        ),
        "firefox" => firefox::stream_history_window(
            &snapshot.history_path,
            window,
            watermark.last_url_last_visit_time,
            watermark.url_scan,
            BATCH_ROWS,
            &mut consumer,
        ),
        "safari" => safari::stream_history_window(
            &snapshot.history_path,
            window,
            watermark.last_url_last_visit_time,
            watermark.url_scan,
            BATCH_ROWS,
            &mut consumer,
        ),
        family => anyhow::bail!("browser family `{family}` is not supported by the archive engine"),
    }
    .map_err(stream_history_error_to_anyhow);
    let streamed = match streamed {
        Ok(streamed) => streamed,
        Err(error) => {
            cancel.check()?;
            // The file could not be read to the end: what was read before is saved, and this
            // profile says so; the run goes on to the others.
            let Some(source) = error.downcast_ref::<SourceError>() else { return Err(error) };
            let outcome = source.outcome();
            let code = outcome.code().expect("source errors have a code").to_owned();
            let progress = consumer.interrupted()?;
            let warning = crate::models::BackupWarning::new(&code, &code)
                .with_profile_id(snapshot.profile.profile_id.clone())
                .with_diagnostic(format!("{error:#}"));
            return profile_summary(vault, snapshot, &progress, outcome, false, vec![warning]);
        }
    };
    let mut progress = consumer.finish()?;
    if let Some(table) = table {
        let unresolved = std::mem::take(&mut progress.unresolved_parents);
        name_openers(vault, source_profile_id, &snapshot.history_path, table, unresolved)
            .map_err(|error| error.context(SourceError::Failed("source-read-failed")))?;
    }
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
    if let Some(table) = table {
        let orphans = browser_history_parser::native_visits::orphan_visits(
            &snapshot.history_path,
            table,
            window.after_id,
            window.recent_since_ms,
        )
        .map_err(|error| {
            anyhow::Error::new(error).context(SourceError::Failed("source-read-failed"))
        })?;
        if orphans > 0 {
            // Rows the browser itself cannot show: their page row is gone, so there is no
            // address to save them under.
            warnings.push(
                crate::models::BackupWarning::new(
                    "source-visits-without-page",
                    format!(
                        "{orphans} {} history entries have no page in the browser's file and were not saved.",
                        snapshot.profile.browser_name
                    ),
                )
                .with_count(orphans)
                .with_profile_id(snapshot.profile.profile_id.clone()),
            );
        }
    }
    if plan == ReadPlan::Everything(Why::Renumbered) {
        warnings.push(
            crate::models::BackupWarning::new(
                "source-ids-reset",
                "The browser’s stored history changed. All available entries were checked again.",
            )
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
    let read_everything = use_watermark && plan.completes_full_read();
    vault.write(Db::Archive, Priority::Batch, move |s| {
        let mut cursor = load_watermark(s, &profile_key)?;
        cursor.last_schema_hash = Some(hash);
        if checkpoint {
            cursor.last_checkpoint_at = Some(now_rfc3339());
        }
        cursor.last_source_batch_id = Some(source_batch_id);
        save_watermark(s, &profile_key, &cursor)?;
        if read_everything {
            parser::finish_full_read(s, &profile_key, &now_rfc3339())?;
        }
        Ok(())
    })?;
    profile_summary(
        vault,
        snapshot,
        &progress,
        crate::models::ProfileOutcome::Complete,
        checkpoint,
        warnings,
    )
}

/// [`name_openers`] for a browser file read outside a backup (a browser history file the user
/// imports).
pub(crate) fn name_openers_in(
    vault: &crate::Vault,
    profile: i64,
    path: &std::path::Path,
    family: &str,
    unresolved: Vec<(i64, i64)>,
) -> Result<()> {
    match read_plan::table(family) {
        Some(table) => name_openers(vault, profile, path, table, unresolved),
        None => Ok(()),
    }
}

/// Names the openers a read did not reach, from the browser's own record of them: an opener
/// written after its child (Sync writes older visits after newer ones) or before the rows read.
fn name_openers(
    vault: &crate::Vault,
    profile: i64,
    path: &std::path::Path,
    table: browser_history_parser::native_visits::VisitTable,
    unresolved: Vec<(i64, i64)>,
) -> Result<()> {
    if unresolved.is_empty() {
        return Ok(());
    }
    let openers = read_plan::openers(path, table, &unresolved)?;
    let links = unresolved
        .into_iter()
        .filter_map(|(child, opener)| openers.get(&opener).map(|mark| (child, mark.clone())))
        .collect::<Vec<_>>();
    for chunk in links.chunks(BATCH_ROWS) {
        let chunk = chunk.to_vec();
        vault.write(Db::Archive, Priority::Batch, move |s| {
            for (child, mark) in chunk {
                if let Some(parent) = identity::archived(s, profile, &mark)?
                    && parent != child
                {
                    writes::set_parent(s, child, parent)?;
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn profile_summary(
    vault: &crate::Vault,
    snapshot: &ProfileSnapshot,
    progress: &ArchiveStreamProgress,
    outcome: crate::models::ProfileOutcome,
    checkpoint: bool,
    warnings: Vec<crate::models::BackupWarning>,
) -> Result<BackupProfileSummary> {
    let replaced = progress.replaced_profiles.iter().copied().collect::<Vec<_>>();
    let replaced_profiles = vault
        .read(Db::Archive, move |s| Ok(crate::archive::same_visit::profile_keys(s, &replaced)?))?;
    Ok(BackupProfileSummary {
        outcome,
        profile_id: snapshot.profile.profile_id.clone(),
        new_urls: progress.new_urls,
        new_visits: progress.new_visits,
        same_visits: progress.same_visits,
        replaced_profiles,
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
        StreamHistoryError::Parse(e) => {
            anyhow::Error::new(e).context(SourceError::Failed("source-stream-failed"))
        }
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
mod dedup_scenarios;
#[cfg(test)]
mod dedup_scenarios_baselines;
#[cfg(test)]
mod dedup_scenarios_edge_cases;
#[cfg(test)]
mod dedup_scenarios_takeout;
#[cfg(test)]
mod import_equivalence;

#[cfg(test)]
mod rowid_resume_tests;
#[cfg(test)]
mod scale_measure;
#[cfg(test)]
mod test_context;
