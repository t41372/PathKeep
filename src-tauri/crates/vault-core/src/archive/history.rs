//! Archive history query and export flows.
//!
//! This module owns the Explorer-facing read surface over canonical visits. It
//! keeps three recall modes explicit:
//!
//! - regular SQL filtering
//! - FTS-backed keyword recall
//! - manual regex post-filtering
//!
//! The accepted product contract is that only visible facts participate in
//! recall/export. Hidden/reverted rows stay out of these surfaces, and regex is
//! a slower manual path instead of pretending to be the default fast query
//! engine.

use super::search_lexical::{FuzzyDocument, FuzzyQuery, LexicalQuery, analyze_query};
use super::search_query::{ParsedHistorySearchQuery, parse_history_search_query};
use super::*;

mod export;
mod favicons;
pub mod og_images;
pub mod og_images_fetch;
pub mod og_images_synth;
mod pagination;

pub use self::export::export_history;
pub use self::favicons::load_history_favicons;
// og_images functions are re-exported via the `og_images` module path so the
// worker and Tauri command crates can address them as
// `vault_core::archive::history::og_images::*`. They land in C3/C4.
#[cfg(test)]
pub(super) use self::favicons::{
    LOAD_FAVICON_CROSS_PROFILE_HOST_SQL, LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL,
    LOAD_FAVICON_SAME_PROFILE_HOST_SQL, LOAD_FAVICON_SAME_PROFILE_PAGE_SQL,
};
#[allow(unused_imports)]
pub use self::og_images::{
    OgImageInsert, clear_cache as clear_og_image_cache, load_og_images, mark_og_images_shown,
    run_cleanup as run_og_image_cleanup, storage_stats as og_image_storage_stats, upsert_og_image,
};
use self::pagination::{
    HistoryCursor, build_history_response, build_lexical_history_response, normalize_history_sort,
    page_count, parse_history_cursor,
};

const LIST_HISTORY_LEXICAL_SQL: &str = r#"
WITH search_matches AS (
  SELECT
    rowid AS url_id,
    bm25(history_search_terms, 6.0, 12.0, 4.0, 5.0, 10.0, 4.0, 2.0) AS score
  FROM search.history_search_terms
  WHERE :termsFtsQuery IS NOT NULL
    AND history_search_terms MATCH :termsFtsQuery
  UNION ALL
  SELECT
    rowid AS url_id,
    bm25(history_search_trigram, 1.0) + 0.35 AS score
  FROM search.history_search_trigram
  WHERE :trigramFtsQuery IS NOT NULL
    AND history_search_trigram MATCH :trigramFtsQuery
),
ranked_urls AS (
  SELECT url_id, MIN(score) AS score
  FROM search_matches
  GROUP BY url_id
)
SELECT
  visits.id,
  source_profiles.profile_key,
  urls.url,
  urls.title,
  visits.visit_time_ms,
  visits.visit_duration_ms,
  visits.transition_type,
  visits.source_visit_id,
  visits.app_id,
  ranked_urls.score
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
JOIN ranked_urls
  ON ranked_urls.url_id = urls.id
