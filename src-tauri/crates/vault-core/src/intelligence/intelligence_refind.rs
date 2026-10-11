//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Refind and top-site Core Intelligence read models.
//!
//! ## Responsibilities
//! - Serve top-site and refind-page read models for route-level Core
//!   Intelligence surfaces.
//! - Build refind explanations and related-trail context from the persisted
//!   refind evidence payloads.
//! - Keep refind-specific SQL and serde decoding out of the rebuild owner.
//!
//! ## Not responsible for
//! - General `/intelligence` overview composition.
//! - Entity explanations for sessions, trails, habits, or compare sets.
//! - Recomputing refind scores during rebuilds.
//!
//! ## Dependencies
//! - Parent-module date helpers and `trail_summary_from_row`.
//! - `refind_pages`, `search_trails`, and `domain_daily_rollups` in the
//!   intelligence plane.
//!
//! ## Performance notes
//! - All `*_with_connection` helpers reuse an existing SQLite handle so staged
//!   overview loads and detail pages avoid reopening the intelligence plane.
//! - Refind detail only loads trails and days referenced by persisted evidence
//!   visit ids; it does not rescan all trails for a profile.

use super::site_days::{SiteDay, SiteDayCache, SiteDayScope, SiteDays};
use super::{date_range_bounds, display_name_for_domain, rfc3339_from_millis};
use crate::models::{
    ExplainRefindRequest, RefindExplanation, RefindPage, RefindPageDetail, RefindPageDetailRequest,
    RefindPagesRequest, RefindScoreFactor, TopSite, TopSitesRequest,
};
use anyhow::{Context, Result};
use vault_store::{OptionalRow, Row, Session, params_owned as params};

/// Returns the most-visited domains in the requested window for overview cards
/// and trusted exports.
pub fn get_top_sites(
    vault: &crate::Vault,
    request: &TopSitesRequest,
) -> Result<crate::models::IntelligencePage<TopSite>> {
    let cache = vault.site_days().clone();
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        get_top_sites_with_connection(connection, &cache, &request)
    })
}

/// Ranks the sites of the range's one shared site-day read (`site_days`), so another order or
/// the next page reads nothing.
pub(super) fn get_top_sites_with_connection(
    connection: &Session<'_>,
    cache: &SiteDayCache,
    request: &TopSitesRequest,
) -> Result<crate::models::IntelligencePage<TopSite>> {
    let profiles = if request.profile_ids.is_empty() {
        request.profile_id.iter().cloned().collect()
    } else {
        request.profile_ids.clone()
    };
    let days = cache.get(connection, &SiteDayScope::new(&request.date_range, profiles))?;
    rank_top_sites(&days, request)
}

/// One page of `request`'s order: visits that were not searches (a search result page is a
/// search, see Frequent searches), the days they fell on, or visits per such day; ties go to
/// the other measure, then the site's name.
pub(super) fn rank_top_sites(
    days: &SiteDays,
    request: &TopSitesRequest,
) -> Result<crate::models::IntelligencePage<TopSite>> {
    let limit = request.limit.unwrap_or(20).clamp(1, 1000) as usize;
    let cursor = super::paging::cursor::<(f64, f64, String)>(request.cursor.as_deref())?;
    let key = |site: &SiteDay| {
        let visits = site.page_visits as f64;
        let days = site.page_days as f64;
        let average = visits / days;
        match request.sort_by.as_deref() {
            Some("unique_days") => (days, average),
            Some("average_daily_visits") => (average, visits),
            _ => (visits, days),
        }
    };
    let mut ranked = days
        .sites
        .iter()
        .filter(|site| site.page_days > 0)
        .map(|site| (key(site), site))
        .filter(|((first, second), site)| {
            cursor.as_ref().is_none_or(|(after_first, after_second, after_site)| {
                first < after_first
                    || (first == after_first
                        && (second < after_second
                            || (second == after_second && site.domain > *after_site)))
            })
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(a, a_site), (b, b_site)| {
        b.0.total_cmp(&a.0)
            .then(b.1.total_cmp(&a.1))
            .then_with(|| a_site.domain.cmp(&b_site.domain))
    });
    ranked.truncate(limit + 1);
    let rows = ranked
        .into_iter()
        .map(|((first, second), site)| {
            (
                TopSite {
                    display_name: display_name_for_domain(&site.domain),
                    registrable_domain: site.domain.clone(),
                    domain_category: site.category.clone().unwrap_or_default(),
                    visit_count: site.page_visits,
                    unique_days: site.page_days,
                    average_daily_visits: (site.page_visits as f64 / site.page_days as f64) as f32,
                    unique_urls: site.page_urls,
                },
                (first, second, site.domain.clone()),
            )
        })
        .collect();
    super::paging::finish(rows, limit)
}

/// Returns the highest-scoring refind pages in the requested window.
pub fn get_refind_pages(
    vault: &crate::Vault,
    request: &RefindPagesRequest,
) -> Result<crate::models::IntelligencePage<RefindPage>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_refind_pages_with_connection(connection, request)
    })
}

