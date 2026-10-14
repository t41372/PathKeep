//! Responsibilities: run resumable, bounded intelligence stages and publish generations.
//! Not responsible for: storage ownership, provider calls or render-path SQL.
//! Failure modes: batch/restart boundaries change results, a profile clears another,
//! or interactive writes wait for an entire rebuild. Tests: intelligence_turso and stage regressions.

use super::*;
use crate::{
    Vault,
    intelligence_runtime::{
        DeterministicModuleRuntimeUpdate, persist_deterministic_module_runtime_updates,
    },
    models::{CoreIntelligenceRebuildReport, CoreIntelligenceRebuildRequest, DerivedRuntimeNote},
};
use anyhow::Result;
use vault_store::{Db, Priority, Session};

/// Rebuilds without requiring a UI progress subscriber.
pub fn run_core_intelligence(
    vault: &Vault,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceRebuildReport> {
    run_core_intelligence_with_progress(vault, request, |_| Ok(()))
}

/// Checkpoints before invoking progress so cancellation never loses an acknowledged batch.
pub fn run_core_intelligence_with_progress<F>(
    vault: &Vault,
    request: &CoreIntelligenceRebuildRequest,
    progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    run_core_intelligence_job_type_with_progress(
        vault,
        RebuildMode::FullRebuild.job_type(),
        request,
        progress,
    )
}

/// How many times one job builds the profiles: once, then again while the user keeps changing
/// site categories during it. Past this the next refresh takes them.
const CATEGORY_PASSES: usize = 4;

/// The queue retains its stage contract while sharing one native generation pipeline.
pub fn run_core_intelligence_job_type_with_progress<F>(
    vault: &Vault,
    job_type: &str,
    request: &CoreIntelligenceRebuildRequest,
    mut progress: F,
) -> Result<CoreIntelligenceRebuildReport>
where
    F: FnMut(CoreIntelligenceProgress) -> Result<()>,
{
    let _guard = vault.intelligence_build();
    let kind = RebuildMode::from_job_type(job_type)?;
    let revisions = vault.read(Db::Intelligence, module_invalidation_revisions)?;
    let started = std::time::Instant::now();
    let run_id = chrono::Utc::now().timestamp_millis();
    let mut aggregate = StageRunResult::default();
    let sources = build_pipeline::sources(vault, request.profile_id.as_deref())?;
    if sources.is_empty() {
        aggregate.execution_mode = Some("noop".into());
        aggregate.notes.push(DerivedRuntimeNote::new(
            crate::models::DERIVED_NOTE_REBUILD_SCOPE_EMPTY,
            "No visible visits matched the requested rebuild scope.",
        ));
    }
    let batch_size = request.limit.unwrap_or(4096).clamp(1, 4096) as usize;
    let mut timings = crate::models::CoreIntelligenceStageTimings::default();
    // A category the user sets while this runs is sorted in before the job ends, by another pass
    // over the profiles still built with older categories (only the changed sites' pages); a
    // refresh asked for while this job runs is not queued again.
    let mut todo = sources.clone();
    let mut pass = 0;
    while !todo.is_empty() {
        for source in std::mem::take(&mut todo) {
            let mut state =
                build_pipeline::prepare(vault, &source, request.full_rebuild && pass == 0)?;
            let had_work = match kind {
                RebuildMode::VisitDerive => !state.facts_done,
                RebuildMode::DailyRollup => !state.facts_done || !state.daily_done,
                _ => {
                    !state.facts_done
                        || !state.daily_done
                        || !state.navigation_done
                        || !state.aggregates_done
                }
            };
            let facts_started = std::time::Instant::now();
            let before = state.processed;
            build_pipeline::derive(vault, &mut state, batch_size, &mut progress)?;
            timings.visit_derive_ms += facts_started.elapsed().as_millis() as u64;
            let mut processed = state.processed - before;
            if kind != RebuildMode::VisitDerive {
                let daily_started = std::time::Instant::now();
                processed = processed.max(build_stages::daily(
                    vault,
                    &mut state,
                    batch_size,
                    &mut progress,
                )?);
                timings.daily_rollup_ms += daily_started.elapsed().as_millis() as u64;
            }
            if matches!(kind, RebuildMode::StructuralRebuild | RebuildMode::FullRebuild) {
                let structural_started = std::time::Instant::now();
                build_stages::navigation(vault, &mut state, batch_size, &mut progress)?;
                build_stages::aggregates(vault, &mut state, batch_size, &mut progress)?;
                timings.structural_rebuild_ms += structural_started.elapsed().as_millis() as u64;
            }
            let groups: &[&str] = match kind {
                RebuildMode::VisitDerive => &["facts"],
                RebuildMode::DailyRollup => &["facts", "daily"],
                RebuildMode::StructuralRebuild | RebuildMode::FullRebuild => {
                    &["facts", "daily", "navigation", "aggregates"]
                }
            };
            build_pipeline::publish(vault, &mut state, groups)?;
            build_state::collect_old_generations(vault, &state)?;
            let count = |name: &str| state.counts.get(name).copied().unwrap_or(0);
            let result = StageRunResult {
                processed_visits: processed,
                visit_derived_facts: state.processed - before,
                sessions: if kind == RebuildMode::VisitDerive || kind == RebuildMode::DailyRollup {
                    0
                } else {
                    count("sessions")
                },
                search_trails: count("search_trails"),
                query_families: count("query_families"),
                refind_pages: count("refind_pages"),
                reopened_investigations: count("reopened_investigations"),
                execution_mode: Some(
                    if !had_work || state.source.visible_visit_count == 0 {
                        "noop"
                    } else if state.full {
                        "fallback-full"
                    } else {
                        "incremental"
                    }
                    .into(),
                ),
                fallback_reason: if had_work {
                    state.fallback.as_deref().map(|reason| fallback_message(kind, reason))
                } else {
                    None
                },
                fallback_reason_code: if had_work {
                    state.fallback.as_ref().map(|reason| {
                        format!(
                            "{}-{reason}",
                            match kind {
                                RebuildMode::VisitDerive => "visit-derive",
                                RebuildMode::DailyRollup => "daily-rollup",
                                _ => "structural",
                            }
                        )
                    })
                } else {
                    None
                },
                notes: if state.source.visible_visit_count == 0 {
                    vec![
                        DerivedRuntimeNote::new(
                            match kind {
                                RebuildMode::VisitDerive => {
                                    crate::models::DERIVED_NOTE_VISIT_FACTS_CLEARED_NO_VISITS
                                }
                                RebuildMode::DailyRollup => {
                                    crate::models::DERIVED_NOTE_DAILY_ROLLUPS_CLEARED_NO_VISITS
                                }
                                _ => crate::models::DERIVED_NOTE_STRUCTURAL_CLEARED_NO_VISITS,
                            },
                            format!(
                                "No visible visits remained for {}; cleared derived rows.",
                                state.profile_id
                            ),
                        )
                        .with_profile_id(&state.profile_id),
                    ]
                } else if !had_work {
                    vec![
                        DerivedRuntimeNote::new(
                            crate::models::DERIVED_NOTE_MODULES_IN_SYNC,
                            "Core Intelligence is already up to date.",
                        )
                        .with_profile_id(&state.profile_id),
                    ]
                } else {
                    Vec::new()
                },
                affected_profiles: vec![state.profile_id.clone()],
                dirty_visit_count: Some(if had_work { state.dirty_visit_count } else { 0 }),
                dirty_date_keys: state.dirty_dates.clone(),
                ..StageRunResult::default()
            };
            merge_stage_run_result(&mut aggregate, result, kind);
        }
        pass += 1;
        if pass < CATEGORY_PASSES {
            todo = build_pipeline::built_with_older_categories(vault, &sources)?;
        }
    }
    timings.total_ms = started.elapsed().as_millis() as u64;
    aggregate.stage_timings_ms = Some(timings);
    ensure_stage_notes(&mut aggregate, kind);
    let built_at = crate::utils::now_rfc3339();
    let at = built_at.clone();
    let notes = aggregate.notes.clone();
    let ids = kind.module_ids().iter().map(|v| v.to_string()).collect::<Vec<_>>();
    vault.write(Db::Intelligence, Priority::Batch, move |s| {
        persist_ready_module_updates(
            s,
            run_id,
            Some(at),
            &ids.iter().map(String::as_str).collect::<Vec<_>>(),
            &notes,
            &revisions,
        )
    })?;
    if !aggregate.affected_profiles.is_empty()
        && matches!(kind, RebuildMode::StructuralRebuild | RebuildMode::FullRebuild)
    {
        let detail = match vault.config().preferred_language {
            crate::models::LanguagePreference::ZhCn => "正在准备洞察视图。",
            crate::models::LanguagePreference::ZhTw => "正在準備洞察檢視。",
            _ => "Preparing insight views.",
        };
        super::intelligence_overview::warm_overview_snapshots(vault, &mut || {
            progress(CoreIntelligenceProgress {
                phase: "overview".into(),
                detail: detail.into(),
                processed_items: None,
                total_items: None,
                progress_percent: None,
            })
        })?;
    }
    Ok(CoreIntelligenceRebuildReport {
        run_id,
        processed_visits: aggregate.processed_visits,
        visit_derived_facts: aggregate.visit_derived_facts,
        sessions: aggregate.sessions,
        search_trails: aggregate.search_trails,
        query_families: aggregate.query_families,
        refind_pages: aggregate.refind_pages,
        reopened_investigations: aggregate.reopened_investigations,
        execution_mode: aggregate.execution_mode,
        affected_profiles: Some(aggregate.affected_profiles),
        dirty_visit_count: aggregate.dirty_visit_count,
        dirty_date_keys: Some(aggregate.dirty_date_keys),
        fallback_reason: aggregate.fallback_reason,
        fallback_reason_code: aggregate.fallback_reason_code,
        stage_timings_ms: aggregate.stage_timings_ms,
        notes: crate::models::derived_runtime_note_messages(&aggregate.notes),
        note_details: aggregate.notes,
        last_run_at: built_at,
    })
}

fn fallback_message(kind: RebuildMode, reason: &str) -> String {
    match reason {
        "manual-full-rebuild" => "Manual full rebuild requested.".into(),
        "no-checkpoint" => "No checkpoint was recorded for this profile yet.".into(),
        "rules-changed" => match kind {
            RebuildMode::VisitDerive => "Visit-derived rules changed since the last build.",
            RebuildMode::DailyRollup => "Daily rollup logic changed since the last build.",
            _ => "Structural logic changed since the last build.",
        }
        .into(),
        "visibility-regressed" => {
            "Archive visibility regressed or source counters moved backwards.".into()
        }
        "delta-mismatch" => {
            "Derived delta rows no longer matched the current archive watermark.".into()
        }
        _ => unreachable!(),
    }
}

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
        aggregate.fallback_reason_code = next.fallback_reason_code;
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

fn ensure_stage_notes(aggregate: &mut StageRunResult, job_kind: RebuildMode) {
    if aggregate.notes.is_empty() {
        aggregate.notes.push(
            DerivedRuntimeNote::new(
                crate::models::DERIVED_NOTE_REBUILD_COMPLETED,
                format!("Completed a {}.", job_kind.label()),
            )
            .with_job_kind(job_kind.job_type()),
        );
    }
}

fn module_invalidation_revisions(
    s: &Session<'_>,
) -> Result<std::collections::BTreeMap<String, i64>> {
    Ok(s.query(
        "SELECT module_id,invalidation_revision FROM deterministic_module_runtime",
        (),
        |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
    )?
    .into_iter()
    .collect())
}

/// The modules a queued build will rebuild. A refresh queues one job per profile and stage, run
/// stage by stage: the first profile's job finishing does not make a module ready while another
/// profile's is still to come, or Insights stop re-reading before the last profile is counted.
fn modules_still_queued(s: &Session<'_>) -> Result<std::collections::BTreeSet<&'static str>> {
    let kinds = s.query(
        "SELECT DISTINCT job_type FROM intelligence_jobs WHERE state='queued' AND job_type IN (?1,?2,?3,?4)",
        [
            RebuildMode::VisitDerive.job_type(),
            RebuildMode::DailyRollup.job_type(),
            RebuildMode::StructuralRebuild.job_type(),
            RebuildMode::FullRebuild.job_type(),
        ],
        |r| Ok(r.get::<String>(0)?),
    )?;
    let mut modules = std::collections::BTreeSet::new();
    for kind in kinds {
        modules.extend(RebuildMode::from_job_type(&kind)?.module_ids());
    }
    Ok(modules)
}

fn persist_ready_module_updates(
    connection: &Session<'_>,
    run_id: i64,
    built_at: Option<String>,
    module_ids: &[&str],
    notes: &[DerivedRuntimeNote],
    revisions: &std::collections::BTreeMap<String, i64>,
) -> Result<()> {
    let shared_notes = if notes.is_empty() {
        vec![DerivedRuntimeNote::new(
            crate::models::DERIVED_NOTE_MODULES_IN_SYNC,
            "Core Intelligence modules are in sync with the current derived plane.",
        )]
    } else {
        notes.to_vec()
    };
    let current = module_invalidation_revisions(connection)?;
    let pending = modules_still_queued(connection)?;
    let updates = module_ids
        .iter()
        .filter(|id| {
            current.get(**id).copied().unwrap_or(0) == revisions.get(**id).copied().unwrap_or(0)
                && !pending.contains(**id)
        })
        .map(|module_id| module_update(module_id, run_id, built_at.clone(), &shared_notes))
        .collect::<Vec<_>>();
    persist_deterministic_module_runtime_updates(connection, &updates)
}

fn module_update(
    module_id: &str,
    run_id: i64,
    built_at: Option<String>,
    notes: &[DerivedRuntimeNote],
) -> DeterministicModuleRuntimeUpdate {
    DeterministicModuleRuntimeUpdate {
        module_id: module_id.to_string(),
        status: "ready".to_string(),
        last_run_id: Some(run_id),
        last_built_at: built_at,
        last_invalidated_at: None,
        stale_reason: None,
        stale_reason_code: None,
        notes: notes.to_vec(),
    }
}
