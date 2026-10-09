//! Responsibilities: queue transitions inside one Intelligence transaction.
//! Not responsible for: ownership, migrations, providers or background threads.
//! Failure modes: concurrent claims both win, late completion overrides cancellation,
//! retry discards progress, malformed payload blocks the queue, or list limits hide work.
use super::*;
use vault_store::{OptionalRow, Row, Session, params_owned as params};
const AI_JOB_LEASE_SECONDS: i64 = 300;
fn lease_owner_label() -> String {
    format!("pathkeep:{}:{}", std::process::id(), std::thread::current().name().unwrap_or("main"))
}

fn lease_expires_at(seconds: i64) -> String {
    (Utc::now() + Duration::seconds(seconds)).to_rfc3339()
}

/// Enqueues a semantic-index build job and returns its persisted row.
pub(crate) fn enqueue_index_job(
    connection: &Session<'_>,
    request: &AiIndexRequest,
    paused: bool,
) -> Result<AiQueueJob> {
    let priority = if request.clear_only || request.full_rebuild { 90 } else { 70 };
    enqueue_job(
        connection,
        if request.clear_only { AiQueueJobType::IndexClear } else { AiQueueJobType::IndexBuild },
        priority,
        if request.clear_only || request.full_rebuild { 2 } else { 3 },
        AiJobPayload::Index { request: request.clone(), cursor: IndexBackfillCursor::default() },
        paused,
    )
}

