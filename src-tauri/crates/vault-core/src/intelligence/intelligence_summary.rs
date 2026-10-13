//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
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
//! - `daily_summary_rollups`, `query_families`, and `search_trails` tables in
//!   the intelligence plane.
//!
//! ## Performance notes
//! - All `*_with_connection` helpers reuse an existing SQLite handle so staged
//!   overview loads do not reopen the same database for each card.
//! - KPI comparisons intentionally aggregate on the database side before doing
//!   light Rust-side math.

use super::site_days::{SiteDayCache, SiteDayScope};
use super::{
    build_kpi, collapse_date_key, date_range_bounds, display_name_for_domain,
    display_name_for_search_engine, new_sites_by_day, previous_date_range, rfc3339_from_millis,
};
use crate::models::{
    ActivityMix, ActivityMixTrend, ActivityMixTrendPoint, CategoryChangeEntry, CategoryMixEntry,
    CoreIntelligenceSectionMeta, CoreIntelligenceSectionWindow, DateRange, DigestSummary,
    EngineEffectiveness, FrictionSignal, GranularityDateRangeRequest, HardTopic, IntelligencePage,
    ReopenedInvestigation, ScopedDateRangeRequest, ScopedPageRequest, SearchEffectiveness,
    SearchEffectivenessRequest, SectionEvidence, SectionEvidenceUnit, StableSource,
};
use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
use vault_store::{Session, params_owned as params};

/// Builds the category-share mix and the delta versus the immediately previous
/// window so the UI can explain where attention shifted.
pub fn get_activity_mix(
    vault: &crate::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<ActivityMix> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_activity_mix_with_connection(connection, request)
    })
}

pub(crate) fn get_activity_mix_with_connection(
    connection: &Session<'_>,
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
    vault: &crate::Vault,
    request: &GranularityDateRangeRequest,
) -> Result<ActivityMixTrend> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

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
                        collapse_date_key(&row.get::<String>(0)?, &request.granularity),
                        row.get::<String>(1)?,
                        row.get::<i64>(2)?,
                    ))
                },
            )?
            .collect::<vault_store::Result<Vec<_>>>()?;
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
    })
}

/// Builds the KPI digest that compares the selected window against the previous
/// equivalent window.
pub fn get_digest_summary(
    vault: &crate::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<DigestSummary> {
    let cache = vault.site_days().clone();
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        get_digest_summary_with_connection(connection, &cache, &request)
    })
}

pub(crate) fn get_digest_summary_with_connection(
    connection: &Session<'_>,
    cache: &SiteDayCache,
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
    // The sites of each range come from its shared site-day read (`site_days`), which the cards
    // ranking sites in the same range read too.
    let profile = request.profile_id.as_deref();
    let current_domains =
        cache.get(connection, &SiteDayScope::of(&request.date_range, profile))?.sites.len() as i64;
    let previous_domains =
        cache.get(connection, &SiteDayScope::of(&previous_range, profile))?.sites.len() as i64;
    let current_active =
        estimate_active_time_ms(connection, &request.date_range, request.profile_id.as_deref())?;
    let previous_active =
        estimate_active_time_ms(connection, &previous_range, request.profile_id.as_deref())?;
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
        new_domains: build_kpi(
            new_sites_by_day(connection, &request.date_range, request.profile_id.as_deref())?
                .values()
                .sum(),
            new_sites_by_day(connection, &previous_range, request.profile_id.as_deref())?
                .values()
                .sum(),
        ),
        deep_read_pages: build_kpi(current_deep, previous_deep),
        refind_pages: build_kpi(current_refind, previous_refind),
        distinct_domains: build_kpi(current_domains, previous_domains),
        active_time_ms: build_kpi(current_active, previous_active),
    })
}

/// The sites searches ended on in the range, most first, across browsers or in one: where the
/// user's answers come from. Counted from the search trails, so a range counts only its own
/// searches (the stored per-profile source rows covered all of history).
pub fn get_stable_sources(
    vault: &crate::Vault,
    request: &ScopedPageRequest,
) -> Result<IntelligencePage<StableSource>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |s| {
        get_stable_sources_with_connection(s, &request)
    })
}

