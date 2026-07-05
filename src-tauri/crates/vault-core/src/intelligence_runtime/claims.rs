//! Queue claim and scheduling-order helpers.
//!
//! ## Responsibilities
//! - Claim deterministic and enrichment jobs safely under compare-and-set SQL.
//! - Surface the next runnable queue items in worker execution order.
//! - Expose queued-enrichment pagination helpers used by regression tests.

use super::*;
use crate::utils::now_rfc3339;
use rusqlite::{Connection, OptionalExtension, params};
#[cfg(test)]
use rusqlite::{params_from_iter, types::Value as SqlValue};

#[derive(Debug, Clone)]
#[cfg(test)]
pub(super) struct QueuedEnrichmentJobSnapshot {
    pub id: i64,
    pub plugin_id: String,
    pub attempt: i64,
    pub payload_json: String,
    pub priority: i64,
    pub created_at: String,
}

#[derive(Debug, Clone)]
#[cfg(test)]
pub(super) struct EnrichmentQueueCursor {
    priority: i64,
    created_at: String,
    id: i64,
}

#[cfg(test)]
impl QueuedEnrichmentJobSnapshot {
    fn cursor(&self) -> EnrichmentQueueCursor {
        EnrichmentQueueCursor {
            priority: self.priority,
            created_at: self.created_at.clone(),
            id: self.id,
        }
    }
}

pub fn intelligence_job_stop_requested(connection: &Connection, job_id: i64) -> Result<bool> {
    connection
        .query_row(
            "SELECT stop_requested
             FROM intelligence_jobs
             WHERE id = ?1",
            [job_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.unwrap_or(0) != 0)
        .with_context(|| format!("loading stop flag for intelligence job {job_id}"))
}

/// Claims one Core Intelligence job by id and returns its request payload.
pub fn claim_core_intelligence_job(
    connection: &Connection,
    job_id: i64,
) -> Result<Option<DeterministicRebuildJobPayload>> {
    ensure_intelligence_runtime_schema(connection)?;
    let now = now_rfc3339();
    let lease_owner = lease_owner_label();
    let lease_expires_at = lease_expires_at(INTELLIGENCE_JOB_LEASE_SECONDS);
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'running',
             attempt = attempt + 1,
             started_at = COALESCE(started_at, ?1),
             finished_at = NULL,
             updated_at = ?1,
             heartbeat_at = ?1,
             lease_owner = ?2,
             lease_expires_at = ?3,
             last_error = NULL,
             cancellation_reason = NULL,
             stop_requested = 0
         WHERE id = ?4
           AND state = 'queued'",
        params![now, lease_owner, lease_expires_at, job_id],
    )?;
    if updated == 0 {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT payload_json
             FROM intelligence_jobs
             WHERE id = ?1",
            [job_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|payload_json| serde_json::from_str::<DeterministicRebuildJobPayload>(&payload_json))
        .transpose()
        .map_err(Into::into)
}

/// Claims one full rebuild job by id and returns its request payload when the row matches.
pub fn claim_deterministic_rebuild_job(
    connection: &Connection,
    job_id: i64,
) -> Result<Option<DeterministicRebuildJobPayload>> {
    Ok(claim_core_intelligence_job(connection, job_id)?
        .filter(|payload| payload.job_type == RebuildMode::FullRebuild.job_type()))
}

/// Claims a batch of runnable enrichment jobs.
#[cfg(test)]
pub(crate) fn claim_enrichment_jobs(
    connection: &Connection,
    allowed_plugin_ids: &[String],
    allowed_history_ids: &std::collections::HashSet<i64>,
    limit: usize,
) -> Result<Vec<ClaimedEnrichmentJob>> {
    ensure_intelligence_runtime_schema(connection)?;
    if allowed_plugin_ids.is_empty() || allowed_history_ids.is_empty() {
        return Ok(Vec::new());
    }

    let target_count = limit.max(1);
    let page_size = target_count.max(64);
    let mut claimed = Vec::new();
    let mut cursor = None::<EnrichmentQueueCursor>;

    loop {
        let candidates = queued_enrichment_candidates_page(
            connection,
            allowed_plugin_ids,
            cursor.as_ref(),
            page_size,
        )?;
        if candidates.is_empty() {
            break;
        }

        cursor = candidates.last().map(QueuedEnrichmentJobSnapshot::cursor);
        for candidate in candidates {
            let payload = serde_json::from_str::<EnrichmentJobPayload>(&candidate.payload_json)
                .with_context(|| format!("parsing enrichment payload for job {}", candidate.id))?;
            if !allowed_history_ids.contains(&payload.history_id) {
                continue;
            }

            let claimed_at = now_rfc3339();
            if !try_claim_enrichment_job(connection, candidate.id, &claimed_at)? {
                continue;
            }
            claimed.push(ClaimedEnrichmentJob {
                id: candidate.id,
                plugin_id: candidate.plugin_id,
                attempt: candidate.attempt.max(0) as usize + 1,
                payload,
            });
            if claimed.len() >= target_count {
                return Ok(claimed);
            }
        }
    }

    Ok(claimed)
}

pub(crate) fn claim_enrichment_job_by_id(
    connection: &Connection,
    job_id: i64,
) -> Result<Option<ClaimedEnrichmentJob>> {
    claim_typed_enrichment_job_by_id(connection, job_id, ENRICHMENT_JOB_TYPE)
}

/// Claims one queued content-fetch job by id (W-ENRICH-1). Mirrors [`claim_enrichment_job_by_id`] but
/// scoped to the `content-fetch` job type so the two drain lanes never claim each other's rows.
pub(crate) fn claim_content_fetch_job_by_id(
    connection: &Connection,
    job_id: i64,
) -> Result<Option<ClaimedEnrichmentJob>> {
    claim_typed_enrichment_job_by_id(connection, job_id, CONTENT_FETCH_JOB_TYPE)
}

/// Shared claim for the enrichment-shaped queues (offline enrichment + content-fetch), scoped by type.
fn claim_typed_enrichment_job_by_id(
    connection: &Connection,
    job_id: i64,
    job_type: &str,
) -> Result<Option<ClaimedEnrichmentJob>> {
    ensure_intelligence_runtime_schema(connection)?;

    let snapshot = connection
        .query_row(
            "SELECT plugin_id, attempt, payload_json
             FROM intelligence_jobs
             WHERE id = ?1
               AND job_type = ?2
               AND state = 'queued'",
            params![job_id, job_type],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?)),
        )
        .optional()?;
    let Some((plugin_id, attempt, payload_json)) = snapshot else {
        return Ok(None);
    };
    let payload = serde_json::from_str::<EnrichmentJobPayload>(&payload_json)
        .with_context(|| format!("parsing enrichment payload for job {job_id}"))?;
    let claimed_at = now_rfc3339();
    if !try_claim_enrichment_job(connection, job_id, &claimed_at)? {
        return Ok(None);
    }

    Ok(Some(ClaimedEnrichmentJob {
        id: job_id,
        plugin_id,
        attempt: attempt.max(0) as usize + 1,
        payload,
    }))
}