/// Enqueues an assistant-question job and returns its persisted row.
pub(crate) fn enqueue_assistant_job(
    connection: &Session<'_>,
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

/// Loads the shell-facing AI queue status read model.
pub(crate) fn load_ai_queue_status(
    connection: &Session<'_>,
    paused: bool,
    concurrency: u32,
    limit: usize,
) -> Result<AiQueueStatus> {
    let counts = load_queue_job_counts(
        connection,
        &[AiQueueJobType::IndexBuild, AiQueueJobType::IndexClear, AiQueueJobType::Assistant],
    )?;
    // Index-only narrowing (M-5): the Smart-search build callout must reflect
    // ONLY semantic-index work. Counting `Assistant` chat jobs here would make
    // an in-flight chat read as "Building the index", which is dishonest.
    let index_counts = load_queue_job_counts(
        connection,
        &[AiQueueJobType::IndexBuild, AiQueueJobType::IndexClear],
    )?;
    // `payload_json` is selected as the last column so the Activity page gets a determinate index-build
    // bar: the index payload's resumable cursor (embedded/scanned/scan_target) is parsed per row in
    // `decode_ai_job_row_with_progress`. Assistant jobs (and any unparseable payload) leave the three
    // progress fields `None` — a bad row never wedges the read model.
    let page = list_jobs(connection, None, limit)?;
    Ok(AiQueueStatus {
        paused,
        concurrency,
        queued: counts.queued,
        running: counts.running,
        failed: counts.failed,
        index_queued: index_counts.queued,
        index_running: index_counts.running,
        recent_jobs: page.items,
        recent_jobs_next_cursor: page.next_cursor,
    })
}

/// Loads queued/running/failed counts for a selected set of AI job types.
pub(crate) fn load_queue_job_counts(
    connection: &Session<'_>,
    job_types: &[AiQueueJobType],
) -> Result<QueueJobCounts> {
    Ok(QueueJobCounts {
        queued: count_jobs_for_types(connection, job_types, &["queued", "paused", "stale"])? as u32,
        running: count_jobs_for_types(connection, job_types, &["running"])? as u32,
        failed: count_jobs_for_types(connection, job_types, &["failed"])? as u32,
    })
}

/// Counts jobs a drain worker could make progress on RIGHT NOW — the signal that decides whether to
/// spawn the background drain pool.
///
/// This is deliberately broader than the read-model `queued` count, which sees only `queued`/`stale`.
/// The bug it fixes: a job orphaned in `running` (its worker died — stale heartbeat / expired lease)
/// is invisible to a `queued`-gated kick, so the drain is never spawned, so `claim_next_ai_job` (which
/// would `mark_stale_jobs` it back to `stale` and re-claim from its cursor) is never called. The job
/// then sits in `running` forever across every app launch and never resumes. Counting orphaned
/// `running` jobs here (mirroring EXACTLY the two orphan predicates `mark_stale_jobs` uses, so this
/// count equals what the next claim will actually recover) lets the periodic status poll kick the
/// drain and self-heal the wedge.
///
/// Read-only: it never mutates. The actual stale-sweep + reclaim still happens only inside the drain's
/// `claim_next_ai_job`, so this changes WHETHER the drain runs, never the sweep timing.
pub(crate) fn count_recoverable_ai_jobs(
    connection: &Session<'_>,
    stale_after_seconds: i64,
) -> Result<u32> {
    let now = now_rfc3339();
    let threshold = (Utc::now() - Duration::seconds(stale_after_seconds)).to_rfc3339();
    let count: i64 = connection.query_one(
        "SELECT COUNT(*) FROM ai_jobs
         WHERE
           -- claimable now: queued or already-stale work that is due
           (state IN ('queued', 'stale') AND available_at <= ?1)
           OR
           -- orphaned running (mark_stale_jobs branch 2 → 'stale' → reclaim from cursor)
           (state = 'running'
              AND COALESCE(stop_requested, 0) = 0
              AND heartbeat_at IS NOT NULL
              AND heartbeat_at < ?2)
           OR
           -- orphaned running with a pending stop (mark_stale_jobs branch 1 → 'cancelled')
           (state = 'running'
              AND stop_requested = 1
              AND (
                (lease_expires_at IS NOT NULL AND lease_expires_at <= ?2)
                OR (heartbeat_at IS NOT NULL AND heartbeat_at < ?2)
              ))",
        params![now, threshold],
        |row| Ok(row.get(0)?),
    )?;
    Ok(count.max(0) as u32)
}

/// Claims the next runnable AI job, recovering stale running jobs first.
pub(crate) fn claim_next_ai_job(
    connection: &Session<'_>,
    stale_after_seconds: i64,
) -> Result<Option<StoredAiJob>> {
    mark_stale_jobs(connection, stale_after_seconds)?;
    let now = now_rfc3339();
    let lease_owner = lease_owner_label();
    let lease_expires_at = lease_expires_at(AI_JOB_LEASE_SECONDS);

    loop {
        // Parse the payload OUTSIDE the row closure: a single unparseable row
        // (schema drift across versions, DB corruption) must not propagate an
        // error out of the claim and wedge the queue head forever. We read the
        // raw columns, then quarantine a bad payload as `failed` and continue.
        let candidate = connection
            .query_one(
                "SELECT id, job_type, attempt, max_attempts, payload_json
                 FROM ai_jobs
                 WHERE state IN ('queued', 'stale')
                   AND available_at <= ?1
                 ORDER BY priority DESC, id ASC
                 LIMIT 1",
                [now.as_str()],
                |row| {
                    Ok((
                        row.get::<i64>(0)?,
                        row.get::<String>(1)?,
                        row.get::<i64>(2)?,
                        row.get::<i64>(3)?,
                        row.get::<String>(4)?,
                    ))
                },
            )
            .optional()?;

        let Some((id, job_type, attempt, max_attempts, payload_json)) = candidate else {
            return Ok(None);
        };

        // RETRY BOUND (the stuck-build fix): a job that has already burned its whole attempt budget
        // must NOT be re-claimed again. Without this, a job that keeps crashing / going stale before it
        // can fail gracefully (so `mark_ai_job_failed`'s attempt check never runs) cycles
        // stale → running → stale forever. Each `claim` increments `attempt`; once `attempt` has reached
        // `max_attempts` WITHOUT any durable progress in between, the budget is spent: move the job to a
        // TERMINAL failed state with an honest reason instead of claiming it, then keep scanning for
        // other runnable work. NOTE: durable progress RESETS `attempt` to 0 (see `persist_index_cursor`),
        // so only a build that makes NO progress across the whole budget terminates here — an
        // interrupted-but-progressing build keeps resuming. A reset / replay also grants a fresh budget.
        if attempt >= max_attempts {
            connection.execute(
                "UPDATE ai_jobs
                 SET state = 'failed',
                     updated_at = ?1,
                     heartbeat_at = ?1,
                     finished_at = ?1,
                     lease_owner = NULL,
                     lease_expires_at = NULL,
                     error_code = 'max-attempts-exhausted',
                     error_message = ?2
                 WHERE id = ?3
                   AND state IN ('queued', 'stale')",
                params![
                    now,
                    format!(
                        "AI job exhausted its retry budget ({max_attempts} attempt(s)) without completing; reset or replay it to try again."
                    ),
                    id,
                ],
            )?;
            continue;
        }

        let payload = match serde_json::from_str::<AiJobPayload>(&payload_json) {
            Ok(payload) => payload,
            Err(error) => {
                connection.execute(
                    "UPDATE ai_jobs
                     SET state = 'failed',
                         updated_at = ?1,
                         heartbeat_at = ?1,
                         finished_at = ?1,
                         lease_owner = NULL,
                         lease_expires_at = NULL,
                         error_code = 'payload-parse-error',
                         error_message = ?2
                     WHERE id = ?3
                       AND state IN ('queued', 'stale')",
                    params![now, format!("unparseable job payload: {error}"), id],
                )?;
                continue;
            }
        };

        let updated = connection.execute(
            "UPDATE ai_jobs
             SET state = 'running',
                 attempt = attempt + 1,
                 updated_at = ?1,
                 started_at = COALESCE(started_at, ?1),
                 heartbeat_at = ?1,
                 lease_owner = ?2,
                 lease_expires_at = ?3,
                 stop_requested = 0
             WHERE id = ?4
               AND state IN ('queued', 'stale')
               AND available_at <= ?1",
            params![now, lease_owner, lease_expires_at, id],
        )?;
        if updated == 1 {
            return Ok(Some(StoredAiJob {
                id,
                job_type: decode_job_type(&job_type),
                attempt: (attempt as u32) + 1,
                max_attempts: max_attempts as u32,
                payload,
            }));
        }
    }
}

/// Claims a specific queued AI job by ID when the state permits it.
pub(crate) fn claim_ai_job_by_id(
    connection: &Session<'_>,
    job_id: i64,
    stale_after_seconds: i64,
) -> Result<Option<StoredAiJob>> {
    mark_stale_jobs(connection, stale_after_seconds)?;
    let now = now_rfc3339();
    let lease_owner = lease_owner_label();
    let lease_expires_at = lease_expires_at(AI_JOB_LEASE_SECONDS);
    let claimed = connection
        .query_one(
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
                let payload = serde_json::from_str::<AiJobPayload>(&payload_json)
                    .map_err(|error| vault_store::Error::Operation(Box::new(error)))?;
                Ok(StoredAiJob {
                    id: row.get(0)?,
                    job_type: decode_job_type(&job_type),
                    attempt: row.get::<i64>(2)? as u32,
                    max_attempts: row.get::<i64>(3)? as u32,
                    payload,
                })
            },
        )
        .optional()?;

    if let Some(job) = claimed {
        let updated = connection.execute(
            "UPDATE ai_jobs
             SET state = 'running',
                 attempt = attempt + 1,
                 updated_at = ?1,
                 started_at = COALESCE(started_at, ?1),
                 heartbeat_at = ?1,
                 lease_owner = ?2,
                 lease_expires_at = ?3,
                 stop_requested = 0
             WHERE id = ?4
               AND state IN ('queued', 'stale')
               AND available_at <= ?1",
            params![now, lease_owner, lease_expires_at, job.id],
        )?;
        if updated == 1 {
            return Ok(Some(StoredAiJob { attempt: job.attempt + 1, ..job }));
        }
    }

    Ok(None)
}

