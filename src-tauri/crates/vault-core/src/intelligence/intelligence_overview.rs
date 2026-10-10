//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
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
    intelligence_overview_snapshot::{OverviewBand, is_all_time_range, resolve_overview_snapshot},
    intelligence_refind::{get_refind_pages_with_connection, get_top_sites_with_connection},
    intelligence_search_metrics::{
        get_search_engine_ranking_with_connection, get_top_search_concepts_with_connection,
    },
    intelligence_search_queries::get_query_families_with_connection,
    phase_four, phase_three,
    site_days::SiteDayCache,
};
use crate::{
    intelligence_runtime::load_intelligence_runtime_from_connection,
    intelligence_sections::build_core_intelligence_section_meta_with_modules,
    models::{
        CoreIntelligencePrimaryOverview, CoreIntelligenceSecondaryOverview,
        CoreIntelligenceSectionResult, CoreIntelligenceSectionTiming,
        CoreIntelligenceSectionWindow, GranularityDateRangeRequest, IntelligenceRuntimeSnapshot,
        PagedDateRangeRequest, PathFlowRequest, RefindPagesRequest, ScopedDateRangeRequest,
        SearchEffectivenessRequest, TopSearchConceptsRequest, TopSitesRequest,
    },
};
use anyhow::Result;
use chrono::Local;
use std::time::Instant;
use vault_store::Session;

/// Loads the first-band `/intelligence` payload that has to be ready for the
/// initial route paint.
///
/// The response deliberately shares one intelligence connection and one runtime
/// snapshot so the shell does not pay repeated open/close costs while staging
/// the top row of cards.
pub fn get_intelligence_primary_overview(
    vault: &crate::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligencePrimaryOverview> {
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    let cache = vault.site_days().clone();
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let paths = &paths;
        let config = &config;
        let request = &request;
        let cache = &cache;

        let runtime = load_intelligence_runtime_from_connection(connection, paths, config)?;
        if is_all_time_range(&request.date_range) {
            // All-time aggregates over the full archive are the heaviest scope, so
            // serve a persisted snapshot whenever the archive fingerprint is stable
            // and only recompute when the data actually changed.
            let mut payload: CoreIntelligencePrimaryOverview =
                resolve_overview_snapshot(connection, OverviewBand::Primary, request, || {
                    build_intelligence_primary_overview_with_connection(
                        connection, cache, &runtime, request,
                    )
                })?;
            refresh_for_today(connection, &runtime, request, &mut payload)?;
            for meta in [
                &mut payload.digest_summary.meta,
                &mut payload.on_this_day.meta,
                &mut payload.top_sites.meta,
                &mut payload.refind_pages.meta,
                &mut payload.search_engine_ranking.meta,
                &mut payload.top_search_concepts.meta,
                &mut payload.query_families.meta,
                &mut payload.activity_mix.meta,
                &mut payload.discovery_trend_day.meta,
                &mut payload.habit_patterns.meta,
                &mut payload.interrupted_habits.meta,
            ] {
                refresh_module_meta(meta, &runtime)?;
            }
            return Ok(payload);
        }
        build_intelligence_primary_overview_with_connection(connection, cache, &runtime, request)
    })
}

