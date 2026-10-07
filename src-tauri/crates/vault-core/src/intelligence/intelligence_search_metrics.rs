//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
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
    date_range_bounds, display_name_for_search_engine_with_map, load_search_engine_display_names,
    rfc3339_from_millis,
};
use super::{delete_search_engine_rule, list_search_engine_rules, upsert_search_engine_rule};
use crate::models::{
    EngineRanking, FrequentSearch, FrequentSearchesRequest, ScopedDateRangeRequest, SearchConcept,
    SearchEngineRule, SearchEngineRuleInput, TopSearchConceptsRequest,
};
use anyhow::Result;
use vault_store::{Session, params_owned as params};

/// Returns ranked search engines for the requested scope and date range.
pub fn get_search_engine_ranking(
    vault: &crate::Vault,
    request: &ScopedDateRangeRequest,
) -> Result<Vec<EngineRanking>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_search_engine_ranking_with_connection(connection, request)
    })
}

/// Reuses an existing connection for search-engine ranking so overview loaders
/// avoid reopening the intelligence plane.
pub(super) fn get_search_engine_ranking_with_connection(
    connection: &Session<'_>,
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
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Lists the merged search-engine rules exposed to Settings.
pub fn list_search_engine_rules_for_settings(
    vault: &crate::Vault,
) -> Result<Vec<SearchEngineRule>> {
    vault.read(vault_store::Db::Intelligence, list_search_engine_rules)
}

/// Persists one search-engine rule edit and returns the refreshed merged list.
pub fn upsert_search_engine_rule_for_settings(
    vault: &crate::Vault,
    input: &SearchEngineRuleInput,
) -> Result<Vec<SearchEngineRule>> {
    let input = input.clone();
    vault.write(
        vault_store::Db::Intelligence,
        vault_store::Priority::Interactive,
        move |connection| {
            let input = &input;

            upsert_search_engine_rule(connection, input)
        },
    )
}

/// Deletes one search-engine rule and returns the refreshed merged list.
pub fn delete_search_engine_rule_for_settings(
    vault: &crate::Vault,
    rule_id: &str,
) -> Result<Vec<SearchEngineRule>> {
    let rule_id = rule_id.to_owned();
    vault.write(
        vault_store::Db::Intelligence,
        vault_store::Priority::Interactive,
        move |connection| {
            let rule_id = rule_id.as_str();

            delete_search_engine_rule(connection, rule_id)
        },
    )
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
    vault: &crate::Vault,
    request: &FrequentSearchesRequest,
) -> Result<crate::models::IntelligencePage<FrequentSearch>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_frequent_searches_with_connection(connection, request)
    })
}

pub(super) fn get_frequent_searches_with_connection(
    connection: &Session<'_>,
    request: &FrequentSearchesRequest,
) -> Result<crate::models::IntelligencePage<FrequentSearch>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let limit = request.limit.unwrap_or(10).clamp(1, FREQUENT_SEARCHES_MAX_LIMIT) as usize;
    let cursor = super::paging::cursor::<(i64, String)>(request.cursor.as_deref())?;
    let rows = connection.query(
        FREQUENT_SEARCHES_SQL,
        params![
            start_ms,
            end_ms,
            request.profile_id.as_deref(),
            (limit + 1) as i64,
            cursor.as_ref().map(|c| c.0),
            cursor.as_ref().map(|c| c.1.as_str())
        ],
        |r| {
            let key = r.get::<String>(0)?;
            let count = r.get::<i64>(2)?;
            Ok((
                FrequentSearch {
                    normalized_query: key.clone(),
                    query: r.get(1)?,
                    search_count: count,
                    last_searched_at: rfc3339_from_millis(r.get(3)?),
                },
                (count, key),
            ))
        },
    )?;
    super::paging::finish(rows, limit)
}