WHERE visits.reverted_at IS NULL
  AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
  AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
  AND (:domainPattern IS NULL OR urls.url LIKE :domainPattern)
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_sites AS advanced_filter WHERE LOWER(urls.url) NOT LIKE '%' || advanced_filter.value || '%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_sites AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_filetypes AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%.' || advanced_filter.value OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '?%' OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '#%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_filetypes AS advanced_filter WHERE LOWER(urls.url) LIKE '%.' || advanced_filter.value OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '?%' OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '#%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_url_terms AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_url LIKE '%' || advanced_filter.value || '%')))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_url_terms AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_url LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_title_terms AS advanced_filter WHERE NOT (LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_title LIKE '%' || advanced_filter.value || '%')))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_title_terms AS advanced_filter WHERE LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_title LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_exact_terms AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND (search_documents.normalized_url LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_title LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_search_terms LIKE '%' || advanced_filter.value || '%' OR search_documents.compact_text LIKE '%' || advanced_filter.value || '%'))))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_terms AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND (search_documents.normalized_url LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_title LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_search_terms LIKE '%' || advanced_filter.value || '%' OR search_documents.compact_text LIKE '%' || advanced_filter.value || '%')))
  AND (:startTimeMs IS NULL OR visits.visit_time_ms >= :startTimeMs)
  AND (:endTimeMs IS NULL OR visits.visit_time_ms <= :endTimeMs)
  AND (
    :cursorVisitTime IS NULL
    OR (
      :sort = 'oldest'
      AND (
        visits.visit_time_ms > :cursorVisitTime
        OR (visits.visit_time_ms = :cursorVisitTime AND visits.id > :cursorId)
      )
    )
    OR (
      :sort = 'newest'
      AND (
        visits.visit_time_ms < :cursorVisitTime
        OR (visits.visit_time_ms = :cursorVisitTime AND visits.id < :cursorId)
      )
    )
    OR (
      :sort = 'relevance'
      AND (
        ranked_urls.score > :cursorScore
        OR (
          ranked_urls.score = :cursorScore
          AND (
            visits.visit_time_ms < :cursorVisitTime
            OR (visits.visit_time_ms = :cursorVisitTime AND visits.id < :cursorId)
          )
        )
      )
    )
  )
ORDER BY
  CASE WHEN :sort = 'oldest' THEN visits.visit_time_ms END ASC,
  CASE WHEN :sort = 'oldest' THEN visits.id END ASC,
  CASE WHEN :sort = 'newest' THEN visits.visit_time_ms END DESC,
  CASE WHEN :sort = 'newest' THEN visits.id END DESC,
  CASE WHEN :sort = 'relevance' THEN ranked_urls.score END ASC,
  CASE WHEN :sort = 'relevance' THEN visits.visit_time_ms END DESC,
  CASE WHEN :sort = 'relevance' THEN visits.id END DESC
LIMIT :pageLimit
OFFSET :pageOffset
"#;

const FUZZY_CANDIDATE_URL_LIMIT: i64 = 200;
const FUZZY_CANDIDATE_VISIT_LIMIT: i64 = 400;
const ADVANCED_FILTER_TABLES: &[&str] = &[
    "history_exact_terms",
    "history_excluded_terms",
    "history_required_title_terms",
    "history_excluded_title_terms",
    "history_required_url_terms",
    "history_excluded_url_terms",
    "history_required_sites",
    "history_excluded_sites",
    "history_required_filetypes",
    "history_excluded_filetypes",
];

const LIST_HISTORY_FUZZY_CANDIDATES_SQL: &str = r#"
WITH fuzzy_url_candidates AS (
  SELECT
    rowid AS url_id,
    bm25(history_search_trigram, 1.0) AS fts_score
  FROM search.history_search_trigram
  WHERE history_search_trigram MATCH :fuzzyFtsQuery
  ORDER BY fts_score ASC
  LIMIT :candidateUrlLimit
)
SELECT
  visits.id,
  source_profiles.profile_key,
  urls.url,
  urls.title,
  visits.visit_time_ms,
  visits.visit_duration_ms,
  visits.transition_type,
  visits.source_visit_id,
  visits.app_id,
  fuzzy_url_candidates.fts_score,
  search_documents.normalized_url,
  search_documents.normalized_title,
  search_documents.normalized_search_terms,
  search_documents.compact_text
FROM fuzzy_url_candidates
JOIN search.search_documents
  ON search_documents.url_id = fuzzy_url_candidates.url_id
JOIN urls
  ON urls.id = fuzzy_url_candidates.url_id
JOIN visits
  ON visits.url_id = urls.id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
