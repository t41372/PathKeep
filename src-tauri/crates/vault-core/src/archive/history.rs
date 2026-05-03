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

use super::search_lexical::{LexicalQuery, analyze_query};
use super::*;
use std::collections::{HashMap, HashSet};

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
  AND (:startTimeMs IS NULL OR visits.visit_time_ms >= :startTimeMs)
  AND (:endTimeMs IS NULL OR visits.visit_time_ms <= :endTimeMs)
"#;

pub(super) const LOAD_FAVICON_PROFILE_SQL: &str = r#"
SELECT id
FROM source_profiles
WHERE profile_key = ?1
LIMIT 1
"#;

pub(super) const LOAD_FAVICON_SAME_PROFILE_PAGE_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_recall_lookup
  WHERE source_profile_id = ?1
    AND page_url = ?2
    AND (?3 <= 0 OR last_updated_ms <= ?3)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

pub(super) const LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_page_lookup
  WHERE source_profile_id != ?1
    AND page_url = ?2
    AND (?3 <= 0 OR last_updated_ms <= ?3)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

pub(super) const LOAD_FAVICON_SAME_PROFILE_HOST_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_host_profile_lookup
  WHERE source_profile_id = ?1
    AND page_host = ?2
    AND page_url != ?3
    AND (?4 <= 0 OR last_updated_ms <= ?4)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

pub(super) const LOAD_FAVICON_CROSS_PROFILE_HOST_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_host_lookup
  WHERE source_profile_id != ?1
    AND page_host = ?2
    AND page_url != ?3
    AND (?4 <= 0 OR last_updated_ms <= ?4)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

pub(super) const LOAD_FAVICON_SAME_PROFILE_REGISTRABLE_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_registrable_profile_lookup
  WHERE source_profile_id = ?1
    AND page_registrable_domain = ?2
    AND page_url != ?3
    AND (?4 <= 0 OR last_updated_ms <= ?4)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

pub(super) const LOAD_FAVICON_CROSS_PROFILE_REGISTRABLE_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_registrable_lookup
  WHERE source_profile_id != ?1
    AND page_registrable_domain = ?2
    AND page_url != ?3
    AND (?4 <= 0 OR last_updated_ms <= ?4)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
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
    let start_time_ms = query.start_time_ms;
    let end_time_ms = query.end_time_ms;
    let q = query.q.clone().filter(|value| !value.trim().is_empty());
    let lexical_query = q.as_deref().and_then(analyze_query);
    let regex = if query.regex_mode.unwrap_or(false) {
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

/// Loads favicon payloads for already-visible Explorer rows after the main
/// history page has rendered.
pub fn load_history_favicons(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    entries: Vec<HistoryFaviconLookupEntry>,
) -> Result<Vec<HistoryFaviconLookupResult>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let connection = open_archive_connection(paths, config, key)?;
    let mut profile_statement = connection.prepare(LOAD_FAVICON_PROFILE_SQL)?;
    let mut same_profile_page_statement = connection.prepare(LOAD_FAVICON_SAME_PROFILE_PAGE_SQL)?;
    let mut cross_profile_page_statement =
        connection.prepare(LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL)?;
    let mut same_profile_host_statement = connection.prepare(LOAD_FAVICON_SAME_PROFILE_HOST_SQL)?;
    let mut cross_profile_host_statement =
        connection.prepare(LOAD_FAVICON_CROSS_PROFILE_HOST_SQL)?;
    let mut same_profile_registrable_statement =
        connection.prepare(LOAD_FAVICON_SAME_PROFILE_REGISTRABLE_SQL)?;
    let mut cross_profile_registrable_statement =
        connection.prepare(LOAD_FAVICON_CROSS_PROFILE_REGISTRABLE_SQL)?;
    let mut profile_ids = HashMap::<String, Option<i64>>::new();
    let mut seen = HashSet::new();
    let mut results = Vec::with_capacity(entries.len());

    for entry in entries {
        let cache_key = format!("{}\n{}\n{}", entry.profile_id, entry.url, entry.visit_time);
        if !seen.insert(cache_key) {
            continue;
        }

        let metadata = super::schema::favicon_url_metadata(&entry.url);
        let source_profile_id = match profile_ids.get(&entry.profile_id) {
            Some(profile_id) => *profile_id,
            None => {
                let profile_id = profile_statement
                    .query_row([&entry.profile_id], |row| row.get::<_, i64>(0))
                    .optional()?;
                profile_ids.insert(entry.profile_id.clone(), profile_id);
                profile_id
            }
        };
        let image_data = if let Some(source_profile_id) = source_profile_id {
            load_entry_favicon(
                &mut same_profile_page_statement,
                &mut cross_profile_page_statement,
                &mut same_profile_host_statement,
                &mut cross_profile_host_statement,
                &mut same_profile_registrable_statement,
                &mut cross_profile_registrable_statement,
                source_profile_id,
                &entry.url,
                entry.visit_time,
                metadata.host.as_deref(),
                metadata.registrable_domain.as_deref(),
            )?
        } else {
            None
        };

        results.push(HistoryFaviconLookupResult {
            profile_id: entry.profile_id,
            url: entry.url,
            visit_time: entry.visit_time,
            favicon: image_data
                .as_deref()
                .and_then(image_data_to_data_url)
                .map(|data_url| HistoryFavicon { data_url }),
        });
    }

    Ok(results)
}

