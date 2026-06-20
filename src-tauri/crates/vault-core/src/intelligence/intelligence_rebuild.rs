//! Core Intelligence rebuild orchestration ownership.
//!
//! ## Responsibilities
//! - Own the public rebuild entrypoints and their progress-reporting contract.
//! - Coordinate per-profile deterministic rebuild stages without reopening the
//!   route-level read-model layer.
//! - Persist ready-state runtime updates once a rebuild finishes.
//!
//! ## Not responsible for
//! - Schema bootstrap and derived-state clear operations.
//! - Route-level `/intelligence` reads or explainability surfaces.
//! - The stage-internal SQL used by visit/daily/structural rebuild workers.
//!
//! ## Dependencies
//! - Parent-module stage executors, batch builders, and rebuildable entity
//!   builders.
//! - Incremental checkpoint helpers and runtime-update persistence.
//! - Canonical archive access through `open_intelligence_connection`.
//!
//! ## Performance notes
//! - This owner coordinates streamed/staged rebuild paths and never blocks on
//!   route rendering work.
//! - Scoped debug rebuilds explicitly stay on the fallback full-recompute path
//!   and do not advance incremental checkpoints.

use super::intelligence_structural_aggregates::{
    build_habit_patterns, build_path_flows, build_refind_pages, build_reopened_investigations,
    build_source_effectiveness,
};
use super::intelligence_structural_build::{
    build_query_families, build_search_trails, build_sessions,
};
use super::intelligence_structural_stage::execute_structural_stage;
use super::{
    CORE_PHASES, CoreIntelligenceProgress, DailyRollupBundle, StageRunResult, build_daily_rollups,
    build_profile_state, clear_core_tables_for_job_kind, delete_stage_checkpoints,
    ensure_core_intelligence_schema, execute_daily_rollup_stage, execute_visit_derive_stage,
    list_core_intelligence_profiles, load_profile_source_watermark, load_visible_visits,
    merge_rollups, persist_core_state_for_job_kind,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    intelligence_catalog::RebuildMode,
    intelligence_runtime::{
        DeterministicModuleRuntimeUpdate, persist_deterministic_module_runtime_updates,
    },
    models::{AppConfig, CoreIntelligenceRebuildReport, CoreIntelligenceRebuildRequest},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;

/// Runs a full deterministic rebuild without streaming progress callbacks.
pub fn run_core_intelligence(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceRebuildReport> {
    run_core_intelligence_job_with_progress(
        paths,
        config,
        key,
        RebuildMode::FullRebuild,
        request,
        |_progress| Ok(()),
    )
}

/// Runs a full deterministic rebuild and forwards stage/profile progress to the
/// caller.
pub fn run_core_intelligence_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &CoreIntelligenceRebuildRequest,
    on_progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    run_core_intelligence_job_with_progress(
        paths,
        config,
        key,
        RebuildMode::FullRebuild,
        request,
        on_progress,
    )
}

/// Runs exactly one deterministic rebuild job type while preserving the shared
/// progress and result contract.
pub fn run_core_intelligence_job_type_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_type: &str,
    request: &CoreIntelligenceRebuildRequest,
    on_progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    run_core_intelligence_job_with_progress(
        paths,
        config,
        key,
        RebuildMode::from_job_type(job_type)?,
        request,
        on_progress,
    )
}

