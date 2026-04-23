//! Worker passthroughs for section-style Core Intelligence surfaces.
//!
//! ## Responsibilities
//! - expose thin worker wrappers for section cards, compare sets, habits, and export payloads
//! - attach the stable section metadata envelope expected by the frontend
//! - keep lower-priority surface wrappers out of the worker parent façade
//!
//! ## Not responsible for
//! - AI queue execution or deterministic runtime queue control
//! - query implementation details, SQL, or derived-state rebuilds owned by `vault-core`
//! - desktop command naming or frontend route shell decisions
//!
//! ## Dependencies
//! - parent `with_core_intelligence` helpers for unlocked archive access and section metadata
//! - `vault_core::intelligence` for the real deterministic reads
//!
//! ## Performance notes
//! - these wrappers stay allocation-light and delegate all heavy work to `vault-core`
//! - section emptiness checks are kept local so the UI can render honest empty-state metadata

use super::*;

/// Loads the section-wrapped activity-mix payload.
pub fn get_activity_mix(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<ActivityMix>> {
    with_core_intelligence_section(
        session_database_key,
        "activity-mix",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_activity_mix(paths, config, session_database_key, request)
        },
        |data| data.categories.is_empty(),
    )
}

/// Loads the trend companion for the activity-mix chart.
pub fn get_activity_mix_trend(
    session_database_key: Option<&str>,
    request: &GranularityDateRangeRequest,
) -> Result<ActivityMixTrend> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_activity_mix_trend(paths, config, session_database_key, request)
    })
}

/// Loads the section-wrapped digest summary payload.
pub fn get_digest_summary(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<DigestSummary>> {
    with_core_intelligence_section(
        session_database_key,
        "digest-summary",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_digest_summary(paths, config, session_database_key, request)
        },
        |data| {
            data.total_visits.value == 0
                && data.total_searches.value == 0
                && data.new_domains.value == 0
                && data.deep_read_pages.value == 0
                && data.refind_pages.value == 0
        },
    )
}

/// Loads the section-wrapped stable-sources payload.
pub fn get_stable_sources(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<StableSource>>> {
    with_core_intelligence_section(
        session_database_key,
        "stable-sources",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_stable_sources(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped search-effectiveness payload.
pub fn get_search_effectiveness(
    session_database_key: Option<&str>,
    request: &SearchEffectivenessRequest,
) -> Result<CoreIntelligenceSectionResult<SearchEffectiveness>> {
    with_core_intelligence_section(
        session_database_key,
        "search-effectiveness",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_search_effectiveness(paths, config, session_database_key, request)
        },
        |data| {
            data.engine_stats.is_empty()
                && data.top_resolving_sources.is_empty()
                && data.hardest_topics.is_empty()
        },
    )
}

/// Loads the section-wrapped friction-signals payload.
pub fn get_friction_signals(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<FrictionSignal>>> {
    with_core_intelligence_section(
        session_database_key,
        "friction-signals",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_friction_signals(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped reopened-investigations payload.
pub fn get_reopened_investigations(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<ReopenedInvestigation>>> {
    with_core_intelligence_section(
        session_database_key,
        "reopened-investigations",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_reopened_investigations(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped domain deep-dive payload.
pub fn get_domain_deep_dive(
    session_database_key: Option<&str>,
    request: &DomainDeepDiveRequest,
) -> Result<CoreIntelligenceSectionResult<DomainDeepDive>> {
    with_core_intelligence_section(
        session_database_key,
        "domain-deep-dive",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_domain_deep_dive(paths, config, session_database_key, request)
        },
        |data| {
            data.total_visits == 0
                && data.active_days == 0
                && data.trail_count == 0
                && data.top_pages.is_empty()
                && data.top_referrers.is_empty()
                && data.top_exits.is_empty()
                && data.visit_trend.is_empty()
        },
    )
}

/// Loads the section-wrapped single-day insights payload.
pub fn get_day_insights(
    session_database_key: Option<&str>,
    request: &DayInsightsRequest,
) -> Result<CoreIntelligenceSectionResult<DayInsights>> {
    with_core_intelligence_section(
        session_database_key,
        "day-insights",
        CoreIntelligenceSectionWindow::DateRange {
            date_range: vault_core::DateRange {
                start: request.date.clone(),
                end: request.date.clone(),
            },
        },
        |paths, config| {
            intelligence::get_day_insights(paths, config, session_database_key, request)
        },
        |data| {
            data.digest_summary.total_visits.value == 0
                && data.digest_summary.total_searches.value == 0
                && data.digest_summary.new_domains.value == 0
                && data.digest_summary.deep_read_pages.value == 0
                && data.digest_summary.refind_pages.value == 0
                && data.top_sites.is_empty()
                && data.query_families.families.is_empty()
                && data.refind_pages.is_empty()
        },
    )
}

/// Loads the section-wrapped calendar heatmap payload.
pub fn get_browsing_rhythm(
    session_database_key: Option<&str>,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<RhythmHeatmap>> {
    with_core_intelligence_section(
        session_database_key,
        "browsing-rhythm",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_browsing_rhythm(paths, config, session_database_key, request)
        },
        |data| data.cells.is_empty(),
    )
}

/// Loads the section-wrapped discovery trend payload.
pub fn get_discovery_trend(
    session_database_key: Option<&str>,
    request: &GranularityDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<DiscoveryTrend>> {
    with_core_intelligence_section(
        session_database_key,
        "discovery-trend",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_discovery_trend(paths, config, session_database_key, request)
        },
        |data| data.points.is_empty(),
    )
}

/// Loads the section-wrapped "On This Day" payload for the current local calendar day.
pub fn get_on_this_day(
    session_database_key: Option<&str>,
    profile_id: Option<&str>,
) -> Result<CoreIntelligenceSectionResult<Vec<OnThisDayEntry>>> {
    with_core_intelligence_section(
        session_database_key,
        "on-this-day",
        CoreIntelligenceSectionWindow::CalendarDayHistory {
            reference_date: Local::now().format("%Y-%m-%d").to_string(),
        },
        |paths, config| {
            intelligence::get_on_this_day(paths, config, session_database_key, profile_id)
        },
        |data| data.is_empty(),
    )
}

/// Builds the trusted embed-card payload set from deterministic intelligence surfaces.
pub fn get_intelligence_embed_cards(
    session_database_key: Option<&str>,
    request: &IntelligenceEmbedCardsRequest,
) -> Result<Vec<IntelligenceEmbedCardPayload>> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_intelligence_embed_cards(paths, config, session_database_key, request)
    })
}

/// Builds the Settings-facing widget snapshot payload.
pub fn get_intelligence_widget_snapshot(
    session_database_key: Option<&str>,
    request: &IntelligenceEmbedCardsRequest,
) -> Result<IntelligenceWidgetSnapshot> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_intelligence_widget_snapshot(paths, config, session_database_key, request)
    })
}

/// Builds the redacted public snapshot payload.
pub fn get_intelligence_public_snapshot(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<IntelligencePublicSnapshot> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_intelligence_public_snapshot(paths, config, session_database_key, request)
    })
}

/// Builds the local-host artifact preview for external-output review.
pub fn preview_intelligence_local_host(
    session_database_key: Option<&str>,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostPreview> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::preview_intelligence_local_host(paths, config, session_database_key, request)
    })
}

