//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Search-query and query-family Core Intelligence reads.
//!
//! ## Responsibilities
//! - Serve recent-search and query-family read models.
//! - Own the dedupe/sort helpers that keep query surfaces aligned across
//!   overview cards and dedicated routes.
//! - Keep query-family detail hydration close to the trail context it needs.
//!
//! ## Not responsible for
//! - Session/trail detail reads.
//! - Domain deep-dive and navigation-path surfaces.
//! - Search-engine rule settings or top-level ranking/concept summaries.
//! - Rebuilding query-family/search-event tables.
//!
//! ## Dependencies
//! - Parent-module date helpers and query normalization helpers.
//! - `search_events`, `query_families`, `search_trails`, and archive
//!   visit/source-profile joins.
//!
//! ## Performance notes
//! - `get_search_queries` dedupes exact query variants in SQL before the Rust
//!   layer applies any in-memory family-frequency ordering.
//! - Helper maps only load the trail ids and family summaries referenced by the
//!   current page of results.

use super::intelligence_sessions::trail_summary_from_row;
use super::{date_range_bounds, normalize_query, rfc3339_from_millis};
use crate::models::{
    PagedDateRangeRequest, QueryFamily, QueryFamilyDetail, QueryFamilyDetailRequest,
    QueryFamilyResult, SearchFamiliesRequest,
};
use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashMap};
use vault_store::{OptionalRow, Row, Session, params_owned as params};

pub use super::search_queries::get_search_queries;

/// Every family that holds `query` (as its anchor or any wording), across all time and
/// browsers: the search page's totals and first and last times. Families are kept per
/// browser profile and engine, so one search can have several; the member table keeps this
/// an indexed lookup however many families the archive holds.
pub fn get_search_families(
    vault: &crate::Vault,
    request: &SearchFamiliesRequest,
) -> Result<QueryFamilyResult> {
    let key = super::build_rows::search_family_key(&request.query);
    let profile = request.profile_id.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let families = connection
            .query(
                "SELECT f.family_id, f.anchor_query, f.member_count, f.search_engine, f.first_seen_ms, f.last_seen_ms, f.queries_json
                 FROM query_family_queries_rows m INDEXED BY idx_query_family_queries_query
                 CROSS JOIN query_families_rows f ON f.generation=m.generation AND f.family_id=m.family_id
                 WHERE m.normalized_query=?1
                   AND m.generation IN (SELECT aggregates FROM intelligence_generations)
                   AND (?2 IS NULL OR m.profile_id=?2)
                 ORDER BY f.last_seen_ms DESC, f.family_id DESC",
                params![key, profile],
                query_family_from_row,
            )?;
        let total = families.len() as i64;
        Ok(QueryFamilyResult { page_size: families.len() as u32, families, total, next_cursor: None })
    })
}

/// Returns the paged query-family list for the requested profile/date window.
pub fn get_query_families(
    vault: &crate::Vault,
    request: &PagedDateRangeRequest,
) -> Result<QueryFamilyResult> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_query_families_with_connection(connection, request)
    })
}