/// Chooses the staged or legacy rebuild path for one deterministic rebuild
/// request and merges per-profile results into one report.
fn run_core_intelligence_job_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_kind: RebuildMode,
    request: &CoreIntelligenceRebuildRequest,
    mut on_progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    if request.limit.is_some() {
        return run_core_intelligence_legacy_job_with_progress(
            paths,
            config,
            key,
            job_kind,
            request,
            on_progress,
        );
    }
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let run_id = Utc::now().timestamp_millis();
    let computed_at = now_rfc3339();
    let profile_ids = list_core_intelligence_profiles(&connection, request.profile_id.as_deref())?;
    if profile_ids.is_empty() {
        clear_core_tables_for_job_kind(&connection, request.profile_id.as_deref(), job_kind)?;
        delete_stage_checkpoints(&connection, request.profile_id.as_deref())?;
        let notes = vec!["No visible visits matched the requested rebuild scope.".to_string()];
        persist_ready_module_updates(
            &connection,
            run_id,
            Some(computed_at.clone()),
            &job_kind.module_ids(),
            &notes,
        )?;
        return Ok(CoreIntelligenceRebuildReport {
            run_id,
            processed_visits: 0,
            visit_derived_facts: 0,
            sessions: 0,
            search_trails: 0,
            query_families: 0,
            refind_pages: 0,
            source_effectiveness: 0,
            reopened_investigations: 0,
            execution_mode: Some(super::StageExecutionMode::Noop.as_str().to_string()),
            affected_profiles: Some(Vec::new()),
            dirty_visit_count: Some(0),
            dirty_date_keys: Some(Vec::new()),
            fallback_reason: None,
            stage_timings_ms: None,
            notes,
            last_run_at: computed_at,
        });
    }

    let mut aggregate = StageRunResult {
        execution_mode: Some(super::StageExecutionMode::Noop.as_str().to_string()),
        ..StageRunResult::default()
    };
    let profile_total = profile_ids.len();
    for (profile_index, profile_id) in profile_ids.iter().enumerate() {
        let watermark = load_profile_source_watermark(&connection, profile_id)?;
        on_progress(CoreIntelligenceProgress {
            phase: "profile-scan".to_string(),
            detail: format!("Preparing {} for {}.", job_kind.label(), profile_id),
            processed_items: Some(profile_index),
            total_items: Some(profile_total),
            progress_percent: Some((profile_index as f32 / profile_total.max(1) as f32) * 100.0),
        })?;
        let result = match job_kind {
            RebuildMode::VisitDerive => execute_visit_derive_stage(
                &connection,
                profile_id,
                &watermark,
                request.full_rebuild,
                run_id,
                &computed_at,
            )?,
            RebuildMode::DailyRollup => execute_daily_rollup_stage(
                &connection,
                profile_id,
                &watermark,
                request.full_rebuild,
                run_id,
                &computed_at,
            )?,
            RebuildMode::StructuralRebuild => execute_structural_stage(
                &connection,
                profile_id,
                &watermark,
                request.full_rebuild,
                run_id,
                &computed_at,
            )?,
            RebuildMode::FullRebuild => execute_full_rebuild_stages(
                &connection,
                profile_id,
                &watermark,
                run_id,
                &computed_at,
            )?,
        };
        merge_stage_run_result(&mut aggregate, result, job_kind);
        on_progress(CoreIntelligenceProgress {
            phase: "profile-build".to_string(),
            detail: format!("Updated {} for profile {}.", job_kind.label(), profile_id),
            processed_items: Some(profile_index + 1),
            total_items: Some(profile_total),
            progress_percent: Some(
                ((profile_index + 1) as f32 / profile_total.max(1) as f32) * 100.0,
            ),
        })?;
    }

    ensure_stage_notes(&mut aggregate, job_kind);
    persist_ready_module_updates(
        &connection,
        run_id,
        Some(computed_at.clone()),
        &job_kind.module_ids(),
        &aggregate.notes,
    )?;
    Ok(CoreIntelligenceRebuildReport {
        run_id,
        processed_visits: aggregate.processed_visits,
        visit_derived_facts: aggregate.visit_derived_facts,
        sessions: aggregate.sessions,
        search_trails: aggregate.search_trails,
        query_families: aggregate.query_families,
        refind_pages: aggregate.refind_pages,
        source_effectiveness: aggregate.source_effectiveness,
        reopened_investigations: aggregate.reopened_investigations,
        execution_mode: aggregate.execution_mode,
        affected_profiles: Some(aggregate.affected_profiles),
        dirty_visit_count: aggregate.dirty_visit_count,
        dirty_date_keys: Some(aggregate.dirty_date_keys),
        fallback_reason: aggregate.fallback_reason,
        stage_timings_ms: aggregate.stage_timings_ms,
        notes: aggregate.notes,
        last_run_at: computed_at,
    })
}

