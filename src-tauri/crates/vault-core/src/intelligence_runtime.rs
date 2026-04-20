//! Deterministic intelligence runtime read models.
//!
//! This module summarizes background queue/plugin/module state for the
//! Settings/runtime surfaces. It reads persisted intelligence jobs and turns
//! them into a shell-friendly snapshot without recomputing the underlying
//! insight data.

use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    intelligence_catalog::{
        IntelligenceModuleDescriptor, RebuildMode, built_in_intelligence_module_descriptor,
        built_in_intelligence_module_descriptors,
    },
    models::{
        AppConfig, CoreIntelligenceRebuildRequest, DeterministicModuleRuntimeStatus,
        EnrichmentPluginStatus, IntelligenceJobOverview, IntelligenceQueueStatus,
        IntelligenceRuntimeSnapshot, READABLE_CONTENT_PLUGIN_ID, TITLE_NORMALIZATION_PLUGIN_ID,
        merge_enrichment_plugin_preferences,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use rusqlite::{Connection, OptionalExtension, params};
#[cfg(test)]
use rusqlite::{params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(test)]
use std::thread::{self, ThreadId};
use std::{
    collections::HashSet,
    path::Path,
    sync::{Mutex, OnceLock},
};

const INTELLIGENCE_RUNTIME_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS intelligence_jobs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_type TEXT NOT NULL,
  plugin_id TEXT,
  run_id INTEGER,
  state TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 100,
  attempt INTEGER NOT NULL DEFAULT 0,
  dedupe_key TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  artifact_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  scheduled_at TEXT NOT NULL,
  started_at TEXT,
  finished_at TEXT,
  heartbeat_at TEXT,
  lease_owner TEXT,
  lease_expires_at TEXT,
  updated_at TEXT NOT NULL,
  last_error TEXT,
  cancellation_reason TEXT,
  stop_requested INTEGER NOT NULL DEFAULT 0,
  UNIQUE(dedupe_key)
);
CREATE INDEX IF NOT EXISTS idx_intelligence_jobs_state
  ON intelligence_jobs(job_type, state, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_intelligence_jobs_plugin_state
  ON intelligence_jobs(plugin_id, state, updated_at DESC);
CREATE TABLE IF NOT EXISTS intelligence_job_triggers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_id INTEGER NOT NULL REFERENCES intelligence_jobs(id) ON DELETE CASCADE,
  run_id INTEGER,
  reason TEXT,
  requested_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_intelligence_job_triggers_job_id
  ON intelligence_job_triggers(job_id, requested_at DESC);
CREATE TABLE IF NOT EXISTS deterministic_module_runtime (
  module_id TEXT PRIMARY KEY,
  version TEXT NOT NULL,
  status TEXT NOT NULL,
  depends_on_json TEXT NOT NULL,
  derived_tables_json TEXT NOT NULL,
  last_run_id INTEGER,
  last_built_at TEXT,
  last_invalidated_at TEXT,
  stale_reason TEXT,
  notes_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL
);
"#;

/// Queue job type identifier used for enrichment plugin jobs.
pub(crate) const ENRICHMENT_JOB_TYPE: &str = "enrichment-plugin";
/// Queue job type identifier used for visit-derived-facts rebuild work.
pub const VISIT_DERIVE_JOB_TYPE: &str = "visit-derive";
/// Queue job type identifier used for daily-rollup work.
pub const DAILY_ROLLUP_JOB_TYPE: &str = "daily-rollup";
/// Queue job type identifier used for structural entity rebuild work.
pub const STRUCTURAL_REBUILD_JOB_TYPE: &str = "structural-rebuild";
/// Queue job type identifier used for a full Core Intelligence rebuild.
pub const FULL_REBUILD_JOB_TYPE: &str = "full-rebuild";
#[allow(dead_code)]
const VISIT_DERIVE_PRIORITY: i64 = 20;
#[allow(dead_code)]
const DAILY_ROLLUP_PRIORITY: i64 = 30;
#[allow(dead_code)]
const STRUCTURAL_REBUILD_PRIORITY: i64 = 40;
const FULL_REBUILD_PRIORITY: i64 = 50;
const INTELLIGENCE_JOB_LEASE_SECONDS: i64 = 300;
/// Source-kind identifier for local-only enrichment plugins.
pub(crate) const LOCAL_PLUGIN_SOURCE_KIND: &str = "local";
/// Source-kind identifier for network-backed enrichment plugins.
pub(crate) const NETWORK_PLUGIN_SOURCE_KIND: &str = "network";

static RECOVERED_RUNTIME_ARCHIVES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
#[cfg(test)]
static LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_CALLS: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_MONITOR_THREAD: OnceLock<Mutex<Option<ThreadId>>> =
    OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub(crate) struct EnrichmentPluginDefinition {
    pub id: &'static str,
    pub source_kind: &'static str,
    #[cfg(test)]
    pub priority: i64,
}

pub(crate) type DeterministicModuleDefinition = IntelligenceModuleDescriptor;

#[derive(Debug, Clone)]
pub(crate) struct DeterministicModuleRuntimeUpdate {
    pub module_id: String,
    pub status: String,
    pub last_run_id: Option<i64>,
    pub last_built_at: Option<String>,
    pub last_invalidated_at: Option<String>,
    pub stale_reason: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnrichmentJobPayload {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicRebuildJobPayload {
    pub job_type: String,
    pub request: CoreIntelligenceRebuildRequest,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct IntelligenceJobArtifact {
    kind: Option<String>,
    phase: Option<String>,
    detail: Option<String>,
    completed_steps: Option<usize>,
    total_steps: Option<usize>,
    processed_items: Option<usize>,
    total_items: Option<usize>,
    progress_percent: Option<f32>,
    processed_visits: Option<usize>,
    card_count: Option<usize>,
    query_group_count: Option<usize>,
    thread_count: Option<usize>,
    execution_mode: Option<String>,
    affected_profiles: Option<Vec<String>>,
    dirty_visit_count: Option<usize>,
    dirty_date_keys: Option<Vec<String>>,
    fallback_reason: Option<String>,
    notes: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaimedEnrichmentJob {
    pub id: i64,
    pub plugin_id: String,
    pub attempt: usize,
    pub payload: EnrichmentJobPayload,
}

#[derive(Debug, Clone)]
#[cfg(test)]
struct QueuedEnrichmentJobSnapshot {
    id: i64,
    plugin_id: String,
    attempt: i64,
    payload_json: String,
    priority: i64,
    created_at: String,
}

#[derive(Debug, Clone)]
#[cfg(test)]
struct EnrichmentQueueCursor {
    priority: i64,
    created_at: String,
    id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedIntelligenceJob {
    pub id: i64,
    pub job_type: String,
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

const BUILT_IN_ENRICHMENT_PLUGINS: [EnrichmentPluginDefinition; 2] = [
    EnrichmentPluginDefinition {
        id: TITLE_NORMALIZATION_PLUGIN_ID,
        source_kind: LOCAL_PLUGIN_SOURCE_KIND,
        #[cfg(test)]
        priority: 10,
    },
    EnrichmentPluginDefinition {
        id: READABLE_CONTENT_PLUGIN_ID,
        source_kind: NETWORK_PLUGIN_SOURCE_KIND,
        #[cfg(test)]
        priority: 30,
    },
];

fn is_core_intelligence_job_type(job_type: &str) -> bool {
    matches!(
        job_type,
        VISIT_DERIVE_JOB_TYPE
            | DAILY_ROLLUP_JOB_TYPE
            | STRUCTURAL_REBUILD_JOB_TYPE
            | FULL_REBUILD_JOB_TYPE
    )
}

fn core_intelligence_job_priority(job_type: &str) -> i64 {
    match job_type {
        VISIT_DERIVE_JOB_TYPE => VISIT_DERIVE_PRIORITY,
        DAILY_ROLLUP_JOB_TYPE => DAILY_ROLLUP_PRIORITY,
        STRUCTURAL_REBUILD_JOB_TYPE => STRUCTURAL_REBUILD_PRIORITY,
        _ => FULL_REBUILD_PRIORITY,
    }
}

fn core_intelligence_job_label(job_type: &str) -> &'static str {
    RebuildMode::from_job_type(job_type)
        .map(|mode| mode.label())
        .unwrap_or("core intelligence rebuild")
}

/// Ensures the persistent intelligence runtime tables exist.
pub(crate) fn ensure_intelligence_runtime_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(INTELLIGENCE_RUNTIME_SCHEMA_SQL)?;
    Ok(())
}

fn lease_owner_label() -> String {
    format!("pathkeep:{}:{}", std::process::id(), std::thread::current().name().unwrap_or("main"))
}

fn lease_expires_at(seconds: i64) -> String {
    (Utc::now() + Duration::seconds(seconds)).to_rfc3339()
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

/// Returns the built-in enrichment plugin catalog.
pub(crate) fn built_in_enrichment_plugins() -> &'static [EnrichmentPluginDefinition] {
    &BUILT_IN_ENRICHMENT_PLUGINS
}

/// Looks up one built-in enrichment plugin definition by ID.
#[cfg(test)]
pub(crate) fn built_in_enrichment_plugin(
    plugin_id: &str,
) -> Option<&'static EnrichmentPluginDefinition> {
    built_in_enrichment_plugins().iter().find(|plugin| plugin.id == plugin_id)
}

/// Returns the built-in deterministic module catalog.
pub(crate) fn built_in_deterministic_modules() -> &'static [&'static DeterministicModuleDefinition]
{
    built_in_intelligence_module_descriptors()
}

/// Looks up one built-in deterministic module definition by ID.
pub(crate) fn built_in_deterministic_module(
    module_id: &str,
) -> Option<&'static DeterministicModuleDefinition> {
    built_in_intelligence_module_descriptor(module_id)
}

/// Returns whether one deterministic module is enabled in the current config.
pub(crate) fn deterministic_module_enabled(config: &AppConfig, module_id: &str) -> bool {
    config
        .deterministic
        .modules
        .iter()
        .find(|module| module.id == module_id)
        .map(|module| module.enabled)
        .unwrap_or(false)
}

/// Returns whether one enrichment plugin is enabled in the current config.
pub(crate) fn enrichment_plugin_enabled(config: &AppConfig, plugin_id: &str) -> bool {
    if !config.ai.enrichment_enabled {
        return false;
    }
    if let Some(plugin) = config.enrichment.plugins.iter().find(|plugin| plugin.id == plugin_id) {
        return plugin.enabled;
    }
    merge_enrichment_plugin_preferences(&config.ai.enrichment_plugins)
        .into_iter()
        .find(|plugin| plugin.plugin_id == plugin_id)
        .map(|plugin| plugin.enabled)
        .unwrap_or(false)
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
fn queued_enrichment_candidates_page(
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

fn try_claim_enrichment_job(
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

/// Requeues any stuck running enrichment jobs globally.
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

fn recover_expired_intelligence_jobs(connection: &Connection) -> Result<()> {
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

/// Returns the next queued intelligence job in worker execution order.
pub fn next_queued_intelligence_job(
    connection: &Connection,
) -> Result<Option<QueuedIntelligenceJob>> {
    ensure_intelligence_runtime_schema(connection)?;
    recover_expired_intelligence_jobs(connection)?;
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
    recover_expired_intelligence_jobs(connection)?;
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

/// Requeues any stuck running enrichment jobs that belong to one run.
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

/// Loads the combined runtime snapshot for deterministic intelligence jobs.
pub fn load_intelligence_runtime(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<IntelligenceRuntimeSnapshot> {
    let mut connection = open_intelligence_connection(paths, config, key)?;
    load_intelligence_runtime_from_connection(&mut connection, paths, config)
}

/// Loads the deterministic intelligence runtime snapshot through an existing
/// intelligence-plane connection.
///
/// This helper lets overview batches reuse one attached archive / SQLite
/// handle for both runtime metadata and section reads instead of reopening the
/// intelligence database for each step in the same foreground request.
pub fn load_intelligence_runtime_from_connection(
    connection: &mut Connection,
    paths: &ProjectPaths,
    config: &AppConfig,
) -> Result<IntelligenceRuntimeSnapshot> {
    #[cfg(test)]
    {
        let current_thread = thread::current().id();
        let should_record = LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_MONITOR_THREAD
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("load intelligence runtime monitor thread lock")
            .as_ref()
            .is_some_and(|thread_id| *thread_id == current_thread);
        if should_record {
            LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_CALLS.fetch_add(1, Ordering::Relaxed);
        }
    }
    let mut notes = Vec::new();
    if !config.ai.enrichment_enabled {
        notes.push(
            "Enrichment plugins are disabled in Settings. Deterministic insights still use canonical archive data."
                .to_string(),
        );
    }

    if !config.initialized || !paths.archive_database_path.exists() {
        if !config.initialized {
            notes.push(
                "Initialize the archive before queue-backed intelligence work can run.".to_string(),
            );
        }
        return Ok(IntelligenceRuntimeSnapshot {
            queue: IntelligenceQueueStatus::default(),
            plugins: empty_plugin_statuses(config),
            modules: empty_module_statuses(config),
            recent_jobs: Vec::new(),
            notes,
        });
    }

    ensure_intelligence_runtime_schema(connection)?;
    if should_recover_runtime_jobs(&paths.archive_database_path) {
        let recovered_deterministic_jobs = recover_interrupted_deterministic_jobs(connection)?;
        if recovered_deterministic_jobs > 0 {
            notes.push(format!(
                "Recovered {} interrupted deterministic rebuild job(s) after the previous session ended unexpectedly.",
                recovered_deterministic_jobs
            ));
        }
        let recovered_enrichment_jobs = requeue_running_enrichment_jobs(connection)?;
        if recovered_enrichment_jobs > 0 {
            notes.push(format!(
                "Recovered {} interrupted enrichment job(s) after the previous session ended unexpectedly.",
                recovered_enrichment_jobs
            ));
        }
    }

    recover_expired_intelligence_jobs(connection)?;
    let snapshot = connection.transaction()?;
    let queue = load_queue_status(&snapshot)?;
    let plugins = load_plugin_statuses(&snapshot, config)?;
    let modules = load_module_statuses(&snapshot, config)?;
    let recent_jobs = load_recent_jobs(&snapshot)?;
    snapshot.commit()?;
    Ok(IntelligenceRuntimeSnapshot { queue, plugins, modules, recent_jobs, notes })
}

#[cfg(test)]
pub(crate) fn reset_load_intelligence_runtime_from_connection_call_count() {
    LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_CALLS.store(0, Ordering::Relaxed);
    *LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_MONITOR_THREAD
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("load intelligence runtime monitor thread lock") = Some(thread::current().id());
}

#[cfg(test)]
pub(crate) fn load_intelligence_runtime_from_connection_call_count() -> usize {
    LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_CALLS.load(Ordering::Relaxed)
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
    let state = load_job_state(&connection, job_id)?;
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
    load_intelligence_runtime(paths, config, key)
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
    let state = load_job_state(&connection, job_id)?;
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
    load_intelligence_runtime(paths, config, key)
}

fn recover_interrupted_deterministic_jobs(connection: &Connection) -> Result<usize> {
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

fn should_recover_runtime_jobs(archive_database_path: &Path) -> bool {
    let recovered_archives = RECOVERED_RUNTIME_ARCHIVES.get_or_init(|| Mutex::new(HashSet::new()));
    let mut recovered_archives =
        recovered_archives.lock().expect("runtime recovery archive registry lock");
    recovered_archives.insert(archive_database_path.display().to_string())
}

fn load_job_state(connection: &Connection, job_id: i64) -> Result<String> {
    connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .with_context(|| format!("Intelligence job {job_id} was not found."))
}

fn empty_plugin_statuses(config: &AppConfig) -> Vec<EnrichmentPluginStatus> {
    built_in_enrichment_plugins()
        .iter()
        .map(|plugin| EnrichmentPluginStatus {
            plugin_id: plugin.id.to_string(),
            source_kind: plugin.source_kind.to_string(),
            enabled: enrichment_plugin_enabled(config, plugin.id),
            ..EnrichmentPluginStatus::default()
        })
        .collect()
}

fn empty_module_statuses(config: &AppConfig) -> Vec<DeterministicModuleRuntimeStatus> {
    built_in_deterministic_modules()
        .iter()
        .map(|module| DeterministicModuleRuntimeStatus {
            module_id: module.id.to_string(),
            enabled: deterministic_module_enabled(config, module.id),
            version: module.version.to_string(),
            status: if deterministic_module_enabled(config, module.id) {
                "idle".to_string()
            } else {
                "disabled".to_string()
            },
            depends_on: module.depends_on.iter().map(|value| (*value).to_string()).collect(),
            derived_tables: module
                .derived_tables
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            notes: if deterministic_module_enabled(config, module.id) {
                vec!["No deterministic rebuild has run yet for this module.".to_string()]
            } else {
                vec!["Disabled in Settings.".to_string()]
            },
            ..DeterministicModuleRuntimeStatus::default()
        })
        .collect()
}

fn load_queue_status(connection: &Connection) -> Result<IntelligenceQueueStatus> {
    connection
        .query_row(
            "SELECT
            SUM(CASE WHEN state = 'queued' THEN 1 ELSE 0 END),
            SUM(CASE WHEN state = 'running' THEN 1 ELSE 0 END),
            SUM(CASE WHEN state = 'succeeded' THEN 1 ELSE 0 END),
            SUM(CASE WHEN state = 'failed' THEN 1 ELSE 0 END),
            SUM(CASE WHEN state = 'cancelled' THEN 1 ELSE 0 END),
            MAX(updated_at)
         FROM intelligence_jobs",
            [],
            |row| {
                Ok(IntelligenceQueueStatus {
                    queued: row.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as usize,
                    running: row.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as usize,
                    succeeded: row.get::<_, Option<i64>>(2)?.unwrap_or(0).max(0) as usize,
                    failed: row.get::<_, Option<i64>>(3)?.unwrap_or(0).max(0) as usize,
                    cancelled: row.get::<_, Option<i64>>(4)?.unwrap_or(0).max(0) as usize,
                    last_activity_at: row.get(5)?,
                })
            },
        )
        .map_err(Into::into)
}

fn load_plugin_statuses(
    connection: &Connection,
    config: &AppConfig,
) -> Result<Vec<EnrichmentPluginStatus>> {
    let mut statuses = Vec::with_capacity(built_in_enrichment_plugins().len());
    for plugin in built_in_enrichment_plugins() {
        let stored_records = connection
            .query_row(
                "SELECT COUNT(*) FROM visit_content_enrichments WHERE content_source = ?1 AND fetch_status = 'success'",
                [plugin.id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            .max(0) as usize;
        let job_counts = connection.query_row(
            "SELECT
                SUM(CASE WHEN state = 'queued' THEN 1 ELSE 0 END),
                SUM(CASE WHEN state = 'running' THEN 1 ELSE 0 END),
                SUM(CASE WHEN state = 'failed' THEN 1 ELSE 0 END),
                MAX(CASE WHEN state = 'succeeded' THEN finished_at ELSE NULL END)
             FROM intelligence_jobs
             WHERE job_type = ?1 AND plugin_id = ?2",
            params![ENRICHMENT_JOB_TYPE, plugin.id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as usize,
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as usize,
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0).max(0) as usize,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )?;
        let last_error = connection
            .query_row(
                "SELECT last_error
                 FROM intelligence_jobs
                 WHERE job_type = ?1 AND plugin_id = ?2 AND state = 'failed'
                 ORDER BY updated_at DESC, id DESC
                 LIMIT 1",
                params![ENRICHMENT_JOB_TYPE, plugin.id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        statuses.push(EnrichmentPluginStatus {
            plugin_id: plugin.id.to_string(),
            source_kind: plugin.source_kind.to_string(),
            enabled: enrichment_plugin_enabled(config, plugin.id),
            stored_records,
            queued_jobs: job_counts.0,
            running_jobs: job_counts.1,
            failed_jobs: job_counts.2,
            last_completed_at: job_counts.3,
            last_error,
        });
    }
    Ok(statuses)
}

fn load_recent_jobs(connection: &Connection) -> Result<Vec<IntelligenceJobOverview>> {
    let mut statement = connection.prepare(
        "SELECT id, job_type, plugin_id, state, attempt, payload_json, artifact_json, created_at,
                started_at, finished_at, updated_at, heartbeat_at, last_error, stop_requested
         FROM intelligence_jobs
         ORDER BY updated_at DESC, id DESC
         LIMIT 12",
    )?;
    statement
        .query_map([], |row| {
            let payload_json: String = row.get(5)?;
            let artifact_json: String = row.get(6)?;
            let job_type = row.get::<_, String>(1)?;
            let payload = serde_json::from_str::<EnrichmentJobPayload>(&payload_json).ok();
            let rebuild_payload =
                serde_json::from_str::<DeterministicRebuildJobPayload>(&payload_json).ok();
            let artifact =
                serde_json::from_str::<IntelligenceJobArtifact>(&artifact_json).unwrap_or_default();
            let state = row.get::<_, String>(3)?;
            let title = match job_type.as_str() {
                type_id if is_core_intelligence_job_type(type_id) => {
                    rebuild_payload.as_ref().map(|payload| {
                        format!(
                            "{} · {}",
                            payload.request.profile_id.as_deref().unwrap_or("All profiles"),
                            core_intelligence_job_label(type_id),
                        )
                    })
                }
                _ => payload.as_ref().and_then(|value| value.title.clone()),
            };
            let progress_percent = artifact.progress_percent.map(|value| value.clamp(0.0, 100.0));
            let progress_current = artifact.processed_items.or(artifact.completed_steps);
            let progress_total = artifact.total_items.or(artifact.total_steps);
            Ok(IntelligenceJobOverview {
                id: row.get(0)?,
                job_type: job_type.clone(),
                plugin_id: row.get(2)?,
                state: state.clone(),
                history_id: payload.as_ref().map(|value| value.history_id),
                profile_id: payload.as_ref().map(|value| value.profile_id.clone()).or_else(|| {
                    rebuild_payload.as_ref().and_then(|value| value.request.profile_id.clone())
                }),
                url: payload.as_ref().map(|value| value.url.clone()),
                title,
                attempt: row.get::<_, i64>(4)?.max(0) as usize,
                created_at: row.get(7)?,
                started_at: row.get(8)?,
                finished_at: row.get(9)?,
                updated_at: row.get(10)?,
                heartbeat_at: if state == "running" {
                    row.get::<_, Option<String>>(11)?
                } else {
                    None
                },
                progress_label: artifact.phase,
                progress_detail: artifact.detail,
                progress_current,
                progress_total,
                progress_percent,
                execution_mode: artifact.execution_mode,
                affected_profiles: artifact.affected_profiles,
                dirty_visit_count: artifact.dirty_visit_count,
                dirty_date_keys: artifact.dirty_date_keys,
                fallback_reason: artifact.fallback_reason,
                last_error: row.get(12)?,
                retryable: matches!(state.as_str(), "failed" | "cancelled"),
                cancellable: match state.as_str() {
                    "queued" => true,
                    "running" => row.get::<_, i64>(13)? == 0,
                    _ => false,
                },
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_module_statuses(
    connection: &Connection,
    config: &AppConfig,
) -> Result<Vec<DeterministicModuleRuntimeStatus>> {
    let mut statement = connection.prepare(
        "SELECT module_id, version, status, depends_on_json, derived_tables_json, last_run_id,
                last_built_at, last_invalidated_at, stale_reason, notes_json
         FROM deterministic_module_runtime",
    )?;
    let stored = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut stored_map = std::collections::HashMap::new();
    for row in stored {
        stored_map.insert(row.0.clone(), row);
    }

    let mut statuses = Vec::with_capacity(built_in_deterministic_modules().len());
    for module in built_in_deterministic_modules() {
        let enabled = deterministic_module_enabled(config, module.id);
        let Some(stored_row) = stored_map.remove(module.id) else {
            statuses.push(DeterministicModuleRuntimeStatus {
                module_id: module.id.to_string(),
                enabled,
                version: module.version.to_string(),
                status: if enabled { "idle".to_string() } else { "disabled".to_string() },
                depends_on: module.depends_on.iter().map(|value| (*value).to_string()).collect(),
                derived_tables: module
                    .derived_tables
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                notes: if enabled {
                    vec!["No successful deterministic rebuild has been recorded yet.".to_string()]
                } else {
                    vec!["Disabled in Settings.".to_string()]
                },
                ..DeterministicModuleRuntimeStatus::default()
            });
            continue;
        };

        let mut status = stored_row.2;
        let mut stale_reason = stored_row.8;
        let mut notes = serde_json::from_str::<Vec<String>>(&stored_row.9).unwrap_or_default();
        if !enabled {
            status = "disabled".to_string();
            notes.push("Disabled in Settings.".to_string());
        } else if stored_row.1 != module.version {
            status = "stale".to_string();
            stale_reason =
                Some("Module version changed since the last deterministic rebuild.".to_string());
            notes.push(
                "The stored module version does not match the current built-in rule pack."
                    .to_string(),
            );
        } else if status == "ready" && stored_row.6.is_none() {
            status = "stale".to_string();
            stale_reason =
                Some("Missing build timestamp for the latest deterministic output.".to_string());
        }

        statuses.push(DeterministicModuleRuntimeStatus {
            module_id: module.id.to_string(),
            enabled,
            version: module.version.to_string(),
            status,
            depends_on: serde_json::from_str::<Vec<String>>(&stored_row.3).unwrap_or_else(|_| {
                module.depends_on.iter().map(|value| (*value).to_string()).collect()
            }),
            derived_tables: serde_json::from_str::<Vec<String>>(&stored_row.4).unwrap_or_else(
                |_| module.derived_tables.iter().map(|value| (*value).to_string()).collect(),
            ),
            last_run_id: stored_row.5,
            last_built_at: stored_row.6,
            last_invalidated_at: stored_row.7,
            stale_reason,
            notes,
        });
    }

    Ok(statuses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{ProjectPaths, ensure_paths, project_paths_with_root},
        models::{AppConfig, ArchiveMode},
        utils::test_env_lock,
    };
    use tempfile::tempdir;

    fn sample_paths(root: &std::path::Path) -> ProjectPaths {
        project_paths_with_root(root)
    }

    fn setup_runtime_archive() -> (tempfile::TempDir, ProjectPaths, AppConfig) {
        let root = tempdir().expect("tempdir");
        let paths = sample_paths(root.path());
        ensure_paths(&paths).expect("ensure paths");
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let connection =
            open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
        ensure_intelligence_runtime_schema(&connection).expect("runtime schema");
        (root, paths, config)
    }

    #[test]
    fn runtime_snapshot_returns_plugin_defaults_before_archive_exists() {
        let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempdir().expect("tempdir");
        let paths = sample_paths(root.path());
        let config = AppConfig::default();

        let snapshot = load_intelligence_runtime(&paths, &config, None).expect("runtime snapshot");
        assert_eq!(snapshot.plugins.len(), 2);
        assert!(snapshot.notes.iter().any(|note| note.contains("Initialize the archive")));
    }

    #[test]
    fn queue_jobs_can_be_enqueued_and_loaded() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_intelligence_runtime_schema(&connection).expect("queue schema");
        enqueue_enrichment_job(
            &connection,
            7,
            &BUILT_IN_ENRICHMENT_PLUGINS[0],
            &EnrichmentJobPayload {
                history_id: 3,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/docs".to_string(),
                title: Some("Docs".to_string()),
            },
        )
        .expect("enqueue");

        let queue = load_queue_status(&connection).expect("queue status");
        assert_eq!(queue.queued, 1);

        let jobs = load_recent_jobs(&connection).expect("recent jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].plugin_id.as_deref(), Some(TITLE_NORMALIZATION_PLUGIN_ID));
    }

    #[test]
    fn deterministic_rebuild_jobs_are_traced_in_runtime_queue() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_intelligence_runtime_schema(&connection).expect("queue schema");

        let job_id = enqueue_deterministic_rebuild_job(
            &connection,
            &CoreIntelligenceRebuildRequest {
                profile_id: Some("chrome:Default".to_string()),
                full_rebuild: true,
                limit: None,
            },
            "Archive changed.",
        )
        .expect("enqueue deterministic rebuild");

        let queue = load_queue_status(&connection).expect("queue status");
        assert_eq!(queue.queued, 1);

        let claimed =
            claim_deterministic_rebuild_job(&connection, job_id).expect("claim deterministic job");
        assert_eq!(
            claimed.expect("deterministic payload").request.profile_id.as_deref(),
            Some("chrome:Default")
        );

        let jobs = load_recent_jobs(&connection).expect("recent jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_type, FULL_REBUILD_JOB_TYPE);
        assert_eq!(
            jobs[0].title.as_deref(),
            Some("chrome:Default · full Core Intelligence rebuild")
        );
    }

    #[test]
    fn recent_jobs_surface_progress_for_running_deterministic_rebuilds() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_intelligence_runtime_schema(&connection).expect("queue schema");

        let job_id = enqueue_deterministic_rebuild_job(
            &connection,
            &CoreIntelligenceRebuildRequest::default(),
            "Archive changed.",
        )
        .expect("enqueue deterministic rebuild");
        claim_deterministic_rebuild_job(&connection, job_id).expect("claim deterministic rebuild");
        update_intelligence_job_artifact(
            &connection,
            job_id,
            &json!({
                "kind": "deterministic-rebuild",
                "phase": "Scoring visits",
                "detail": "12,000 / 64,781 visits",
                "completedSteps": 4,
                "totalSteps": 8,
                "processedItems": 12_000,
                "totalItems": 64_781,
                "progressPercent": 43.5,
                "executionMode": "incremental",
                "affectedProfiles": ["chrome:Default"],
                "dirtyVisitCount": 321,
                "dirtyDateKeys": ["2024-04-02", "2024-04-03"],
                "fallbackReason": null
            }),
        )
        .expect("update progress artifact");

        let jobs = load_recent_jobs(&connection).expect("recent jobs");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].progress_label.as_deref(), Some("Scoring visits"));
        assert_eq!(jobs[0].progress_detail.as_deref(), Some("12,000 / 64,781 visits"));
        assert_eq!(jobs[0].progress_current, Some(12_000));
        assert_eq!(jobs[0].progress_total, Some(64_781));
        assert_eq!(jobs[0].progress_percent, Some(43.5));
        assert_eq!(jobs[0].execution_mode.as_deref(), Some("incremental"));
        assert_eq!(jobs[0].affected_profiles.as_deref(), Some(&["chrome:Default".to_string()][..]));
        assert_eq!(jobs[0].dirty_visit_count, Some(321));
        assert_eq!(
            jobs[0].dirty_date_keys.as_deref(),
            Some(&["2024-04-02".to_string(), "2024-04-03".to_string()][..])
        );
        assert!(jobs[0].heartbeat_at.is_some());
    }

    #[test]
    fn deterministic_rebuild_jobs_run_before_optional_enrichment() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_intelligence_runtime_schema(&connection).expect("queue schema");

        enqueue_enrichment_job(
            &connection,
            7,
            &BUILT_IN_ENRICHMENT_PLUGINS[1],
            &EnrichmentJobPayload {
                history_id: 3,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/docs".to_string(),
                title: Some("Docs".to_string()),
            },
        )
        .expect("enqueue enrichment");

        let deterministic_job_id = enqueue_deterministic_rebuild_job(
            &connection,
            &CoreIntelligenceRebuildRequest::default(),
            "Archive changed.",
        )
        .expect("enqueue deterministic rebuild");

        let next_job = next_queued_intelligence_job(&connection)
            .expect("next queued job")
            .expect("queued job");
        assert_eq!(next_job.id, deterministic_job_id);
        assert_eq!(next_job.job_type, FULL_REBUILD_JOB_TYPE);
    }

    #[test]
    fn plugin_enabled_respects_global_toggle() {
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Encrypted,
            ..AppConfig::default()
        };
        config.ai.enrichment_enabled = false;
        assert!(!enrichment_plugin_enabled(&config, TITLE_NORMALIZATION_PLUGIN_ID));
    }

    #[test]
    fn running_jobs_can_be_requeued_globally_or_by_run() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_intelligence_runtime_schema(&connection).expect("queue schema");
        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, started_at, updated_at)
                 VALUES (?1, ?2, 11, 'running', 10, 1, 'job-1', ?3, '{}', ?4, ?4, ?4, ?4),
                        (?1, ?2, 12, 'running', 10, 1, 'job-2', ?3, '{}', ?4, ?4, ?4, ?4)",
                params![
                    ENRICHMENT_JOB_TYPE,
                    TITLE_NORMALIZATION_PLUGIN_ID,
                    serde_json::to_string(&EnrichmentJobPayload {
                        history_id: 3,
                        profile_id: "chrome:Default".to_string(),
                        url: "https://example.com/docs".to_string(),
                        title: Some("Docs".to_string()),
                    })
                    .expect("payload"),
                    now,
                ],
            )
            .expect("insert running jobs");

        let requeued_for_run =
            requeue_running_enrichment_jobs_for_run(&connection, 11).expect("requeue run jobs");
        assert_eq!(requeued_for_run, 1);
        let first_state = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("first state");
        let second_state = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-2'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("second state");
        assert_eq!(first_state, "queued");
        assert_eq!(second_state, "running");

        let requeued_all =
            requeue_running_enrichment_jobs(&connection).expect("requeue all running jobs");
        assert_eq!(requeued_all, 1);
        let final_state = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-2'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("final state");
        assert_eq!(final_state, "queued");
    }

    #[test]
    fn claim_enrichment_jobs_scans_past_disallowed_queue_fronts() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_intelligence_runtime_schema(&connection).expect("queue schema");
        let now = now_rfc3339();
        let payload = |history_id: i64| {
            serde_json::to_string(&EnrichmentJobPayload {
                history_id,
                profile_id: "chrome:Default".to_string(),
                url: format!("https://example.com/{history_id}"),
                title: Some(format!("Title {history_id}")),
            })
            .expect("payload")
        };

        for history_id in 1..=6 {
            connection
                .execute(
                    "INSERT INTO intelligence_jobs
                     (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key,
                      payload_json, artifact_json, created_at, scheduled_at, updated_at)
                     VALUES (?1, ?2, ?3, 'queued', 10, 0, ?4, ?5, '{}', ?6, ?6, ?6)",
                    params![
                        ENRICHMENT_JOB_TYPE,
                        TITLE_NORMALIZATION_PLUGIN_ID,
                        history_id,
                        format!("job-blocked-{history_id}"),
                        payload(history_id),
                        now,
                    ],
                )
                .expect("insert blocked queued job");
        }
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, updated_at)
                 VALUES (?1, ?2, 99, 'queued', 10, 0, 'job-target', ?3, '{}', ?4, ?4, ?4)",
                params![ENRICHMENT_JOB_TYPE, TITLE_NORMALIZATION_PLUGIN_ID, payload(99), now,],
            )
            .expect("insert target job");

        let claimed = claim_enrichment_jobs(
            &connection,
            &[TITLE_NORMALIZATION_PLUGIN_ID.to_string()],
            &std::collections::HashSet::from([99]),
            1,
        )
        .expect("claim enrichment jobs");

        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].payload.history_id, 99);
        let state = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-target'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("job state");
        assert_eq!(state, "running");
    }

    #[test]
    fn compare_and_set_claim_skips_jobs_taken_by_another_connection() {
        let root = tempdir().expect("tempdir");
        let database_path = root.path().join("queue.sqlite");
        let connection = Connection::open(&database_path).expect("open queue db");
        ensure_intelligence_runtime_schema(&connection).expect("queue schema");
        let competing_connection =
            Connection::open(&database_path).expect("open competing queue db");
        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, updated_at)
                 VALUES (?1, ?2, 11, 'queued', 10, 0, 'job-race', ?3, '{}', ?4, ?4, ?4)",
                params![
                    ENRICHMENT_JOB_TYPE,
                    TITLE_NORMALIZATION_PLUGIN_ID,
                    serde_json::to_string(&EnrichmentJobPayload {
                        history_id: 42,
                        profile_id: "chrome:Default".to_string(),
                        url: "https://example.com/race".to_string(),
                        title: Some("Race".to_string()),
                    })
                    .expect("payload"),
                    now,
                ],
            )
            .expect("insert queued job");

        let snapshot = queued_enrichment_candidates_page(
            &connection,
            &[TITLE_NORMALIZATION_PLUGIN_ID.to_string()],
            None,
            1,
        )
        .expect("queued candidate page");
        assert_eq!(snapshot.len(), 1);

        let claimed_at = now_rfc3339();
        assert!(
            try_claim_enrichment_job(&competing_connection, snapshot[0].id, &claimed_at)
                .expect("competing claim"),
            "the first claimer should win"
        );
        assert!(
            !try_claim_enrichment_job(&connection, snapshot[0].id, &claimed_at)
                .expect("stale snapshot claim"),
            "stale snapshots must not double-claim the same job"
        );

        let (state, attempt) = connection
            .query_row(
                "SELECT state, attempt FROM intelligence_jobs WHERE dedupe_key = 'job-race'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .expect("job state after competing claims");
        assert_eq!(state, "running");
        assert_eq!(attempt, 1);
    }

    #[test]
    fn retry_and_cancel_require_valid_job_states() {
        let (_root, paths, config) = setup_runtime_archive();
        let connection =
            open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, updated_at)
                 VALUES
                    (?1, ?2, 21, 'succeeded', 10, 1, 'job-succeeded', ?3, '{}', ?4, ?4, ?4),
                    (?1, ?2, 22, 'queued', 10, 0, 'job-queued', ?3, '{}', ?4, ?4, ?4)",
                params![
                    ENRICHMENT_JOB_TYPE,
                    TITLE_NORMALIZATION_PLUGIN_ID,
                    serde_json::to_string(&EnrichmentJobPayload {
                        history_id: 9,
                        profile_id: "chrome:Default".to_string(),
                        url: "https://example.com/docs".to_string(),
                        title: Some("Docs".to_string()),
                    })
                    .expect("payload"),
                    now,
                ],
            )
            .expect("insert jobs");

        let retry_error = retry_intelligence_job(&paths, &config, None, 1)
            .expect_err("retry should reject succeeded jobs");
        assert!(retry_error.to_string().contains("cannot be retried"));

        let cancel_error = cancel_intelligence_job(&paths, &config, None, 1)
            .expect_err("cancel should reject succeeded jobs");
        assert!(cancel_error.to_string().contains("cannot be cancelled"));

        cancel_intelligence_job(&paths, &config, None, 2).expect("cancel queued job");
        let cancelled_state = connection
            .query_row("SELECT state FROM intelligence_jobs WHERE id = 2", [], |row| {
                row.get::<_, String>(0)
            })
            .expect("cancelled state");
        assert_eq!(cancelled_state, "cancelled");
    }

    #[test]
    fn retry_and_cancel_fail_for_missing_jobs() {
        let (_root, paths, config) = setup_runtime_archive();

        let retry_error = retry_intelligence_job(&paths, &config, None, 404)
            .expect_err("retry should fail for missing jobs");
        assert!(retry_error.to_string().contains("404"));

        let cancel_error = cancel_intelligence_job(&paths, &config, None, 404)
            .expect_err("cancel should fail for missing jobs");
        assert!(cancel_error.to_string().contains("404"));
    }

    #[test]
    fn load_intelligence_runtime_recovers_interrupted_deterministic_jobs() {
        let (_root, paths, config) = setup_runtime_archive();
        let connection =
            open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, started_at, updated_at)
                 VALUES (?1, NULL, NULL, 'running', 5, 1, 'core-intelligence:all:false:full', ?2,
                         '{}', ?3, ?3, ?3, ?3)",
                params![
                    FULL_REBUILD_JOB_TYPE,
                    serde_json::to_string(&DeterministicRebuildJobPayload {
                        job_type: FULL_REBUILD_JOB_TYPE.to_string(),
                        request: CoreIntelligenceRebuildRequest::default(),
                        reason: "recover me".to_string(),
                    })
                    .expect("payload"),
                    now,
                ],
            )
            .expect("insert interrupted deterministic job");

        let snapshot =
            load_intelligence_runtime(&paths, &config, None).expect("load runtime snapshot");
        assert!(snapshot.notes.iter().any(|note| note.contains("Recovered 1 interrupted")));

        let (state, last_error) = connection
            .query_row(
                "SELECT state, last_error FROM intelligence_jobs WHERE job_type = ?1",
                [FULL_REBUILD_JOB_TYPE],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .expect("recovered job state");
        assert_eq!(state, "queued");
        assert!(
            last_error
                .expect("recovery note")
                .contains("restarted before this Core Intelligence job finished")
        );
    }

    #[test]
    fn load_intelligence_runtime_recovers_interrupted_enrichment_jobs() {
        let (_root, paths, config) = setup_runtime_archive();
        let connection =
            open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, started_at, updated_at)
                 VALUES (?1, ?2, 41, 'running', 10, 1, 'readable-content-refetch:1', ?3,
                         '{}', ?4, ?4, ?4, ?4)",
                params![
                    ENRICHMENT_JOB_TYPE,
                    READABLE_CONTENT_PLUGIN_ID,
                    serde_json::to_string(&EnrichmentJobPayload {
                        history_id: 1,
                        profile_id: "chrome:Default".to_string(),
                        url: "https://example.com/article".to_string(),
                        title: Some("Example".to_string()),
                    })
                    .expect("payload"),
                    now,
                ],
            )
            .expect("insert interrupted enrichment job");

        let snapshot =
            load_intelligence_runtime(&paths, &config, None).expect("load runtime snapshot");
        assert!(
            snapshot.notes.iter().any(|note| note.contains("Recovered 1 interrupted enrichment"))
        );

        let state = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE job_type = ?1",
                [ENRICHMENT_JOB_TYPE],
                |row| row.get::<_, String>(0),
            )
            .expect("recovered job state");
        assert_eq!(state, "queued");
    }

    #[test]
    fn load_intelligence_runtime_only_recovers_running_jobs_once_per_archive() {
        let (_root, paths, config) = setup_runtime_archive();
        let connection =
            open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, started_at, updated_at)
                 VALUES (?1, NULL, NULL, 'running', ?2, 1, 'core-intelligence:all:false:full', ?3,
                         '{}', ?4, ?4, ?4, ?4)",
                params![
                    FULL_REBUILD_JOB_TYPE,
                    FULL_REBUILD_PRIORITY,
                    serde_json::to_string(&DeterministicRebuildJobPayload {
                        job_type: FULL_REBUILD_JOB_TYPE.to_string(),
                        request: CoreIntelligenceRebuildRequest::default(),
                        reason: "recover me".to_string(),
                    })
                    .expect("payload"),
                    now,
                ],
            )
            .expect("insert interrupted deterministic job");

        let first_snapshot =
            load_intelligence_runtime(&paths, &config, None).expect("first runtime snapshot");
        assert!(
            first_snapshot
                .notes
                .iter()
                .any(|note| note.contains("Recovered 1 interrupted deterministic"))
        );

        connection
            .execute(
                "UPDATE intelligence_jobs
                 SET state = 'running',
                     started_at = ?1,
                     updated_at = ?1,
                     last_error = NULL
                 WHERE job_type = ?2",
                params![now_rfc3339(), FULL_REBUILD_JOB_TYPE],
            )
            .expect("mark deterministic job running again");

        let second_snapshot =
            load_intelligence_runtime(&paths, &config, None).expect("second runtime snapshot");
        assert!(
            second_snapshot
                .notes
                .iter()
                .all(|note| !note.contains("Recovered 1 interrupted deterministic"))
        );

        let state = connection
            .query_row(
                "SELECT state FROM intelligence_jobs WHERE job_type = ?1",
                [FULL_REBUILD_JOB_TYPE],
                |row| row.get::<_, String>(0),
            )
            .expect("deterministic job state after second load");
        assert_eq!(state, "running");
    }

    #[test]
    fn next_queued_intelligence_job_recovers_expired_leases() {
        let connection = Connection::open_in_memory().expect("memory db");
        ensure_intelligence_runtime_schema(&connection).expect("queue schema");
        let expired_at = (Utc::now() - Duration::minutes(10)).to_rfc3339();
        let now = now_rfc3339();
        connection
            .execute(
                "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
                  artifact_json, created_at, scheduled_at, started_at, heartbeat_at, lease_owner,
                  lease_expires_at, updated_at, stop_requested)
                 VALUES
                    (?1, NULL, NULL, 'running', ?2, 1, 'expired-queued', ?3, '{}', ?4, ?4, ?4, ?4, 'worker', ?5, ?4, 0),
                    (?6, NULL, NULL, 'running', ?7, 1, 'expired-cancelled', ?8, '{}', ?4, ?4, ?4, ?4, 'worker', ?5, ?4, 1)",
                params![
                    STRUCTURAL_REBUILD_JOB_TYPE,
                    STRUCTURAL_REBUILD_PRIORITY,
                    serde_json::to_string(&DeterministicRebuildJobPayload {
                        job_type: STRUCTURAL_REBUILD_JOB_TYPE.to_string(),
                        request: CoreIntelligenceRebuildRequest::default(),
                        reason: "expired lease".to_string(),
                    })
                    .expect("payload"),
                    now,
                    expired_at,
                    DAILY_ROLLUP_JOB_TYPE,
                    DAILY_ROLLUP_PRIORITY,
                    serde_json::to_string(&DeterministicRebuildJobPayload {
                        job_type: DAILY_ROLLUP_JOB_TYPE.to_string(),
                        request: CoreIntelligenceRebuildRequest::default(),
                        reason: "expired cancelled lease".to_string(),
                    })
                    .expect("payload"),
                ],
            )
            .expect("insert expired jobs");

        let next_job = next_queued_intelligence_job(&connection)
            .expect("next queued job after recovery")
            .expect("recovered queued job");
        assert_eq!(next_job.job_type, STRUCTURAL_REBUILD_JOB_TYPE);

        let states = connection
            .prepare(
                "SELECT dedupe_key, state, last_error
                 FROM intelligence_jobs
                 WHERE dedupe_key IN ('expired-queued', 'expired-cancelled')
                 ORDER BY dedupe_key ASC",
            )
            .expect("prepare recovered states")
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .expect("query recovered states")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect recovered states");
        assert_eq!(states[0].0, "expired-cancelled");
        assert_eq!(states[0].1, "cancelled");
        assert_eq!(states[1].0, "expired-queued");
        assert_eq!(states[1].1, "queued");
        assert!(
            states[1]
                .2
                .as_deref()
                .is_some_and(|value| value.contains("expired intelligence lease"))
        );
    }
}