pub(crate) fn get_stable_sources_with_connection(
    connection: &Session<'_>,
    request: &ScopedPageRequest,
) -> Result<IntelligencePage<StableSource>> {
    let (start, end) = date_range_bounds(connection, &request.date_range)?;
    let limit = request.limit.unwrap_or(10).clamp(1, 1000) as usize;
    let after = super::paging::cursor::<(i64, String)>(request.cursor.as_deref())?;
    let rows = connection.query(
        "SELECT r.landing_domain, COUNT(*), COUNT(DISTINCT r.initial_query)
         FROM search_trails_rows r INDEXED BY idx_search_trails_recent
         WHERE r.first_visit_ms >= ?2 AND r.first_visit_ms < ?3
           AND r.generation IN (SELECT navigation FROM intelligence_generations)
           AND (?1 IS NULL OR r.profile_id = ?1)
           AND r.landing_domain IS NOT NULL
         GROUP BY r.landing_domain
         HAVING ?4 IS NULL OR COUNT(*) < ?4 OR (COUNT(*) = ?4 AND r.landing_domain > ?5)
         ORDER BY COUNT(*) DESC, r.landing_domain LIMIT ?6",
        params![
            request.profile_id.as_deref(),
            start,
            end,
            after.as_ref().map(|value| value.0),
            after.as_ref().map(|value| value.1.as_str()),
            (limit + 1) as i64
        ],
        |r| {
            let domain = r.get::<String>(0)?;
            let searches = r.get::<i64>(1)?;
            Ok((
                StableSource {
                    display_name: display_name_for_domain(&domain),
                    registrable_domain: domain.clone(),
                    search_count: searches,
                    query_count: r.get(2)?,
                },
                (searches, domain),
            ))
        },
    )?;
    super::paging::finish(rows, limit)
}

/// Summarizes search-engine difficulty and likely hard topics without exposing
/// visit-level raw search rows.
pub fn get_search_effectiveness(
    vault: &crate::Vault,
    request: &SearchEffectivenessRequest,
) -> Result<SearchEffectiveness> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_search_effectiveness_with_connection(connection, request)
    })
}

pub(crate) fn get_search_effectiveness_with_connection(
    connection: &Session<'_>,
    request: &SearchEffectivenessRequest,
) -> Result<SearchEffectiveness> {
    let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT r.search_engine,
                AVG(r.reformulation_count),
                COUNT(*),
                AVG(r.max_depth)
         FROM search_trails_rows r INDEXED BY idx_search_trails_recent
         WHERE r.first_visit_ms >= ?3 AND r.first_visit_ms < ?4
           AND r.generation IN (SELECT navigation FROM intelligence_generations)
           AND (?1 IS NULL OR r.profile_id = ?1)
           AND (?2 IS NULL OR r.search_engine = ?2)
         GROUP BY r.search_engine
         ORDER BY COUNT(*) DESC, r.search_engine ASC",
    )?;
    let engine_stats = statement
        .query_map(
            params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
            |row| {
                let engine: String = row.get(0)?;
                Ok(EngineEffectiveness {
                    display_name: display_name_for_search_engine(&engine),
                    search_engine: engine,
                    avg_reformulations: row.get::<f64>(1)? as f32,
                    total_trails: row.get(2)?,
                    avg_depth: row.get::<f64>(3)? as f32,
                })
            },
        )?
        .collect::<vault_store::Result<Vec<_>>>()?;
    let top_resolving_sources = get_stable_sources_with_connection(
        connection,
        &ScopedPageRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            ..Default::default()
        },
    )?;
    let limit = request.topics_limit.unwrap_or(5).clamp(1, 1000) as usize;
    let after = super::paging::cursor::<(i64, i64, String)>(request.topics_cursor.as_deref())?;
    let after_clause = if after.is_some() {
        "AND r.member_count<=?5 AND (r.member_count<?5 OR (r.last_seen_ms<=?6 AND (r.last_seen_ms<?6 OR r.family_id>?7)))"
    } else {
        ""
    };
    // The families searched in the range, from its start in recency order, sorted by size.
    // Walked in size order instead, a short range read families until it found a page of its own.
    let rows = connection.query(
        &format!(
            "SELECT r.family_id,r.anchor_query,r.member_count,r.first_seen_ms,r.last_seen_ms
        FROM query_families_rows r INDEXED BY idx_query_families_recent
        WHERE r.last_seen_ms>=?3 AND r.first_seen_ms<?4
          AND r.generation IN (SELECT aggregates FROM intelligence_generations)
          AND (?1 IS NULL OR r.profile_id=?1) AND (?2 IS NULL OR r.search_engine=?2) {after_clause}
        ORDER BY r.member_count DESC,r.last_seen_ms DESC,r.family_id LIMIT ?8"
        ),
        params![
            request.profile_id,
            request.engine,
            start_ms,
            end_ms,
            after.as_ref().map(|v| v.0),
            after.as_ref().map(|v| v.1),
            after.as_ref().map(|v| v.2.as_str()),
            (limit + 1) as i64
        ],
        |r| {
            let id = r.get::<String>(0)?;
            let count = r.get::<i64>(2)?;
            let first = r.get::<i64>(3)?;
            let last = r.get::<i64>(4)?;
            Ok((
                HardTopic {
                    family_id: id.clone(),
                    query_family: r.get(1)?,
                    reformulation_count: count,
                    re_search_lag_days: ((last - first) as f32 / 86_400_000.0).max(0.0),
                },
                (count, last, id),
            ))
        },
    )?;
    let hardest_topics = super::paging::finish(rows, limit)?;
    Ok(SearchEffectiveness { engine_stats, top_resolving_sources, hardest_topics })
}

