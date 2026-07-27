//! Worker bridge adapters for Core Intelligence read models.
//!
//! ## Responsibilities
//!
//! - Keep desktop command handlers independent from `vault-worker` call details.
//! - Convert worker errors into the existing `Result<_, String>` desktop envelope.
//! - Preserve request and response payload shapes for route-level intelligence reads.
//!
//! ## Not responsible for
//!
//! - Computing intelligence read models or touching SQLite directly.
//! - Deciding Tauri command names.
//! - Adding pagination or filtering outside the typed request contracts.
//!
//! ## Dependencies
//!
//! - `vault_worker` for all orchestration and query execution.
//! - `worker_result` for the desktop string-error boundary.
//!
//! ## Performance notes
//!
//! This layer is a pass-through adapter. Any large-data protection must stay in
//! the worker/core query implementations; adapters here must not clone or cache
//! large result sets.

use crate::command_error::CommandError;
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
) -> Result<vault_core::SessionListResult, CommandError> {
    worker_result(vault_worker::get_sessions(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads one session detail read model.
pub(crate) fn get_session_detail_impl(
    session_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::SessionDetail, CommandError> {
    worker_result(vault_worker::get_session_detail(session_database_key, &session_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads one paginated search-trail list.
pub(crate) fn get_search_trails_impl(
    request: SearchTrailQueryRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::TrailListResult, CommandError> {
    worker_result(vault_worker::get_search_trails(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the evidence trail and grouped visits for one search trail id.
pub(crate) fn get_trail_detail_impl(
    trail_id: String,
    session_database_key: Option<&str>,
) -> Result<vault_core::TrailDetail, CommandError> {
    worker_result(vault_worker::get_trail_detail(session_database_key, &trail_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the typed navigation context anchored at one canonical visit.
pub(crate) fn get_navigation_path_impl(
    visit_id: i64,
    session_database_key: Option<&str>,
) -> Result<vault_core::NavigationPath, CommandError> {
    worker_result(vault_worker::get_navigation_path(session_database_key, visit_id))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads high-traffic hub pages for the requested scope.
pub(crate) fn get_hub_pages_impl(
    request: TopSitesRequest,
    session_database_key: Option<&str>,
) -> Result<Vec<vault_core::HubPage>, CommandError> {
    worker_result(vault_worker::get_hub_pages(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads search-engine ranking metrics for a bounded date/profile scope.
pub(crate) fn get_search_engine_ranking_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::EngineRanking>>, CommandError>
{
    worker_result(vault_worker::get_search_engine_ranking(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads top search concepts after taxonomy-level navigational-noise filtering.
pub(crate) fn get_top_search_concepts_impl(
    request: TopSearchConceptsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::SearchConcept>>, CommandError>
{
    worker_result(vault_worker::get_top_search_concepts(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads normalized search-query rows for the requested list scope.
pub(crate) fn get_search_queries_impl(
    request: SearchQueryListRequest,
    session_database_key: Option<&str>,
) -> Result<
    vault_core::CoreIntelligenceSectionResult<vault_core::SearchQueryListResult>,
    CommandError,
> {
    worker_result(vault_worker::get_search_queries(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads query-family summaries with pagination handled by the request object.
pub(crate) fn get_query_families_impl(
    request: PagedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::QueryFamilyResult>, CommandError>
{
    worker_result(vault_worker::get_query_families(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads one query-family detail page using the shared family identity.
pub(crate) fn get_query_family_detail_impl(
    request: QueryFamilyDetailRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::QueryFamilyDetail>, CommandError>
{
    worker_result(vault_worker::get_query_family_detail(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads top-site rollups for the requested scope.
pub(crate) fn get_top_sites_impl(
    request: TopSitesRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::TopSite>>, CommandError> {
    worker_result(vault_worker::get_top_sites(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads a domain trend series for route and card drilldowns.
pub(crate) fn get_domain_trend_impl(
    request: DomainTrendRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::DomainTrend, CommandError> {
    worker_result(vault_worker::get_domain_trend(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads refind candidates for the requested scope.
pub(crate) fn get_refind_pages_impl(
    request: RefindPagesRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::RefindPage>>, CommandError> {
    worker_result(vault_worker::get_refind_pages(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the evidence-backed detail payload for one refind candidate.
pub(crate) fn get_refind_page_detail_impl(
    request: RefindPageDetailRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::RefindPageDetail>, CommandError> {
    worker_result(vault_worker::get_refind_page_detail(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Explains why a page is considered a refind candidate.
pub(crate) fn explain_refind_impl(
    request: ExplainRefindRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::RefindExplanation, CommandError> {
    worker_result(vault_worker::explain_refind(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Explains one shared intelligence entity using deterministic evidence only.
pub(crate) fn explain_entity_impl(
    request: EntityExplanationRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::Explanation, CommandError> {
    worker_result(vault_worker::explain_entity(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads category activity mix for the requested scope.
pub(crate) fn get_activity_mix_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::ActivityMix>, CommandError> {
    worker_result(vault_worker::get_activity_mix(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads activity-mix trend buckets at the requested granularity.
pub(crate) fn get_activity_mix_trend_impl(
    request: GranularityDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::ActivityMixTrend, CommandError> {
    worker_result(vault_worker::get_activity_mix_trend(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the deterministic digest summary for the current intelligence overview.
pub(crate) fn get_digest_summary_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DigestSummary>, CommandError> {
    worker_result(vault_worker::get_digest_summary(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads stable source domains that repeatedly appear in the selected scope.
pub(crate) fn get_stable_sources_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::StableSource>>, CommandError>
{
    worker_result(vault_worker::get_stable_sources(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads search effectiveness counters and ratios for a bounded scope.
pub(crate) fn get_search_effectiveness_impl(
    request: SearchEffectivenessRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::SearchEffectiveness>, CommandError>
{
    worker_result(vault_worker::get_search_effectiveness(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads friction signals inferred from deterministic browsing evidence.
pub(crate) fn get_friction_signals_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::FrictionSignal>>, CommandError>
{
    worker_result(vault_worker::get_friction_signals(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads reopened investigation candidates without running any assistant model.
pub(crate) fn get_reopened_investigations_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<
    vault_core::CoreIntelligenceSectionResult<Vec<vault_core::ReopenedInvestigation>>,
    CommandError,
> {
    worker_result(vault_worker::get_reopened_investigations(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the domain deep-dive read model for a canonical domain target.
pub(crate) fn get_domain_deep_dive_impl(
    request: DomainDeepDiveRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DomainDeepDive>, CommandError> {
    worker_result(vault_worker::get_domain_deep_dive(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads exact-day insights for the shared day entity route.
pub(crate) fn get_day_insights_impl(
    request: DayInsightsRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DayInsights>, CommandError> {
    worker_result(vault_worker::get_day_insights(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads calendar rhythm buckets, optionally filtered by activity category.
pub(crate) fn get_browsing_rhythm_impl(
    request: CategoryFilteredDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::RhythmHeatmap>, CommandError> {
    worker_result(vault_worker::get_browsing_rhythm(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads discovery trend buckets for Dashboard and Intelligence surfaces.
pub(crate) fn get_discovery_trend_impl(
    request: GranularityDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DiscoveryTrend>, CommandError> {
    worker_result(vault_worker::get_discovery_trend(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads historical same-day entries for the optional profile scope.
pub(crate) fn get_on_this_day_impl(
    profile_id: Option<String>,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::OnThisDayEntry>>, CommandError>
{
    worker_result(vault_worker::get_on_this_day(session_database_key, profile_id.as_deref()))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads the breadth index for the selected date/profile scope.
pub(crate) fn get_breadth_index_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::BreadthIndex>, CommandError> {
    worker_result(vault_worker::get_breadth_index(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads recurring habit patterns from deterministic visit rollups.
pub(crate) fn get_habit_patterns_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::HabitPattern>>, CommandError>
{
    worker_result(vault_worker::get_habit_patterns(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads interrupted habit candidates for a profile-scoped request.
pub(crate) fn get_interrupted_habits_impl(
    request: ProfileScopedRequest,
    session_database_key: Option<&str>,
) -> Result<
    vault_core::CoreIntelligenceSectionResult<Vec<vault_core::InterruptedHabit>>,
    CommandError,
> {
    worker_result(vault_worker::get_interrupted_habits(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads typed path-flow sequences with stable flow identities.
pub(crate) fn get_path_flows_impl(
    request: PathFlowRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::PathFlow>>, CommandError> {
    worker_result(vault_worker::get_path_flows(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads observed interaction signals for the selected scope.
pub(crate) fn get_observed_interactions_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<
    vault_core::CoreIntelligenceSectionResult<Vec<vault_core::ObservedInteraction>>,
    CommandError,
> {
    worker_result(vault_worker::get_observed_interactions(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads compare-set summaries for route promotion and overview cards.
pub(crate) fn get_compare_sets_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::CompareSet>>, CommandError> {
    worker_result(vault_worker::get_compare_sets(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads one compare-set detail using the shared compare-set id.
pub(crate) fn get_compare_set_detail_impl(
    request: CompareSetDetailRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::CompareSetDetail>, CommandError> {
    worker_result(vault_worker::get_compare_set_detail(session_database_key, &request))
}

#[cfg_attr(test, allow(dead_code))]
/// Loads multi-browser divergence signals for the selected scope.
pub(crate) fn get_multi_browser_diff_impl(
    request: ScopedDateRangeRequest,
    session_database_key: Option<&str>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::BrowserDiff>, CommandError> {
    worker_result(vault_worker::get_multi_browser_diff(session_database_key, &request))
}
