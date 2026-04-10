use crate::{
    archive::{create_schema, open_archive_connection},
    config::ProjectPaths,
    models::{
        AppConfig, EnrichmentPluginStatus, IntelligenceJobOverview, IntelligenceQueueStatus,
        IntelligenceRuntimeSnapshot, READABLE_CONTENT_PLUGIN_ID, TITLE_NORMALIZATION_PLUGIN_ID,
        merge_enrichment_plugin_preferences,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
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
"#;

pub(crate) const ENRICHMENT_JOB_TYPE: &str = "enrichment-plugin";
pub(crate) const LOCAL_PLUGIN_SOURCE_KIND: &str = "local";
pub(crate) const NETWORK_PLUGIN_SOURCE_KIND: &str = "network";

#[derive(Debug, Clone, Copy)]
pub(crate) struct EnrichmentPluginDefinition {
    pub id: &'static str,
    pub source_kind: &'static str,
    pub freshness_window_days: Option<i64>,
    pub priority: i64,
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

pub(crate) fn ensure_intelligence_runtime_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(INTELLIGENCE_RUNTIME_SCHEMA_SQL)?;
    Ok(())
}

pub(crate) fn built_in_enrichment_plugins() -> &'static [EnrichmentPluginDefinition] {
    &BUILT_IN_ENRICHMENT_PLUGINS
}

pub(crate) fn built_in_enrichment_plugin(
    plugin_id: &str,
) -> Option<&'static EnrichmentPluginDefinition> {
    built_in_enrichment_plugins().iter().find(|plugin| plugin.id == plugin_id)
}

pub(crate) fn enrichment_plugin_enabled(config: &AppConfig, plugin_id: &str) -> bool {
    if !config.ai.enrichment_enabled {
        return false;
    }
    merge_enrichment_plugin_preferences(&config.ai.enrichment_plugins)
        .into_iter()
        .find(|plugin| plugin.plugin_id == plugin_id)
        .map(|plugin| plugin.enabled)
        .unwrap_or(false)
}

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

pub(crate) fn claim_enrichment_jobs(
    connection: &Connection,
    allowed_plugin_ids: &[String],
    allowed_history_ids: &std::collections::HashSet<i64>,
    limit: usize,
) -> Result<Vec<ClaimedEnrichmentJob>> {
    ensure_intelligence_runtime_schema(connection)?;
    let mut statement = connection.prepare(
        "SELECT id, plugin_id, attempt, payload_json
         FROM intelligence_jobs
         WHERE job_type = ?1 AND state = 'queued'
         ORDER BY priority ASC, created_at ASC, id ASC
         LIMIT ?2",
    )?;
    let candidate_rows = statement
        .query_map(params![ENRICHMENT_JOB_TYPE, (limit.max(1) * 4) as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut claimed = Vec::new();
    for (id, plugin_id, attempt, payload_json) in candidate_rows {
        let Some(plugin_id) = plugin_id else {
            continue;
        };
        if !allowed_plugin_ids.iter().any(|candidate| candidate == &plugin_id) {
            continue;
        }
        let payload = serde_json::from_str::<EnrichmentJobPayload>(&payload_json)
            .with_context(|| format!("parsing enrichment payload for job {id}"))?;
        if !allowed_history_ids.contains(&payload.history_id) {
            continue;
        }

        let now = now_rfc3339();
        connection.execute(
            "UPDATE intelligence_jobs
             SET state = 'running', attempt = attempt + 1, started_at = ?1, updated_at = ?1
             WHERE id = ?2",
            params![now, id],
        )?;
        claimed.push(ClaimedEnrichmentJob {
            id,
            plugin_id,
            attempt: attempt.max(0) as usize + 1,
            payload,
        });
        if claimed.len() >= limit.max(1) {
            break;
        }
    }

    Ok(claimed)
}

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
            recent_jobs: Vec::new(),
            notes,
        });
    }

    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;

    let queue = load_queue_status(&connection)?;
    let plugins = load_plugin_statuses(&connection, config)?;
    let recent_jobs = load_recent_jobs(&connection)?;
    Ok(IntelligenceRuntimeSnapshot { queue, plugins, recent_jobs, notes })
}

pub fn retry_intelligence_job(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;
    let now = now_rfc3339();
    connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'queued', scheduled_at = ?1, updated_at = ?1, started_at = NULL,
             finished_at = NULL, last_error = NULL, cancellation_reason = NULL
         WHERE id = ?2",
        params![now, job_id],
    )?;
    load_intelligence_runtime(paths, config, key)
}

pub fn cancel_intelligence_job(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    job_id: i64,
) -> Result<IntelligenceRuntimeSnapshot> {
    let connection = open_archive_connection(paths, config, key)?;
    create_schema(&connection)?;
    ensure_intelligence_runtime_schema(&connection)?;
    let now = now_rfc3339();
    connection.execute(
        "UPDATE intelligence_jobs
         SET state = 'cancelled', finished_at = ?1, updated_at = ?1,
             cancellation_reason = 'cancelled from UI'
         WHERE id = ?2",
        params![now, job_id],
    )?;
    load_intelligence_runtime(paths, config, key)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::ProjectPaths,
        models::{AppConfig, ArchiveMode},
        utils::test_env_lock,
    };
    use tempfile::tempdir;

    fn sample_paths(root: &std::path::Path) -> ProjectPaths {
        ProjectPaths {
            app_root: root.to_path_buf(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
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
        let mut config = AppConfig::default();
        config.initialized = true;
        config.archive_mode = ArchiveMode::Encrypted;
        config.ai.enrichment_enabled = false;
        assert!(!enrichment_plugin_enabled(&config, TITLE_NORMALIZATION_PLUGIN_ID));
    }
}