/// Flags likely friction patterns from deterministic search-trail heuristics so
/// the overview can surface where search sessions struggled to settle.
pub fn get_friction_signals(
    vault: &crate::Vault,
    request: &ScopedPageRequest,
) -> Result<IntelligencePage<FrictionSignal>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |s| {
        get_friction_signals_with_connection(s, &request)
    })
}

pub(crate) fn get_friction_signals_with_connection(
    connection: &Session<'_>,
    request: &ScopedPageRequest,
) -> Result<IntelligencePage<FrictionSignal>> {
    let (start, end) = date_range_bounds(connection, &request.date_range)?;
    let limit = request.limit.unwrap_or(10).clamp(1, 1000) as usize;
    let after = super::paging::cursor::<(i64, i64, String)>(request.cursor.as_deref())?;
    let after_clause = if after.is_some() {
        "AND r.reformulation_count<=?4 AND (r.reformulation_count<?4 OR (r.visit_count<=?5 AND (r.visit_count<?5 OR r.trail_id>?6)))"
    } else {
        ""
    };
    // The trails begun in the range that may be friction, from their own partial index, sorted.
    // Walked in friction order instead, a range read trails until it found a page of its own:
    // 0.4 s for any range at 14.4M visits.
    let rows = connection.query(
        &format!(
            "SELECT r.initial_query,r.landing_domain,r.reformulation_count,r.visit_count,r.trail_id
        FROM search_trails_rows r INDEXED BY idx_search_trails_friction
        WHERE r.first_visit_ms>=?2 AND r.first_visit_ms<?3
          AND (r.reformulation_count >= 2 OR (r.landing_domain IS NULL AND r.visit_count >= 3))
          AND r.generation IN (SELECT navigation FROM intelligence_generations)
          AND (?1 IS NULL OR r.profile_id=?1) {after_clause}
        ORDER BY r.reformulation_count DESC,r.visit_count DESC,r.trail_id LIMIT ?7"
        ),
        params![
            request.profile_id,
            start,
            end,
            after.as_ref().map(|v| v.0),
            after.as_ref().map(|v| v.1),
            after.as_ref().map(|v| v.2.as_str()),
            (limit + 1) as i64
        ],
        |r| {
            let reforms = r.get::<i64>(2)?;
            let visits = r.get::<i64>(3)?;
            let query = r.get::<String>(0)?;
            let description = if reforms >= 2 {
                format!("Repeated search reformulation after query '{query}'.")
            } else {
                "Search trail did not settle on a stable landing page.".to_owned()
            };
            Ok((
                FrictionSignal {
                    registrable_domain: r.get(1)?,
                    url: None,
                    evidence_type: "weak".into(),
                    signal_kind: if reforms >= 2 {
                        "excessive_reformulation"
                    } else {
                        "bounce_pattern"
                    }
                    .into(),
                    occurrence_count: visits.max(reforms),
                    description,
                    search_query: query,
                    reformulation_count: reforms,
                    visit_count: visits,
                },
                (reforms, visits, r.get::<String>(4)?),
            ))
        },
    )?;
    super::paging::finish(rows, limit)
}

