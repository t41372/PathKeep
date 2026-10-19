//! Responsibilities: read compare sets and browser/profile differences.
//! Not responsible for: canonical facts or deterministic stage execution.
//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Phase 4 Core Intelligence read models.
//!
//! This module owns the backend heuristics for:
//! - compare sets detected from search trails
//! - multi-browser/profile behavioral diffs

use super::date_range_bounds;
use super::site_days::{SiteDayCache, SiteDayScope, SiteDays};
use crate::models::{
    BrowserCategoryDistribution, BrowserDiff, BrowserProfileSummary, CategoryMixEntry, CompareSet,
    CompareSetDetail, CompareSetDetailRequest, CompareSetPage, ExclusiveDomainEntry,
    ScopedDateRangeRequest, SessionSummary, TrailSummary,
};
use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashMap, HashSet};
use vault_store::{OptionalRow, Session, params_owned as params};

pub fn get_compare_set_detail(
    vault: &crate::Vault,
    request: &CompareSetDetailRequest,
) -> Result<CompareSetDetail> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_compare_set_detail_with_connection(connection, request)
    })
}

pub(crate) fn get_compare_set_detail_with_connection(
    connection: &Session<'_>,
    request: &CompareSetDetailRequest,
) -> Result<CompareSetDetail> {
    let (trail_id, page_category) = parse_compare_set_id(&request.compare_set_id)?;
    let scoped = ScopedDateRangeRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
    };
    let trail_members = load_compare_trail_members(connection, &scoped, Some(trail_id))?;
    let compare_set = build_compare_sets_for_trail(&trail_members)
        .into_iter()
        .find(|entry| entry.compare_set_id == request.compare_set_id)
        .with_context(|| format!("compare set {} was not found", request.compare_set_id))?;
    let trail = load_trail_summary(connection, trail_id)?;
    let session = trail
        .session_id
        .as_deref()
        .map(|session_id| load_session_summary(connection, session_id))
        .transpose()?;
    let zone = super::InsightsZone::for_reading(connection)?;
    let recent_days = trail_members
        .iter()
        .filter(|member| member.page_category == page_category)
        .map(|member| zone.date_key(member.visit_time_ms))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    Ok(CompareSetDetail { compare_set, trail, session, recent_days })
}

pub fn get_multi_browser_diff(
    vault: &crate::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<BrowserDiff> {
    let cache = vault.site_days().clone();
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        get_multi_browser_diff_with_connection(connection, &cache, &request)
    })
}

/// Every profile's sites in the range, side by side, from the range's shared site-day read for
/// all profiles (the comparison is between profiles, so a profile filter does not apply).
pub(crate) fn get_multi_browser_diff_with_connection(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    request: &ScopedDateRangeRequest,
) -> Result<BrowserDiff> {
    let days = cache.get(connection, &SiteDayScope::of(&request.date_range, None))?;
    let profiles = load_browser_profile_summaries(connection, &days)?;
    let category_distributions = load_category_distributions(connection, request)?;

    let mut exclusive_domains = Vec::new();
    let mut shared = Vec::new();
    for site in &days.sites {
        match site.per_profile.as_slice() {
            [(profile, visit_count)] => exclusive_domains.push(ExclusiveDomainEntry {
                registrable_domain: site.domain.clone(),
                profile_id: days.profiles[*profile].profile_id.clone(),
                visit_count: *visit_count,
            }),
            [] => {}
            both => shared.push((
                both.iter().map(|(_, visit_count)| visit_count).sum::<i64>(),
                site.domain.clone(),
            )),
        }
    }

    exclusive_domains.sort_by(|left, right| {
        right
            .visit_count
            .cmp(&left.visit_count)
            .then_with(|| left.profile_id.cmp(&right.profile_id))
            .then_with(|| left.registrable_domain.cmp(&right.registrable_domain))
    });
    // Most visits in every profile together first, so the card can name the ones that matter.
    shared.sort_by(|(left_visits, left), (right_visits, right)| {
        right_visits.cmp(left_visits).then_with(|| left.cmp(right))
    });
    let shared_domains = shared.into_iter().map(|(_, domain)| domain).collect();

    Ok(BrowserDiff { profiles, exclusive_domains, shared_domains, category_distributions })
}

