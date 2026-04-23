//! Worker-side deterministic intelligence runtime orchestration.
//!
//! ## Responsibilities
//! - own background queue draining for deterministic rebuild and enrichment jobs
//! - expose "run now", retry, cancel, and runtime snapshot helpers to the desktop facade
//! - persist job progress artifacts so Settings and Jobs surfaces can stay honest
//!
//! ## Not responsible for
//! - AI provider search/assistant/index queue work
//! - route-level read-model passthroughs for Core Intelligence pages
//! - deterministic rebuild algorithms or SQLite schema owned by `vault-core`
//!
//! ## Dependencies
//! - `crate::context` for unlocked config and archive connection access
//! - `crate::job_runtime` for bounded worker pool spawning
//! - `vault_core::intelligence_runtime` for queue state transitions and persisted artifacts
//!
//! ## Performance notes
//! - deterministic rebuild workers keep one priority lane plus bounded enrichment workers
//!   so expensive rebuilds do not monopolize a small machine
//! - long-running rebuild stages report incremental artifacts back to SQLite instead of
//!   buffering large progress state in memory

use super::{INTELLIGENCE_ENRICHMENT_WORKERS, INTELLIGENCE_PRIORITY_WORKERS};
use crate::context::{ai_archive_connection, load_unlocked_config};
use crate::job_runtime::maybe_spawn_worker_pool;
use anyhow::Result;
use serde_json::json;
use vault_core::{
    AppConfig, CoreIntelligenceQueueReport, CoreIntelligenceRebuildReport,
    CoreIntelligenceRebuildRequest, IntelligenceRuntimeSnapshot, cancel_intelligence_job,
    execute_enrichment_job_by_id, intelligence, intelligence_job_stop_requested,
    intelligence_runtime::{
        DAILY_ROLLUP_JOB_TYPE, STRUCTURAL_REBUILD_JOB_TYPE, VISIT_DERIVE_JOB_TYPE,
        claim_core_intelligence_job, enqueue_deterministic_rebuild_job,
        mark_intelligence_job_failed, mark_intelligence_job_succeeded,
        mark_running_intelligence_job_cancelled, next_queued_enrichment_job,
        next_queued_intelligence_job, update_intelligence_job_artifact,
    },
    load_intelligence_runtime, retry_intelligence_job,
};

/// Starts background deterministic intelligence workers when queued work exists.
///
/// Archive import, backup, and Settings surfaces all call this through the same
/// helper so paused-state checks and concurrency rules stay consistent.
pub(crate) fn maybe_spawn_intelligence_queue_drain(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    queued_jobs: usize,
) {
    if config.ai.job_queue_paused || queued_jobs == 0 {
        return;
    }
    spawn_intelligence_queue_drain(
        paths.clone(),
        config.ai.job_queue_concurrency.max(1) as usize,
        session_database_key.map(ToOwned::to_owned),
    );
}

/// Drains deterministic rebuild and enrichment jobs through two bounded worker lanes.
///
/// The priority lane exclusively handles structural rebuild work, while the
/// enrichment lane can fan out up to `desired_workers - 1` jobs when allowed.
fn spawn_intelligence_queue_drain(
    paths: vault_core::ProjectPaths,
    desired_workers: usize,
    session_database_key: Option<String>,
) {
    maybe_spawn_worker_pool("pathkeep-intelligence-priority", &INTELLIGENCE_PRIORITY_WORKERS, 1, {
        let paths = paths.clone();
        let session_database_key = session_database_key.clone();
        move || loop {
            let config = match load_unlocked_config(&paths) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("PathKeep could not load intelligence queue config: {error:#}");
                    break;
                }
            };
            if !config.initialized || config.ai.job_queue_paused {
                break;
            }
            let connection = match ai_archive_connection(
                &paths,
                &config,
                session_database_key.as_deref(),
            ) {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!(
                        "PathKeep could not open the archive for intelligence queue work: {error:#}"
                    );
                    break;
                }
            };
            let Some(job) = (match next_queued_intelligence_job(&connection) {
                Ok(job) => job,
                Err(error) => {
                    eprintln!("PathKeep could not load the next intelligence queue job: {error:#}");
                    break;
                }
            }) else {
                break;
            };

            let _ = execute_core_intelligence_job(
                &paths,
                &config,
                session_database_key.as_deref(),
                job.id,
                &job.job_type,
            );
        }
    });

    let enrichment_workers = desired_workers.saturating_sub(1);
    if enrichment_workers == 0 {
        return;
    }
    maybe_spawn_worker_pool(
        "pathkeep-intelligence-enrichment",
        &INTELLIGENCE_ENRICHMENT_WORKERS,
        enrichment_workers,
        move || loop {
            let config = match load_unlocked_config(&paths) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("PathKeep could not load intelligence queue config: {error:#}");
                    break;
                }
            };
            if !config.initialized || config.ai.job_queue_paused {
                break;
            }
            let connection = match ai_archive_connection(
                &paths,
                &config,
                session_database_key.as_deref(),
            ) {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!(
                        "PathKeep could not open the archive for intelligence queue work: {error:#}"
                    );
                    break;
                }
            };
            let Some(job_id) = (match next_queued_enrichment_job(&connection) {
                Ok(job) => job,
                Err(error) => {
                    eprintln!("PathKeep could not load the next queued enrichment job: {error:#}");
                    break;
                }
            }) else {
                break;
            };
            let _ = execute_enrichment_job_by_id(&paths, &connection, job_id);
        },
    );
}

