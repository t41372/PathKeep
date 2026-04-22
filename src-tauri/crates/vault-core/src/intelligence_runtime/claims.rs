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
    ensure_intelligence_runtime_schema(connection)?;

    let snapshot = connection
        .query_row(
            "SELECT plugin_id, attempt, payload_json
             FROM intelligence_jobs
             WHERE id = ?1
               AND job_type = ?2
               AND state = 'queued'",
            params![job_id, ENRICHMENT_JOB_TYPE],
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
