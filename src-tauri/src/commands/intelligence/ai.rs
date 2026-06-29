//! Tauri command façade for optional AI provider, queue, search, and assistant flows.
//!
//! ## Responsibilities
//!
//! - Preserve frontend-facing command names for optional AI features.
//! - Pass the current session key into the worker bridge.
//! - Keep provider secret, queue, semantic search, and assistant commands thin.
//!
//! ## Not responsible for
//!
//! - Calling LLM or embedding providers directly.
//! - Managing AI queue lifecycle transitions or semantic index writes.
//! - Treating optional AI failures as Core Intelligence availability failures.
//!
//! ## Dependencies
//!
//! - `SessionState` for the optional in-memory database key.
//! - `worker_bridge` for provider, queue, index, search, and assistant behavior.
//!
//! ## Performance notes
//!
//! Commands in this file must not run model/index work on the Tauri UI thread;
//! heavy work stays behind worker-owned queue and index entrypoints. The
//! conversation-persistence commands touch SQLite synchronously (a save does a
//! full DELETE + re-INSERT of a transcript), so they hop onto the blocking
//! thread pool via `run_blocking_command` — on the 14.4M-record baseline even a
//! bounded agent-plane write must never block the WebView thread.

#[cfg(not(test))]
use super::super::blocking::run_blocking_command;
#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

