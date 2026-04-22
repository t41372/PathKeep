//! `/intelligence` staged overview read models.
//!
//! ## Responsibilities
//! - Build the first-band and deferred overview payloads from one shared
//!   intelligence SQLite connection.
//! - Reuse a single runtime snapshot so section timing and empty-state metadata
//!   stay consistent across one response.
//! - Keep the overview composition logic out of the core rebuild module.
//!
//! ## Not responsible for
//! - Rebuilding derived intelligence tables.
//! - Entity explanation payloads or export-specific formatting.
//! - The underlying query helpers for trails, refind pages, or summary cards.
//!
//! ## Dependencies
//! - `summary` and `intelligence_domain` for digest-oriented read models.
//! - `phase_three` and `phase_four` for deeper deterministic surfaces.
//! - Parent-module search/refind helpers that still own those SQL queries.
//!
//! ## Performance notes
//! - Both overview entrypoints intentionally reuse one connection and one
//!   runtime snapshot to avoid repeated SQLite open costs during first paint.
//! - Section timing is measured around each fetch so the frontend can keep
//!   honest staged-loading metadata without reopening data sources.

use super::{
    ensure_core_intelligence_schema, get_query_families_with_connection,
    get_search_engine_ranking_with_connection, get_top_search_concepts_with_connection,
    intelligence_refind::{get_refind_pages_with_connection, get_top_sites_with_connection},
    phase_four, phase_three,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    intelligence_runtime::load_intelligence_runtime_from_connection,
    intelligence_sections::build_core_intelligence_section_meta_with_runtime,
    models::{
        AppConfig, CoreIntelligencePrimaryOverview, CoreIntelligenceSecondaryOverview,
        CoreIntelligenceSectionResult, CoreIntelligenceSectionTiming,
        CoreIntelligenceSectionWindow, GranularityDateRangeRequest, IntelligenceRuntimeSnapshot,
        PagedDateRangeRequest, PathFlowRequest, ProfileScopedRequest, RefindPagesRequest,
        ScopedDateRangeRequest, SearchEffectivenessRequest, TopSearchConceptsRequest,
        TopSitesRequest,
    },
};
use anyhow::Result;
use chrono::Local;
use rusqlite::Connection;
use std::time::Instant;

