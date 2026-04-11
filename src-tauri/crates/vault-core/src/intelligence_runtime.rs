//! Deterministic intelligence runtime read models.
//!
//! This module summarizes background queue/plugin/module state for the
//! Settings/runtime surfaces. It reads persisted intelligence jobs and turns
//! them into a shell-friendly snapshot without recomputing the underlying
//! insight data.

use crate::{
    archive::{create_schema, open_archive_connection},
    config::ProjectPaths,
    models::{
        AppConfig, DeterministicModuleRuntimeStatus, EnrichmentPluginStatus,
        IntelligenceJobOverview, IntelligenceQueueStatus, IntelligenceRuntimeSnapshot,
        QUERY_GROUPS_MODULE_ID, QUERY_GROUPS_MODULE_VERSION, READABLE_CONTENT_PLUGIN_ID,
        REFERENCE_PAGES_MODULE_ID, REFERENCE_PAGES_MODULE_VERSION, SOURCE_EFFECTIVENESS_MODULE_ID,
        SOURCE_EFFECTIVENESS_MODULE_VERSION, TEMPLATE_SUMMARIES_MODULE_ID,
        TEMPLATE_SUMMARIES_MODULE_VERSION, THREADS_MODULE_ID, THREADS_MODULE_VERSION,
        TITLE_NORMALIZATION_PLUGIN_ID, merge_enrichment_plugin_preferences,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value as SqlValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
  updated_at TEXT NOT NULL,
  last_error TEXT,
  cancellation_reason TEXT,
  UNIQUE(dedupe_key)
);
CREATE INDEX IF NOT EXISTS idx_intelligence_jobs_state
  ON intelligence_jobs(job_type, state, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_intelligence_jobs_plugin_state
  ON intelligence_jobs(plugin_id, state, updated_at DESC);
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
/// Source-kind identifier for local-only enrichment plugins.
pub(crate) const LOCAL_PLUGIN_SOURCE_KIND: &str = "local";
/// Source-kind identifier for network-backed enrichment plugins.
pub(crate) const NETWORK_PLUGIN_SOURCE_KIND: &str = "network";

#[derive(Debug, Clone, Copy)]
pub(crate) struct EnrichmentPluginDefinition {
    pub id: &'static str,
    pub source_kind: &'static str,
    pub freshness_window_days: Option<i64>,
    pub priority: i64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeterministicModuleDefinition {
    pub id: &'static str,
    pub version: &'static str,
    pub depends_on: &'static [&'static str],
    pub derived_tables: &'static [&'static str],
}

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

#[derive(Debug, Clone)]
pub(crate) struct ClaimedEnrichmentJob {
    pub id: i64,
    pub plugin_id: String,
    pub attempt: usize,
    pub payload: EnrichmentJobPayload,
}

#[derive(Debug, Clone)]
struct QueuedEnrichmentJobSnapshot {
    id: i64,
    plugin_id: String,
    attempt: i64,
    payload_json: String,
    priority: i64,
    created_at: String,
}

#[derive(Debug, Clone)]
struct EnrichmentQueueCursor {
    priority: i64,
    created_at: String,
    id: i64,
}

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
        freshness_window_days: None,
        priority: 10,
    },
    EnrichmentPluginDefinition {
        id: READABLE_CONTENT_PLUGIN_ID,
        source_kind: NETWORK_PLUGIN_SOURCE_KIND,
        freshness_window_days: Some(7),
        priority: 30,
    },
];

