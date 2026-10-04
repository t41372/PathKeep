//! Domain, discovery, and calendar-oriented Core Intelligence read models.
//!
//! ## Responsibilities
//! - Serve domain deep-dive summaries for one registrable domain and date
//!   range.
//! - Build aggregate discovery/browsing patterns that the overview and detail
//!   routes share.
//! - Keep domain-specific visit-shaping logic out of the rebuild path.
//!
//! ## Not responsible for
//! - Session/trail detail queries.
//! - `/intelligence` overview section composition.
//! - Export payload formatting or entity explanations.
//!
//! ## Dependencies
//! - Parent-module visit/date helpers and domain visit loaders.
//! - `daily_summary_rollups`, `visit_derived_facts`, and `sessions` in the
//!   intelligence plane plus archive visit/url joins.
//!
//! ## Performance notes
//! - Deep-dive requests never load visit rows into Rust. Totals and the trend
//!   come from `domain_daily_rollups`; top pages seek the site's URLs through
//!   `idx_urls_registrable_domain` and count each URL's visits in the window
//!   through `idx_visits_visible_url_time`; landing searches read
//!   `search_trails` through its profile-and-time index. The cost grows with
//!   the site's own URLs and visits in the window, not with the archive.
//! - Discovery trend collapses dates after SQL aggregation so the Rust side
//!   never materializes visit-level rows for charting.

use super::{
    collapse_date_key, date_range_bounds, display_name_for_domain, ensure_core_intelligence_schema,
    local_datetime_from_millis,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{
        AppConfig, CategoryFilteredDateRangeRequest, DiscoveryTrend, DiscoveryTrendPoint,
        DomainDeepDive, DomainDeepDiveRequest, DomainPageStat, DomainSearchStat, DomainTrend,
        DomainTrendPoint, DomainTrendRequest, OnThisDayEntry, RhythmHeatmap, RhythmHeatmapCell,
    },
};
use anyhow::Result;
use chrono::{Datelike, Local, Timelike};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, HashMap};