/// Preserves the accepted scoped-debug rebuild behavior that recomputes the
/// requested subset in-memory without advancing incremental checkpoints.
fn run_core_intelligence_legacy_job_with_progress<F>(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_kind: RebuildMode,
    request: &CoreIntelligenceRebuildRequest,
    mut on_progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let run_id = Utc::now().timestamp_millis();
    let computed_at = now_rfc3339();
    let notes =
        vec![format!("Completed a {} through the scoped debug fallback path.", job_kind.label())];
    let visits = load_visible_visits(&connection, request.profile_id.as_deref(), request.limit)?;
    if visits.is_empty() {
        clear_core_tables_for_job_kind(&connection, request.profile_id.as_deref(), job_kind)?;
        persist_ready_module_updates(
            &connection,
            run_id,
            Some(computed_at.clone()),
            &job_kind.module_ids(),
            &["No visible visits matched the requested rebuild scope.".to_string()],
        )?;
        return Ok(CoreIntelligenceRebuildReport {
            run_id,
            processed_visits: 0,
            visit_derived_facts: 0,
            sessions: 0,
            search_trails: 0,
            query_families: 0,
            refind_pages: 0,
            source_effectiveness: 0,
            reopened_investigations: 0,
            execution_mode: Some(super::StageExecutionMode::FallbackFull.as_str().to_string()),
            affected_profiles: request.profile_id.clone().map(|profile_id| vec![profile_id]),
            dirty_visit_count: Some(0),
            dirty_date_keys: Some(Vec::new()),
            fallback_reason: Some(
                "Scoped debug rebuilds use the legacy full recompute path and do not advance incremental checkpoints."
                    .to_string(),
            ),
            stage_timings_ms: None,
            notes,
            last_run_at: computed_at,
        });
    }

    let total_visible_visits = visits.len();
    on_progress(progress_for_phase(0, Some(0), Some(total_visible_visits)))?;
    let by_profile = build_profile_state(visits);
    on_progress(progress_for_phase(1, None, None))?;

    let needs_visit_derived_facts = job_kind.requires_visit_derived_facts();
    let needs_daily_rollups = job_kind.requires_daily_rollups();
    let needs_structural_entities = job_kind.requires_structural_entities();
    let mut all_visits = Vec::new();
    let mut all_sessions = Vec::new();
    let mut all_search_events = Vec::new();
    let mut all_trails = Vec::new();
    let mut all_query_families = Vec::new();
    let mut all_refind_pages = Vec::new();
    let mut all_source_effectiveness = Vec::new();
    let mut all_reopened = Vec::new();
    let mut all_path_flows = Vec::new();
    let mut all_habits = Vec::new();
    let mut rollups = DailyRollupBundle::default();

    let profile_total = by_profile.len();
    let mut affected_profiles = Vec::new();
    for (profile_index, (profile_id, mut profile_visits)) in by_profile.into_iter().enumerate() {
        affected_profiles.push(profile_id.clone());
        super::compute_is_new_domain(&mut profile_visits);
        if needs_structural_entities {
            let sessions = build_sessions(&mut profile_visits);
            let (search_events, trails) = build_search_trails(&mut profile_visits);
            let query_families = build_query_families(&search_events);
            let refind_pages = build_refind_pages(&profile_visits);
            let source_effectiveness = build_source_effectiveness(&trails, &refind_pages);
            let reopened = build_reopened_investigations(&query_families, &refind_pages);
            let path_flows = build_path_flows(&profile_visits);
            let habits = build_habit_patterns(&profile_visits);
            all_sessions.extend(sessions);
            all_search_events.extend(search_events);
            all_trails.extend(trails);
            all_query_families.extend(query_families);
            all_refind_pages.extend(refind_pages);
            all_source_effectiveness.extend(source_effectiveness);
            all_reopened.extend(reopened);
            all_path_flows.extend(path_flows);
            all_habits.extend(habits);
        }
        if needs_daily_rollups {
            merge_rollups(&mut rollups, build_daily_rollups(&profile_visits));
        }
        if needs_visit_derived_facts {
            all_visits.extend(profile_visits);
        }
        on_progress(CoreIntelligenceProgress {
            phase: "profile-build".to_string(),
            detail: format!("Built Core Intelligence entities for profile {profile_id}"),
            processed_items: Some(profile_index + 1),
            total_items: Some(profile_total),
            progress_percent: Some(
                ((profile_index + 1) as f32 / profile_total.max(1) as f32) * 100.0,
            ),
        })?;
    }

    on_progress(progress_for_phase(4, None, None))?;
    persist_core_state_for_job_kind(
        &connection,
        request.profile_id.as_deref(),
        job_kind,
        &computed_at,
        &all_visits,
        &rollups,
        &all_sessions,
        &all_search_events,
        &all_trails,
        &all_query_families,
        &all_refind_pages,
        &all_source_effectiveness,
        &all_habits,
        &all_reopened,
        &all_path_flows,
    )?;
    on_progress(progress_for_phase(6, Some(total_visible_visits), Some(total_visible_visits)))?;

    persist_ready_module_updates(
        &connection,
        run_id,
        Some(computed_at.clone()),
        &job_kind.module_ids(),
        &notes,
    )?;

    Ok(CoreIntelligenceRebuildReport {
        run_id,
        processed_visits: total_visible_visits,
        visit_derived_facts: all_visits.len(),
        sessions: all_sessions.len(),
        search_trails: all_trails.len(),
        query_families: all_query_families.len(),
        refind_pages: all_refind_pages.len(),
        source_effectiveness: all_source_effectiveness.len(),
        reopened_investigations: all_reopened.len(),
        execution_mode: Some(super::StageExecutionMode::FallbackFull.as_str().to_string()),
        affected_profiles: Some(affected_profiles),
        dirty_visit_count: Some(total_visible_visits),
        dirty_date_keys: Some(Vec::new()),
        fallback_reason: Some(
            "Scoped debug rebuilds use the legacy full recompute path and do not advance incremental checkpoints."
                .to_string(),
        ),
        stage_timings_ms: None,
        notes,
        last_run_at: computed_at,
    })
}

