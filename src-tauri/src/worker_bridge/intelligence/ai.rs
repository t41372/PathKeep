//! Worker bridge adapters for optional AI provider, queue, search, and assistant flows.
//!
//! ## Responsibilities
//!
//! - Keep desktop command handlers independent from `vault-worker` AI function names.
//! - Preserve the `Result<_, String>` desktop error envelope.
//! - Forward provider, queue, index, search, and assistant payloads without reshaping them.
//!
//! ## Not responsible for
//!
//! - Running provider network calls directly in the desktop façade.
//! - Owning semantic index schema or queue lifecycle transitions.
//! - Blurring optional AI availability with deterministic Core Intelligence availability.
//!
//! ## Dependencies
//!
//! - `vault_worker` for provider, queue, index, search, and assistant orchestration.
//! - `worker_result` for error normalization.
//!
//! ## Performance notes
//!
//! This layer stays a thin adapter; expensive provider/index work must remain in
//! worker-owned jobs or bounded worker calls.

use vault_core::{
    AgentConversationDetail, AgentConversationListResponse, AgentConversationSummary,
    AiAssistantRequest, AiChatSendRequest, AiChatStreamEvent, AiIndexRequest,
    AiProviderConnectionTestRequest, AiProviderSecretInput, AiSearchRequest,
    DeleteAgentConversationResult, ListAgentConversationsRequest, RenameAgentConversationRequest,
    SaveAgentConversationRequest,
};

use super::super::worker_result;

/// Stores one AI provider API key and returns the refreshed app snapshot.
pub(crate) fn store_ai_provider_api_key_impl(
    input: AiProviderSecretInput,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::store_ai_provider_api_key(&input, session_database_key))
}

/// Clears one AI provider API key and returns the refreshed app snapshot.
pub(crate) fn clear_ai_provider_api_key_impl(
    provider_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::clear_ai_provider_api_key(&provider_id, session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Tests one AI provider connection using the worker's runtime resolution rules.
pub(crate) fn test_ai_provider_connection_impl(
    request: AiProviderConnectionTestRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiProviderConnectionTestReport, String> {
    worker_result(vault_worker::test_ai_provider_connection_report(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the AI queue read model.
pub(crate) fn load_ai_queue_status_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_result(vault_worker::load_ai_queue(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
/// Drains AI queue jobs immediately.
pub(crate) fn run_ai_queue_jobs_impl(
    max_jobs: Option<u32>,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_result(vault_worker::run_ai_queue_jobs(session_database_key, max_jobs))
}

#[cfg_attr(test, allow(dead_code))]
/// Requeues a single AI job.
pub(crate) fn replay_ai_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_result(vault_worker::replay_ai_job(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Cancels a single AI job.
pub(crate) fn cancel_ai_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_result(vault_worker::cancel_ai_job(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the persisted assistant result for a queue-backed AI job.
pub(crate) fn load_ai_assistant_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_result(vault_worker::load_ai_assistant_job(session_database_key, job_id))
}

/// Builds or refreshes the semantic index right away.
pub(crate) fn build_ai_index_impl(
    request: AiIndexRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiIndexReport, String> {
    worker_result(vault_worker::build_ai_index_now(session_database_key, &request))
}

/// Runs semantic-plus-lexical search through the worker layer.
pub(crate) fn search_ai_history_impl(
    request: AiSearchRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiSearchResponse, String> {
    worker_result(vault_worker::search_ai_history(session_database_key, &request))
}

/// Answers one question with first-party assistant tooling and citations.
pub(crate) fn ask_ai_assistant_impl(
    request: AiAssistantRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_result(vault_worker::ask_ai_assistant(session_database_key, &request))
}

/// Previews the generated MCP and skill integration artifacts.
pub(crate) fn preview_ai_integrations_impl() -> Result<vault_core::AiIntegrationPreview, String> {
    worker_result(vault_worker::preview_ai_integration_files())
}

#[cfg_attr(test, allow(dead_code))]
/// Starts a streaming chat run; chunks reach the UI through the `emit` sink.
///
/// `emit` wraps `AppHandle::emit("pathkeep://ai-stream", ...)`; it must be `Send + Sync +
/// 'static` so the worker's background streaming thread can own it.
pub(crate) fn ai_chat_send_impl<E>(
    request: AiChatSendRequest,
    session_database_key: Option<&str>,
    emit: E,
) -> Result<vault_core::AiChatSendAck, String>
where
    E: Fn(AiChatStreamEvent) + Send + Sync + 'static,
{
    worker_result(vault_worker::ai_chat_send(session_database_key, &request, emit))
}

#[cfg_attr(test, allow(dead_code))]
/// Requests cooperative cancellation of one live streaming chat run.
pub(crate) fn ai_chat_cancel_impl(
    run_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiChatCancelResult, String> {
    worker_result(vault_worker::ai_chat_cancel(session_database_key, &run_id))
}

/// Persists (upsert) one assistant conversation and replaces its message transcript.
///
/// The agent plane is a keyless derived sidecar, so no session key is threaded here.
///
/// `cfg_attr(test, allow(dead_code))`: the only callers are the `#[cfg(not(test))]` Tauri command
/// and the dev-IPC dispatch, so this is genuinely unused in the lib's test build (same pattern as
/// the chat-stream impls above).
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn save_ai_conversation_impl(
    request: SaveAgentConversationRequest,
) -> Result<AgentConversationSummary, String> {
    worker_result(vault_worker::save_ai_conversation(&request))
}

/// Lists persisted conversations newest-first (bounded list of summaries).
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn list_ai_conversations_impl(
    request: ListAgentConversationsRequest,
) -> Result<AgentConversationListResponse, String> {
    worker_result(vault_worker::list_ai_conversations(&request))
}

/// Loads one persisted conversation plus its full message transcript.
///
/// Returns `Ok(None)` for an unknown id so the front end can show a clear "not found" instead of
/// surfacing an error envelope for an ordinary deleted-elsewhere race.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_ai_conversation_impl(
    conversation_id: String,
) -> Result<Option<AgentConversationDetail>, String> {
    worker_result(vault_worker::load_ai_conversation(&conversation_id))
}

/// Deletes one persisted conversation (cascading its messages).
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn delete_ai_conversation_impl(
    conversation_id: String,
) -> Result<DeleteAgentConversationResult, String> {
    worker_result(vault_worker::delete_ai_conversation(&conversation_id))
}

/// Renames one persisted conversation; returns `Ok(None)` when the id is unknown.
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn rename_ai_conversation_impl(
    request: RenameAgentConversationRequest,
) -> Result<Option<AgentConversationSummary>, String> {
    worker_result(vault_worker::rename_ai_conversation(&request))
}
