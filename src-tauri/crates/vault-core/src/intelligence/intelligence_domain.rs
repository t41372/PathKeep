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
//!   `search_trails` through its profile-and-time index; arrivals and onward
//!   sites sum the site's `site_flows` rows for the window's days. The cost
//!   grows with the site's own URLs and visits in the window, not with the
//!   archive.
//! - Discovery trend collapses dates after SQL aggregation so the Rust side
//!   never materializes visit-level rows for charting.

use super::{collapse_date_key, date_range_bounds, display_name_for_domain};
use crate::models::{
    CategoryFilteredDateRangeRequest, DiscoveryTrend, DiscoveryTrendPoint, DomainDeepDive,
    DomainDeepDiveRequest, DomainPageStat, DomainSearchStat, DomainTrend, DomainTrendPoint,
    DomainTrendRequest, OnThisDayEntry, RhythmHeatmap, RhythmHeatmapCell, SiteArrivals,
    SiteFlowStat,
};
use anyhow::Result;
use chrono::{Datelike, Timelike};
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

        let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
        let domain = request.registrable_domain.as_str();
        let profile_id = request.profile_id.as_deref();

        let mut trend_statement = connection.prepare(
            "SELECT r.date_key, SUM(r.visit_count), MIN(r.domain_category)
         FROM domain_daily_rollups_rows r INDEXED BY idx_domain_daily_site
         WHERE r.registrable_domain = ?1 AND r.date_key >= ?2 AND r.date_key <= ?3
           AND r.generation IN (SELECT daily FROM intelligence_generations)
           AND (?4 IS NULL OR r.profile_id = ?4)
         GROUP BY r.date_key
         ORDER BY r.date_key ASC",
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
            SELECT r.initial_query,COUNT(*) AS landings
            FROM search_trails_rows r INDEXED BY idx_search_trails_landing
            WHERE r.landing_domain=?1 AND r.first_visit_ms>=?2 AND r.first_visit_ms<?3
              AND r.generation IN (SELECT navigation FROM intelligence_generations)
              AND r.profile_id IN (SELECT profile_key FROM archive.source_profiles WHERE profile_key IS NOT NULL AND (?4 IS NULL OR profile_key=?4))
            GROUP BY r.initial_query
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
        let (arrivals, referrers, search_sites, onward) =
            site_flows(connection, domain, &request.date_range, profile_id)?;
        let site_searches = site_searches(connection, domain, start_ms, end_ms, profile_id)?;

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
            arrivals,
            referrers,
            search_sites,
            onward,
            site_searches,
        })
    })
}

/// Searches shown per site.
const SITE_SEARCHES_SHOWN: usize = 8;

/// One engine's most frequent keyword searches in a range, grouped by normalized query as
/// Frequent searches groups them, each with its newest spelling.
const SITE_SEARCHES_SQL: &str = "WITH counts AS (
        SELECT normalized_query,COUNT(*) AS search_count,MAX(visit_time_ms) AS last_time
        FROM search_events_rows INDEXED BY idx_search_events_engine_time
        WHERE query_kind='keyword' AND search_engine=?1 AND visit_time_ms>=?2 AND visit_time_ms<?3
          AND generation IN (SELECT navigation FROM intelligence_generations)
          AND (?4 IS NULL OR profile_id=?4)
        GROUP BY normalized_query ORDER BY search_count DESC,normalized_query LIMIT ?5
     ) SELECT c.normalized_query,c.search_count,c.last_time,
        (SELECT raw_query FROM search_events_rows newest INDEXED BY idx_search_events_query_time
         WHERE newest.query_kind='keyword' AND newest.normalized_query=c.normalized_query
           AND newest.search_engine=?1
           AND newest.generation IN (SELECT navigation FROM intelligence_generations)
           AND newest.visit_time_ms>=?2 AND newest.visit_time_ms<?3
           AND (?4 IS NULL OR newest.profile_id=?4)
         ORDER BY newest.visit_time_ms DESC,newest.visit_id DESC LIMIT 1)
     FROM counts c";

