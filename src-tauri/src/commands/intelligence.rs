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
/// Executes a Core Intelligence rebuild now instead of waiting for background work.
pub(crate) fn run_core_intelligence_now(
    request: vault_core::CoreIntelligenceRebuildRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceRebuildReport, String> {
    worker_bridge::run_core_intelligence_now_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Queues one Core Intelligence rebuild so it can run through the background jobs surface.
#[allow(dead_code)]
pub(crate) fn queue_core_intelligence_rebuild(
    request: vault_core::CoreIntelligenceRebuildRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceQueueReport, String> {
    worker_bridge::queue_core_intelligence_rebuild_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads one paginated sessions list.
pub(crate) fn get_sessions(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::SessionListResult, String> {
    worker_bridge::get_sessions_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads one session detail read model.
pub(crate) fn get_session_detail(
    session_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::SessionDetail, String> {
    worker_bridge::get_session_detail_impl(session_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads one paginated search-trail list.
pub(crate) fn get_search_trails(
    request: vault_core::SearchTrailQueryRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::TrailListResult, String> {
    worker_bridge::get_search_trails_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_trail_detail(
    trail_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::TrailDetail, String> {
    worker_bridge::get_trail_detail_impl(trail_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_navigation_path(
    visit_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::NavigationPath, String> {
    worker_bridge::get_navigation_path_impl(visit_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_hub_pages(
    request: vault_core::TopSitesRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::HubPage>, String> {
    worker_bridge::get_hub_pages_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_search_engine_ranking(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::EngineRanking>, String> {
    worker_bridge::get_search_engine_ranking_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_top_search_concepts(
    request: vault_core::TopSearchConceptsRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::SearchConcept>, String> {
    worker_bridge::get_top_search_concepts_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_query_families(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::QueryFamilyResult, String> {
    worker_bridge::get_query_families_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_top_sites(
    request: vault_core::TopSitesRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::TopSite>, String> {
    worker_bridge::get_top_sites_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_domain_trend(
    request: vault_core::DomainTrendRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::DomainTrend, String> {
    worker_bridge::get_domain_trend_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_refind_pages(
    request: vault_core::RefindPagesRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::RefindPage>, String> {
    worker_bridge::get_refind_pages_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn explain_refind(
    request: vault_core::ExplainRefindRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RefindExplanation, String> {
    worker_bridge::explain_refind_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_activity_mix(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ActivityMix, String> {
    worker_bridge::get_activity_mix_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_activity_mix_trend(
    request: vault_core::GranularityDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ActivityMixTrend, String> {
    worker_bridge::get_activity_mix_trend_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_digest_summary(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::DigestSummary, String> {
    worker_bridge::get_digest_summary_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_stable_sources(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::StableSource>, String> {
    worker_bridge::get_stable_sources_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_search_effectiveness(
    request: vault_core::SearchEffectivenessRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::SearchEffectiveness, String> {
    worker_bridge::get_search_effectiveness_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_friction_signals(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::FrictionSignal>, String> {
    worker_bridge::get_friction_signals_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_reopened_investigations(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::ReopenedInvestigation>, String> {
    worker_bridge::get_reopened_investigations_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_domain_deep_dive(
    request: vault_core::DomainDeepDiveRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::DomainDeepDive, String> {
    worker_bridge::get_domain_deep_dive_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_browsing_rhythm(
    request: vault_core::CategoryFilteredDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RhythmHeatmap, String> {
    worker_bridge::get_browsing_rhythm_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_discovery_trend(
    request: vault_core::GranularityDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::DiscoveryTrend, String> {
    worker_bridge::get_discovery_trend_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn get_on_this_day(
    profile_id: Option<String>,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::OnThisDayEntry>, String> {
    worker_bridge::get_on_this_day_impl(profile_id, state.get_key().as_deref())
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