const BUILT_IN_DETERMINISTIC_MODULES: [DeterministicModuleDefinition; 5] = [
    DeterministicModuleDefinition {
        id: QUERY_GROUPS_MODULE_ID,
        version: QUERY_GROUPS_MODULE_VERSION,
        depends_on: &[],
        derived_tables: &["insight_bursts", "insight_query_groups", "insight_query_group_members"],
    },
    DeterministicModuleDefinition {
        id: THREADS_MODULE_ID,
        version: THREADS_MODULE_VERSION,
        depends_on: &[QUERY_GROUPS_MODULE_ID],
        derived_tables: &["insight_threads", "insight_thread_members"],
    },
    DeterministicModuleDefinition {
        id: REFERENCE_PAGES_MODULE_ID,
        version: REFERENCE_PAGES_MODULE_VERSION,
        depends_on: &[QUERY_GROUPS_MODULE_ID, THREADS_MODULE_ID],
        derived_tables: &["insight_reference_pages"],
    },
    DeterministicModuleDefinition {
        id: SOURCE_EFFECTIVENESS_MODULE_ID,
        version: SOURCE_EFFECTIVENESS_MODULE_VERSION,
        depends_on: &[QUERY_GROUPS_MODULE_ID, THREADS_MODULE_ID, REFERENCE_PAGES_MODULE_ID],
        derived_tables: &["insight_source_effectiveness"],
    },
    DeterministicModuleDefinition {
        id: TEMPLATE_SUMMARIES_MODULE_ID,
        version: TEMPLATE_SUMMARIES_MODULE_VERSION,
        depends_on: &[QUERY_GROUPS_MODULE_ID, THREADS_MODULE_ID, REFERENCE_PAGES_MODULE_ID],
        derived_tables: &["insight_cards"],
    },
];

/// Ensures the persistent intelligence runtime tables exist.
pub(crate) fn ensure_intelligence_runtime_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(INTELLIGENCE_RUNTIME_SCHEMA_SQL)?;
    Ok(())
}

/// Returns the built-in enrichment plugin catalog.
pub(crate) fn built_in_enrichment_plugins() -> &'static [EnrichmentPluginDefinition] {
    &BUILT_IN_ENRICHMENT_PLUGINS
}

/// Looks up one built-in enrichment plugin definition by ID.
pub(crate) fn built_in_enrichment_plugin(
    plugin_id: &str,
) -> Option<&'static EnrichmentPluginDefinition> {
    built_in_enrichment_plugins().iter().find(|plugin| plugin.id == plugin_id)
}

/// Returns the built-in deterministic module catalog.
pub(crate) fn built_in_deterministic_modules() -> &'static [DeterministicModuleDefinition] {
    &BUILT_IN_DETERMINISTIC_MODULES
}

/// Looks up one built-in deterministic module definition by ID.
pub(crate) fn built_in_deterministic_module(
    module_id: &str,
) -> Option<&'static DeterministicModuleDefinition> {
    built_in_deterministic_modules().iter().find(|module| module.id == module_id)
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

    let existing_state = connection
        .query_row(
            "SELECT state FROM intelligence_jobs WHERE dedupe_key = ?1",
            [&dedupe_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    if existing_state.is_some() {
        connection.execute(
            "UPDATE intelligence_jobs
             SET run_id = ?1,
                 priority = ?2,
                 payload_json = ?3,
                 scheduled_at = ?4,
                 updated_at = ?4,
                 state = CASE WHEN state = 'running' THEN state ELSE 'queued' END,
                 started_at = CASE WHEN state = 'running' THEN started_at ELSE NULL END,
                 finished_at = CASE WHEN state = 'running' THEN finished_at ELSE NULL END,
                 last_error = CASE WHEN state = 'running' THEN last_error ELSE NULL END,
                 cancellation_reason = CASE WHEN state = 'running' THEN cancellation_reason ELSE NULL END
             WHERE dedupe_key = ?5",
            params![run_id, plugin.priority, payload_json, now, dedupe_key],
        )?;
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
    }

    Ok(())
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
pub(crate) fn mark_all_deterministic_modules_stale(
    connection: &Connection,
    reason: &str,
) -> Result<()> {
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
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'running',
             attempt = attempt + 1,
             started_at = ?1,
             finished_at = NULL,
             updated_at = ?1,
             last_error = NULL,
             cancellation_reason = NULL
         WHERE id = ?2
           AND state = 'queued'",
        params![claimed_at, job_id],
    )?;
    Ok(updated == 1)
}

/// Marks one intelligence job as succeeded.
pub(crate) fn mark_intelligence_job_succeeded(
    connection: &Connection,
    job_id: i64,
    artifact: &Value,
) -> Result<()> {
    let now = now_rfc3339();
    connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'succeeded', artifact_json = ?1, finished_at = ?2, updated_at = ?2,
             last_error = NULL, cancellation_reason = NULL
         WHERE id = ?3",
        params![serde_json::to_string(artifact)?, now, job_id],
    )?;
    Ok(())
}

/// Marks one intelligence job as failed.
pub(crate) fn mark_intelligence_job_failed(
    connection: &Connection,
    job_id: i64,
    error: &str,
) -> Result<()> {
    let now = now_rfc3339();
    connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'failed', finished_at = ?1, updated_at = ?1, last_error = ?2
         WHERE id = ?3",
        params![now, error, job_id],
    )?;
    Ok(())
}

