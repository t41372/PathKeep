//! Queue enqueue and module-runtime mutation helpers.
//!
//! ## Responsibilities
//! - Insert or refresh deterministic rebuild and enrichment queue rows.
//! - Record queue trigger provenance for deduped jobs.
//! - Persist deterministic module runtime bookkeeping after rebuild stages.

use super::*;
use crate::utils::now_rfc3339;
use rusqlite::{Connection, OptionalExtension, params};

fn record_intelligence_job_trigger(
    connection: &Connection,
    job_id: i64,
    run_id: Option<i64>,
    reason: Option<&str>,
    requested_at: &str,
) -> Result<()> {
    connection.execute(
        "INSERT INTO intelligence_job_triggers (job_id, run_id, reason, requested_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![job_id, run_id, reason, requested_at],
    )?;
    Ok(())
}

/// Enqueues one enrichment job for a history row when plugin/runtime rules allow it.
#[cfg(test)]
pub(crate) fn enqueue_enrichment_job(
    connection: &Connection,
    run_id: i64,
    plugin: &EnrichmentPluginDefinition,
    payload: &EnrichmentJobPayload,
) -> Result<()> {
    ensure_intelligence_runtime_schema(connection)?;
    let now = now_rfc3339();
    let dedupe_key = format!("{}:{}", plugin.id, payload.history_id);
    let payload_json = serde_json::to_string(payload)?;

    let existing = connection
        .query_row(
            "SELECT id, state
             FROM intelligence_jobs
             WHERE dedupe_key = ?1",
            [&dedupe_key],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;

    if let Some((job_id, state)) = existing {
        record_intelligence_job_trigger(
            connection,
            job_id,
            Some(run_id),
            Some("Deterministic rebuild requested matching enrichment."),
            &now,
        )?;
        if state != "running" {
            connection.execute(
                "UPDATE intelligence_jobs
                 SET state = 'queued',
                     priority = ?1,
                     payload_json = ?2,
                     scheduled_at = ?3,
                     started_at = NULL,
                     finished_at = NULL,
                     heartbeat_at = NULL,
                     lease_owner = NULL,
                     lease_expires_at = NULL,
                     updated_at = ?3,
                     last_error = NULL,
                     cancellation_reason = NULL,
                     stop_requested = 0
                 WHERE id = ?4",
                params![plugin.priority, payload_json, now, job_id],
            )?;
        }
    } else {
        connection.execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, updated_at)
             VALUES (?1, ?2, ?3, 'queued', ?4, 0, ?5, ?6, '{}', ?7, ?7, ?7)",
            params![
                ENRICHMENT_JOB_TYPE,
                plugin.id,
                run_id,
                plugin.priority,
                dedupe_key,
                payload_json,
                now,
            ],
        )?;
        record_intelligence_job_trigger(
            connection,
            connection.last_insert_rowid(),
            Some(run_id),
            Some("Deterministic rebuild scheduled enrichment."),
            &now,
        )?;
    }

    Ok(())
}

