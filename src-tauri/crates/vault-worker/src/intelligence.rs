//! Intelligence, queue, and derived-state worker flows.
//!
//! ## Responsibilities
//! - expose the worker-facing AI queue, deterministic rebuild, and read-model helpers
//! - keep shared worker counters and section-meta helpers in one place
//! - re-export focused owner modules so `vault-worker` stays a thin facade
//!
//! ## Not responsible for
//! - canonical archive schema or deterministic rebuild algorithms
//! - Tauri command naming and desktop IPC payload design
//! - platform adapters, keyring plumbing, or archive ingest orchestration
//!
//! ## Dependencies
//! - `crate::context` for unlocked config access
//! - `vault_core::intelligence` for deterministic read-model and rebuild logic
//! - child modules `ai_queue` and `runtime` for heavy worker orchestration
//!
//! ## Performance notes
//! - section helpers only read enough runtime metadata to label surfaces honestly
//! - background worker counts stay in shared atomics so the worker never fans out
//!   unbounded concurrency on a 4-core host

mod ai_queue;
mod runtime;

use crate::context::load_unlocked_config;
use anyhow::Result;
use chrono::Local;
use std::sync::atomic::AtomicUsize;
use vault_core::{
    ActivityMix, ActivityMixTrend, AppConfig, BreadthIndex, BrowserDiff,
    CategoryFilteredDateRangeRequest, CompareSet, CompareSetDetail, CompareSetDetailRequest,
    CoreIntelligencePrimaryOverview, CoreIntelligenceSecondaryOverview,
    CoreIntelligenceSectionResult, CoreIntelligenceSectionWindow, DayInsights, DayInsightsRequest,
    DigestSummary, DiscoveryTrend, DomainDeepDive, DomainDeepDiveRequest, DomainTrend,
    DomainTrendRequest, EngineRanking, EntityExplanationRequest, Explanation, FrictionSignal,
    GranularityDateRangeRequest, HabitPattern, HubPage, IntelligenceEmbedCardPayload,
    IntelligenceEmbedCardsRequest, IntelligenceLocalHostBuildResult, IntelligenceLocalHostPreview,
    IntelligenceLocalHostRequest, IntelligencePublicSnapshot, IntelligenceWidgetSnapshot,
    InterruptedHabit, NavigationPath, ObservedInteraction, OnThisDayEntry, PagedDateRangeRequest,
    PathFlow, PathFlowRequest, ProfileScopedRequest, QueryFamilyDetail, QueryFamilyDetailRequest,
    QueryFamilyResult, RefindExplanation, RefindPage, RefindPageDetail, RefindPageDetailRequest,
    RefindPagesRequest, ReopenedInvestigation, RhythmHeatmap, ScopedDateRangeRequest,
    SearchConcept, SearchEffectiveness, SearchEffectivenessRequest, SearchEngineRule,
    SearchEngineRuleInput, SearchQueryListRequest, SearchQueryListResult, SearchTrailQueryRequest,
    SessionDetail, SessionListResult, StableSource, TopSearchConceptsRequest, TopSite,
    TopSitesRequest, TrailDetail, TrailListResult, build_core_intelligence_section_meta,
    intelligence,
};

pub(crate) use self::ai_queue::maybe_spawn_ai_queue_drain;
pub use self::ai_queue::{
    ask_ai_assistant, build_ai_index_now, cancel_ai_job, load_ai_assistant_job, load_ai_queue,
    preview_ai_integration_files, replay_ai_job, run_ai_queue_jobs, search_ai_history,
    test_ai_provider_connection_report,
};
pub(crate) use self::runtime::maybe_spawn_intelligence_queue_drain;
pub use self::runtime::{
    cancel_intelligence_job_now, load_intelligence_runtime_snapshot,
    queue_core_intelligence_rebuild, retry_intelligence_job_now, run_core_intelligence_now,
};

static AI_QUEUE_ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
static INTELLIGENCE_PRIORITY_WORKERS: AtomicUsize = AtomicUsize::new(0);
static INTELLIGENCE_ENRICHMENT_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// Opens the unlocked project context required by deterministic intelligence reads.
///
/// The worker uses this helper so every read-model wrapper resolves paths and config
/// the same way, without each function reimplementing archive bootstrap logic.
fn with_core_intelligence<R>(
    _session_database_key: Option<&str>,
    f: impl FnOnce(&vault_core::ProjectPaths, &AppConfig) -> Result<R>,
) -> Result<R> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    f(&paths, &config)
}

/// Wraps one Core Intelligence section payload with the persisted runtime metadata.
///
/// The UI needs the section payload and its freshness/empty-state context together.
/// Centralizing that composition here prevents every worker wrapper from drifting on
/// `section_id`, window semantics, or what qualifies as an empty dataset.
fn with_core_intelligence_section<R>(
    session_database_key: Option<&str>,
    section_id: &str,
    window: CoreIntelligenceSectionWindow,
    fetch: impl FnOnce(&vault_core::ProjectPaths, &AppConfig) -> Result<R>,
    is_empty: impl FnOnce(&R) -> bool,
) -> Result<CoreIntelligenceSectionResult<R>> {
    with_core_intelligence(session_database_key, |paths, config| {
        let data = fetch(paths, config)?;
        let meta = build_core_intelligence_section_meta(
            paths,
            config,
            session_database_key,
            section_id,
            window,
            is_empty(&data),
        )?;
        Ok(CoreIntelligenceSectionResult { data, meta })
    })
}

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
        |data| data.related_trails.is_empty(),
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
        |data| {
            data.explanation.visit_ids.is_empty()
                && data.related_trails.is_empty()
                && data.recent_days.is_empty()
        },
    )
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
