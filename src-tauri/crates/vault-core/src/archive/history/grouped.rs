//! History search results with one row per page (`HistoryQuery::group_by_url`).
//!
//! A page visited 2,000 times would otherwise fill the first twenty screens of a search. Grouped,
//! each matching URL appears once: its most recent matching visit, plus how many visits matched.
//!
//! ## Responsibilities
//! - Keyword (FTS), fuzzy-fallback and operator-only search (starting from the tagged URLs when
//!   `tag:` is present), grouped by URL string, with the same browser, profile, date, domain and
//!   `site:`-style filters as the visit list. Regex grouping lives in `regex_scan`.
//! - Cursor paging over pages, and the totals: matching pages and matching visits.
//!
//! ## Not responsible for
//! - Browsing without search text. The timeline lists visits (`list_history_with_sql`).
//! - Query parsing, lexical analysis and fuzzy scoring (`search_query`, `search_lexical`).
//!
//! ## How it is computed
//! A page is a URL string. `urls` rows are per browser profile, so the same address in Chrome and
//! Firefox is two `url_id`s and one page. The SQL first aggregates each matching `url_id` over its
//! visible visits inside the filters (count, newest visit), reading `idx_visits_visible_url_time`,
//! then folds the `url_id`s of one URL together with window functions and keeps the newest.
//!
//! ## Ways this can go wrong, and what guards each
//! 1. The same page listed once per browser. Pages are keyed by `urls.url`, not `url_id`; a test
//!    visits one URL from two profiles and expects one row.
//! 2. The row shows a visit that is not the newest, or a title/browser from another visit. SQLite
//!    only ties bare columns to the `MAX()` row when the query has exactly one min/max aggregate,
//!    so the per-`url_id` step has only `MAX(visit_time_ms)` and the per-URL step picks its row
//!    with `ROW_NUMBER()`. A test gives the newer visit to the profile whose copy ranks worse.
//! 3. Counts or "newest" taken outside the date or browser filter. Both filters apply to visits
//!    inside the aggregate; a test filters to one browser and checks count and row.
//! 4. A cursor that skips or repeats a page when two pages tie on score and time. The URL breaks
//!    ties; a test walks one row at a time and compares with a single large page.
//! 5. The page total and the visit total disagree with the list. Both are computed from the same
//!    `url_matches` step; a test compares them with the walked rows.
//! 6. Work that grows with every visit in the archive instead of with the matches. A plan test
//!    requires the visit lookups to use `idx_visits_visible_url_time` and never scan `visits`.
//! 7. The URL filters here drift from the visit list's. They are the same text, checked by a test.
//! 8. Regex rows. Regex is scanned in chunks (`regex_scan`); a grouped row counts the visits of its
//!    chunk, and a page met again in a later chunk comes back as another row for the list to add up.
//! 9. A `tag:` search that starts from every URL. Tag-only searches start from `url_tags`; a plan
//!    test requires `urls` to be read through `idx_urls_url`, never scanned. A tag set is written
//!    by hand and small, so tag searches are not windowed.
//!
//! ## Performance notes
//! - Keyword search costs one FTS match plus one index range per matching `url_id`; the sort runs
//!   over matching pages, not visits. Measured at 14.4M visits in
//!   `docs/architecture/ipc-performance.md`.

use super::pagination::{PageCursor, build_page_response, parse_page_cursor};
use super::*;

/// The visit list's URL-level filters (domain and the `site:` / `intitle:` / `tag:` operators),
/// verbatim. `url_filters_match_the_visit_list` keeps this text equal to theirs.
macro_rules! page_url_filters {
    () => {
        r#"  AND (:domainPattern IS NULL OR urls.url LIKE :domainPattern)
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
"#
    };
}

/// The matching `url_id`s and their scores, collected first by `window::rank_matching_urls` (keyword
/// and operator-only searches) or by the fuzzy scorer.
macro_rules! ranked_urls {
    () => {
        "ranked_urls AS (SELECT url_id, score FROM temp.history_ranked_urls)"
    };
}

