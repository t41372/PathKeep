//! Core Intelligence summary and secondary read models.
//!
//! ## Responsibilities
//! - Serve aggregate digest, activity-mix, stable-source, and friction surfaces.
//! - Keep comparison-to-previous-window logic close to the SQL that produces it.
//! - Own the derived read helpers that the staged overview reuses.
//!
//! ## Not responsible for
//! - Session/trail/refind detail queries.
//! - `/intelligence` section composition and timing.
//! - Rebuild/migration orchestration.
//!
//! ## Dependencies
//! - Parent-module date helpers and search-engine naming helpers.
//! - `source_effectiveness`, `daily_summary_rollups`, `query_families`, and
//!   `search_trails` tables in the intelligence plane.
//!
//! ## Performance notes
//! - All `*_with_connection` helpers reuse an existing SQLite handle so staged
//!   overview loads do not reopen the same database for each card.
//! - KPI comparisons intentionally aggregate on the database side before doing
//!   light Rust-side math.

use super::{
    build_kpi, collapse_date_key, date_range_bounds, display_name_for_domain,
    display_name_for_search_engine, ensure_core_intelligence_schema, previous_date_range,
    rfc3339_from_millis,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{
        ActivityMix, ActivityMixTrend, ActivityMixTrendPoint, AppConfig, CategoryChangeEntry,
        CategoryMixEntry, DateRange, DigestSummary, EngineEffectiveness, FrictionSignal,
        GranularityDateRangeRequest, HardTopic, ReopenedInvestigation, ScopedDateRangeRequest,
        SearchEffectiveness, SearchEffectivenessRequest, StableSource,
    },
};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, HashMap};

/// Builds the category-share mix and the delta versus the immediately previous
/// window so the UI can explain where attention shifted.
pub fn get_activity_mix(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<ActivityMix> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_activity_mix_with_connection(&connection, request)
}

pub(crate) fn get_activity_mix_with_connection(
    connection: &Connection,
    request: &ScopedDateRangeRequest,
) -> Result<ActivityMix> {
    let current =
        load_category_shares(connection, &request.date_range, request.profile_id.as_deref())?;
    let previous = load_category_shares(
        connection,
        &previous_date_range(&request.date_range)?,
        request.profile_id.as_deref(),
    )?;
    let previous_map = previous
        .iter()
        .map(|entry| (entry.domain_category.clone(), entry.clone()))
        .collect::<HashMap<_, _>>();
    let categories = current;
    let change_vs_previous = categories
        .iter()
        .map(|entry| {
            let previous_share = previous_map
                .get(&entry.domain_category)
                .map(|value| value.share)
                .unwrap_or_default();
            CategoryChangeEntry {
                domain_category: entry.domain_category.clone(),
                current_share: entry.share,
                previous_share,
                change_points: entry.share - previous_share,
            }
        })
        .collect::<Vec<_>>();
    Ok(ActivityMix { categories, change_vs_previous })
}