#[allow(clippy::too_many_arguments)]
fn load_entry_favicon(
    same_profile_page_statement: &mut rusqlite::Statement<'_>,
    cross_profile_page_statement: &mut rusqlite::Statement<'_>,
    same_profile_host_statement: &mut rusqlite::Statement<'_>,
    cross_profile_host_statement: &mut rusqlite::Statement<'_>,
    same_profile_registrable_statement: &mut rusqlite::Statement<'_>,
    cross_profile_registrable_statement: &mut rusqlite::Statement<'_>,
    source_profile_id: i64,
    url: &str,
    visit_time: i64,
    host: Option<&str>,
    registrable_domain: Option<&str>,
) -> Result<Option<Vec<u8>>> {
    if let Some(image_data) = query_favicon_statement(
        same_profile_page_statement,
        params![source_profile_id, url, visit_time],
    )? {
        return Ok(Some(image_data));
    }
    if let Some(image_data) = query_favicon_statement(
        cross_profile_page_statement,
        params![source_profile_id, url, visit_time],
    )? {
        return Ok(Some(image_data));
    }

    if let Some(host) = host {
        if let Some(image_data) = query_favicon_statement(
            same_profile_host_statement,
            params![source_profile_id, host, url, visit_time],
        )? {
            return Ok(Some(image_data));
        }
        if let Some(image_data) = query_favicon_statement(
            cross_profile_host_statement,
            params![source_profile_id, host, url, visit_time],
        )? {
            return Ok(Some(image_data));
        }
    }

    if let Some(registrable_domain) = registrable_domain.filter(|domain| Some(*domain) != host) {
        if let Some(image_data) = query_favicon_statement(
            same_profile_registrable_statement,
            params![source_profile_id, registrable_domain, url, visit_time],
        )? {
            return Ok(Some(image_data));
        }
        if let Some(image_data) = query_favicon_statement(
            cross_profile_registrable_statement,
            params![source_profile_id, registrable_domain, url, visit_time],
        )? {
            return Ok(Some(image_data));
        }
    }

    Ok(None)
}

fn query_favicon_statement<P: rusqlite::Params>(
    statement: &mut rusqlite::Statement<'_>,
    params: P,
) -> Result<Option<Vec<u8>>> {
    Ok(statement.query_row(params, |row| row.get::<_, Option<Vec<u8>>>(0)).optional()?.flatten())
}

/// Re-queries history until all visible matches are collected for export.
fn collect_history_for_export(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let mut export_query = query;
    // Export should always walk the full visible result set, not stay pinned to
    // whichever UI page happened to be open when the user clicked export.
    export_query.page = None;
    export_query.cursor = None;
    export_query.limit = Some(1_000);

    let mut items = Vec::new();

    let total = loop {
        let page = list_history(paths, config, key, export_query.clone())?;
        let total = page.total;
        let next_cursor = page.next_cursor.clone();
        items.extend(page.items);

        let Some(next_cursor) = next_cursor else {
            break total;
        };

        export_query.cursor = Some(next_cursor);
    };

    Ok(HistoryQueryResponse {
        total,
        page: 1,
        page_size: items.len(),
        page_count: 1,
        has_previous: false,
        has_next: false,
        items,
        next_cursor: None,
    })
}

/// Exports the currently visible query result set to one local artifact file.
pub fn export_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: ExportRequest,
) -> Result<ExportResult> {
    let results = collect_history_for_export(paths, config, key, request.query)?;
    fs::create_dir_all(&paths.exports_dir)?;
    let format = request.format;
    let extension = match format {
        ExportFormat::Html => "html",
        ExportFormat::Markdown => "md",
        ExportFormat::Text => "txt",
        ExportFormat::Jsonl => "jsonl",
    };
    let file_name = format!("export-{}.{}", now_rfc3339().replace(':', "-"), extension);
    let target_path = paths.exports_dir.join(file_name);
    let content = render_export_content(&results, &format)?;
    fs::write(&target_path, content)
        .with_context(|| format!("writing {}", target_path.display()))?;
    Ok(ExportResult { format, path: target_path.display().to_string(), count: results.items.len() })
}

#[derive(Clone, Copy)]
enum HistoryCursor {
    Chronological { visit_time: i64, id: i64 },
    Relevance { score: f64, visit_time: i64, id: i64 },
}

impl HistoryCursor {
    fn chronological(self) -> Option<(i64, i64)> {
        match self {
            HistoryCursor::Chronological { visit_time, id }
            | HistoryCursor::Relevance { visit_time, id, .. } => Some((visit_time, id)),
        }
    }

    fn relevance(self) -> Option<(f64, i64, i64)> {
        match self {
            HistoryCursor::Relevance { score, visit_time, id } => Some((score, visit_time, id)),
            HistoryCursor::Chronological { .. } => None,
        }
    }
}