fn build_intelligence_primary_overview_with_connection(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    runtime: &IntelligenceRuntimeSnapshot,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligencePrimaryOverview> {
    let overview_started_at = Instant::now();
    let top_sites_request = TopSitesRequest {
        cursor: None,
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        profile_ids: Vec::new(),
        sort_by: Some("visit_count".to_string()),
        limit: Some(40),
    };
    let query_family_request = PagedDateRangeRequest {
        cursor: None,
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),

        page_size: 10,
    };
    let top_search_concepts_request = TopSearchConceptsRequest {
        cursor: None,
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        limit: Some(50),
    };
    // The card scrolls; twenty fill it beside the searches column (`useRefindPages`).
    let refind_pages_request = RefindPagesRequest {
        cursor: None,
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        limit: Some(20),
    };
    let discovery_trend_day_request = GranularityDateRangeRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        granularity: "day".to_string(),
    };

    let mut timings = Vec::new();
    let (digest_summary, timing) = build_overview_timed_section_result(
        "digest-summary",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            super::intelligence_summary::get_digest_summary_with_connection(
                connection, cache, request,
            )
        },
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
        || get_top_sites_with_connection(connection, cache, &top_sites_request),
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
        || phase_three::get_habit_patterns_with_connection(connection, cache, request),
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    // The scope's habits that went quiet, from the habits just read.
    let (interrupted_habits, timing) = build_overview_timed_section_result(
        "habits",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            Ok(phase_three::interrupted_among(
                &habit_patterns.data,
                Local::now().timestamp_millis(),
            ))
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
    vault: &crate::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSecondaryOverview> {
    let observed = phase_three::get_observed_interactions(
        vault,
        &crate::models::ScopedPageRequest::from(request),
    )?;
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    let cache = vault.site_days().clone();
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let paths = &paths;
        let config = &config;
        let request = &request;
        let cache = &cache;

        let runtime = load_intelligence_runtime_from_connection(connection, paths, config)?;
        if is_all_time_range(&request.date_range) {
            let mut payload: CoreIntelligenceSecondaryOverview =
                resolve_overview_snapshot(connection, OverviewBand::Secondary, request, || {
                    build_intelligence_secondary_overview_with_connection(
                        connection, cache, &observed, &runtime, request,
                    )
                })?;
            // The snapshot outlives the day it was built on; report this request's dates.
            for meta in [
                &mut payload.stable_sources.meta,
                &mut payload.search_effectiveness.meta,
                &mut payload.friction_signals.meta,
                &mut payload.reopened_investigations.meta,
                &mut payload.discovery_trend_week.meta,
                &mut payload.breadth_index.meta,
                &mut payload.path_flows.meta,
                &mut payload.compare_sets.meta,
                &mut payload.multi_browser_diff.meta,
                &mut payload.observed_interactions.meta,
            ] {
                if let CoreIntelligenceSectionWindow::DateRange { date_range } = &mut meta.window {
                    *date_range = request.date_range.clone();
                }
                refresh_module_meta(meta, &runtime)?;
            }
            payload.observed_interactions.data = observed.clone();
            payload.observed_interactions.meta = build_core_intelligence_section_meta_with_modules(
                "observed-interactions",
                payload.observed_interactions.meta.window.clone(),
                observed.items.is_empty() && observed.next_cursor.is_none(),
                &runtime.modules,
            )?;
            return Ok(payload);
        }
        build_intelligence_secondary_overview_with_connection(
            connection, cache, &observed, &runtime, request,
        )
    })
}

