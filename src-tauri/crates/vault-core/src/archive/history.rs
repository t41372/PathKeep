//! Archive history query and export flows.
//!
//! This module owns the Explorer-facing read surface over canonical visits. It
//! keeps three recall modes explicit:
//!
//! - regular SQL filtering
//! - FTS-backed keyword recall, ranked inside a bounded window of matches (`grouped/window.rs`)
//! - manual regex matching, scanned in time-boxed chunks across the whole archive (`regex_scan`)
//!
//! Searches can also return one row per page instead of one per visit (`group_by_url`); that
//! lives in `grouped`.
//!
//! The accepted product contract is that only visible facts participate in
//! recall/export. Hidden/reverted rows stay out of these surfaces, and regex is
//! a slower manual path instead of pretending to be the default fast query
//! engine.

use super::search_lexical::{FuzzyDocument, FuzzyQuery, LexicalQuery, analyze_query};
use super::search_query::{ParsedHistorySearchQuery, parse_history_search_query};
use super::*;

pub mod day_insights;
mod export;
mod favicons;
mod grouped;
// `net_guard` is the SSRF guard reused by W-ENRICH-1's content-fetch egress (06 §2b: every page URL
// AND every API sub-resource is checked). Promoted to `pub(crate)` so the enrichment plane can reach
// `url_target_is_blocked` through the same chokepoint og:image fetching uses.
pub(crate) mod net_guard;
pub mod og_images;
pub mod og_images_fetch;
pub mod og_images_synth;
mod pagination;
mod regex_scan;

pub use self::day_insights::{
    BrowseDayInsights, BrowseDayInsightsRequest, BrowseDaySearchQuery, BrowseDayTopDomain,
    BrowseDayTopUrl, get_browse_day_insights,
};
pub use self::export::{
    ExportCancelled, cancel_export, export_history, get_export_progress, record_export_failure,
};
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
    HistoryCursor, build_history_response, build_lexical_history_response,
    build_uncounted_history_response, normalize_history_sort, page_count, parse_history_cursor,
};

/// Visits of the matching URLs, which `grouped::window::rank_matching_urls` collected (with their
/// scores, inside the window) into `temp.history_ranked_urls` first.
///
/// `CROSS JOIN` fixes the join order: the temp table has no statistics, and without it SQLite may
/// walk every visit in time order and probe the table instead of reading each ranked URL's visits.
const LIST_HISTORY_LEXICAL_SQL: &str = r#"
WITH ranked_urls AS (
  SELECT url_id, score FROM temp.history_ranked_urls
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
  -- The matched URL's capped enrichment summary + metadata (06 §6). A LEFT JOIN keeps the row even
  -- when the projection has no doc (NULL → no excerpt); this is the SAME row the recall already
  -- scored, so surfacing it costs ONE extra projection-keyed lookup per result, never an N+1.
  search_documents.enrichment_text,
  ranked_urls.score
FROM ranked_urls
CROSS JOIN urls
  ON urls.id = ranked_urls.url_id
CROSS JOIN visits
  ON visits.url_id = urls.id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
LEFT JOIN search.search_documents
  ON search_documents.url_id = urls.id
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
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_tags AS advanced_filter WHERE NOT EXISTS (SELECT 1 FROM url_tags WHERE url_tags.url = urls.url AND LOWER(url_tags.tag) = advanced_filter.value))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_tags AS advanced_filter WHERE EXISTS (SELECT 1 FROM url_tags WHERE url_tags.url = urls.url AND LOWER(url_tags.tag) = advanced_filter.value))
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_notes AS advanced_filter WHERE NOT EXISTS (SELECT 1 FROM url_annotations WHERE url_annotations.url = urls.url AND LOWER(url_annotations.notes) LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_notes AS advanced_filter WHERE EXISTS (SELECT 1 FROM url_annotations WHERE url_annotations.url = urls.url AND LOWER(url_annotations.notes) LIKE '%' || advanced_filter.value || '%'))
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
    "history_required_tags",
    "history_excluded_tags",
    "history_required_notes",
    "history_excluded_notes",
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
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_tags AS advanced_filter WHERE NOT EXISTS (SELECT 1 FROM url_tags WHERE url_tags.url = urls.url AND LOWER(url_tags.tag) = advanced_filter.value))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_tags AS advanced_filter WHERE EXISTS (SELECT 1 FROM url_tags WHERE url_tags.url = urls.url AND LOWER(url_tags.tag) = advanced_filter.value))
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_notes AS advanced_filter WHERE NOT EXISTS (SELECT 1 FROM url_annotations WHERE url_annotations.url = urls.url AND LOWER(url_annotations.notes) LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_notes AS advanced_filter WHERE EXISTS (SELECT 1 FROM url_annotations WHERE url_annotations.url = urls.url AND LOWER(url_annotations.notes) LIKE '%' || advanced_filter.value || '%'))
  AND (:startTimeMs IS NULL OR visits.visit_time_ms >= :startTimeMs)
  AND (:endTimeMs IS NULL OR visits.visit_time_ms <= :endTimeMs)
