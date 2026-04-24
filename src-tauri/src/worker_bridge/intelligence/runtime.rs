//! Worker bridge adapters for Core Intelligence runtime and export commands.
//!
//! ## Responsibilities
//!
//! - Keep Tauri command handlers decoupled from `vault-worker` function names.
//! - Preserve the `Result<_, String>` desktop error envelope.
//! - Forward runtime, search-rule, and trusted-output requests without reshaping payloads.
//!
//! ## Not responsible for
//!
//! - Running rebuild stages or managing queue leases directly.
//! - Generating local host artifacts in the desktop façade.
//! - Owning Settings or Jobs presentation decisions.
//!
//! ## Dependencies
//!
//! - `vault_worker` for runtime orchestration and output builders.
//! - `worker_result` for error envelope normalization.
//!
//! ## Performance notes
//!
//! Adapters here must not do blocking work themselves; expensive reads and
//! rebuilds are handled by worker/core code and, where needed, the command layer
//! wraps them in `run_blocking_command`.

use vault_core::{
    CoreIntelligenceRebuildRequest, IntelligenceEmbedCardsRequest, IntelligenceLocalHostRequest,
    ScopedDateRangeRequest, SearchEngineRuleInput,
};

use super::super::worker_result;

#[cfg_attr(test, allow(dead_code))]
/// Clears rebuildable intelligence state while leaving canonical archive facts untouched.
pub(crate) fn clear_derived_intelligence_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, String> {
    worker_result(vault_worker::clear_derived_intelligence(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Rebuilds Core Intelligence immediately.
pub(crate) fn run_core_intelligence_now_impl(
    request: CoreIntelligenceRebuildRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceRebuildReport, String> {
    worker_result(vault_worker::run_core_intelligence_now(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
#[cfg_attr(not(test), allow(dead_code))]
/// Queues one Core Intelligence rebuild so heavy work can stay in the background.
pub(crate) fn queue_core_intelligence_rebuild_impl(
    request: CoreIntelligenceRebuildRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceQueueReport, String> {
    worker_result(vault_worker::queue_core_intelligence_rebuild(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Lists search-engine rules after applying archive-specific customizations.
pub(crate) fn list_search_engine_rules_impl(
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::SearchEngineRule>, String> {
    worker_result(vault_worker::list_search_engine_rules(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Upserts one custom search-engine rule and returns the refreshed rule list.
pub(crate) fn upsert_search_engine_rule_impl(
    input: SearchEngineRuleInput,
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::SearchEngineRule>, String> {
    worker_result(vault_worker::upsert_search_engine_rule(session_database_key, &input))
}

#[cfg_attr(test, allow(dead_code))]
/// Deletes one custom search-engine rule and returns the refreshed rule list.
pub(crate) fn delete_search_engine_rule_impl(
    rule_id: String,
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::SearchEngineRule>, String> {
    worker_result(vault_worker::delete_search_engine_rule(session_database_key, &rule_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the first-paint intelligence overview payload.
pub(crate) fn get_intelligence_primary_overview_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligencePrimaryOverview, String> {
    worker_result(vault_worker::get_intelligence_primary_overview(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads secondary overview sections after the primary route payload is visible.
pub(crate) fn get_intelligence_secondary_overview_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSecondaryOverview, String> {
    worker_result(vault_worker::get_intelligence_secondary_overview(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads trusted embed-card payloads for manual external-output review.
pub(crate) fn get_intelligence_embed_cards_impl(
    request: IntelligenceEmbedCardsRequest,
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::IntelligenceEmbedCardPayload>, String> {
    worker_result(vault_worker::get_intelligence_embed_cards(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads a trusted widget snapshot payload without building a host artifact.
pub(crate) fn get_intelligence_widget_snapshot_impl(
    request: IntelligenceEmbedCardsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceWidgetSnapshot, String> {
    worker_result(vault_worker::get_intelligence_widget_snapshot(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the public redacted snapshot payload for manual review/export.
pub(crate) fn get_intelligence_public_snapshot_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligencePublicSnapshot, String> {
    worker_result(vault_worker::get_intelligence_public_snapshot(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Previews the files that would be generated for a trusted local host build.
pub(crate) fn preview_intelligence_local_host_impl(
    request: IntelligenceLocalHostRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceLocalHostPreview, String> {
    worker_result(vault_worker::preview_intelligence_local_host(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Builds the trusted local host artifact after manual review.
pub(crate) fn build_intelligence_local_host_impl(
    request: IntelligenceLocalHostRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceLocalHostBuildResult, String> {
    worker_result(vault_worker::build_intelligence_local_host(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the combined runtime snapshot for intelligence queues and plugins.
pub(crate) fn load_intelligence_runtime_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::load_intelligence_runtime_snapshot(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Retries one deterministic intelligence job.
pub(crate) fn retry_intelligence_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::retry_intelligence_job_now(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Cancels one deterministic intelligence job.
pub(crate) fn cancel_intelligence_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::cancel_intelligence_job_now(session_database_key, job_id))
}