/// Builds the local-host artifact set after the user confirms the preview.
pub fn build_intelligence_local_host(
    session_database_key: Option<&str>,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostBuildResult> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::build_intelligence_local_host(paths, config, session_database_key, request)
    })
}

/// Loads the section-wrapped breadth-index payload.
pub fn get_breadth_index(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<BreadthIndex>> {
    with_core_intelligence_section(
        session_database_key,
        "breadth-index",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_breadth_index(paths, config, session_database_key, request)
        },
        |_| false,
    )
}

/// Loads the section-wrapped habit-pattern list.
pub fn get_habit_patterns(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<HabitPattern>>> {
    with_core_intelligence_section(
        session_database_key,
        "habits",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_habit_patterns(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped interrupted-habits list.
pub fn get_interrupted_habits(
    session_database_key: Option<&str>,
    request: &ProfileScopedRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<InterruptedHabit>>> {
    with_core_intelligence_section(
        session_database_key,
        "habits",
        CoreIntelligenceSectionWindow::DateRange {
            date_range: vault_core::DateRange { start: "".to_string(), end: "".to_string() },
        },
        |paths, config| {
            intelligence::get_interrupted_habits(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped path-flow payload.
pub fn get_path_flows(
    session_database_key: Option<&str>,
    request: &PathFlowRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<PathFlow>>> {
    with_core_intelligence_section(
        session_database_key,
        "path-flows",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| intelligence::get_path_flows(paths, config, session_database_key, request),
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped observed-interactions payload.
pub fn get_observed_interactions(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<ObservedInteraction>>> {
    with_core_intelligence_section(
        session_database_key,
        "observed-interactions",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_observed_interactions(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped compare-set list.
pub fn get_compare_sets(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<CompareSet>>> {
    with_core_intelligence_section(
        session_database_key,
        "compare-sets",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_compare_sets(paths, config, session_database_key, request)
        },
        |data| data.is_empty(),
    )
}

/// Loads one compare-set detail payload and its freshness metadata.
pub fn get_compare_set_detail(
    session_database_key: Option<&str>,
    request: &CompareSetDetailRequest,
) -> Result<CoreIntelligenceSectionResult<CompareSetDetail>> {
    with_core_intelligence_section(
        session_database_key,
        "compare-set-detail",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_compare_set_detail(paths, config, session_database_key, request)
        },
        |data| data.compare_set.pages.is_empty() && data.recent_days.is_empty(),
    )
}

/// Loads the section-wrapped multi-browser diff payload.
pub fn get_multi_browser_diff(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<BrowserDiff>> {
    with_core_intelligence_section(
        session_database_key,
        "multi-browser-diff",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        |paths, config| {
            intelligence::get_multi_browser_diff(paths, config, session_database_key, request)
        },
        |data| data.profiles.is_empty() && data.category_distributions.is_empty(),
    )
}

/// Loads the secondary overview payload used below the route shell fold.
pub fn get_intelligence_secondary_overview(
    session_database_key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSecondaryOverview> {
    with_core_intelligence(session_database_key, |paths, config| {
        intelligence::get_intelligence_secondary_overview(
            paths,
            config,
            session_database_key,
            request,
        )
    })
}