#[derive(Debug, Clone)]
pub(super) struct CompareTrailMember {
    pub trail_id: String,
    pub initial_query: String,
    pub landing_url: Option<String>,
    pub canonical_url: String,
    pub url: String,
    pub title: Option<String>,
    pub registrable_domain: String,
    pub page_category: String,
    pub visit_time_ms: i64,
}

#[derive(Debug, Default, Clone)]
struct ComparePageAggregate {
    canonical_url: String,
    url: String,
    title: Option<String>,
    registrable_domain: String,
    visit_count: i64,
    is_landing: bool,
}

pub(super) fn build_compare_sets_for_trail(
    trail_members: &[CompareTrailMember],
) -> Vec<CompareSet> {
    let mut by_category = HashMap::<String, Vec<&CompareTrailMember>>::new();
    for member in trail_members {
        if !is_compare_candidate_category(&member.page_category) {
            continue;
        }
        by_category.entry(member.page_category.clone()).or_default().push(member);
    }

    let mut compare_sets = Vec::new();
    for (page_category, members) in by_category {
        let mut by_page = HashMap::<String, ComparePageAggregate>::new();
        let mut alternations = 0_i64;
        let mut distinct_domains = HashMap::<String, usize>::new();
        let mut previous_url = None::<&str>;
        for member in &members {
            let page = by_page.entry(member.canonical_url.clone()).or_insert_with(|| {
                ComparePageAggregate {
                    canonical_url: member.canonical_url.clone(),
                    url: member.url.clone(),
                    title: member.title.clone(),
                    registrable_domain: member.registrable_domain.clone(),
                    visit_count: 0,
                    is_landing: false,
                }
            });
            page.visit_count += 1;
            if member
                .landing_url
                .as_deref()
                .is_some_and(|landing| landing == member.url || landing == member.canonical_url)
            {
                page.is_landing = true;
            }
            *distinct_domains.entry(member.registrable_domain.clone()).or_default() += 1;
            if previous_url.is_some_and(|previous| previous != member.canonical_url) {
                alternations += 1;
            }
            previous_url = Some(&member.canonical_url);
        }

        if by_page.len() < 2 || alternations < 2 {
            continue;
        }

        let multi_domain = distinct_domains.len() >= 2;
        let sibling_compare = !multi_domain
            && has_sibling_pages(&members)
            && members
                .windows(2)
                .filter(|window| {
                    window[0].registrable_domain == window[1].registrable_domain
                        && window[0].canonical_url != window[1].canonical_url
                        && (window[1].visit_time_ms - window[0].visit_time_ms).abs()
                            <= 10 * 60 * 1000
                })
                .count()
                >= 2;
        if !multi_domain && !sibling_compare {
            continue;
        }

        let trail_id = members[0].trail_id.clone();
        let mut pages = by_page.into_values().collect::<Vec<_>>();
        pages.sort_by(|left, right| {
            right
                .visit_count
                .cmp(&left.visit_count)
                .then_with(|| left.registrable_domain.cmp(&right.registrable_domain))
                .then_with(|| left.url.cmp(&right.url))
        });

        let last_visit_ms = members.iter().map(|member| member.visit_time_ms).max().unwrap_or(0);
        compare_sets.push(CompareSet {
            compare_set_id: format!("compare:{trail_id}:{page_category}"),
            trail_id,
            search_query: members[0].initial_query.clone(),
            page_category,
            last_visited_at: super::rfc3339_from_millis(last_visit_ms),
            pages: pages
                .into_iter()
                .map(|page| CompareSetPage {
                    canonical_url: page.canonical_url,
                    url: page.url,
                    title: page.title,
                    registrable_domain: page.registrable_domain,
                    visit_count: page.visit_count,
                    is_landing: page.is_landing,
                })
                .collect(),
        });
    }

    compare_sets
}

pub(super) fn is_compare_candidate_category(page_category: &str) -> bool {
    matches!(
        page_category,
        "article_page"
            | "category_page"
            | "docs_page"
            | "forum_thread"
            | "issue"
            | "product_page"
            | "pull_request"
            | "repo"
            | "video_page"
    )
}