fn build_intelligence_secondary_overview_with_connection(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    observed: &crate::models::IntelligencePage<crate::models::ObservedInteraction>,
    runtime: &IntelligenceRuntimeSnapshot,
    request: &ScopedDateRangeRequest,
) -> Result<CoreIntelligenceSecondaryOverview> {
    let overview_started_at = Instant::now();
    let search_effectiveness_request = SearchEffectivenessRequest {
        topics_cursor: None,
        topics_limit: None,
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
        cursor: None,
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
        || {
            super::intelligence_summary::get_stable_sources_with_connection(
                connection,
                &crate::models::ScopedPageRequest::from(request),
            )
        },
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
        search_effectiveness_is_empty,
    )?;
    timings.push(timing);
    let (friction_signals, timing) = build_overview_timed_section_result(
        "friction-signals",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            super::intelligence_summary::get_friction_signals_with_connection(
                connection,
                &crate::models::ScopedPageRequest::from(request),
            )
        },
        |data| data.is_empty(),
    )?;
    timings.push(timing);
    let (reopened_investigations, timing) = build_overview_timed_section_result(
        "reopened-investigations",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || {
            super::intelligence_summary::get_reopened_investigations_with_connection(
                connection,
                &crate::models::ScopedPageRequest::from(request),
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
        || phase_three::get_breadth_index_with_connection(connection, cache, request),
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
        || {
            super::compare_sets::get_compare_sets_with_connection(
                connection,
                &crate::models::ScopedPageRequest::from(request),
            )
        },
        |data| data.items.is_empty(),
    )?;
    timings.push(timing);
    let (multi_browser_diff, timing) = build_overview_timed_section_result(
        "multi-browser-diff",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || phase_four::get_multi_browser_diff_with_connection(connection, cache, request),
        |data| data.profiles.is_empty() && data.category_distributions.is_empty(),
    )?;
    timings.push(timing);
    let (observed_interactions, timing) = build_overview_timed_section_result(
        "observed-interactions",
        CoreIntelligenceSectionWindow::DateRange { date_range: request.date_range.clone() },
        runtime,
        || Ok(observed.clone()),
        |data| data.items.is_empty() && data.next_cursor.is_none(),
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

fn refresh_module_meta(
    meta: &mut crate::models::CoreIntelligenceSectionMeta,
    runtime: &IntelligenceRuntimeSnapshot,
) -> Result<()> {
    if !meta.module_ids.is_empty() {
        *meta = build_core_intelligence_section_meta_with_modules(
            &meta.section_id,
            meta.window.clone(),
            false,
            &runtime.modules,
        )?;
    }
    Ok(())
}

/// Brings a primary overview served from the all-time snapshot up to today.
/// The snapshot outlives the day it was built on (its fingerprint leaves the
/// date out), so the parts that depend on today are redone here, each a small
/// read: On this day, and whether each habit is interrupted by now. Every
/// date-range section reports the dates of this request.
fn refresh_for_today(
    connection: &Session<'_>,
    runtime: &IntelligenceRuntimeSnapshot,
    request: &ScopedDateRangeRequest,
    payload: &mut CoreIntelligencePrimaryOverview,
) -> Result<()> {
    let today = Local::now();
    let on_this_day = super::intelligence_domain::get_on_this_day_with_connection(
        connection,
        request.profile_id.as_deref(),
    )?;
    payload.on_this_day.meta = build_core_intelligence_section_meta_with_modules(
        "on-this-day",
        CoreIntelligenceSectionWindow::CalendarDayHistory {
            reference_date: today.format("%Y-%m-%d").to_string(),
        },
        on_this_day.is_empty(),
        &runtime.modules,
    )?;
    payload.on_this_day.data = on_this_day;
    let interrupted =
        phase_three::interrupted_among(&payload.habit_patterns.data, today.timestamp_millis());
    for habit in &mut payload.habit_patterns.data {
        habit.is_interrupted = interrupted
            .iter()
            .any(|quiet| quiet.habit.registrable_domain == habit.registrable_domain);
    }
    payload.interrupted_habits.data = interrupted;
    for meta in [
        &mut payload.digest_summary.meta,
        &mut payload.top_sites.meta,
        &mut payload.refind_pages.meta,
        &mut payload.search_engine_ranking.meta,
        &mut payload.top_search_concepts.meta,
        &mut payload.query_families.meta,
        &mut payload.activity_mix.meta,
        &mut payload.discovery_trend_day.meta,
        &mut payload.habit_patterns.meta,
        &mut payload.interrupted_habits.meta,
    ] {
        if let CoreIntelligenceSectionWindow::DateRange { date_range } = &mut meta.window {
            *date_range = request.date_range.clone();
        }
    }
    Ok(())
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
    let meta = build_core_intelligence_section_meta_with_modules(
        section_id,
        window,
        is_empty(&data),
        &runtime.modules,
    )?;
    Ok((
        CoreIntelligenceSectionResult { data, meta },
        CoreIntelligenceSectionTiming { section_id: section_id.to_string(), duration_ms },
    ))
}

fn search_effectiveness_is_empty(data: &crate::models::SearchEffectiveness) -> bool {
    data.engine_stats.is_empty()
        && data.top_resolving_sources.is_empty()
        && data.hardest_topics.is_empty()
}

/// Warms immutable all-time payloads after publication, outside the render path.
pub(super) fn warm_overview_snapshots(
    vault: &crate::Vault,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let profiles = vault.read(vault_store::Db::Intelligence, |s| {
        Ok(s.query(
            "SELECT profile_id FROM intelligence_generations ORDER BY profile_id",
            (),
            |r| Ok(r.get::<String>(0)?),
        )?)
    })?;
    for profile in std::iter::once(None).chain(profiles.into_iter().map(Some)) {
        let request = ScopedDateRangeRequest {
            date_range: crate::models::DateRange {
                start: super::intelligence_overview_snapshot::ALL_TIME_SCOPE_START.into(),
                end: Local::now().format("%Y-%m-%d").to_string(),
            },
            profile_id: profile.clone(),
        };
        checkpoint()?;
        let scope_request = request.clone();
        let fingerprint = vault.read(vault_store::Db::Intelligence, move |s| {
            super::intelligence_overview_snapshot::request_fingerprint(s, &scope_request)
        })?;
        let primary = get_intelligence_primary_overview(vault, &request)?;
        checkpoint()?;
        let secondary = get_intelligence_secondary_overview(vault, &request)?;
        checkpoint()?;
        vault.write(vault_store::Db::Intelligence, vault_store::Priority::Batch, move |s| {
            use super::intelligence_overview_snapshot::{
                all_time_scope_key, request_fingerprint, save_overview_snapshot,
            };
            if request_fingerprint(s, &request)? != fingerprint {
                return Ok(());
            }
            let scope = all_time_scope_key(profile.as_deref());
            save_overview_snapshot(
                s,
                &scope,
                OverviewBand::Primary,
                &fingerprint,
                &serde_json::to_string(&primary)?,
            )?;
            save_overview_snapshot(
                s,
                &scope,
                OverviewBand::Secondary,
                &fingerprint,
                &serde_json::to_string(&secondary)?,
            )?;
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HardTopic, SearchEffectiveness};

    #[test]
    fn search_effectiveness_empty_state_requires_all_signal_groups_to_be_empty() {
        assert!(search_effectiveness_is_empty(&SearchEffectiveness::default()));

        let search_effectiveness = SearchEffectiveness {
            hardest_topics: crate::models::IntelligencePage {
                items: vec![HardTopic {
                    family_id: "family".to_string(),
                    query_family: "rust coverage".to_string(),
                    reformulation_count: 2,
                    re_search_lag_days: 1.0,
                }],
                next_cursor: None,
            },
            ..SearchEffectiveness::default()
        };

        assert!(!search_effectiveness_is_empty(&search_effectiveness));
    }
}
