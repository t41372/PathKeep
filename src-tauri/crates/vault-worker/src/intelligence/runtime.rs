//! Failure modes: workers exceed their lane, stale claims remain stranded or completion hides a newer refresh trigger.
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
//! - `crate::context` for unlocked config and archive vault access
//! - `crate::job_runtime` for bounded worker pool spawning
//! - `vault_core::intelligence_runtime` for queue state transitions and persisted artifacts
//!
//! ## Performance notes
//! - deterministic rebuild workers keep one priority lane plus bounded enrichment workers
//!   so expensive rebuilds do not monopolize a small machine
//! - long-running rebuild stages report incremental artifacts back to SQLite instead of
//!   buffering large progress state in memory

use super::{INTELLIGENCE_ENRICHMENT_WORKERS, INTELLIGENCE_PRIORITY_WORKERS};
use crate::context::load_unlocked_config;
use crate::job_runtime::maybe_spawn_worker_pool;
use anyhow::{Context, Result};
use serde_json::json;
use vault_core::{
    CoreIntelligenceQueueReport, CoreIntelligenceRebuildReport, CoreIntelligenceRebuildRequest,
    IntelligenceRuntimeSnapshot, cancel_intelligence_job, execute_enrichment_job_by_id,
    intelligence, intelligence_job_stop_requested,
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
pub(crate) fn maybe_spawn_intelligence_queue_drain(vault: &vault_core::Vault, queued_jobs: usize) {
    let config = vault.config();
    if config.ai.job_queue_paused || queued_jobs == 0 {
        return;
    }
    spawn_intelligence_queue_drain(vault.clone(), config.ai.job_queue_concurrency.max(1) as usize);
}

/// Drains deterministic rebuild and enrichment jobs through two bounded worker lanes.
///
/// The priority lane exclusively handles structural rebuild work, while the
/// enrichment lane can fan out up to `desired_workers - 1` jobs when allowed.
fn spawn_intelligence_queue_drain(vault: vault_core::Vault, desired_workers: usize) {
    maybe_spawn_worker_pool("pathkeep-intelligence-priority", &INTELLIGENCE_PRIORITY_WORKERS, 1, {
        let vault = vault.clone();
        move || loop {
            match drain_one_priority_intelligence_job(&vault) {
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
            if !continue_enrichment_queue_drain(drain_one_enrichment_intelligence_job(&vault)) {
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

pub(crate) fn drain_one_priority_intelligence_job(vault: &vault_core::Vault) -> Result<bool> {
    let config = load_unlocked_config(vault.paths()).context("load intelligence queue config")?;
    if !config.initialized || config.ai.job_queue_paused {
        return Ok(false);
    }
    let Some(job) =
        next_queued_intelligence_job(vault).context("load next intelligence queue job")?
    else {
        return Ok(false);
    };
    // The drain outlives the settings it started with; each build uses the ones saved now.
    let vault = vault.with_config(config);
    execute_core_intelligence_job(&vault, job.id, &job.job_type)
        .with_context(|| format!("execute intelligence queue job {}", job.id))?;
    // The Insights time zone was changed in Settings while the build ran in the old one.
    let now = load_unlocked_config(vault.paths()).context("load intelligence queue config")?;
    if now.insights_time_zone != vault.config().insights_time_zone {
        rebuild_in_chosen_zone(&vault.with_config(now))?;
    }
    Ok(true)
}

/// Queues an Insights build when the zone the next build would use is not the one the Insights
/// on screen are in: a zone was just chosen in Settings. Unlike [`resume_insights`] it builds also
/// after a failed or cancelled build, since choosing a zone asks for one.
pub(crate) fn rebuild_in_chosen_zone(vault: &vault_core::Vault) -> Result<()> {
    let zone = vault_core::insights_time_zone(vault)?;
    if zone.shown != zone.next {
        enqueue_deterministic_rebuild_job(
            vault,
            &CoreIntelligenceRebuildRequest::default(),
            "Insights are built again in the time zone chosen in Settings.",
        )?;
        maybe_spawn_intelligence_queue_drain(vault, 1);
    }
    Ok(())
}

pub(crate) fn drain_one_enrichment_intelligence_job(vault: &vault_core::Vault) -> Result<bool> {
    let config = load_unlocked_config(vault.paths()).context("load intelligence queue config")?;
    if !config.initialized || config.ai.job_queue_paused {
        return Ok(false);
    }
    let Some(job_id) =
        next_queued_enrichment_job(vault).context("load next queued enrichment job")?
    else {
        return Ok(false);
    };
    if let Err(error) = execute_enrichment_job_by_id(vault, job_id) {
        let message = format!("{error:#}");
        mark_intelligence_job_failed(vault, job_id, &message)
            .with_context(|| format!("persist enrichment queue failure for job {job_id}"))?;
        return Err(error).with_context(|| format!("execute enrichment queue job {job_id}"));
    }
    Ok(true)
}

/// Runs one deterministic Core Intelligence rebuild synchronously for the caller.
pub fn run_core_intelligence_now(
    vault: &vault_core::Vault,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceRebuildReport> {
    let config = load_unlocked_config(vault.paths())?;
    intelligence::run_core_intelligence(&vault.with_config(config), request)
}

/// Gets Insights going when an archive opens. A build the app quit in the middle of is queued
/// again by the store, but nothing ran the queue until the next backup or a visit to Settings.
/// An archive whose Insights were never built showed empty Insights, and an empty Home, until
/// someone pressed Rebuild; and after an update with new rules, Insights kept the old labels
/// until the next backup. The build decides how much to redo (`build_pipeline::prepare`): all of
/// it for new rules or none built yet, only the changed sites for a category the user set.
/// Forcing a full build here rebuilt 14.4M visits for one site's category.
pub(crate) fn resume_insights(vault: &vault_core::Vault) -> Result<()> {
    if let Some(reason) = vault_core::intelligence_runtime::insights_build_needed(vault)? {
        enqueue_deterministic_rebuild_job(
            vault,
            &CoreIntelligenceRebuildRequest::default(),
            reason,
        )?;
    }
    maybe_spawn_intelligence_queue_drain(vault, 1);
    Ok(())
}

/// Queues one deterministic rebuild and starts background draining if allowed.
pub fn queue_core_intelligence_rebuild(
    vault: &vault_core::Vault,
    request: &CoreIntelligenceRebuildRequest,
) -> Result<CoreIntelligenceQueueReport> {
    let config = load_unlocked_config(vault.paths())?;
    let job_id = enqueue_deterministic_rebuild_job(
        vault,
        request,
        "User requested a Core Intelligence rebuild from the UI.",
    )?;
    let state = vault.store().read(vault_store::Db::Intelligence, move |s| {
        s.query_one("SELECT state FROM intelligence_jobs WHERE id=?1", [job_id], |r| {
            Ok(r.get::<String>(0)?)
        })
    })?;
    maybe_spawn_intelligence_queue_drain(vault, 1);
    Ok(CoreIntelligenceQueueReport {
        job_id,
        state: state.clone(),
        notes: vec![core_intelligence_queue_note(job_id, &state, config.ai.job_queue_paused)],
    })
}

/// Sets a site's category from its Insights page (`None`: back to PathKeep's lists) and starts
/// building Insights again, which sorts only that site's pages in (`build_reclassify`).
pub fn set_site_category(
    vault: &vault_core::Vault,
    site: &str,
    category: Option<&str>,
) -> Result<CoreIntelligenceQueueReport> {
    vault_core::set_site_category(vault, site, category)?;
    queue_core_intelligence_rebuild(vault, &CoreIntelligenceRebuildRequest::default())
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
    vault: &vault_core::Vault,
    job_id: i64,
    job_type: &str,
) -> Result<bool> {
    let Some(payload) = claim_core_intelligence_job(vault, job_id)? else {
        return Ok(false);
    };
    if payload.job_type != job_type {
        let error = anyhow::anyhow!(
            "Core Intelligence job {job_id} payload type '{}' did not match queued type '{job_type}'.",
            payload.job_type
        );
        return record_core_intelligence_job_error(vault, job_id, error);
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
        bail_if_core_intelligence_job_cancelled(vault, job_id, detail)
    };
    cancel_requested(&format!("{job_label} was cancelled before work started."))?;
    let _ = update_intelligence_job_artifact(
        vault,
        job_id,
        &json!({
            "kind": job_type,
            "phase": "queued",
            "detail": format!("Preparing a {job_label} for {initial_profile}."),
            "progressPercent": 0.0
        }),
    );
    match intelligence::run_core_intelligence_job_type_with_progress(
        vault,
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
            let _ = update_intelligence_job_artifact(vault, job_id, &artifact);
            cancel_requested(&format!(
                "{job_label} was cancelled after the latest progress update."
            ))?;
            Ok(())
        },
    ) {
        Ok(report) => {
            finish_core_intelligence_job_success(vault, job_id, job_type, job_label, report)
        }
        Err(error) => record_core_intelligence_job_error(vault, job_id, error),
    }
}

fn finish_core_intelligence_job_success(
    vault: &vault_core::Vault,
    job_id: i64,
    job_type: &str,
    job_label: &str,
    report: CoreIntelligenceRebuildReport,
) -> Result<bool> {
    if cancel_core_intelligence_job_if_requested(vault, job_id)? {
        return Ok(true);
    }
    finalize_core_intelligence_job_success(
        vault,
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
            "fallbackReasonCode": report.fallback_reason_code,
            "notes": report.notes,
        }),
    )?;
    Ok(true)
}

fn bail_if_core_intelligence_job_cancelled(
    vault: &vault_core::Vault,
    job_id: i64,
    detail: &str,
) -> Result<()> {
    if intelligence_job_stop_requested(vault, job_id)? {
        let _ = mark_running_intelligence_job_cancelled(vault, job_id, "cancelled from UI");
        anyhow::bail!(detail.to_string());
    }
    Ok(())
}

fn cancel_core_intelligence_job_if_requested(
    vault: &vault_core::Vault,
    job_id: i64,
) -> Result<bool> {
    if intelligence_job_stop_requested(vault, job_id)? {
        let _ = mark_running_intelligence_job_cancelled(vault, job_id, "cancelled from UI");
        return Ok(true);
    }
    Ok(false)
}

fn finalize_core_intelligence_job_success(
    vault: &vault_core::Vault,
    job_id: i64,
    artifact: serde_json::Value,
) -> Result<()> {
    if !mark_intelligence_job_succeeded(vault, job_id, &artifact)? {
        let _ = mark_running_intelligence_job_cancelled(vault, job_id, "cancelled from UI");
    }
    Ok(())
}

fn record_core_intelligence_job_error(
    vault: &vault_core::Vault,
    job_id: i64,
    error: anyhow::Error,
) -> Result<bool> {
    let message = error.to_string();
    if message.contains("cancelled") {
        let _ = mark_running_intelligence_job_cancelled(vault, job_id, "cancelled from UI");
        return Ok(true);
    }
    mark_intelligence_job_failed(vault, job_id, &message)?;
    Err(error)
}

/// Loads the Settings-facing runtime snapshot and opportunistically restarts drains.
pub fn load_intelligence_runtime_snapshot(
    vault: &vault_core::Vault,
) -> Result<IntelligenceRuntimeSnapshot> {
    let mut config = vault_core::load_config(vault.paths())?;
    crate::context::hydrate_derived_config_state(&mut config);
    let snapshot = load_intelligence_runtime(vault)?;
    maybe_spawn_intelligence_queue_drain(vault, snapshot.queue.queued);
    Ok(snapshot)
}

/// Retries one failed runtime job and reloads the fresh queue snapshot.
pub fn retry_intelligence_job_now(
    vault: &vault_core::Vault,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let mut config = vault_core::load_config(vault.paths())?;
    crate::context::hydrate_derived_config_state(&mut config);
    retry_intelligence_job(vault, job_id)?;
    let snapshot = load_intelligence_runtime(vault)?;
    maybe_spawn_intelligence_queue_drain(vault, snapshot.queue.queued);
    Ok(snapshot)
}

/// Cancels one queued runtime job and returns the updated runtime snapshot.
pub fn cancel_intelligence_job_now(
    vault: &vault_core::Vault,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let mut config = vault_core::load_config(vault.paths())?;
    crate::context::hydrate_derived_config_state(&mut config);
    cancel_intelligence_job(vault, job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vault_core::{AppConfig, test_support::native::TestDatabase};
    use vault_store::{Db, params_owned as params};

    use vault_core::ArchiveMode;

    fn runtime_connection() -> (vault_core::Vault, TestDatabase) {
        let vault = vault_core::Vault::open_temp().unwrap().with_config(AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..Default::default()
        });
        vault_core::save_config(vault.paths(), vault.config()).unwrap();
        let db = TestDatabase::new(&vault, Db::Intelligence);
        (vault, db)
    }

    #[test]
    fn drains_surface_and_quarantine_malformed_payloads() {
        let (vault, connection) = runtime_connection();
        let now = chrono::Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs (
                    job_type, state, priority, attempt, dedupe_key, payload_json, artifact_json,
                    created_at, scheduled_at, updated_at
                 ) VALUES (?1, 'queued', 1, 0, 'malformed-drain-core', '{', '{}', ?2, ?2, ?2)",
                params![VISIT_DERIVE_JOB_TYPE, now],
            )
            .expect("insert malformed core job");
        let core_id = connection
            .query_one("SELECT MAX(id) FROM intelligence_jobs", (), |r| Ok(r.get::<i64>(0)?))
            .unwrap();

        let core_error = drain_one_priority_intelligence_job(&vault)
            .expect_err("core drain must surface executor failure");
        assert!(core_error.to_string().contains("execute intelligence queue job"));

        connection
            .execute(
                "INSERT INTO intelligence_jobs (
                    job_type, plugin_id, state, priority, attempt, dedupe_key, payload_json,
                    artifact_json, created_at, scheduled_at, updated_at
                 ) VALUES ('enrichment-plugin', ?1, 'queued', 1, 0,
                    'malformed-drain-enrichment', '{', '{}', ?2, ?2, ?2)",
                params![vault_core::TITLE_NORMALIZATION_PLUGIN_ID, now],
            )
            .expect("insert malformed enrichment job");
        let enrichment_id = connection
            .query_one("SELECT MAX(id) FROM intelligence_jobs", (), |r| Ok(r.get::<i64>(0)?))
            .unwrap();

        let enrichment_error = drain_one_enrichment_intelligence_job(&vault)
            .expect_err("enrichment drain must surface executor failure");
        assert!(enrichment_error.to_string().contains("execute enrichment queue job"));
        for job_id in [core_id, enrichment_id] {
            let (state, last_error): (String, String) = connection
                .query_row(
                    "SELECT state, last_error FROM intelligence_jobs WHERE id = ?1",
                    [job_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("failed queue row");
            assert_eq!(state, "failed");
            assert!(!last_error.is_empty());
        }
        assert!(
            !drain_one_enrichment_intelligence_job(&vault)
                .expect("malformed row is no longer runnable"),
            "a quarantined malformed row must not tight-loop"
        );
    }
}