/// Aggregates category mix over time for charts that need week/month/year
/// collapsing without exposing visit-level rows.
pub fn get_activity_mix_trend(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &GranularityDateRangeRequest,
) -> Result<ActivityMixTrend> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut statement = connection.prepare(
        "SELECT date_key, domain_category, SUM(visit_count)
         FROM category_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY date_key, domain_category
         ORDER BY date_key ASC, SUM(visit_count) DESC",
    )?;
    let rows = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.date_range.start,
                request.date_range.end
            ],
            |row| {
                Ok((
                    collapse_date_key(&row.get::<_, String>(0)?, &request.granularity),
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut grouped = BTreeMap::<String, Vec<(String, i64)>>::new();
    for (date_key, category, visit_count) in rows {
        grouped.entry(date_key).or_default().push((category, visit_count));
    }
    let points = grouped
        .into_iter()
        .map(|(date_key, categories)| {
            let total = categories.iter().map(|(_, count)| *count).sum::<i64>().max(1) as f32;
            ActivityMixTrendPoint {
                date_key,
                categories: categories
                    .into_iter()
                    .map(|(domain_category, visit_count)| CategoryMixEntry {
                        share: visit_count as f32 / total,
                        domain_category,
                        visit_count,
                    })
                    .collect(),
            }
        })
        .collect();
    Ok(ActivityMixTrend { points })
}

/// Builds the KPI digest that compares the selected window against the previous
/// equivalent window.
pub fn get_digest_summary(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<DigestSummary> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_digest_summary_with_connection(&connection, request)
}

pub(crate) fn get_digest_summary_with_connection(
    connection: &Connection,
    request: &ScopedDateRangeRequest,
) -> Result<DigestSummary> {
    let current =
        load_summary_totals(connection, &request.date_range, request.profile_id.as_deref())?;
    let previous_range = previous_date_range(&request.date_range)?;
    let previous = load_summary_totals(connection, &previous_range, request.profile_id.as_deref())?;
    let current_deep =
        count_deep_dive_sessions(connection, &request.date_range, request.profile_id.as_deref())?;
    let previous_deep =
        count_deep_dive_sessions(connection, &previous_range, request.profile_id.as_deref())?;
    let current_refind = super::count_refind_pages_in_range(
        connection,
        &request.date_range,
        request.profile_id.as_deref(),
    )?;
    let previous_refind = super::count_refind_pages_in_range(
        connection,
        &previous_range,
        request.profile_id.as_deref(),
    )?;
    Ok(DigestSummary {
        date_range: request.date_range.clone(),
        total_visits: build_kpi(current.total_visits, previous.total_visits),
        total_searches: build_kpi(current.total_searches, previous.total_searches),
        new_domains: build_kpi(current.new_domains, previous.new_domains),
        deep_read_pages: build_kpi(current_deep, previous_deep),
        refind_pages: build_kpi(current_refind, previous_refind),
    })
}

/// Returns the domains that most consistently resolved search trails in the
/// requested window.
pub fn get_stable_sources(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<StableSource>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_stable_sources_with_connection(&connection, request)
}

pub(crate) fn get_stable_sources_with_connection(
    connection: &Connection,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<StableSource>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT registrable_domain, source_role, trail_count, stable_landing_count, effectiveness_score
         FROM source_effectiveness
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3
         ORDER BY effectiveness_score DESC, registrable_domain ASC
         LIMIT ?4",
    )?;
    statement
        .query_map(params![request.profile_id.as_deref(), start_ms, end_ms, 10_i64], |row| {
            let domain: String = row.get(0)?;
            Ok(StableSource {
                display_name: display_name_for_domain(&domain),
                registrable_domain: domain,
                source_role: row.get(1)?,
                trail_count: row.get(2)?,
                stable_landing_count: row.get(3)?,
                effectiveness_score: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Summarizes search-engine difficulty and likely hard topics without exposing
/// visit-level raw search rows.
pub fn get_search_effectiveness(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SearchEffectivenessRequest,
) -> Result<SearchEffectiveness> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_search_effectiveness_with_connection(&connection, request)
}

pub(crate) fn get_search_effectiveness_with_connection(
    connection: &Connection,
    request: &SearchEffectivenessRequest,
) -> Result<SearchEffectiveness> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT search_engine,
                AVG(reformulation_count),
                COUNT(*),
                AVG(max_depth)
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4
         GROUP BY search_engine
         ORDER BY COUNT(*) DESC, search_engine ASC",
    )?;
    let engine_stats = statement
        .query_map(
            params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
            |row| {
                let engine: String = row.get(0)?;
                Ok(EngineEffectiveness {
                    display_name: display_name_for_search_engine(&engine),
                    search_engine: engine,
                    avg_reformulations: row.get::<_, f32>(1)?,
                    total_trails: row.get(2)?,
                    avg_depth: row.get::<_, f32>(3)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let top_resolving_sources = get_stable_sources_with_connection(
        connection,
        &ScopedDateRangeRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
        },
    )?;
    let mut family_statement = connection.prepare(
        "SELECT family_id, anchor_query, member_count, first_seen_ms, last_seen_ms
         FROM query_families
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_seen_ms >= ?3
           AND first_seen_ms < ?4
         ORDER BY member_count DESC, last_seen_ms DESC
         LIMIT 5",
    )?;
    let hardest_topics = family_statement
        .query_map(
            params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
            |row| {
                let family_id: String = row.get(0)?;
                let first_seen: i64 = row.get(3)?;
                let last_seen: i64 = row.get(4)?;
                let lag_days = ((last_seen - first_seen) as f32 / 86_400_000.0).max(0.0);
                Ok(HardTopic {
                    family_id,
                    query_family: row.get(1)?,
                    reformulation_count: row.get(2)?,
                    re_search_lag_days: lag_days,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SearchEffectiveness { engine_stats, top_resolving_sources, hardest_topics })
}

/// Flags likely friction patterns from deterministic search-trail heuristics so
/// the overview can surface where search sessions struggled to settle.
pub fn get_friction_signals(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<FrictionSignal>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_friction_signals_with_connection(&connection, request)
}

pub(crate) fn get_friction_signals_with_connection(
    connection: &Connection,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<FrictionSignal>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT initial_query, landing_domain, reformulation_count, visit_count
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3
           AND (reformulation_count >= 2 OR (landing_domain IS NULL AND visit_count >= 3))
         ORDER BY reformulation_count DESC, visit_count DESC
         LIMIT ?4",
    )?;
    statement
        .query_map(params![request.profile_id.as_deref(), start_ms, end_ms, 10_i64], |row| {
            let landing_domain: Option<String> = row.get(1)?;
            let reformulation_count: i64 = row.get(2)?;
            let visit_count: i64 = row.get(3)?;
            let signal_kind =
                if reformulation_count >= 2 { "excessive_reformulation" } else { "bounce_pattern" };
            let description = if reformulation_count >= 2 {
                format!("Repeated search reformulation after query '{}'.", row.get::<_, String>(0)?)
            } else {
                "Search trail did not settle on a stable landing page.".to_string()
            };
            Ok(FrictionSignal {
                registrable_domain: landing_domain,
                url: None,
                evidence_type: "weak".to_string(),
                signal_kind: signal_kind.to_string(),
                occurrence_count: visit_count.max(reformulation_count),
                description,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Returns investigations that resurfaced across multiple days so the overview
/// can call out recurring unresolved work.
pub fn get_reopened_investigations(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<ReopenedInvestigation>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_reopened_investigations_with_connection(&connection, request)
}

pub(crate) fn get_reopened_investigations_with_connection(
    connection: &Connection,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<ReopenedInvestigation>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT investigation_id, anchor_type, anchor_id, anchor_label, occurrence_count, distinct_days,
                first_seen_ms, last_seen_ms
         FROM reopened_investigations
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3
         ORDER BY last_seen_ms DESC, occurrence_count DESC
         LIMIT ?4",
    )?;
    statement
        .query_map(params![request.profile_id.as_deref(), start_ms, end_ms, 10_i64], |row| {
            Ok(ReopenedInvestigation {
                investigation_id: row.get(0)?,
                anchor_type: row.get(1)?,
                anchor_id: row.get(2)?,
                anchor_label: row.get(3)?,
                occurrence_count: row.get(4)?,
                distinct_days: row.get(5)?,
                first_seen_at: rfc3339_from_millis(row.get(6)?),
                last_seen_at: rfc3339_from_millis(row.get(7)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn load_category_shares(
    connection: &Connection,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<Vec<CategoryMixEntry>> {
    let mut statement = connection.prepare(
        "SELECT domain_category, SUM(visit_count)
         FROM category_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY domain_category
         ORDER BY SUM(visit_count) DESC, domain_category ASC",
    )?;
    let rows = statement
        .query_map(params![profile_id, range.start, range.end], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let total = rows.iter().map(|(_, count)| *count).sum::<i64>().max(1) as f32;
    Ok(rows
        .into_iter()
        .map(|(domain_category, visit_count)| CategoryMixEntry {
            share: visit_count as f32 / total,
            domain_category,
            visit_count,
        })
        .collect())
}

#[derive(Default)]
struct SummaryTotals {
    total_visits: i64,
    total_searches: i64,
    new_domains: i64,
}

fn load_summary_totals(
    connection: &Connection,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<SummaryTotals> {
    connection
        .query_row(
            "SELECT SUM(total_visits), SUM(total_searches), SUM(new_domains)
             FROM daily_summary_rollups
             WHERE (?1 IS NULL OR profile_id = ?1)
               AND date_key >= ?2
               AND date_key <= ?3",
            params![profile_id, range.start, range.end],
            |row| {
                Ok(SummaryTotals {
                    total_visits: row.get::<_, Option<i64>>(0)?.unwrap_or_default(),
                    total_searches: row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
                    new_domains: row.get::<_, Option<i64>>(2)?.unwrap_or_default(),
                })
            },
        )
        .map_err(Into::into)
}

fn count_deep_dive_sessions(
    connection: &Connection,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<i64> {
    let (start_ms, end_ms) = date_range_bounds(range)?;
    connection
        .query_row(
            "SELECT COUNT(*)
             FROM sessions
             WHERE (?1 IS NULL OR profile_id = ?1)
               AND is_deep_dive = 1
               AND first_visit_ms >= ?2
               AND first_visit_ms < ?3",
            params![profile_id, start_ms, end_ms],
            |row| row.get(0),
        )
        .map_err(Into::into)
}
