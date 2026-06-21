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
/// Stores one AI provider secret and returns the refreshed app snapshot.
pub(crate) fn store_ai_provider_api_key(
    input: vault_core::AiProviderSecretInput,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::store_ai_provider_api_key_impl(input, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Removes one stored AI provider secret and returns the refreshed app snapshot.
pub(crate) fn clear_ai_provider_api_key(
    provider_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_bridge::clear_ai_provider_api_key_impl(provider_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Probes an AI provider connection without launching archive-wide jobs.
pub(crate) fn test_ai_provider_connection(
    request: vault_core::AiProviderConnectionTestRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiProviderConnectionTestReport, String> {
    worker_bridge::test_ai_provider_connection_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the persisted AI queue status read model.
pub(crate) fn load_ai_queue_status(
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_bridge::load_ai_queue_status_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Drains queued AI jobs up to the requested limit.
pub(crate) fn run_ai_queue_jobs(
    max_jobs: Option<u32>,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_bridge::run_ai_queue_jobs_impl(max_jobs, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Requeues one failed or skipped AI job.
pub(crate) fn replay_ai_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_bridge::replay_ai_job_impl(job_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Cancels one queued or running AI job.
pub(crate) fn cancel_ai_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_bridge::cancel_ai_job_impl(job_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the persisted assistant result for one queue-backed job.
pub(crate) fn load_ai_assistant_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_bridge::load_ai_assistant_job_impl(job_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Builds or refreshes the semantic index.
pub(crate) fn build_ai_index(
    request: vault_core::AiIndexRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiIndexReport, String> {
    worker_bridge::build_ai_index_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Runs semantic-plus-lexical search over visible archive history.
pub(crate) fn search_ai_history(
    request: vault_core::AiSearchRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiSearchResponse, String> {
    worker_bridge::search_ai_history_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Asks the first-party assistant to answer a question with archive citations.
pub(crate) fn ask_ai_assistant(
    request: vault_core::AiAssistantRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_bridge::ask_ai_assistant_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Generates the local MCP and skill integration preview files.
pub(crate) fn preview_ai_integrations() -> Result<vault_core::AiIntegrationPreview, String> {
    worker_bridge::preview_ai_integrations_impl()
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