pub(super) const FREQUENT_SEARCHES_SQL: &str = "WITH counts AS (
        SELECT normalized_query,COUNT(*) AS search_count,MAX(visit_time_ms) AS last_time
        FROM search_events_rows INDEXED BY idx_search_events_kind_time
        WHERE generation IN (SELECT navigation FROM intelligence_generations)
          AND query_kind='keyword' AND visit_time_ms>=?1 AND visit_time_ms<?2
          AND (?3 IS NULL OR profile_id=?3)
        GROUP BY normalized_query
        HAVING ?5 IS NULL OR COUNT(*)<?5 OR (COUNT(*)=?5 AND normalized_query>?6)
        ORDER BY search_count DESC,normalized_query LIMIT ?4
     ) SELECT c.normalized_query,
        (SELECT raw_query FROM search_events_rows newest INDEXED BY idx_search_events_query_time
         WHERE newest.query_kind='keyword' AND newest.normalized_query=c.normalized_query
           AND newest.generation IN (SELECT navigation FROM intelligence_generations)
           AND newest.visit_time_ms>=?1 AND newest.visit_time_ms<?2
           AND (?3 IS NULL OR newest.profile_id=?3)
         ORDER BY newest.visit_time_ms DESC,newest.visit_id DESC LIMIT 1),
        c.search_count,c.last_time FROM counts c
     ORDER BY c.search_count DESC,c.normalized_query";

/// Returns top keyword concepts for one date range without leaking
/// navigational-noise queries back into the UI.
pub fn get_top_search_concepts(
    vault: &crate::Vault,
    request: &TopSearchConceptsRequest,
) -> Result<crate::models::IntelligencePage<SearchConcept>> {
    let request = request.clone();
    vault.read(vault_store::Db::Intelligence, move |connection| {
        let request = &request;

        get_top_search_concepts_with_connection(connection, request)
    })
}

/// Reuses an existing connection for concept ranking so overview loaders can
/// batch multiple read models together.
pub(super) fn get_top_search_concepts_with_connection(
    connection: &Session<'_>,
    request: &TopSearchConceptsRequest,
) -> Result<crate::models::IntelligencePage<SearchConcept>> {
    let (start_ms, end_ms) = date_range_bounds(&request.date_range)?;
    let limit = request.limit.unwrap_or(20).clamp(1, 1000) as usize;
    let cursor = super::paging::cursor::<(i64, String)>(request.cursor.as_deref())?;
    let mut statement = connection.prepare(
        "SELECT search_event_terms.term,
                COUNT(*) AS frequency,
                GROUP_CONCAT(DISTINCT search_events.search_engine)
         FROM search_event_terms
         JOIN search_events ON search_events.visit_id = search_event_terms.visit_id
         WHERE (?1 IS NULL OR search_events.profile_id = ?1)
           AND search_events.query_kind = 'keyword'
           AND search_events.visit_time_ms >= ?2
           AND search_events.visit_time_ms < ?3
         GROUP BY search_event_terms.term
         HAVING ?5 IS NULL OR COUNT(*)<?5 OR (COUNT(*)=?5 AND search_event_terms.term>?6)
         ORDER BY frequency DESC, search_event_terms.term ASC
         LIMIT ?4",
    )?;
    let rows = statement
        .query_map(
            params![
                request.profile_id.as_deref(),
                start_ms,
                end_ms,
                (limit + 1) as i64,
                cursor.as_ref().map(|c| c.0),
                cursor.as_ref().map(|c| c.1.as_str())
            ],
            |row| {
                let engines = row
                    .get::<Option<String>>(2)?
                    .unwrap_or_default()
                    .split(',')
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>();
                Ok((
                    SearchConcept { term: row.get(0)?, frequency: row.get(1)?, engines },
                    (row.get::<i64>(1)?, row.get::<String>(0)?),
                ))
            },
        )?
        .collect::<vault_store::Result<Vec<_>>>()?;
    super::paging::finish(rows, limit)
}