ORDER BY
  fuzzy_url_candidates.fts_score ASC,
  visits.visit_time_ms DESC,
  visits.id DESC
LIMIT :candidateVisitLimit
"#;

/// Counts the visits of the ranked URLs, so a windowed search counts only its window.
const COUNT_HISTORY_LEXICAL_SQL: &str = r#"
WITH ranked_urls AS (
  SELECT url_id FROM temp.history_ranked_urls
)
SELECT COUNT(*)
FROM ranked_urls
CROSS JOIN urls
  ON urls.id = ranked_urls.url_id
CROSS JOIN visits
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
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_tags AS advanced_filter WHERE NOT EXISTS (SELECT 1 FROM url_tags WHERE url_tags.url = urls.url AND LOWER(url_tags.tag) = advanced_filter.value))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_tags AS advanced_filter WHERE EXISTS (SELECT 1 FROM url_tags WHERE url_tags.url = urls.url AND LOWER(url_tags.tag) = advanced_filter.value))
  AND NOT EXISTS (SELECT 1 FROM temp.history_required_notes AS advanced_filter WHERE NOT EXISTS (SELECT 1 FROM url_annotations WHERE url_annotations.url = urls.url AND LOWER(url_annotations.notes) LIKE '%' || advanced_filter.value || '%'))
  AND NOT EXISTS (SELECT 1 FROM temp.history_excluded_notes AS advanced_filter WHERE EXISTS (SELECT 1 FROM url_annotations WHERE url_annotations.url = urls.url AND LOWER(url_annotations.notes) LIKE '%' || advanced_filter.value || '%'))
  AND (:startTimeMs IS NULL OR visits.visit_time_ms >= :startTimeMs)
  AND (:endTimeMs IS NULL OR visits.visit_time_ms <= :endTimeMs)
"#;

/// Compiles a History regex: Rust `regex` syntax, case-insensitive.
///
/// The one place the dialect is decided. The History screen checks patterns against the same
/// rules before sending them (`src/features/history/regex-dialect.ts`); the shared case table
/// `regex_dialect_cases` keeps the two in step.
pub(crate) fn build_history_regex(pattern: &str) -> Result<regex::Regex> {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .with_context(|| format!("invalid regex pattern `{pattern}`"))
}

/// Queries visible history rows with pagination, FTS, and regex support.
pub fn list_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    list_history_with_window(paths, config, key, query, grouped::window::keyword_window_cap())
}

/// [`list_history`] with the keyword window cap as a parameter, so tests can hit the window with a
/// handful of rows.
fn list_history_with_window(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: HistoryQuery,
    window_cap: usize,
) -> Result<HistoryQueryResponse> {
    let connection = open_archive_connection(paths, config, key)?;
    list_history_on_connection_with_window(&connection, query, window_cap)
}

// Export reuses this dispatch on its one connection; recall paths and filters stay shared.
fn list_history_on_connection(
    connection: &Connection,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    list_history_on_connection_with_window(connection, query, grouped::window::keyword_window_cap())
}

