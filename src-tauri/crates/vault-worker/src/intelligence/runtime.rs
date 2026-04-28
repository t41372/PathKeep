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
use anyhow::{Context, Result};
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
            match drain_one_priority_intelligence_job(&paths, session_database_key.as_deref()) {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    eprintln!("PathKeep could not drain intelligence queue work: {error:#}");
                    break;
                }
            }
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
            if !continue_enrichment_queue_drain(drain_one_enrichment_intelligence_job(
                &paths,
                session_database_key.as_deref(),
            )) {
                break;
            }
        },
    );
}

fn continue_enrichment_queue_drain(result: Result<bool>) -> bool {
    match result {
        Ok(true) => true,
        Ok(false) => false,
        Err(error) => {
            eprintln!("PathKeep could not drain enrichment queue work: {error:#}");
            false
        }
    }
}

pub(crate) fn drain_one_priority_intelligence_job(
    paths: &vault_core::ProjectPaths,
    session_database_key: Option<&str>,
) -> Result<bool> {
    let config = load_unlocked_config(paths).context("load intelligence queue config")?;
    if !config.initialized || config.ai.job_queue_paused {
        return Ok(false);
    }
    let connection = ai_archive_connection(paths, &config, session_database_key)
        .context("open archive for intelligence queue work")?;
    let Some(job) =
        next_queued_intelligence_job(&connection).context("load next intelligence queue job")?
    else {
        return Ok(false);
    };
    let _ =
        execute_core_intelligence_job(paths, &config, session_database_key, job.id, &job.job_type);
    Ok(true)
}

pub(crate) fn drain_one_enrichment_intelligence_job(
    paths: &vault_core::ProjectPaths,
    session_database_key: Option<&str>,
) -> Result<bool> {
    let config = load_unlocked_config(paths).context("load intelligence queue config")?;
    if !config.initialized || config.ai.job_queue_paused {
        return Ok(false);
    }
    let connection = ai_archive_connection(paths, &config, session_database_key)
        .context("open archive for intelligence queue work")?;
    let Some(job_id) =
        next_queued_enrichment_job(&connection).context("load next queued enrichment job")?
    else {
        return Ok(false);
    };
    let _ = execute_enrichment_job_by_id(paths, &connection, job_id);
    Ok(true)
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
        notes: vec![core_intelligence_queue_note(job_id, &state, config.ai.job_queue_paused)],
    })
}

fn core_intelligence_queue_note(job_id: i64, state: &str, queue_paused: bool) -> String {
    if queue_paused {
        format!("Queued Core Intelligence job {} while the runtime queue is paused.", job_id)
    } else if state == "running" {
        format!("Core Intelligence job {} is already running in the background.", job_id)
    } else {
        format!(
            "Queued Core Intelligence job {}. PathKeep is processing it in the background.",
            job_id
        )
    }
}

/// Executes one claimed deterministic rebuild job and writes progress artifacts back to SQLite.
///
/// This is the worker-side bridge between the persisted queue row and the
/// rebuild code in `vault_core::intelligence`. It keeps progress snapshots and
/// cancellation handling explicit so the UI can render honest job state.
pub(crate) fn execute_core_intelligence_job(
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
        bail_if_core_intelligence_job_cancelled(&connection, job_id, detail)
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
            finish_core_intelligence_job_success(&connection, job_id, job_type, job_label, report)
        }
        Err(error) => record_core_intelligence_job_error(&connection, job_id, error),
    }
}

fn finish_core_intelligence_job_success(
    connection: &rusqlite::Connection,
    job_id: i64,
    job_type: &str,
    job_label: &str,
    report: CoreIntelligenceRebuildReport,
) -> Result<bool> {
    if cancel_core_intelligence_job_if_requested(connection, job_id)? {
        return Ok(true);
    }
    finalize_core_intelligence_job_success(
        connection,
        job_id,
        json!({
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
    )?;
    Ok(true)
}

fn bail_if_core_intelligence_job_cancelled(
    connection: &rusqlite::Connection,
    job_id: i64,
    detail: &str,
) -> Result<()> {
    if intelligence_job_stop_requested(connection, job_id)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job_id, "cancelled from UI");
        anyhow::bail!(detail.to_string());
    }
    Ok(())
}

fn cancel_core_intelligence_job_if_requested(
    connection: &rusqlite::Connection,
    job_id: i64,
) -> Result<bool> {
    if intelligence_job_stop_requested(connection, job_id)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job_id, "cancelled from UI");
        return Ok(true);
    }
    Ok(false)
}

