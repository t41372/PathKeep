//! Structural rebuild stage orchestration for Core Intelligence.
//!
//! ## Responsibilities
//! - Own the structural stage's incremental-versus-fallback decision logic.
//! - Load the bounded dirty-range metadata needed to replay structural rows.
//! - Bridge streamed structural replay with aggregate recomputation and
//!   checkpoint persistence.
//!
//! ## Not responsible for
//! - Schema bootstrap or top-level rebuild entrypoints.
//! - Session/trail/refind/habit builder internals.
//! - Route-level query and detail reads.
//!
//! ## Dependencies
//! - Incremental checkpoint helpers from `incremental`.
//! - Structural replay/persistence owners and aggregate builders.
//! - Search-trail and derived-visit tables in the intelligence plane.
//!
//! ## Performance notes
//! - Incremental paths only replay the dirty structural tail and rebuild
//!   profile-scoped aggregates from streamed batches.
//! - Fallback paths are explicit and recorded in checkpoint metadata so callers
//!   can surface why a full replay happened.

use super::intelligence_structural_aggregates::{
    build_reopened_investigations, build_structural_profile_aggregates_from_batches,
};
use super::intelligence_structural_build::build_query_families_from_batches;
use super::intelligence_structural_persist::{
    replace_query_families, replace_structural_profile_aggregates,
};
use super::intelligence_structural_stream::rebuild_structural_tail_state;
use super::{
    ProfileSourceWatermark, StageCheckpoint, StageExecutionMode, StageRunResult,
    StructuralDeltaSummary, TrailBatchCursor, TrailRecord, clear_core_tables_for_job_kind,
    load_stage_checkpoint, save_stage_checkpoint, stage_name, stage_version, watermark_regressed,
};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