/// What was searched on the site in the window: the keyword searches of each search engine whose
/// rule matches the site (usually one), merged by normalized query. Cost follows the engine's
/// searches in the range, as Frequent searches' does (`ipc-performance.md` §8).
fn site_searches(
    connection: &Session<'_>,
    domain: &str,
    start_ms: i64,
    end_ms: i64,
    profile_id: Option<&str>,
) -> Result<Vec<DomainSearchStat>> {
    let engines = super::load_search_engine_sites(connection)?
        .into_iter()
        .filter(|(_, host)| crate::visit_taxonomy::registrable_domain_for_host(host) == domain)
        .map(|(engine, _)| engine)
        .collect::<Vec<_>>();
    // normalized query -> (count, newest time, newest spelling)
    let mut merged = HashMap::<String, (i64, i64, String)>::new();
    for engine in engines {
        let rows = connection.query(
            SITE_SEARCHES_SQL,
            params![engine.as_str(), start_ms, end_ms, profile_id, SITE_SEARCHES_SHOWN as i64],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<i64>(1)?,
                    r.get::<i64>(2)?,
                    r.get::<Option<String>>(3)?,
                ))
            },
        )?;
        for (normalized, count, last, raw) in rows {
            let entry = merged.entry(normalized.clone()).or_insert((0, i64::MIN, normalized));
            entry.0 += count;
            if last > entry.1 {
                entry.1 = last;
                if let Some(raw) = raw {
                    entry.2 = raw;
                }
            }
        }
    }
    // Most often first, then by normalized query, as Frequent searches orders them.
    let mut searches = merged.into_iter().collect::<Vec<_>>();
    searches.sort_by(|(a, (a_count, ..)), (b, (b_count, ..))| {
        b_count.cmp(a_count).then_with(|| a.cmp(b))
    });
    Ok(searches
        .into_iter()
        .take(SITE_SEARCHES_SHOWN)
        .map(|(_, (count, _, query))| DomainSearchStat { query, count })
        .collect())
}

/// Other sites shown on the site page per list.
const SITE_FLOWS_SHOWN: usize = 8;

/// The site's arrivals in the window and, most visits first, the sites whose links opened it,
/// the search engines' sites whose results did, and the sites its links opened.
fn site_flows(
    connection: &Session<'_>,
    domain: &str,
    range: &crate::models::DateRange,
    profile_id: Option<&str>,
) -> Result<(SiteArrivals, Vec<SiteFlowStat>, Vec<SiteFlowStat>, Vec<SiteFlowStat>)> {
    let rows = connection.query(
        "SELECT r.kind,r.other,SUM(r.visit_count)
         FROM site_flows_rows r INDEXED BY idx_site_flows_site
         WHERE r.registrable_domain=?1 AND r.date_key>=?2 AND r.date_key<=?3
           AND r.generation IN (SELECT navigation FROM intelligence_generations)
           AND (?4 IS NULL OR r.profile_id=?4)
         GROUP BY r.kind,r.other",
        params![domain, range.start.as_str(), range.end.as_str(), profile_id],
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?)),
    )?;
    let mut arrivals = SiteArrivals::default();
    let (mut referrers, mut search_sites, mut onward) = (Vec::new(), Vec::new(), Vec::new());
    for (kind, other, visit_count) in rows {
        let list = match kind.as_str() {
            "direct" => {
                arrivals.direct += visit_count;
                continue;
            }
            "unrecorded" => {
                arrivals.unrecorded += visit_count;
                continue;
            }
            "search" => {
                arrivals.search += visit_count;
                &mut search_sites
            }
            "link" => {
                arrivals.link += visit_count;
                &mut referrers
            }
            "onward" => &mut onward,
            _ => continue,
        };
        // An opener PathKeep did not see counts toward the arrivals but names no site.
        if !other.is_empty() {
            list.push(SiteFlowStat { registrable_domain: other, visit_count });
        }
    }
    for list in [&mut referrers, &mut search_sites, &mut onward] {
        list.sort_by(|a, b| {
            b.visit_count
                .cmp(&a.visit_count)
                .then_with(|| a.registrable_domain.cmp(&b.registrable_domain))
        });
        list.truncate(SITE_FLOWS_SHOWN);
    }
    Ok((arrivals, referrers, search_sites, onward))
}

/// Buckets visits into a day-of-week by hour heatmap without materializing
/// every visit row on the frontend.
pub fn get_browsing_rhythm(
    vault: &crate::Vault,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<RhythmHeatmap> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        if request.category.is_none()
            && let Some(heatmap) = rhythm_from_daily_hours(connection, &request)?
        {
            return Ok(heatmap);
        }
        rhythm_from_visits(connection, &request)
    })
}