/// Loads the first-band `/intelligence` payload that has to be ready for the
/// initial route paint.
///
/// The response deliberately shares one intelligence connection and one runtime
/// snapshot so the shell does not pay repeated open/close costs while staging
/// the top row of cards.
pub fn get_intelligence_primary_overview(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligencePrimaryOverview> {
    let mut connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let runtime = load_intelligence_runtime_from_connection(&mut connection, paths, config)?;
    build_intelligence_primary_overview_with_connection(&connection, &runtime, request)
}

fn build_intelligence_primary_overview_with_connection(
    connection: &Connection,
    runtime: &IntelligenceRuntimeSnapshot,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligencePrimaryOverview> {
    let overview_started_at = Instant::now();
    let top_sites_request = TopSitesRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        sort_by: Some("visit_count".to_string()),
        limit: Some(40),
    };
    let query_family_request = PagedDateRangeRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        page: 0,
        page_size: 10,
    };
    let top_search_concepts_request = TopSearchConceptsRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        limit: Some(50),
    };
    let refind_pages_request = RefindPagesRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        limit: Some(5),
    };
    let discovery_trend_day_request = GranularityDateRangeRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        granularity: "day".to_string(),
    };
    let interrupted_habits_request =
        ProfileScopedRequest { profile_id: request.profile_id.clone() };

    let mut timings = Vec::new();
    let (digest_summary, timing) = build_overview_timed_section_result(
        "digest-summary",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || super::intelligence_summary::get_digest_summary_with_connection(connection, request),
        |data| {
            data.total_visits.value == 0
                && data.total_searches.value == 0
                && data.new_domains.value == 0
                && data.deep_read_pages.value == 0
                && data.refind_pages.value == 0
        },
    )?;
    timings.push(timing);
    let (on_this_day, timing) = build_overview_timed_section_result(
        "on-this-day",
        CoreIntelligenceSectionWindow::CalendarDayHistory {
            reference_date: Local::now().format("%Y-%m-%d").to_string(),
        },
        runtime,
        || {
            super::intelligence_domain::get_on_this_day_with_connection(
                connection,
                request.profile_id.as_deref(),
            )
        },
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (top_sites, timing) = build_overview_timed_section_result(
        "top-sites",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || get_top_sites_with_connection(connection, &top_sites_request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (refind_pages, timing) = build_overview_timed_section_result(
        "refind-pages",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || get_refind_pages_with_connection(connection, &refind_pages_request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (search_engine_ranking, timing) = build_overview_timed_section_result(
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || get_search_engine_ranking_with_connection(connection, request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (top_search_concepts, timing) = build_overview_timed_section_result(
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || get_top_search_concepts_with_connection(connection, &top_search_concepts_request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (query_families, timing) = build_overview_timed_section_result(
        "search-activity",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || get_query_families_with_connection(connection, &query_family_request),
        |data| data.families.is_empty(),
    )?;
    timings.push(timing);
    let (activity_mix, timing) = build_overview_timed_section_result(
        "activity-mix",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || super::intelligence_summary::get_activity_mix_with_connection(connection, request),
        |data| data.categories.is_empty(),
    )?;
    timings.push(timing);
    let (discovery_trend_day, timing) = build_overview_timed_section_result(
        "browsing-rhythm",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            super::intelligence_domain::get_discovery_trend_with_connection(
                connection,
                &discovery_trend_day_request,
            )
        },
        |data| data.points.is_empty(),
    )?;
    timings.push(timing);
    let (habit_patterns, timing) = build_overview_timed_section_result(
        "habits",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || phase_three::get_habit_patterns_with_connection(connection, request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (interrupted_habits, timing) = build_overview_timed_section_result(
        "habits",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            phase_three::get_interrupted_habits_with_connection(
                connection,
                &interrupted_habits_request,
            )
        },
        |data| data.is_empty(),
    )?;
    timings.push(timing);

    Ok(CoreIntelligencePrimaryOverview {
        digest_summary,
        on_this_day,
        top_sites,
        refind_pages,
        search_engine_ranking,
        top_search_concepts,
        query_families,
        activity_mix,
        discovery_trend_day,
        habit_patterns,
        interrupted_habits,
        timings,
        total_duration_ms: overview_started_at.elapsed().as_millis() as u64,
    })
}

/// Loads the deferred `/intelligence` payload that can arrive after the first
/// band has already painted.
///
/// This keeps the heavier secondary surfaces off the initial critical path
/// while still reusing one runtime snapshot for honest section metadata.
pub fn get_intelligence_secondary_overview(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSecondaryOverview> {
    let mut connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let runtime = load_intelligence_runtime_from_connection(&mut connection, paths, config)?;
    build_intelligence_secondary_overview_with_connection(
        paths,
        config,
        key,
        &connection,
        &runtime,
        request,
    )
}

fn build_intelligence_secondary_overview_with_connection(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    connection: &Connection,
    runtime: &IntelligenceRuntimeSnapshot,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSecondaryOverview> {
    let overview_started_at = Instant::now();
    let search_effectiveness_request = SearchEffectivenessRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        engine: None,
    };
    let discovery_trend_week_request = GranularityDateRangeRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        granularity: "week".to_string(),
    };
    let path_flow_request = PathFlowRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        step_count: 3,
        limit: Some(15),
    };

    let mut timings = Vec::new();
    let (stable_sources, timing) = build_overview_timed_section_result(
        "stable-sources",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || super::intelligence_summary::get_stable_sources_with_connection(connection, request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (search_effectiveness, timing) = build_overview_timed_section_result(
        "search-effectiveness",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            super::intelligence_summary::get_search_effectiveness_with_connection(
                connection,
                &search_effectiveness_request,
            )
        },
        |data| {
            data.engine_stats.is_empty()
                && data.top_resolving_sources.is_empty()
                && data.hardest_topics.is_empty()
        },
    )?;
    timings.push(timing);
    let (friction_signals, timing) = build_overview_timed_section_result(
        "friction-signals",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || super::intelligence_summary::get_friction_signals_with_connection(connection, request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (reopened_investigations, timing) = build_overview_timed_section_result(
        "reopened-investigations",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            super::intelligence_summary::get_reopened_investigations_with_connection(
                connection, request,
            )
        },
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (discovery_trend_week, timing) = build_overview_timed_section_result(
        "discovery-trend",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            super::intelligence_domain::get_discovery_trend_with_connection(
                connection,
                &discovery_trend_week_request,
            )
        },
        |data| data.points.is_empty(),
    )?;
    timings.push(timing);
    let (breadth_index, timing) = build_overview_timed_section_result(
        "breadth-index",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || phase_three::get_breadth_index_with_connection(connection, request),
        |_| false,
    )?;
    timings.push(timing);
    let (path_flows, timing) = build_overview_timed_section_result(
        "path-flows",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || phase_three::get_path_flows_with_connection(connection, &path_flow_request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (compare_sets, timing) = build_overview_timed_section_result(
        "compare-sets",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || phase_four::get_compare_sets_with_connection(connection, request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (multi_browser_diff, timing) = build_overview_timed_section_result(
        "multi-browser-diff",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || phase_four::get_multi_browser_diff_with_connection(connection, request),
        |data| data.profiles.is_empty() && data.category_distributions.is_empty(),
    )?;
    timings.push(timing);
    let (observed_interactions, timing) = build_overview_timed_section_result(
        "observed-interactions",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            phase_three::get_observed_interactions_with_connection(
                paths, config, key, connection, request,
            )
        },
        |data| data.is_empty(),
    )?;
    timings.push(timing);

    Ok(CoreIntelligenceSecondaryOverview {
        stable_sources,
        search_effectiveness,
        friction_signals,
        reopened_investigations,
        discovery_trend_week,
        breadth_index,
        path_flows,
        compare_sets,
        multi_browser_diff,
        observed_interactions,
        timings,
        total_duration_ms: overview_started_at.elapsed().as_millis() as u64,
    })
}

fn build_overview_timed_section_result<R>(
    section_id: &str,
    window: CoreIntelligenceSectionWindow,
    runtime: &IntelligenceRuntimeSnapshot,
    fetch: impl FnOnce() -> Result<R>,
    is_empty: impl FnOnce(&R) -> bool,
) -> Result<(CoreIntelligenceSectionResult<R>, CoreIntelligenceSectionTiming)> {
    let started_at = Instant::now();
    let data = fetch()?;
    let duration_ms = started_at.elapsed().as_millis() as u64;
    let meta = build_core_intelligence_section_meta_with_runtime(
        section_id,
        window,
        is_empty(&data),
        runtime,
    )?;
    Ok((
        CoreIntelligenceSectionResult { data, meta },
        CoreIntelligenceSectionTiming { section_id: section_id.to_string(), duration_ms },
    ))
}