/// Runs the structural stage for one profile, choosing incremental replay when
/// the checkpoint and archive watermark still line up.
pub(super) fn execute_structural_stage(
    connection: &Connection,
    profile_id: &str,
    watermark: &ProfileSourceWatermark,
    force_full: bool,
    run_id: i64,
    computed_at: &str,
) -> Result<StageRunResult> {
    let current_version =
        stage_version(connection, crate::intelligence_catalog::RebuildMode::StructuralRebuild)?;
    let checkpoint = load_stage_checkpoint(
        connection,
        profile_id,
        crate::intelligence_catalog::RebuildMode::StructuralRebuild,
    )?;
    if watermark.visible_visit_count == 0 {
        clear_core_tables_for_job_kind(
            connection,
            Some(profile_id),
            crate::intelligence_catalog::RebuildMode::StructuralRebuild,
        )?;
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(crate::intelligence_catalog::RebuildMode::StructuralRebuild)
                    .to_string(),
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
                "No visible visits remained for {profile_id}; cleared structural entities."
            )],
            ..StageRunResult::default()
        });
    }

    let mut fallback_reason =
        structural_fallback_reason(force_full, checkpoint.as_ref(), watermark, &current_version);

    if fallback_reason.is_none()
        && checkpoint.as_ref().is_some_and(|checkpoint| checkpoint.source_watermark == *watermark)
    {
        save_stage_checkpoint(
            connection,
            &StageCheckpoint {
                profile_id: profile_id.to_string(),
                stage: stage_name(crate::intelligence_catalog::RebuildMode::StructuralRebuild)
                    .to_string(),
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
            notes: vec![format!("Structural entities for {profile_id} were already up to date.")],
            ..StageRunResult::default()
        });
    }

    let (execution_mode, dirty_visit_count, dirty_date_keys, structural_start_ms) =
        if let Some(_reason) = fallback_reason.clone() {
            (
                StageExecutionMode::FallbackFull,
                watermark.visible_visit_count as usize,
                load_profile_dirty_date_keys(connection, profile_id, None, None)?,
                None,
            )
        } else {
            let last_processed_visit_id =
                checkpoint.as_ref().map(|value| value.last_processed_visit_id).unwrap_or_default();
            let delta_summary =
                load_structural_delta_summary(connection, profile_id, last_processed_visit_id)?;
            let expected_delta = (watermark.visible_visit_count
                - checkpoint
                    .as_ref()
                    .map(|value| value.source_watermark.visible_visit_count)
                    .unwrap_or_default())
            .max(0) as usize;
            if delta_summary.delta_count == 0 || delta_summary.delta_count != expected_delta {
                fallback_reason = Some(
                    "Structural delta rows no longer matched the current archive watermark."
                        .to_string(),
                );
                (
                    StageExecutionMode::FallbackFull,
                    watermark.visible_visit_count as usize,
                    load_profile_dirty_date_keys(connection, profile_id, None, None)?,
                    None,
                )
            } else {
                let dirty_from_visit_ms = delta_summary.dirty_from_visit_ms.unwrap_or_default();
                (
                    StageExecutionMode::Incremental,
                    expected_delta,
                    delta_summary.dirty_date_keys,
                    Some(expand_structural_rebuild_start(
                        connection,
                        profile_id,
                        dirty_from_visit_ms,
                    )?),
                )
            }
        };

    let tail_report = rebuild_structural_tail_state(
        connection,
        profile_id,
        structural_start_ms,
        computed_at,
        super::STRUCTURAL_TAIL_STREAM_BATCH_SIZE,
    )?;

    let query_families = build_query_families_from_batches(connection, profile_id)?;
    replace_query_families(connection, profile_id, &query_families, computed_at)?;

    let (refind_pages, path_flows, habits) =
        build_structural_profile_aggregates_from_batches(connection, profile_id)?;
    let source_effectiveness =
        build_source_effectiveness_from_database(connection, profile_id, &refind_pages)?;
    let reopened = build_reopened_investigations(&query_families, &refind_pages);
    replace_structural_profile_aggregates(
        connection,
        profile_id,
        &refind_pages,
        &source_effectiveness,
        &habits,
        &reopened,
        &path_flows,
        computed_at,
    )?;

    save_stage_checkpoint(
        connection,
        &StageCheckpoint {
            profile_id: profile_id.to_string(),
            stage: stage_name(crate::intelligence_catalog::RebuildMode::StructuralRebuild)
                .to_string(),
            stage_version: current_version,
            source_watermark: watermark.clone(),
            last_processed_visit_id: watermark.max_visit_id,
            dirty_from_visit_ms: structural_start_ms
                .or_else(|| {
                    load_profile_first_visible_visit_ms(connection, profile_id).ok().flatten()
                })
                .or(tail_report.first_visit_ms),
            dirty_date_key: dirty_date_keys.first().cloned(),
            last_run_id: Some(run_id),
            fallback_reason: fallback_reason.clone(),
            updated_at: computed_at.to_string(),
        },
    )?;

    Ok(StageRunResult {
        processed_visits: if execution_mode == StageExecutionMode::FallbackFull {
            watermark.visible_visit_count.max(0) as usize
        } else {
            tail_report.processed_visits
        },
        sessions: tail_report.sessions,
        search_trails: tail_report.trails,
        query_families: query_families.len(),
        refind_pages: refind_pages.len(),
        source_effectiveness: source_effectiveness.len(),
        reopened_investigations: reopened.len(),
        execution_mode: Some(execution_mode.as_str().to_string()),
        affected_profiles: vec![profile_id.to_string()],
        dirty_visit_count: Some(dirty_visit_count),
        dirty_date_keys,
        fallback_reason: fallback_reason.clone(),
        notes: vec![if execution_mode == StageExecutionMode::Incremental {
            format!("Rebuilt structural tail entities for {profile_id}.")
        } else {
            format!("Rebuilt all structural entities for {profile_id}.")
        }],
        ..StageRunResult::default()
    })
}

fn structural_fallback_reason(
    force_full: bool,
    checkpoint: Option<&StageCheckpoint>,
    watermark: &ProfileSourceWatermark,
    current_version: &str,
) -> Option<String> {
    if force_full {
        return Some("Manual full rebuild requested for structural entities.".to_string());
    }
    match checkpoint {
        None => Some("No structural checkpoint was recorded for this profile yet.".to_string()),
        Some(checkpoint) if checkpoint.stage_version != current_version => {
            Some("Structural rebuild logic changed since the last successful rebuild.".to_string())
        }
        Some(checkpoint) if watermark_regressed(watermark, &checkpoint.source_watermark) => Some(
            "Archive visibility regressed or source counters moved backwards for structural entities."
                .to_string(),
        ),
        _ => None,
    }
}

