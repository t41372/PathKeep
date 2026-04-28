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
    session_database_key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<SessionListResult> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_sessions(paths, config, session_database_key, request)
    })
}

/// Loads the detail read model for one browsing session.
pub fn get_session_detail(
    session_database_key: Option<&str>,
    session_id: &str,
) -> Result<SessionDetail> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_session_detail(paths, config, session_database_key, session_id)
    })
}

/// Loads one paginated search trail list.
pub fn get_search_trails(
    session_database_key: Option<&str>,
    request: &SearchTrailQueryRequest,
) -> Result<TrailListResult> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_search_trails(paths, config, session_database_key, request)
    })
}

/// Loads the detail read model for one search trail.
pub fn get_trail_detail(session_database_key: Option<&str>, trail_id: &str) -> Result<TrailDetail> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_trail_detail(paths, config, session_database_key, trail_id)
    })
}

/// Loads the navigation path centered on one canonical visit id.
pub fn get_navigation_path(
    session_database_key: Option<&str>,
    visit_id: i64,
) -> Result<NavigationPath> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_navigation_path(paths, config, session_database_key, visit_id)
    })
}

/// Loads the hub-page list used by Dashboard and Intelligence summaries.
pub fn get_hub_pages(
    session_database_key: Option<&str>,
    request: &TopSitesRequest,
) -> Result<Vec<HubPage>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_hub_pages(paths, config, session_database_key, request)
    })
}

/// Loads the section-wrapped search-engine ranking surface.
pub fn get_search_engine_ranking(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<EngineRanking>>> {
    with_core_intelligence_section(
        session_database_key,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_search_engine_ranking(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Lists the Settings-owned search-engine override rules.
pub fn list_search_engine_rules(
    session_database_key: Option<&str>,
) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::list_search_engine_rules_for_settings(paths, config, session_database_key)
    })
}

/// Upserts one Settings-owned search-engine override rule and returns the new rule set.
pub fn upsert_search_engine_rule(
    session_database_key: Option<&str>,
    input: &SearchEngineRuleInput,
) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::upsert_search_engine_rule_for_settings(
            paths,
            config,
            session_database_key,
            input,
        )
    })
}

/// Deletes one Settings-owned search-engine override rule and returns the remaining rules.
pub fn delete_search_engine_rule(
    session_database_key: Option<&str>,
    rule_id: &str,
) -> Result<Vec<SearchEngineRule>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::delete_search_engine_rule_for_settings(
            paths,
            config,
            session_database_key,
            rule_id,
        )
    })
}

/// Loads the primary overview payload that seeds the `/intelligence` route shell.
pub fn get_intelligence_primary_overview(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligencePrimaryOverview> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_intelligence_primary_overview(
            paths,
            config,
            session_database_key,
            request,
        )
    })
}

/// Loads the ranked top-search-concepts section payload.
pub fn get_top_search_concepts(
    session_database_key: Option<&str>,
    request: &TopSearchConceptsRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<SearchConcept>>> {
    with_core_intelligence_section(
        session_database_key,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_top_search_concepts(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads the paginated search-query table used by the search activity surface.
pub fn get_search_queries(
    session_database_key: Option<&str>,
    request: &SearchQueryListRequest,
) -> Result<CoreIntelligenceSectionResult<SearchQueryListResult>> {
    with_core_intelligence_section(
        session_database_key,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_search_queries(paths, config, session_database_key, request)
        },
        |data| data.rows.is_empty(),
    )
}

/// Loads the paginated query-family list used by the search activity surface.
pub fn get_query_families(
    session_database_key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<QueryFamilyResult>> {
    with_core_intelligence_section(
        session_database_key,
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_query_families(paths, config, session_database_key, request)
        },
        |data| data.families.is_empty(),
    )
}

/// Loads one query-family detail payload and its freshness metadata.
pub fn get_query_family_detail(
    session_database_key: Option<&str>,
    request: &QueryFamilyDetailRequest,
) -> Result<CoreIntelligenceSectionResult<QueryFamilyDetail>> {
    with_core_intelligence_section(
        session_database_key,
        "query-family-detail",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_query_family_detail(paths, config, session_database_key, request)
        },
        query_family_detail_is_empty,
    )
}

/// Loads the section-wrapped top-sites list.
pub fn get_top_sites(
    session_database_key: Option<&str>,
    request: &TopSitesRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<TopSite>>> {
    with_core_intelligence_section(
        session_database_key,
        "top-sites",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| intelligence::get_top_sites(paths, config, session_database_key, request),
        |data| data.is_empty(),
    )
}

/// Loads the day-bucket trend for one registrable domain.
pub fn get_domain_trend(
    session_database_key: Option<&str>,
    request: &DomainTrendRequest,
) -> Result<DomainTrend> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_domain_trend(paths, config, session_database_key, request)
    })
}

/// Loads the section-wrapped refind pages list.
pub fn get_refind_pages(
    session_database_key: Option<&str>,
    request: &RefindPagesRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<RefindPage>>> {
    with_core_intelligence_section(
        session_database_key,
        "refind-pages",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_refind_pages(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads one refind-page detail payload and its freshness metadata.
pub fn get_refind_page_detail(
    session_database_key: Option<&str>,
    request: &RefindPageDetailRequest,
) -> Result<CoreIntelligenceSectionResult<RefindPageDetail>> {
    with_core_intelligence_section(
        session_database_key,
        "refind-page-detail",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_refind_page_detail(paths, config, session_database_key, request)
        },
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
    session_database_key: Option<&str>,
    request: &vault_core::ExplainRefindRequest,
) -> Result<RefindExplanation> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::explain_refind(paths, config, session_database_key, request)
    })
}

/// Explains one deterministic Core Intelligence entity with evidence-backed metadata.
pub fn explain_entity(
    session_database_key: Option<&str>,
    request: &EntityExplanationRequest,
) -> Result<Explanation> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::explain_entity(paths, config, session_database_key, request)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detail_empty_predicates_cover_related_entity_edges() {
        assert!(query_family_detail_is_empty(&QueryFamilyDetail::default()));
        let mut query_detail = QueryFamilyDetail::default();
        query_detail.related_trails.push(vault_core::TrailSummary::default());
        assert!(!query_family_detail_is_empty(&query_detail));

        assert!(refind_page_detail_is_empty(&RefindPageDetail::default()));
        let mut refind_detail = RefindPageDetail::default();
        refind_detail.explanation.visit_ids.push(42);
        assert!(!refind_page_detail_is_empty(&refind_detail));
    }
}