/// Requeues any stuck running enrichment jobs globally.
pub(crate) fn requeue_running_enrichment_jobs(connection: &Connection) -> Result<usize> {
    ensure_intelligence_runtime_schema(connection)?;
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued', scheduled_at = ?1, started_at = NULL, finished_at = NULL,
             updated_at = ?1, last_error = NULL, cancellation_reason = NULL
         WHERE job_type = ?2 AND state = 'running'",
        params![now, ENRICHMENT_JOB_TYPE],
    )?;
    Ok(updated)
}

/// Requeues any stuck running enrichment jobs that belong to one run.
pub(crate) fn requeue_running_enrichment_jobs_for_run(
    connection: &Connection,
    run_id: i64,
) -> Result<usize> {
    ensure_intelligence_runtime_schema(connection)?;
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued', scheduled_at = ?1, started_at = NULL, finished_at = NULL,
             updated_at = ?1, last_error = NULL, cancellation_reason = NULL
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

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;

    let queue = load_queue_status(&connection)?;
    let plugins = load_plugin_statuses(&connection, config)?;
    let modules = load_module_statuses(&connection, config)?;
    let recent_jobs = load_recent_jobs(&connection)?;
    Ok(IntelligenceRuntimeSnapshot { queue, plugins, modules, recent_jobs, notes })
}

/// Retries one deterministic intelligence job if its current state allows it.
pub fn retry_intelligence_job(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;
    let state = load_job_state(&connection, job_id)?;
    if !matches!(state.as_str(), "failed" | "cancelled") {
        anyhow::bail!("Intelligence job {job_id} is in state '{state}' and cannot be retried.");
    }
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued', scheduled_at = ?1, updated_at = ?1, started_at = NULL,
             finished_at = NULL, last_error = NULL, cancellation_reason = NULL
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
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;
    let state = load_job_state(&connection, job_id)?;
    if !matches!(state.as_str(), "queued" | "running") {
        anyhow::bail!("Intelligence job {job_id} is in state '{state}' and cannot be cancelled.");
    }
    let now = now_rfc3339();
    let updated = connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'cancelled', finished_at = ?1, updated_at = ?1,
             last_error = NULL, cancellation_reason = 'cancelled from UI'
         WHERE id = ?2",
        params![now, job_id],
    )?;
    if updated == 0 {
        anyhow::bail!("Intelligence job {job_id} could not be cancelled.");
    }
    load_intelligence_runtime(paths, config, key)
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
         FROM intelligence_jobs
         WHERE job_type = ?1",
            [ENRICHMENT_JOB_TYPE],
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
        "SELECT id, job_type, plugin_id, state, attempt, payload_json, created_at, started_at,
                finished_at, last_error
         FROM intelligence_jobs
         WHERE job_type = ?1
         ORDER BY updated_at DESC, id DESC
         LIMIT 12",
    )?;
    statement
        .query_map([ENRICHMENT_JOB_TYPE], |row| {
            let payload_json: String = row.get(5)?;
            let payload = serde_json::from_str::<EnrichmentJobPayload>(&payload_json).ok();
            let state = row.get::<_, String>(3)?;
            Ok(IntelligenceJobOverview {
                id: row.get(0)?,
                job_type: row.get(1)?,
                plugin_id: row.get(2)?,
                state: state.clone(),
                history_id: payload.as_ref().map(|value| value.history_id),
                profile_id: payload.as_ref().map(|value| value.profile_id.clone()),
                url: payload.as_ref().map(|value| value.url.clone()),
                title: payload.as_ref().and_then(|value| value.title.clone()),
                attempt: row.get::<_, i64>(4)?.max(0) as usize,
                created_at: row.get(6)?,
                started_at: row.get(7)?,
                finished_at: row.get(8)?,
                last_error: row.get(9)?,
                retryable: matches!(state.as_str(), "failed" | "cancelled"),
                cancellable: matches!(state.as_str(), "queued" | "running"),
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
            open_archive_connection(&paths, &config, None).expect("open runtime archive");
        create_schema(&connection).expect("core schema");
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
            open_archive_connection(&paths, &config, None).expect("open runtime archive");
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
}
