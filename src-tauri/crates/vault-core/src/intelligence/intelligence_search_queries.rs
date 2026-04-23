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
use super::{
    date_range_bounds, display_name_for_search_engine_with_map, ensure_core_intelligence_schema,
    load_search_engine_display_names, normalize_query, rfc3339_from_millis,
};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{
        AppConfig, PagedDateRangeRequest, QueryFamily, QueryFamilyDetail, QueryFamilyDetailRequest,
        QueryFamilyResult, SearchQueryListRequest, SearchQueryListResult, SearchQueryRow,
    },
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Returns the paged recent-search surface, with exact-query dedupe and
/// family/trail context attached to each row.
pub fn get_search_queries(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &SearchQueryListRequest,
) -> Result<SearchQueryListResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let display_names = load_search_engine_display_names(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let sort = request.sort.as_deref().unwrap_or("newest");
    let sort_in_rust = matches!(sort, "family-frequency" | "family_frequency");
    let query_filter =
        request.query.as_deref().map(normalize_query).filter(|value| !value.is_empty());
    let total: i64 = connection.query_row(
        "WITH filtered AS (
            SELECT search_events.visit_id,
                   search_events.profile_id,
                   source_profiles.browser_kind,
                   search_events.search_engine,
                   search_events.raw_query,
                   search_events.normalized_query,
                   search_events.trail_id,
                   visits.visit_time_ms
            FROM search_events
            JOIN archive.visits AS visits ON visits.id = search_events.visit_id
            JOIN archive.source_profiles AS source_profiles
              ON source_profiles.profile_key = search_events.profile_id
            JOIN visit_derived_facts ON visit_derived_facts.visit_id = search_events.visit_id
            WHERE (?1 IS NULL OR search_events.profile_id = ?1)
              AND (?2 IS NULL OR source_profiles.browser_kind = ?2)
              AND (?3 IS NULL OR search_events.search_engine = ?3)
              AND (?4 IS NULL OR visit_derived_facts.registrable_domain = ?4)
              AND (?5 IS NULL
                   OR search_events.normalized_query LIKE '%' || ?5 || '%'
                   OR LOWER(search_events.raw_query) LIKE '%' || ?5 || '%')
              AND search_events.query_kind = 'keyword'
              AND visits.visit_time_ms >= ?6
              AND visits.visit_time_ms < ?7
         ),
         ranked AS (
            SELECT filtered.*,
                   ROW_NUMBER() OVER (
                     PARTITION BY filtered.search_engine, filtered.normalized_query
                     ORDER BY filtered.visit_time_ms DESC, filtered.visit_id DESC
                   ) AS row_rank
            FROM filtered
         )
         SELECT COUNT(*) FROM ranked WHERE row_rank = 1",
        params![
            request.profile_id.as_deref(),
            request.browser_kind.as_deref(),
            request.engine.as_deref(),
            request.domain.as_deref(),
            query_filter.as_deref(),
            start_ms,
            end_ms,
        ],
        |row| row.get(0),
    )?;
    let requested_page_size = request.page_size.max(1);
    let offset =
        if sort_in_rust { 0 } else { request.page.saturating_mul(requested_page_size) as i64 };
    let fetch_limit = if sort_in_rust { total.max(1) } else { requested_page_size as i64 };
    let order_by = search_query_sort_sql(request.sort.as_deref());
    let sql = format!(
        "WITH filtered AS (
            SELECT search_events.visit_id,
                   search_events.profile_id,
                   source_profiles.browser_kind,
                   search_events.search_engine,
                   search_events.raw_query,
                   search_events.normalized_query,
                   search_events.trail_id,
                   visits.visit_time_ms
            FROM search_events
            JOIN archive.visits AS visits ON visits.id = search_events.visit_id
            JOIN archive.source_profiles AS source_profiles
              ON source_profiles.profile_key = search_events.profile_id
            JOIN visit_derived_facts ON visit_derived_facts.visit_id = search_events.visit_id
            WHERE (?1 IS NULL OR search_events.profile_id = ?1)
              AND (?2 IS NULL OR source_profiles.browser_kind = ?2)
              AND (?3 IS NULL OR search_events.search_engine = ?3)
              AND (?4 IS NULL OR visit_derived_facts.registrable_domain = ?4)
              AND (?5 IS NULL
                   OR search_events.normalized_query LIKE '%' || ?5 || '%'
                   OR LOWER(search_events.raw_query) LIKE '%' || ?5 || '%')
              AND search_events.query_kind = 'keyword'
              AND visits.visit_time_ms >= ?6
              AND visits.visit_time_ms < ?7
         ),
         ranked AS (
            SELECT filtered.*,
                   COUNT(*) OVER (
                     PARTITION BY filtered.search_engine, filtered.normalized_query
                   ) AS exact_repeat_count,
                   ROW_NUMBER() OVER (
                     PARTITION BY filtered.search_engine, filtered.normalized_query
                     ORDER BY filtered.visit_time_ms DESC, filtered.visit_id DESC
                   ) AS row_rank
            FROM filtered
         )
         SELECT visit_id,
                profile_id,
                browser_kind,
                search_engine,
                raw_query,
                normalized_query,
                trail_id,
                visit_time_ms,
                exact_repeat_count
         FROM ranked
         WHERE row_rank = 1
         ORDER BY {order_by}
         LIMIT ?8 OFFSET ?9"
    );
    let rows = connection
        .prepare(&sql)?
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.browser_kind.as_deref(),
                request.engine.as_deref(),
                request.domain.as_deref(),
                query_filter.as_deref(),
                start_ms,
                end_ms,
                fetch_limit,
                offset,
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let trail_ids = rows
        .iter()
        .filter_map(|(_, _, _, _, _, _, trail_id, _, _)| trail_id.clone())
        .collect::<Vec<_>>();
    let trail_context = load_trail_context_map(&connection, &trail_ids)?;
    let family_summaries = load_query_family_summary_map(
        &connection,
        request.profile_id.as_deref(),
        request.engine.as_deref(),
        start_ms,
        end_ms,
    )?;
    let mut results = rows
        .into_iter()
        .map(
            |(
                visit_id,
                profile_id,
                browser_kind,
                search_engine,
                raw_query,
                normalized_query,
                trail_id,
                visit_time_ms,
                exact_repeat_count,
            )| {
                let trail = trail_id.as_ref().and_then(|value| trail_context.get(value));
                let family_summary =
                    family_summaries.get(&(search_engine.clone(), normalized_query.clone()));
                SearchQueryRow {
                    visit_id,
                    profile_id,
                    browser_kind,
                    display_name: display_name_for_search_engine_with_map(
                        &search_engine,
                        &display_names,
                    ),
                    search_engine: search_engine.clone(),
                    raw_query,
                    normalized_query: normalized_query.clone(),
                    searched_at: rfc3339_from_millis(visit_time_ms),
                    searched_at_ms: visit_time_ms,
                    exact_repeat_count,
                    family_count: family_summary
                        .map(|(_, count)| *count)
                        .unwrap_or(exact_repeat_count.max(1)),
                    family_id: family_summary.map(|(family_id, _)| family_id.clone()),
                    trail_id,
                    trail_initial_query: trail.map(|value| value.0.clone()),
                    trail_reformulation_count: trail.map(|value| value.1),
                }
            },
        )
        .collect::<Vec<_>>();
    if sort_in_rust {
        results.sort_by(|left, right| {
            right
                .family_count
                .cmp(&left.family_count)
                .then_with(|| right.exact_repeat_count.cmp(&left.exact_repeat_count))
                .then_with(|| right.searched_at_ms.cmp(&left.searched_at_ms))
                .then_with(|| left.search_engine.cmp(&right.search_engine))
                .then_with(|| left.normalized_query.cmp(&right.normalized_query))
        });
        let start = request.page.saturating_mul(requested_page_size) as usize;
        results =
            results.into_iter().skip(start).take(requested_page_size as usize).collect::<Vec<_>>();
    }
    Ok(SearchQueryListResult {
        rows: results,
        total,
        page: request.page,
        page_size: requested_page_size,
    })
}

