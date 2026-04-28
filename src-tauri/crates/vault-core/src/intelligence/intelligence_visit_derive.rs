//! Visit-derived stage execution and fallback replay helpers.
//!
//! ## Responsibilities
//! - Run the visit-derived rebuild stage in incremental, fallback-full, or
//!   no-op mode based on profile watermarks and checkpoints.
//! - Stream visible archive visits in bounded batches during fallback replay.
//! - Persist refreshed visit-derived facts and checkpoint metadata for one
//!   profile at a time.
//!
//! ## Not responsible for
//! - Loading generic route-level visit reads.
//! - Computing daily rollups, sessions, trails, or aggregate read models.
//! - Owning the site-dictionary classification logic itself.
//!
//! ## Dependencies
//! - Incremental checkpoint helpers and rebuild mode metadata.
//! - Visit-loading/persistence helpers from `intelligence_visit_records`.
//! - Archive source-profile resolution from `intelligence_rebuild`.
//!
//! ## Performance notes
//! - Incremental execution only reads the post-checkpoint suffix.
//! - Fallback replay uses bounded ordered batches so one profile rebuild does
//!   not materialize all visible visits in memory.

use super::incremental::{
    ProfileSourceWatermark, StageCheckpoint, StageExecutionMode, load_stage_checkpoint,
    save_stage_checkpoint, stage_name, stage_version, watermark_regressed,
};
use super::{
    RebuildMode, StageRunResult, VisibleVisitBatchCursor, VisitRecord,
    clear_core_tables_for_job_kind, compute_is_new_domain_with_seen,
    load_archive_source_profile_id, load_seen_domains, local_date_key, persist_visit_derived_facts,
    unique_date_keys, visit_from_row,
};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::{BTreeSet, HashSet};

/// Runs the visit-derived stage for one profile without reopening any later
/// structural rebuild work.
pub(super) fn execute_visit_derive_stage(
    connection: &Connection,
    profile_id: &str,
    watermark: &ProfileSourceWatermark,
    force_full: bool,
    run_id: i64,
    computed_at: &str,
) -> Result<StageRunResult> {
    let current_version = stage_version(connection, RebuildMode::VisitDerive)?;
    let checkpoint = load_stage_checkpoint(connection, profile_id, RebuildMode::VisitDerive)?;
    if watermark.visible_visit_count == 0 {
        clear_core_tables_for_job_kind(connection, Some(profile_id), RebuildMode::VisitDerive)?;
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::VisitDerive).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: 0,
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!(
                "No visible visits remained for {profile_id}; cleared visit-derived facts."
            )],
            ..StageRunResult::default()
        });
    }

    let mut fallback_reason = if force_full {
        Some("Manual full rebuild requested for visit-derived facts.".to_string())
    } else {
        None
    };
    if !force_full {
        match checkpoint.as_ref() {
            None => fallback_reason =
                Some("No visit-derived checkpoint was recorded for this profile yet.".to_string()),
            Some(checkpoint) if checkpoint.stage_version != current_version => {
                fallback_reason =
                    Some("Visit-derived rules changed since the last successful rebuild.".to_string())
            }
            Some(checkpoint) if watermark_regressed(watermark, &checkpoint.source_watermark) => {
                fallback_reason = Some(
                    "Archive visibility regressed or source counters moved backwards for visit-derived facts."
                        .to_string(),
                )
            }
            _ => {}
        }
    }

    if fallback_reason.is_none()
        && checkpoint.as_ref().is_some_and(|checkpoint| checkpoint.source_watermark == *watermark)
    {
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(RebuildMode::VisitDerive).to_string(),
                stage_version: current_version,
                source_watermark: watermark.clone(),
                last_processed_visit_id: checkpoint
                    .as_ref()
                    .map(|value| value.last_processed_visit_id)
                    .unwrap_or_default(),
                last_run_id: Some(run_id),
                updated_at: computed_at.to_string(),
                ..StageCheckpoint::default()
            },
        )?;
        return Ok(StageRunResult {
            execution_mode: Some(StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: vec![profile_id.to_string()],
            notes: vec![format!("Visit-derived facts for {profile_id} were already up to date.")],
            ..StageRunResult::default()
        });
    }

    let (visits, execution_mode, dirty_visit_count, dirty_date_keys, dirty_from_visit_ms) =
        if let Some(_reason) = fallback_reason.clone() {
            clear_core_tables_for_job_kind(connection, Some(profile_id), RebuildMode::VisitDerive)?;
            let fallback_summary = rebuild_visit_derived_facts_in_batches(
                connection,
                profile_id,
                computed_at,
                super::VISIT_DERIVE_FALLBACK_BATCH_SIZE,
            )?;
            (
                Vec::new(),
                StageExecutionMode::FallbackFull,
                fallback_summary.processed_visits,
                fallback_summary.dirty_date_keys,
                fallback_summary.dirty_from_visit_ms,
            )
        } else {
            let last_processed_visit_id =
                checkpoint.as_ref().map(|value| value.last_processed_visit_id).unwrap_or_default();
            let source_profile_id = load_archive_source_profile_id(connection, profile_id)?;
            let mut visits = load_visible_visits_after_id(
                connection,
                profile_id,
                source_profile_id,
                last_processed_visit_id,
            )?;
            let expected_delta = (watermark.visible_visit_count
                - checkpoint
                    .as_ref()
                    .map(|value| value.source_watermark.visible_visit_count)
                    .unwrap_or_default())
            .max(0) as usize;
            if visits.is_empty() || visits.len() != expected_delta {
                fallback_reason = Some(
                    "Visit-derived delta rows no longer matched the current archive watermark."
                        .to_string(),
                );
                clear_core_tables_for_job_kind(
                    connection,
                    Some(profile_id),
                    RebuildMode::VisitDerive,
                )?;
                let fallback_summary = rebuild_visit_derived_facts_in_batches(
                    connection,
                    profile_id,
                    computed_at,
                    super::VISIT_DERIVE_FALLBACK_BATCH_SIZE,
                )?;
                (
                    Vec::new(),
                    StageExecutionMode::FallbackFull,
                    fallback_summary.processed_visits,
                    fallback_summary.dirty_date_keys,
                    fallback_summary.dirty_from_visit_ms,
                )
            } else {
                let mut seen_domains = load_seen_domains(connection, profile_id)?;
                compute_is_new_domain_with_seen(&mut visits, &mut seen_domains);
                let dirty_date_keys = unique_date_keys(&visits);
                let dirty_from_visit_ms = visits.first().map(|visit| visit.visit_time_ms);
                (
                    visits,
                    StageExecutionMode::Incremental,
                    expected_delta,
                    dirty_date_keys,
                    dirty_from_visit_ms,
                )
            }
        };

    if execution_mode != StageExecutionMode::FallbackFull {
        persist_visit_derived_facts(connection, &visits, computed_at)?;
    }
    save_stage_checkpoint(
        connection,
        &StageCheckpoint {
            profile_id: profile_id.to_string(),
            stage: stage_name(RebuildMode::VisitDerive).to_string(),
            stage_version: current_version,
            source_watermark: watermark.clone(),
            last_processed_visit_id: watermark.max_visit_id,
            dirty_from_visit_ms,
            dirty_date_key: dirty_date_keys.first().cloned(),
            last_run_id: Some(run_id),
            fallback_reason: fallback_reason.clone(),
            updated_at: computed_at.to_string(),
        },
    )?;
    Ok(StageRunResult {
        processed_visits: dirty_visit_count,
        visit_derived_facts: dirty_visit_count,
        execution_mode: Some(execution_mode.as_str().to_string()),
        affected_profiles: vec![profile_id.to_string()],
        dirty_visit_count: Some(dirty_visit_count),
        dirty_date_keys,
        fallback_reason: fallback_reason.clone(),
        notes: vec![if execution_mode == StageExecutionMode::Incremental {
            format!("Incrementally refreshed visit-derived facts for {profile_id}.")
        } else {
            format!("Rebuilt visit-derived facts for {profile_id} with a scoped full refresh.")
        }],
        ..StageRunResult::default()
    })
}

