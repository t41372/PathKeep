use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderConnectionTestRequest, AiProviderSecretInput,
    AiSearchRequest, ExplainInsightRequest, RunInsightsRequest,
};

use super::worker_result;

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn clear_derived_intelligence_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, String> {
    worker_result(vault_worker::clear_derived_intelligence(session_database_key))
}

pub(crate) fn store_ai_provider_api_key_impl(
    input: AiProviderSecretInput,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::store_ai_provider_api_key(&input, session_database_key))
}

pub(crate) fn clear_ai_provider_api_key_impl(
    provider_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot, String> {
    worker_result(vault_worker::clear_ai_provider_api_key(&provider_id, session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn test_ai_provider_connection_impl(
    request: AiProviderConnectionTestRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiProviderConnectionTestReport, String> {
    worker_result(vault_worker::test_ai_provider_connection_report(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_ai_queue_status_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_result(vault_worker::load_ai_queue(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn run_ai_queue_jobs_impl(
    max_jobs: Option<u32>,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueStatus, String> {
    worker_result(vault_worker::run_ai_queue_jobs(session_database_key, max_jobs))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn replay_ai_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_result(vault_worker::replay_ai_job(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn cancel_ai_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiQueueJob, String> {
    worker_result(vault_worker::cancel_ai_job(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_ai_assistant_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_result(vault_worker::load_ai_assistant_job(session_database_key, job_id))
}

pub(crate) fn build_ai_index_impl(
    request: AiIndexRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiIndexReport, String> {
    worker_result(vault_worker::build_ai_index_now(session_database_key, &request))
}

pub(crate) fn search_ai_history_impl(
    request: AiSearchRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiSearchResponse, String> {
    worker_result(vault_worker::search_ai_history(session_database_key, &request))
}

pub(crate) fn ask_ai_assistant_impl(
    request: AiAssistantRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::AiAssistantResponse, String> {
    worker_result(vault_worker::ask_ai_assistant(session_database_key, &request))
}

pub(crate) fn preview_ai_integrations_impl() -> Result<vault_core::AiIntegrationPreview, String> {
    worker_result(vault_worker::preview_ai_integration_files())
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn run_insights_now_impl(
    request: RunInsightsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::RunInsightsReport, String> {
    worker_result(vault_worker::run_insights_now(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_insights_impl(
    request: RunInsightsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::InsightSnapshot, String> {
    worker_result(vault_worker::load_insights_snapshot(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_thread_detail_impl(
    thread_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::InsightThreadDetail, String> {
    worker_result(vault_worker::load_insight_thread(session_database_key, &thread_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn explain_insight_impl(
    request: ExplainInsightRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::InsightExplanation, String> {
    worker_result(vault_worker::explain_insight_now(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn load_intelligence_runtime_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::load_intelligence_runtime_snapshot(session_database_key))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn retry_intelligence_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::retry_intelligence_job_now(session_database_key, job_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn cancel_intelligence_job_impl(
    job_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceRuntimeSnapshot, String> {
    worker_result(vault_worker::cancel_intelligence_job_now(session_database_key, job_id))
}
