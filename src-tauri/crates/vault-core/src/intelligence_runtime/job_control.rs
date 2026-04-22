//! Runtime job state transition helpers.
//!
//! ## Responsibilities
//! - Persist running-job success, failure, cancellation, and heartbeat updates.
//! - Apply UI-facing retry/cancel commands while keeping job-state rules
//!   consistent with the persisted queue contract.

use super::*;
use crate::{archive::open_intelligence_connection, config::ProjectPaths, utils::now_rfc3339};
use rusqlite::{Connection, params};
use serde_json::Value;

/// Marks one intelligence job as succeeded.
pub fn mark_intelligence_job_succeeded(
    connection: &Connection,
    job_id: i64,
    artifact: &Value,
) -> Result<bool> {
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'succeeded', artifact_json = ?1, finished_at = ?2, updated_at = ?2,
             heartbeat_at = ?2, lease_owner = NULL, lease_expires_at = NULL,
             last_error = NULL, cancellation_reason = NULL
         WHERE id = ?3
           AND state = 'running'
           AND stop_requested = 0",
        params![serde_json::to_string(artifact)?, now, job_id],
    )?;
    Ok(updated == 1)
}

/// Updates one running intelligence job with progress/heartbeat metadata.
pub fn update_intelligence_job_artifact(
    connection: &Connection,
    job_id: i64,
    artifact: &Value,
) -> Result<()> {
    let now = now_rfc3339();
    connection.execute(
        "UPDATE intelligence_jobs
         SET artifact_json = ?1,
             updated_at = ?2,
             heartbeat_at = ?2,
             lease_expires_at = ?3
         WHERE id = ?4
           AND state = 'running'",
        params![
            serde_json::to_string(artifact)?,
            now,
            lease_expires_at(INTELLIGENCE_JOB_LEASE_SECONDS),
            job_id,
        ],
    )?;
    Ok(())
}

/// Marks one intelligence job as failed.
pub fn mark_intelligence_job_failed(
    connection: &Connection,
    job_id: i64,
    error: &str,
) -> Result<bool> {
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'failed',
             finished_at = ?1,
             heartbeat_at = ?1,
             lease_owner = NULL,
             lease_expires_at = NULL,
             updated_at = ?1,
             last_error = ?2
         WHERE id = ?3
           AND state = 'running'
           AND stop_requested = 0",
        params![now, error, job_id],
    )?;
    Ok(updated == 1)
}

/// Marks one running intelligence job as cancelled after its worker observes a stop request.
pub fn mark_running_intelligence_job_cancelled(
    connection: &Connection,
    job_id: i64,
    reason: &str,
) -> Result<bool> {
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'cancelled',
             finished_at = ?1,
             heartbeat_at = ?1,
             lease_owner = NULL,
             lease_expires_at = NULL,
             updated_at = ?1,
             last_error = NULL,
             cancellation_reason = ?2
         WHERE id = ?3
           AND state = 'running'",
        params![now, reason, job_id],
    )?;
    Ok(updated == 1)
}

/// Retries one deterministic intelligence job if its current state allows it.
pub fn retry_intelligence_job(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_intelligence_runtime_schema(&connection)?;
    let state = super::snapshot::load_job_state(&connection, job_id)?;
    if !matches!(state.as_str(), "failed" | "cancelled") {
        anyhow::bail!("Intelligence job {job_id} is in state '{state}' and cannot be retried.");
    }
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued', scheduled_at = ?1, updated_at = ?1, started_at = NULL,
             finished_at = NULL, heartbeat_at = NULL, lease_owner = NULL,
             lease_expires_at = NULL, last_error = NULL, cancellation_reason = NULL,
             stop_requested = 0
         WHERE id = ?2",
        params![now, job_id],
    )?;
    if updated == 0 {
        anyhow::bail!("Intelligence job {job_id} could not be retried.");
    }
    super::snapshot::load_intelligence_runtime(paths, config, key)
}

/// Cancels one deterministic intelligence job if its current state allows it.
pub fn cancel_intelligence_job(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_intelligence_runtime_schema(&connection)?;
    let state = super::snapshot::load_job_state(&connection, job_id)?;
    if !matches!(state.as_str(), "queued" | "running") {
        anyhow::bail!("Intelligence job {job_id} is in state '{state}' and cannot be cancelled.");
    }
    let now = now_rfc3339();
    let updated = match state.as_str() {
        "queued" => connection.execute(
            "UPDATE intelligence_jobs
             SET state = 'cancelled',
                 finished_at = ?1,
                 heartbeat_at = NULL,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 updated_at = ?1,
                 last_error = NULL,
                 cancellation_reason = 'cancelled from UI',
                 stop_requested = 1
             WHERE id = ?2
               AND state = 'queued'",
            params![now, job_id],
        )?,
        "running" => connection.execute(
            "UPDATE intelligence_jobs
             SET updated_at = ?1,
                 cancellation_reason = 'cancelled from UI',
                 stop_requested = 1
             WHERE id = ?2
               AND state = 'running'",
            params![now, job_id],
        )?,
        _ => 0,
    };
    if updated == 0 {
        anyhow::bail!("Intelligence job {job_id} could not be cancelled.");
    }
    super::snapshot::load_intelligence_runtime(paths, config, key)
}