/// Merges one profile-scoped stage result into the aggregate report returned to
/// the worker or Tauri command layer.
pub(super) fn merge_stage_run_result(
    aggregate: &mut StageRunResult,
    next: StageRunResult,
    job_kind: RebuildMode,
) {
    let execution_mode = next.execution_mode.clone();
    aggregate.visit_derived_facts += next.visit_derived_facts;
    aggregate.sessions += next.sessions;
    aggregate.search_trails += next.search_trails;
    aggregate.query_families += next.query_families;
    aggregate.refind_pages += next.refind_pages;
    aggregate.source_effectiveness += next.source_effectiveness;
    aggregate.reopened_investigations += next.reopened_investigations;
    if matches!(job_kind, RebuildMode::VisitDerive | RebuildMode::FullRebuild) {
        aggregate.processed_visits += next.processed_visits;
    } else {
        aggregate.processed_visits = aggregate.processed_visits.max(next.processed_visits);
    }
    if matches!(execution_mode.as_deref(), Some("fallback-full"))
        || aggregate.execution_mode.is_none()
        || matches!(aggregate.execution_mode.as_deref(), Some("noop"))
    {
        aggregate.execution_mode = execution_mode;
    }
    aggregate.affected_profiles.extend(next.affected_profiles);
    aggregate.affected_profiles.sort();
    aggregate.affected_profiles.dedup();
    if let Some(value) = next.dirty_visit_count {
        aggregate.dirty_visit_count = Some(aggregate.dirty_visit_count.unwrap_or(0) + value);
    }
    aggregate.dirty_date_keys.extend(next.dirty_date_keys);
    aggregate.dirty_date_keys.sort();
    aggregate.dirty_date_keys.dedup();
    if aggregate.fallback_reason.is_none() {
        aggregate.fallback_reason = next.fallback_reason;
    }
    match (&mut aggregate.stage_timings_ms, next.stage_timings_ms) {
        (Some(current), Some(next)) => {
            current.visit_derive_ms += next.visit_derive_ms;
            current.daily_rollup_ms += next.daily_rollup_ms;
            current.structural_rebuild_ms += next.structural_rebuild_ms;
            current.total_ms += next.total_ms;
        }
        (None, Some(next)) => {
            aggregate.stage_timings_ms = Some(next);
        }
        _ => {}
    }
    aggregate.notes.extend(next.notes);
}

