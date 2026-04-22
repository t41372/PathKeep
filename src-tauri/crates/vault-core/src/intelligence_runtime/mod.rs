//! Deterministic intelligence runtime boundary.
//!
//! ## Responsibilities
//! - Own the persisted queue/runtime schema for deterministic rebuild and
//!   enrichment jobs.
//! - Define the shared runtime payloads, module/plugin catalogs, and lease
//!   helpers used across enqueue, recovery, and snapshot code.
//! - Re-export the stable runtime entrypoints consumed by workers, read models,
//!   and tests without forcing all queue/recovery logic back into one file.
//!
//! ## Not responsible for
//! - Rebuilding deterministic intelligence facts themselves.
//! - Tauri command transport or worker-loop orchestration.
//! - UI-specific copy shaping beyond the persisted runtime snapshot fields.
//!
//! ## Dependencies
//! - `archive::open_intelligence_connection` for the derived intelligence DB.
//! - `intelligence_catalog` for built-in module metadata and rebuild labels.
//! - `models` for the serialized runtime snapshot surface shared with the shell.
//!
//! ## Performance notes
//! - Runtime snapshot loads must stay read-only and avoid recomputing
//!   deterministic insights.
//! - Queue recovery and lease handling operate inside the derived runtime DB,
//!   not by rescanning canonical archive facts.

mod claims;
mod enqueue;
mod job_control;
mod recovery;
mod snapshot;

#[cfg(test)]
mod tests_queue;
#[cfg(test)]
mod tests_runtime;

use crate::{
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
};
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(test)]
use std::thread::ThreadId;
use std::{
    collections::HashSet,
    path::Path,
    sync::{Mutex, OnceLock},
};

pub(crate) use self::claims::claim_enrichment_job_by_id;
#[cfg(test)]
pub(crate) use self::claims::claim_enrichment_jobs;
pub use self::claims::{
    claim_core_intelligence_job, claim_deterministic_rebuild_job, intelligence_job_stop_requested,
    next_queued_enrichment_job, next_queued_intelligence_job,
};
#[cfg(test)]
pub(crate) use self::enqueue::enqueue_enrichment_job;
pub use self::enqueue::mark_all_deterministic_modules_stale;
pub(crate) use self::enqueue::persist_deterministic_module_runtime_updates;
pub use self::enqueue::{enqueue_core_intelligence_job, enqueue_deterministic_rebuild_job};
pub use self::job_control::{
    cancel_intelligence_job, mark_intelligence_job_failed, mark_intelligence_job_succeeded,
    mark_running_intelligence_job_cancelled, retry_intelligence_job,
    update_intelligence_job_artifact,
};
pub use self::snapshot::load_intelligence_runtime;
pub(crate) use self::snapshot::load_intelligence_runtime_from_connection;
#[cfg(test)]
pub(crate) use self::snapshot::{
    load_intelligence_runtime_from_connection_call_count,
    reset_load_intelligence_runtime_from_connection_call_count,
};

pub(super) const INTELLIGENCE_RUNTIME_SCHEMA_SQL: &str = r#"
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
pub(super) const VISIT_DERIVE_PRIORITY: i64 = 20;
pub(super) const DAILY_ROLLUP_PRIORITY: i64 = 30;
pub(super) const STRUCTURAL_REBUILD_PRIORITY: i64 = 40;
pub(super) const FULL_REBUILD_PRIORITY: i64 = 50;
pub(super) const INTELLIGENCE_JOB_LEASE_SECONDS: i64 = 300;
/// Source-kind identifier for local-only enrichment plugins.
pub(crate) const LOCAL_PLUGIN_SOURCE_KIND: &str = "local";
/// Source-kind identifier for network-backed enrichment plugins.
pub(crate) const NETWORK_PLUGIN_SOURCE_KIND: &str = "network";

pub(super) static RECOVERED_RUNTIME_ARCHIVES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
#[cfg(test)]
pub(super) static LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_CALLS: AtomicUsize =
    AtomicUsize::new(0);
#[cfg(test)]
pub(super) static LOAD_INTELLIGENCE_RUNTIME_FROM_CONNECTION_MONITOR_THREAD: OnceLock<
    Mutex<Option<ThreadId>>,
> = OnceLock::new();

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
pub(super) struct IntelligenceJobArtifact {
    pub kind: Option<String>,
    pub phase: Option<String>,
    pub detail: Option<String>,
    pub completed_steps: Option<usize>,
    pub total_steps: Option<usize>,
    pub processed_items: Option<usize>,
    pub total_items: Option<usize>,
    pub progress_percent: Option<f32>,
    pub processed_visits: Option<usize>,
    pub card_count: Option<usize>,
    pub query_group_count: Option<usize>,
    pub thread_count: Option<usize>,
    pub execution_mode: Option<String>,
    pub affected_profiles: Option<Vec<String>>,
    pub dirty_visit_count: Option<usize>,
    pub dirty_date_keys: Option<Vec<String>>,
    pub fallback_reason: Option<String>,
    pub notes: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaimedEnrichmentJob {
    pub id: i64,
    pub plugin_id: String,
    pub attempt: usize,
    pub payload: EnrichmentJobPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedIntelligenceJob {
    pub id: i64,
    pub job_type: String,
}

pub(super) const BUILT_IN_ENRICHMENT_PLUGINS: [EnrichmentPluginDefinition; 2] = [
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

/// Ensures the persistent intelligence runtime tables exist.
pub(crate) fn ensure_intelligence_runtime_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(INTELLIGENCE_RUNTIME_SCHEMA_SQL)?;
    Ok(())
}

pub(super) fn is_core_intelligence_job_type(job_type: &str) -> bool {
    matches!(
        job_type,
        VISIT_DERIVE_JOB_TYPE
            | DAILY_ROLLUP_JOB_TYPE
            | STRUCTURAL_REBUILD_JOB_TYPE
            | FULL_REBUILD_JOB_TYPE
    )
}

pub(super) fn core_intelligence_job_priority(job_type: &str) -> i64 {
    match job_type {
        VISIT_DERIVE_JOB_TYPE => VISIT_DERIVE_PRIORITY,
        DAILY_ROLLUP_JOB_TYPE => DAILY_ROLLUP_PRIORITY,
        STRUCTURAL_REBUILD_JOB_TYPE => STRUCTURAL_REBUILD_PRIORITY,
        _ => FULL_REBUILD_PRIORITY,
    }
}

pub(super) fn core_intelligence_job_label(job_type: &str) -> &'static str {
    RebuildMode::from_job_type(job_type)
        .map(|mode| mode.label())
        .unwrap_or("core intelligence rebuild")
}

pub(super) fn lease_owner_label() -> String {
    format!("pathkeep:{}:{}", std::process::id(), std::thread::current().name().unwrap_or("main"))
}

pub(super) fn lease_expires_at(seconds: i64) -> String {
    (Utc::now() + Duration::seconds(seconds)).to_rfc3339()
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