fn finalize_core_intelligence_job_success(
    connection: &rusqlite::Connection,
    job_id: i64,
    artifact: serde_json::Value,
) -> Result<()> {
    if !mark_intelligence_job_succeeded(connection, job_id, &artifact)? {
        let _ = mark_running_intelligence_job_cancelled(connection, job_id, "cancelled from UI");
    }
    Ok(())
}

fn record_core_intelligence_job_error(
    connection: &rusqlite::Connection,
    job_id: i64,
    error: anyhow::Error,
) -> Result<bool> {
    let message = error.to_string();
    if message.contains("cancelled") {
        let _ = mark_running_intelligence_job_cancelled(connection, job_id, "cancelled from UI");
        return Ok(true);
    }
    mark_intelligence_job_failed(connection, job_id, &message)?;
    Err(error)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vault_core::{
        ArchiveMode, archive::open_intelligence_connection, config::project_paths_with_root,
    };

    fn runtime_connection() -> (tempfile::TempDir, rusqlite::Connection) {
        let root = tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let connection =
            open_intelligence_connection(&paths, &config, None).expect("runtime connection");
        (root, connection)
    }

    fn insert_runtime_job(
        connection: &rusqlite::Connection,
        state: &str,
        stop_requested: bool,
    ) -> i64 {
        let now = chrono::Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs (
                    job_type, state, priority, attempt, dedupe_key, payload_json, artifact_json,
                    created_at, scheduled_at, started_at, updated_at, stop_requested
                 )
                 VALUES (?1, ?2, 1, 0, ?3, '{}', '{}', ?4, ?4, ?4, ?4, ?5)",
                rusqlite::params![
                    VISIT_DERIVE_JOB_TYPE,
                    state,
                    format!(
                        "runtime-helper:{state}:{stop_requested}:{}",
                        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
                    ),
                    now,
                    i64::from(stop_requested),
                ],
            )
            .expect("insert runtime job");
        connection.last_insert_rowid()
    }

    #[test]
    fn runtime_helpers_cover_queue_notes_and_cancellation_branches() {
        assert!(
            core_intelligence_queue_note(1, "queued", true).contains("runtime queue is paused")
        );
        assert!(core_intelligence_queue_note(2, "running", false).contains("already running"));
        assert!(core_intelligence_queue_note(3, "queued", false).contains("processing it"));
        assert!(continue_enrichment_queue_drain(Ok(true)));
        assert!(!continue_enrichment_queue_drain(Ok(false)));
        assert!(!continue_enrichment_queue_drain(Err(anyhow::anyhow!("drain failed"))));

        let (_root, connection) = runtime_connection();
        let stopped_id = insert_runtime_job(&connection, "running", true);
        let stopped_error = bail_if_core_intelligence_job_cancelled(
            &connection,
            stopped_id,
            "cancelled before test",
        )
        .expect_err("stop request should bail");
        assert!(stopped_error.to_string().contains("cancelled before test"));
        let stopped_state: String = connection
            .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [stopped_id], |row| {
                row.get(0)
            })
            .expect("stopped state");
        assert_eq!(stopped_state, "cancelled");

        let post_success_id = insert_runtime_job(&connection, "running", true);
        assert!(
            cancel_core_intelligence_job_if_requested(&connection, post_success_id)
                .expect("post success cancellation")
        );
        let finish_cancelled_id = insert_runtime_job(&connection, "running", true);
        assert!(
            finish_core_intelligence_job_success(
                &connection,
                finish_cancelled_id,
                VISIT_DERIVE_JOB_TYPE,
                "visit-derived facts refresh",
                CoreIntelligenceRebuildReport::default(),
            )
            .expect("finish helper handles cancellation")
        );

        let stale_id = insert_runtime_job(&connection, "queued", false);
        finalize_core_intelligence_job_success(
            &connection,
            stale_id,
            json!({"phase": "completed"}),
        )
        .expect("stale success finalization should not fail");
        let stale_state: String = connection
            .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [stale_id], |row| {
                row.get(0)
            })
            .expect("stale state");
        assert_eq!(stale_state, "queued");

        let cancelled_error_id = insert_runtime_job(&connection, "running", false);
        assert!(
            record_core_intelligence_job_error(
                &connection,
                cancelled_error_id,
                anyhow::anyhow!("cancelled by test"),
            )
            .expect("cancelled error is terminal success")
        );
        let cancelled_error_state: String = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE id = ?1",
                [cancelled_error_id],
                |row| row.get(0),
            )
            .expect("cancelled error state");
        assert_eq!(cancelled_error_state, "cancelled");
    }
}