/// Reuses an existing connection for query-family loads during overview
/// composition.
pub(super) fn get_query_families_with_connection(
    connection: &Session<'_>,
    request: &PagedDateRangeRequest,
) -> Result<QueryFamilyResult> {
    let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
    // Counted on the rows, from the range's start in the list's index: through the
    // `query_families` view Turso read every family of the build for any range.
    let total: i64 = connection.query_one(
        "SELECT COUNT(*)
         FROM query_families_rows r INDEXED BY idx_query_families_recent
         WHERE r.last_seen_ms >= ?2
           AND r.first_seen_ms < ?3
           AND r.generation IN (SELECT aggregates FROM intelligence_generations)
           AND (?1 IS NULL OR r.profile_id = ?1)",
        params![request.profile_id.as_deref(), start_ms, end_ms],
        |row| Ok(row.get(0)?),
    )?;
    let limit = request.page_size.clamp(1, 1000) as usize;
    let cursor = super::paging::cursor::<(i64, String)>(request.cursor.as_deref())?;
    // Through the `query_families` view Turso reads the whole generation and sorts it on every
    // page. The rows table, walked through idx_query_families_recent, is read in list order and
    // seeks to the cursor; it needs the bounded cursor form, with no `?4 IS NULL OR` escape.
    let after = if cursor.is_some() {
        "AND r.last_seen_ms<=?4 AND (r.last_seen_ms<?4 OR r.family_id<?5)"
    } else {
        ""
    };
    let mut statement = connection.prepare(&format!(
        "SELECT r.family_id, r.anchor_query, r.member_count, r.search_engine, r.first_seen_ms, r.last_seen_ms, r.queries_json
         FROM query_families_rows r INDEXED BY idx_query_families_recent
         WHERE r.generation IN (SELECT aggregates FROM intelligence_generations)
           AND (?1 IS NULL OR r.profile_id = ?1)
           AND r.last_seen_ms >= ?2
           AND r.first_seen_ms < ?3
           {after}
         ORDER BY r.last_seen_ms DESC, r.family_id DESC LIMIT ?6"
    ))?;
    let families = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                cursor.as_ref().map(|c| c.0),
                cursor.as_ref().map(|c| c.1.as_str()),
                (limit + 1) as i64
            ],
            |r| Ok((query_family_from_row(r)?, (r.get::<i64>(5)?, r.get::<String>(0)?))),
        )?
        .collect::<vault_store::Result<Vec<_>>>()?;
    let page = super::paging::finish(families, limit)?;
    Ok(QueryFamilyResult {
        families: page.items,
        total,
        next_cursor: page.next_cursor,
        page_size: limit as u32,
    })
}

/// Loads one query family plus its eight most recent trails in the window.
///
/// The trails are found through the family's search events
/// (`idx_search_events_profile_query`), each joined to its visit for the time
/// window, so the cost follows how often the family was searched, not how
/// many trails the profile has.
pub fn get_query_family_detail(
    vault: &crate::Vault,
    request: &QueryFamilyDetailRequest,
) -> Result<QueryFamilyDetail> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence,move |connection| {
let request=&request;

    let (start_ms, end_ms) = date_range_bounds(connection, &request.date_range)?;
    let (profile_id, family) = load_query_family_detail_row(connection, request)?;
    let normalized_queries = family
        .queries
        .iter()
        .map(|query| normalize_query(query))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if normalized_queries.is_empty() {
        return Ok(QueryFamilyDetail {family,related_trails:Default::default()});
    }
    let limit=request.page_size.unwrap_or(8).clamp(1,1000) as usize;
    let after=super::paging::cursor::<(i64,String)>(request.cursor.as_deref())?;
    let rows=connection.query("SELECT DISTINCT t.trail_id,t.session_id,t.initial_query,t.search_engine,t.reformulation_count,t.visit_count,
            t.landing_url,t.landing_domain,t.first_visit_ms,t.last_visit_ms,t.max_depth,t.queries_json
        FROM intelligence_generations g CROSS JOIN json_each(?2) requested
        CROSS JOIN search_events_rows e INDEXED BY idx_search_events_profile_query
          ON e.profile_id=g.profile_id AND e.normalized_query=requested.value AND e.generation=g.navigation
        CROSS JOIN search_trails_rows t ON t.generation=g.navigation AND t.trail_id=e.trail_id
        WHERE g.profile_id=?1 AND e.trail_id IS NOT NULL AND e.visit_time_ms>=?3 AND e.visit_time_ms<?4
          AND (?5 IS NULL OR t.last_visit_ms<?5 OR (t.last_visit_ms=?5 AND t.trail_id<?6))
        ORDER BY t.last_visit_ms DESC,t.trail_id DESC LIMIT ?7",
        params![profile_id,serde_json::to_string(&normalized_queries)?,start_ms,end_ms,after.as_ref().map(|v|v.0),after.as_ref().map(|v|v.1.as_str()),(limit+1) as i64],|r|Ok((trail_summary_from_row(r)?,(r.get::<i64>(9)?,r.get::<String>(0)?))))?;
    let related_trails=super::paging::finish(rows,limit)?;
    Ok(QueryFamilyDetail { family, related_trails })

})
}