/// Reuses an existing SQLite handle for refind-page reads so overview and
/// export surfaces can share one intelligence transaction.
pub(super) fn get_refind_pages_with_connection(
    connection: &Session<'_>,
    request: &RefindPagesRequest,
) -> Result<crate::models::IntelligencePage<RefindPage>> {
    let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
    let limit = request.limit.unwrap_or(20).clamp(1, 1000) as usize;
    let cursor = super::paging::cursor::<(f64, i64, String, String)>(request.cursor.as_deref())?;
    // Through the `refind_pages` view Turso sorts the whole generation on every page and builds
    // a temporary index over it for the per-URL lookups. The rows table is read in list order
    // through idx_refind_pages_score and seeks to the cursor, which must be in the bounded form
    // and only present after the first page; the lookups use idx_refind_pages_canonical.
    let after = if cursor.is_some() {
        "AND p.refind_score<=?4 AND (p.refind_score<?4 OR (p.last_seen_ms<=?5 AND (p.last_seen_ms<?5 OR (p.canonical_url>=?6 AND (p.canonical_url>?6 OR p.profile_id>?7)))))"
    } else {
        ""
    };
    let other = "refind_pages_rows other INDEXED BY idx_refind_pages_canonical WHERE other.canonical_url=p.canonical_url
        AND other.generation IN (SELECT aggregates FROM intelligence_generations) AND other.last_seen_ms>=?2 AND other.first_seen_ms<?3";
    let rows=connection.query(
        &format!("SELECT p.profile_id,p.canonical_url,p.url,p.title,p.registrable_domain,p.cross_day_count,p.trail_count,
            p.search_arrival_count,p.typed_revisit_count,p.refind_score,p.first_seen_ms,p.last_seen_ms,
            CASE WHEN ?1 IS NOT NULL THEN 1 ELSE (SELECT COUNT(*) FROM {other} LIMIT 1 OFFSET 0) END
         FROM refind_pages_rows p INDEXED BY idx_refind_pages_score
         WHERE p.generation IN (SELECT aggregates FROM intelligence_generations)
           AND (?1 IS NULL OR p.profile_id=?1) AND p.last_seen_ms>=?2 AND p.first_seen_ms<?3
           AND (?1 IS NOT NULL OR NOT EXISTS(SELECT 1 FROM {other}
              AND (other.refind_score>p.refind_score OR (other.refind_score=p.refind_score AND (other.last_seen_ms>p.last_seen_ms OR (other.last_seen_ms=p.last_seen_ms AND other.profile_id<p.profile_id)))) LIMIT 1 OFFSET 0))
           {after}
         ORDER BY p.refind_score DESC,p.last_seen_ms DESC,p.canonical_url ASC,p.profile_id ASC LIMIT ?8"),
         params![request.profile_id.as_deref(),start_ms,end_ms,cursor.as_ref().map(|c|c.0),cursor.as_ref().map(|c|c.1),cursor.as_ref().map(|c|c.2.as_str()),cursor.as_ref().map(|c|c.3.as_str()),(limit+1) as i64],|r|Ok((refind_page_from_row(r)?,(r.get::<f64>(9)?,r.get::<i64>(11)?,r.get::<String>(1)?,r.get::<String>(0)?))))?;
    super::paging::finish(rows, limit)
}

/// Re-found pages opened in `range` (one day, for the day page), highest score first, one row
/// per page with its best profile. `get_refind_pages` takes every re-found page whose first and
/// last visit span the range, opened in it or not; this one starts from the days each was opened
/// on (`refind_days`), as the "Pages reopened" count does.
pub(super) fn get_opened_refind_pages_with_connection(
    connection: &Session<'_>,
    range: &crate::models::DateRange,
    profile_id: Option<&str>,
    limit: usize,
) -> Result<crate::models::IntelligencePage<RefindPage>> {
    // The pages opened on the range's days (`refind_days`), the ones "Pages reopened" counts.
    let opened = super::refind_days::opened(connection, range, profile_id)?;
    let mut rows = Vec::with_capacity(opened.len());
    for (profile, key) in opened {
        rows.extend(connection.query(
            "SELECT p.profile_id,p.canonical_url,p.url,p.title,p.registrable_domain,p.cross_day_count,
                    p.trail_count,p.search_arrival_count,p.typed_revisit_count,p.refind_score,
                    p.first_seen_ms,p.last_seen_ms,1
             FROM intelligence_generations g
             CROSS JOIN refind_pages_rows p INDEXED BY idx_refind_pages_key
               ON p.page_key=?2 AND p.generation=g.aggregates AND p.profile_id=g.profile_id
             WHERE g.profile_id=?1",
            params![profile.as_str(), key],
            refind_page_from_row,
        )?);
    }
    rows.sort_by(|a, b| {
        b.refind_score
            .total_cmp(&a.refind_score)
            .then_with(|| b.last_seen_at.cmp(&a.last_seen_at))
            .then_with(|| a.canonical_url.cmp(&b.canonical_url))
            .then_with(|| a.profile_id.cmp(&b.profile_id))
    });
    let mut items: Vec<RefindPage> = Vec::new();
    for page in rows {
        match items.iter_mut().find(|kept| kept.canonical_url == page.canonical_url) {
            Some(kept) => kept.profile_count += 1,
            None => items.push(page),
        }
    }
    items.truncate(limit);
    Ok(crate::models::IntelligencePage { items, next_cursor: None })
}

/// Loads one refind page plus the evidence-derived days and related trails that
/// justify why the page is considered worth resurfacing.
pub fn get_refind_page_detail(
    vault: &crate::Vault,
    request: &RefindPageDetailRequest,
) -> Result<RefindPageDetail> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
        let page = load_refind_page_detail_row(connection, request)?;
        let profile_id = page.profile_id.clone();
        let explanation = build_refind_explanation(
            connection,
            &page.canonical_url,
            Some(&profile_id),
            request.visits_cursor.as_deref(),
            request.page_size.unwrap_or(50).clamp(1, 1000) as usize,
        )?;
        let recent_days = super::refind_evidence::days(
            connection,
            &profile_id,
            &page.canonical_url,
            start_ms,
            end_ms,
        )?;
        let related_trails = super::refind_evidence::trails(
            connection,
            &profile_id,
            &page.canonical_url,
            start_ms,
            end_ms,
            request.trails_cursor.as_deref(),
            request.page_size.unwrap_or(8).clamp(1, 1000) as usize,
        )?;

        Ok(RefindPageDetail { page, explanation, recent_days, related_trails })
    })
}

