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

use super::{
    date_range_bounds, display_name_for_domain, ensure_core_intelligence_schema,
    intelligence_sessions::trail_summary_from_row, rfc3339_from_millis,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{
        AppConfig, ExplainRefindRequest, RefindExplanation, RefindPage, RefindPageDetail,
        RefindPageDetailRequest, RefindPagesRequest, RefindScoreFactor, TopSite, TopSitesRequest,
    },
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};

/// Returns the most-visited domains in the requested window for overview cards
/// and trusted exports.
pub fn get_top_sites(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TopSitesRequest,
) -> Result<Vec<TopSite>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_top_sites_with_connection(&connection, request)
}

/// Reuses an existing SQLite handle for top-site reads so overview loaders can
/// compose multiple intelligence widgets without reopening the database.
pub(super) fn get_top_sites_with_connection(
    connection: &Connection,
    request: &TopSitesRequest,
) -> Result<Vec<TopSite>> {
    let order_clause = match request.sort_by.as_deref() {
        Some("unique_days") => {
            "unique_days DESC, average_daily_visits DESC, registrable_domain ASC"
        }
        Some("average_daily_visits") => {
            "average_daily_visits DESC, visit_count DESC, registrable_domain ASC"
        }
        _ => "visit_count DESC, unique_days DESC, registrable_domain ASC",
    };
    let sql = format!(
        "SELECT registrable_domain,
                MIN(domain_category),
                SUM(visit_count) AS visit_count,
                COUNT(DISTINCT date_key) AS unique_days,
                CAST(SUM(visit_count) AS REAL) / COUNT(DISTINCT date_key) AS average_daily_visits,
                SUM(unique_urls) AS unique_urls
         FROM domain_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY registrable_domain
         ORDER BY {order_clause}
         LIMIT ?4"
    );
    let mut statement = connection.prepare(&sql)?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.date_range.start,
                request.date_range.end,
                request.limit.unwrap_or(20).max(1) as i64
            ],
            |row| {
                let domain: String = row.get(0)?;
                Ok(TopSite {
                    display_name: display_name_for_domain(&domain),
                    registrable_domain: domain,
                    domain_category: row.get(1)?,
                    visit_count: row.get(2)?,
                    unique_days: row.get(3)?,
                    average_daily_visits: row.get::<_, f32>(4)?,
                    unique_urls: row.get(5)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Returns the highest-scoring refind pages in the requested window.
pub fn get_refind_pages(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &RefindPagesRequest,
) -> Result<Vec<RefindPage>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_refind_pages_with_connection(&connection, request)
}

/// Reuses an existing SQLite handle for refind-page reads so overview and
/// export surfaces can share one intelligence transaction.
pub(super) fn get_refind_pages_with_connection(
    connection: &Connection,
    request: &RefindPagesRequest,
) -> Result<Vec<RefindPage>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT canonical_url, url, title, registrable_domain, cross_day_count, trail_count,
                search_arrival_count, typed_revisit_count, refind_score, first_seen_ms, last_seen_ms
         FROM refind_pages
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3
         ORDER BY refind_score DESC, last_seen_ms DESC
         LIMIT ?4",
    )?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.limit.unwrap_or(20).max(1) as i64
            ],
            |row| refind_page_from_row_with_offset(row, 0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Loads one refind page plus the evidence-derived days and related trails that
/// justify why the page is considered worth resurfacing.
pub fn get_refind_page_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &RefindPageDetailRequest,
) -> Result<RefindPageDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let (profile_id, page) = load_refind_page_detail_row(&connection, request)?;
    let explanation = build_refind_explanation(&connection, &page.canonical_url)?;
    let recent_days = load_refind_recent_days(&connection, &explanation.visit_ids)?;
    let related_trails = load_refind_related_trails(
        &connection,
        &profile_id,
        &explanation.visit_ids,
        start_ms,
        end_ms,
    )?;

    Ok(RefindPageDetail { page, explanation, recent_days, related_trails })
}

