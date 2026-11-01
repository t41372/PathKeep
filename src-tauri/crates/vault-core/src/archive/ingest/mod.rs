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
use browser_history_parser::native_visits::VisitMark;
use browser_history_parser::{
    ChromiumReadCursor, HistoryDatabaseSet, StreamHistoryError, chromium, firefox, safari,
};
pub(crate) use consumer::{
    ArchiveChunkConsumer, ArchiveIngestProgress, ArchiveStreamProgress, BATCH_ROWS, ImportOrigin,
    WriteContext,
};
use parser::{Watermark, load_watermark, save_watermark, should_checkpoint};
use read_plan::{ReadPlan, Why};
use std::collections::HashMap;
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
    let first = match table {
        Some(table) if use_watermark => {
            browser_history_parser::native_visits::first_visit(&snapshot.history_path, table)
                .map_err(|error| {
                    anyhow::Error::new(error).context(SourceError::Failed("source-read-failed"))
                })?
        }
        _ => None,
    };
    let plan = match table {
        Some(table) if use_watermark => read_plan::plan(
            &snapshot.history_path,
            table,
            &watermark,
            first.as_ref(),
            chrono::Utc::now(),
        )
        .map_err(|error| error.context(SourceError::Failed("source-read-failed")))?,
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
    // Saved with the first batch: a read of every row cut off before it ends is carried on only
    // in the table it started in.
    if let Some(first) = &first {
        watermark.set_first_visit(first);
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
        captured_at_ms: snapshot.captured_at_ms,
    };
    let consumer = ArchiveChunkConsumer::new(
        vault,
        context,
        watermark.clone(),
        cancel,
        progress,
        source_batch_id,
    );
    // A read after the last row saves the mark it started from until its openers are named. A
    // read of every row saves the row it reached: one cut off is carried on only while that row
    // still holds its visit, and reads its visits from the first again anyway. A checkpoint
    // replay reads an older copy of the file and saves no cursor of it.
    let mut consumer = if !use_watermark {
        consumer.leave_cursors()
    } else if plan.completes_full_read() {
        consumer
    } else {
        consumer.hold_visit_mark()
    };
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
    // The file could not be read to the end: what was read before is saved, and this profile
    // says so; the run goes on to the others. Its visit mark stays where the read started, so
    // the next backup reads those rows again.
    let source_failed = |error: anyhow::Error, progress: &ArchiveStreamProgress| {
        cancel.check()?;
        let Some(source) = error.downcast_ref::<SourceError>() else { return Err(error) };
        let outcome = source.outcome();
        let code = outcome.code().expect("source errors have a code").to_owned();
        end_source_batch_early(vault, source_batch_id, progress, &code, &error)?;
        let warning = crate::models::BackupWarning::new(&code, &code)
            .with_profile_id(snapshot.profile.profile_id.clone())
            .with_diagnostic(format!("{error:#}"));
        profile_summary(vault, snapshot, progress, outcome, false, vec![warning])
    };
    let streamed = match streamed {
        Ok(streamed) => streamed,
        Err(error) => {
            cancel.check()?;
            if error.downcast_ref::<SourceError>().is_none() {
                return Err(error);
            }
            return source_failed(error, &consumer.interrupted()?);
        }
    };
    let mut progress = consumer.finish()?;
    if let Some(table) = table {
        let unresolved = std::mem::take(&mut progress.unresolved_parents);
        // Only the browser file is read here; the archive writes after it fail the run.
        let openers = match opener_marks(&snapshot.history_path, table, &unresolved, cancel) {
            Ok(openers) => openers,
            Err(error) => {
                let error = error.context(SourceError::Failed("source-read-failed"));
                return source_failed(error, &progress);
            }
        };
        let family = snapshot.profile.browser_family.clone();
        name_openers(vault, source_profile_id, &family, unresolved, openers, cancel)?;
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
        let orphans = match browser_history_parser::native_visits::orphan_visits(
            &snapshot.history_path,
            table,
            window.after_id,
            window.recent_since_ms,
        ) {
            Ok(orphans) => orphans,
            Err(error) => {
                let error =
                    anyhow::Error::new(error).context(SourceError::Failed("source-read-failed"));
                return source_failed(error, &progress);
            }
        };
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
    let reached = progress.reached.take();
    vault.write(Db::Archive, Priority::Batch, move |s| {
        let mut cursor = load_watermark(s, &profile_key)?;
        if let Some(reached) = &reached {
            cursor.take_visit_mark(reached);
        }
        if let Some(first) = &first {
            cursor.set_first_visit(first);
        }
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
    cancel: &CancelToken,
) -> Result<()> {
    match read_plan::table(family) {
        Some(table) => {
            let openers = opener_marks(path, table, &unresolved, cancel)?;
            name_openers(vault, profile, family, unresolved, openers, cancel)
        }
        None => Ok(()),
    }
}

/// The openers a read did not reach, from the browser's own record of them: an opener written
/// after its child (Sync writes older visits after newer ones) or before the rows read. Sync
/// writing newest first can leave most of a large file's visits here; a stop is heard between
/// lookups, as between the read's batches.
fn opener_marks(
    path: &std::path::Path,
    table: browser_history_parser::native_visits::VisitTable,
    unresolved: &[(i64, i64)],
    cancel: &CancelToken,
) -> Result<HashMap<i64, VisitMark>> {
    let mut marks = HashMap::new();
    for chunk in unresolved.chunks(BATCH_ROWS) {
        cancel.check()?;
        marks.extend(read_plan::openers(path, table, chunk)?);
    }
    Ok(marks)
}

/// Gives each visit the archived visit its opener is, by the [`opener_marks`] read for them.
fn name_openers(
    vault: &crate::Vault,
    profile: i64,
    family: &str,
    unresolved: Vec<(i64, i64)>,
    openers: HashMap<i64, VisitMark>,
    cancel: &CancelToken,
) -> Result<()> {
    let links = unresolved
        .into_iter()
        .filter_map(|(child, opener)| openers.get(&opener).map(|mark| (child, mark.clone())))
        .collect::<Vec<_>>();
    for chunk in links.chunks(BATCH_ROWS) {
        cancel.check()?;
        let (chunk, family) = (chunk.to_vec(), family.to_owned());
        vault.write(Db::Archive, Priority::Batch, move |s| {
            for (child, mark) in chunk {
                if let Some(parent) = identity::archived(s, profile, &family, &mark)?
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

/// A read that failed part way kept what it saved; its batch says how much and why it ended,
/// instead of the empty coverage it was opened with. What only a whole read knows (the file's
/// capabilities and schema observation) stays unknown.
fn end_source_batch_early(
    vault: &crate::Vault,
    id: i64,
    progress: &ArchiveStreamProgress,
    code: &str,
    error: &anyhow::Error,
) -> Result<()> {
    let coverage = progress.evidence_counts.coverage(
        progress.url_count,
        progress.visit_count,
        progress.download_count,
        progress.search_term_count,
    );
    let notes = serde_json::to_string(&[browser_history_parser::ParserWarning {
        code: code.to_owned(),
        message: format!("The read ended part way: {error:#}"),
    }])?;
    vault.write(Db::SourceEvidence, Priority::Batch, move |s| {
        s.execute(
            "UPDATE source_batches SET coverage_stats_json=?2,notes_json=?3 WHERE id=?1",
            vault_store::params![id, coverage, notes],
        )?;
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
