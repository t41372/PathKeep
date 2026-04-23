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

use super::{date_range_bounds, ensure_core_intelligence_schema};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{
        AppConfig, PagedDateRangeRequest, SearchTrailQueryRequest, SessionDetail,
        SessionListResult, SessionSummary, SessionVisit, TrailDetail, TrailListResult, TrailMember,
        TrailSummary,
    },
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};

/// Returns one page of deterministic sessions for the requested scope and date
/// range.
pub fn get_sessions(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<SessionListResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM sessions
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3",
        params![request.profile_id.as_deref(), start_ms, end_ms],
        |row| row.get(0),
    )?;
    let offset = request.page.saturating_mul(request.page_size.max(1)) as i64;
    let mut statement = connection.prepare(
        "SELECT session_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title
         FROM sessions
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3
         ORDER BY first_visit_ms DESC, session_id DESC
         LIMIT ?4 OFFSET ?5",
    )?;
    let sessions = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.page_size.max(1) as i64,
                offset
            ],
            |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    first_visit_ms: row.get(1)?,
                    last_visit_ms: row.get(2)?,
                    visit_count: row.get(3)?,
                    search_count: row.get(4)?,
                    domain_count: row.get(5)?,
                    is_deep_dive: row.get::<_, i64>(6)? != 0,
                    auto_title: row.get(7)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SessionListResult { sessions, total, page: request.page, page_size: request.page_size })
}

/// Loads one session plus the visits and trails that belong to it so the
/// frontend can inspect the deterministic grouping.
pub fn get_session_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    session_id: &str,
) -> Result<SessionDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let session = connection
        .query_row(
            "SELECT session_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title
             FROM sessions WHERE session_id = ?1",
            [session_id],
            |row| {
                Ok(SessionSummary {
                    session_id: row.get(0)?,
                    first_visit_ms: row.get(1)?,
                    last_visit_ms: row.get(2)?,
                    visit_count: row.get(3)?,
                    search_count: row.get(4)?,
                    domain_count: row.get(5)?,
                    is_deep_dive: row.get::<_, i64>(6)? != 0,
                    auto_title: row.get(7)?,
                })
            },
        )
        .optional()?
        .with_context(|| format!("session {session_id} was not found"))?;
    let visits = load_session_visits(&connection, session_id)?;
    let trails = load_session_trails(&connection, session_id)?;
    Ok(SessionDetail { session, visits, trails })
}

/// Returns one page of search trails for the requested scope and engine filter.
pub fn get_search_trails(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SearchTrailQueryRequest,
) -> Result<TrailListResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4",
        params![request.profile_id.as_deref(), request.engine.as_deref(), start_ms, end_ms],
        |row| row.get(0),
    )?;
    let offset = request.page.saturating_mul(request.page_size.max(1)) as i64;
    let mut statement = connection.prepare(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_visit_ms >= ?3
           AND first_visit_ms < ?4
         ORDER BY first_visit_ms DESC, trail_id DESC
         LIMIT ?5 OFFSET ?6",
    )?;
    let trails = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.engine.as_deref(),
                start_ms,
                end_ms,
                request.page_size.max(1) as i64,
                offset
            ],
            trail_summary_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(TrailListResult { trails, total, page: request.page, page_size: request.page_size })
}

/// Loads one deterministic trail plus all ordered member visits that belong to
/// it.
pub fn get_trail_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    trail_id: &str,
) -> Result<TrailDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let trail = connection
        .query_row(
            "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                    landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
             FROM search_trails
             WHERE trail_id = ?1",
            [trail_id],
            trail_summary_from_row,
        )
        .optional()?
        .with_context(|| format!("trail {trail_id} was not found"))?;
    let mut statement = connection.prepare(
        "SELECT search_trail_members.trail_id, search_trail_members.visit_id, search_trail_members.ordinal,
                search_trail_members.role, urls.url, visit_derived_facts.canonical_url, urls.title,
                visit_derived_facts.registrable_domain, visits.visit_time_ms, visit_derived_facts.search_query
         FROM search_trail_members
         JOIN archive.visits AS visits ON visits.id = search_trail_members.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         LEFT JOIN visit_derived_facts ON visit_derived_facts.visit_id = visits.id
         WHERE search_trail_members.trail_id = ?1
         ORDER BY search_trail_members.ordinal ASC",
    )?;
    let members = statement
        .query_map([trail_id], |row| {
            Ok(TrailMember {
                trail_id: row.get(0)?,
                visit_id: row.get(1)?,
                ordinal: row.get(2)?,
                role: row.get(3)?,
                url: row.get(4)?,
                canonical_url: row.get(5)?,
                title: row.get(6)?,
                registrable_domain: row.get(7)?,
                visit_time_ms: row.get(8)?,
                search_query: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(TrailDetail { trail, members })
}

/// Loads all ordered visits that the given deterministic session references.
fn load_session_visits(connection: &Connection, session_id: &str) -> Result<Vec<SessionVisit>> {
    let mut statement = connection.prepare(
        "SELECT visits.id, urls.url, urls.title, visit_derived_facts.registrable_domain, visits.visit_time_ms,
                visit_derived_facts.is_search_event, visit_derived_facts.search_query,
                visit_derived_facts.search_engine, visit_derived_facts.trail_id, visits.transition_type
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.session_id = ?1
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    statement
        .query_map([session_id], |row| {
            Ok(SessionVisit {
                visit_id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                registrable_domain: row.get(3)?,
                visit_time_ms: row.get(4)?,
                is_search_event: row.get::<_, i64>(5)? != 0,
                search_query: row.get(6)?,
                search_engine: row.get(7)?,
                trail_id: row.get(8)?,
                transition_type: row.get::<_, Option<i64>>(9)?.map(|value| value.to_string()),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Loads all persisted trails that belong to one session.
fn load_session_trails(connection: &Connection, session_id: &str) -> Result<Vec<TrailSummary>> {
    let mut statement = connection.prepare(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE session_id = ?1
         ORDER BY first_visit_ms ASC, trail_id ASC",
    )?;
    statement
        .query_map([session_id], trail_summary_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Decodes one `search_trails` row into the shared summary type that multiple
/// higher-level read models reuse.
pub(super) fn trail_summary_from_row(row: &Row<'_>) -> rusqlite::Result<TrailSummary> {
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
        queries: serde_json::from_str(&row.get::<_, String>(11)?).unwrap_or_default(),
    })
}