WHERE visits.reverted_at IS NULL
  AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
  AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
  AND (:domainPattern IS NULL OR urls.url LIKE :domainPattern)
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_sites AS advanced_filter WHERE LOWER(urls.url) NOT LIKE '%' || advanced_filter.value || '%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_sites AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_filetypes AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%.' || advanced_filter.value OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '?%' OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '#%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_filetypes AS advanced_filter WHERE LOWER(urls.url) LIKE '%.' || advanced_filter.value OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '?%' OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '#%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_url_terms AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_url LIKE '%' || advanced_filter.value || '%')))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_url_terms AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_url LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_title_terms AS advanced_filter WHERE NOT (LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_title LIKE '%' || advanced_filter.value || '%')))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_title_terms AS advanced_filter WHERE LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_title LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_exact_terms AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND (search_documents.normalized_url LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_title LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_search_terms LIKE '%' || advanced_filter.value || '%' OR search_documents.compact_text LIKE '%' || advanced_filter.value || '%'))))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_terms AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND (search_documents.normalized_url LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_title LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_search_terms LIKE '%' || advanced_filter.value || '%' OR search_documents.compact_text LIKE '%' || advanced_filter.value || '%')))
  AND (:startTimeMs IS NULL OR visits.visit_time_ms >= :startTimeMs)
  AND (:endTimeMs IS NULL OR visits.visit_time_ms <= :endTimeMs)
ORDER BY
  fuzzy_url_candidates.fts_score ASC,
  visits.visit_time_ms DESC,
  visits.id DESC
LIMIT :candidateVisitLimit
"#;

const COUNT_HISTORY_LEXICAL_SQL: &str = r#"
WITH search_matches AS (
  SELECT rowid AS url_id
  FROM search.history_search_terms
  WHERE :termsFtsQuery IS NOT NULL
    AND history_search_terms MATCH :termsFtsQuery
  UNION
  SELECT rowid AS url_id
  FROM search.history_search_trigram
  WHERE :trigramFtsQuery IS NOT NULL
    AND history_search_trigram MATCH :trigramFtsQuery
),
ranked_urls AS (
  SELECT url_id
  FROM search_matches
  GROUP BY url_id
)
SELECT COUNT(*)
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
JOIN ranked_urls
  ON ranked_urls.url_id = urls.id
WHERE visits.reverted_at IS NULL
  AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
  AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
  AND (:domainPattern IS NULL OR urls.url LIKE :domainPattern)
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_sites AS advanced_filter WHERE LOWER(urls.url) NOT LIKE '%' || advanced_filter.value || '%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_sites AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_filetypes AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%.' || advanced_filter.value OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '?%' OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '#%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_filetypes AS advanced_filter WHERE LOWER(urls.url) LIKE '%.' || advanced_filter.value OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '?%' OR LOWER(urls.url) LIKE '%.' || advanced_filter.value || '#%')
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_url_terms AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_url LIKE '%' || advanced_filter.value || '%')))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_url_terms AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_url LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_title_terms AS advanced_filter WHERE NOT (LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_title LIKE '%' || advanced_filter.value || '%')))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_title_terms AS advanced_filter WHERE LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND search_documents.normalized_title LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_exact_terms AS advanced_filter WHERE NOT (LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND (search_documents.normalized_url LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_title LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_search_terms LIKE '%' || advanced_filter.value || '%' OR search_documents.compact_text LIKE '%' || advanced_filter.value || '%'))))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_terms AS advanced_filter WHERE LOWER(urls.url) LIKE '%' || advanced_filter.value || '%' OR LOWER(IFNULL(urls.title, '')) LIKE '%' || advanced_filter.value || '%' OR EXISTS (SELECT 1 FROM search.search_documents WHERE search_documents.url_id = urls.id AND (search_documents.normalized_url LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_title LIKE '%' || advanced_filter.value || '%' OR search_documents.normalized_search_terms LIKE '%' || advanced_filter.value || '%' OR search_documents.compact_text LIKE '%' || advanced_filter.value || '%')))
  AND (:startTimeMs IS NULL OR visits.visit_time_ms >= :startTimeMs)
  AND (:endTimeMs IS NULL OR visits.visit_time_ms <= :endTimeMs)
"#;