/// Enqueues one Core Intelligence job for the requested scope and stage.
pub fn enqueue_core_intelligence_job(
    connection: &Connection,
    job_type: &str,
    request: &CoreIntelligenceRebuildRequest,
    reason: &str,
) -> Result<i64> {
    ensure_intelligence_runtime_schema(connection)?;
    if !is_core_intelligence_job_type(job_type) {
        anyhow::bail!("'{job_type}' is not a valid Core Intelligence job type.");
    }
    let now = now_rfc3339();
    let payload = DeterministicRebuildJobPayload {
        job_type: job_type.to_string(),
        request: request.clone(),
        reason: reason.to_string(),
    };
    let payload_json = serde_json::to_string(&payload)?;
    let dedupe_key = format!(
        "core-intelligence:{}:{}:{}:{}",
        job_type,
        request.profile_id.as_deref().unwrap_or("all"),
        request.full_rebuild,
        request.limit.map(|limit| limit.max(1).to_string()).unwrap_or_else(|| "full".to_string()),
    );

    let existing = connection
        .query_row(
            "SELECT id, state FROM intelligence_jobs WHERE dedupe_key = ?1",
            [&dedupe_key],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;

    if let Some((job_id, state)) = existing {
        record_intelligence_job_trigger(connection, job_id, None, Some(reason), &now)?;
        if state != "running" {
            connection.execute(
                "UPDATE intelligence_jobs
                 SET state = 'queued',
                     priority = ?1,
                     job_type = ?2,
                     scheduled_at = ?3,
                     payload_json = ?4,
                     started_at = NULL,
                     finished_at = NULL,
                     heartbeat_at = NULL,
                     lease_owner = NULL,
                     lease_expires_at = NULL,
                     updated_at = ?3,
                     last_error = NULL,
                     cancellation_reason = NULL,
                     stop_requested = 0
                 WHERE id = ?5",
                params![
                    core_intelligence_job_priority(job_type),
                    job_type,
                    now,
                    payload_json,
                    job_id
                ],
            )?;
        }
        return Ok(job_id);
    }

    connection.execute(
        "INSERT INTO intelligence_jobs
         (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
          artifact_json, created_at, scheduled_at, updated_at)
         VALUES (?1, NULL, NULL, 'queued', ?2, 0, ?3, ?4, '{}', ?5, ?5, ?5)",
        params![job_type, core_intelligence_job_priority(job_type), dedupe_key, payload_json, now],
    )?;
    let job_id = connection.last_insert_rowid();
    record_intelligence_job_trigger(connection, job_id, None, Some(reason), &now)?;
    Ok(job_id)
}

/// Enqueues one full Core Intelligence rebuild job for the requested scope.
pub fn enqueue_deterministic_rebuild_job(
    connection: &Connection,
    request: &CoreIntelligenceRebuildRequest,
    reason: &str,
) -> Result<i64> {
    enqueue_core_intelligence_job(connection, RebuildMode::FullRebuild.job_type(), request, reason)
}

/// Persists runtime bookkeeping updates for deterministic modules.
pub(crate) fn persist_deterministic_module_runtime_updates(
    connection: &Connection,
    updates: &[DeterministicModuleRuntimeUpdate],
) -> Result<()> {
    ensure_intelligence_runtime_schema(connection)?;
    let now = now_rfc3339();
    for update in updates {
        let Some(definition) = built_in_deterministic_module(&update.module_id) else {
            continue;
        };
        connection.execute(
            "INSERT INTO deterministic_module_runtime
             (module_id, version, status, depends_on_json, derived_tables_json, last_run_id,
              last_built_at, last_invalidated_at, stale_reason, notes_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(module_id) DO UPDATE SET
               version = excluded.version,
               status = excluded.status,
               depends_on_json = excluded.depends_on_json,
               derived_tables_json = excluded.derived_tables_json,
               last_run_id = excluded.last_run_id,
               last_built_at = excluded.last_built_at,
               last_invalidated_at = excluded.last_invalidated_at,
               stale_reason = excluded.stale_reason,
               notes_json = excluded.notes_json,
               updated_at = excluded.updated_at",
            params![
                update.module_id,
                definition.version,
                update.status,
                serde_json::to_string(&definition.depends_on)?,
                serde_json::to_string(&definition.derived_tables)?,
                update.last_run_id,
                update.last_built_at,
                update.last_invalidated_at,
                update.stale_reason,
                serde_json::to_string(&update.notes)?,
                now,
            ],
        )?;
    }
    Ok(())
}

/// Marks every deterministic module as stale so the next rebuild can refresh them.
pub fn mark_all_deterministic_modules_stale(connection: &Connection, reason: &str) -> Result<()> {
    let now = now_rfc3339();
    let updates = built_in_deterministic_modules()
        .iter()
        .map(|module| DeterministicModuleRuntimeUpdate {
            module_id: module.id.to_string(),
            status: "stale".to_string(),
            last_run_id: None,
            last_built_at: None,
            last_invalidated_at: Some(now.clone()),
            stale_reason: Some(reason.to_string()),
            notes: vec![
                "Deterministic rebuild is required before these summaries are fresh again."
                    .to_string(),
            ],
        })
        .collect::<Vec<_>>();
    persist_deterministic_module_runtime_updates(connection, &updates)
}