/// Tag-only searches (`tag:rust`, maybe with other operators but no words): only the URLs that
/// carry one of the required tags, found through `url_tags` (user-authored, small) and
/// `idx_urls_url`, instead of every URL in the archive. The page filters still require every
/// listed tag, so several `tag:` operators keep their AND meaning.
macro_rules! tagged_urls {
    () => {
        r#"ranked_urls AS (
  SELECT urls.id AS url_id, 0.0 AS score
  FROM urls
  WHERE urls.url IN (
    SELECT url_tags.url
    FROM url_tags
    JOIN temp.history_required_tags AS required_tag
      ON LOWER(url_tags.tag) = required_tag.value
  )
)"#
    };
}

pub(super) mod window;

/// One row per matching `url_id` that has visits inside the filters: how many, and the newest.
///
/// `visits.id` is a bare column next to the only min/max aggregate, so SQLite takes it from the
/// row with the newest `visit_time_ms`. Leave it the only one. `CROSS JOIN` keeps the ranked URLs
/// as the outer loop: the temp table has no statistics for the planner to go on.
macro_rules! url_matches {
    ($ranked_urls:expr) => {
        concat!(
            "WITH ",
            $ranked_urls,
            r#",
url_matches AS (
  SELECT
    urls.id AS url_id,
    urls.url AS url,
    ranked_urls.score AS score,
    COUNT(*) AS visit_count,
    MAX(visits.visit_time_ms) AS last_time,
    visits.id AS last_id
  FROM ranked_urls
  CROSS JOIN urls
    ON urls.id = ranked_urls.url_id
  CROSS JOIN visits
    ON visits.url_id = urls.id
  WHERE visits.reverted_at IS NULL
    AND visits.visit_time_ms >= :startTimeMs
    AND visits.visit_time_ms <= :endTimeMs
    AND (
      (:profileId IS NULL AND :browserKind IS NULL)
      OR visits.source_profile_id IN (
        SELECT source_profiles.id
        FROM source_profiles
        WHERE (:profileId IS NULL OR source_profiles.profile_key = :profileId)
          AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
      )
    )
"#,
            page_url_filters!(),
            "  GROUP BY urls.id\n)\n"
        )
    };
}

/// One list page: fold `url_id`s into pages, sort, cut at the cursor, then read the rows to show.
macro_rules! page_list {
    ($ranked_urls:expr) => {
        concat!(
            url_matches!($ranked_urls),
            r#",
pages AS (
  SELECT
    url_id,
    url,
    last_id,
    last_time,
    SUM(visit_count) OVER same_url AS page_visits,
    MIN(score) OVER same_url AS page_score,
    ROW_NUMBER() OVER (PARTITION BY url ORDER BY last_time DESC, last_id DESC) AS newest_first
  FROM url_matches
  WINDOW same_url AS (PARTITION BY url)
),
page_window AS (
  SELECT *
  FROM pages
  WHERE newest_first = 1
    AND (
      :cursorTime IS NULL
      OR (:sort = 'relevance' AND (page_score > :cursorScore OR (page_score = :cursorScore AND (last_time < :cursorTime OR (last_time = :cursorTime AND url > :cursorUrl)))))
      OR (:sort = 'newest' AND (last_time < :cursorTime OR (last_time = :cursorTime AND url > :cursorUrl)))
      OR (:sort = 'oldest' AND (last_time > :cursorTime OR (last_time = :cursorTime AND url > :cursorUrl)))
    )
  ORDER BY
    CASE WHEN :sort = 'relevance' THEN page_score END ASC,
    CASE WHEN :sort = 'oldest' THEN last_time END ASC,
    CASE WHEN :sort <> 'oldest' THEN last_time END DESC,
    url ASC
  LIMIT :pageLimit
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
  search_documents.enrichment_text,
  page_window.page_score,
  page_window.page_visits
FROM page_window
JOIN visits
  ON visits.id = page_window.last_id
JOIN urls
  ON urls.id = page_window.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
LEFT JOIN search.search_documents
  ON search_documents.url_id = page_window.url_id
ORDER BY
  CASE WHEN :sort = 'relevance' THEN page_window.page_score END ASC,
  CASE WHEN :sort = 'oldest' THEN page_window.last_time END ASC,
  CASE WHEN :sort <> 'oldest' THEN page_window.last_time END DESC,
  page_window.url ASC
"#
        )
    };
}

/// Matching pages and matching visits.
macro_rules! page_totals {
    ($ranked_urls:expr) => {
        concat!(
            url_matches!($ranked_urls),
            "SELECT COUNT(DISTINCT url), IFNULL(SUM(visit_count), 0) FROM url_matches\n"
        )
    };
}

const RANKED_PAGES_SQL: &str = page_list!(ranked_urls!());
const RANKED_PAGE_TOTALS_SQL: &str = page_totals!(ranked_urls!());
const TAGGED_PAGES_SQL: &str = page_list!(tagged_urls!());
const TAGGED_PAGE_TOTALS_SQL: &str = page_totals!(tagged_urls!());

/// Trigram candidates for the typo-tolerant fallback, with the fields the Rust scorer reads.
const FUZZY_URL_CANDIDATES_SQL: &str = r#"
SELECT
  history_search_trigram.rowid,
  search_documents.normalized_url,
  search_documents.normalized_title,
  search_documents.normalized_search_terms,
  search_documents.compact_text
FROM search.history_search_trigram
JOIN search.search_documents
  ON search_documents.url_id = history_search_trigram.rowid
WHERE history_search_trigram MATCH :fuzzyFtsQuery
ORDER BY bm25(history_search_trigram, 1.0) ASC
LIMIT :candidateUrlLimit
"#;

/// Filters that apply to every grouped search, already normalized by `list_history`.
pub(super) struct PageFilters {
    pub profile_id: Option<String>,
    pub browser_kind: Option<String>,
    pub domain_pattern: Option<String>,
    pub start_time_ms: Option<i64>,
    pub end_time_ms: Option<i64>,
}

/// One page of keyword results, falling back to typo-tolerant matching when nothing matches.
///
/// The matches are ranked inside a window of at most `cap` URLs (`window`); a windowed response says
/// so and its cursor keeps later pages in the same window.
#[allow(clippy::too_many_arguments)]
pub(super) fn list_keyword_pages(
    connection: &Connection,
    filters: &PageFilters,
    sort: &str,
    limit: usize,
    cursor: Option<&str>,
    include_total: bool,
    lexical_query: LexicalQuery,
    cap: usize,
) -> Result<HistoryQueryResponse> {
    let (edge, cursor) = window::split_cursor(cursor);
    let cursor = parse_page_cursor(cursor);
    let ranked = window::rank_matching_urls(
        connection,
        window::Matching::Words(&lexical_query),
        filters,
        sort,
        edge,
        cap,
    )?;
    if ranked.count > 0 || cursor.is_some() || lexical_query.fuzzy_query.is_none() {
        let totals = if include_total { Some(ranked_totals(connection, filters)?) } else { None };
        let response = ranked_page(connection, filters, sort, limit, cursor.as_ref(), totals)?;
        let first_page_empty = cursor.is_none() && response.items.is_empty();
        if !first_page_empty || lexical_query.fuzzy_query.is_none() {
            return Ok(ranked.label(response));
        }
    }
    match lexical_query.fuzzy_query {
        Some(fuzzy_query) => {
            list_fuzzy_pages(connection, filters, sort, limit, cursor.as_ref(), &fuzzy_query)
        }
        None => Ok(build_page_response(limit, false, false, Vec::new(), sort, None)),
    }
}

/// One page of an operator-only search (`site:`, `intitle:` and the like with no words), windowed
/// like keyword search: every URL is a candidate.
pub(super) fn list_operator_pages(
    connection: &Connection,
    filters: &PageFilters,
    sort: &str,
    limit: usize,
    cursor: Option<&str>,
    include_total: bool,
    cap: usize,
) -> Result<HistoryQueryResponse> {
    let (edge, cursor) = window::split_cursor(cursor);
    let ranked = window::rank_matching_urls(
        connection,
        window::Matching::EveryUrl,
        filters,
        sort,
        edge,
        cap,
    )?;
    let totals = if include_total { Some(ranked_totals(connection, filters)?) } else { None };
    let response =
        ranked_page(connection, filters, sort, limit, parse_page_cursor(cursor).as_ref(), totals)?;
    Ok(ranked.label(response))
}

/// Operator-only pages when a `tag:` operator is present: the candidates are the tagged URLs.
///
/// Costs one pass over `url_tags` plus an index range per tagged URL, so a tag filter stays fast
/// however large the archive is (the generic operator path starts from every URL).
pub(super) fn list_tagged_pages(
    connection: &Connection,
    filters: &PageFilters,
    sort: &str,
    limit: usize,
    cursor: Option<&str>,
    include_total: bool,
) -> Result<HistoryQueryResponse> {
    let totals = if include_total {
        Some(totals_of(connection, TAGGED_PAGE_TOTALS_SQL, filters)?)
    } else {
        None
    };
    let cursor = parse_page_cursor(cursor);
    page_of(connection, TAGGED_PAGES_SQL, filters, sort, limit, cursor.as_ref(), totals)
}

/// Typo-tolerant pages: score the bounded trigram candidates, then group them like keyword hits.
///
/// The candidate set is at most `FUZZY_CANDIDATE_URL_LIMIT` URLs, so the totals are always counted.
fn list_fuzzy_pages(
    connection: &Connection,
    filters: &PageFilters,
    sort: &str,
    limit: usize,
    cursor: Option<&PageCursor>,
    fuzzy_query: &FuzzyQuery,
) -> Result<HistoryQueryResponse> {
    connection.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS history_ranked_urls (
           url_id INTEGER PRIMARY KEY,
           score REAL NOT NULL
         );
         DELETE FROM temp.history_ranked_urls;",
    )?;
    {
        let mut candidates = connection.prepare(FUZZY_URL_CANDIDATES_SQL)?;
        let mut insert = connection
            .prepare("INSERT INTO temp.history_ranked_urls (url_id, score) VALUES (?1, ?2)")?;
        let mut rows = candidates.query(named_params! {
            ":fuzzyFtsQuery": fuzzy_query.candidate_query,
            ":candidateUrlLimit": FUZZY_CANDIDATE_URL_LIMIT,
        })?;
        while let Some(row) = rows.next()? {
            let url_id: i64 = row.get(0)?;
            let (url, title, terms, compact): (String, String, String, String) =
                (row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?);
            let document = FuzzyDocument {
                normalized_url: &url,
                normalized_title: &title,
                normalized_search_terms: &terms,
                compact_text: &compact,
            };
            if let Some(score) = fuzzy_query.score_document(&document) {
                insert.execute(params![url_id, score])?;
            }
        }
    }
    let totals = ranked_totals(connection, filters)?;
    ranked_page(connection, filters, sort, limit, cursor, Some(totals))
}

/// `(pages, visits)` among the ranked URLs inside the filters.
fn ranked_totals(connection: &Connection, filters: &PageFilters) -> Result<(usize, usize)> {
    totals_of(connection, RANKED_PAGE_TOTALS_SQL, filters)
}

/// The pages of the ranked URLs after `cursor`, reading one extra to know whether more follow.
fn ranked_page(
    connection: &Connection,
    filters: &PageFilters,
    sort: &str,
    limit: usize,
    cursor: Option<&PageCursor>,
    totals: Option<(usize, usize)>,
) -> Result<HistoryQueryResponse> {
    page_of(connection, RANKED_PAGES_SQL, filters, sort, limit, cursor, totals)
}

/// `(pages, visits)` from one `page_totals!` statement.
fn totals_of(connection: &Connection, sql: &str, filters: &PageFilters) -> Result<(usize, usize)> {
    let mut statement = connection.prepare_cached(sql)?;
    bind_filters(&mut statement, filters)?;
    let mut rows = statement.raw_query();
    let row = rows.next()?.context("page totals returned no row")?;
    let pages: i64 = row.get(0)?;
    let visits: i64 = row.get(1)?;
    Ok((usize::try_from(pages).unwrap_or(0), usize::try_from(visits).unwrap_or(0)))
}

/// One `page_list!` statement's rows after `cursor`, reading one extra to know whether more follow.
fn page_of(
    connection: &Connection,
    sql: &str,
    filters: &PageFilters,
    sort: &str,
    limit: usize,
    cursor: Option<&PageCursor>,
    totals: Option<(usize, usize)>,
) -> Result<HistoryQueryResponse> {
    let mut statement = connection.prepare_cached(sql)?;
    bind_filters(&mut statement, filters)?;
    bind(&mut statement, ":sort", &sort)?;
    bind(&mut statement, ":cursorScore", &cursor.and_then(|cursor| cursor.score))?;
    bind(&mut statement, ":cursorTime", &cursor.map(|cursor| cursor.last_time))?;
    bind(&mut statement, ":cursorUrl", &cursor.map(|cursor| cursor.url.as_str()))?;
    bind(&mut statement, ":pageLimit", &(i64::try_from(limit).unwrap_or(i64::MAX - 1) + 1))?;
    let mut rows = statement.raw_query();
    let mut items = Vec::with_capacity(limit + 1);
    while let Some(row) = rows.next()? {
        items.push(page_row(row)?);
    }
    let has_more = items.len() > limit;
    items.truncate(limit);
    Ok(build_page_response(limit, cursor.is_some(), has_more, items, sort, totals))
}

/// Binds the filter parameters every grouped statement shares.
fn bind_filters(statement: &mut rusqlite::Statement<'_>, filters: &PageFilters) -> Result<()> {
    bind(statement, ":profileId", &filters.profile_id)?;
    bind(statement, ":browserKind", &filters.browser_kind)?;
    bind(statement, ":domainPattern", &filters.domain_pattern)?;
    // Sentinels, not NULL, so the time bounds stay a plain index range.
    bind(statement, ":startTimeMs", &filters.start_time_ms.unwrap_or(i64::MIN))?;
    bind(statement, ":endTimeMs", &filters.end_time_ms.unwrap_or(i64::MAX))?;
    Ok(())
}

/// Binds `name` when the statement uses it; not every statement uses every filter.
fn bind(
    statement: &mut rusqlite::Statement<'_>,
    name: &str,
    value: &dyn rusqlite::ToSql,
) -> Result<()> {
    if let Some(index) = statement.parameter_index(name)? {
        statement.raw_bind_parameter(index, value)?;
    }
    Ok(())
}

/// Hydrates one grouped row: the newest visit, its enrichment excerpt, the page score and count.
fn page_row(row: &Row<'_>) -> rusqlite::Result<(HistoryEntry, f64)> {
    let mut entry = history_entry_from_row(row)?;
    entry.enrichment_excerpt =
        row.get::<_, Option<String>>(9)?.as_deref().and_then(cap_enrichment_excerpt);
    entry.visit_count = Some(u64::try_from(row.get::<_, i64>(11)?).unwrap_or(0));
    Ok((entry, row.get(10)?))
}

#[cfg(test)]
pub(super) mod tests;