#[derive(Debug, Default)]
struct VisitDeriveFallbackSummary {
    processed_visits: usize,
    dirty_date_keys: Vec<String>,
    dirty_from_visit_ms: Option<i64>,
}

fn load_visible_visits_after_id(
    connection: &Connection,
    profile_id: &str,
    source_profile_id: i64,
    last_processed_visit_id: i64,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                ?1,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visits.reverted_at IS NULL
           AND visits.source_profile_id = ?2
           AND visits.id > ?3
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    let rows = statement.query_map(
        params![profile_id, source_profile_id, last_processed_visit_id],
        visit_from_row,
    )?;
    let mut visits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    super::hydrate_search_terms(connection, &mut visits)?;
    Ok(visits)
}

fn load_visible_visit_batch(
    connection: &Connection,
    profile_id: &str,
    source_profile_id: i64,
    after: Option<VisibleVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                ?1,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visits.reverted_at IS NULL
           AND visits.source_profile_id = ?2
           AND (
             ?3 IS NULL
             OR visits.visit_time_ms > ?3
             OR (visits.visit_time_ms = ?3 AND visits.id > ?4)
           )
         ORDER BY visits.visit_time_ms ASC, visits.id ASC
         LIMIT ?5",
    )?;
    let rows = statement.query_map(
        params![
            profile_id,
            source_profile_id,
            after.map(|cursor| cursor.visit_time_ms),
            after.map(|cursor| cursor.visit_id),
            limit.max(1) as i64,
        ],
        visit_from_row,
    )?;
    let mut visits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    super::hydrate_search_terms(connection, &mut visits)?;
    Ok(visits)
}

fn rebuild_visit_derived_facts_in_batches(
    connection: &Connection,
    profile_id: &str,
    computed_at: &str,
    batch_size: usize,
) -> Result<VisitDeriveFallbackSummary> {
    let source_profile_id = load_archive_source_profile_id(connection, profile_id)?;
    let mut cursor = None;
    let mut seen_domains = HashSet::<String>::new();
    let mut dirty_date_keys = BTreeSet::<String>::new();
    let mut dirty_from_visit_ms = None;
    let mut processed_visits = 0usize;

    loop {
        let mut batch = load_visible_visit_batch(
            connection,
            profile_id,
            source_profile_id,
            cursor,
            batch_size,
        )?;
        if batch.is_empty() {
            break;
        }
        compute_is_new_domain_with_seen(&mut batch, &mut seen_domains);
        if dirty_from_visit_ms.is_none() {
            dirty_from_visit_ms = batch.first().map(|visit| visit.visit_time_ms);
        }
        dirty_date_keys.extend(batch.iter().map(|visit| local_date_key(visit.visit_time_ms)));
        processed_visits += batch.len();
        persist_visit_derived_facts(connection, &batch, computed_at)?;
        cursor = batch.last().map(|visit| VisibleVisitBatchCursor {
            visit_time_ms: visit.visit_time_ms,
            visit_id: visit.visit_id,
        });
    }

    Ok(VisitDeriveFallbackSummary {
        processed_visits,
        dirty_date_keys: dirty_date_keys.into_iter().collect(),
        dirty_from_visit_ms,
    })
}