/// Updates the heartbeat timestamp for one running AI job.
pub(crate) fn heartbeat_ai_job(connection: &Session<'_>, job_id: i64) -> Result<()> {
    connection.execute(
        "UPDATE ai_jobs
         SET heartbeat_at = ?1, updated_at = ?1, lease_expires_at = ?2
         WHERE id = ?3 AND state = 'running'",
        params![now_rfc3339(), lease_expires_at(AI_JOB_LEASE_SECONDS), job_id],
    )?;
    Ok(())
}

/// Outcome of a [`persist_index_cursor`] write, distinguishing a real lease loss from a benign skip.
///
/// The embed loop MUST react differently to "the lease was reclaimed by another worker" (abort —
/// keeping going would let two workers double-write the vector store, HIGH-3) versus "the job is not
/// an index job / no longer exists" (a harmless no-op the caller can ignore). A bare row count could
/// not tell these apart, so the cursor write returns this enum.
pub(crate) fn persist_index_cursor(
    connection: &Session<'_>,
    job_id: i64,
    cursor: &IndexBackfillCursor,
    summary: Option<&str>,
) -> Result<CursorPersistOutcome> {
    let Some(mut payload) = load_optional_ai_job_payload(connection, job_id)? else {
        return Ok(CursorPersistOutcome::NotIndexJob);
    };
    let AiJobPayload::Index { request, cursor: stored } = &mut payload else {
        return Ok(CursorPersistOutcome::NotIndexJob);
    };
    // PRESERVE the determinate scan denominator across resumes: the backfill captures `scan_target`
    // ONCE at the build's true start and reports `0` ("unknown") on every resume, so a fresh build
    // sets it here while a resume keeps the value already stored. Without this, a resumed cursor would
    // clobber the original target with 0 and the Activity bar would lose its denominator mid-build.
    let scan_target = if cursor.scan_target > 0 { cursor.scan_target } else { stored.scan_target };
    // Same preserve-on-resume contract for the embed denominator: a resume reports 0, so keep the
    // value captured at the build's true start rather than clobbering it.
    let embed_target =
        if cursor.embed_target > 0 { cursor.embed_target } else { stored.embed_target };
    *stored = IndexBackfillCursor { scan_target, embed_target, ..cursor.clone() };
    // Once the backfill has made durable progress, a `full_rebuild` must NEVER fire its destructive
    // wipe again: a worker that re-claims this job after a crash would otherwise re-run the clear +
    // delete and lose every row already embedded below the cursor (CRITICAL-1). Flipping the stored
    // request to a plain incremental build the moment the cursor leaves the origin is the durable
    // backstop to the in-process `start_history_id == 0` gate in `build_ai_index_with_control`.
    if cursor.next_history_id > 0 && request.full_rebuild {
        request.full_rebuild = false;
    }
    let payload_json = serde_json::to_string(&payload)?;
    let now = now_rfc3339();
    // DURABLE PROGRESS EARNS BACK THE RETRY BUDGET (review fix): the attempt counter increments on every
    // CLAIM, so a long build that makes real progress but is INTERRUPTED (app quit → stale → reclaim)
    // `max_attempts` times would otherwise hit the retry bound and go terminal despite advancing — the
    // common case for a user who keeps interrupting a slow first fill. Resetting `attempt` to 0 here, at
    // the durable-progress checkpoint, means only a build that makes NO progress across the whole budget
    // is judged genuinely wedged; a slow-but-progressing, frequently-interrupted build keeps resuming.
    let updated = connection.execute(
        "UPDATE ai_jobs
         SET payload_json = ?1,
             summary = COALESCE(?2, summary),
             updated_at = ?3,
             heartbeat_at = ?3,
             lease_expires_at = ?4,
             attempt = 0
         WHERE id = ?5 AND state = 'running'",
        params![payload_json, summary, now, lease_expires_at(AI_JOB_LEASE_SECONDS), job_id],
    )?;
    Ok(if updated == 1 {
        CursorPersistOutcome::Persisted
    } else {
        // The row exists as an index job (we loaded its payload) but the running-state guard matched
        // nothing → the lease was reclaimed (stale-swept) out from under this worker.
        CursorPersistOutcome::LeaseLost
    })
}