/// Runs the three staged rebuild owners in order and folds their timings into
/// one full-rebuild result. Each stage uses its own checkpoint to decide whether
/// an incremental or fallback-full path is needed, so a FullRebuild with warm
/// checkpoints only reprocesses the delta since the last run.
fn execute_full_rebuild_stages(
    connection: &Connection,
    profile_id: &str,
    watermark: &super::ProfileSourceWatermark,
    run_id: i64,
    computed_at: &str,
) -> Result<StageRunResult> {
    let visit_started = std::time::Instant::now();
    let mut combined =
        execute_visit_derive_stage(connection, profile_id, watermark, false, run_id, computed_at)?;
    let visit_derive_ms = visit_started.elapsed().as_millis() as u64;
    let daily_started = std::time::Instant::now();
    let daily =
        execute_daily_rollup_stage(connection, profile_id, watermark, false, run_id, computed_at)?;
    let daily_rollup_ms = daily_started.elapsed().as_millis() as u64;
    let structural_started = std::time::Instant::now();
    let structural =
        execute_structural_stage(connection, profile_id, watermark, false, run_id, computed_at)?;
    let structural_rebuild_ms = structural_started.elapsed().as_millis() as u64;
    merge_stage_run_result(&mut combined, daily, RebuildMode::DailyRollup);
    merge_stage_run_result(&mut combined, structural, RebuildMode::StructuralRebuild);
    combined.execution_mode = Some(super::StageExecutionMode::FallbackFull.as_str().to_string());
    combined.dirty_visit_count = Some(watermark.visible_visit_count.max(0) as usize);
    combined.stage_timings_ms = Some(crate::models::CoreIntelligenceStageTimings {
        visit_derive_ms,
        daily_rollup_ms,
        structural_rebuild_ms,
        total_ms: visit_derive_ms + daily_rollup_ms + structural_rebuild_ms,
    });
    combined.notes.push(format!(
        "Ran checkpoint-aware Core Intelligence rebuild for {}; each stage used its incremental path when a warm checkpoint was available.",
        profile_id
    ));
    Ok(combined)
}

fn ensure_stage_notes(aggregate: &mut StageRunResult, job_kind: RebuildMode) {
    if aggregate.notes.is_empty() {
        aggregate.notes.push(format!("Completed a {}.", job_kind.label()));
    }
}

/// Builds the shared progress payload for legacy scoped rebuild phases.
fn progress_for_phase(
    phase_index: usize,
    processed_items: Option<usize>,
    total_items: Option<usize>,
) -> CoreIntelligenceProgress {
    let (phase, detail) = CORE_PHASES
        .get(phase_index.min(CORE_PHASES.len().saturating_sub(1)))
        .copied()
        .unwrap_or(("core-intelligence", "Updating Core Intelligence"));
    CoreIntelligenceProgress {
        phase: phase.to_string(),
        detail: detail.to_string(),
        processed_items,
        total_items,
        progress_percent: match (processed_items, total_items) {
            (Some(current), Some(total)) if total > 0 => {
                Some((current as f32 / total as f32) * 100.0)
            }
            _ => None,
        },
    }
}

