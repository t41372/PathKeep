//! Runtime snapshot read models.
//!
//! ## Responsibilities
//! - Load the shell-facing runtime snapshot without recomputing deterministic
//!   intelligence facts.
//! - Project queue, plugin, module, and recent-job state from the derived
//!   runtime DB into typed read models.
//! - Trigger one-time recovery hooks before foreground snapshot reads.

use super::*;
use crate::{archive::open_intelligence_connection, config::ProjectPaths};
use rusqlite::{Connection, OptionalExtension, params};

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
pub(crate) fn load_intelligence_runtime_from_connection(
    connection: &mut Connection,
    paths: &ProjectPaths,
    config: &AppConfig,
) -> Result<IntelligenceRuntimeSnapshot> {
    #[cfg(test)]
    {
        let current_thread = std::thread::current().id();
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
    if super::recovery::should_recover_runtime_jobs(&paths.archive_database_path) {
        let recovered_deterministic_jobs =
            super::recovery::recover_interrupted_deterministic_jobs(connection)?;
        if recovered_deterministic_jobs > 0 {
            notes.push(format!(
                "Recovered {} interrupted deterministic rebuild job(s) after the previous session ended unexpectedly.",
                recovered_deterministic_jobs
            ));
        }
        let recovered_enrichment_jobs =
            super::recovery::requeue_running_enrichment_jobs(connection)?;
        if recovered_enrichment_jobs > 0 {
            notes.push(format!(
                "Recovered {} interrupted enrichment job(s) after the previous session ended unexpectedly.",
                recovered_enrichment_jobs
            ));
        }
    }

    super::recovery::recover_expired_intelligence_jobs(connection)?;
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
        .expect("load intelligence runtime monitor thread lock") =
        Some(std::thread::current().id());
}

#[cfg(test)]
pub(crate) fn load_intelligence_runtime_from_connection_call_count() -> usize {
    LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_CALLS.load(Ordering::Relaxed)
}

pub(super) fn load_job_state(connection: &Connection, job_id: i64) -> Result<String> {
    connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = ?1", [job_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .with_context(|| format!("Intelligence job {job_id} was not found."))
}

pub(super) fn empty_plugin_statuses(config: &AppConfig) -> Vec<EnrichmentPluginStatus> {
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

pub(super) fn empty_module_statuses(config: &AppConfig) -> Vec<DeterministicModuleRuntimeStatus> {
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

pub(super) fn load_queue_status(connection: &Connection) -> Result<IntelligenceQueueStatus> {
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

pub(super) fn load_plugin_statuses(
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

pub(super) fn load_recent_jobs(connection: &Connection) -> Result<Vec<IntelligenceJobOverview>> {
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

pub(super) fn load_module_statuses(
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
