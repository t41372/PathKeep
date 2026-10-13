//! Not responsible for: archive ingestion, provider execution or connection ownership.
//! Failure modes: duplicate claims, lost recovery progress, overwritten cancellation,
//! hidden jobs, or a read initializes storage. Tests: runtime native queue regressions.
//! Queue recovery helpers.
//!
//! ## Responsibilities
//! - Requeue or cancel stale running jobs when leases expire or the process
//!   restarts unexpectedly.
//! - Keep one-time runtime recovery scoped per archive path so foreground
//!   snapshot loads do not repeatedly mutate queue state.

use super::*;
use crate::utils::now_rfc3339;
use vault_store::{Session, params_owned as params};

pub(crate) fn recover_expired_intelligence_jobs(connection: &Session<'_>) -> Result<()> {
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