/// Queries visible history rows with pagination, FTS, and regex support.
pub fn list_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let connection = open_archive_connection(paths, config, key)?;
    let limit = query.limit.unwrap_or(150).clamp(1, 1_000);
    let limit_usize = limit as usize;
    let requested_page = query.page.map(|page| usize::try_from(page.max(1)).unwrap_or(usize::MAX));
    let profile_id = query.profile_id.clone();
    let browser_kind = query.browser_kind.clone();
    let raw_q = query.q.clone().filter(|value| !value.trim().is_empty());
    let regex_mode = query.regex_mode.unwrap_or(false);
    let parsed_query = if regex_mode {
        ParsedHistorySearchQuery::default()
    } else {
        raw_q.as_deref().map(parse_history_search_query).unwrap_or_default()
    };
    prepare_advanced_search_filters(&connection, &parsed_query)?;
    let start_time_ms = max_optional_i64(query.start_time_ms, parsed_query.after_ms);
    let end_time_ms = min_optional_i64(query.end_time_ms, parsed_query.before_ms);
    let q = if regex_mode { raw_q.clone() } else { parsed_query.keyword_text.clone() };
    let lexical_query = q.as_deref().and_then(analyze_query);
    let regex = if regex_mode {
        q.as_ref()
            .map(|value| {
                RegexBuilder::new(value)
                    .case_insensitive(true)
                    .build()
                    .with_context(|| format!("invalid regex pattern `{value}`"))
            })
            .transpose()?
    } else {
        None
    };
    let domain_pattern = query
        .domain
        .clone()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{value}%"));
    let sort = normalize_history_sort(query.sort.as_deref(), q.is_some(), lexical_query.is_some());
    let cursor = parse_history_cursor(query.cursor.as_deref());

    if let Some(regex) = regex {
        return list_history_with_regex(
            &connection,
            limit_usize,
            requested_page,
            profile_id,
            browser_kind,
            domain_pattern,
            start_time_ms,
            end_time_ms,
            sort,
            cursor,
            regex,
        );
    }

    if q.is_some() && lexical_query.is_none() {
        return Ok(HistoryQueryResponse::default());
    }

    if let Some(lexical_query) = lexical_query {
        return list_history_with_lexical_search(
            &connection,
            limit,
            limit_usize,
            requested_page,
            profile_id,
            browser_kind,
            domain_pattern,
            start_time_ms,
            end_time_ms,
            sort,
            cursor,
            lexical_query,
        );
    }

    let (cursor_visit_time, cursor_id) =
        cursor.and_then(HistoryCursor::chronological).unwrap_or((0, 0));

    list_history_with_sql(
        &connection,
        limit,
        limit_usize,
        requested_page,
        profile_id,
        browser_kind,
        domain_pattern,
        start_time_ms,
        end_time_ms,
        sort,
        cursor,
        cursor_visit_time,
        cursor_id,
        q,
    )
}

