//! Failure modes: recovery repeats completed jobs or snapshots mutate queue state.
//! Deterministic intelligence runtime boundary.
//!
//! ## Responsibilities
//! - Expose the persisted queue/runtime contract for deterministic rebuild and
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
//! - `Vault` and `Session` for the dedicated intelligence database.
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
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) use self::claims::intelligence_job_stop_requested_in_session;
pub(crate) use self::claims::{
    claim_content_fetch_job_by_id, content_fetch_job_due, requeue_content_fetch_job_after,
};
pub(crate) use self::claims::{claim_enrichment_job_by_id, claim_result};
pub(crate) use self::enqueue::enqueue_content_fetch_job;
#[cfg(test)]
pub(crate) use self::enqueue::enqueue_enrichment_job;
pub(crate) use self::enqueue::persist_deterministic_module_runtime_updates;
pub(crate) use self::job_control::{
    mark_intelligence_job_failed_in_session, mark_intelligence_job_succeeded_in_session,
    mark_running_intelligence_job_cancelled_in_session,
};

pub(crate) use self::snapshot::load_intelligence_runtime_from_connection;

/// Queue job type identifier used for enrichment plugin jobs.
pub(crate) const ENRICHMENT_JOB_TYPE: &str = "enrichment-plugin";
/// Queue job type identifier used for W-ENRICH-1 site content-fetch jobs (06 §5).
///
/// A SEPARATE job type from the offline enrichment plugins so the content-fetch drain lane can be
/// kept low-concurrency + off the backup/import critical path, and so its negative-cache `scheduled_at`
/// gating is independent of the title-normalization queue.
pub(crate) const CONTENT_FETCH_JOB_TYPE: &str = "content-fetch";
/// Stored `plugin_id` for content-fetch jobs (the concrete extractor id is the stored `content_source`
/// on the resulting `visit_content_enrichments` row, not the job's plugin_id).
pub(crate) const CONTENT_FETCH_PLUGIN_ID: &str = "content-fetch";
/// Default priority for content-fetch jobs (lower = sooner; the working-set selector feeds the order).
pub(crate) const CONTENT_FETCH_PRIORITY: i64 = 60;
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
    /// Persisted diagnostic prose; the fallback channel for
    /// [`Self::stale_reason_code`], never the localization key.
    pub stale_reason: Option<String>,
    /// Persisted stable code for [`Self::stale_reason`]. Read-time derivations
    /// in `snapshot::load_module_statuses` (version drift, missing build
    /// timestamp) still override both fields; this column carries the reasons
    /// that only the *writer* knows (archive data changed, visibility repair).
    pub stale_reason_code: Option<String>,
    /// Coded notes; persisted as JSON so `notes_json` keeps the code channel.
    pub notes: Vec<crate::models::DerivedRuntimeNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnrichmentJobPayload {
    pub history_id: Option<i64>,
    pub profile_id: Option<String>,
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
    pub fallback_reason_code: Option<String>,
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
    if plugin_id == READABLE_CONTENT_PLUGIN_ID {
        return false;
    }
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

use crate::Vault;
use vault_store::{Db, OptionalRow, Priority};

/// Executes `intelligence_job_stop_requested` on the current storage owner.
pub fn intelligence_job_stop_requested(vault: &Vault, job_id: i64) -> Result<bool> {
    vault.read(Db::Intelligence, move |s| {
        claims::intelligence_job_stop_requested_in_session(s, job_id)
    })
}

/// Executes `claim_core_intelligence_job` on the current storage owner.
pub fn claim_core_intelligence_job(
    vault: &Vault,
    job_id: i64,
) -> Result<Option<DeterministicRebuildJobPayload>> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        claims::claim_result(claims::claim_core_intelligence_job_in_session(s, job_id))
    })?
}

/// Executes `claim_deterministic_rebuild_job` on the current storage owner.
pub fn claim_deterministic_rebuild_job(
    vault: &Vault,
    job_id: i64,
) -> Result<Option<DeterministicRebuildJobPayload>> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        claims::claim_result(claims::claim_deterministic_rebuild_job_in_session(s, job_id))
    })?
}

