//! Responsibilities: route durable AI queue operations through the Vault owner.
//! Not responsible for: providers, embeddings or schema installation.
//! Failure modes: claims lost after restart, finished work repeated, cancellation
//! overwritten, or jobs hidden beyond a status preview. Tests: tests and ai_queue_turso.

pub(crate) mod store;
#[cfg(test)]
mod tests;
mod types;
use crate::{
    Vault,
    models::{
        AiAssistantRequest, AiIndexRequest, AiQueueJob, AiQueueJobState, AiQueueJobType,
        AiQueueStatus,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
pub use types::*;
use vault_store::{Db, Priority};

/// Continues the newest-first queue history without a lifetime record cap.
pub fn list_ai_jobs(vault: &Vault, cursor: Option<i64>, limit: usize) -> Result<AiJobPage> {
    vault.read(Db::Intelligence, move |s| store::list_jobs(s, cursor, limit))
}

/// Startup owns every lease; workers from the previous owner cannot still be alive.
pub(crate) fn recover_owner_claims(storage: &vault_store::Store) -> vault_store::Result<()> {
    storage.write(Db::Intelligence, Priority::Interactive, |s| {
        let now = now_rfc3339();
        s.execute("UPDATE ai_jobs SET state=CASE WHEN stop_requested=1 THEN 'cancelled' ELSE 'queued' END,
            lease_owner=NULL,lease_expires_at=NULL,updated_at=?1 WHERE state='running'", [now.as_str()])?;
        retire_assistant_jobs(s, &now)?;
        Ok(())
    })
}

/// Why a questions-in-the-queue job ended without an answer; Background work says it in words.
pub const ASSISTANT_RETIRED: &str = "assistant-retired";

/// Ask answers as it streams (`ai_chat_send`) and no longer queues questions; an older archive may
/// still hold queued ones. Answering a question asked long ago helps no one, so they end as
/// cancelled with [`ASSISTANT_RETIRED`] instead of running, and are not tried again.
fn retire_assistant_jobs(s: &vault_store::Session<'_>, now: &str) -> vault_store::Result<()> {
    s.execute(
        "UPDATE ai_jobs SET state='cancelled',error_code=?2,
            error_message='Ask no longer answers questions in the background; this one was not answered.',
            finished_at=?1,updated_at=?1,lease_owner=NULL,lease_expires_at=NULL
         WHERE job_type='assistant' AND state NOT IN ('succeeded','failed','cancelled')",
        [now, ASSISTANT_RETIRED],
    )?;
    Ok(())
}

/// Uses the current owner for `enqueue_index_job`.
pub fn enqueue_index_job(
    vault: &Vault,
    request: &AiIndexRequest,
    paused: bool,
) -> Result<AiQueueJob> {
    let request = request.clone();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        store::enqueue_index_job(s, &request, paused)
    })
}

/// Uses the current owner for `load_ai_queue_status`.
pub fn load_ai_queue_status(
    vault: &Vault,
    paused: bool,
    concurrency: u32,
    limit: usize,
) -> Result<AiQueueStatus> {
    vault
        .read(Db::Intelligence, move |s| store::load_ai_queue_status(s, paused, concurrency, limit))
}

/// Uses the current owner for `load_queue_job_counts`.
pub fn load_queue_job_counts(
    vault: &Vault,
    job_types: &[AiQueueJobType],
) -> Result<QueueJobCounts> {
    let job_types = job_types.to_vec();
    vault.read(Db::Intelligence, move |s| store::load_queue_job_counts(s, &job_types))
}

/// Uses the current owner for `count_recoverable_ai_jobs`.
pub fn count_recoverable_ai_jobs(vault: &Vault, stale_after_seconds: i64) -> Result<u32> {
    vault.read(Db::Intelligence, move |s| store::count_recoverable_ai_jobs(s, stale_after_seconds))
}

/// Uses the current owner for `claim_next_ai_job`.
pub fn claim_next_ai_job(vault: &Vault, stale_after_seconds: i64) -> Result<Option<StoredAiJob>> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        store::claim_next_ai_job(s, stale_after_seconds)
    })
}

