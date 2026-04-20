use vault_core::{
    CategoryFilteredDateRangeRequest, CompareSetDetailRequest, DayInsightsRequest,
    DomainDeepDiveRequest, DomainTrendRequest, EntityExplanationRequest, ExplainRefindRequest,
    GranularityDateRangeRequest, PagedDateRangeRequest, PathFlowRequest, ProfileScopedRequest,
    QueryFamilyDetailRequest, RefindPageDetailRequest, RefindPagesRequest, ScopedDateRangeRequest,
    SearchEffectivenessRequest, SearchQueryListRequest, SearchTrailQueryRequest,
    TopSearchConceptsRequest, TopSitesRequest,
};

use super::super::worker_result;

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
pub(crate) fn get_search_queries_impl(
    request: SearchQueryListRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::SearchQueryListResult>, String> {
    worker_result(vault_worker::get_search_queries(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_query_families_impl(
    request: PagedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::QueryFamilyResult>, String> {
    worker_result(vault_worker::get_query_families(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_query_family_detail_impl(
    request: QueryFamilyDetailRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::QueryFamilyDetail>, String> {
    worker_result(vault_worker::get_query_family_detail(session_database_key, &request))
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
pub(crate) fn get_refind_page_detail_impl(
    request: RefindPageDetailRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::RefindPageDetail>, String> {
    worker_result(vault_worker::get_refind_page_detail(session_database_key, &request))
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
pub(crate) fn get_day_insights_impl(
    request: DayInsightsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DayInsights>, String> {
    worker_result(vault_worker::get_day_insights(session_database_key, &request))
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
pub(crate) fn get_compare_set_detail_impl(
    request: CompareSetDetailRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::CompareSetDetail>, String> {
    worker_result(vault_worker::get_compare_set_detail(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn get_multi_browser_diff_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::BrowserDiff>, String> {
    worker_result(vault_worker::get_multi_browser_diff(session_database_key, &request))
}