fn list_history_on_connection_with_window(
    connection: &Connection,
    query: HistoryQuery,
    window_cap: usize,
) -> Result<HistoryQueryResponse> {
    let limit = query.limit.unwrap_or(150).clamp(1, 1_000);
    let limit_usize = limit as usize;
    let requested_page = query.page.map(|page| usize::try_from(page.max(1)).unwrap_or(usize::MAX));
    let profile_id = query.profile_id.clone();
    let browser_kind = query.browser_kind.clone();
    let raw_q = query.q.clone().filter(|value| !value.trim().is_empty());
    let regex_mode = query.regex_mode.unwrap_or(false);
    let include_total = query.include_total.unwrap_or(true);
    let parsed_query = if regex_mode {
        ParsedHistorySearchQuery::default()
    } else {
        raw_q.as_deref().map(parse_history_search_query).unwrap_or_default()
    };
    prepare_advanced_search_filters(connection, &parsed_query)?;
    let start_time_ms = max_optional_i64(query.start_time_ms, parsed_query.after_ms);
    let end_time_ms = min_optional_i64(query.end_time_ms, parsed_query.before_ms);
    let q = if regex_mode { raw_q.clone() } else { parsed_query.keyword_text.clone() };
    let lexical_query = q.as_deref().and_then(analyze_query);
    let regex = if regex_mode { q.as_deref().map(build_history_regex).transpose()? } else { None };
    let domain_pattern = query
        .domain
        .clone()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{value}%"));
    let sort = normalize_history_sort(query.sort.as_deref(), q.is_some(), lexical_query.is_some());

    if query.group_by_url.unwrap_or(false) && raw_q.is_some() {
        let filters = grouped::PageFilters {
            profile_id,
            browser_kind,
            domain_pattern,
            start_time_ms,
            end_time_ms,
        };
        let cursor = query.cursor.as_deref();
        return match (regex, lexical_query) {
            (Some(regex), _) => regex_scan::scan_regex(
                connection,
                &regex_scan::RegexScan {
                    regex: &regex,
                    filters: &filters,
                    sort: &sort,
                    limit: limit_usize,
                    cursor,
                    requested_page: None,
                    grouped: true,
                },
                regex_scan::ScanBudget::chunk(),
            ),
            (None, Some(lexical_query)) => grouped::list_keyword_pages(
                connection,
                &filters,
                &sort,
                limit_usize,
                cursor,
                include_total,
                lexical_query,
                window_cap,
            ),
            // Words that analyze to nothing searchable match nothing, as in the visit list.
            (None, None) if q.is_some() => Ok(HistoryQueryResponse::default()),
            (None, None) if !parsed_query.required_tags.is_empty() => grouped::list_tagged_pages(
                connection,
                &filters,
                &sort,
                limit_usize,
                cursor,
                include_total,
            ),
            (None, None) => grouped::list_operator_pages(
                connection,
                &filters,
                &sort,
                limit_usize,
                cursor,
                include_total,
                window_cap,
            ),
        };
    }

    let (window_edge, visit_cursor) = grouped::window::split_cursor(query.cursor.as_deref());
    let cursor = parse_history_cursor(visit_cursor);

    if let Some(regex) = regex {
        let filters = grouped::PageFilters {
            profile_id,
            browser_kind,
            domain_pattern,
            start_time_ms,
            end_time_ms,
        };
        return regex_scan::scan_regex(
            connection,
            &regex_scan::RegexScan {
                regex: &regex,
                filters: &filters,
                sort: &sort,
                limit: limit_usize,
                cursor: query.cursor.as_deref(),
                requested_page,
                grouped: false,
            },
            regex_scan::ScanBudget::chunk(),
        );
    }

    if q.is_some() && lexical_query.is_none() {
        return Ok(HistoryQueryResponse::default());
    }

    if let Some(lexical_query) = lexical_query {
        return list_history_with_lexical_search(
            connection,
            include_total,
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
            LexicalWindow { edge: window_edge, cap: window_cap },
        );
    }

    let (cursor_visit_time, cursor_id) =
        cursor.and_then(HistoryCursor::chronological).unwrap_or((0, 0));

    list_history_with_sql(
        connection,
        include_total,
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
    insert_filter_values(connection, "history_required_tags", &parsed_query.required_tags)?;
    insert_filter_values(connection, "history_excluded_tags", &parsed_query.excluded_tags)?;
    insert_filter_values(connection, "history_required_notes", &parsed_query.required_notes)?;
    insert_filter_values(connection, "history_excluded_notes", &parsed_query.excluded_notes)?;
    Ok(())
}

fn insert_filter_values(connection: &Connection, table: &str, values: &[String]) -> Result<()> {
    let mut statement = connection.prepare(&format!("INSERT INTO {table} (value) VALUES (?1)"))?;
    for value in values {
        statement.execute(params![value])?;
    }
    Ok(())
}

/// The keyword window of one visit-list request: the cap, and the edge a cursor pinned.
struct LexicalWindow {
    edge: Option<grouped::window::WindowEdge>,
    cap: usize,
}

/// Runs normalized FTS-backed keyword recall over the matches `rank_matching_urls` keeps.
#[allow(clippy::too_many_arguments)]
fn list_history_with_lexical_search(
    connection: &Connection,
    include_total: bool,
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
    window: LexicalWindow,
) -> Result<HistoryQueryResponse> {
    let ranked = grouped::window::rank_matching_urls(
        connection,
        grouped::window::Matching::Words(&lexical_query),
        &grouped::PageFilters {
            profile_id: profile_id.clone(),
            browser_kind: browser_kind.clone(),
            domain_pattern: domain_pattern.clone(),
            start_time_ms,
            end_time_ms,
        },
        &sort,
        window.edge,
        window.cap,
    )?;
    let fuzzy_query = lexical_query.fuzzy_query.clone();
    let total: Option<usize> = if include_total {
        let count: usize = connection
            .query_row(
                COUNT_HISTORY_LEXICAL_SQL,
                named_params! {
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
        Some(count)
    } else {
        None
    };

    if total == Some(0)
        && let Some(fuzzy_query) = fuzzy_query.clone()
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
    let page = match total {
        Some(total) => requested_page.unwrap_or(1).min(page_count(total, limit_usize)),
        None => requested_page.unwrap_or(1),
    };
    let start_index = page.saturating_sub(1) * limit_usize;
    // Without a total, always read one extra row so `has_next` needs no count query.
    let page_limit = if requested_page.is_some() && total.is_some() {
        i64::from(limit)
    } else {
        i64::from(limit) + 1
    };
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
            ":profileId": profile_id.clone(),
            ":browserKind": browser_kind.clone(),
            ":domainPattern": domain_pattern.clone(),
            ":startTimeMs": start_time_ms,
            ":endTimeMs": end_time_ms,
            ":sort": sort.clone(),
            ":cursorVisitTime": if requested_page.is_some() { Option::<i64>::None } else { cursor_visit_time },
            ":cursorId": if requested_page.is_some() { Option::<i64>::None } else { cursor_id },
            ":cursorScore": if requested_page.is_some() { Option::<f64>::None } else { cursor_score },
            ":pageLimit": page_limit,
            ":pageOffset": page_offset,
        },
        history_entry_with_score_from_row,
    )?;
    let mut scored_items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = scored_items.len() > limit_usize;
    if requested_page.is_none() || total.is_none() {
        scored_items.truncate(limit_usize);
    }
    let response_start_index = if requested_page.is_some() {
        start_index
    } else if chronological_cursor.is_some() || relevance_cursor.is_some() {
        limit_usize
    } else {
        0
    };

    let Some(total) = total else {
        // An empty first page with no total to consult may still deserve typo-tolerant recall.
        if scored_items.is_empty()
            && response_start_index == 0
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
        return Ok(ranked.label(build_uncounted_history_response(
            limit_usize,
            response_start_index,
            has_more,
            scored_items,
            &sort,
        )));
    };

    Ok(ranked.label(build_lexical_history_response(
        total,
        limit_usize,
        page,
        response_start_index,
        scored_items,
        &sort,
    )))
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
    include_total: bool,
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
) -> Result<HistoryQueryResponse> {
    let total: Option<usize> = if include_total {
        let count: usize = connection
            .query_row(
                COUNT_HISTORY_SQL,
                named_params! {
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
        Some(count)
    } else {
        None
    };

    let mut statement = connection.prepare(list_history_sql(&sort))?;
    let page = match total {
        Some(total) => requested_page.unwrap_or(1).min(page_count(total, limit_usize)),
        None => requested_page.unwrap_or(1),
    };
    let start_index = page.saturating_sub(1) * limit_usize;
    // Without a total, always read one extra row so `has_next` needs no count query.
    let page_limit = if requested_page.is_some() && total.is_some() {
        i64::from(limit)
    } else {
        i64::from(limit) + 1
    };
    let page_offset =
        if requested_page.is_some() { i64::try_from(start_index).unwrap_or(i64::MAX) } else { 0 };
    let page_cursor =
        (requested_page.is_none() && cursor.is_some()).then_some((cursor_visit_time, cursor_id));
    let (start_bound, end_bound, cursor_time_bound, cursor_id_bound) =
        list_history_bounds(&sort, start_time_ms, end_time_ms, page_cursor);
    let cursor_bound = (cursor_time_bound, cursor_id_bound);
    let rows = statement.query_map(
        named_params! {
            ":profileId": profile_id,
            ":browserKind": browser_kind,
            ":domainPattern": domain_pattern,
            ":startTimeMs": start_bound,
            ":endTimeMs": end_bound,
            ":cursorVisitTime": cursor_bound.0,
            ":cursorId": cursor_bound.1,
            ":pageLimit": page_limit,
            ":pageOffset": page_offset,
        },
        history_entry_from_row,
    )?;
    let mut items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = items.len() > limit_usize;
    if requested_page.is_none() || total.is_none() {
        items.truncate(limit_usize);
    }
    let response_start_index = if requested_page.is_some() {
        start_index
    } else if cursor.is_some() {
        limit_usize
    } else {
        0
    };

    match total {
        Some(total) => {
            Ok(build_history_response(total, limit_usize, page, response_start_index, items))
        }
        None => Ok(build_uncounted_history_response(
            limit_usize,
            response_start_index,
            has_more,
            items.into_iter().map(|entry| (entry, 0.0)).collect(),
            "newest",
        )),
    }
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
        // The shared hydration carries NO enrichment excerpt: browse / regex / fuzzy rows leave it
        // `None`. Only the lexical-search reader attaches one (see `history_entry_with_score_from_row`),
        // so the Explorer affordance stays suppressed outside keyword search.
        enrichment_excerpt: None,
        visit_count: None,
    })
}

/// Maximum characters surfaced in a lexical-search enrichment excerpt (06 §6).
///
/// A single-line teaser, not the full summary: the FE clamps to two lines and highlights query terms,
/// so a tight char cap keeps the search response payload bounded at the 14.4M tail (one capped string
/// per RESULT, not per visit) without an extra scan. 180 sits inside the prompt's ~160-200 guidance.
const ENRICHMENT_EXCERPT_MAX_CHARS: usize = 180;

/// Caps one enrichment summary into a search-result excerpt on a CHAR boundary (CJK-safe).
///
/// PURE → unit-tested. Empty/whitespace text yields `None` (the FE suppresses a blank affordance).
/// Text already within budget is returned verbatim; longer text is sliced by CHARACTER count — never
/// by byte index, so a multi-byte CJK or emoji codepoint can never be split — and gets a trailing
/// ellipsis. Mirrors the char-count cap pattern used by `enrichment::truncate_text` /
/// `agent_store` auto-titles rather than inventing a new truncation idiom.
pub(crate) fn cap_enrichment_excerpt(enrichment_text: &str) -> Option<String> {
    let trimmed = enrichment_text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().count() <= ENRICHMENT_EXCERPT_MAX_CHARS {
        return Some(trimmed.to_string());
    }
    let capped: String = trimmed.chars().take(ENRICHMENT_EXCERPT_MAX_CHARS).collect();
    Some(format!("{}…", capped.trim_end()))
}

/// Hydrates one lexical-search row: the shared entry plus its relevance score, and — uniquely on this
/// path — a capped excerpt of the matched URL's enrichment text (column 9; the score is column 10).
fn history_entry_with_score_from_row(row: &Row<'_>) -> rusqlite::Result<(HistoryEntry, f64)> {
    let mut entry = history_entry_from_row(row)?;
    entry.enrichment_excerpt =
        row.get::<_, Option<String>>(9)?.as_deref().and_then(cap_enrichment_excerpt);
    Ok((entry, row.get(10)?))
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

#[cfg(test)]
mod excerpt_tests {
    use super::{ENRICHMENT_EXCERPT_MAX_CHARS, cap_enrichment_excerpt};

    #[test]
    fn cap_enrichment_excerpt_suppresses_blank_and_keeps_short_text_verbatim() {
        // Empty / whitespace-only text yields None so the FE suppresses an empty affordance.
        assert_eq!(cap_enrichment_excerpt(""), None);
        assert_eq!(cap_enrichment_excerpt("   \t\n "), None);
        // Surrounding whitespace is trimmed, but in-budget text is otherwise returned verbatim.
        assert_eq!(
            cap_enrichment_excerpt("  Reusable workflow runner  "),
            Some("Reusable workflow runner".to_string())
        );
        // Text sitting exactly on the budget is returned without an ellipsis.
        let at_budget = "x".repeat(ENRICHMENT_EXCERPT_MAX_CHARS);
        assert_eq!(cap_enrichment_excerpt(&at_budget), Some(at_budget));
    }

    #[test]
    fn cap_enrichment_excerpt_truncates_over_budget_and_trims_before_ellipsis() {
        // One char over budget triggers truncation; the trailing space left at the cut is trimmed
        // before the ellipsis so the excerpt never reads "word …".
        let mut over_budget = "a".repeat(ENRICHMENT_EXCERPT_MAX_CHARS - 1);
        over_budget.push(' '); // char #180 (the cut point) is a space → trimmed off.
        over_budget.push('b'); // pushes the input over the cap so truncation engages.
        let capped = cap_enrichment_excerpt(&over_budget).expect("truncated excerpt");
        // (180 chars taken − 1 trailing space trimmed) + the ellipsis = 180 chars.
        assert_eq!(capped.chars().count(), ENRICHMENT_EXCERPT_MAX_CHARS);
        assert!(capped.ends_with('…'));
        assert!(!capped.ends_with(" …"), "the trailing space must be trimmed before the ellipsis");
    }
}

#[cfg(test)]
mod list_plan_tests {
    use super::*;
    use crate::config::project_paths_with_root;

    #[test]
    fn the_history_list_walks_the_time_index_instead_of_sorting() {
        // Sorting every visible visit to return one page took ~0.9 s at 1M visits. Both sort
        // directions, with and without a cursor, must read `idx_visits_visible_time_id` in order.
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let connection =
            open_archive_connection(&paths, &AppConfig::default(), None).expect("open archive");
        prepare_advanced_search_filters(&connection, &ParsedHistorySearchQuery::default())
            .expect("filter tables");
        for sort in ["newest", "oldest"] {
            for cursor in [None, Some((1_700_000_000_000, 42))] {
                let (start, end, cursor_time, cursor_id) =
                    list_history_bounds(sort, None, None, cursor);
                let mut statement = connection
                    .prepare(&format!("EXPLAIN QUERY PLAN {}", list_history_sql(sort)))
                    .expect("plan");
                let plan = statement
                    .query_map(
                        named_params! {
                            ":profileId": Option::<String>::None,
                            ":browserKind": Option::<String>::None,
                            ":domainPattern": Option::<String>::None,
                            ":startTimeMs": start,
                            ":endTimeMs": end,
                            ":cursorVisitTime": cursor_time,
                            ":cursorId": cursor_id,
                            ":pageLimit": 101,
                            ":pageOffset": 0,
                        },
                        |row| row.get::<_, String>(3),
                    )
                    .expect("plan rows")
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .expect("plan text")
                    .join("\n");
                assert!(
                    plan.contains("idx_visits_visible_time_id") && !plan.contains("TEMP B-TREE"),
                    "{sort} with cursor {cursor:?} must walk the time index, got:\n{plan}"
                );
            }
        }
    }
}

#[cfg(test)]
mod regex_dialect_cases {
    use super::build_history_regex;

    /// The patterns the History screen's dialect check is tested against, with whether Rust
    /// accepts each. If a `regex` upgrade changes a verdict, this fails and the screen's check
    /// (`regex-dialect.ts`) has to follow.
    const CASES: &str =
        include_str!("../../../../../src/features/history/regex-dialect-cases.json");

    #[test]
    fn the_screen_and_the_backend_agree_on_the_regex_dialect() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(CASES).expect("case table");
        assert!(cases.len() > 40, "the case table went missing");
        for case in cases {
            let pattern = case["pattern"].as_str().expect("pattern");
            let accepted = case["accepted"].as_bool().expect("accepted");
            assert_eq!(build_history_regex(pattern).is_ok(), accepted, "pattern {pattern:?}");
        }
    }
}