/// Returns the paged query-family list for the requested profile/date window.
pub fn get_query_families(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &PagedDateRangeRequest,
) -> Result<QueryFamilyResult> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_query_families_with_connection(&connection, request)
}

/// Reuses an existing connection for query-family loads during overview
/// composition.
pub(super) fn get_query_families_with_connection(
    connection: &Connection,
    request: &PagedDateRangeRequest,
) -> Result<QueryFamilyResult> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let total: i64 = connection.query_row(
        "SELECT COUNT(*)
         FROM query_families
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3",
        params![request.profile_id.as_deref(), start_ms, end_ms],
        |row| row.get(0),
    )?;
    let offset = request.page.saturating_mul(request.page_size.max(1)) as i64;
    let mut statement = connection.prepare(
        "SELECT family_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json
         FROM query_families
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND last_seen_ms >= ?2
           AND first_seen_ms < ?3
         ORDER BY last_seen_ms DESC, family_id DESC
         LIMIT ?4 OFFSET ?5",
    )?;
    let families = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                request.page_size.max(1) as i64,
                offset
            ],
            query_family_from_row,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(QueryFamilyResult { families, total, page: request.page, page_size: request.page_size })
}

/// Loads one query family plus a bounded set of related trails that reference
/// the same normalized queries.
pub fn get_query_family_detail(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &QueryFamilyDetailRequest,
) -> Result<QueryFamilyDetail> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let (profile_id, family) = load_query_family_detail_row(&connection, request)?;
    let normalized_queries =
        family.queries.iter().map(|query| normalize_query(query)).collect::<HashSet<_>>();
    let mut statement = connection.prepare(
        "SELECT trail_id, session_id, initial_query, search_engine, reformulation_count, visit_count,
                landing_url, landing_domain, first_visit_ms, last_visit_ms, max_depth, queries_json
         FROM search_trails
         WHERE profile_id = ?1
           AND last_visit_ms >= ?2
           AND first_visit_ms < ?3
         ORDER BY last_visit_ms DESC, trail_id DESC",
    )?;
    let related_trails = statement
        .query_map(params![profile_id, start_ms, end_ms], trail_summary_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|trail| {
            trail.queries.iter().any(|query| normalized_queries.contains(&normalize_query(query)))
                || normalized_queries.contains(&normalize_query(&trail.initial_query))
        })
        .take(8)
        .collect::<Vec<_>>();
    Ok(QueryFamilyDetail { family, related_trails })
}

