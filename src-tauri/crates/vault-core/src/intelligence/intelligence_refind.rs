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
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_top_sites_with_connection(connection, request)
    })
}

/// Reuses an existing SQLite handle for top-site reads so overview loaders can
/// compose multiple intelligence widgets without reopening the database.
pub(super) fn get_top_sites_with_connection(
    connection: &Session<'_>,
    request: &TopSitesRequest,
) -> Result<crate::models::IntelligencePage<TopSite>> {
    let limit = request.limit.unwrap_or(20).clamp(1, 1000) as usize;
    let cursor = super::paging::cursor::<(f64, f64, String)>(request.cursor.as_deref())?;
    let visits = "SUM(visit_count)";
    let days = "COUNT(DISTINCT date_key)";
    let average = "CAST(SUM(visit_count) AS REAL)/COUNT(DISTINCT date_key)";
    let (primary, secondary) = match request.sort_by.as_deref() {
        Some("unique_days") => (days, average),
        Some("average_daily_visits") => (average, visits),
        _ => (visits, days),
    };
    let profiles: Vec<&str> = if request.profile_ids.is_empty() {
        request.profile_id.as_deref().into_iter().collect()
    } else {
        request.profile_ids.iter().map(String::as_str).collect()
    };
    // Profiles bind after the seven fixed parameters.
    let profiles_clause = if profiles.is_empty() {
        "?1 IS NULL".to_owned()
    } else {
        let marks: Vec<String> = (0..profiles.len()).map(|i| format!("?{}", i + 8)).collect();
        format!("(?1 IS NULL AND profile_id IN ({}))", marks.join(","))
    };
    let sql=format!("SELECT registrable_domain,MIN(domain_category),SUM(visit_count),COUNT(DISTINCT date_key),
         CAST(SUM(visit_count) AS REAL)/COUNT(DISTINCT date_key),SUM(unique_urls),{primary},{secondary}
         FROM domain_daily_rollups WHERE {profiles_clause} AND date_key>=?2 AND date_key<=?3
         GROUP BY registrable_domain
         HAVING ?5 IS NULL OR {primary}<?5 OR ({primary}=?5 AND ({secondary}<?6 OR ({secondary}=?6 AND registrable_domain>?7)))
         ORDER BY {primary} DESC,{secondary} DESC,registrable_domain ASC LIMIT ?4");
    let rows = connection.query(
        &sql,
        {
            let mut values: Vec<vault_store::Value> = vec![
                vault_store::Value::Null,
                request.date_range.start.clone().into(),
                request.date_range.end.clone().into(),
                ((limit + 1) as i64).into(),
                cursor.as_ref().map(|c| c.0).into(),
                cursor.as_ref().map(|c| c.1).into(),
                cursor.as_ref().map(|c| c.2.clone()).into(),
            ];
            values.extend(profiles.iter().map(|id| vault_store::Value::from(id.to_string())));
            values
        },
        |r| {
            let domain = r.get::<String>(0)?;
            Ok((
                TopSite {
                    display_name: display_name_for_domain(&domain),
                    registrable_domain: domain.clone(),
                    domain_category: r.get(1)?,
                    visit_count: r.get(2)?,
                    unique_days: r.get(3)?,
                    average_daily_visits: r.get::<f64>(4)? as f32,
                    unique_urls: r.get(5)?,
                },
                (r.get::<f64>(6)?, r.get::<f64>(7)?, domain),
            ))
        },
    )?;
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
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
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

/// Loads one refind page plus the evidence-derived days and related trails that
/// justify why the page is considered worth resurfacing.
pub fn get_refind_page_detail(
    vault: &crate::Vault,
    request: &RefindPageDetailRequest,
) -> Result<RefindPageDetail> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
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
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
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