/// Returns investigations that resurfaced across multiple days so the overview
/// can call out recurring unresolved work.
pub fn get_reopened_investigations(
    vault: &crate::Vault,
    request: &ScopedPageRequest,
) -> Result<IntelligencePage<ReopenedInvestigation>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |s| {
        get_reopened_investigations_with_connection(s, &request)
    })
}

pub(crate) fn get_reopened_investigations_with_connection(
    connection: &Session<'_>,
    request: &ScopedPageRequest,
) -> Result<IntelligencePage<ReopenedInvestigation>> {
    let (start, end) = date_range_bounds(connection, &request.date_range)?;
    let limit = request.limit.unwrap_or(10).clamp(1, 1000) as usize;
    let after = super::paging::cursor::<(i64, i64, String)>(request.cursor.as_deref())?;
    let after_clause = if after.is_some() {
        "AND r.last_seen_ms<=?4 AND (r.last_seen_ms<?4 OR (r.occurrence_count<=?5 AND (r.occurrence_count<?5 OR r.investigation_id>?6)))"
    } else {
        ""
    };
    let rows=connection.query(&format!("SELECT r.investigation_id,r.anchor_type,r.anchor_id,r.anchor_label,r.occurrence_count,r.distinct_days,r.first_seen_ms,r.last_seen_ms
        FROM reopened_investigations_rows r INDEXED BY idx_reopened_investigations_recent
        WHERE r.generation IN (SELECT aggregates FROM intelligence_generations)
          AND (?1 IS NULL OR r.profile_id=?1) AND r.last_seen_ms>=?2 AND r.first_seen_ms<?3 {after_clause}
        ORDER BY r.last_seen_ms DESC,r.occurrence_count DESC,r.investigation_id LIMIT ?7"),
        params![request.profile_id,start,end,after.as_ref().map(|v|v.0),after.as_ref().map(|v|v.1),after.as_ref().map(|v|v.2.as_str()),(limit+1) as i64],|r| {
            let id=r.get::<String>(0)?;let count=r.get::<i64>(4)?;let time=r.get::<i64>(7)?;
            Ok((ReopenedInvestigation {investigation_id:id.clone(),anchor_type:r.get(1)?,anchor_id:r.get(2)?,anchor_label:r.get(3)?,occurrence_count:count,distinct_days:r.get(5)?,first_seen_at:rfc3339_from_millis(r.get(6)?),last_seen_at:rfc3339_from_millis(time)},(time,count,id)))
        })?;
    super::paging::finish(rows, limit)
}

fn load_category_shares(
    connection: &Session<'_>,
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
            Ok((row.get::<String>(0)?, row.get::<i64>(1)?))
        })?
        .collect::<vault_store::Result<Vec<_>>>()?;
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
}

fn load_summary_totals(
    connection: &Session<'_>,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<SummaryTotals> {
    connection
        .query_one(
            "SELECT SUM(r.total_visits), SUM(r.total_searches)
             FROM daily_summary_rollups_rows r INDEXED BY idx_daily_summary_range
             WHERE r.date_key >= ?2 AND r.date_key <= ?3
               AND r.generation IN (SELECT daily FROM intelligence_generations)
               AND (?1 IS NULL OR r.profile_id = ?1)",
            params![profile_id, range.start, range.end],
            |row| {
                Ok(SummaryTotals {
                    total_visits: row.get::<Option<i64>>(0)?.unwrap_or_default(),
                    total_searches: row.get::<Option<i64>>(1)?.unwrap_or_default(),
                })
            },
        )
        .map_err(Into::into)
}