fn max_optional_i64(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn min_optional_i64(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn prepare_advanced_search_filters(
    connection: &Connection,
    parsed_query: &ParsedHistorySearchQuery,
) -> Result<()> {
    for table in ADVANCED_FILTER_TABLES {
        connection.execute(
            &format!("CREATE TEMP TABLE IF NOT EXISTS {table} (value TEXT NOT NULL)"),
            [],
        )?;
        connection.execute(&format!("DELETE FROM {table}"), [])?;
    }
    insert_filter_values(connection, "history_exact_terms", &parsed_query.exact_terms)?;
    insert_filter_values(connection, "history_excluded_terms", &parsed_query.excluded_terms)?;
    insert_filter_values(
        connection,
        "history_required_title_terms",
        &parsed_query.required_title_terms,
    )?;
    insert_filter_values(
        connection,
        "history_excluded_title_terms",
        &parsed_query.excluded_title_terms,
    )?;
    insert_filter_values(
        connection,
        "history_required_url_terms",
        &parsed_query.required_url_terms,
    )?;
    insert_filter_values(
        connection,
        "history_excluded_url_terms",
        &parsed_query.excluded_url_terms,
    )?;
    insert_filter_values(connection, "history_required_sites", &parsed_query.required_sites)?;
    insert_filter_values(connection, "history_excluded_sites", &parsed_query.excluded_sites)?;
    insert_filter_values(
        connection,
        "history_required_filetypes",
        &parsed_query.required_filetypes,
    )?;
    insert_filter_values(
        connection,
        "history_excluded_filetypes",
        &parsed_query.excluded_filetypes,
    )?;
    Ok(())
}

fn insert_filter_values(connection: &Connection, table: &str, values: &[String]) -> Result<()> {
    let mut statement = connection.prepare(&format!("INSERT INTO {table} (value) VALUES (?1)"))?;
    for value in values {
        statement.execute(params![value])?;
    }
    Ok(())
}

/// Runs regex recall as a manual post-filter over the canonical SQL query.
#[allow(clippy::too_many_arguments)]
fn list_history_with_regex(
    connection: &Connection,
    limit_usize: usize,
    requested_page: Option<usize>,
    profile_id: Option<String>,
    browser_kind: Option<String>,
    domain_pattern: Option<String>,
    start_time_ms: Option<i64>,
    end_time_ms: Option<i64>,
    sort: String,
    cursor: Option<HistoryCursor>,
    regex: regex::Regex,
) -> Result<HistoryQueryResponse> {
    let mut statement = connection.prepare(LIST_HISTORY_SQL)?;
    let rows = statement.query_map(
        named_params! {
            ":profileId": profile_id,
            ":browserKind": browser_kind,
            ":query": Option::<String>::None,
            ":domainPattern": domain_pattern,
            ":startTimeMs": start_time_ms,
            ":endTimeMs": end_time_ms,
            ":sort": sort,
            ":cursorVisitTime": Option::<i64>::None,
            ":cursorId": Option::<i64>::None,
            ":pageLimit": -1i64,
            ":pageOffset": 0i64,
        },
        history_entry_from_row,
    )?;
    let filtered_items = rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|entry| {
            regex.is_match(&entry.url)
                || entry.title.as_ref().is_some_and(|title| regex.is_match(title))
        })
        .collect::<Vec<_>>();
    let total = filtered_items.len();
    let normalized_page_count = page_count(total, limit_usize);
    let page = requested_page.unwrap_or(1).min(normalized_page_count);
    let start_index = if requested_page.is_some() {
        page.saturating_sub(1) * limit_usize
    } else if let Some((cursor_visit_time, cursor_id)) =
        cursor.and_then(HistoryCursor::chronological)
    {
        filtered_items
            .iter()
            .position(|entry| {
                if sort == "oldest" {
                    entry.visit_time > cursor_visit_time
                        || (entry.visit_time == cursor_visit_time && entry.id > cursor_id)
                } else {
                    entry.visit_time < cursor_visit_time
                        || (entry.visit_time == cursor_visit_time && entry.id < cursor_id)
                }
            })
            .unwrap_or(total)
    } else {
        0
    };
    let items = filtered_items.into_iter().skip(start_index).take(limit_usize).collect::<Vec<_>>();

    Ok(build_history_response(total, limit_usize, page, start_index, items))
}

/// Runs normalized FTS-backed keyword recall.
#[allow(clippy::too_many_arguments)]
fn list_history_with_lexical_search(
    connection: &Connection,
    limit: u32,
    limit_usize: usize,
    requested_page: Option<usize>,
    profile_id: Option<String>,
    browser_kind: Option<String>,
    domain_pattern: Option<String>,
    start_time_ms: Option<i64>,
    end_time_ms: Option<i64>,
    sort: String,
    cursor: Option<HistoryCursor>,
    lexical_query: LexicalQuery,
) -> Result<HistoryQueryResponse> {
    let fuzzy_query = lexical_query.fuzzy_query.clone();
    let total: usize = connection
        .query_row(
            COUNT_HISTORY_LEXICAL_SQL,
            named_params! {
                ":termsFtsQuery": lexical_query.terms_query.clone(),
                ":trigramFtsQuery": lexical_query.trigram_query.clone(),
                ":profileId": profile_id.clone(),
                ":browserKind": browser_kind.clone(),
                ":domainPattern": domain_pattern.clone(),
                ":startTimeMs": start_time_ms,
                ":endTimeMs": end_time_ms,
            },
            |row| row.get::<_, i64>(0),
        )?
        .try_into()
        .expect("history count fits in usize");

    if total == 0
        && let Some(fuzzy_query) = fuzzy_query
    {
        return list_history_with_fuzzy_fallback(
            connection,
            limit_usize,
            requested_page,
            profile_id,
            browser_kind,
            domain_pattern,
            start_time_ms,
            end_time_ms,
            sort,
            cursor,
            fuzzy_query,
        );
    }

    let mut statement = connection.prepare(LIST_HISTORY_LEXICAL_SQL)?;
    let normalized_page_count = page_count(total, limit_usize);
    let page = requested_page.unwrap_or(1).min(normalized_page_count);
    let start_index = page.saturating_sub(1) * limit_usize;
    let page_limit = if requested_page.is_some() { i64::from(limit) } else { i64::from(limit) + 1 };
    let page_offset =
        if requested_page.is_some() { i64::try_from(start_index).unwrap_or(i64::MAX) } else { 0 };
    let chronological_cursor =
        (sort != "relevance").then(|| cursor.and_then(HistoryCursor::chronological)).flatten();
    let relevance_cursor =
        (sort == "relevance").then(|| cursor.and_then(HistoryCursor::relevance)).flatten();
    let cursor_visit_time = chronological_cursor
        .map(|(visit_time, _)| visit_time)
        .or_else(|| relevance_cursor.map(|(_, visit_time, _)| visit_time));
    let cursor_id =
        chronological_cursor.map(|(_, id)| id).or_else(|| relevance_cursor.map(|(_, _, id)| id));
    let cursor_score = relevance_cursor.map(|(score, _, _)| score);
    let rows = statement.query_map(
        named_params! {
            ":termsFtsQuery": lexical_query.terms_query,
            ":trigramFtsQuery": lexical_query.trigram_query,
            ":profileId": profile_id,
            ":browserKind": browser_kind,
            ":domainPattern": domain_pattern,
            ":startTimeMs": start_time_ms,
            ":endTimeMs": end_time_ms,
            ":sort": sort,
            ":cursorVisitTime": if requested_page.is_some() { Option::<i64>::None } else { cursor_visit_time },
            ":cursorId": if requested_page.is_some() { Option::<i64>::None } else { cursor_id },
            ":cursorScore": if requested_page.is_some() { Option::<f64>::None } else { cursor_score },
            ":pageLimit": page_limit,
            ":pageOffset": page_offset,
        },
        history_entry_with_score_from_row,
    )?;
    let scored_items = if requested_page.is_some() {
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        let mut window_items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        if window_items.len() > limit_usize {
            window_items.truncate(limit_usize);
        }
        window_items
    };

    Ok(build_lexical_history_response(
        total,
        limit_usize,
        page,
        if requested_page.is_some() {
            start_index
        } else if chronological_cursor.is_some() || relevance_cursor.is_some() {
            limit_usize
        } else {
            0
        },
        scored_items,
        &sort,
    ))
}

/// Runs Latin typo tolerance over a bounded trigram candidate set.
#[allow(clippy::too_many_arguments)]
fn list_history_with_fuzzy_fallback(
    connection: &Connection,
    limit_usize: usize,
    requested_page: Option<usize>,
    profile_id: Option<String>,
    browser_kind: Option<String>,
    domain_pattern: Option<String>,
    start_time_ms: Option<i64>,
    end_time_ms: Option<i64>,
    sort: String,
    cursor: Option<HistoryCursor>,
    fuzzy_query: FuzzyQuery,
) -> Result<HistoryQueryResponse> {
    let mut statement = connection.prepare(LIST_HISTORY_FUZZY_CANDIDATES_SQL)?;
    let rows = statement.query_map(
        named_params! {
            ":fuzzyFtsQuery": fuzzy_query.candidate_query,
            ":candidateUrlLimit": FUZZY_CANDIDATE_URL_LIMIT,
            ":candidateVisitLimit": FUZZY_CANDIDATE_VISIT_LIMIT,
            ":profileId": profile_id,
            ":browserKind": browser_kind,
            ":domainPattern": domain_pattern,
            ":startTimeMs": start_time_ms,
            ":endTimeMs": end_time_ms,
        },
        fuzzy_candidate_from_row,
    )?;
    let mut scored_items = rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|candidate| {
            let score = fuzzy_query.score_document(&candidate.document())?;
            Some((candidate.entry, score))
        })
        .collect::<Vec<_>>();

    sort_fuzzy_items(&mut scored_items, &sort);
    let total = scored_items.len();
    let normalized_page_count = page_count(total, limit_usize);
    let page = requested_page.unwrap_or(1).min(normalized_page_count);
    let start_index =
        fuzzy_start_index(&scored_items, limit_usize, requested_page, page, &sort, cursor);
    let page_items = scored_items.into_iter().skip(start_index).take(limit_usize).collect();

    Ok(build_lexical_history_response(total, limit_usize, page, start_index, page_items, &sort))
}