/// The rhythm from each day's hour counts in the daily summary rollup: a row per day and
/// profile, where the scan below reads every visit in the range (half a second for all time
/// on a 387k-visit archive, every row of 14.4M). `None` when a day in the range was built
/// before the rollup kept hours.
fn rhythm_from_daily_hours(
    connection: &Session<'_>,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<Option<RhythmHeatmap>> {
    let rows = connection.query(
        "SELECT date_key, hours FROM daily_summary_rollups
         WHERE (?1 IS NULL OR profile_id = ?1) AND date_key >= ?2 AND date_key <= ?3",
        params![
            request.profile_id.as_deref(),
            request.date_range.start.as_str(),
            request.date_range.end.as_str()
        ],
        |row| Ok((row.get::<String>(0)?, row.get::<String>(1)?)),
    )?;
    let mut buckets = HashMap::<(i64, i64), i64>::new();
    for (date_key, hours) in rows {
        if hours.is_empty() {
            return Ok(None);
        }
        let dow = chrono::NaiveDate::parse_from_str(&date_key, "%Y-%m-%d")?
            .weekday()
            .num_days_from_sunday() as i64;
        for (hour, count) in hours.split(',').enumerate() {
            let count = count.parse::<i64>()?;
            if count > 0 {
                *buckets.entry((dow, hour as i64)).or_default() += count;
            }
        }
    }
    Ok(Some(rhythm_heatmap(buckets)))
}

/// The rhythm counted visit by visit: for a category filter, which the daily rollup does not
/// split by, and for days built before it kept hours.
pub(super) fn rhythm_from_visits(
    connection: &Session<'_>,
    request: &CategoryFilteredDateRangeRequest,
) -> Result<RhythmHeatmap> {
    let zone = super::InsightsZone::for_reading(connection)?;
    let (start_ms, end_ms) = zone.range_bounds(&request.date_range)?;
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
        let local = zone.datetime(visit_time_ms);
        let dow = local.weekday().num_days_from_sunday() as i64;
        let hour = local.hour() as i64;
        *buckets.entry((dow, hour)).or_default() += 1;
    }
    Ok(rhythm_heatmap(buckets))
}

fn rhythm_heatmap(buckets: HashMap<(i64, i64), i64>) -> RhythmHeatmap {
    let max_count = buckets.values().copied().max().unwrap_or_default();
    let cells = buckets
        .into_iter()
        .map(|((dow, hour), visit_count)| RhythmHeatmapCell { dow, hour, visit_count })
        .collect::<Vec<_>>();
    RhythmHeatmap { cells, max_count }
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
    let new_sites = super::intelligence_shared::new_sites_by_day(
        connection,
        &request.date_range,
        request.profile_id.as_deref(),
    )?;
    let mut statement = connection.prepare(
        "SELECT date_key, SUM(total_visits)
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
                let day = row.get::<String>(0)?;
                let new = new_sites.get(&day).copied().unwrap_or(0);
                Ok((collapse_date_key(&day, &request.granularity), new, row.get::<i64>(1)?))
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
    let today = super::InsightsZone::for_reading(s)?.today();
    let month_day = today.format("%m-%d").to_string();
    let year = today.year();
    let days=s.query("SELECT date_key,SUM(total_visits) FROM daily_summary_rollups WHERE (?1 IS NULL OR profile_id=?1) AND SUBSTR(date_key,6)=?2 AND CAST(SUBSTR(date_key,1,4) AS INTEGER)<>?3 GROUP BY date_key ORDER BY date_key DESC",params![profile,month_day,year],|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?)))?;
    let mut result = Vec::new();
    for (date, total_visits) in days {
        let range = crate::models::DateRange { start: date.clone(), end: date.clone() };
        let (start, end) = date_range_bounds(s, &range)?;
        let deep_dive_sessions=s.query_one("SELECT COUNT(*) FROM intelligence_generations g CROSS JOIN sessions_rows r INDEXED BY idx_sessions_overlap ON r.profile_id=g.profile_id WHERE r.generation=g.navigation AND (?1 IS NULL OR g.profile_id=?1) AND r.is_deep_dive=1 AND r.last_visit_ms>=?2 AND r.first_visit_ms<?3",params![profile,start,end],|r|Ok(r.get::<i64>(0)?))?;
        // One day: read it directly rather than through the range cache (`site_days`).
        let sites = super::site_days::SiteDays::read(
            s,
            &super::site_days::SiteDayScope::of(&range, profile),
        )?;
        let page = super::intelligence_refind::rank_top_sites(
            &sites,
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
            "SELECT r.date_key, SUM(r.visit_count)
         FROM domain_daily_rollups_rows r INDEXED BY idx_domain_daily_site
         WHERE r.registrable_domain = ?1 AND r.date_key >= ?2 AND r.date_key <= ?3
           AND r.generation IN (SELECT daily FROM intelligence_generations)
         GROUP BY r.date_key
         ORDER BY r.date_key ASC",
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