/// Executes `next_queued_intelligence_job` on the current storage owner.
pub fn next_queued_intelligence_job(vault: &Vault) -> Result<Option<QueuedIntelligenceJob>> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        claims::next_queued_intelligence_job_in_session(s)
    })
}

/// Executes `next_queued_enrichment_job` on the current storage owner.
pub fn next_queued_enrichment_job(vault: &Vault) -> Result<Option<i64>> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        claims::next_queued_enrichment_job_in_session(s)
    })
}

/// Executes `next_queued_content_fetch_job` on the current storage owner.
pub fn next_queued_content_fetch_job(vault: &Vault) -> Result<Option<i64>> {
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        claims::next_queued_content_fetch_job_in_session(s)
    })
}

/// Executes `next_content_fetch_schedule_eta_secs` on the current storage owner.
pub fn next_content_fetch_schedule_eta_secs(vault: &Vault) -> Result<Option<u64>> {
    vault.read(Db::Intelligence, claims::next_content_fetch_schedule_eta_secs_in_session)
}

/// Executes `enqueue_core_intelligence_job` on the current storage owner.
pub fn enqueue_core_intelligence_job(
    vault: &Vault,
    job_type: &str,
    request: &CoreIntelligenceRebuildRequest,
    reason: &str,
) -> Result<i64> {
    let job_type = job_type.to_owned();
    let request = request.clone();
    let reason = reason.to_owned();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        enqueue::enqueue_core_intelligence_job_in_session(s, &job_type, &request, &reason)
    })
}

/// Why the archive's Insights need a build that nothing has queued, if they do. Either the archive
/// has visits and no Insights build was attempted since the store was new or cleared (a fresh
/// install that imported visits outside a backup, a removed store, Insights data cleared in
/// Settings, whose dialog says they come back the next time PathKeep opens), so every Insights
/// read stays empty; or they were built with rules this version of PathKeep changed, so they show
/// the old labels until the next backup. Not while an Insights build is queued or running, nor
/// after the last one failed or was cancelled: Settings shows that and retrying is the user's call.
pub fn insights_build_needed(vault: &Vault) -> Result<Option<&'static str>> {
    let config = vault.config().clone();
    vault.read(Db::Intelligence, move |s| {
        let deterministic = [
            VISIT_DERIVE_JOB_TYPE,
            DAILY_ROLLUP_JOB_TYPE,
            STRUCTURAL_REBUILD_JOB_TYPE,
            FULL_REBUILD_JOB_TYPE,
        ];
        let latest = s
            .query_one(
                "SELECT state FROM intelligence_jobs WHERE job_type IN (?1,?2,?3,?4)
                 ORDER BY id DESC LIMIT 1",
                deterministic,
                |row| Ok(row.get::<String>(0)?),
            )
            .optional()?;
        match latest.as_deref() {
            None => {
                let has_visits = s.query_one(
                    "SELECT COALESCE((SELECT value FROM archive.archive_totals WHERE name='visits'), 0) > 0
                        AND NOT EXISTS(SELECT 1 FROM intelligence_generations)",
                    (),
                    |row| Ok(row.get::<i64>(0)? != 0),
                )?;
                Ok(has_visits.then_some("The archive has visits and its Insights were never built."))
            }
            Some("succeeded") if crate::intelligence::built_with_other_rules(s, &config)? => {
                Ok(Some(
                    "The Insights were built with rules this version of PathKeep changed, or in another time zone.",
                ))
            }
            Some(_) => Ok(None),
        }
    })
}

/// Executes `enqueue_deterministic_rebuild_job` on the current storage owner.
pub fn enqueue_deterministic_rebuild_job(
    vault: &Vault,
    request: &CoreIntelligenceRebuildRequest,
    reason: &str,
) -> Result<i64> {
    let request = request.clone();
    let reason = reason.to_owned();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        enqueue::enqueue_deterministic_rebuild_job_in_session(s, &request, &reason)
    })
}