/// Loads distinct dirty day keys for the structural stage from the current
/// derived visit facts.
pub(super) fn load_profile_dirty_date_keys(
    connection: &Connection,
    profile_id: &str,
    start_ms: Option<i64>,
    last_processed_visit_id: Option<i64>,
) -> Result<Vec<String>> {
    connection
        .prepare(
            "SELECT DISTINCT strftime('%Y-%m-%d', archive.visits.visit_time_ms / 1000.0, 'unixepoch', 'localtime')
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL
               AND (?2 IS NULL OR archive.visits.visit_time_ms >= ?2)
               AND (?3 IS NULL OR visit_derived_facts.visit_id > ?3)
             ORDER BY 1 ASC",
        )?
        .query_map(params![profile_id, start_ms, last_processed_visit_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Summarizes the current structural delta so the stage can decide whether the
/// incremental replay window is still trustworthy.
pub(super) fn load_structural_delta_summary(
    connection: &Connection,
    profile_id: &str,
    last_processed_visit_id: i64,
) -> Result<StructuralDeltaSummary> {
    let (delta_count, dirty_from_visit_ms) = connection.query_row(
        "SELECT COUNT(*), MIN(archive.visits.visit_time_ms)
         FROM visit_derived_facts
         JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?1
           AND archive.visits.reverted_at IS NULL
           AND visit_derived_facts.visit_id > ?2",
        params![profile_id, last_processed_visit_id],
        |row| Ok((row.get::<_, i64>(0)?.max(0) as usize, row.get::<_, Option<i64>>(1)?)),
    )?;
    Ok(StructuralDeltaSummary {
        delta_count,
        dirty_from_visit_ms,
        dirty_date_keys: load_profile_dirty_date_keys(
            connection,
            profile_id,
            None,
            Some(last_processed_visit_id),
        )?,
    })
}

/// Reads the earliest currently visible visit for one profile so fallback
/// checkpoints can keep a truthful dirty-start watermark.
pub(super) fn load_profile_first_visible_visit_ms(
    connection: &Connection,
    profile_id: &str,
) -> Result<Option<i64>> {
    connection
        .query_row(
            "SELECT MIN(archive.visits.visit_time_ms)
             FROM visit_derived_facts
             JOIN archive.visits ON archive.visits.id = visit_derived_facts.visit_id
             WHERE visit_derived_facts.profile_id = ?1
               AND archive.visits.reverted_at IS NULL",
            [profile_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .map(|value| value.flatten())
        .map_err(Into::into)
}

/// Streams persisted trails in `first_visit_ms` order so source-effectiveness
/// rebuilds can stay bounded.
fn load_profile_trail_batch(
    connection: &Connection,
    profile_id: &str,
    after: Option<&TrailBatchCursor>,
    limit: usize,
) -> Result<Vec<TrailRecord>> {
    let mut statement = connection.prepare(
        "SELECT trail_id, profile_id, session_id, initial_query, search_engine, reformulation_count,
                visit_count, landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE profile_id = ?1
           AND (
             ?2 IS NULL
             OR first_visit_ms > ?2
             OR (first_visit_ms = ?2 AND trail_id > ?3)
           )
         ORDER BY first_visit_ms ASC, trail_id ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                profile_id,
                after.map(|cursor| cursor.first_visit_ms),
                after.map(|cursor| cursor.trail_id.as_str()),
                limit.max(1) as i64
            ],
            |row| {
                let queries_json: String = row.get(12)?;
                Ok(TrailRecord {
                    trail_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    session_id: row.get(2)?,
                    initial_query: row.get(3)?,
                    search_engine: row.get(4)?,
                    reformulation_count: row.get(5)?,
                    visit_count: row.get(6)?,
                    landing_url: row.get(7)?,
                    landing_domain: row.get(8)?,
                    first_visit_ms: row.get(9)?,
                    last_visit_ms: row.get(10)?,
                    max_depth: row.get(11)?,
                    queries: serde_json::from_str(&queries_json).unwrap_or_default(),
                    members: Vec::new(),
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Test helper that reloads all persisted trails for one profile.
#[cfg(test)]
pub(super) fn load_profile_trails(
    connection: &Connection,
    profile_id: &str,
) -> Result<Vec<TrailRecord>> {
    let mut cursor = None::<TrailBatchCursor>;
    let mut trails = Vec::new();
    loop {
        let batch = load_profile_trail_batch(
            connection,
            profile_id,
            cursor.as_ref(),
            super::STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        cursor = batch.last().map(|trail| TrailBatchCursor {
            first_visit_ms: trail.first_visit_ms,
            trail_id: trail.trail_id.clone(),
        });
        trails.extend(batch);
    }
    Ok(trails)
}

/// Rebuilds source-effectiveness rows by streaming already-persisted trails
/// instead of reloading full visit history.
pub(super) fn build_source_effectiveness_from_database(
    connection: &Connection,
    profile_id: &str,
    refind_pages: &[super::RefindPageRecord],
) -> Result<Vec<super::SourceEffectivenessRecord>> {
    let mut landing_counts = std::collections::HashMap::<String, i64>::new();
    let mut trail_counts = std::collections::HashMap::<String, i64>::new();
    let mut first_seen = std::collections::HashMap::<String, i64>::new();
    let mut last_seen = std::collections::HashMap::<String, i64>::new();
    let mut cursor = None::<TrailBatchCursor>;

    loop {
        let batch = load_profile_trail_batch(
            connection,
            profile_id,
            cursor.as_ref(),
            super::STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        for trail in &batch {
            if let Some(domain) = &trail.landing_domain {
                *landing_counts.entry(domain.clone()).or_default() += 1;
                *trail_counts.entry(domain.clone()).or_default() += 1;
                first_seen
                    .entry(domain.clone())
                    .and_modify(|value| *value = (*value).min(trail.first_visit_ms))
                    .or_insert(trail.first_visit_ms);
                last_seen
                    .entry(domain.clone())
                    .and_modify(|value| *value = (*value).max(trail.last_visit_ms))
                    .or_insert(trail.last_visit_ms);
            }
        }
        cursor = batch.last().map(|trail| TrailBatchCursor {
            first_visit_ms: trail.first_visit_ms,
            trail_id: trail.trail_id.clone(),
        });
    }

    let reference_counts = refind_pages.iter().fold(
        std::collections::HashMap::<String, i64>::new(),
        |mut acc, page| {
            *acc.entry(page.registrable_domain.clone()).or_default() += 1;
            acc
        },
    );
    let domains = landing_counts
        .keys()
        .chain(reference_counts.keys())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();

    Ok(domains
        .into_iter()
        .map(|domain| {
            let stable_landing_count = *landing_counts.get(&domain).unwrap_or(&0);
            let reference_count = *reference_counts.get(&domain).unwrap_or(&0);
            let trail_count = *trail_counts.get(&domain).unwrap_or(&0);
            let source_role = if reference_count >= stable_landing_count && reference_count > 0 {
                "reference"
            } else {
                "landing"
            };
            let effectiveness_score = (stable_landing_count as f32 * 2.0)
                + (reference_count as f32 * 1.5)
                + (trail_count as f32 * 0.5);
            let evidence_json = serde_json::json!({
                "stableLandingCount": stable_landing_count,
                "referenceCount": reference_count,
                "trailCount": trail_count
            })
            .to_string();
            super::SourceEffectivenessRecord {
                profile_id: profile_id.to_string(),
                registrable_domain: domain.clone(),
                source_role: source_role.to_string(),
                trail_count,
                stable_landing_count,
                effectiveness_score,
                evidence_json,
                first_seen_ms: *first_seen.get(&domain).unwrap_or(&0),
                last_seen_ms: *last_seen.get(&domain).unwrap_or(&0),
            }
        })
        .collect())
}

/// Expands the dirty structural replay window backward to the first overlapping
/// session or trail so replayed boundaries stay deterministic.
pub(super) fn expand_structural_rebuild_start(
    connection: &Connection,
    profile_id: &str,
    dirty_from_visit_ms: i64,
) -> Result<i64> {
    let session_start = connection
        .query_row(
            "SELECT MIN(first_visit_ms)
             FROM sessions
             WHERE profile_id = ?1
               AND last_visit_ms >= ?2",
            params![profile_id, dirty_from_visit_ms - super::SESSION_GAP_MS],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten();
    let trail_start = connection
        .query_row(
            "SELECT MIN(first_visit_ms)
             FROM search_trails
             WHERE profile_id = ?1
               AND last_visit_ms >= ?2",
            params![profile_id, dirty_from_visit_ms - super::TRAIL_GAP_MS],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten();
    Ok(session_start.into_iter().chain(trail_start).min().unwrap_or(dirty_from_visit_ms))
}

#[cfg(test)]
mod tests {
    use super::{ProfileSourceWatermark, StageCheckpoint, structural_fallback_reason};

    #[test]
    fn structural_fallback_reason_reports_regressed_watermarks() {
        let checkpoint = StageCheckpoint {
            stage_version: "structural-rebuild-v2".to_string(),
            source_watermark: ProfileSourceWatermark {
                visible_visit_count: 10,
                max_visit_id: 10,
                max_url_last_visit_ms: 100,
                visible_search_term_count: 4,
            },
            ..StageCheckpoint::default()
        };
        let regressed = ProfileSourceWatermark {
            visible_visit_count: 9,
            max_visit_id: 10,
            max_url_last_visit_ms: 100,
            visible_search_term_count: 4,
        };

        let reason = structural_fallback_reason(
            false,
            Some(&checkpoint),
            &regressed,
            "structural-rebuild-v2",
        )
        .expect("regression reason");

        assert!(reason.contains("regressed"));
        assert!(
            structural_fallback_reason(
                false,
                Some(&checkpoint),
                &checkpoint.source_watermark,
                "structural-rebuild-v2",
            )
            .is_none()
        );
    }
}