/// Fills each section's `evidence`: how much of the archive its card was computed from, in the
/// unit the card counts (`SectionEvidenceUnit`), for the scope it was read in. A section without
/// a unit, or read for one calendar day across the years, has none.
pub fn fill_section_evidence(
    vault: &crate::Vault,
    meta: &mut CoreIntelligenceSectionMeta,
    profile_id: Option<&str>,
) -> Result<()> {
    if crate::intelligence_sections::evidence_unit(&meta.section_id).is_none() {
        return Ok(());
    }
    let mut owned = meta.clone();
    let cache = vault.site_days().clone();
    let profile = profile_id.map(str::to_owned);
    let filled = vault.read(vault_store::Db::Intelligence, move |s| {
        fill_sections_evidence(s, &cache, &mut [&mut owned], profile.as_deref())?;
        Ok(owned)
    })?;
    *meta = filled;
    Ok(())
}

/// [`fill_section_evidence`] for the sections of one batch, on its open connection. Each count
/// is taken once per window and unit; the site counts come from the site-day read the cards of
/// the same scope keep (`SiteDayCache`).
pub(crate) fn fill_sections_evidence(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    metas: &mut [&mut CoreIntelligenceSectionMeta],
    profile_id: Option<&str>,
) -> Result<()> {
    let mut known: Vec<(DateRange, SectionEvidenceUnit, i64)> = Vec::new();
    for meta in metas.iter_mut() {
        let Some(unit) = crate::intelligence_sections::evidence_unit(&meta.section_id) else {
            continue;
        };
        let range = match &meta.window {
            CoreIntelligenceSectionWindow::DateRange { date_range } => date_range.clone(),
            CoreIntelligenceSectionWindow::AllTime => {
                DateRange { start: "0000-01-01".into(), end: "9999-12-31".into() }
            }
            CoreIntelligenceSectionWindow::CalendarDayHistory { .. } => continue,
        };
        let count =
            match known.iter().find(|(seen, seen_unit, _)| *seen == range && *seen_unit == unit) {
                Some((.., count)) => *count,
                None => {
                    let count = count_evidence(connection, cache, unit, &range, profile_id)?;
                    known.push((range, unit, count));
                    count
                }
            };
        meta.evidence = Some(SectionEvidence { count, unit });
    }
    Ok(())
}