/// Loads the typed payload for one job, returning `None` when the job no longer exists.
fn load_optional_ai_job_payload(
    connection: &Session<'_>,
    job_id: i64,
) -> Result<Option<AiJobPayload>> {
    connection
        .query_one("SELECT payload_json FROM ai_jobs WHERE id = ?1", [job_id], |row| {
            Ok(row.get::<String>(0)?)
        })
        .optional()?
        .map(|json| serde_json::from_str::<AiJobPayload>(&json).map_err(anyhow::Error::from))
        .transpose()
}

/// Marks a running AI job as succeeded and stores any result metadata.
pub(crate) fn mark_ai_job_succeeded(
    connection: &Session<'_>,
    job_id: i64,
    run_id: Option<i64>,
    summary: Option<&str>,
) -> Result<bool> {
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE ai_jobs
         SET state = 'succeeded',
             run_id = COALESCE(?1, run_id),
             summary = COALESCE(?2, summary),
             updated_at = ?3,
             heartbeat_at = ?3,
             finished_at = ?3,
             lease_owner = NULL,
             lease_expires_at = NULL,
             error_code = NULL,
             error_message = NULL
         WHERE id = ?4
           AND state = 'running'
           AND stop_requested = 0",
        params![run_id, summary, now, job_id],
    )?;
    Ok(updated == 1)
}

