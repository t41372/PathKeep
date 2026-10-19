//! Failure modes: section metadata disagrees with native data or a wrapper loses list continuation.
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
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<ActivityMix>> {
    with_core_intelligence_section(
        vault,
        "activity-mix",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_activity_mix(vault, request),
        |data| data.categories.is_empty(),
    )
}

/// Loads the trend companion for the activity-mix chart.
pub fn get_activity_mix_trend(
    vault: &vault_core::Vault,
    request: &GranularityDateRangeRequest,
) -> Result<ActivityMixTrend> {
    with_core_intelligence(vault, |vault| intelligence::get_activity_mix_trend(vault, request))
}

/// Loads the section-wrapped digest summary payload.
pub fn get_digest_summary(
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<DigestSummary>> {
    with_core_intelligence_section(
        vault,
        "digest-summary",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_digest_summary(vault, request),
        digest_summary_is_empty,
    )
}

/// Loads the section-wrapped stable-sources payload.
pub fn get_stable_sources(
    vault: &vault_core::Vault,
    request: &vault_core::ScopedPageRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<StableSource>>> {
    with_core_intelligence_section(
        vault,
        "stable-sources",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_stable_sources(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped search-effectiveness payload.
pub fn get_search_effectiveness(
    vault: &vault_core::Vault,
    request: &SearchEffectivenessRequest,
) -> Result<CoreIntelligenceSectionResult<SearchEffectiveness>> {
    with_core_intelligence_section(
        vault,
        "search-effectiveness",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_search_effectiveness(vault, request),
        search_effectiveness_is_empty,
    )
}

/// Loads the section-wrapped friction-signals payload.
pub fn get_friction_signals(
    vault: &vault_core::Vault,
    request: &vault_core::ScopedPageRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<FrictionSignal>>> {
    with_core_intelligence_section(
        vault,
        "friction-signals",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_friction_signals(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped reopened-investigations payload.
pub fn get_reopened_investigations(
    vault: &vault_core::Vault,
    request: &vault_core::ScopedPageRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<ReopenedInvestigation>>> {
    with_core_intelligence_section(
        vault,
        "reopened-investigations",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_reopened_investigations(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped domain deep-dive payload.
pub fn get_domain_deep_dive(
    vault: &vault_core::Vault,
    request: &DomainDeepDiveRequest,
) -> Result<CoreIntelligenceSectionResult<DomainDeepDive>> {
    with_core_intelligence_section(
        vault,
        "domain-deep-dive",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_domain_deep_dive(vault, request),
        domain_deep_dive_is_empty,
    )
}

/// Loads the section-wrapped single-day insights payload.
pub fn get_day_insights(
    vault: &vault_core::Vault,
    request: &DayInsightsRequest,
) -> Result<CoreIntelligenceSectionResult<DayInsights>> {
    with_core_intelligence_section(
        vault,
        "day-insights",
        CoreIntelligenceSectionWindow::DateRange {
            date_range: vault_core::DateRange {
                start: request.date.clone(),
                end: request.date.clone(),
            },
        },
        request.profile_id.as_deref(),
        |vault| intelligence::get_day_insights(vault, request),
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
    vault: &vault_core::Vault,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<RhythmHeatmap>> {
    with_core_intelligence_section(
        vault,
        "browsing-rhythm",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_browsing_rhythm(vault, request),
        |data| data.cells.is_empty(),
    )
}

/// Loads the section-wrapped discovery trend payload.
pub fn get_discovery_trend(
    vault: &vault_core::Vault,
    request: &GranularityDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<DiscoveryTrend>> {
    with_core_intelligence_section(
        vault,
        "discovery-trend",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_discovery_trend(vault, request),
        |data| data.points.is_empty(),
    )
}

/// Loads the section-wrapped "On This Day" payload for today in Insights' time zone.
pub fn get_on_this_day(
    vault: &vault_core::Vault,
    profile_id: Option<&str>,
) -> Result<CoreIntelligenceSectionResult<Vec<OnThisDayEntry>>> {
    with_core_intelligence_section(
        vault,
        "on-this-day",
        CoreIntelligenceSectionWindow::CalendarDayHistory {
            reference_date: vault_core::insights_today(vault)?,
        },
        profile_id,
        |vault| intelligence::get_on_this_day(vault, profile_id),
        |data| data.is_empty(),
    )
}

/// Builds the trusted embed-card payload set from deterministic intelligence surfaces.
pub fn get_intelligence_embed_cards(
    vault: &vault_core::Vault,
    request: &IntelligenceEmbedCardsRequest,
) -> Result<vault_core::IntelligencePage<IntelligenceEmbedCardPayload>> {
    with_core_intelligence(vault, |vault| {
        intelligence::get_intelligence_embed_cards(vault, request)
    })
}

/// Builds the Settings-facing widget snapshot payload.
pub fn get_intelligence_widget_snapshot(
    vault: &vault_core::Vault,
    request: &IntelligenceEmbedCardsRequest,
) -> Result<IntelligenceWidgetSnapshot> {
    with_core_intelligence(vault, |vault| {
        intelligence::get_intelligence_widget_snapshot(vault, request)
    })
}

/// Builds the redacted public snapshot payload.
pub fn get_intelligence_public_snapshot(
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<IntelligencePublicSnapshot> {
    with_core_intelligence(vault, |vault| {
        intelligence::get_intelligence_public_snapshot(vault, request)
    })
}

/// Builds the local-host artifact preview for external-output review.
pub fn preview_intelligence_local_host(
    vault: &vault_core::Vault,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostPreview> {
    with_core_intelligence(vault, |vault| {
        intelligence::preview_intelligence_local_host(vault, request)
    })
}

/// Builds the local-host artifact set after the user confirms the preview.
pub fn build_intelligence_local_host(
    vault: &vault_core::Vault,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostBuildResult> {
    with_core_intelligence(vault, |vault| {
        intelligence::build_intelligence_local_host(vault, request)
    })
}

/// Loads the section-wrapped breadth-index payload.
pub fn get_breadth_index(
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<BreadthIndex>> {
    with_core_intelligence_section(
        vault,
        "breadth-index",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_breadth_index(vault, request),
        |_| false,
    )
}

/// Loads the section-wrapped habit-pattern list.
pub fn get_habit_patterns(
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<HabitPattern>>> {
    with_core_intelligence_section(
        vault,
        "habits",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_habit_patterns(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped interrupted-habits list.
pub fn get_interrupted_habits(
    vault: &vault_core::Vault,
    request: &ProfileScopedRequest,
) -> Result<CoreIntelligenceSectionResult<Vec<InterruptedHabit>>> {
    with_core_intelligence_section(
        vault,
        "habits",
        // Interrupted habits are read over everything the archive holds.
        CoreIntelligenceSectionWindow::AllTime,
        request.profile_id.as_deref(),
        |vault| intelligence::get_interrupted_habits(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped path-flow payload.
pub fn get_path_flows(
    vault: &vault_core::Vault,
    request: &PathFlowRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<PathFlow>>> {
    with_core_intelligence_section(
        vault,
        "path-flows",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_path_flows(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads the section-wrapped observed-interactions payload.
pub fn get_observed_interactions(
    vault: &vault_core::Vault,
    request: &vault_core::ScopedPageRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<ObservedInteraction>>> {
    with_core_intelligence_section(
        vault,
        "observed-interactions",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_observed_interactions(vault, request),
        |data| data.is_empty(),
    )
}

/// Loads one page of compare sets, newest first, with its freshness metadata.
pub fn get_compare_sets(
    vault: &vault_core::Vault,
    request: &vault_core::ScopedPageRequest,
) -> Result<CoreIntelligenceSectionResult<vault_core::IntelligencePage<CompareSet>>> {
    with_core_intelligence_section(
        vault,
        "compare-sets",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_compare_sets(vault, request),
        |data| data.items.is_empty(),
    )
}

/// Loads one compare-set detail payload and its freshness metadata.
pub fn get_compare_set_detail(
    vault: &vault_core::Vault,
    request: &CompareSetDetailRequest,
) -> Result<CoreIntelligenceSectionResult<CompareSetDetail>> {
    with_core_intelligence_section(
        vault,
        "compare-set-detail",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_compare_set_detail(vault, request),
        compare_set_detail_is_empty,
    )
}

fn digest_summary_is_empty(data: &DigestSummary) -> bool {
    data.total_visits.value == 0
        && data.total_searches.value == 0
        && data.new_domains.value == 0
        && data.deep_read_pages.value == 0
        && data.refind_pages.value == 0
}

fn search_effectiveness_is_empty(data: &SearchEffectiveness) -> bool {
    data.engine_stats.is_empty()
        && data.top_resolving_sources.is_empty()
        && data.hardest_topics.is_empty()
}

fn domain_deep_dive_is_empty(data: &DomainDeepDive) -> bool {
    data.total_visits == 0 && data.top_pages.is_empty() && data.visit_trend.is_empty()
}

fn compare_set_detail_is_empty(data: &CompareSetDetail) -> bool {
    data.compare_set.pages.is_empty() && data.recent_days.is_empty()
}

/// Loads the section-wrapped multi-browser diff payload.
pub fn get_multi_browser_diff(
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSectionResult<BrowserDiff>> {
    with_core_intelligence_section(
        vault,
        "multi-browser-diff",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        request.profile_id.as_deref(),
        |vault| intelligence::get_multi_browser_diff(vault, request),
        |data| data.profiles.is_empty() && data.category_distributions.is_empty(),
    )
}

/// Loads the secondary overview payload used below the route shell fold.
pub fn get_intelligence_secondary_overview(
    vault: &vault_core::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSecondaryOverview> {
    with_core_intelligence(vault, |vault| {
        intelligence::get_intelligence_secondary_overview(vault, request)
    })
}