/// Returns only the persisted refind scoring evidence for one canonical URL.
pub fn explain_refind(
    vault: &crate::Vault,
    request: &ExplainRefindRequest,
) -> Result<RefindExplanation> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        build_refind_explanation(
            connection,
            &request.canonical_url,
            request.profile_id.as_deref(),
            request.cursor.as_deref(),
            request.page_size.unwrap_or(50).clamp(1, 1000) as usize,
        )
    })
}

/// Decodes the persisted refind evidence payload for one canonical URL instead
/// of recomputing the score from archive facts at request time. With a
/// profile, the evidence is that profile's row (the one the caller shows);
/// without one, the best-scoring profile's.
pub(super) fn build_refind_explanation(
    connection: &Session<'_>,
    canonical_url: &str,
    profile_id: Option<&str>,
    cursor: Option<&str>,
    limit: usize,
) -> Result<RefindExplanation> {
    let (canonical_url, refind_score, profile_id, counts) = connection
        .query_one(
            "SELECT canonical_url, refind_score, profile_id,cross_day_count,trail_count,search_arrival_count,typed_revisit_count
             FROM refind_pages
             WHERE canonical_url = ?1
               AND (?2 IS NULL OR profile_id = ?2)
             ORDER BY refind_score DESC,last_seen_ms DESC,profile_id
             LIMIT 1",
            params![canonical_url, profile_id],
            |row| Ok((row.get::<String>(0)?, row.get::<f64>(1)? as f32, row.get::<String>(2)?,[row.get::<i64>(3)?,row.get(4)?,row.get(5)?,row.get(6)?])),
        )
        .optional()?
        .with_context(|| format!("refind page {canonical_url} was not found"))?;
    let factors = super::intelligence_structural_aggregates::refind_signals(
        counts[0], counts[1], counts[2], counts[3],
    )
    .into_iter()
    .map(|(signal, raw, weight)| RefindScoreFactor {
        signal: signal.into(),
        raw_value: raw as f32,
        weight,
        contribution: raw as f32 * weight,
    })
    .collect();
    let visits =
        super::refind_evidence::visits(connection, &profile_id, &canonical_url, cursor, limit)?;
    Ok(RefindExplanation {
        canonical_url,
        profile_id,
        refind_score,
        factors,
        visit_ids: visits.items,
        visit_ids_next_cursor: visits.next_cursor,
    })
}

