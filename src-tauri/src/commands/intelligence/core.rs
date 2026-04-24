//! Tauri command façade for Core Intelligence read models.
//!
//! ## Responsibilities
//!
//! - Preserve the frontend-facing command names for deterministic intelligence reads.
//! - Pull the current session database key from Tauri state.
//! - Delegate all archive/query behavior to the worker bridge.
//!
//! ## Not responsible for
//!
//! - Opening archive databases or interpreting Core Intelligence schema details.
//! - Building derived-state projections or running rebuild jobs.
//! - Changing payload shape for existing frontend callers.
//!
//! ## Dependencies
//!
//! - `SessionState` for the optional in-memory database key.
//! - `worker_bridge` for the string-error desktop envelope.
//!
//! ## Performance notes
//!
//! Commands in this file must stay as thin read-model façades. Large result
//! sets are expected to be paginated or section-bounded in `vault-core`; this
//! layer must not add full-archive materialization.

#[cfg(not(test))]
use crate::{session::SessionState, worker_bridge};
#[cfg(not(test))]
use tauri::State;

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
/// Loads the evidence trail and grouped visits for one search trail id.
pub(crate) fn get_trail_detail(
    trail_id: String,
    state: State<'_, SessionState>,
) -> Result<vault_core::TrailDetail, String> {
    worker_bridge::get_trail_detail_impl(trail_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the typed navigation context anchored at one canonical visit.
pub(crate) fn get_navigation_path(
    visit_id: i64,
    state: State<'_, SessionState>,
) -> Result<vault_core::NavigationPath, String> {
    worker_bridge::get_navigation_path_impl(visit_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads high-traffic hub pages for the requested scope without exposing raw SQL.
pub(crate) fn get_hub_pages(
    request: vault_core::TopSitesRequest,
    state: State<'_, SessionState>,
) -> Result<Vec<vault_core::HubPage>, String> {
    worker_bridge::get_hub_pages_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads search-engine ranking metrics for a bounded date/profile scope.
pub(crate) fn get_search_engine_ranking(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::EngineRanking>>, String> {
    worker_bridge::get_search_engine_ranking_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads top search concepts after taxonomy-level navigational-noise filtering.
pub(crate) fn get_top_search_concepts(
    request: vault_core::TopSearchConceptsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::SearchConcept>>, String> {
    worker_bridge::get_top_search_concepts_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads normalized search-query rows for the requested list scope.
pub(crate) fn get_search_queries(
    request: vault_core::SearchQueryListRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::SearchQueryListResult>, String> {
    worker_bridge::get_search_queries_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads query-family summaries with pagination handled by the request object.
pub(crate) fn get_query_families(
    request: vault_core::PagedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::QueryFamilyResult>, String> {
    worker_bridge::get_query_families_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads one query-family detail page using the shared family identity.
pub(crate) fn get_query_family_detail(
    request: vault_core::QueryFamilyDetailRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::QueryFamilyDetail>, String> {
    worker_bridge::get_query_family_detail_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads top-site rollups for the requested scope.
pub(crate) fn get_top_sites(
    request: vault_core::TopSitesRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::TopSite>>, String> {
    worker_bridge::get_top_sites_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads a domain trend series for route and card drilldowns.
pub(crate) fn get_domain_trend(
    request: vault_core::DomainTrendRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::DomainTrend, String> {
    worker_bridge::get_domain_trend_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads refind candidates for the requested scope.
pub(crate) fn get_refind_pages(
    request: vault_core::RefindPagesRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::RefindPage>>, String> {
    worker_bridge::get_refind_pages_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the evidence-backed detail payload for one refind candidate.
pub(crate) fn get_refind_page_detail(
    request: vault_core::RefindPageDetailRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::RefindPageDetail>, String> {
    worker_bridge::get_refind_page_detail_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Explains why a page is considered a refind candidate.
pub(crate) fn explain_refind(
    request: vault_core::ExplainRefindRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::RefindExplanation, String> {
    worker_bridge::explain_refind_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Explains one shared intelligence entity using deterministic evidence only.
pub(crate) fn explain_entity(
    request: vault_core::EntityExplanationRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::Explanation, String> {
    worker_bridge::explain_entity_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads category activity mix for the requested scope.
pub(crate) fn get_activity_mix(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::ActivityMix>, String> {
    worker_bridge::get_activity_mix_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads activity-mix trend buckets at the requested granularity.
pub(crate) fn get_activity_mix_trend(
    request: vault_core::GranularityDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::ActivityMixTrend, String> {
    worker_bridge::get_activity_mix_trend_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the deterministic digest summary for the current intelligence overview.
pub(crate) fn get_digest_summary(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DigestSummary>, String> {
    worker_bridge::get_digest_summary_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads stable source domains that repeatedly appear in the selected scope.
pub(crate) fn get_stable_sources(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::StableSource>>, String> {
    worker_bridge::get_stable_sources_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads search effectiveness counters and ratios for a bounded scope.
pub(crate) fn get_search_effectiveness(
    request: vault_core::SearchEffectivenessRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::SearchEffectiveness>, String> {
    worker_bridge::get_search_effectiveness_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads friction signals inferred from deterministic browsing evidence.
pub(crate) fn get_friction_signals(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::FrictionSignal>>, String> {
    worker_bridge::get_friction_signals_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads reopened investigation candidates without running any assistant model.
pub(crate) fn get_reopened_investigations(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::ReopenedInvestigation>>, String>
{
    worker_bridge::get_reopened_investigations_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the domain deep-dive read model for a canonical domain target.
pub(crate) fn get_domain_deep_dive(
    request: vault_core::DomainDeepDiveRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DomainDeepDive>, String> {
    worker_bridge::get_domain_deep_dive_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads exact-day insights for the shared day entity route.
pub(crate) fn get_day_insights(
    request: vault_core::DayInsightsRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DayInsights>, String> {
    worker_bridge::get_day_insights_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads calendar rhythm buckets, optionally filtered by activity category.
pub(crate) fn get_browsing_rhythm(
    request: vault_core::CategoryFilteredDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::RhythmHeatmap>, String> {
    worker_bridge::get_browsing_rhythm_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads discovery trend buckets for Dashboard and Intelligence surfaces.
pub(crate) fn get_discovery_trend(
    request: vault_core::GranularityDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::DiscoveryTrend>, String> {
    worker_bridge::get_discovery_trend_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads historical same-day entries for the optional profile scope.
pub(crate) fn get_on_this_day(
    profile_id: Option<String>,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::OnThisDayEntry>>, String> {
    worker_bridge::get_on_this_day_impl(profile_id, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads the breadth index for the selected date/profile scope.
pub(crate) fn get_breadth_index(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::BreadthIndex>, String> {
    worker_bridge::get_breadth_index_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads recurring habit patterns from deterministic visit rollups.
pub(crate) fn get_habit_patterns(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::HabitPattern>>, String> {
    worker_bridge::get_habit_patterns_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads interrupted habit candidates for a profile-scoped request.
pub(crate) fn get_interrupted_habits(
    request: vault_core::ProfileScopedRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::InterruptedHabit>>, String> {
    worker_bridge::get_interrupted_habits_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads typed path-flow sequences with stable flow identities.
pub(crate) fn get_path_flows(
    request: vault_core::PathFlowRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::PathFlow>>, String> {
    worker_bridge::get_path_flows_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads observed interaction signals for the selected scope.
pub(crate) fn get_observed_interactions(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::ObservedInteraction>>, String>
{
    worker_bridge::get_observed_interactions_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads compare-set summaries for route promotion and overview cards.
pub(crate) fn get_compare_sets(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<Vec<vault_core::CompareSet>>, String> {
    worker_bridge::get_compare_sets_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads one compare-set detail using the shared compare-set id.
pub(crate) fn get_compare_set_detail(
    request: vault_core::CompareSetDetailRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::CompareSetDetail>, String> {
    worker_bridge::get_compare_set_detail_impl(request, state.get_key().as_deref())
}

#[cfg(not(test))]
#[tauri::command]
/// Loads multi-browser divergence signals for the selected scope.
pub(crate) fn get_multi_browser_diff(
    request: vault_core::ScopedDateRangeRequest,
    state: State<'_, SessionState>,
) -> Result<vault_core::CoreIntelligenceSectionResult<vault_core::BrowserDiff>, String> {
    worker_bridge::get_multi_browser_diff_impl(request, state.get_key().as_deref())
}
