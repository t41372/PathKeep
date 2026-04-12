//! Tauri commands for optional AI and deterministic intelligence flows.

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
/// Executes a deterministic insights rebuild now instead of waiting for background work.
pub(crate) fn run_insights_now(
    request: vault_core::RunInsightsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RunInsightsReport, String> {
    worker_bridge::run_insights_now_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Queues one deterministic rebuild so it can run through the background jobs surface.
#[allow(dead_code)]
pub(crate) fn queue_insights_rebuild(
    request: vault_core::RunInsightsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::DeterministicRebuildQueueReport, String> {
    worker_bridge::queue_insights_rebuild_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the latest deterministic insights snapshot for the requested scope.
pub(crate) fn load_insights(
    request: vault_core::RunInsightsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::InsightSnapshot, String> {
    worker_bridge::load_insights_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads one deterministic insight thread in detail.
pub(crate) fn load_thread_detail(
    thread_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::InsightThreadDetail, String> {
    worker_bridge::load_thread_detail_impl(thread_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Produces a human-readable explanation for one deterministic insight.
pub(crate) fn explain_insight(
    request: vault_core::ExplainInsightRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::InsightExplanation, String> {
    worker_bridge::explain_insight_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Returns queue/runtime state for deterministic intelligence and enrichment work.
pub(crate) fn load_intelligence_runtime(
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_bridge::load_intelligence_runtime_impl(state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Retries one deterministic intelligence job immediately.
pub(crate) fn retry_intelligence_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_bridge::retry_intelligence_job_impl(job_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Cancels one deterministic intelligence job.
pub(crate) fn cancel_intelligence_job(
    job_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_bridge::cancel_intelligence_job_impl(job_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Generates the local MCP and skill integration preview files.
pub(crate) fn preview_ai_integrations() -> Result<vault_core::AiIntegrationPreview, String> {
    worker_bridge::preview_ai_integrations_impl()
}