fn count_evidence(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    unit: SectionEvidenceUnit,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<i64> {
    Ok(match unit {
        SectionEvidenceUnit::Visits => {
            load_summary_totals(connection, range, profile_id)?.total_visits
        }
        SectionEvidenceUnit::Searches => {
            load_summary_totals(connection, range, profile_id)?.total_searches
        }
        SectionEvidenceUnit::Days => connection.query_one(
            "SELECT COUNT(DISTINCT r.date_key)
             FROM daily_summary_rollups_rows r INDEXED BY idx_daily_summary_range
             WHERE r.date_key >= ?2 AND r.date_key <= ?3
               AND r.generation IN (SELECT daily FROM intelligence_generations)
               AND (?1 IS NULL OR r.profile_id = ?1)
               AND r.total_visits > 0",
            params![profile_id, range.start, range.end],
            |row| Ok(row.get(0)?),
        )?,
        SectionEvidenceUnit::SiteVisits | SectionEvidenceUnit::PageVisits => {
            let sites = cache.get(connection, &SiteDayScope::of(range, profile_id))?;
            sites
                .sites
                .iter()
                .map(|site| {
                    if unit == SectionEvidenceUnit::PageVisits {
                        site.page_visits
                    } else {
                        site.visits
                    }
                })
                .sum()
        }
        SectionEvidenceUnit::SearchTrails | SectionEvidenceUnit::LandedSearches => {
            // The trails begun in the range, as the search cards read them; `landed` keeps the
            // ones that reached a site, all that Stable sources looks at.
            let (start_ms, end_ms) = date_range_bounds(connection, range)?;
            connection.query_one(
                "SELECT COUNT(*)
                 FROM search_trails_rows r INDEXED BY idx_search_trails_recent
                 WHERE r.first_visit_ms >= ?2 AND r.first_visit_ms < ?3
                   AND r.generation IN (SELECT navigation FROM intelligence_generations)
                   AND (?1 IS NULL OR r.profile_id = ?1)
                   AND (?4 = 0 OR r.landing_domain IS NOT NULL)",
                params![
                    profile_id,
                    start_ms,
                    end_ms,
                    i64::from(unit == SectionEvidenceUnit::LandedSearches)
                ],
                |row| Ok(row.get(0)?),
            )?
        }
        SectionEvidenceUnit::Sessions => {
            // Sessions begun in the range. Common paths reads each session's segments by day, so a
            // session running into the range's first day from the night before is the difference.
            let (start_ms, end_ms) = date_range_bounds(connection, range)?;
            connection.query_one(
                "SELECT COUNT(*)
                 FROM sessions_rows r INDEXED BY idx_sessions_range
                 WHERE r.first_visit_ms >= ?2 AND r.first_visit_ms < ?3
                   AND r.generation IN (SELECT navigation FROM intelligence_generations)
                   AND (?1 IS NULL OR r.profile_id = ?1)",
                params![profile_id, start_ms, end_ms],
                |row| Ok(row.get(0)?),
            )?
        }
    })
}

fn count_deep_dive_sessions(
    connection: &Session<'_>,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<i64> {
    let (start_ms, end_ms) = date_range_bounds(connection, range)?;
    connection
        .query_one(
            "SELECT COUNT(*)
             FROM sessions_rows r INDEXED BY idx_sessions_range
             WHERE r.first_visit_ms >= ?2 AND r.first_visit_ms < ?3
               AND r.generation IN (SELECT navigation FROM intelligence_generations)
               AND (?1 IS NULL OR r.profile_id = ?1)
               AND r.is_deep_dive = 1",
            params![profile_id, start_ms, end_ms],
            |row| Ok(row.get(0)?),
        )
        .map_err(Into::into)
}

/// Longest dwell credited to one visit when estimating active time.
const ACTIVE_TIME_PER_VISIT_CAP_MS: i64 = 5 * 60_000;
/// Time credited for the page a session ended on, which has no later visit to measure against.
const ACTIVE_TIME_LAST_PAGE_MS: i64 = 60_000;

/// Estimates active browsing time from session spans.
///
/// Browsers record visit durations unevenly (Safari and Firefox mostly leave them empty), so the
/// estimate uses the sessions table instead: each session counts its first-to-last-visit span plus
/// one minute for the final page, and never more than five minutes per visit so a tab left open
/// inside a session does not inflate the total. Sessions are counted in the window they start in.
fn estimate_active_time_ms(
    connection: &Session<'_>,
    range: &DateRange,
    profile_id: Option<&str>,
) -> Result<i64> {
    let (start_ms, end_ms) = date_range_bounds(connection, range)?;
    connection
        .query_one(
            "SELECT COALESCE(SUM(MIN(r.last_visit_ms - r.first_visit_ms + ?4, r.visit_count * ?5)), 0)
             FROM sessions_rows r INDEXED BY idx_sessions_range
             WHERE r.first_visit_ms >= ?2 AND r.first_visit_ms < ?3
               AND r.generation IN (SELECT navigation FROM intelligence_generations)
               AND (?1 IS NULL OR r.profile_id = ?1)",
            params![
                profile_id,
                start_ms,
                end_ms,
                ACTIVE_TIME_LAST_PAGE_MS,
                ACTIVE_TIME_PER_VISIT_CAP_MS
            ],
            |row| Ok(row.get(0)?),
        )
        .map_err(Into::into)
}