#[cfg(test)]
pub(super) fn queued_enrichment_candidates_page(
    connection: &Connection,
    allowed_plugin_ids: &[String],
    after: Option<&EnrichmentQueueCursor>,
    limit: usize,
) -> Result<Vec<QueuedEnrichmentJobSnapshot>> {
    let placeholders =
        std::iter::repeat_n("?", allowed_plugin_ids.len()).collect::<Vec<_>>().join(", ");
    let mut query = format!(
        "SELECT id, plugin_id, attempt, payload_json, priority, created_at
         FROM intelligence_jobs
         WHERE job_type = ?
           AND state = 'queued'
           AND plugin_id IN ({placeholders})"
    );
    let mut bindings = Vec::<SqlValue>::with_capacity(allowed_plugin_ids.len() + 5);
    bindings.push(SqlValue::from(ENRICHMENT_JOB_TYPE.to_string()));
    bindings.extend(allowed_plugin_ids.iter().cloned().map(SqlValue::from));

    if let Some(after) = after {
        query.push_str(
            " AND (
                priority > ?
                OR (priority = ? AND (created_at > ? OR (created_at = ? AND id > ?)))
              )",
        );
        bindings.push(SqlValue::from(after.priority));
        bindings.push(SqlValue::from(after.priority));
        bindings.push(SqlValue::from(after.created_at.clone()));
        bindings.push(SqlValue::from(after.created_at.clone()));
        bindings.push(SqlValue::from(after.id));
    }

    query.push_str(" ORDER BY priority ASC, created_at ASC, id ASC LIMIT ?");
    bindings.push(SqlValue::from(limit.max(1) as i64));

    let mut statement = connection.prepare(&query)?;
    statement
        .query_map(params_from_iter(bindings.iter()), |row| {
            Ok(QueuedEnrichmentJobSnapshot {
                id: row.get(0)?,
                plugin_id: row.get(1)?,
                attempt: row.get(2)?,
                payload_json: row.get(3)?,
                priority: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn try_claim_enrichment_job(
    connection: &Connection,
    job_id: i64,
    claimed_at: &str,
) -> Result<bool> {
    let lease_owner = lease_owner_label();
    let lease_expires_at = lease_expires_at(INTELLIGENCE_JOB_LEASE_SECONDS);
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'running',
             attempt = attempt + 1,
             started_at = COALESCE(started_at, ?1),
             finished_at = NULL,
             updated_at = ?1,
             heartbeat_at = ?1,
             lease_owner = ?2,
             lease_expires_at = ?3,
             last_error = NULL,
             cancellation_reason = NULL,
             stop_requested = 0
         WHERE id = ?4
           AND state = 'queued'",
        params![claimed_at, lease_owner, lease_expires_at, job_id],
    )?;
    Ok(updated == 1)
}

/// Returns the next queued intelligence job in worker execution order.
pub fn next_queued_intelligence_job(
    connection: &Connection,
) -> Result<Option<QueuedIntelligenceJob>> {
    ensure_intelligence_runtime_schema(connection)?;
    super::recovery::recover_expired_intelligence_jobs(connection)?;
    connection
        .query_row(
            "SELECT id, job_type
             FROM intelligence_jobs
             WHERE state = 'queued'
               AND job_type IN (?1, ?2, ?3, ?4)
             ORDER BY priority ASC,
                      scheduled_at ASC,
                      id ASC
             LIMIT 1",
            [
                VISIT_DERIVE_JOB_TYPE,
                DAILY_ROLLUP_JOB_TYPE,
                STRUCTURAL_REBUILD_JOB_TYPE,
                FULL_REBUILD_JOB_TYPE,
            ],
            |row| Ok(QueuedIntelligenceJob { id: row.get(0)?, job_type: row.get(1)? }),
        )
        .optional()
        .context("loading next queued intelligence job")
}

/// Returns the next queued enrichment job in execution order.
pub fn next_queued_enrichment_job(connection: &Connection) -> Result<Option<i64>> {
    ensure_intelligence_runtime_schema(connection)?;
    super::recovery::recover_expired_intelligence_jobs(connection)?;
    connection
        .query_row(
            "SELECT id
             FROM intelligence_jobs
             WHERE job_type = ?1
               AND state = 'queued'
             ORDER BY priority DESC, scheduled_at ASC, id ASC
             LIMIT 1",
            [ENRICHMENT_JOB_TYPE],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .context("loading next queued enrichment job")
}

/// Returns the next due content-fetch job id, or `None` when none is ready (W-ENRICH-1).
///
/// A content-fetch job is "due" when it is queued AND its `scheduled_at` is in the past — the same
/// row that the negative cache pushes into the future on a transient failure (the requeue stamps a
/// future `scheduled_at`). Ordered by priority then schedule so the working-set selector's
/// higher-priority enqueues drain first. LOW concurrency keeps egress polite (06 §5).
pub fn next_queued_content_fetch_job(connection: &Connection) -> Result<Option<i64>> {
    ensure_intelligence_runtime_schema(connection)?;
    super::recovery::recover_expired_intelligence_jobs(connection)?;
    connection
        .query_row(
            "SELECT id
             FROM intelligence_jobs
             WHERE job_type = ?1
               AND state = 'queued'
               AND scheduled_at <= ?2
             ORDER BY priority ASC, scheduled_at ASC, id ASC
             LIMIT 1",
            params![CONTENT_FETCH_JOB_TYPE, now_rfc3339()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .context("loading next queued content-fetch job")
}

/// Seconds until the soonest deferred content-fetch job becomes due, or `None` when none is pending.
///
/// Companion to [`next_queued_content_fetch_job`] for the worker drain (SEC-2): when the drain reports
/// idle but a job was REQUEUED with a future `scheduled_at` (rate-limit back-pressure), the lane should
/// sleep until that schedule rather than exiting — otherwise the deferred work only completes on a
/// later user action. Returns the ETA in WHOLE seconds (rounded up, floored at 1 so the lane always
/// makes progress) to the nearest queued content-fetch row whose `scheduled_at` is in the FUTURE;
/// `None` when there is no future-scheduled queued row (the lane can exit). The caller caps the sleep.
pub fn next_content_fetch_schedule_eta_secs(connection: &Connection) -> Result<Option<u64>> {
    ensure_intelligence_runtime_schema(connection)?;
    let now = chrono::Utc::now();
    let soonest: Option<String> = connection
        .query_row(
            "SELECT MIN(scheduled_at)
             FROM intelligence_jobs
             WHERE job_type = ?1
               AND state = 'queued'
               AND scheduled_at > ?2",
            params![CONTENT_FETCH_JOB_TYPE, now_rfc3339()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    let Some(soonest) = soonest else {
        return Ok(None);
    };
    let Ok(scheduled) = chrono::DateTime::parse_from_rfc3339(&soonest) else {
        // An unparseable stamp shouldn't wedge the lane: treat it as due-now (sleep the floor).
        return Ok(Some(1));
    };
    let secs = (scheduled.with_timezone(&chrono::Utc) - now).num_seconds();
    Ok(Some(secs.max(1) as u64))
}

/// Whether a URL's content-fetch is DUE (no stored row, OR a stored row whose `refetch_after` has
/// passed / is absent), gating against the negative cache + extractor version (06 §3/§5).
///
/// PURE-ish (one read): returns `false` when a `visit_content_enrichments` row exists for the URL's
/// resolved extractor whose `extractor_version` matches the current extractor AND whose `refetch_after`
/// is still in the future — i.e. "fetched recently, do not refetch". Returns `true` otherwise (never
/// fetched, the negative cache cooled down, or the extractor version bumped → bounded refetch). The
/// `history_id` identifies the row (the dedup fans the stored row across visits separately).
pub(crate) fn content_fetch_job_due(
    connection: &Connection,
    history_id: i64,
    content_source: &str,
    extractor_version: i64,
) -> Result<bool> {
    ensure_intelligence_runtime_schema(connection)?;
    let row = connection
        .query_row(
            "SELECT extractor_version, refetch_after
             FROM visit_content_enrichments
             WHERE history_id = ?1 AND content_source = ?2",
            params![history_id, content_source],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .context("loading content-fetch due state")?;
    let Some((stored_version, refetch_after)) = row else {
        return Ok(true); // Never fetched → due.
    };
    if stored_version != Some(extractor_version) {
        return Ok(true); // Extractor bumped → bounded refetch of this source's rows.
    }
    match refetch_after {
        // A future refetch_after means the negative cache is still cooling down → NOT due. Compare as
        // INSTANTS, not lexically: `refetch_after` is `...Secs, true` (`...56Z`) while `now_rfc3339`
        // carries fractional seconds + a numeric offset (`...56.789+00:00`), and `'Z' > '.'` would
        // mis-rank a row as not-due at the boundary second. An unparseable stamp is treated as due
        // (fail toward refetching rather than wedging a row forever).
        Some(when) => match chrono::DateTime::parse_from_rfc3339(&when) {
            Ok(dt) => Ok(dt <= chrono::Utc::now()),
            Err(_) => Ok(true),
        },
        // No refetch_after on a version-matching row means a successful fetch → NOT due.
        None => Ok(false),
    }
}

/// Requeues a running content-fetch job for a later drain at `scheduled_at` (W-ENRICH-1, SEC-2).
///
/// A rate-limited egress is BACK-PRESSURE, not a failure: rather than terminally cancelling the job
/// (which the queued-only drain selector would never re-pick), we put it back to `queued` with a
/// FUTURE `scheduled_at` (= the host's token-refill ETA) so [`next_queued_content_fetch_job`] — which
/// gates on `scheduled_at <= now` — picks it up exactly when the host bucket has a token again. Clears
/// the lease/error/cancellation fields so the requeued row is a clean queued job. Scoped to `running`
/// (the claim owns it) so a concurrent transition can't be clobbered. Returns whether a row changed.
pub(crate) fn requeue_content_fetch_job_after(
    connection: &Connection,
    job_id: i64,
    scheduled_at: &str,
) -> Result<bool> {
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued',
             scheduled_at = ?1,
             started_at = NULL,
             finished_at = NULL,
             heartbeat_at = NULL,
             lease_owner = NULL,
             lease_expires_at = NULL,
             updated_at = ?2,
             last_error = NULL,
             cancellation_reason = NULL,
             stop_requested = 0
         WHERE id = ?3
           AND state = 'running'",
        params![scheduled_at, now, job_id],
    )?;
    Ok(updated == 1)
}
