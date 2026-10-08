//! Responsibilities: read compare sets and browser/profile differences.
//! Not responsible for: canonical facts or deterministic stage execution.
//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Phase 4 Core Intelligence read models.
//!
//! This module owns the backend heuristics for:
//! - compare sets detected from search trails
//! - multi-browser/profile behavioral diffs

use super::{date_range_bounds, local_date_key};
use crate::models::{
    BrowserCategoryDistribution, BrowserDiff, BrowserProfileSummary, CategoryMixEntry, CompareSet,
    CompareSetDetail, CompareSetDetailRequest, CompareSetPage, ExclusiveDomainEntry,
    ScopedDateRangeRequest, SessionSummary, TrailSummary,
};
use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashMap};
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
    let recent_days = trail_members
        .iter()
        .filter(|member| member.page_category == page_category)
        .map(|member| local_date_key(member.visit_time_ms))
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
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_multi_browser_diff_with_connection(connection, request)
    })
}

pub(crate) fn get_multi_browser_diff_with_connection(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
) -> Result<BrowserDiff> {
    let profiles = load_browser_profile_summaries(connection, request)?;
    let domain_counts = load_profile_domain_counts(connection, request)?;
    let category_distributions = load_category_distributions(connection, request)?;

    let mut exclusive_domains = Vec::new();
    let mut shared_domains = Vec::new();
    for (registrable_domain, entries) in domain_counts {
        if entries.len() == 1 {
            let (profile_id, visit_count) = &entries[0];
            exclusive_domains.push(ExclusiveDomainEntry {
                registrable_domain,
                profile_id: profile_id.clone(),
                visit_count: *visit_count,
            });
        } else if entries.len() > 1 {
            shared_domains.push(registrable_domain);
        }
    }

    exclusive_domains.sort_by(|left, right| {
        right
            .visit_count
            .cmp(&left.visit_count)
            .then_with(|| left.profile_id.cmp(&right.profile_id))
            .then_with(|| left.registrable_domain.cmp(&right.registrable_domain))
    });
    shared_domains.sort();

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
            && has_sibling_path_variation(&members)
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

fn has_sibling_path_variation(members: &[&CompareTrailMember]) -> bool {
    let mut path_prefixes = HashMap::<String, usize>::new();
    for member in members {
        let prefix = path_prefix(&member.url);
        *path_prefixes.entry(prefix).or_default() += 1;
    }
    path_prefixes.len() >= 2
}

fn path_prefix(url: &str) -> String {
    url.split_once("://")
        .and_then(|(_, rest)| rest.split_once('/'))
        .map(|(_, path)| {
            let mut segments = path.split('/').filter(|segment| !segment.is_empty());
            segments.next().unwrap_or("/").to_string()
        })
        .unwrap_or_else(|| "/".to_string())
}

fn load_compare_trail_members(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
    trail_id: Option<&str>,
) -> Result<Vec<CompareTrailMember>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
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

fn load_browser_profile_summaries(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<BrowserProfileSummary>> {
    let mut statement = connection.prepare(
        "SELECT source_profiles.profile_key,
                source_profiles.profile_name,
                COALESCE(source_profiles.browser_family, source_profiles.browser_kind),
                COUNT(DISTINCT domain_daily_rollups.registrable_domain),
                COALESCE(SUM(domain_daily_rollups.visit_count), 0)
         FROM archive.source_profiles AS source_profiles
         LEFT JOIN domain_daily_rollups
           ON domain_daily_rollups.profile_id = source_profiles.profile_key
          AND domain_daily_rollups.date_key >= ?1
          AND domain_daily_rollups.date_key <= ?2
         GROUP BY source_profiles.profile_key, source_profiles.profile_name, source_profiles.browser_family, source_profiles.browser_kind
         HAVING COALESCE(SUM(domain_daily_rollups.visit_count), 0) > 0
         ORDER BY source_profiles.profile_name ASC, source_profiles.profile_key ASC",
    )?;
    let profiles = statement
        .query_map(params![request.date_range.start, request.date_range.end], |row| {
            Ok(BrowserProfileSummary {
                profile_id: row.get(0)?,
                profile_name: row.get(1)?,
                browser_family: row.get(2)?,
                domain_count: row.get(3)?,
                visit_count: row.get(4)?,
            })
        })?
        .collect::<vault_store::Result<Vec<_>>>()?;
    Ok(profiles)
}

fn load_profile_domain_counts(
    connection: &Session<'_>,
    request: &ScopedDateRangeRequest,
) -> Result<HashMap<String, Vec<(String, i64)>>> {
    let mut statement = connection.prepare(
        "SELECT registrable_domain, profile_id, SUM(visit_count)
         FROM domain_daily_rollups
         WHERE date_key >= ?1
           AND date_key <= ?2
         GROUP BY registrable_domain, profile_id
         ORDER BY registrable_domain ASC, profile_id ASC",
    )?;
    let rows = statement
        .query_map(params![request.date_range.start, request.date_range.end], |row| {
            Ok((row.get::<String>(0)?, row.get::<String>(1)?, row.get::<i64>(2)?))
        })?
        .collect::<vault_store::Result<Vec<_>>>()?;

    let mut domains = HashMap::<String, Vec<(String, i64)>>::new();
    for (registrable_domain, profile_id, visit_count) in rows {
        domains.entry(registrable_domain).or_default().push((profile_id, visit_count));
    }
    Ok(domains)
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

#[cfg(test)]
mod tests {
    use super::{
        CompareTrailMember, build_compare_sets_for_trail, get_compare_set_detail,
        get_compare_set_detail_with_connection, get_multi_browser_diff_with_connection,
        load_session_summary, load_trail_summary, parse_compare_set_id, path_prefix,
    };
    use crate::models::{CompareSetDetailRequest, DateRange, ScopedDateRangeRequest};
    use crate::test_support::native::TestDatabase;
    use vault_store::{Db, params_owned as params};

    fn request(profile_id: Option<&str>) -> ScopedDateRangeRequest {
        ScopedDateRangeRequest {
            date_range: DateRange {
                start: "1970-01-01".to_string(),
                end: "2100-01-01".to_string(),
            },
            profile_id: profile_id.map(str::to_string),
        }
    }

    fn seed_phase_four_connection() -> TestDatabase {
        let vault = crate::Vault::open_temp().unwrap();
        let connection = TestDatabase::new(&vault, Db::Intelligence);
        let archive = TestDatabase::new(&vault, Db::Archive);
        connection.execute("INSERT INTO intelligence_generations(profile_id) VALUES('profile-a'),('profile-b')",()).unwrap();
        archive
            .execute(
                "INSERT INTO source_profiles(profile_key,profile_name,browser_family,browser_kind,profile_path,discovered_at) VALUES ('profile-a','Chrome Work','chrome','chromium','/fixture',''),
('profile-b','Safari Personal',NULL,'safari','/fixture','')",
                (),
            )
            .expect("insert source profiles");
        let base_visit_ms = 1_800_000_000_000_i64;
        connection
            .execute(
                "INSERT INTO sessions_rows(session_id,first_visit_ms,last_visit_ms,visit_count,search_count,domain_count,is_deep_dive,auto_title,profile_id,computed_at) VALUES ('session-1',?1,?2,4,1,2,1,'SQLite WAL research','profile-a','')",
                params![base_visit_ms, base_visit_ms + 3_000],
            )
            .expect("insert session");
        connection
            .execute(
                "INSERT INTO search_trails_rows(trail_id,session_id,initial_query,search_engine,reformulation_count,visit_count,landing_url,landing_domain,first_visit_ms,last_visit_ms,max_depth,queries_json,profile_id,computed_at) VALUES ('trail-1','session-1','sqlite wal','google',1,4,'https://sqlite.org/wal.html','sqlite.org',?1,?2,2,'[\"sqlite wal\",\"sqlite checkpoint\"]','profile-a','')",
                params![base_visit_ms, base_visit_ms + 3_000],
            )
            .expect("insert trail");

        let pages = [
            (1_i64, "https://sqlite.org/wal.html", "WAL", "sqlite.org", "docs_page", base_visit_ms),
            (
                2_i64,
                "https://postgresql.org/docs/wal.html",
                "Postgres WAL",
                "postgresql.org",
                "docs_page",
                base_visit_ms + 1_000,
            ),
            (
                3_i64,
                "https://sqlite.org/wal.html",
                "WAL",
                "sqlite.org",
                "docs_page",
                base_visit_ms + 2_000,
            ),
            (
                4_i64,
                "https://postgresql.org/docs/wal.html",
                "Postgres WAL",
                "postgresql.org",
                "docs_page",
                base_visit_ms + 3_000,
            ),
        ];
        for (visit_id, url, title, domain, category, visit_time_ms) in pages {
            archive.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(?1,?2,?2,?3) ON CONFLICT(url) DO NOTHING",params![visit_id,url,domain]).unwrap();
            archive
                .execute(
                    "INSERT INTO urls(id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,page_id) VALUES (?1,?2,?3,0,'',0,'',1,0,(SELECT id FROM pages WHERE url=?2))",
                    params![visit_id, url, title],
                )
                .expect("insert url");
            archive
                .execute(
                    "INSERT INTO visits(id,url_id,visit_time_ms,source_profile_id,created_by_run_id,visit_time_iso) VALUES (?1,?1,?2,1,0,'')",
                    params![visit_id, visit_time_ms],
                )
                .expect("insert visit");
            connection
                .execute(
                    "INSERT INTO visit_derived_facts_rows(visit_id,canonical_url,registrable_domain,page_category,profile_id,computed_at) VALUES (?1,?2,?3,?4,'profile-a','')",
                    params![visit_id, url, domain, category],
                )
                .expect("insert derived fact");
            connection
                .execute(
                    "INSERT INTO search_trail_members_rows(trail_id,visit_id,ordinal,profile_id,role) VALUES ('trail-1',?1,?1,'profile-a','result')",
                    params![visit_id],
                )
                .expect("insert trail member");
        }

        connection
            .execute(
                "INSERT INTO domain_daily_rollups_rows(profile_id,date_key,registrable_domain,visit_count,domain_category,search_count,new_domain_visits,unique_urls) VALUES ('profile-a','2026-01-01','sqlite.org',3,'unknown',0,0,1),
('profile-a','2026-01-01','shared.dev',2,'unknown',0,0,1),
('profile-b','2026-01-01','apple.com',5,'unknown',0,0,1),
('profile-b','2026-01-01','shared.dev',1,'unknown',0,0,1)",
                (),
            )
            .expect("insert domain rollups");
        connection
            .execute(
                "INSERT INTO category_daily_rollups_rows(profile_id,date_key,domain_category,visit_count,unique_domains) VALUES ('profile-a','2026-01-01','docs',3,1),
('profile-a','2026-01-01','tools',2,1),
('profile-b','2026-01-01','news',3,1)",
                (),
            )
            .expect("insert category rollups");
        connection
            .execute(
                "INSERT INTO daily_summary_rollups_rows(profile_id,date_key,total_visits,total_searches,new_domains,unique_domains) VALUES ('profile-a','2026-01-01',5,0,0,1),
('profile-b','2026-01-01',6,0,0,1)",
                (),
            )
            .expect("insert daily totals");
        connection
    }

    #[test]
    fn path_prefix_extracts_first_segment() {
        assert_eq!(path_prefix("https://example.com/docs/rust"), "docs");
        assert_eq!(path_prefix("https://example.com"), "/");
    }

    #[test]
    fn compare_set_requires_alternation_and_multiple_pages() {
        let members = vec![
            CompareTrailMember {
                trail_id: "trail-1".to_string(),
                initial_query: "sqlite wal".to_string(),
                landing_url: Some("https://sqlite.org/wal.html".to_string()),
                canonical_url: "https://sqlite.org/wal.html".to_string(),
                url: "https://sqlite.org/wal.html".to_string(),
                title: Some("WAL".to_string()),
                registrable_domain: "sqlite.org".to_string(),
                page_category: "docs_page".to_string(),
                visit_time_ms: 10,
            },
            CompareTrailMember {
                trail_id: "trail-1".to_string(),
                initial_query: "sqlite wal".to_string(),
                landing_url: Some("https://sqlite.org/checkpoint.html".to_string()),
                canonical_url: "https://sqlite.org/checkpoint.html".to_string(),
                url: "https://sqlite.org/checkpoint.html".to_string(),
                title: Some("Checkpoint".to_string()),
                registrable_domain: "sqlite.org".to_string(),
                page_category: "docs_page".to_string(),
                visit_time_ms: 20,
            },
            CompareTrailMember {
                trail_id: "trail-1".to_string(),
                initial_query: "sqlite wal".to_string(),
                landing_url: Some("https://sqlite.org/wal.html".to_string()),
                canonical_url: "https://sqlite.org/wal.html".to_string(),
                url: "https://sqlite.org/wal.html".to_string(),
                title: Some("WAL".to_string()),
                registrable_domain: "sqlite.org".to_string(),
                page_category: "docs_page".to_string(),
                visit_time_ms: 30,
            },
            CompareTrailMember {
                trail_id: "trail-1".to_string(),
                initial_query: "sqlite wal".to_string(),
                landing_url: Some("https://sqlite.org/checkpoint.html".to_string()),
                canonical_url: "https://sqlite.org/checkpoint.html".to_string(),
                url: "https://sqlite.org/checkpoint.html".to_string(),
                title: Some("Checkpoint".to_string()),
                registrable_domain: "sqlite.org".to_string(),
                page_category: "docs_page".to_string(),
                visit_time_ms: 35,
            },
        ];
        let compare_sets = build_compare_sets_for_trail(&members);
        assert_eq!(compare_sets.len(), 1);
        assert_eq!(compare_sets[0].page_category, "docs_page");
        assert_eq!(compare_sets[0].pages.len(), 2);
        assert_eq!(compare_sets[0].pages[0].canonical_url, "https://sqlite.org/checkpoint.html");

        let not_enough_alternation = build_compare_sets_for_trail(&members[..2]);
        assert!(not_enough_alternation.is_empty());
    }

    #[test]
    fn compare_set_ignores_same_domain_without_sibling_path_variation() {
        let members = vec![
            CompareTrailMember {
                trail_id: "trail-1".to_string(),
                initial_query: "sqlite wal".to_string(),
                landing_url: Some("https://sqlite.org/wal.html".to_string()),
                canonical_url: "https://sqlite.org/wal.html".to_string(),
                url: "https://sqlite.org/wal.html".to_string(),
                title: Some("WAL".to_string()),
                registrable_domain: "sqlite.org".to_string(),
                page_category: "docs_page".to_string(),
                visit_time_ms: 10,
            },
            CompareTrailMember {
                trail_id: "trail-1".to_string(),
                initial_query: "sqlite wal".to_string(),
                landing_url: Some("https://sqlite.org/checkpoint.html".to_string()),
                canonical_url: "https://sqlite.org/checkpoint.html".to_string(),
                url: "https://sqlite.org/checkpoint.html".to_string(),
                title: Some("Checkpoint".to_string()),
                registrable_domain: "sqlite.org".to_string(),
                page_category: "docs_page".to_string(),
                visit_time_ms: 20,
            },
            CompareTrailMember {
                trail_id: "trail-1".to_string(),
                initial_query: "sqlite wal".to_string(),
                landing_url: Some("https://sqlite.org/wal.html".to_string()),
                canonical_url: "https://sqlite.org/wal.html".to_string(),
                url: "https://sqlite.org/wal.html".to_string(),
                title: Some("WAL".to_string()),
                registrable_domain: "sqlite.org".to_string(),
                page_category: "docs_page".to_string(),
                visit_time_ms: 40 * 60 * 1000,
            },
            CompareTrailMember {
                trail_id: "trail-1".to_string(),
                initial_query: "sqlite wal".to_string(),
                landing_url: Some("https://sqlite.org/checkpoint.html".to_string()),
                canonical_url: "https://sqlite.org/checkpoint.html".to_string(),
                url: "https://sqlite.org/checkpoint.html".to_string(),
                title: Some("Checkpoint".to_string()),
                registrable_domain: "sqlite.org".to_string(),
                page_category: "docs_page".to_string(),
                visit_time_ms: 60 * 60 * 1000,
            },
        ];

        let compare_sets = build_compare_sets_for_trail(&members);

        assert!(compare_sets.is_empty());
    }

    #[test]
    fn compare_set_detail_reads_its_trail_and_parses_errors() {
        let connection = seed_phase_four_connection();
        let direct_detail = {
            connection.read(move |session| {
                get_compare_set_detail_with_connection(
                    session,
                    &CompareSetDetailRequest {
                        compare_set_id: "compare:trail-1:docs_page".to_string(),
                        date_range: request(None).date_range,
                        profile_id: None,
                    },
                )
            })
        }
        .expect("connection-scoped detail");
        assert_eq!(direct_detail.compare_set.compare_set_id, "compare:trail-1:docs_page");
        assert_eq!(direct_detail.trail.trail_id, "trail-1");
        assert_eq!(
            direct_detail.session.as_ref().map(|session| session.session_id.as_str()),
            Some("session-1")
        );
        assert!(!direct_detail.recent_days.is_empty());

        let root = tempfile::tempdir().expect("temp root");
        let _paths = crate::config::project_paths_with_root(root.path());
        let detail = get_compare_set_detail(
            &crate::Vault::open_temp().unwrap(),
            &CompareSetDetailRequest {
                compare_set_id: "compare:trail-1:docs_page".to_string(),
                date_range: request(None).date_range,
                profile_id: None,
            },
        )
        .expect_err(
            "public wrapper should require initialized read-model tables in this unit test",
        );
        assert!(!detail.to_string().is_empty());

        let (trail_id, category) = parse_compare_set_id("compare:trail-1:docs_page").unwrap();
        assert_eq!((trail_id, category), ("trail-1", "docs_page"));
        assert!(parse_compare_set_id("trail-1:docs_page").is_err());
        assert!(parse_compare_set_id("compare:trail-only").is_err());

        let trail =
            { connection.read(move |session| load_trail_summary(session, "trail-1")) }.unwrap();
        assert_eq!(trail.queries, vec!["sqlite wal", "sqlite checkpoint"]);
        let session =
            { connection.read(move |session| load_session_summary(session, "session-1")) }.unwrap();
        assert!(session.is_deep_dive);
        assert!(
            { connection.read(move |session| load_trail_summary(session, "missing")) }.is_err()
        );
        assert!(
            { connection.read(move |session| load_session_summary(session, "missing")) }.is_err()
        );
    }

    #[test]
    fn browser_diff_read_model_groups_shared_exclusive_and_category_mix() {
        let connection = seed_phase_four_connection();
        let diff = {
            connection.read(move |session| {
                get_multi_browser_diff_with_connection(session, &request(None))
            })
        }
        .unwrap();

        assert_eq!(diff.profiles.len(), 2);
        assert_eq!(diff.profiles[0].profile_name, "Chrome Work");
        assert_eq!(diff.profiles[1].browser_family, "safari");
        assert_eq!(diff.shared_domains, vec!["shared.dev"]);
        assert_eq!(diff.exclusive_domains[0].registrable_domain, "apple.com");
        assert_eq!(diff.exclusive_domains[1].registrable_domain, "sqlite.org");

        let chrome_mix = diff
            .category_distributions
            .iter()
            .find(|profile| profile.profile_id == "profile-a")
            .expect("chrome category distribution");
        assert_eq!(chrome_mix.categories.len(), 2);
        assert!(chrome_mix.categories.iter().any(|entry| {
            entry.domain_category == "docs" && entry.visit_count == 3 && entry.share > 0.59
        }));
    }
}