/// Normalizes the public sort string into the backend ordering contract.
fn normalize_history_sort(
    requested_sort: Option<&str>,
    has_query: bool,
    has_lexical_query: bool,
) -> String {
    match requested_sort {
        Some("oldest") => "oldest".to_string(),
        Some("newest") => "newest".to_string(),
        Some("relevance") if has_lexical_query => "relevance".to_string(),
        None if has_query && has_lexical_query => "relevance".to_string(),
        _ => "newest".to_string(),
    }
}

/// Parses the opaque cursor used by cursor-based history pagination.
fn parse_history_cursor(cursor: Option<&str>) -> Option<HistoryCursor> {
    let raw = cursor?;
    if let Some(rest) = raw.strip_prefix("r|") {
        let mut parts = rest.split('|');
        return Some(HistoryCursor::Relevance {
            score: parts.next()?.parse().ok()?,
            visit_time: parts.next()?.parse().ok()?,
            id: parts.next()?.parse().ok()?,
        });
    }
    let (visit_time, id) = raw.split_once('|')?;
    Some(HistoryCursor::Chronological {
        visit_time: visit_time.parse().ok()?,
        id: id.parse().ok()?,
    })
}

/// Encodes one history row back into the opaque cursor form.
fn encode_history_cursor(entry: &HistoryEntry) -> String {
    format!("{}|{}", entry.visit_time, entry.id)
}

fn encode_relevance_history_cursor(entry: &HistoryEntry, score: f64) -> String {
    format!("r|{score}|{}|{}", entry.visit_time, entry.id)
}

/// Computes the number of pages for a result set, never returning zero.
fn page_count(total: usize, page_size: usize) -> usize {
    if total == 0 || page_size == 0 { 1 } else { ((total - 1) / page_size) + 1 }
}

/// Builds the normalized response envelope shared by all recall modes.
fn build_history_response(
    total: usize,
    page_size: usize,
    page: usize,
    start_index: usize,
    items: Vec<HistoryEntry>,
) -> HistoryQueryResponse {
    let normalized_page_size = page_size.max(1);
    let normalized_page_count = page_count(total, normalized_page_size);
    let normalized_page = page.clamp(1, normalized_page_count);
    let has_previous = start_index > 0;
    let has_next = start_index + items.len() < total;

    HistoryQueryResponse {
        total,
        page: normalized_page,
        page_size: normalized_page_size,
        page_count: normalized_page_count,
        has_previous,
        has_next,
        next_cursor: has_next.then(|| items.last().map(encode_history_cursor)).flatten(),
        items,
    }
}

fn build_lexical_history_response(
    total: usize,
    page_size: usize,
    page: usize,
    start_index: usize,
    scored_items: Vec<(HistoryEntry, f64)>,
    sort: &str,
) -> HistoryQueryResponse {
    let normalized_page_size = page_size.max(1);
    let normalized_page_count = page_count(total, normalized_page_size);
    let normalized_page = page.clamp(1, normalized_page_count);
    let has_previous = start_index > 0;
    let has_next = start_index + scored_items.len() < total;
    let next_cursor = has_next
        .then(|| {
            scored_items.last().map(|(entry, score)| {
                if sort == "relevance" {
                    encode_relevance_history_cursor(entry, *score)
                } else {
                    encode_history_cursor(entry)
                }
            })
        })
        .flatten();
    let items = scored_items.into_iter().map(|(entry, _)| entry).collect();

    HistoryQueryResponse {
        total,
        page: normalized_page,
        page_size: normalized_page_size,
        page_count: normalized_page_count,
        has_previous,
        has_next,
        next_cursor,
        items,
    }
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

/// Renders one export artifact in the requested output format.
fn render_export_content(results: &HistoryQueryResponse, format: &ExportFormat) -> Result<String> {
    Ok(match format {
        ExportFormat::Html => render_html_export(results),
        ExportFormat::Markdown => render_markdown_export(results),
        ExportFormat::Text => render_text_export(results),
        ExportFormat::Jsonl => results
            .items
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n"),
    })
}

/// Renders export output as a minimal HTML artifact.
fn render_html_export(results: &HistoryQueryResponse) -> String {
    let body = results
        .items
        .iter()
        .map(|item| {
            format!(
                "<article><h2>{}</h2><p><a href=\"{url}\">{url}</a></p><p>{}</p></article>",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.visited_at,
                url = item.url,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("<html><body>{body}</body></html>")
}

/// Renders export output as Markdown.
fn render_markdown_export(results: &HistoryQueryResponse) -> String {
    results
        .items
        .iter()
        .map(|item| {
            format!(
                "- [{}]({}) — {}",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.url,
                item.visited_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders export output as plain text.
fn render_text_export(results: &HistoryQueryResponse) -> String {
    results
        .items
        .iter()
        .map(|item| {
            format!(
                "{}\n{}\n{}\n",
                item.title.clone().unwrap_or_else(|| item.url.clone()),
                item.url,
                item.visited_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