/// Whether two different pages of the trail sit side by side under one parent path
/// (`/products/a` and `/products/b`): the "sibling URLs" a same-site comparison is made of.
fn has_sibling_pages(members: &[&CompareTrailMember]) -> bool {
    let mut pages_by_parent = HashMap::<&str, HashSet<&str>>::new();
    for member in members {
        let parent = parent_path(&member.url);
        if !parent.is_empty() {
            pages_by_parent.entry(parent).or_default().insert(&member.canonical_url);
        }
    }
    pages_by_parent.values().any(|pages| pages.len() >= 2)
}

/// Host and path up to the last segment, without query or fragment; empty for a site's root.
fn parent_path(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let page = rest.split(['?', '#']).next().unwrap_or(rest).trim_end_matches('/');
    page.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn load_compare_trail_members(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
    trail_id: Option<&str>,
) -> Result<Vec<CompareTrailMember>> {
    let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT search_trails.trail_id,
                search_trails.initial_query,
                search_trails.landing_url,
                visit_derived_facts.canonical_url,
                urls.url,
                urls.title,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.page_category,
                visits.visit_time_ms
         FROM search_trails
         JOIN search_trail_members ON search_trail_members.trail_id = search_trails.trail_id
         JOIN visit_derived_facts ON visit_derived_facts.visit_id = search_trail_members.visit_id
         JOIN archive.visits AS visits ON visits.id = search_trail_members.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE (?1 IS NULL OR search_trails.profile_id = ?1)
           AND (?2 IS NULL OR search_trails.trail_id = ?2)
           AND search_trails.last_visit_ms >= ?3
           AND search_trails.first_visit_ms < ?4
         ORDER BY search_trails.trail_id ASC, visits.visit_time_ms ASC, search_trail_members.ordinal ASC",
    )?;
    statement
        .query_map(params![request.profile_id.as_deref(), trail_id, start_ms, end_ms], |row| {
            Ok(CompareTrailMember {
                trail_id: row.get(0)?,
                initial_query: row.get(1)?,
                landing_url: row.get(2)?,
                canonical_url: row.get(3)?,
                url: row.get(4)?,
                title: row.get(5)?,
                registrable_domain: row.get(6)?,
                page_category: row.get(7)?,
                visit_time_ms: row.get(8)?,
            })
        })?
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn parse_compare_set_id(compare_set_id: &str) -> Result<(&str, &str)> {
    let payload = compare_set_id.strip_prefix("compare:").with_context(|| {
        format!(
            "compare_set detail expects ids shaped like 'compare:<trail_id>:<page_category>', got {compare_set_id}",
        )
    })?;
    payload
        .rsplit_once(':')
        .with_context(|| format!("compare set {compare_set_id} is missing page category"))
}

fn load_trail_summary(connection: &Session<'_>, trail_id: &str) -> Result<TrailSummary> {
    connection
        .query_one(
            "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                    landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
             FROM search_trails
             WHERE trail_id = ?1",
            [trail_id],
            trail_summary_from_row,
        )
        .with_context(|| format!("trail {trail_id} was not found"))
}

fn load_session_summary(connection: &Session<'_>, session_id: &str) -> Result<SessionSummary> {
    connection
        .query_one(
            "SELECT session_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title
             FROM sessions
             WHERE session_id = ?1",
            [session_id],
            |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    first_visit_ms: row.get(1)?,
                    last_visit_ms: row.get(2)?,
                    visit_count: row.get(3)?,
                    search_count: row.get(4)?,
                    domain_count: row.get(5)?,
                    is_deep_dive: row.get::< i64>(6)? != 0,
                    auto_title: row.get(7)?,
                })
            },
        )
        .optional()?
        .with_context(|| format!("session {session_id} was not found"))
}