/// Executes `mark_all_deterministic_modules_stale` on the current storage owner.
pub fn mark_all_deterministic_modules_stale(
    vault: &Vault,
    reason_code: &str,
    reason: &str,
) -> Result<()> {
    let reason_code = reason_code.to_owned();
    let reason = reason.to_owned();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        enqueue::mark_all_deterministic_modules_stale_in_session(s, &reason_code, &reason)
    })
}

/// Executes `mark_intelligence_job_succeeded` on the current storage owner.
pub fn mark_intelligence_job_succeeded(
    vault: &Vault,
    job_id: i64,
    artifact: &Value,
) -> Result<bool> {
    let artifact = artifact.clone();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        job_control::mark_intelligence_job_succeeded_in_session(s, job_id, &artifact)
    })
}

/// Executes `update_intelligence_job_artifact` on the current storage owner.
pub fn update_intelligence_job_artifact(
    vault: &Vault,
    job_id: i64,
    artifact: &Value,
) -> Result<()> {
    let artifact = artifact.clone();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        job_control::update_intelligence_job_artifact_in_session(s, job_id, &artifact)
    })
}

/// Executes `mark_intelligence_job_failed` on the current storage owner.
pub fn mark_intelligence_job_failed(vault: &Vault, job_id: i64, error: &str) -> Result<bool> {
    let error = error.to_owned();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        job_control::mark_intelligence_job_failed_in_session(s, job_id, &error)
    })
}

/// Executes `mark_running_intelligence_job_cancelled` on the current storage owner.
pub fn mark_running_intelligence_job_cancelled(
    vault: &Vault,
    job_id: i64,
    reason: &str,
) -> Result<bool> {
    let reason = reason.to_owned();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        job_control::mark_running_intelligence_job_cancelled_in_session(s, job_id, &reason)
    })
}

/// Executes `retry_intelligence_job` on the current storage owner.
pub fn retry_intelligence_job(vault: &Vault, job_id: i64) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        job_control::retry_intelligence_job_in_session(s, &paths, &config, job_id)
    })
}

/// Executes `cancel_intelligence_job` on the current storage owner.
pub fn cancel_intelligence_job(vault: &Vault, job_id: i64) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        job_control::cancel_intelligence_job_in_session(s, &paths, &config, job_id)
    })
}

/// Reads runtime state without opening files, installing schemas or recovering jobs.
pub fn load_intelligence_runtime(vault: &Vault) -> Result<IntelligenceRuntimeSnapshot> {
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    vault.read(Db::Intelligence, move |s| {
        snapshot::load_intelligence_runtime_from_connection(s, &paths, &config)
    })
}

/// The module states alone, for the status line of every Insights card; the full
/// snapshot also counts plugin records and lists recent jobs.
pub fn load_module_runtime(vault: &Vault) -> Result<Vec<DeterministicModuleRuntimeStatus>> {
    let config = vault.config().clone();
    vault.read(Db::Intelligence, move |s| snapshot::load_module_statuses(s, &config))
}

/// The exclusive owner reclaims unfinished work once at open, preserving completed jobs.
pub(crate) fn recover_owner_claims(store: &vault_store::Store) -> vault_store::Result<()> {
    store.write(Db::Intelligence,Priority::Interactive,|s| {
        s.execute("UPDATE intelligence_jobs SET state=CASE WHEN stop_requested=1 THEN 'cancelled' ELSE 'queued' END,scheduled_at=?1,updated_at=?1,started_at=NULL,finished_at=CASE WHEN stop_requested=1 THEN ?1 ELSE NULL END,lease_owner=NULL,lease_expires_at=NULL,heartbeat_at=NULL WHERE state='running'",[crate::utils::now_rfc3339()])?;
        Ok(())
    })
}

/// Reads every queue record through explicit continuation.
pub fn list_intelligence_jobs(
    vault: &Vault,
    cursor: Option<crate::models::IntelligenceJobCursor>,
    limit: usize,
) -> Result<crate::models::IntelligenceJobPage> {
    vault.read(Db::Intelligence, move |s| snapshot::load_job_page(s, cursor.as_ref(), limit))
}