/// Returns only the persisted refind scoring evidence for one canonical URL.
pub fn explain_refind(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ExplainRefindRequest,
) -> Result<RefindExplanation> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    build_refind_explanation(&connection, &request.canonical_url)
}

/// Decodes the persisted refind evidence payload for one canonical URL instead
/// of recomputing the score from archive facts at request time.
pub(super) fn build_refind_explanation(
    connection: &Connection,
    canonical_url: &str,
) -> Result<RefindExplanation> {
    let (canonical_url, refind_score, evidence_json) = connection
        .query_row(
            "SELECT canonical_url, refind_score, evidence_json
             FROM refind_pages
             WHERE canonical_url = ?1
             ORDER BY refind_score DESC
             LIMIT 1",
            [canonical_url],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?, row.get::<_, String>(2)?)),
        )
        .optional()?
        .with_context(|| format!("refind page {canonical_url} was not found"))?;
    let evidence: serde_json::Value = serde_json::from_str(&evidence_json).unwrap_or_default();
    let factors = evidence
        .get("factors")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|factor| {
            Some(RefindScoreFactor {
                signal: factor.get("signal")?.as_str()?.to_string(),
                raw_value: factor.get("rawValue")?.as_f64().unwrap_or_default() as f32,
                weight: factor.get("weight")?.as_f64().unwrap_or_default() as f32,
                contribution: factor.get("contribution")?.as_f64().unwrap_or_default() as f32,
            })
        })
        .collect::<Vec<_>>();
    let visit_ids = evidence
        .get("visitIds")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_i64())
        .collect::<Vec<_>>();
    Ok(RefindExplanation { canonical_url, refind_score, factors, visit_ids })
}

/// Resolves the persisted refind row that anchors the detail page and keeps the
/// list/detail filtering rules identical for profile and date-range scope.
fn load_refind_page_detail_row(
    connection: &Connection,
    request: &RefindPageDetailRequest,
) -> Result<(String, RefindPage)> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    connection
        .query_row(
            "SELECT profile_id, canonical_url, url, title, registrable_domain, cross_day_count, trail_count,
                    search_arrival_count, typed_revisit_count, refind_score, first_seen_ms, last_seen_ms
             FROM refind_pages
             WHERE canonical_url = ?1
               AND (?2 IS NULL OR profile_id = ?2)
               AND last_seen_ms >= ?3
               AND first_seen_ms < ?4
             ORDER BY refind_score DESC, last_seen_ms DESC
             LIMIT 1",
            params![request.canonical_url, request.profile_id.as_deref(), start_ms, end_ms],
            |row| Ok((row.get::<_, String>(0)?, refind_page_from_row_with_offset(row, 1)?)),
        )
        .optional()?
        .with_context(|| format!("refind page {} was not found", request.canonical_url))
}