/// Uses the current owner for `claim_ai_job_by_id`.
pub fn claim_ai_job_by_id(
    vault: &Vault,
    job_id: i64,
    stale_after_seconds: i64,
) -> Result<Option<StoredAiJob>> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        store::claim_ai_job_by_id(s, job_id, stale_after_seconds)
    })
}

/// Uses the current owner for `heartbeat_ai_job`.
pub fn heartbeat_ai_job(vault: &Vault, job_id: i64) -> Result<()> {
    vault
        .write(Db::Intelligence, Priority::Interactive, move |s| store::heartbeat_ai_job(s, job_id))
}

/// Uses the current owner for `persist_index_cursor`.
pub fn persist_index_cursor(
    vault: &Vault,
    job_id: i64,
    cursor: &IndexBackfillCursor,
    summary: Option<&str>,
) -> Result<CursorPersistOutcome> {
    let cursor = cursor.clone();
    let summary = summary.map(str::to_owned);
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        store::persist_index_cursor(s, job_id, &cursor, summary.as_deref())
    })
}

/// Uses the current owner for `mark_ai_job_succeeded`.
pub fn mark_ai_job_succeeded(
    vault: &Vault,
    job_id: i64,
    run_id: Option<i64>,
    summary: Option<&str>,
) -> Result<bool> {
    let summary = summary.map(str::to_owned);
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        store::mark_ai_job_succeeded(s, job_id, run_id, summary.as_deref())
    })
}

/// Uses the current owner for `mark_ai_job_failed`.
pub fn mark_ai_job_failed(
    vault: &Vault,
    job_id: i64,
    run_id: Option<i64>,
    failure: &AiJobFailure,
    paused: bool,
) -> Result<AiQueueJob> {
    let failure = failure.clone();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        store::mark_ai_job_failed(s, job_id, run_id, &failure, paused)
    })
}

/// Uses the current owner for `ai_job_stop_requested`.
pub fn ai_job_stop_requested(vault: &Vault, job_id: i64) -> Result<bool> {
    vault.read(Db::Intelligence, move |s| store::ai_job_stop_requested(s, job_id))
}

/// Uses the current owner for `mark_running_ai_job_cancelled`.
pub fn mark_running_ai_job_cancelled(
    vault: &Vault,
    job_id: i64,
    summary: Option<&str>,
) -> Result<AiQueueJob> {
    let summary = summary.map(str::to_owned);
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        store::mark_running_ai_job_cancelled(s, job_id, summary.as_deref())
    })
}

/// Uses the current owner for `replay_ai_job`.
pub fn replay_ai_job(vault: &Vault, job_id: i64, paused: bool) -> Result<AiQueueJob> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        store::replay_ai_job(s, job_id, paused)
    })
}

/// Uses the current owner for `clear_index_jobs`.
pub fn clear_index_jobs(vault: &Vault) -> Result<usize> {
    vault.write(Db::Intelligence, Priority::Interactive, store::clear_index_jobs)
}

/// Uses the current owner for `cancel_ai_job`.
pub fn cancel_ai_job(vault: &Vault, job_id: i64) -> Result<AiQueueJob> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| store::cancel_ai_job(s, job_id))
}

/// Uses the current owner for `pause_queued_jobs`.
pub fn pause_queued_jobs(vault: &Vault) -> Result<usize> {
    vault.write(Db::Intelligence, Priority::Interactive, store::pause_queued_jobs)
}

/// Uses the current owner for `resume_paused_jobs`.
pub fn resume_paused_jobs(vault: &Vault) -> Result<usize> {
    vault.write(Db::Intelligence, Priority::Interactive, store::resume_paused_jobs)
}

/// Uses the current owner for `load_ai_job`.
pub fn load_ai_job(vault: &Vault, job_id: i64) -> Result<AiQueueJob> {
    vault.read(Db::Intelligence, move |s| store::load_ai_job(s, job_id))
}

/// Uses the current owner for `load_ai_job_payload`.
pub fn load_ai_job_payload(vault: &Vault, job_id: i64) -> Result<AiJobPayload> {
    vault.read(Db::Intelligence, move |s| store::load_ai_job_payload(s, job_id))
}
