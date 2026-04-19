//! Worker-bridge helpers for optional AI and deterministic intelligence.

use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderConnectionTestRequest, AiProviderSecretInput,
    AiSearchRequest, CategoryFilteredDateRangeRequest, CoreIntelligenceRebuildRequest,
    DomainDeepDiveRequest, DomainTrendRequest, EntityExplanationRequest, ExplainRefindRequest,
    GranularityDateRangeRequest, IntelligenceEmbedCardsRequest, IntelligenceLocalHostRequest,
    PagedDateRangeRequest, PathFlowRequest, ProfileScopedRequest, RefindPagesRequest,
    ScopedDateRangeRequest, SearchEffectivenessRequest, SearchTrailQueryRequest,
    TopSearchConceptsRequest, TopSitesRequest,
};

use super::worker_result;

#[cfg_attr(test, allow(dead_code))]
/// Clears rebuildable intelligence state while leaving canonical archive facts untouched.
pub(crate) fn clear_derived_intelligence_impl(
    session_database_key: Option<&str>,
) -> Result<vault_core::ClearDerivedIntelligenceReport, String> {
    worker_result(vault_worker::clear_derived_intelligence(session_database_key))
}

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
/// Loads one paginated sessions list.
pub(crate) fn get_sessions_impl(
    request: PagedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::SessionListResult, String> {
    worker_result(vault_worker::get_sessions(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads one session detail read model.
pub(crate) fn get_session_detail_impl(
    session_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::SessionDetail, String> {
    worker_result(vault_worker::get_session_detail(session_database_key, &session_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads one paginated search-trail list.
pub(crate) fn get_search_trails_impl(
    request: SearchTrailQueryRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::TrailListResult, String> {
    worker_result(vault_worker::get_search_trails(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_trail_detail_impl(
    trail_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::TrailDetail, String> {
    worker_result(vault_worker::get_trail_detail(session_database_key, &trail_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_navigation_path_impl(
    visit_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::NavigationPath, String> {
    worker_result(vault_worker::get_navigation_path(session_database_key, visit_id))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_hub_pages_impl(
    request: TopSitesRequest,
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::HubPage>, String> {
    worker_result(vault_worker::get_hub_pages(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_search_engine_ranking_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::EngineRanking>>, String> {
    worker_result(vault_worker::get_search_engine_ranking(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_top_search_concepts_impl(
    request: TopSearchConceptsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::SearchConcept>>, String> {
    worker_result(vault_worker::get_top_search_concepts(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_query_families_impl(
    request: PagedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::QueryFamilyResult>, String> {
    worker_result(vault_worker::get_query_families(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_top_sites_impl(
    request: TopSitesRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::TopSite>>, String> {
    worker_result(vault_worker::get_top_sites(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_domain_trend_impl(
    request: DomainTrendRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::DomainTrend, String> {
    worker_result(vault_worker::get_domain_trend(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_refind_pages_impl(
    request: RefindPagesRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::RefindPage>>, String> {
    worker_result(vault_worker::get_refind_pages(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn explain_refind_impl(
    request: ExplainRefindRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::RefindExplanation, String> {
    worker_result(vault_worker::explain_refind(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn explain_entity_impl(
    request: EntityExplanationRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::Explanation, String> {
    worker_result(vault_worker::explain_entity(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_activity_mix_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::ActivityMix>, String> {
    worker_result(vault_worker::get_activity_mix(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_activity_mix_trend_impl(
    request: GranularityDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::ActivityMixTrend, String> {
    worker_result(vault_worker::get_activity_mix_trend(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_digest_summary_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DigestSummary>, String> {
    worker_result(vault_worker::get_digest_summary(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_intelligence_primary_overview_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligencePrimaryOverview, String> {
    worker_result(vault_worker::get_intelligence_primary_overview(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_intelligence_secondary_overview_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSecondaryOverview, String> {
    worker_result(vault_worker::get_intelligence_secondary_overview(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_stable_sources_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::StableSource>>, String> {
    worker_result(vault_worker::get_stable_sources(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_search_effectiveness_impl(
    request: SearchEffectivenessRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::SearchEffectiveness>, String> {
    worker_result(vault_worker::get_search_effectiveness(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_friction_signals_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::FrictionSignal>>, String> {
    worker_result(vault_worker::get_friction_signals(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_reopened_investigations_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::ReopenedInvestigation>>, String>
{
    worker_result(vault_worker::get_reopened_investigations(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_domain_deep_dive_impl(
    request: DomainDeepDiveRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DomainDeepDive>, String> {
    worker_result(vault_worker::get_domain_deep_dive(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_browsing_rhythm_impl(
    request: CategoryFilteredDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::RhythmHeatmap>, String> {
    worker_result(vault_worker::get_browsing_rhythm(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_discovery_trend_impl(
    request: GranularityDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DiscoveryTrend>, String> {
    worker_result(vault_worker::get_discovery_trend(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_intelligence_embed_cards_impl(
    request: IntelligenceEmbedCardsRequest,
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::IntelligenceEmbedCardPayload>, String> {
    worker_result(vault_worker::get_intelligence_embed_cards(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_intelligence_widget_snapshot_impl(
    request: IntelligenceEmbedCardsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceWidgetSnapshot, String> {
    worker_result(vault_worker::get_intelligence_widget_snapshot(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_intelligence_public_snapshot_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligencePublicSnapshot, String> {
    worker_result(vault_worker::get_intelligence_public_snapshot(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn preview_intelligence_local_host_impl(
    request: IntelligenceLocalHostRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceLocalHostPreview, String> {
    worker_result(vault_worker::preview_intelligence_local_host(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn build_intelligence_local_host_impl(
    request: IntelligenceLocalHostRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::IntelligenceLocalHostBuildResult, String> {
    worker_result(vault_worker::build_intelligence_local_host(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_on_this_day_impl(
    profile_id: Option<String>,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::OnThisDayEntry>>, String> {
    worker_result(vault_worker::get_on_this_day(session_database_key, profile_id.as_deref()))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_breadth_index_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::BreadthIndex>, String> {
    worker_result(vault_worker::get_breadth_index(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_habit_patterns_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::HabitPattern>>, String> {
    worker_result(vault_worker::get_habit_patterns(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_interrupted_habits_impl(
    request: ProfileScopedRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::InterruptedHabit>>, String> {
    worker_result(vault_worker::get_interrupted_habits(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_path_flows_impl(
    request: PathFlowRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::PathFlow>>, String> {
    worker_result(vault_worker::get_path_flows(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_observed_interactions_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::ObservedInteraction>>, String>
{
    worker_result(vault_worker::get_observed_interactions(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_compare_sets_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::CompareSet>>, String> {
    worker_result(vault_worker::get_compare_sets(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_multi_browser_diff_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::BrowserDiff>, String> {
    worker_result(vault_worker::get_multi_browser_diff(session_database_key, &request))
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