#[cfg(not(test))]
#[tauri::command]
/// Stores one AI provider secret and returns the refreshed app snapshot, off the UI thread.
///
/// Writes the secret to the OS keyring and rebuilds the config snapshot — both synchronous I/O, so
/// the work hops onto the blocking pool to keep the WebView thread free.
pub(crate) async fn store_ai_provider_api_key(
    input: vault_core::AiProviderSecretInput,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("store_ai_provider_api_key", move || {
        worker_bridge::store_ai_provider_api_key_impl(input, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Removes one stored AI provider secret and returns the refreshed app snapshot, off the UI thread.
pub(crate) async fn clear_ai_provider_api_key(
    provider_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("clear_ai_provider_api_key", move || {
        worker_bridge::clear_ai_provider_api_key_impl(provider_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Probes an AI provider connection without launching archive-wide jobs, off the UI thread.
///
/// The probe makes a real provider network round-trip (an embedding or a short chat completion). A
/// slow or misconfigured endpoint can take seconds, so it MUST run on the blocking pool — never on
/// the WebView thread, or the whole UI freezes until the network call returns.
pub(crate) async fn test_ai_provider_connection(
    request: vault_core::AiProviderConnectionTestRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiProviderConnectionTestReport, String> {
    let key = state.get_key();
    run_blocking_command("test_ai_provider_connection", move || {
        worker_bridge::test_ai_provider_connection_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the persisted AI queue status read model, off the UI thread.
pub(crate) async fn load_ai_queue_status(
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueStatus, String> {
    let key = state.get_key();
    run_blocking_command("load_ai_queue_status", move || {
        worker_bridge::load_ai_queue_status_impl(key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Drains queued AI jobs up to the requested limit, off the UI thread.
pub(crate) async fn run_ai_queue_jobs(
    max_jobs: Option<u32>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueStatus, String> {
    let key = state.get_key();
    run_blocking_command("run_ai_queue_jobs", move || {
        worker_bridge::run_ai_queue_jobs_impl(max_jobs, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Requeues one failed or skipped AI job, off the UI thread.
pub(crate) async fn replay_ai_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueJob, String> {
    let key = state.get_key();
    run_blocking_command("replay_ai_job", move || {
        worker_bridge::replay_ai_job_impl(job_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Cancels one queued or running AI job, off the UI thread.
pub(crate) async fn cancel_ai_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueJob, String> {
    let key = state.get_key();
    run_blocking_command("cancel_ai_job", move || {
        worker_bridge::cancel_ai_job_impl(job_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the persisted assistant result for one queue-backed job, off the UI thread.
pub(crate) async fn load_ai_assistant_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiAssistantResponse, String> {
    let key = state.get_key();
    run_blocking_command("load_ai_assistant_job", move || {
        worker_bridge::load_ai_assistant_job_impl(job_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Builds or refreshes the semantic index, off the UI thread.
pub(crate) async fn build_ai_index(
    request: vault_core::AiIndexRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiIndexReport, String> {
    let key = state.get_key();
    run_blocking_command("build_ai_index", move || {
        worker_bridge::build_ai_index_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Recovers a wedged semantic-index build (clears stuck job(s) + re-enqueues a clean one), off the
/// UI thread (F2).
pub(crate) async fn reset_ai_index_build(
    request: vault_core::AiIndexRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiIndexReport, String> {
    let key = state.get_key();
    run_blocking_command("reset_ai_index_build", move || {
        worker_bridge::reset_ai_index_build_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Returns a read-only cost/time estimate for a re-embed run of the given scope (W-AI-9 Sub-block D).
///
/// No model load, no embedding, no network: it sizes the work (bounded working-set length or unique
/// page count) so the FE can show the cost BEFORE the user fires a re-embed (PME). The sizing query
/// still touches SQLite, so it runs on the blocking pool.
pub(crate) async fn estimate_reembed(
    scope: vault_core::ReembedScope,
    state: State<'_, SessionState>,
) -> Result<vault_core::ReembedEstimate, String> {
    let key = state.get_key();
    run_blocking_command("estimate_reembed", move || {
        worker_bridge::estimate_reembed_impl(scope, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Runs semantic-plus-lexical search over visible archive history, off the UI thread.
///
/// Semantic search embeds the query (network round-trip to the embedding provider) and scans the
/// vector index, so it must never run on the WebView thread.
pub(crate) async fn search_ai_history(
    request: vault_core::AiSearchRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiSearchResponse, String> {
    let key = state.get_key();
    run_blocking_command("search_ai_history", move || {
        worker_bridge::search_ai_history_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Asks the first-party assistant to answer a question with archive citations, off the UI thread.
///
/// The assistant runs an LLM agent loop (multiple network round-trips), so it must stay on the
/// blocking pool.
pub(crate) async fn ask_ai_assistant(
    request: vault_core::AiAssistantRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiAssistantResponse, String> {
    let key = state.get_key();
    run_blocking_command("ask_ai_assistant", move || {
        worker_bridge::ask_ai_assistant_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Generates the local MCP and skill integration preview files, off the UI thread.
///
/// Writes preview artifacts to disk, so the filesystem work runs on the blocking pool.
pub(crate) async fn preview_ai_integrations() -> Result<vault_core::AiIntegrationPreview, String> {
    run_blocking_command("preview_ai_integrations", worker_bridge::preview_ai_integrations_impl)
        .await
}

#[cfg(not(test))]
#[tauri::command]
/// Persists (upsert) one assistant conversation and replaces its message transcript, off the UI
/// thread.
///
/// The agent sidecar is a keyless derived plane, so no session key is passed. The save is a full
/// DELETE + re-INSERT of the transcript in SQLite, so it runs on the blocking thread pool.
pub(crate) async fn save_ai_conversation(
    request: vault_core::SaveAgentConversationRequest,
) -> Result<vault_core::AgentConversationSummary, String> {
    run_blocking_command("save_ai_conversation", move || {
        worker_bridge::save_ai_conversation_impl(request)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Lists persisted conversations newest-first for the chat-history explorer, off the UI thread.
pub(crate) async fn list_ai_conversations(
    request: vault_core::ListAgentConversationsRequest,
) -> Result<vault_core::AgentConversationListResponse, String> {
    run_blocking_command("list_ai_conversations", move || {
        worker_bridge::list_ai_conversations_impl(request)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Loads one persisted conversation plus its full message transcript, off the UI thread.
pub(crate) async fn load_ai_conversation(
    conversation_id: String,
) -> Result<Option<vault_core::AgentConversationDetail>, String> {
    run_blocking_command("load_ai_conversation", move || {
        worker_bridge::load_ai_conversation_impl(conversation_id)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Deletes one persisted conversation (cascading its messages), off the UI thread.
pub(crate) async fn delete_ai_conversation(
    conversation_id: String,
) -> Result<vault_core::DeleteAgentConversationResult, String> {
    run_blocking_command("delete_ai_conversation", move || {
        worker_bridge::delete_ai_conversation_impl(conversation_id)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Renames one persisted conversation, off the UI thread.
pub(crate) async fn rename_ai_conversation(
    request: vault_core::RenameAgentConversationRequest,
) -> Result<Option<vault_core::AgentConversationSummary>, String> {
    run_blocking_command("rename_ai_conversation", move || {
        worker_bridge::rename_ai_conversation_impl(request)
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Reads the W-ENRICH-1 content-fetch consent + status surface for Settings, off the UI thread.
pub(crate) async fn get_content_fetch_settings(
    state: State<'_, SessionState>,
) -> Result<vault_core::ContentFetchSettings, String> {
    let key = state.get_key();
    run_blocking_command("get_content_fetch_settings", move || {
        worker_bridge::content_fetch_settings_impl(key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Persists the content-fetch consent settings (master switch + per-extractor + per-domain).
///
/// Turning the master switch on is the consent gate (hard-default-OFF); enabling it also primes the
/// prioritized working-set enqueue + starts the low-concurrency drain. Runs off the UI thread.
pub(crate) async fn set_content_fetch_settings(
    settings: vault_core::ContentFetchSettings,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    let key = state.get_key();
    run_blocking_command("set_content_fetch_settings", move || {
        worker_bridge::set_content_fetch_settings_impl(settings, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Lists the stored content enrichment for one visit (detail panel), off the UI thread.
///
/// Read-only: it returns whatever has been fetched + stored. It NEVER blocks on the network — an
/// absent enrichment simply yields an empty list and the detail panel falls back to title/URL.
pub(crate) async fn list_visit_enrichment(
    history_id: i64,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::VisitEnrichmentRecord>, String> {
    let key = state.get_key();
    run_blocking_command("list_visit_enrichment", move || {
        worker_bridge::list_visit_enrichment_impl(history_id, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Manual "fetch now" PME trigger for one URL's content enrichment, off the UI thread.
///
/// Honest about consent: returns a `disabled` result without queuing when fetching is off for the URL.
pub(crate) async fn content_fetch_now(
    request: vault_core::ContentFetchNowRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ContentFetchNowResult, String> {
    let key = state.get_key();
    run_blocking_command("content_fetch_now", move || {
        worker_bridge::content_fetch_now_impl(request, key.as_deref())
    })
    .await
}

#[cfg(not(test))]
#[tauri::command]
/// Enqueues the prioritized working set for content fetch (the bulk hook), off the UI thread.
///
/// Returns the number of jobs enqueued (0 when fetching is disabled). Drains run on the worker lane.
pub(crate) async fn enqueue_content_fetch_working_set(
    limit: Option<u32>,
    state: State<'_, SessionState>,
) -> Result<usize, String> {
    let key = state.get_key();
    run_blocking_command("enqueue_content_fetch_working_set", move || {
        worker_bridge::enqueue_content_fetch_working_set_impl(limit, key.as_deref())
    })
    .await
}
