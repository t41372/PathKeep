use crate::{
    models::{
        AiAssistantRequest, AiIndexRequest, AiQueueJob, AiQueueJobState, AiQueueJobType,
        AiQueueStatus,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

const AI_QUEUE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS ai_jobs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_type TEXT NOT NULL,
  state TEXT NOT NULL,
  priority INTEGER NOT NULL,
  attempt INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 1,
  payload_json TEXT NOT NULL,
  run_id INTEGER REFERENCES runs(id),
  summary TEXT,
  available_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  started_at TEXT,
  finished_at TEXT,
  heartbeat_at TEXT,
  error_code TEXT,
  error_message TEXT
);
CREATE INDEX IF NOT EXISTS idx_ai_jobs_state_available
  ON ai_jobs(state, available_at, priority DESC, id ASC);
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantJobPayload {
    pub request: AiAssistantRequest,
    pub llm_provider_id: String,
    pub embedding_provider_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueJobCounts {
    pub queued: u32,
    pub running: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AiJobPayload {
    Index { request: AiIndexRequest },
    Assistant { payload: AssistantJobPayload },
}

#[derive(Debug, Clone)]
pub struct StoredAiJob {
    pub id: i64,
    pub job_type: AiQueueJobType,
    pub attempt: u32,
    pub max_attempts: u32,
    pub payload: AiJobPayload,
}

#[derive(Debug, Clone, Default)]
pub struct AiJobFailure {
    pub error_code: Option<String>,
    pub error_message: String,
    pub retryable: bool,
    pub retry_after_seconds: u64,
    pub summary: Option<String>,
}

pub fn ensure_ai_queue_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(AI_QUEUE_SCHEMA_SQL)?;
    Ok(())
}

pub fn enqueue_index_job(
    connection: &Connection,
    request: &AiIndexRequest,
    paused: bool,
) -> Result<AiQueueJob> {
    let priority = if request.clear_only || request.full_rebuild { 90 } else { 70 };
    enqueue_job(
        connection,
        if request.clear_only { AiQueueJobType::IndexClear } else { AiQueueJobType::IndexBuild },
        priority,
        if request.clear_only || request.full_rebuild { 2 } else { 3 },
        AiJobPayload::Index { request: request.clone() },
        paused,
    )
}

pub fn enqueue_assistant_job(
    connection: &Connection,
    request: &AiAssistantRequest,
    llm_provider_id: &str,
    embedding_provider_id: Option<&str>,
    paused: bool,
) -> Result<AiQueueJob> {
    enqueue_job(
        connection,
        AiQueueJobType::Assistant,
        100,
        1,
        AiJobPayload::Assistant {
            payload: AssistantJobPayload {
                request: request.clone(),
                llm_provider_id: llm_provider_id.to_string(),
                embedding_provider_id: embedding_provider_id.map(ToOwned::to_owned),
            },
        },
        paused,
    )
}

pub fn load_ai_queue_status(
    connection: &Connection,
    paused: bool,
    concurrency: u32,
    limit: usize,
) -> Result<AiQueueStatus> {
    ensure_ai_queue_schema(connection)?;
    let counts = load_queue_job_counts(
        connection,
        &[AiQueueJobType::IndexBuild, AiQueueJobType::IndexClear, AiQueueJobType::Assistant],
    )?;
    let mut statement = connection.prepare(
        "SELECT id, job_type, state, priority, attempt, max_attempts, run_id, summary,
                created_at, available_at, started_at, finished_at, heartbeat_at,
                error_code, error_message
         FROM ai_jobs
         ORDER BY id DESC
         LIMIT ?1",
    )?;
    let recent_jobs = statement
        .query_map([limit as i64], decode_ai_job_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(AiQueueStatus {
        paused,
        concurrency,
        queued: counts.queued,
        running: counts.running,
        failed: counts.failed,
        recent_jobs,
    })
}

pub fn load_queue_job_counts(
    connection: &Connection,
    job_types: &[AiQueueJobType],
) -> Result<QueueJobCounts> {
    ensure_ai_queue_schema(connection)?;
    Ok(QueueJobCounts {
        queued: count_jobs_for_types(connection, job_types, &["queued", "paused", "stale"])? as u32,
        running: count_jobs_for_types(connection, job_types, &["running"])? as u32,
        failed: count_jobs_for_types(connection, job_types, &["failed"])? as u32,
    })
}

pub fn claim_next_ai_job(
    connection: &Connection,
    stale_after_seconds: i64,
) -> Result<Option<StoredAiJob>> {
    ensure_ai_queue_schema(connection)?;
    mark_stale_jobs(connection, stale_after_seconds)?;
    let now = now_rfc3339();
    let claimed = connection
        .query_row(
            "SELECT id, job_type, attempt, max_attempts, payload_json
             FROM ai_jobs
             WHERE state IN ('queued', 'stale')
               AND available_at <= ?1
             ORDER BY priority DESC, id ASC
             LIMIT 1",
            [&now],
            |row| {
                let job_type: String = row.get(1)?;
                let payload_json: String = row.get(4)?;
                let payload =
                    serde_json::from_str::<AiJobPayload>(&payload_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(StoredAiJob {
                    id: row.get(0)?,
                    job_type: decode_job_type(&job_type),
                    attempt: row.get::<_, i64>(2)? as u32,
                    max_attempts: row.get::<_, i64>(3)? as u32,
                    payload,
                })
            },
        )
        .optional()?;

    if let Some(job) = claimed {
        connection.execute(
            "UPDATE ai_jobs
             SET state = 'running',
                 attempt = attempt + 1,
                 updated_at = ?1,
                 started_at = COALESCE(started_at, ?1),
                 heartbeat_at = ?1
             WHERE id = ?2",
            params![now, job.id],
        )?;
        return Ok(Some(StoredAiJob { attempt: job.attempt + 1, ..job }));
    }

    Ok(None)
}

pub fn claim_ai_job_by_id(
    connection: &Connection,
    job_id: i64,
    stale_after_seconds: i64,
) -> Result<Option<StoredAiJob>> {
    ensure_ai_queue_schema(connection)?;
    mark_stale_jobs(connection, stale_after_seconds)?;
    let now = now_rfc3339();
    let claimed = connection
        .query_row(
            "SELECT id, job_type, attempt, max_attempts, payload_json
             FROM ai_jobs
             WHERE id = ?1
               AND state IN ('queued', 'stale')
               AND available_at <= ?2
             LIMIT 1",
            params![job_id, now],
            |row| {
                let job_type: String = row.get(1)?;
                let payload_json: String = row.get(4)?;
                let payload =
                    serde_json::from_str::<AiJobPayload>(&payload_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(StoredAiJob {
                    id: row.get(0)?,
                    job_type: decode_job_type(&job_type),
                    attempt: row.get::<_, i64>(2)? as u32,
                    max_attempts: row.get::<_, i64>(3)? as u32,
                    payload,
                })
            },
        )
        .optional()?;

    if let Some(job) = claimed {
        connection.execute(
            "UPDATE ai_jobs
             SET state = 'running',
                 attempt = attempt + 1,
                 updated_at = ?1,
                 started_at = COALESCE(started_at, ?1),
                 heartbeat_at = ?1
             WHERE id = ?2",
            params![now, job.id],
        )?;
        return Ok(Some(StoredAiJob { attempt: job.attempt + 1, ..job }));
    }

    Ok(None)
}

pub fn heartbeat_ai_job(connection: &Connection, job_id: i64) -> Result<()> {
    ensure_ai_queue_schema(connection)?;
    connection.execute(
        "UPDATE ai_jobs
         SET heartbeat_at = ?1, updated_at = ?1
         WHERE id = ?2 AND state = 'running'",
        params![now_rfc3339(), job_id],
    )?;
    Ok(())
}

pub fn mark_ai_job_succeeded(
    connection: &Connection,
    job_id: i64,
    run_id: Option<i64>,
    summary: Option<&str>,
) -> Result<AiQueueJob> {
    ensure_ai_queue_schema(connection)?;
    let now = now_rfc3339();
    connection.execute(
        "UPDATE ai_jobs
         SET state = 'succeeded',
             run_id = COALESCE(?1, run_id),
             summary = COALESCE(?2, summary),
             updated_at = ?3,
             heartbeat_at = ?3,
             finished_at = ?3,
             error_code = NULL,
             error_message = NULL
         WHERE id = ?4",
        params![run_id, summary, now, job_id],
    )?;
    load_ai_job(connection, job_id)
}

pub fn mark_ai_job_failed(
    connection: &Connection,
    job_id: i64,
    run_id: Option<i64>,
    failure: &AiJobFailure,
    paused: bool,
) -> Result<AiQueueJob> {
    ensure_ai_queue_schema(connection)?;
    let job = load_ai_job(connection, job_id)?;
    let now = parse_rfc3339(&now_rfc3339())?;
    let should_retry = failure.retryable
        && job.attempt < job.max_attempts
        && !paused
        && failure.retry_after_seconds > 0;
    let next_available = if should_retry {
        (now + Duration::seconds(failure.retry_after_seconds as i64)).to_rfc3339()
    } else {
        now.to_rfc3339()
    };
    let next_state = if paused {
        AiQueueJobState::Paused
    } else if should_retry {
        AiQueueJobState::Queued
    } else {
        AiQueueJobState::Failed
    };
    connection.execute(
        "UPDATE ai_jobs
         SET state = ?1,
             run_id = COALESCE(?2, run_id),
             summary = COALESCE(?3, summary),
             updated_at = ?4,
             heartbeat_at = ?4,
             available_at = ?5,
             finished_at = CASE WHEN ?1 = 'failed' THEN ?4 ELSE NULL END,
             error_code = ?6,
             error_message = ?7
         WHERE id = ?8",
        params![
            encode_job_state(next_state),
            run_id,
            failure.summary.as_deref(),
            now.to_rfc3339(),
            next_available,
            failure.error_code.as_deref(),
            failure.error_message.as_str(),
            job_id,
        ],
    )?;
    load_ai_job(connection, job_id)
}

pub fn replay_ai_job(connection: &Connection, job_id: i64, paused: bool) -> Result<AiQueueJob> {
    ensure_ai_queue_schema(connection)?;
    let job = load_ai_job(connection, job_id)?;
    let state = decode_job_state(&job.state);
    if !matches!(
        state,
        AiQueueJobState::Failed
            | AiQueueJobState::Cancelled
            | AiQueueJobState::Stale
            | AiQueueJobState::Paused
    ) {
        anyhow::bail!("Only failed, cancelled, stale, or paused AI jobs can be replayed.");
    }
    let now = now_rfc3339();
    connection.execute(
        "UPDATE ai_jobs
         SET state = ?1,
             attempt = 0,
             run_id = NULL,
             updated_at = ?2,
             available_at = ?2,
             started_at = NULL,
             finished_at = NULL,
             heartbeat_at = NULL,
             error_code = NULL,
             error_message = NULL
         WHERE id = ?3",
        params![
            encode_job_state(if paused {
                AiQueueJobState::Paused
            } else {
                AiQueueJobState::Queued
            }),
            now,
            job_id,
        ],
    )?;
    load_ai_job(connection, job_id)
}

pub fn cancel_ai_job(connection: &Connection, job_id: i64) -> Result<AiQueueJob> {
    ensure_ai_queue_schema(connection)?;
    let job = load_ai_job(connection, job_id)?;
    if decode_job_state(&job.state) == AiQueueJobState::Running {
        anyhow::bail!("Running AI jobs cannot be cancelled mid-flight in the current worker.");
    }
    let now = now_rfc3339();
    connection.execute(
        "UPDATE ai_jobs
         SET state = 'cancelled',
             updated_at = ?1,
             finished_at = ?1
         WHERE id = ?2",
        params![now, job_id],
    )?;
    load_ai_job(connection, job_id)
}

pub fn pause_queued_jobs(connection: &Connection) -> Result<usize> {
    ensure_ai_queue_schema(connection)?;
    Ok(connection.execute(
        "UPDATE ai_jobs
         SET state = 'paused',
             updated_at = ?1
         WHERE state IN ('queued', 'stale')",
        params![now_rfc3339()],
    )?)
}

pub fn resume_paused_jobs(connection: &Connection) -> Result<usize> {
    ensure_ai_queue_schema(connection)?;
    Ok(connection.execute(
        "UPDATE ai_jobs
         SET state = 'queued',
             updated_at = ?1,
             available_at = ?1
         WHERE state = 'paused'",
        params![now_rfc3339()],
    )?)
}

pub fn load_ai_job(connection: &Connection, job_id: i64) -> Result<AiQueueJob> {
    ensure_ai_queue_schema(connection)?;
    connection
        .query_row(
            "SELECT id, job_type, state, priority, attempt, max_attempts, run_id, summary,
                    created_at, available_at, started_at, finished_at, heartbeat_at,
                    error_code, error_message
             FROM ai_jobs
             WHERE id = ?1",
            [job_id],
            decode_ai_job_row,
        )
        .with_context(|| format!("loading AI job {job_id}"))
}

pub fn load_ai_job_payload(connection: &Connection, job_id: i64) -> Result<AiJobPayload> {
    ensure_ai_queue_schema(connection)?;
    connection
        .query_row(
            "SELECT payload_json
             FROM ai_jobs
             WHERE id = ?1",
            [job_id],
            |row| {
                let payload_json: String = row.get(0)?;
                serde_json::from_str::<AiJobPayload>(&payload_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            },
        )
        .with_context(|| format!("loading AI job payload {job_id}"))
}

fn enqueue_job(
    connection: &Connection,
    job_type: AiQueueJobType,
    priority: i64,
    max_attempts: u32,
    payload: AiJobPayload,
    paused: bool,
) -> Result<AiQueueJob> {
    ensure_ai_queue_schema(connection)?;
    let now = now_rfc3339();
    let state = if paused { AiQueueJobState::Paused } else { AiQueueJobState::Queued };
    connection.execute(
        "INSERT INTO ai_jobs (
           job_type, state, priority, attempt, max_attempts, payload_json,
           available_at, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?6, ?6)",
        params![
            encode_job_type(job_type),
            encode_job_state(state),
            priority,
            max_attempts as i64,
            serde_json::to_string(&payload)?,
            now,
        ],
    )?;
    load_ai_job(connection, connection.last_insert_rowid())
}

fn count_jobs_for_types(
    connection: &Connection,
    job_types: &[AiQueueJobType],
    states: &[&str],
) -> Result<i64> {
    if job_types.is_empty() || states.is_empty() {
        return Ok(0);
    }
    let type_clause = std::iter::repeat_n("?", job_types.len()).collect::<Vec<_>>().join(", ");
    let state_clause = std::iter::repeat_n("?", states.len()).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT COUNT(*)
         FROM ai_jobs
         WHERE job_type IN ({type_clause})
           AND state IN ({state_clause})"
    );
    let type_values =
        job_types.iter().map(|job_type| encode_job_type(job_type.clone()).to_string());
    let state_values = states.iter().map(|state| (*state).to_string());
    let params = type_values.chain(state_values).collect::<Vec<_>>();
    let param_refs = params.iter().map(|value| value.as_str()).collect::<Vec<_>>();
    connection
        .query_row(sql.as_str(), rusqlite::params_from_iter(param_refs), |row: &Row<'_>| row.get(0))
        .context("counting filtered AI jobs")
}

fn mark_stale_jobs(connection: &Connection, stale_after_seconds: i64) -> Result<usize> {
    let threshold = (Utc::now() - Duration::seconds(stale_after_seconds)).to_rfc3339();
    Ok(connection.execute(
        "UPDATE ai_jobs
         SET state = 'stale', updated_at = ?1
         WHERE state = 'running'
           AND heartbeat_at IS NOT NULL
           AND heartbeat_at < ?2",
        params![now_rfc3339(), threshold],
    )?)
}

fn decode_ai_job_row(row: &Row<'_>) -> rusqlite::Result<AiQueueJob> {
    Ok(AiQueueJob {
        id: row.get(0)?,
        job_type: row.get(1)?,
        state: row.get(2)?,
        priority: row.get(3)?,
        attempt: row.get::<_, i64>(4)? as u32,
        max_attempts: row.get::<_, i64>(5)? as u32,
        run_id: row.get(6)?,
        summary: row.get(7)?,
        queued_at: row.get(8)?,
        available_at: row.get(9)?,
        started_at: row.get(10)?,
        finished_at: row.get(11)?,
        heartbeat_at: row.get(12)?,
        error_code: row.get(13)?,
        error_message: row.get(14)?,
    })
}

fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn encode_job_type(job_type: AiQueueJobType) -> &'static str {
    match job_type {
        AiQueueJobType::IndexBuild => "index-build",
        AiQueueJobType::IndexClear => "index-clear",
        AiQueueJobType::Assistant => "assistant",
    }
}

fn decode_job_type(value: &str) -> AiQueueJobType {
    match value {
        "index-clear" => AiQueueJobType::IndexClear,
        "assistant" => AiQueueJobType::Assistant,
        _ => AiQueueJobType::IndexBuild,
    }
}

fn encode_job_state(state: AiQueueJobState) -> &'static str {
    match state {
        AiQueueJobState::Queued => "queued",
        AiQueueJobState::Running => "running",
        AiQueueJobState::Succeeded => "succeeded",
        AiQueueJobState::Failed => "failed",
        AiQueueJobState::Paused => "paused",
        AiQueueJobState::Cancelled => "cancelled",
        AiQueueJobState::Stale => "stale",
    }
}

fn decode_job_state(value: &str) -> AiQueueJobState {
    match value {
        "running" => AiQueueJobState::Running,
        "succeeded" => AiQueueJobState::Succeeded,
        "failed" => AiQueueJobState::Failed,
        "paused" => AiQueueJobState::Paused,
        "cancelled" => AiQueueJobState::Cancelled,
        "stale" => AiQueueJobState::Stale,
        _ => AiQueueJobState::Queued,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory db");
        connection
            .execute_batch(
                "CREATE TABLE runs (
                   id INTEGER PRIMARY KEY AUTOINCREMENT
                 );
                 INSERT INTO runs (id) VALUES (42);",
            )
            .expect("create runs table");
        ensure_ai_queue_schema(&connection).expect("ensure queue schema");
        connection
    }

    #[test]
    fn enqueue_and_load_queue_status_tracks_counts_and_recent_jobs() {
        let connection = connection();
        enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue index");
        enqueue_assistant_job(
            &connection,
            &AiAssistantRequest {
                question: "What changed?".to_string(),
                profile_id: None,
                domain: None,
            },
            "llm-primary",
            Some("embed-primary"),
            true,
        )
        .expect("enqueue assistant");

        let status = load_ai_queue_status(&connection, true, 2, 8).expect("status");
        assert!(status.paused);
        assert_eq!(status.concurrency, 2);
        assert_eq!(status.queued, 2);
        assert_eq!(status.running, 0);
        assert_eq!(status.failed, 0);
        assert_eq!(status.recent_jobs.len(), 2);
        assert_eq!(status.recent_jobs[0].state, "paused");
    }

    #[test]
    fn claim_and_complete_job_advances_lifecycle() {
        let connection = connection();
        let queued =
            enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue");
        let claimed = claim_next_ai_job(&connection, 60).expect("claim").expect("job");
        assert_eq!(claimed.id, queued.id);
        heartbeat_ai_job(&connection, claimed.id).expect("heartbeat");

        let finished =
            mark_ai_job_succeeded(&connection, claimed.id, Some(42), Some("Index refreshed"))
                .expect("succeeded");
        assert_eq!(finished.state, "succeeded");
        assert_eq!(finished.run_id, Some(42));
        assert_eq!(finished.summary.as_deref(), Some("Index refreshed"));
        assert!(finished.finished_at.is_some());
    }

    #[test]
    fn retryable_failures_requeue_until_attempt_budget_is_exhausted() {
        let connection = connection();
        let queued =
            enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue");
        let claimed = claim_next_ai_job(&connection, 60).expect("claim").expect("job");
        assert_eq!(claimed.id, queued.id);

        let retried = mark_ai_job_failed(
            &connection,
            claimed.id,
            None,
            &AiJobFailure {
                error_code: Some("network-error".to_string()),
                error_message: "connection timed out".to_string(),
                retryable: true,
                retry_after_seconds: 5,
                summary: Some("Will retry after timeout".to_string()),
            },
            false,
        )
        .expect("mark retryable failure");
        assert_eq!(retried.state, "queued");
        assert_eq!(retried.error_code.as_deref(), Some("network-error"));

        connection
            .execute(
                "UPDATE ai_jobs SET available_at = ?1 WHERE id = ?2",
                params![now_rfc3339(), retried.id],
            )
            .expect("release retry delay");
        let claimed_again =
            claim_next_ai_job(&connection, 60).expect("claim second attempt").expect("second job");
        assert_eq!(claimed_again.attempt, 2);

        let retried_again = mark_ai_job_failed(
            &connection,
            claimed_again.id,
            None,
            &AiJobFailure {
                error_code: Some("network-error".to_string()),
                error_message: "connection timed out".to_string(),
                retryable: true,
                retry_after_seconds: 5,
                summary: Some("Retry budget exhausted".to_string()),
            },
            false,
        )
        .expect("mark second retry");
        assert_eq!(retried_again.state, "queued");
        connection
            .execute(
                "UPDATE ai_jobs SET available_at = ?1 WHERE id = ?2",
                params![now_rfc3339(), retried_again.id],
            )
            .expect("release final retry delay");
        let final_claim =
            claim_next_ai_job(&connection, 60).expect("claim final attempt").expect("final job");
        assert_eq!(final_claim.attempt, 3);

        let terminal = mark_ai_job_failed(
            &connection,
            final_claim.id,
            None,
            &AiJobFailure {
                error_code: Some("network-error".to_string()),
                error_message: "connection timed out".to_string(),
                retryable: true,
                retry_after_seconds: 5,
                summary: Some("Retry budget exhausted".to_string()),
            },
            false,
        )
        .expect("mark terminal failure");
        assert_eq!(terminal.state, "failed");
        assert!(terminal.finished_at.is_some());
    }

    #[test]
    fn stale_jobs_are_reclaimed_and_replay_cancel_respect_boundaries() {
        let connection = connection();
        let queued = enqueue_assistant_job(
            &connection,
            &AiAssistantRequest {
                question: "Summarize MCP research".to_string(),
                profile_id: None,
                domain: None,
            },
            "llm-primary",
            Some("embed-primary"),
            false,
        )
        .expect("enqueue assistant");
        let claimed = claim_next_ai_job(&connection, 60).expect("claim").expect("job");
        assert_eq!(claimed.id, queued.id);
        connection
            .execute(
                "UPDATE ai_jobs SET heartbeat_at = ?1 WHERE id = ?2",
                params!["2000-01-01T00:00:00+00:00", claimed.id],
            )
            .expect("age heartbeat");

        let reclaimed = claim_next_ai_job(&connection, 1).expect("claim stale").expect("stale job");
        assert_eq!(reclaimed.id, claimed.id);
        let failed = mark_ai_job_failed(
            &connection,
            reclaimed.id,
            None,
            &AiJobFailure {
                error_code: Some("bad-model".to_string()),
                error_message: "model missing".to_string(),
                retryable: false,
                retry_after_seconds: 0,
                summary: Some("Pick a valid model".to_string()),
            },
            false,
        )
        .expect("terminal fail");
        let replayed = replay_ai_job(&connection, failed.id, true).expect("replay");
        assert_eq!(replayed.state, "paused");
        let cancelled = cancel_ai_job(&connection, replayed.id).expect("cancel");
        assert_eq!(cancelled.state, "cancelled");
    }

    #[test]
    fn cancel_rejects_running_jobs() {
        let connection = connection();
        let queued =
            enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue");
        let claimed = claim_next_ai_job(&connection, 60).expect("claim").expect("job");
        assert_eq!(claimed.id, queued.id);

        let error = cancel_ai_job(&connection, claimed.id).expect_err("cancel should fail");
        assert!(error.to_string().contains("cannot be cancelled"));
    }

    #[test]
    fn claim_by_id_and_pause_resume_cover_targeted_orchestration_paths() {
        let connection = connection();
        let first =
            enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("first");
        let second =
            enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("second");

        let claimed =
            claim_ai_job_by_id(&connection, second.id, 60).expect("claim by id").expect("job");
        assert_eq!(claimed.id, second.id);
        assert_eq!(claimed.attempt, 1);

        let untouched = load_ai_job(&connection, first.id).expect("first remains queued");
        assert_eq!(untouched.state, "queued");

        let paused = pause_queued_jobs(&connection).expect("pause queued jobs");
        assert_eq!(paused, 1);
        assert_eq!(load_ai_job(&connection, first.id).expect("paused job").state, "paused");

        let resumed = resume_paused_jobs(&connection).expect("resume paused jobs");
        assert_eq!(resumed, 1);
        assert_eq!(load_ai_job(&connection, first.id).expect("resumed job").state, "queued");
    }
}