/// Builds the site detail page: how much one registrable domain was visited
/// in a window, on which days, which of its pages, and which searches ended
/// there. Everything is aggregated in SQL; see the module performance notes.
pub fn get_domain_deep_dive(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &DomainDeepDiveRequest,
) -> Result<DomainDeepDive> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let domain = request.registrable_domain.as_str();
    let profile_id = request.profile_id.as_deref();

    let mut trend_statement = connection.prepare(
        "SELECT date_key, SUM(visit_count), MIN(domain_category)
         FROM domain_daily_rollups
         WHERE registrable_domain = ?1
           AND date_key >= ?2
           AND date_key <= ?3
           AND (?4 IS NULL OR profile_id = ?4)
         GROUP BY date_key
         ORDER BY date_key ASC",
    )?;
    let mut domain_category: Option<String> = None;
    let visit_trend = trend_statement
        .query_map(
            params![domain, request.date_range.start, request.date_range.end, profile_id],
            |row| {
                Ok((
                    DomainTrendPoint { date_key: row.get(0)?, visit_count: row.get(1)? },
                    row.get::<_, String>(2)?,
                ))
            },
        )?
        .map(|row| {
            row.map(|(point, category)| {
                domain_category.get_or_insert(category);
                point
            })
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let total_visits = visit_trend.iter().map(|point| point.visit_count).sum::<i64>();
    let active_days = visit_trend.len() as i64;

    // URL rows are per profile, so the same page in two browsers is grouped
    // by its URL string.
    let mut pages_statement = connection.prepare(
        "SELECT url, title, visits, COUNT(*) OVER ()
         FROM (
           SELECT urls.url AS url, MAX(urls.title) AS title, COUNT(visits.id) AS visits
           FROM archive.urls AS urls
           JOIN archive.visits AS visits
             ON visits.url_id = urls.id
            AND visits.reverted_at IS NULL
            AND visits.visit_time_ms >= ?2
            AND visits.visit_time_ms < ?3
           WHERE urls.registrable_domain = ?1
             AND (?4 IS NULL OR urls.source_profile_id IN (
                   SELECT id FROM archive.source_profiles WHERE profile_key = ?4))
           GROUP BY urls.url
         )
         ORDER BY visits DESC, url ASC
         LIMIT 10",
    )?;
    let mut page_count = 0_i64;
    let top_pages = pages_statement
        .query_map(params![domain, start_ms, end_ms, profile_id], |row| {
            Ok((
                DomainPageStat {
                    path: path_from_url(&row.get::<_, String>(0)?),
                    url: row.get(0)?,
                    title: row.get(1)?,
                    visit_count: row.get(2)?,
                },
                row.get::<_, i64>(3)?,
            ))
        })?
        .map(|row| {
            row.map(|(page, total)| {
                page_count = total;
                page
            })
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // `profile_id IN (...)` instead of `?4 IS NULL OR profile_id = ?4` so the
    // profile-and-time index applies with or without a profile filter.
    let mut searches_statement = connection.prepare(
        "SELECT initial_query, COUNT(*) AS landings, SUM(COUNT(*)) OVER ()
         FROM search_trails
         WHERE profile_id IN (
                 SELECT profile_key FROM archive.source_profiles
                 WHERE profile_key IS NOT NULL AND (?4 IS NULL OR profile_key = ?4))
           AND first_visit_ms >= ?2
           AND first_visit_ms < ?3
           AND landing_domain = ?1
         GROUP BY initial_query
         ORDER BY landings DESC, initial_query ASC
         LIMIT 8",
    )?;
    let mut landing_search_count = 0_i64;
    let landing_searches = searches_statement
        .query_map(params![domain, start_ms, end_ms, profile_id], |row| {
            Ok((DomainSearchStat { query: row.get(0)?, count: row.get(1)? }, row.get::<_, i64>(2)?))
        })?
        .map(|row| {
            row.map(|(search, total)| {
                landing_search_count = total;
                search
            })
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(DomainDeepDive {
        registrable_domain: domain.to_string(),
        display_name: display_name_for_domain(domain),
        domain_category: domain_category.unwrap_or_else(|| "unknown".to_string()),
        total_visits,
        active_days,
        page_count,
        landing_search_count,
        top_pages,
        landing_searches,
        visit_trend,
    })
}

/// Buckets visits into a day-of-week by hour heatmap without materializing
/// every visit row on the frontend.
pub fn get_browsing_rhythm(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<RhythmHeatmap> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT visits.visit_time_ms
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         WHERE (?1 IS NULL OR visit_derived_facts.profile_id = ?1)
           AND (?2 IS NULL OR visit_derived_facts.domain_category = ?2)
           AND visits.visit_time_ms >= ?3
           AND visits.visit_time_ms < ?4",
    )?;
    let mut buckets = HashMap::<(i64, i64), i64>::new();
    let rows = statement.query_map(
        params![request.profile_id.as_deref(), request.category.as_deref(), start_ms, end_ms],
        |row| row.get::<_, i64>(0),
    )?;
    for row in rows {
        let visit_time_ms = row?;
        let local = local_datetime_from_millis(visit_time_ms);
        let dow = local.weekday().num_days_from_sunday() as i64;
        let hour = local.hour() as i64;
        *buckets.entry((dow, hour)).or_default() += 1;
    }
    let max_count = buckets.values().copied().max().unwrap_or_default();
    let cells = buckets
        .into_iter()
        .map(|((dow, hour), visit_count)| RhythmHeatmapCell { dow, hour, visit_count })
        .collect::<Vec<_>>();
    Ok(RhythmHeatmap { cells, max_count })
}

/// Returns new-domain discovery over time with day/week/month collapsing done
/// after database aggregation.
pub fn get_discovery_trend(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &crate::models::GranularityDateRangeRequest,
) -> Result<DiscoveryTrend> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_discovery_trend_with_connection(&connection, request)
}

pub(crate) fn get_discovery_trend_with_connection(
    connection: &Connection,
    request: &crate::models::GranularityDateRangeRequest,
) -> Result<DiscoveryTrend> {
    let mut statement = connection.prepare(
        "SELECT date_key, SUM(new_domains), SUM(total_visits)
         FROM daily_summary_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY date_key
         ORDER BY date_key ASC",
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
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut available_years_statement = connection.prepare(
        "SELECT DISTINCT CAST(SUBSTR(date_key, 1, 4) AS INTEGER)
         FROM daily_summary_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
         ORDER BY 1 DESC",
    )?;
    let available_years = available_years_statement
        .query_map(params![request.profile_id.as_deref()], |row| row.get::<_, i32>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut grouped = BTreeMap::<String, (i64, i64)>::new();
    for (date_key, new_domain_count, total_visits) in rows {
        let entry = grouped.entry(date_key).or_insert((0, 0));
        entry.0 += new_domain_count;
        entry.1 += total_visits;
    }
    Ok(DiscoveryTrend {
        points: grouped
            .into_iter()
            .map(|(date_key, (new_domain_count, total_visits))| DiscoveryTrendPoint {
                discovery_rate: if total_visits == 0 {
                    0.0
                } else {
                    new_domain_count as f32 / total_visits as f32
                },
                date_key,
                new_domain_count,
                total_visits,
            })
            .collect(),
        available_years,
    })
}

/// Surfaces historical anniversaries for the current local month/day without
/// exposing the underlying visit ids.
pub fn get_on_this_day(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    profile_id: Option<&str>,
) -> Result<Vec<OnThisDayEntry>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_on_this_day_with_connection(&connection, profile_id)
}

pub(crate) fn get_on_this_day_with_connection(
    connection: &Connection,
    profile_id: Option<&str>,
) -> Result<Vec<OnThisDayEntry>> {
    let today = Local::now();
    let month_day = today.format("%m-%d").to_string();
    let current_year = today.year();
    let mut statement = connection.prepare(
        "SELECT visits.visit_time_ms, visit_derived_facts.registrable_domain, sessions.is_deep_dive
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         LEFT JOIN sessions ON sessions.session_id = visit_derived_facts.session_id
         WHERE (?1 IS NULL OR visit_derived_facts.profile_id = ?1)
           AND strftime('%m-%d', datetime(visits.visit_time_ms / 1000, 'unixepoch', 'localtime')) = ?2
           AND CAST(strftime('%Y', datetime(visits.visit_time_ms / 1000, 'unixepoch', 'localtime')) AS INTEGER) <> ?3",
    )?;
    let rows = statement.query_map(params![profile_id, month_day, current_year], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?.unwrap_or(0) != 0,
        ))
    })?;
    let mut grouped = BTreeMap::<i32, Vec<(String, bool)>>::new();
    for row in rows {
        let (visit_time_ms, domain, is_deep_dive) = row?;
        grouped
            .entry(local_datetime_from_millis(visit_time_ms).year())
            .or_default()
            .push((domain, is_deep_dive));
    }
    Ok(grouped
        .into_iter()
        .rev()
        .map(|(year, items)| {
            let total_visits = items.len() as i64;
            let deep_dive_sessions = items.iter().filter(|(_, is_deep)| *is_deep).count() as i64;
            let mut top_domains = items
                .iter()
                .fold(HashMap::<String, i64>::new(), |mut acc, (domain, _)| {
                    *acc.entry(domain.clone()).or_default() += 1;
                    acc
                })
                .into_iter()
                .collect::<Vec<_>>();
            top_domains
                .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            let top_domains =
                top_domains.into_iter().take(3).map(|(domain, _)| domain).collect::<Vec<_>>();
            OnThisDayEntry {
                year,
                date: format!("{year}-{}", Local::now().format("%m-%d")),
                total_visits,
                top_domains: top_domains.clone(),
                summary: top_domains.first().map(|domain| format!("Mostly browsing {domain}")),
                deep_dive_sessions,
            }
        })
        .collect())
}

/// Returns one domain's day-level visit trend for the requested date window.
pub fn get_domain_trend(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &DomainTrendRequest,
) -> Result<DomainTrend> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let mut statement = connection.prepare(
        "SELECT date_key, SUM(visit_count)
         FROM domain_daily_rollups
         WHERE registrable_domain = ?1
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY date_key
         ORDER BY date_key ASC",
    )?;
    let points = statement
        .query_map(
            params![request.registrable_domain, request.date_range.start, request.date_range.end],
            |row| Ok(DomainTrendPoint { date_key: row.get(0)?, visit_count: row.get(1)? }),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(DomainTrend { registrable_domain: request.registrable_domain.clone(), points })
}

/// Extracts the URL path portion used by the domain detail top-pages summary.
pub(super) fn path_from_url(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|value| value.split_once('/'))
        .map(|(_, path)| format!("/{}", path))
        .unwrap_or_else(|| "/".to_string())
}
