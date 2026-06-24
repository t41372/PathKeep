//! Tauri command façade for Core Intelligence runtime and export surfaces.
//!
//! ## Responsibilities
//!
//! - Preserve command names for rebuilds, runtime status, search rules, and trusted outputs.
//! - Pull session keys from Tauri state and pass them into the worker bridge.
//! - Keep heavy overview/runtime reads on the blocking-command path where required.
//!
//! ## Not responsible for
//!
//! - Executing rebuild stages or mutating derived-state tables directly.
//! - Owning Settings UI grammar for integration/export review.
//! - Changing external-output payload contracts.
//!
//! ## Dependencies
//!
//! - `run_blocking_command` for read paths that can touch SQLite or derived runtime state.
//! - `worker_bridge` for all domain behavior and string error mapping.
//!
//! ## Performance notes
//!
//! Overview/runtime reads can touch derived SQLite state, so they must remain
//! off the Tauri UI thread. Command handlers in this file should stay thin.

#[cfg(not(test))]
use super::super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Executes a Core Intelligence rebuild now instead of waiting for background work, off the UI
/// thread.
///
/// A synchronous rebuild scans and rewrites derived SQLite state — the single heaviest read-path
/// command here — so it must run on the blocking pool, never on the WebView thread.
pub(crate) async fn run_core_intelligence_now(
    request: vault_core::CoreIntelligenceRebuildRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceRebuildReport, String> {
    let key = state.get_key();
    run_blocking_command("run_core_intelligence_now", move || {
        worker_bridge::run_core_intelligence_now_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Queues one Core Intelligence rebuild so it can run through the background jobs surface, off the
/// UI thread.
#[allow(dead_code)]
pub(crate) async fn queue_core_intelligence_rebuild(
    request: vault_core::CoreIntelligenceRebuildRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceQueueReport, String> {
    let key = state.get_key();
    run_blocking_command("queue_core_intelligence_rebuild", move || {
        worker_bridge::queue_core_intelligence_rebuild_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Lists search-engine rules after applying archive-specific customizations, off the UI thread.
pub(crate) async fn list_search_engine_rules(
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::SearchEngineRule>, String> {
    let key = state.get_key();
    run_blocking_command("list_search_engine_rules", move || {
        worker_bridge::list_search_engine_rules_impl(key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Upserts one custom search-engine rule and returns the refreshed rule list, off the UI thread.
pub(crate) async fn upsert_search_engine_rule(
    input: vault_core::SearchEngineRuleInput,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::SearchEngineRule>, String> {
    let key = state.get_key();
    run_blocking_command("upsert_search_engine_rule", move || {
        worker_bridge::upsert_search_engine_rule_impl(input, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Deletes one custom search-engine rule and returns the refreshed rule list, off the UI thread.
pub(crate) async fn delete_search_engine_rule(
    rule_id: String,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::SearchEngineRule>, String> {
    let key = state.get_key();
    run_blocking_command("delete_search_engine_rule", move || {
        worker_bridge::delete_search_engine_rule_impl(rule_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the first-paint intelligence overview through the blocking bridge.
pub(crate) async fn get_intelligence_primary_overview(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligencePrimaryOverview, String> {
    let session_database_key = state.get_key();
    run_blocking_command("get_intelligence_primary_overview", move || {
        worker_bridge::get_intelligence_primary_overview_impl(
            request,
            session_database_key.as_deref(),
        )
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads secondary overview sections after the primary route payload is visible.
pub(crate) async fn get_intelligence_secondary_overview(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSecondaryOverview, String> {
    let session_database_key = state.get_key();
    run_blocking_command("get_intelligence_secondary_overview", move || {
        worker_bridge::get_intelligence_secondary_overview_impl(
            request,
            session_database_key.as_deref(),
        )
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads trusted embed-card payloads for manual external-output review, off the UI thread.
pub(crate) async fn get_intelligence_embed_cards(
    request: vault_core::IntelligenceEmbedCardsRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::IntelligenceEmbedCardPayload>, String> {
    let key = state.get_key();
    run_blocking_command("get_intelligence_embed_cards", move || {
        worker_bridge::get_intelligence_embed_cards_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads a trusted widget snapshot payload without building a host artifact, off the UI thread.
pub(crate) async fn get_intelligence_widget_snapshot(
    request: vault_core::IntelligenceEmbedCardsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceWidgetSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("get_intelligence_widget_snapshot", move || {
        worker_bridge::get_intelligence_widget_snapshot_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the public redacted snapshot payload for manual review/export, off the UI thread.
pub(crate) async fn get_intelligence_public_snapshot(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligencePublicSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("get_intelligence_public_snapshot", move || {
        worker_bridge::get_intelligence_public_snapshot_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Previews the files that would be generated for a trusted local host build, off the UI thread.
pub(crate) async fn preview_intelligence_local_host(
    request: vault_core::IntelligenceLocalHostRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceLocalHostPreview, String> {
    let key = state.get_key();
    run_blocking_command("preview_intelligence_local_host", move || {
        worker_bridge::preview_intelligence_local_host_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Builds the trusted local host artifact after manual review, off the UI thread.
pub(crate) async fn build_intelligence_local_host(
    request: vault_core::IntelligenceLocalHostRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceLocalHostBuildResult, String> {
    let key = state.get_key();
    run_blocking_command("build_intelligence_local_host", move || {
        worker_bridge::build_intelligence_local_host_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Returns queue/runtime state for deterministic intelligence and enrichment work.
pub(crate) async fn load_intelligence_runtime(
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    let session_database_key = state.get_key();
    run_blocking_command("load_intelligence_runtime", move || {
        worker_bridge::load_intelligence_runtime_impl(session_database_key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Retries one deterministic intelligence job immediately, off the UI thread.
pub(crate) async fn retry_intelligence_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("retry_intelligence_job", move || {
        worker_bridge::retry_intelligence_job_impl(job_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Cancels one deterministic intelligence job, off the UI thread.
pub(crate) async fn cancel_intelligence_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("cancel_intelligence_job", move || {
        worker_bridge::cancel_intelligence_job_impl(job_id, key.as_deref())
    })
    .await
}