/// Runs one deterministic Core Intelligence rebuild synchronously for the caller.
pub fn run_core_intelligence_now(
    session_database_key: Option<&str>,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceRebuildReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    intelligence::run_core_intelligence(&paths, &config, session_database_key, request)
}

/// Queues one deterministic rebuild and starts background draining if allowed.
pub fn queue_core_intelligence_rebuild(
    session_database_key: Option<&str>,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceQueueReport> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    let connection = ai_archive_connection(&paths, &config, session_database_key)?;
    let job_id = enqueue_deterministic_rebuild_job(
        &connection,
        request,
        "User requested a Core Intelligence rebuild from the UI.",
    )?;
    let state = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            row.get::<_, String>(0)
        })
        .unwrap_or_else(|_| "queued".to_string());
    maybe_spawn_intelligence_queue_drain(&paths, &config, session_database_key, 1);
    Ok(CoreIntelligenceQueueReport {
        job_id,
        state: state.clone(),
        notes: vec![if config.ai.job_queue_paused {
            format!("Queued Core Intelligence job {} while the runtime queue is paused.", job_id)
        } else if state == "running" {
            format!("Core Intelligence job {} is already running in the background.", job_id)
        } else {
            format!(
                "Queued Core Intelligence job {}. PathKeep is processing it in the background.",
                job_id
            )
        }],
    })
}

