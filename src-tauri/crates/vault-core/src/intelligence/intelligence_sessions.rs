//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Session and trail Core Intelligence read models.
//!
//! ## Responsibilities
//! - Serve session list/detail and trail list/detail queries for route-level
//!   Core Intelligence and Explorer-linked surfaces.
//! - Keep session/trail row decoding and session-local member loading out of
//!   the parent intelligence module.
//! - Reuse one consistent trail-summary decoder across refind and query-family
//!   detail reads.
//!
//! ## Not responsible for
//! - Rebuilding sessions or trails.
//! - Search-query, query-family, or domain-deep-dive reads.
//! - Export payload formatting or explainability surfaces.
//!
//! ## Dependencies
//! - Parent-module date-range helpers and shared archive connection setup.
//! - `sessions`, `search_trails`, `search_trail_members`, and
//!   `visit_derived_facts` in the intelligence plane plus archive joins.
//!
//! ## Performance notes
//! - List reads stay bounded by the requested page size.
//! - Detail reads only load rows linked to one `session_id` or `trail_id`;
//!   they do not scan unrelated sessions/trails in the same date range.

use super::date_range_bounds;
use crate::models::{
    PagedDateRangeRequest, SearchTrailQueryRequest, SessionListResult, SessionSummary,
    TrailListResult, TrailSummary,
};
use anyhow::Result;
use vault_store::{Row, params_owned as params};

/// Returns one page of deterministic sessions for the requested scope and date
/// range.
pub fn get_sessions(
    vault: &crate::Vault,
    request: &PagedDateRangeRequest,
) -> Result<SessionListResult> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence,move |connection| {
let request=&request;

    let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
    // Exact interval overlap includes sessions spanning any number of days.
    let total: i64 = connection.query_one(
        "SELECT COUNT(*)
         FROM sessions
         WHERE profile_id IN (
                 SELECT profile_key FROM archive.source_profiles
                 WHERE profile_key IS NOT NULL AND (?1 IS NULL OR profile_key = ?1))
           AND first_visit_ms < ?3
           AND last_visit_ms >= ?2",
        params![request.profile_id.as_deref(), start_ms, end_ms],
        |row| Ok(row.get(0)?),
    )?;
    let limit=request.page_size.clamp(1,1000) as usize;
    // A cursor at or past the window's end excludes nothing; any other cursor replaces the end.
    let cursor=super::paging::cursor::<(i64,String)>(request.cursor.as_deref())?.filter(|c|c.0<end_ms);
    let upper=if cursor.is_some() {"r.first_visit_ms<=?4 AND (r.first_visit_ms<?4 OR r.session_id<?5)"} else {"r.first_visit_ms<?3"};
    // Through the `sessions` view Turso reads the whole generation and sorts it on every page.
    // The rows table, walked through idx_sessions_recent, is read in list order and seeks to the
    // cursor; it needs the bounded cursor form above, with no `?4 IS NULL OR` escape.
    let mut statement = connection.prepare(&format!(
        "SELECT r.session_id, r.first_visit_ms, r.last_visit_ms, r.visit_count, r.search_count, r.domain_count, r.is_deep_dive, r.auto_title
         FROM sessions_rows r INDEXED BY idx_sessions_recent
         WHERE r.generation IN (SELECT navigation FROM intelligence_generations)
           AND r.profile_id IN (
                 SELECT profile_key FROM archive.source_profiles
                 WHERE profile_key IS NOT NULL AND (?1 IS NULL OR profile_key = ?1))
           AND r.last_visit_ms >= ?2
           AND {upper}
         ORDER BY r.first_visit_ms DESC, r.session_id DESC LIMIT ?6"
    ))?;
    let sessions = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                cursor.as_ref().map(|c|c.0),cursor.as_ref().map(|c|c.1.as_str()),
                (limit+1) as i64
            ],
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
        )?
        .collect::<vault_store::Result<Vec<_>>>()?;
    let page=super::paging::finish(sessions.into_iter().map(|row| {let key=(row.first_visit_ms,row.session_id.clone());(row,key)}).collect(),limit)?;
    Ok(SessionListResult {sessions:page.items,total,next_cursor:page.next_cursor,page_size:limit as u32})

})
}

pub use super::session_details::{get_session_detail, get_trail_detail};

/// Returns one page of the search trails begun in the requested range, for the scope and engine
/// filter. A trail counts where its search was made: counting every trail that touched the range
/// needs a bound on how far back one can begin, and without it the count read every trail of the
/// build (0.45 s at 14.4M visits for any range).
pub fn get_search_trails(
    vault: &crate::Vault,
    request: &SearchTrailQueryRequest,
) -> Result<TrailListResult> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence,move |connection| {
let request=&request;

    let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
    let total: i64 = connection.query_one(
        "SELECT COUNT(*)
         FROM search_trails_rows r INDEXED BY idx_search_trails_recent
         WHERE r.first_visit_ms >= ?3
           AND r.first_visit_ms < ?4
           AND r.generation IN (SELECT navigation FROM intelligence_generations)
           AND (?1 IS NULL OR r.profile_id = ?1)
           AND (?2 IS NULL OR r.search_engine = ?2)",
        params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
        |row| Ok(row.get(0)?),
    )?;
    let limit=request.page_size.clamp(1,1000) as usize;
    // Same shape as `get_sessions`: the rows table in list order, seeking to the cursor.
    let cursor=super::paging::cursor::<(i64,String)>(request.cursor.as_deref())?.filter(|c|c.0<end_ms);
    let upper=if cursor.is_some() {"r.first_visit_ms<=?5 AND (r.first_visit_ms<?5 OR r.trail_id<?6)"} else {"r.first_visit_ms<?4"};
    let mut statement = connection.prepare(&format!(
        "SELECT r.trail_id, r.session_id, r.initial_query, r.search_engine, r.reformulation_count, r.visit_count,
                r.landing_url, r.landing_domain, r.first_visit_ms, r.last_visit_ms, r.max_depth, r.queries_json
         FROM search_trails_rows r INDEXED BY idx_search_trails_recent
         WHERE r.generation IN (SELECT navigation FROM intelligence_generations)
           AND (?1 IS NULL OR r.profile_id = ?1)
           AND (?2 IS NULL OR r.search_engine = ?2)
           AND r.first_visit_ms >= ?3
           AND {upper}
         ORDER BY r.first_visit_ms DESC, r.trail_id DESC LIMIT ?7"
    ))?;
    let trails = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.engine.as_deref(),
                start_ms,
                end_ms,
                cursor.as_ref().map(|c|c.0),cursor.as_ref().map(|c|c.1.as_str()),
                (limit+1) as i64
            ],
            trail_summary_from_row,
        )?
        .collect::<vault_store::Result<Vec<_>>>()?;
    let page=super::paging::finish(trails.into_iter().map(|row| {let key=(row.first_visit_ms,row.trail_id.clone());(row,key)}).collect(),limit)?;
    Ok(TrailListResult {trails:page.items,total,next_cursor:page.next_cursor,page_size:limit as u32})

})
}

/// Decodes one `search_trails` row into the shared summary type that multiple
/// higher-level read models reuse.
pub(super) fn trail_summary_from_row(row: &Row) -> vault_store::Result<TrailSummary> {
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
