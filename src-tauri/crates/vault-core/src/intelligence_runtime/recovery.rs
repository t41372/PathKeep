//! Queue recovery helpers.
//!
//! ## Responsibilities
//! - Requeue or cancel stale running jobs when leases expire or the process
//!   restarts unexpectedly.
//! - Keep one-time runtime recovery scoped per archive path so foreground
//!   snapshot loads do not repeatedly mutate queue state.

use super::*;
use crate::utils::now_rfc3339;
use rusqlite::{Connection, params};

pub(crate) fn requeue_running_enrichment_jobs(connection: &Connection) -> Result<usize> {
    ensure_intelligence_runtime_schema(connection)?;
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued', scheduled_at = ?1, started_at = NULL, finished_at = NULL,
             heartbeat_at = NULL, lease_owner = NULL, lease_expires_at = NULL,
             updated_at = ?1, last_error = NULL, cancellation_reason = NULL, stop_requested = 0
         WHERE job_type = ?2 AND state = 'running'",
        params![now, ENRICHMENT_JOB_TYPE],
    )?;
    Ok(updated)
}

pub(crate) fn recover_expired_intelligence_jobs(connection: &Connection) -> Result<()> {
    let now = now_rfc3339();
    let expired_at = Utc::now().to_rfc3339();
    connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'cancelled',
             finished_at = ?1,
             heartbeat_at = ?1,
             lease_owner = NULL,
             lease_expires_at = NULL,
             updated_at = ?1,
             last_error = NULL
         WHERE state = 'running'
           AND stop_requested = 1
           AND lease_expires_at IS NOT NULL
           AND lease_expires_at <= ?2",
        params![now, expired_at],
    )?;
    connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued',
             scheduled_at = ?1,
             started_at = NULL,
             finished_at = NULL,
             heartbeat_at = NULL,
             lease_owner = NULL,
             lease_expires_at = NULL,
             updated_at = ?1,
             last_error = 'PathKeep recovered an expired intelligence lease.',
             cancellation_reason = NULL,
             stop_requested = 0
         WHERE state = 'running'
           AND COALESCE(stop_requested, 0) = 0
           AND lease_expires_at IS NOT NULL
           AND lease_expires_at <= ?2",
        params![now, expired_at],
    )?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn requeue_running_enrichment_jobs_for_run(
    connection: &Connection,
    run_id: i64,
) -> Result<usize> {
    ensure_intelligence_runtime_schema(connection)?;
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued', scheduled_at = ?1, started_at = NULL, finished_at = NULL,
             heartbeat_at = NULL, lease_owner = NULL, lease_expires_at = NULL,
             updated_at = ?1, last_error = NULL, cancellation_reason = NULL, stop_requested = 0
         WHERE job_type = ?2 AND state = 'running' AND run_id = ?3",
        params![now, ENRICHMENT_JOB_TYPE, run_id],
    )?;
    Ok(updated)
}

pub(crate) fn recover_interrupted_deterministic_jobs(connection: &Connection) -> Result<usize> {
    let now = now_rfc3339();
    let mut updated = 0;
    for job_type in [
        VISIT_DERIVE_JOB_TYPE,
        DAILY_ROLLUP_JOB_TYPE,
        STRUCTURAL_REBUILD_JOB_TYPE,
        FULL_REBUILD_JOB_TYPE,
    ] {
        updated += connection.execute(
            "UPDATE intelligence_jobs
             SET state = 'queued',
                 priority = ?2,
                 scheduled_at = ?1,
                 updated_at = ?1,
                 started_at = NULL,
                 finished_at = NULL,
                 heartbeat_at = NULL,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 last_error = 'PathKeep restarted before this Core Intelligence job finished.',
                 cancellation_reason = NULL,
                 stop_requested = 0
             WHERE job_type = ?3
               AND state = 'running'",
            params![now, core_intelligence_job_priority(job_type), job_type],
        )?;
    }
    Ok(updated)
}

pub(crate) fn should_recover_runtime_jobs(archive_database_path: &Path) -> bool {
    let recovered_archives = RECOVERED_RUNTIME_ARCHIVES.get_or_init(|| Mutex::new(HashSet::new()));
    let mut recovered_archives =
        recovered_archives.lock().expect("runtime recovery archive registry lock");
    recovered_archives.insert(archive_database_path.display().to_string())
}