/// Marks a running AI job as failed and decides whether it should retry.
pub(crate) fn mark_ai_job_failed(
    connection: &Session<'_>,
    job_id: i64,
    run_id: Option<i64>,
    failure: &AiJobFailure,
    paused: bool,
) -> Result<AiQueueJob> {
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
             lease_owner = NULL,
             lease_expires_at = NULL,
             available_at = ?5,
             finished_at = CASE WHEN ?1 = 'failed' THEN ?4 ELSE NULL END,
             error_code = ?6,
             error_message = ?7
         WHERE id = ?8
           AND state = 'running'
           AND stop_requested = 0",
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

/// Returns whether a running AI job has been asked to stop cooperatively.
pub(crate) fn ai_job_stop_requested(connection: &Session<'_>, job_id: i64) -> Result<bool> {
    connection
        .query_one(
            "SELECT stop_requested
             FROM ai_jobs
             WHERE id = ?1",
            [job_id],
            |row| Ok(row.get::<i64>(0)?),
        )
        .optional()
        .map(|value| value.unwrap_or(0) != 0)
        .with_context(|| format!("loading stop flag for AI job {job_id}"))
}

/// Marks a running AI job as cancelled after its worker observes a stop request.
pub(crate) fn mark_running_ai_job_cancelled(
    connection: &Session<'_>,
    job_id: i64,
    summary: Option<&str>,
) -> Result<AiQueueJob> {
    let now = now_rfc3339();
    connection.execute(
        "UPDATE ai_jobs
         SET state = 'cancelled',
             summary = COALESCE(?1, summary),
             updated_at = ?2,
             heartbeat_at = ?2,
             finished_at = ?2,
             lease_owner = NULL,
             lease_expires_at = NULL,
             error_code = NULL,
             error_message = NULL
         WHERE id = ?3
           AND state = 'running'",
        params![summary, now, job_id],
    )?;
    load_ai_job(connection, job_id)
}

/// Requeues one failed or canceled AI job when replay is allowed.
pub(crate) fn replay_ai_job(
    connection: &Session<'_>,
    job_id: i64,
    paused: bool,
) -> Result<AiQueueJob> {
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
            lease_owner = NULL,
            lease_expires_at = NULL,
            stop_requested = 0,
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

/// Clears every non-terminal semantic-index job, returning how many were cleared (the reset path).
///
/// Recovery primitive for a WEDGED index build (F2): it moves ALL `index-build` / `index-clear` jobs
/// that are still queued / running / stale / paused into a TERMINAL `cancelled` state, dropping any
/// lease and requesting cooperative stop, so a fresh build can be enqueued cleanly without colliding
/// with a stuck job. Terminal states (`succeeded`, `failed`, `cancelled`) are left untouched: a failed
/// build stays failed, with the error the user saw.
/// A running job is asked to stop via `stop_requested = 1` AND flipped to `cancelled` here: the reset
/// is an explicit user action to abandon the current build, and the next `claim_next_ai_job` for that
/// row would no longer match (it is no longer `running`). Returns the number of rows cleared so the
/// caller can report it.
pub(crate) fn clear_index_jobs(connection: &Session<'_>) -> Result<usize> {
    let now = now_rfc3339();
    Ok(connection.execute(
        "UPDATE ai_jobs
         SET state = 'cancelled',
             updated_at = ?1,
             finished_at = ?1,
             heartbeat_at = NULL,
             lease_owner = NULL,
             lease_expires_at = NULL,
             stop_requested = 1
         WHERE job_type IN ('index-build', 'index-clear')
           AND state IN ('queued', 'running', 'stale', 'paused')",
        params![now],
    )? as usize)
}

/// Cancels one queued/paused AI job immediately or requests cooperative stop for running jobs.
pub(crate) fn cancel_ai_job(connection: &Session<'_>, job_id: i64) -> Result<AiQueueJob> {
    let job = load_ai_job(connection, job_id)?;
    let now = now_rfc3339();
    match decode_job_state(&job.state) {
        AiQueueJobState::Running => {
            connection.execute(
                "UPDATE ai_jobs
                 SET updated_at = ?1,
                     summary = COALESCE(summary, 'Cancellation requested from the UI.'),
                     stop_requested = 1
                 WHERE id = ?2
                   AND state = 'running'",
                params![now, job_id],
            )?;
        }
        AiQueueJobState::Queued | AiQueueJobState::Paused | AiQueueJobState::Stale => {
            connection.execute(
                "UPDATE ai_jobs
                 SET state = 'cancelled',
                     updated_at = ?1,
                     finished_at = ?1,
                     heartbeat_at = NULL,
                     lease_owner = NULL,
                     lease_expires_at = NULL,
                     stop_requested = 1
                 WHERE id = ?2",
                params![now, job_id],
            )?;
        }
        _ => anyhow::bail!("AI job {job_id} is in state '{}' and cannot be cancelled.", job.state),
    }
    load_ai_job(connection, job_id)
}

/// Moves queued jobs into the paused state.
pub(crate) fn pause_queued_jobs(connection: &Session<'_>) -> Result<usize> {
    Ok(connection.execute(
        "UPDATE ai_jobs
         SET state = 'paused',
             updated_at = ?1,
             stop_requested = 0
         WHERE state IN ('queued', 'stale')",
        params![now_rfc3339()],
    )? as usize)
}

/// Moves paused jobs back into the queued state.
pub(crate) fn resume_paused_jobs(connection: &Session<'_>) -> Result<usize> {
    Ok(connection.execute(
        "UPDATE ai_jobs
         SET state = 'queued',
             updated_at = ?1,
             available_at = ?1,
             stop_requested = 0
         WHERE state = 'paused'",
        params![now_rfc3339()],
    )? as usize)
}

/// Loads one AI job read model by ID.
pub(crate) fn load_ai_job(connection: &Session<'_>, job_id: i64) -> Result<AiQueueJob> {
    connection
        .query_one(
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

/// Loads and deserializes the typed payload for one AI job.
pub(crate) fn load_ai_job_payload(connection: &Session<'_>, job_id: i64) -> Result<AiJobPayload> {
    connection
        .query_one(
            "SELECT payload_json
             FROM ai_jobs
             WHERE id = ?1",
            [job_id],
            |row| {
                let payload_json: String = row.get(0)?;
                serde_json::from_str::<AiJobPayload>(&payload_json)
                    .map_err(|error| vault_store::Error::Operation(Box::new(error)))
            },
        )
        .with_context(|| format!("loading AI job payload {job_id}"))
}

fn enqueue_job(
    connection: &Session<'_>,
    job_type: AiQueueJobType,
    priority: i64,
    max_attempts: u32,
    payload: AiJobPayload,
    paused: bool,
) -> Result<AiQueueJob> {
    let now = now_rfc3339();
    let state = if paused { AiQueueJobState::Paused } else { AiQueueJobState::Queued };
    let id = connection.query_one(
        "INSERT INTO ai_jobs (
           job_type, state, priority, attempt, max_attempts, payload_json,
           available_at, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?6, ?6) RETURNING id",
        params![
            encode_job_type(job_type),
            encode_job_state(state),
            priority,
            max_attempts as i64,
            serde_json::to_string(&payload)?,
            now,
        ],
        |row| Ok(row.get::<i64>(0)?),
    )?;
    load_ai_job(connection, id)
}

pub(super) fn count_jobs_for_types(
    connection: &Session<'_>,
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
        .query_one(sql.as_str(), vault_store::params_from_iter(param_refs), |row| Ok(row.get(0)?))
        .context("counting filtered AI jobs")
}

fn mark_stale_jobs(connection: &Session<'_>, stale_after_seconds: i64) -> Result<usize> {
    let threshold = (Utc::now() - Duration::seconds(stale_after_seconds)).to_rfc3339();
    let now = now_rfc3339();
    connection.execute(
        "UPDATE ai_jobs
         SET state = 'cancelled',
             updated_at = ?1,
             finished_at = ?1,
             heartbeat_at = ?1,
             lease_owner = NULL,
             lease_expires_at = NULL
         WHERE state = 'running'
           AND stop_requested = 1
           AND (
             (lease_expires_at IS NOT NULL AND lease_expires_at <= ?2)
             OR (heartbeat_at IS NOT NULL AND heartbeat_at < ?2)
           )",
        params![now, threshold],
    )?;
    Ok(connection.execute(
        "UPDATE ai_jobs
         SET state = 'stale',
             updated_at = ?1,
             lease_owner = NULL,
             lease_expires_at = NULL
         WHERE state = 'running'
           AND COALESCE(stop_requested, 0) = 0
           AND heartbeat_at IS NOT NULL
           AND heartbeat_at < ?2",
        params![now, threshold],
    )? as usize)
}

fn decode_ai_job_row(row: &Row) -> vault_store::Result<AiQueueJob> {
    Ok(AiQueueJob {
        id: row.get(0)?,
        job_type: row.get(1)?,
        state: row.get(2)?,
        priority: row.get(3)?,
        attempt: row.get::<i64>(4)? as u32,
        max_attempts: row.get::<i64>(5)? as u32,
        run_id: row.get(6)?,
        summary: row.get(7)?,
        queued_at: row.get(8)?,
        available_at: row.get(9)?,
        started_at: row.get(10)?,
        finished_at: row.get(11)?,
        heartbeat_at: row.get(12)?,
        error_code: row.get(13)?,
        error_message: row.get(14)?,
        // The row-only decode (no `payload_json`) leaves progress unknown; the status path uses
        // `decode_ai_job_row_with_progress`, which fills these from the index cursor.
        progress_embedded: None,
        progress_scanned: None,
        progress_scan_target: None,
        progress_embed_target: None,
    })
}

/// Decodes one queue row INCLUDING index-build progress parsed from `payload_json` (column index 15).
///
/// The Activity page renders a determinate bar for a running index build, so this enriches the base
/// read model with the resumable cursor's `embedded_so_far` (vectors embedded), `next_history_id`
/// (scan watermark), and `scan_target` (scan denominator). ONLY an [`AiJobPayload::Index`] carries a
/// cursor, so an assistant job leaves all three `None`; a payload that fails to parse (schema drift /
/// corruption) is treated the same way rather than erroring the whole status read on one bad row.
fn decode_ai_job_row_with_progress(row: &Row) -> vault_store::Result<AiQueueJob> {
    let mut job = decode_ai_job_row(row)?;
    let payload_json: String = row.get(15)?;
    if let Ok(AiJobPayload::Index { cursor, .. }) =
        serde_json::from_str::<AiJobPayload>(&payload_json)
    {
        job.progress_embedded = Some(cursor.embedded_so_far);
        job.progress_scanned = Some(cursor.next_history_id);
        job.progress_scan_target = Some(cursor.scan_target);
        job.progress_embed_target = Some(cursor.embed_target);
    }
    Ok(job)
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

pub(super) fn decode_job_type(value: &str) -> AiQueueJobType {
    match value {
        "index-clear" => AiQueueJobType::IndexClear,
        "assistant" => AiQueueJobType::Assistant,
        _ => AiQueueJobType::IndexBuild,
    }
}

pub(super) fn encode_job_state(state: AiQueueJobState) -> &'static str {
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

pub(super) fn decode_job_state(value: &str) -> AiQueueJobState {
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

/// Reads one ID-ordered page on the caller's short reader transaction.
pub(crate) fn list_jobs(s: &Session<'_>, cursor: Option<i64>, limit: usize) -> Result<AiJobPage> {
    let limit = limit.clamp(1, 1000);
    let columns = "SELECT id,job_type,state,priority,attempt,max_attempts,run_id,summary,
        created_at,available_at,started_at,finished_at,heartbeat_at,error_code,error_message,payload_json FROM ai_jobs";
    let mut items = if let Some(cursor) = cursor {
        s.query(
            &format!("{columns} WHERE id<?1 ORDER BY id DESC LIMIT ?2"),
            params![cursor, limit as i64 + 1],
            decode_ai_job_row_with_progress,
        )?
    } else {
        s.query(
            &format!("{columns} ORDER BY id DESC LIMIT ?1"),
            [limit as i64 + 1],
            decode_ai_job_row_with_progress,
        )?
    };
    let next_cursor = if items.len() > limit {
        items.pop();
        Some(items.last().unwrap().id)
    } else {
        None
    };
    Ok(AiJobPage { items, next_cursor })
}
