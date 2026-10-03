//! Search-engine ranking, keyword concept, and rule-setting Core Intelligence
//! reads.
//!
//! ## Responsibilities
//! - Serve engine-ranking, keyword-concept and frequent-search read models that the overview,
//!   Insights and search UI share.
//! - Own the Settings-facing search-engine rule CRUD surface.
//! - Keep search-metric SQL separate from the heavier recent-query and
//!   query-family loaders.
//!
//! ## Not responsible for
//! - Recent-search pagination or query-family detail reads.
//! - Session/trail detail reads.
//! - Rebuilding any search-event or query-family tables.
//!
//! ## Dependencies
//! - Parent-module date-range helpers, search-engine display-name helpers, and
//!   rule persistence.
//! - `engine_daily_rollups`, `search_event_terms`, `search_events`, and archive
//!   visit joins.
//!
//! ## Performance notes
//! - Both concept and engine ranking aggregate inside SQLite and only return
//!   bounded result sets.
//! - Overview composition can reuse the `*_with_connection` helpers to avoid
//!   reopening the intelligence database for each card.

use super::{
    date_range_bounds, display_name_for_search_engine_with_map, ensure_core_intelligence_schema,
    load_search_engine_display_names, rfc3339_from_millis,
};
use super::{delete_search_engine_rule, list_search_engine_rules, upsert_search_engine_rule};
use crate::{
    archive::open_intelligence_connection,
    config::ProjectPaths,
    models::{
        AppConfig, EngineRanking, FrequentSearch, FrequentSearchesRequest, ScopedDateRangeRequest,
        SearchConcept, SearchEngineRule, SearchEngineRuleInput, TopSearchConceptsRequest,
    },
};
use anyhow::Result;
use rusqlite::{Connection, params};

/// Returns ranked search engines for the requested scope and date range.
pub fn get_search_engine_ranking(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<EngineRanking>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_search_engine_ranking_with_connection(&connection, request)
}

/// Reuses an existing connection for search-engine ranking so overview loaders
/// avoid reopening the intelligence plane.
pub(super) fn get_search_engine_ranking_with_connection(
    connection: &Connection,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<EngineRanking>> {
    let display_names = load_search_engine_display_names(connection)?;
    let mut statement = connection.prepare(
        "SELECT search_engine, SUM(search_count)
         FROM engine_daily_rollups
         WHERE (?1 IS NULL OR profile_id = ?1)
           AND date_key >= ?2
           AND date_key <= ?3
         GROUP BY search_engine
         ORDER BY SUM(search_count) DESC, search_engine ASC",
    )?;
    statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                request.date_range.start,
                request.date_range.end
            ],
            |row| {
                let engine: String = row.get(0)?;
                Ok(EngineRanking {
                    display_name: display_name_for_search_engine_with_map(&engine, &display_names),
                    search_engine: engine,
                    search_count: row.get(1)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Lists the merged search-engine rules exposed to Settings.
pub fn list_search_engine_rules_for_settings(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Vec<SearchEngineRule>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    list_search_engine_rules(&connection)
}

/// Persists one search-engine rule edit and returns the refreshed merged list.
pub fn upsert_search_engine_rule_for_settings(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    input: &SearchEngineRuleInput,
) -> Result<Vec<SearchEngineRule>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    upsert_search_engine_rule(&connection, input)
}

/// Deletes one search-engine rule and returns the refreshed merged list.
pub fn delete_search_engine_rule_for_settings(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    rule_id: &str,
) -> Result<Vec<SearchEngineRule>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    delete_search_engine_rule(&connection, rule_id)
}

/// Largest `limit` [`get_frequent_searches`] accepts.
const FREQUENT_SEARCHES_MAX_LIMIT: u32 = 100;

/// Returns the queries searched most often inside one date range, counted from the search
/// events in that range and grouped by normalized query across search engines.
///
/// Only keyword searches count (navigational ones such as "github" are left out), so the counts
/// add up to no more than the digest's search total for the same range.
///
/// Cost: walks `idx_search_events_kind_time` over the keyword searches in the range, then groups
/// them; it never reads search events outside the range. Numbers in
/// `docs/architecture/ipc-performance.md`.
pub fn get_frequent_searches(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &FrequentSearchesRequest,
) -> Result<Vec<FrequentSearch>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_frequent_searches_with_connection(&connection, request)
}

pub(super) fn get_frequent_searches_with_connection(
    connection: &Connection,
    request: &FrequentSearchesRequest,
) -> Result<Vec<FrequentSearch>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let limit = request.limit.unwrap_or(10).clamp(1, FREQUENT_SEARCHES_MAX_LIMIT);
    // `raw_query` is a bare column next to `MAX(visit_time_ms)`, so SQLite takes it from the
    // newest event in each group: the spelling the user typed most recently.
    let mut statement = connection.prepare(FREQUENT_SEARCHES_SQL)?;
    statement
        .query_map(
            params![start_ms, end_ms, request.profile_id.as_deref(), i64::from(limit)],
            |row| {
                Ok(FrequentSearch {
                    normalized_query: row.get(0)?,
                    query: row.get(1)?,
                    search_count: row.get(2)?,
                    last_searched_at: rfc3339_from_millis(row.get(3)?),
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) const FREQUENT_SEARCHES_SQL: &str = "SELECT normalized_query,
            raw_query,
            COUNT(*) AS search_count,
            MAX(visit_time_ms)
     FROM search_events
     WHERE query_kind = 'keyword'
       AND visit_time_ms >= ?1
       AND visit_time_ms < ?2
       AND (?3 IS NULL OR profile_id = ?3)
     GROUP BY normalized_query
     ORDER BY search_count DESC, normalized_query ASC
     LIMIT ?4";

/// Returns top keyword concepts for one date range without leaking
/// navigational-noise queries back into the UI.
pub fn get_top_search_concepts(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &TopSearchConceptsRequest,
) -> Result<Vec<SearchConcept>> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_core_intelligence_schema(&connection)?;
    get_top_search_concepts_with_connection(&connection, request)
}

/// Reuses an existing connection for concept ranking so overview loaders can
/// batch multiple read models together.
pub(super) fn get_top_search_concepts_with_connection(
    connection: &Connection,
    request: &TopSearchConceptsRequest,
) -> Result<Vec<SearchConcept>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let mut statement = connection.prepare(
        "SELECT search_event_terms.term,
                COUNT(*) AS frequency,
                GROUP_CONCAT(DISTINCT search_events.search_engine)
         FROM search_event_terms
         JOIN search_events ON search_events.visit_id = search_event_terms.visit_id
         JOIN archive.visits AS visits ON visits.id = search_events.visit_id
         WHERE (?1 IS NULL OR search_events.profile_id = ?1)
           AND search_events.query_kind = 'keyword'
           AND visits.visit_time_ms >= ?2
           AND visits.visit_time_ms < ?3
         GROUP BY search_event_terms.term
         ORDER BY frequency DESC, search_event_terms.term ASC
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
            |row| {
                let engines = row
                    .get::<_, Option<String>>(2)?
                    .unwrap_or_default()
                    .split(',')
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>();
                Ok(SearchConcept { term: row.get(0)?, frequency: row.get(1)?, engines })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}