fn sort_fuzzy_items(scored_items: &mut [(HistoryEntry, f64)], sort: &str) {
    match sort {
        "oldest" => scored_items.sort_by(|(left, _), (right, _)| {
            left.visit_time.cmp(&right.visit_time).then_with(|| left.id.cmp(&right.id))
        }),
        "newest" => scored_items.sort_by(|(left, _), (right, _)| {
            right.visit_time.cmp(&left.visit_time).then_with(|| right.id.cmp(&left.id))
        }),
        _ => scored_items.sort_by(|(left_entry, left_score), (right_entry, right_score)| {
            left_score
                .total_cmp(right_score)
                .then_with(|| right_entry.visit_time.cmp(&left_entry.visit_time))
                .then_with(|| right_entry.id.cmp(&left_entry.id))
        }),
    }
}

fn fuzzy_start_index(
    scored_items: &[(HistoryEntry, f64)],
    limit_usize: usize,
    requested_page: Option<usize>,
    page: usize,
    sort: &str,
    cursor: Option<HistoryCursor>,
) -> usize {
    if requested_page.is_some() {
        return page.saturating_sub(1) * limit_usize;
    }
    match sort {
        "oldest" => cursor
            .and_then(HistoryCursor::chronological)
            .and_then(|(cursor_visit_time, cursor_id)| {
                scored_items.iter().position(|(entry, _)| {
                    entry.visit_time > cursor_visit_time
                        || (entry.visit_time == cursor_visit_time && entry.id > cursor_id)
                })
            })
            .unwrap_or(0),
        "newest" => cursor
            .and_then(HistoryCursor::chronological)
            .and_then(|(cursor_visit_time, cursor_id)| {
                scored_items.iter().position(|(entry, _)| {
                    entry.visit_time < cursor_visit_time
                        || (entry.visit_time == cursor_visit_time && entry.id < cursor_id)
                })
            })
            .unwrap_or(0),
        _ => cursor
            .and_then(HistoryCursor::relevance)
            .and_then(|(cursor_score, cursor_visit_time, cursor_id)| {
                scored_items.iter().position(|(entry, score)| {
                    *score > cursor_score
                        || (*score == cursor_score
                            && (entry.visit_time < cursor_visit_time
                                || (entry.visit_time == cursor_visit_time && entry.id < cursor_id)))
                })
            })
            .unwrap_or(0),
    }
}