fn trail_summary_from_row(row: &vault_store::Row) -> vault_store::Result<TrailSummary> {
    Ok(TrailSummary {
        trail_id: row.get(0)?,
        session_id: row.get(1)?,
        initial_query: row.get(2)?,
        search_engine: row.get(3)?,
        reformulation_count: row.get(4)?,
        visit_count: row.get(5)?,
        landing_url: row.get(6)?,
        landing_domain: row.get(7)?,
        first_visit_ms: row.get(8)?,
        last_visit_ms: row.get(9)?,
        max_depth: row.get(10)?,
        queries: serde_json::from_str(&row.get::<String>(11)?).unwrap_or_default(),
    })
}

/// Each profile with visits in the range, with its name from the archive, by name.
fn load_browser_profile_summaries(
    connection: &Session<'_>,
    days: &SiteDays,
) -> Result<Vec<BrowserProfileSummary>> {
    let names = connection.query(
        "SELECT profile_key, profile_name, COALESCE(browser_family, browser_kind)
         FROM archive.source_profiles",
        (),
        |row| Ok((row.get::<String>(0)?, (row.get::<String>(1)?, row.get::<String>(2)?))),
    )?;
    let names = names.into_iter().collect::<HashMap<_, _>>();
    let mut profiles = days
        .profiles
        .iter()
        .filter(|profile| profile.visits > 0)
        .filter_map(|profile| {
            let (profile_name, browser_family) = names.get(&profile.profile_id)?.clone();
            Some(BrowserProfileSummary {
                profile_id: profile.profile_id.clone(),
                profile_name,
                browser_family,
                domain_count: profile.sites,
                visit_count: profile.visits,
            })
        })
        .collect::<Vec<_>>();
    profiles.sort_by(|a, b| {
        a.profile_name.cmp(&b.profile_name).then_with(|| a.profile_id.cmp(&b.profile_id))
    });
    Ok(profiles)
}

fn load_category_distributions(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<BrowserCategoryDistribution>> {
    let profile_totals = load_profile_totals(connection, request)?;
    let mut statement = connection.prepare(
        "SELECT archive.source_profiles.profile_key,
                archive.source_profiles.profile_name,
                category_daily_rollups.domain_category,
                SUM(category_daily_rollups.visit_count)
         FROM category_daily_rollups
         JOIN archive.source_profiles ON archive.source_profiles.profile_key = category_daily_rollups.profile_id
         WHERE category_daily_rollups.date_key >= ?1
           AND category_daily_rollups.date_key <= ?2
         GROUP BY archive.source_profiles.profile_key,
                  archive.source_profiles.profile_name,
                  category_daily_rollups.domain_category
         ORDER BY archive.source_profiles.profile_name ASC, category_daily_rollups.domain_category ASC",
    )?;
    let rows = statement
        .query_map(params![request.date_range.start, request.date_range.end], |row| {
            Ok((
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                row.get::<String>(2)?,
                row.get::<i64>(3)?,
            ))
        })?
        .collect::<vault_store::Result<Vec<_>>>()?;

    let mut grouped = HashMap::<String, BrowserCategoryDistribution>::new();
    for (profile_id, profile_name, domain_category, visit_count) in rows {
        let total = profile_totals.get(&profile_id).copied().unwrap_or(1) as f32;
        grouped
            .entry(profile_id.clone())
            .or_insert_with(|| BrowserCategoryDistribution {
                profile_id: profile_id.clone(),
                profile_name: profile_name.clone(),
                categories: Vec::new(),
            })
            .categories
            .push(CategoryMixEntry {
                domain_category,
                visit_count,
                share: visit_count as f32 / total,
            });
    }

    let mut distributions = grouped.into_values().collect::<Vec<_>>();
    distributions.sort_by(|left, right| left.profile_name.cmp(&right.profile_name));
    Ok(distributions)
}

fn load_profile_totals(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
) -> Result<HashMap<String, i64>> {
    let mut statement = connection.prepare(
        "SELECT profile_id, SUM(total_visits)
         FROM daily_summary_rollups
         WHERE date_key >= ?1
           AND date_key <= ?2
         GROUP BY profile_id",
    )?;
    let rows = statement
        .query_map(params![request.date_range.start, request.date_range.end], |row| {
            Ok((row.get::<String>(0)?, row.get::<i64>(1)?))
        })?
        .collect::<vault_store::Result<Vec<_>>>()?;
    Ok(rows.into_iter().collect())
}