/// Collapses evidence visit ids into distinct local date keys so the detail
/// page can show why the page keeps resurfacing across separate days.
fn load_refind_recent_days(connection: &Connection, visit_ids: &[i64]) -> Result<Vec<String>> {
    if visit_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = std::iter::repeat_n("?", visit_ids.len()).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT DISTINCT strftime('%Y-%m-%d', datetime(visit_time_ms / 1000, 'unixepoch', 'localtime'))
         FROM archive.visits
         WHERE id IN ({placeholders})
         ORDER BY 1 DESC"
    );
    let mut statement = connection.prepare(&sql)?;
    statement
        .query_map(
            rusqlite::params_from_iter(visit_ids.iter().map(|value| value as &dyn rusqlite::ToSql)),
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Loads only the search trails touched by the refind evidence ids, which keeps
/// the detail page bounded even when the profile has a large trail history.
fn load_refind_related_trails(
    connection: &Connection,
    profile_id: &str,
    visit_ids: &[i64],
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<crate::models::TrailSummary>> {
    if visit_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = std::iter::repeat_n("?", visit_ids.len()).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT DISTINCT visit_derived_facts.trail_id
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         WHERE visit_derived_facts.profile_id = ?
           AND visit_derived_facts.visit_id IN ({placeholders})
           AND visit_derived_facts.trail_id IS NOT NULL
           AND visits.visit_time_ms >= ?
           AND visits.visit_time_ms < ?"
    );
    let mut trail_id_statement = connection.prepare(&sql)?;
    let params = std::iter::once(&profile_id as &dyn rusqlite::ToSql)
        .chain(visit_ids.iter().map(|value| value as &dyn rusqlite::ToSql))
        .chain(std::iter::once(&start_ms as &dyn rusqlite::ToSql))
        .chain(std::iter::once(&end_ms as &dyn rusqlite::ToSql));
    let trail_ids = trail_id_statement
        .query_map(rusqlite::params_from_iter(params), |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if trail_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = std::iter::repeat_n("?", trail_ids.len()).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE trail_id IN ({placeholders})
         ORDER BY last_visit_ms DESC, trail_id DESC"
    );
    let mut statement = connection.prepare(&sql)?;
    statement
        .query_map(
            rusqlite::params_from_iter(trail_ids.iter().map(|value| value as &dyn rusqlite::ToSql)),
            trail_summary_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map(|items| items.into_iter().take(8).collect::<Vec<_>>())
        .map_err(Into::into)
}

/// Rehydrates a `RefindPage` from a shared SQL projection where some callers
/// prepend `profile_id` ahead of the canonical refind columns.
fn refind_page_from_row_with_offset(row: &Row<'_>, offset: usize) -> rusqlite::Result<RefindPage> {
    Ok(RefindPage {
        canonical_url: row.get(offset)?,
        url: row.get(offset + 1)?,
        title: row.get(offset + 2)?,
        registrable_domain: row.get(offset + 3)?,
        cross_day_count: row.get(offset + 4)?,
        trail_count: row.get(offset + 5)?,
        search_arrival_count: row.get(offset + 6)?,
        typed_revisit_count: row.get(offset + 7)?,
        refind_score: row.get(offset + 8)?,
        first_seen_at: rfc3339_from_millis(row.get(offset + 9)?),
        last_seen_at: rfc3339_from_millis(row.get(offset + 10)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refind_detail_helpers_skip_empty_or_unlinked_visit_sets() {
        let connection = Connection::open_in_memory().expect("memory db");
        assert!(load_refind_recent_days(&connection, &[]).expect("empty recent days").is_empty());
        assert!(
            load_refind_related_trails(&connection, "chrome:Default", &[], 0, 1)
                .expect("empty related trails")
                .is_empty()
        );

        connection
            .execute_batch(
                "
                ATTACH DATABASE ':memory:' AS archive;
                CREATE TABLE archive.visits (
                  id INTEGER PRIMARY KEY,
                  visit_time_ms INTEGER NOT NULL
                );
                CREATE TABLE visit_derived_facts (
                  profile_id TEXT NOT NULL,
                  visit_id INTEGER NOT NULL,
                  trail_id TEXT
                );
                CREATE TABLE search_trails (
                  trail_id TEXT PRIMARY KEY,
                  session_id TEXT,
                  initial_query TEXT NOT NULL,
                  search_engine TEXT NOT NULL,
                  reformulation_count INTEGER NOT NULL,
                  visit_count INTEGER NOT NULL,
                  landing_url TEXT,
                  landing_domain TEXT,
                  first_visit_ms INTEGER NOT NULL,
                  last_visit_ms INTEGER NOT NULL,
                  max_depth INTEGER NOT NULL,
                  queries_json TEXT NOT NULL
                );
                INSERT INTO archive.visits (id, visit_time_ms) VALUES (1, 1711929600000);
                INSERT INTO visit_derived_facts (profile_id, visit_id, trail_id)
                VALUES ('chrome:Default', 1, NULL);
                ",
            )
            .expect("schema");
        assert!(
            load_refind_related_trails(&connection, "chrome:Default", &[1], 0, 1712016000000)
                .expect("unlinked related trails")
                .is_empty()
        );
    }
}
