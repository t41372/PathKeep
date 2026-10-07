//! Failure modes: a wrapper drops cursor fields, opens legacy storage or resolves a different Vault.
//! Worker passthroughs for route-level Core Intelligence reads.
//!
//! ## Responsibilities
//! - expose thin worker wrappers for route-first entities and top-level read models
//! - keep section ids / date-window metadata attached where the frontend expects them
//! - preserve the existing worker export surface while keeping `intelligence.rs` small
//!
//! ## Not responsible for
//! - AI queue execution or deterministic runtime queue control
//! - canonical query logic, SQL, or read-model assembly owned by `vault-core`
//! - Tauri command naming or frontend route grammar decisions
//!
//! ## Dependencies
//! - parent `with_core_intelligence` helpers for unlocked archive access and section metadata
//! - `vault_core::intelligence` for the actual read-model implementations
//!
//! ## Performance notes
//! - every wrapper is intentionally thin and defers heavy work to `vault-core`, so the
//!   worker layer does not introduce extra materialization on large archives

use super::*;

/// Loads one paginated sessions list.
pub fn get_sessions(
    vault: &vault_core::Vault,
    request: &PagedDateRangeRequest,
) -> Result<SessionListResult> {
    with_core_intelligence(vault, |vault| intelligence::get_sessions(vault, request))
}

/// Loads the detail read model for one browsing session.
pub fn get_session_detail(
    vault: &vault_core::Vault,
    request: &vault_core::SessionDetailRequest,
) -> Result<SessionDetail> {
    with_core_intelligence(vault, |vault| intelligence::get_session_detail(vault, request))
}

/// Loads one paginated search trail list.
pub fn get_search_trails(
    vault: &vault_core::Vault,
    request: &SearchTrailQueryRequest,
) -> Result<TrailListResult> {
    with_core_intelligence(vault, |vault| intelligence::get_search_trails(vault, request))
}

/// Loads the detail read model for one search trail.
pub fn get_trail_detail(
    vault: &vault_core::Vault,
    request: &vault_core::TrailDetailRequest,
) -> Result<TrailDetail> {
    with_core_intelligence(vault, |vault| intelligence::get_trail_detail(vault, request))
}

/// Loads the navigation path centered on one canonical visit id.
pub fn get_navigation_path(vault: &vault_core::Vault, visit_id: i64) -> Result<NavigationPath> {
    with_core_intelligence(vault, |vault| intelligence::get_navigation_path(vault, visit_id))
}

/// Loads the hub-page list used by Dashboard and Intelligence summaries.
pub fn get_hub_pages(
    vault: &vault_core::Vault,
    request: &TopSitesRequest,
) -> Result<vault_core::IntelligencePage<HubPage>> {
    with_core_intelligence(vault, |vault| intelligence::get_hub_pages(vault, request))
}

/// Loads the section-wrapped search-engine ranking surface.
pub fn get_search_engine_ranking(
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<EngineRanking>>> {
    with_core_intelligence_section(
        vault,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_search_engine_ranking(vault, request),
        |data| data.is_empty(),
    )
}

/// Lists the Settings-owned search-engine override rules.
pub fn list_search_engine_rules(vault: &vault_core::Vault) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(vault, |vault| {
        intelligence::list_search_engine_rules_for_settings(vault)
    })
}

/// Upserts one Settings-owned search-engine override rule and returns the new rule set.
pub fn upsert_search_engine_rule(
    vault: &vault_core::Vault,
    input: &SearchEngineRuleInput,
) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(vault, |vault| {
        intelligence::upsert_search_engine_rule_for_settings(vault, input)
    })
}

/// Deletes one Settings-owned search-engine override rule and returns the remaining rules.
pub fn delete_search_engine_rule(
    vault: &vault_core::Vault,
    rule_id: &str,
) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(vault, |vault| {
        intelligence::delete_search_engine_rule_for_settings(vault, rule_id)
    })
}