/// Runs the baseline SQL-filtered recall path.
#[allow(clippy::too_many_arguments)]
fn list_history_with_sql(
    connection: &Connection,
    limit: u32,
    limit_usize: usize,
    requested_page: Option<usize>,
    profile_id: Option<String>,
    browser_kind: Option<String>,
    domain_pattern: Option<String>,
    start_time_ms: Option<i64>,
    end_time_ms: Option<i64>,
    sort: String,
    cursor: Option<HistoryCursor>,
    cursor_visit_time: i64,
    cursor_id: i64,
    q: Option<String>,
) -> Result<HistoryQueryResponse> {
    let total: usize = connection
        .query_row(
            COUNT_HISTORY_SQL,
            named_params! {
                ":profileId": profile_id.clone(),
                ":browserKind": browser_kind.clone(),
                ":query": q.clone(),
                ":domainPattern": domain_pattern.clone(),
                ":startTimeMs": start_time_ms,
                ":endTimeMs": end_time_ms,
            },
            |row| row.get::<_, i64>(0),
        )?
        .try_into()
        .expect("history count fits in usize");

    let mut statement = connection.prepare(LIST_HISTORY_SQL)?;
    let normalized_page_count = page_count(total, limit_usize);
    let page = requested_page.unwrap_or(1).min(normalized_page_count);
    let start_index = page.saturating_sub(1) * limit_usize;
    let page_limit = if requested_page.is_some() { i64::from(limit) } else { i64::from(limit) + 1 };
    let page_offset =
        if requested_page.is_some() { i64::try_from(start_index).unwrap_or(i64::MAX) } else { 0 };
    let rows = statement.query_map(
        named_params! {
            ":profileId": profile_id,
            ":browserKind": browser_kind,
            ":query": q,
            ":domainPattern": domain_pattern,
            ":startTimeMs": start_time_ms,
            ":endTimeMs": end_time_ms,
            ":sort": sort,
            ":cursorVisitTime": if requested_page.is_some() { Option::<i64>::None } else { cursor.map(|_| cursor_visit_time) },
            ":cursorId": if requested_page.is_some() { Option::<i64>::None } else { cursor.map(|_| cursor_id) },
            ":pageLimit": page_limit,
            ":pageOffset": page_offset,
        },
        history_entry_from_row,
    )?;
    let items = if requested_page.is_some() {
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        let mut window_items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        if window_items.len() > limit_usize {
            window_items.truncate(limit_usize);
        }
        window_items
    };

    Ok(build_history_response(
        total,
        limit_usize,
        page,
        if requested_page.is_some() {
            start_index
        } else if cursor.is_some() {
            limit_usize
        } else {
            0
        },
        items,
    ))
}