/// Resolves the persisted refind row that anchors the detail page and keeps the
/// list/detail filtering rules identical for profile and date-range scope.
fn load_refind_page_detail_row(
    connection: &Session<'_>,
    request: &RefindPageDetailRequest,
) -> Result<RefindPage> {
    let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
    connection
        .query_one(
            "SELECT profile_id, canonical_url, url, title, registrable_domain, cross_day_count, trail_count,
                    search_arrival_count, typed_revisit_count, refind_score, first_seen_ms, last_seen_ms,
                    (SELECT COUNT(*) FROM refind_pages AS other
                     WHERE other.canonical_url = refind_pages.canonical_url
                       AND other.last_seen_ms >= ?3
                       AND other.first_seen_ms < ?4 LIMIT 1 OFFSET 0)
             FROM refind_pages
             WHERE canonical_url = ?1
               AND (?2 IS NULL OR profile_id = ?2)
               AND last_seen_ms >= ?3
               AND first_seen_ms < ?4
             ORDER BY refind_score DESC, last_seen_ms DESC
             LIMIT 1",
            params![request.canonical_url, request.profile_id.as_deref(), start_ms, end_ms],
            refind_page_from_row,
        )
        .optional()?
        .with_context(|| format!("refind page {} was not found", request.canonical_url))
}

/// Rehydrates a `RefindPage` from a shared SQL projection where some callers
/// prepend `profile_id` ahead of the canonical refind columns.
fn refind_page_from_row(row: &Row) -> vault_store::Result<RefindPage> {
    Ok(RefindPage {
        profile_id: row.get(0)?,
        canonical_url: row.get(1)?,
        url: row.get(2)?,
        title: row.get(3)?,
        registrable_domain: row.get(4)?,
        cross_day_count: row.get(5)?,
        trail_count: row.get(6)?,
        search_arrival_count: row.get(7)?,
        typed_revisit_count: row.get(8)?,
        refind_score: row.get::<f64>(9)? as f32,
        first_seen_at: rfc3339_from_millis(row.get(10)?),
        last_seen_at: rfc3339_from_millis(row.get(11)?),
        profile_count: row.get(12)?,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn refind_detail_helpers_skip_empty_or_unlinked_visit_sets() {
        for key in [None, Some([28; 32])] {
            let vault = crate::Vault::open_temp_with_key(key).unwrap();
            vault.write(vault_store::Db::Archive,vault_store::Priority::Batch,|s| {
                s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','chrome:Default','')",())?;
                s.execute("INSERT INTO pages(id,url,canonical_url,registrable_domain) VALUES(1,'https://example.com/','https://example.com/','example.com')",())?;
                s.execute("INSERT INTO urls(id,page_id,url,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) VALUES(1,1,'https://example.com/',1,0,1711929600000,'',1711929600000,'')",())?;
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) VALUES(1,1,1,0,1711929600000,'')",())?;Ok(())
            }).unwrap();
            vault.write(vault_store::Db::Intelligence,vault_store::Priority::Batch,|s| {
                s.execute("INSERT INTO intelligence_generations(profile_id) VALUES('chrome:Default')",())?;
                s.execute("INSERT INTO visit_derived_facts_rows(visit_id,profile_id,registrable_domain,canonical_url,computed_at) VALUES(1,'chrome:Default','example.com','https://example.com/','')",())?;Ok(())
            }).unwrap();
            vault
                .read(vault_store::Db::Intelligence, |s| {
                    assert!(
                        super::super::refind_evidence::days(s, "chrome:Default", "", 0, 1)?
                            .is_empty()
                    );
                    assert!(
                        super::super::refind_evidence::trails(
                            s,
                            "chrome:Default",
                            "",
                            0,
                            1,
                            None,
                            8
                        )?
                        .is_empty()
                    );
                    assert!(
                        super::super::refind_evidence::trails(
                            s,
                            "chrome:Default",
                            "https://example.com/",
                            0,
                            1712016000000,
                            None,
                            8
                        )?
                        .is_empty()
                    );
                    Ok(())
                })
                .unwrap();
        }
    }
}