/// Loads the primary overview payload that seeds the `/intelligence` route shell.
pub fn get_intelligence_primary_overview(
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligencePrimaryOverview> {
    with_core_intelligence(vault, |vault| {
        intelligence::get_intelligence_primary_overview(vault, request)
    })
}

/// Loads the ranked top-search-concepts section payload.
pub fn get_top_search_concepts(
    vault: &vault_core::Vault,
    request: &TopSearchConceptsRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<SearchConcept>>> {
    with_core_intelligence_section(
        vault,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_top_search_concepts(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the most frequent searches in a date range, with counts scoped to that range.
pub fn get_frequent_searches(
    vault: &vault_core::Vault,
    request: &FrequentSearchesRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<FrequentSearch>>> {
    with_core_intelligence_section(
        vault,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_frequent_searches(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the paginated search-query table used by the search activity surface.
pub fn get_search_queries(
    vault: &vault_core::Vault,
    request: &SearchQueryListRequest,
) -> Result<CoreIntelligenceSectionResult<SearchQueryListResult>> {
    with_core_intelligence_section(
        vault,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_search_queries(vault, request),
        |data| data.rows.is_empty(),
    )
}

/// Loads the paginated query-family list used by the search activity surface.
pub fn get_query_families(
    vault: &vault_core::Vault,
    request: &PagedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<QueryFamilyResult>> {
    with_core_intelligence_section(
        vault,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_query_families(vault, request),
        |data| data.families.is_empty(),
    )
}

/// Loads one query-family detail payload and its freshness metadata.
pub fn get_query_family_detail(
    vault: &vault_core::Vault,
    request: &QueryFamilyDetailRequest,
) -> Result<CoreIntelligenceSectionResult<QueryFamilyDetail>> {
    with_core_intelligence_section(
        vault,
        "query-family-detail",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_query_family_detail(vault, request),
        query_family_detail_is_empty,
    )
}

/// Loads the section-wrapped top-sites list.
pub fn get_top_sites(
    vault: &vault_core::Vault,
    request: &TopSitesRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<TopSite>>> {
    with_core_intelligence_section(
        vault,
        "top-sites",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_top_sites(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the day-bucket trend for one registrable domain.
pub fn get_domain_trend(
    vault: &vault_core::Vault,
    request: &DomainTrendRequest,
) -> Result<DomainTrend> {
    with_core_intelligence(vault, |vault| intelligence::get_domain_trend(vault, request))
}

/// Loads the section-wrapped refind pages list.
pub fn get_refind_pages(
    vault: &vault_core::Vault,
    request: &RefindPagesRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<RefindPage>>> {
    with_core_intelligence_section(
        vault,
        "refind-pages",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_refind_pages(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads one refind-page detail payload and its freshness metadata.
pub fn get_refind_page_detail(
    vault: &vault_core::Vault,
    request: &RefindPageDetailRequest,
) -> Result<CoreIntelligenceSectionResult<RefindPageDetail>> {
    with_core_intelligence_section(
        vault,
        "refind-page-detail",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |vault| intelligence::get_refind_page_detail(vault, request),
        refind_page_detail_is_empty,
    )
}

fn query_family_detail_is_empty(data: &QueryFamilyDetail) -> bool {
    data.related_trails.is_empty()
}

fn refind_page_detail_is_empty(data: &RefindPageDetail) -> bool {
    data.explanation.visit_ids.is_empty()
        && data.related_trails.is_empty()
        && data.recent_days.is_empty()
}

/// Explains why one canonical refind page qualifies as refind-worthy.
pub fn explain_refind(
    vault: &vault_core::Vault,
    request: &vault_core::ExplainRefindRequest,
) -> Result<RefindExplanation> {
    with_core_intelligence(vault, |vault| intelligence::explain_refind(vault, request))
}

/// Explains one deterministic Core Intelligence entity with evidence-backed metadata.
pub fn explain_entity(
    vault: &vault_core::Vault,
    request: &EntityExplanationRequest,
) -> Result<Explanation> {
    with_core_intelligence(vault, |vault| intelligence::explain_entity(vault, request))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_empty_predicates_cover_related_entity_edges() {
        assert!(query_family_detail_is_empty(&QueryFamilyDetail::default()));
        let mut query_detail = QueryFamilyDetail::default();
        query_detail.related_trails.items.push(vault_core::TrailSummary::default());
        assert!(!query_family_detail_is_empty(&query_detail));

        assert!(refind_page_detail_is_empty(&RefindPageDetail::default()));
        let mut refind_detail = RefindPageDetail::default();
        refind_detail.explanation.visit_ids.push(42);
        assert!(!refind_page_detail_is_empty(&refind_detail));
    }
}