pub(super) fn load_trail_context_map(
    s: &Session<'_>,
    ids: &[(String, String)],
) -> Result<HashMap<String, (String, i64)>> {
    let mut result = HashMap::new();
    let unique = ids.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    for chunk in unique.chunks(400) {
        let values = (0..chunk.len())
            .map(|i| format!("(?{},?{})", i * 2 + 1, i * 2 + 2))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "WITH requested(profile_id,trail_id) AS (VALUES {values}) SELECT t.trail_id,t.initial_query,t.reformulation_count FROM requested CROSS JOIN intelligence_generations g ON g.profile_id=requested.profile_id CROSS JOIN search_trails_rows t ON t.generation=g.navigation AND t.trail_id=requested.trail_id"
        );
        let bindings = chunk.iter().flat_map(|(profile, trail)| [profile.clone(), trail.clone()]);
        for (id, query, count) in s.query(&sql, vault_store::params_from_iter(bindings), |r| {
            Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?))
        })? {
            result.insert(id, (query, count));
        }
    }
    Ok(result)
}

/// Builds a normalized-query to `(family_id, member_count)` map for the current
/// scope so recent-search rows can point back to their deterministic family.
pub(super) fn load_query_family_summary_map(
    connection: &Session<'_>,
    profile_id: Option<&str>,
    search_engine: Option<&str>,
    start_ms: i64,
    end_ms: i64,
) -> Result<HashMap<(String, String), (String, i64)>> {
    let mut statement = connection.prepare(
        "SELECT family_id, search_engine, member_count, queries_json
         FROM query_families
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND (?2 IS NULL OR search_engine = ?2)
           AND last_seen_ms >= ?3
           AND first_seen_ms < ?4",
    )?;
    let rows =
        statement.query_map(params![profile_id, search_engine, start_ms, end_ms], |row| {
            Ok((
                row.get::<String>(0)?,
                row.get::<String>(1)?,
                row.get::<i64>(2)?,
                row.get::<String>(3)?,
            ))
        })?;
    let mut counts = HashMap::<(String, String), (String, i64)>::new();
    for row in rows {
        let (family_id, engine, member_count, queries_json) = row?;
        let queries = serde_json::from_str::<Vec<String>>(&queries_json).unwrap_or_default();
        for query in queries {
            let normalized = normalize_query(&query);
            counts
                .entry((engine.clone(), normalized))
                .and_modify(|current| {
                    if member_count > current.1 {
                        *current = (family_id.clone(), member_count);
                    }
                })
                .or_insert((family_id.clone(), member_count.max(1)));
        }
    }
    Ok(counts)
}

/// Loads one persisted query family and its owning profile for the detail page.
fn load_query_family_detail_row(
    connection: &Session<'_>,
    request: &QueryFamilyDetailRequest,
) -> Result<(String, QueryFamily)> {
    connection
        .query_one(
            "SELECT profile_id, family_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json
             FROM query_families
             WHERE family_id = ?1
               AND (?2 IS NULL OR profile_id = ?2)",
            params![request.family_id, request.profile_id.as_deref()],
            |row| Ok((row.get::< String>(0)?, query_family_from_row_with_offset(row, 1)?)),
        )
        .optional()?
        .with_context(|| format!("query family {} was not found", request.family_id))
}

/// Decodes one `query_families` row into the shared list/detail type.
fn query_family_from_row(row: &Row) -> vault_store::Result<QueryFamily> {
    query_family_from_row_with_offset(row, 0)
}

/// Decodes one `query_families` row with a caller-controlled column offset.
fn query_family_from_row_with_offset(row: &Row, offset: usize) -> vault_store::Result<QueryFamily> {
    Ok(QueryFamily {
        family_id: row.get(offset)?,
        anchor_query: row.get(offset + 1)?,
        member_count: row.get(offset + 2)?,
        search_engine: row.get(offset + 3)?,
        first_seen_at: rfc3339_from_millis(row.get(offset + 4)?),
        last_seen_at: rfc3339_from_millis(row.get(offset + 5)?),
        queries: serde_json::from_str(&row.get::<String>(offset + 6)?).unwrap_or_default(),
    })
}