/// Shapes one SQL row into the Explorer-facing history entry model.
pub(super) fn history_entry_from_row(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let url: String = row.get(2)?;
    let source_visit_id = row
        .get::<_, Option<String>>(7)?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    Ok(HistoryEntry {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        domain: url_domain(&url),
        url,
        title: row.get(3)?,
        favicon: None,
        visited_at: row.get(4).map(|ms: i64| {
            DateTime::<Utc>::from_timestamp_millis(ms).unwrap_or_else(Utc::now).to_rfc3339()
        })?,
        visit_time: row.get(4)?,
        duration_ms: row.get(5)?,
        transition: row.get(6)?,
        source_visit_id,
        app_id: row.get(8)?,
    })
}

fn history_entry_with_score_from_row(row: &Row<'_>) -> rusqlite::Result<(HistoryEntry, f64)> {
    Ok((history_entry_from_row(row)?, row.get(9)?))
}

struct FuzzyCandidate {
    entry: HistoryEntry,
    normalized_url: String,
    normalized_title: String,
    normalized_search_terms: String,
    compact_text: String,
}

impl FuzzyCandidate {
    fn document(&self) -> FuzzyDocument<'_> {
        FuzzyDocument {
            normalized_url: &self.normalized_url,
            normalized_title: &self.normalized_title,
            normalized_search_terms: &self.normalized_search_terms,
            compact_text: &self.compact_text,
        }
    }
}

fn fuzzy_candidate_from_row(row: &Row<'_>) -> rusqlite::Result<FuzzyCandidate> {
    Ok(FuzzyCandidate {
        entry: history_entry_from_row(row)?,
        normalized_url: row.get(10)?,
        normalized_title: row.get(11)?,
        normalized_search_terms: row.get(12)?,
        compact_text: row.get(13)?,
    })
}