/// Executes one claimed deterministic rebuild job and writes progress artifacts back to SQLite.
///
/// This is the worker-side bridge between the persisted queue row and the
/// rebuild code in `vault_core::intelligence`. It keeps progress snapshots and
/// cancellation handling explicit so the UI can render honest job state.
fn execute_core_intelligence_job(
    paths: &vault_core::ProjectPaths,
    config: &AppConfig,
    session_database_key: Option<&str>,
    job_id: i64,
    job_type: &str,
) -> Result<bool> {
    let connection = ai_archive_connection(paths, config, session_database_key)?;
    let Some(payload) = claim_core_intelligence_job(&connection, job_id)? else {
        return Ok(false);
    };
    if payload.job_type != job_type {
        return Ok(false);
    }
    let initial_profile =
        payload.request.profile_id.as_deref().unwrap_or("all profiles").to_string();
    let job_label = match job_type {
        VISIT_DERIVE_JOB_TYPE => "visit-derived facts refresh",
        DAILY_ROLLUP_JOB_TYPE => "daily rollup refresh",
        STRUCTURAL_REBUILD_JOB_TYPE => "structural entity rebuild",
        _ => "Core Intelligence rebuild",
    };
    let cancel_requested = |detail: &str| -> Result<()> {
        if intelligence_job_stop_requested(&connection, job_id)? {
            let _ =
                mark_running_intelligence_job_cancelled(&connection, job_id, "cancelled from UI");
            anyhow::bail!(detail.to_string());
        }
        Ok(())
    };
    cancel_requested(&format!("{job_label} was cancelled before work started."))?;
    let _ = update_intelligence_job_artifact(
        &connection,
        job_id,
        &json!({
            "kind": job_type,
            "phase": "queued",
            "detail": format!("Preparing a {job_label} for {initial_profile}."),
            "progressPercent": 0.0
        }),
    );
    match intelligence::run_core_intelligence_job_type_with_progress(
        paths,
        config,
        session_database_key,
        job_type,
        &payload.request,
        |progress| {
            cancel_requested(&format!(
                "{job_label} was cancelled while progress was being reported."
            ))?;
            let artifact = json!({
                "kind": job_type,
                "phase": progress.phase,
                "detail": progress.detail,
                "processedItems": progress.processed_items,
                "totalItems": progress.total_items,
                "progressPercent": progress.progress_percent,
            });
            let _ = update_intelligence_job_artifact(&connection, job_id, &artifact);
            cancel_requested(&format!(
                "{job_label} was cancelled after the latest progress update."
            ))?;
            Ok(())
        },
    ) {
        Ok(report) => {
            if intelligence_job_stop_requested(&connection, job_id)? {
                let _ = mark_running_intelligence_job_cancelled(
                    &connection,
                    job_id,
                    "cancelled from UI",
                );
                return Ok(true);
            }
            if !mark_intelligence_job_succeeded(
                &connection,
                job_id,
                &json!({
                    "kind": job_type,
                    "phase": "completed",
                    "detail": format!(
                        "{} finished with {} visits processed.",
                        job_label,
                        report.processed_visits,
                    ),
                    "processedItems": report.processed_visits,
                    "totalItems": report.processed_visits,
                    "progressPercent": 100.0,
                    "processedVisits": report.processed_visits,
                    "sessionCount": report.sessions,
                    "trailCount": report.search_trails,
                    "queryFamilyCount": report.query_families,
                    "refindPageCount": report.refind_pages,
                    "executionMode": report.execution_mode,
                    "affectedProfiles": report.affected_profiles,
                    "dirtyVisitCount": report.dirty_visit_count,
                    "dirtyDateKeys": report.dirty_date_keys,
                    "fallbackReason": report.fallback_reason,
                    "notes": report.notes,
                }),
            )? {
                let _ = mark_running_intelligence_job_cancelled(
                    &connection,
                    job_id,
                    "cancelled from UI",
                );
            }
            Ok(true)
        }
        Err(error) => {
            if error.to_string().contains("cancelled") {
                let _ = mark_running_intelligence_job_cancelled(
                    &connection,
                    job_id,
                    "cancelled from UI",
                );
                return Ok(true);
            }
            mark_intelligence_job_failed(&connection, job_id, &error.to_string())?;
            Err(error)
        }
    }
}

/// Loads the Settings-facing runtime snapshot and opportunistically restarts drains.
pub fn load_intelligence_runtime_snapshot(
    session_database_key: Option<&str>,
) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut config = vault_core::load_config(&paths)?;
    crate::context::hydrate_derived_config_state(&mut config);
    let snapshot = load_intelligence_runtime(&paths, &config, session_database_key)?;
    maybe_spawn_intelligence_queue_drain(
        &paths,
        &config,
        session_database_key,
        snapshot.queue.queued,
    );
    Ok(snapshot)
}

/// Retries one failed runtime job and reloads the fresh queue snapshot.
pub fn retry_intelligence_job_now(
    session_database_key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut config = vault_core::load_config(&paths)?;
    crate::context::hydrate_derived_config_state(&mut config);
    retry_intelligence_job(&paths, &config, session_database_key, job_id)?;
    let snapshot = load_intelligence_runtime(&paths, &config, session_database_key)?;
    maybe_spawn_intelligence_queue_drain(
        &paths,
        &config,
        session_database_key,
        snapshot.queue.queued,
    );
    Ok(snapshot)
}

/// Cancels one queued runtime job and returns the updated runtime snapshot.
pub fn cancel_intelligence_job_now(
    session_database_key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault_core::project_paths()?;
    let mut config = vault_core::load_config(&paths)?;
    crate::context::hydrate_derived_config_state(&mut config);
    cancel_intelligence_job(&paths, &config, session_database_key, job_id)
}
