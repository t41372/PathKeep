//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
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
    collapse_date_key, date_range_bounds, display_name_for_domain, local_datetime_from_millis,
};
use crate::models::{
    CategoryFilteredDateRangeRequest, DiscoveryTrend, DiscoveryTrendPoint, DomainDeepDive,
    DomainDeepDiveRequest, DomainPageStat, DomainSearchStat, DomainTrend, DomainTrendPoint,
    DomainTrendRequest, OnThisDayEntry, RhythmHeatmap, RhythmHeatmapCell,
};
use anyhow::Result;
use chrono::{Datelike, Local, Timelike};
use std::collections::{BTreeMap, HashMap};
use vault_store::{Session, params_owned as params};

/// Builds the site detail page: how much one registrable domain was visited
/// in a window, on which days, which of its pages, and which searches ended
/// there. Everything is aggregated in SQL; see the module performance notes.
pub fn get_domain_deep_dive(
    vault: &crate::Vault,
    request: &DomainDeepDiveRequest,
) -> Result<DomainDeepDive> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

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
                        row.get::<String>(2)?,
                    ))
                },
            )?
            .map(|row| {
                row.map(|(point, category)| {
                    domain_category.get_or_insert(category);
                    point
                })
            })
            .collect::<vault_store::Result<Vec<_>>>()?;
        let total_visits = visit_trend.iter().map(|point| point.visit_count).sum::<i64>();
        let active_days = visit_trend.len() as i64;

        let limit=request.page_size.unwrap_or(10).clamp(1,1000) as usize;
        let pages_after=super::paging::cursor::<(i64,String)>(request.pages_cursor.as_deref())?;
        let mut page_count=0;
        // OFFSET 0 prevents Turso's aggregate decorrelation from grouping the whole archive.
        let rows=connection.query("WITH matched AS (
            SELECT url,MAX(title) AS title,SUM(visits) AS visits FROM (
                SELECT u.url,u.title,(SELECT COUNT(*) FROM archive.visits INDEXED BY idx_visits_visible_url_time
                    WHERE url_id=u.id AND reverted_at IS NULL AND visit_time_ms>=?2 AND visit_time_ms<?3 LIMIT 1 OFFSET 0) AS visits
                FROM archive.pages p INDEXED BY idx_pages_registrable_domain
                CROSS JOIN archive.urls u ON u.page_id=p.id
                WHERE p.registrable_domain IS NOT NULL AND p.registrable_domain=?1
                  AND (?4 IS NULL OR u.source_profile_id IN (SELECT id FROM archive.source_profiles WHERE profile_key=?4))
            ) WHERE visits>0 GROUP BY url
        ), page AS (
            SELECT * FROM matched WHERE ?5 IS NULL OR visits<?5 OR (visits=?5 AND url>?6)
            ORDER BY visits DESC,url LIMIT ?7
        ), totals AS (SELECT COUNT(*) AS total_count FROM matched)
        SELECT page.url,page.title,page.visits,totals.total_count FROM totals LEFT JOIN page ON 1=1 ORDER BY page.visits DESC,page.url",
            params![domain,start_ms,end_ms,profile_id,pages_after.as_ref().map(|v|v.0),pages_after.as_ref().map(|v|v.1.as_str()),(limit+1) as i64],|r| {
                page_count=r.get::<i64>(3)?;
                let item=match r.get::<Option<String>>(0)? {
                    Some(url)=>{let count=r.get::<i64>(2)?;Some((DomainPageStat {path:path_from_url(&url),url:url.clone(),title:r.get(1)?,visit_count:count},(count,url)))},
                    None=>None,
                };Ok(item)
            })?;
        let top_pages=super::paging::finish(rows.into_iter().flatten().collect(),limit)?;
        let searches_after=super::paging::cursor::<(i64,String)>(request.searches_cursor.as_deref())?;
        let mut landing_search_count=0;
        let rows=connection.query("WITH matched AS (
            SELECT initial_query,COUNT(*) AS landings FROM search_trails
            WHERE profile_id IN (SELECT profile_key FROM archive.source_profiles WHERE profile_key IS NOT NULL AND (?4 IS NULL OR profile_key=?4))
              AND first_visit_ms>=?2 AND first_visit_ms<?3 AND landing_domain=?1
            GROUP BY initial_query
        ), page AS (
            SELECT * FROM matched WHERE ?5 IS NULL OR landings<?5 OR (landings=?5 AND initial_query>?6)
            ORDER BY landings DESC,initial_query LIMIT ?7
        ), totals AS (SELECT COALESCE(SUM(landings),0) AS total_count FROM matched)
        SELECT page.initial_query,page.landings,totals.total_count FROM totals LEFT JOIN page ON 1=1 ORDER BY page.landings DESC,page.initial_query",
            params![domain,start_ms,end_ms,profile_id,searches_after.as_ref().map(|v|v.0),searches_after.as_ref().map(|v|v.1.as_str()),(limit+1) as i64],|r| {
                landing_search_count=r.get::<i64>(2)?;
                let item=match r.get::<Option<String>>(0)? {
                    Some(query)=>{let count=r.get::<i64>(1)?;Some((DomainSearchStat {query:query.clone(),count},(count,query)))},
                    None=>None,
                };Ok(item)
            })?;
        let landing_searches=super::paging::finish(rows.into_iter().flatten().collect(),limit)?;

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
    })
}