/// Resolves the canonical archive `source_profile_id` used by streamed rebuild
/// readers for one deterministic profile key.
pub(super) fn load_archive_source_profile_id(
    connection: &Connection,
    profile_id: &str,
) -> Result<i64> {
    connection
        .query_row(
            "SELECT id
             FROM archive.source_profiles
             WHERE profile_key = ?1
             LIMIT 1",
            [profile_id],
            |row| row.get(0),
        )
        .with_context(|| format!("loading source_profile_id for {profile_id}"))
}

/// Marks deterministic module runtimes as ready after a successful rebuild so
/// route shells do not need to infer freshness from raw job rows.
fn persist_ready_module_updates(
    connection: &Connection,
    run_id: i64,
    built_at: Option<String>,
    module_ids: &[&str],
    notes: &[String],
) -> Result<()> {
    let shared_notes = if notes.is_empty() {
        vec!["Core Intelligence modules are in sync with the current derived plane.".to_string()]
    } else {
        notes.to_vec()
    };
    let updates = module_ids
        .iter()
        .map(|module_id| module_update(module_id, run_id, built_at.clone(), &shared_notes))
        .collect::<Vec<_>>();
    persist_deterministic_module_runtime_updates(connection, &updates)
}

/// Shapes one runtime-update row for the deterministic module registry.
fn module_update(
    module_id: &str,
    run_id: i64,
    built_at: Option<String>,
    notes: &[String],
) -> DeterministicModuleRuntimeUpdate {
    DeterministicModuleRuntimeUpdate {
        module_id: module_id.to_string(),
        status: "ready".to_string(),
        last_run_id: Some(run_id),
        last_built_at: built_at,
        last_invalidated_at: None,
        stale_reason: None,
        notes: notes.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StageRunResult, ensure_stage_notes, module_update, persist_ready_module_updates,
        progress_for_phase,
    };
    use crate::{
        intelligence_catalog::RebuildMode,
        intelligence_runtime::ensure_intelligence_runtime_schema,
        models::{ACTIVITY_MIX_MODULE_ID, SESSIONS_MODULE_ID},
    };
    use rusqlite::Connection;

    #[test]
    fn rebuild_helpers_keep_default_notes_and_progress_edges_truthful() {
        let first = progress_for_phase(0, Some(2), Some(4));
        assert_eq!(first.phase, "visit-derived-facts");
        assert_eq!(first.progress_percent, Some(50.0));

        let bounded = progress_for_phase(usize::MAX, Some(1), Some(0));
        assert_eq!(bounded.phase, "deep-intelligence");
        assert_eq!(bounded.progress_percent, None);
        assert!(bounded.detail.contains("Phase 2"));

        let update =
            module_update(SESSIONS_MODULE_ID, 41, Some("2026-04-17T00:00:00Z".to_string()), &[]);
        assert_eq!(update.module_id, SESSIONS_MODULE_ID);
        assert_eq!(update.status, "ready");
        assert_eq!(update.last_run_id, Some(41));
        assert!(update.notes.is_empty());

        let mut aggregate = StageRunResult::default();
        ensure_stage_notes(&mut aggregate, RebuildMode::DailyRollup);
        assert_eq!(aggregate.notes, vec!["Completed a daily rollup refresh.".to_string()]);

        let connection = Connection::open_in_memory().expect("sqlite");
        ensure_intelligence_runtime_schema(&connection).expect("runtime schema");
        persist_ready_module_updates(&connection, 42, None, &[ACTIVITY_MIX_MODULE_ID], &[])
            .expect("persist ready update");
        let notes_json: String = connection
            .query_row(
                "SELECT notes_json FROM deterministic_module_runtime WHERE module_id = ?1",
                [ACTIVITY_MIX_MODULE_ID],
                |row| row.get(0),
            )
            .expect("notes json");
        let notes = serde_json::from_str::<Vec<String>>(&notes_json).expect("notes parse");
        assert_eq!(
            notes,
            vec![
                "Core Intelligence modules are in sync with the current derived plane.".to_string()
            ]
        );
    }
}