/// Picks the SQL `ORDER BY` clause for the current recent-search sort mode.
fn search_query_sort_sql(sort: Option<&str>) -> &'static str {
    match sort.unwrap_or("newest") {
        "exact-frequency" | "exact_frequency" => {
            "exact_repeat_count DESC, visit_time_ms DESC, search_engine ASC, normalized_query ASC"
        }
        "family-frequency" | "family_frequency" => {
            "exact_repeat_count DESC, visit_time_ms DESC, search_engine ASC, normalized_query ASC"
        }
        "alphabetical" => "normalized_query ASC, visit_time_ms DESC, search_engine ASC",
        _ => "visit_time_ms DESC, search_engine ASC, normalized_query ASC",
    }
}

/// Loads trail-level context for only the trail ids referenced by the current
/// page of recent-search rows.
fn load_trail_context_map(
    connection: &Connection,
    trail_ids: &[String],
) -> Result<HashMap<String, (String, i64)>> {
    if trail_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let unique_ids =
        trail_ids.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let placeholders = std::iter::repeat_n("?", unique_ids.len()).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT trail_id, initial_query, reformulation_count
         FROM search_trails
         WHERE trail_id IN ({placeholders})"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(rusqlite::params_from_iter(unique_ids.iter()), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (trail_id, initial_query, reformulation_count) = row?;
        map.insert(trail_id, (initial_query, reformulation_count));
    }
    Ok(map)
}

/// Builds a normalized-query to `(family_id, member_count)` map for the current
/// scope so recent-search rows can point back to their deterministic family.
fn load_query_family_summary_map(
    connection: &Connection,
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
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
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
    connection: &Connection,
    request: &QueryFamilyDetailRequest,
) -> Result<(String, QueryFamily)> {
    connection
        .query_row(
            "SELECT profile_id, family_id, anchor_query, member_count, search_engine, first_seen_ms, last_seen_ms, queries_json
             FROM query_families
             WHERE family_id = ?1
               AND (?2 IS NULL OR profile_id = ?2)",
            params![request.family_id, request.profile_id.as_deref()],
            |row| Ok((row.get::<_, String>(0)?, query_family_from_row_with_offset(row, 1)?)),
        )
        .optional()?
        .with_context(|| format!("query family {} was not found", request.family_id))
}

/// Decodes one `query_families` row into the shared list/detail type.
fn query_family_from_row(row: &Row<'_>) -> rusqlite::Result<QueryFamily> {
    query_family_from_row_with_offset(row, 0)
}

/// Decodes one `query_families` row with a caller-controlled column offset.
fn query_family_from_row_with_offset(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<QueryFamily> {
    Ok(QueryFamily {
        family_id: row.get(offset)?,
        anchor_query: row.get(offset + 1)?,
        member_count: row.get(offset + 2)?,
        search_engine: row.get(offset + 3)?,
        first_seen_at: rfc3339_from_millis(row.get(offset + 4)?),
        last_seen_at: rfc3339_from_millis(row.get(offset + 5)?),
        queries: serde_json::from_str(&row.get::<_, String>(offset + 6)?).unwrap_or_default(),
    })
}