/// Buckets visits into a day-of-week by hour heatmap without materializing
/// every visit row on the frontend.
pub fn get_browsing_rhythm(
    vault: &crate::Vault,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<RhythmHeatmap> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

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
            |row| Ok(row.get::<i64>(0)?),
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
    })
}

/// Returns new-domain discovery over time with day/week/month collapsing done
/// after database aggregation.
pub fn get_discovery_trend(
    vault: &crate::Vault,
    request: &crate::models::GranularityDateRangeRequest,
) -> Result<DiscoveryTrend> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_discovery_trend_with_connection(connection, request)
    })
}

pub(crate) fn get_discovery_trend_with_connection(
    connection: &Session<'_>,
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
                    collapse_date_key(&row.get::<String>(0)?, &request.granularity),
                    row.get::<i64>(1)?,
                    row.get::<i64>(2)?,
                ))
            },
        )?
        .collect::<vault_store::Result<Vec<_>>>()?;
    let mut available_years_statement = connection.prepare(
        "SELECT DISTINCT CAST(SUBSTR(date_key, 1, 4) AS INTEGER)
         FROM daily_summary_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
         ORDER BY 1 DESC",
    )?;
    let available_years = available_years_statement
        .query_map(params![request.profile_id.as_deref()], |row| Ok(row.get::<i32>(0)?))?
        .collect::<vault_store::Result<Vec<_>>>()?;
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
    vault: &crate::Vault,
    profile_id: Option<&str>,
) -> Result<Vec<OnThisDayEntry>> {
    let profile_id = profile_id.map(str::to_owned);
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let profile_id = profile_id.as_deref();

        get_on_this_day_with_connection(connection, profile_id)
    })
}

pub(crate) fn get_on_this_day_with_connection(
    s: &Session<'_>,
    profile: Option<&str>,
) -> Result<Vec<OnThisDayEntry>> {
    let today = Local::now();
    let month_day = today.format("%m-%d").to_string();
    let year = today.year();
    let days=s.query("SELECT date_key,SUM(total_visits) FROM daily_summary_rollups WHERE (?1 IS NULL OR profile_id=?1) AND SUBSTR(date_key,6)=?2 AND CAST(SUBSTR(date_key,1,4) AS INTEGER)<>?3 GROUP BY date_key ORDER BY date_key DESC",params![profile,month_day,year],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?)))?;
    let mut result = Vec::new();
    for (date, total_visits) in days {
        let range = crate::models::DateRange { start: date.clone(), end: date.clone() };
        let (start, end) = date_range_bounds(&range)?;
        let deep_dive_sessions=s.query_one("SELECT COUNT(*) FROM intelligence_generations g CROSS JOIN sessions_rows r INDEXED BY idx_sessions_overlap ON r.profile_id=g.profile_id WHERE r.generation=g.navigation AND (?1 IS NULL OR g.profile_id=?1) AND r.is_deep_dive=1 AND r.last_visit_ms>=?2 AND r.first_visit_ms<?3",params![profile,start,end],|r|Ok(r.get::<i64>(0)?))?;
        let page = super::intelligence_refind::get_top_sites_with_connection(
            s,
            &crate::models::TopSitesRequest {
                date_range: range,
                profile_id: profile.map(str::to_owned),
                profile_ids: Vec::new(),
                sort_by: Some("visit_count".into()),
                limit: Some(3),
                cursor: None,
            },
        )?;
        let top_domains =
            page.items.into_iter().map(|item| item.registrable_domain).collect::<Vec<_>>();
        result.push(OnThisDayEntry {
            year: date[..4].parse()?,
            date,
            total_visits,
            summary: top_domains.first().map(|domain| format!("Mostly browsing {domain}")),
            top_domains,
            top_domains_next_cursor: page.next_cursor,
            deep_dive_sessions,
        });
    }
    Ok(result)
}

/// Returns one domain's day-level visit trend for the requested date window.
pub fn get_domain_trend(vault: &crate::Vault, request: &DomainTrendRequest) -> Result<DomainTrend> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

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
                params![
                    request.registrable_domain,
                    request.date_range.start,
                    request.date_range.end
                ],
                |row| Ok(DomainTrendPoint { date_key: row.get(0)?, visit_count: row.get(1)? }),
            )?
            .collect::<vault_store::Result<Vec<_>>>()?;
        Ok(DomainTrend { registrable_domain: request.registrable_domain.clone(), points })
    })
}

/// Extracts the URL path portion used by the domain detail top-pages summary.
pub(super) fn path_from_url(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|value| value.split_once('/'))
        .map(|(_, path)| format!("/{}", path))
        .unwrap_or_else(|| "/".to_string())
}
