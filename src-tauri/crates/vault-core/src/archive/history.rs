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

use super::*;

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
    let fts_query = q.as_deref().and_then(build_fts_query);
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
    let sort = query.sort.clone().unwrap_or_else(|| "newest".to_string());
    let cursor = parse_history_cursor(query.cursor.as_deref());
    let (cursor_visit_time, cursor_id) = cursor.unwrap_or((0, 0));

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

    if q.is_some() && fts_query.is_none() {
        return Ok(HistoryQueryResponse::default());
    }

    if let Some(fts_query) = fts_query {
        return list_history_with_fts(
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
            fts_query,
        );
    }

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

/// Parses the opaque cursor used by cursor-based history pagination.
fn parse_history_cursor(cursor: Option<&str>) -> Option<(i64, i64)> {
    let raw = cursor?;
    let (visit_time, id) = raw.split_once('|')?;
    Some((visit_time.parse().ok()?, id.parse().ok()?))
}

/// Encodes one history row back into the opaque cursor form.
fn encode_history_cursor(entry: &HistoryEntry) -> String {
    format!("{}|{}", entry.visit_time, entry.id)
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

/// Converts a raw keyword string into the FTS prefix query grammar used by Explorer.
fn build_fts_query(raw: &str) -> Option<String> {
    let tokens = raw
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    if tokens.is_empty() { None } else { Some(tokens.join(" AND ")) }
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
    cursor: Option<(i64, i64)>,
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
    } else if let Some((cursor_visit_time, cursor_id)) = cursor {
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

/// Runs FTS-backed keyword recall.
#[allow(clippy::too_many_arguments)]
fn list_history_with_fts(
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
    cursor: Option<(i64, i64)>,
    cursor_visit_time: i64,
    cursor_id: i64,
    fts_query: String,
) -> Result<HistoryQueryResponse> {
    let total: usize = connection
        .query_row(
            COUNT_HISTORY_FTS_SQL,
            named_params! {
                ":ftsQuery": fts_query.clone(),
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

    let mut statement = connection.prepare(LIST_HISTORY_FTS_SQL)?;
    let normalized_page_count = page_count(total, limit_usize);
    let page = requested_page.unwrap_or(1).min(normalized_page_count);
    let start_index = page.saturating_sub(1) * limit_usize;
    let page_limit = if requested_page.is_some() { i64::from(limit) } else { i64::from(limit) + 1 };
    let page_offset =
        if requested_page.is_some() { i64::try_from(start_index).unwrap_or(i64::MAX) } else { 0 };
    let rows = statement.query_map(
        named_params! {
            ":ftsQuery": fts_query,
            ":profileId": profile_id,
            ":browserKind": browser_kind,
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
    cursor: Option<(i64, i64)>,
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
        favicon: row
            .get::<_, Option<Vec<u8>>>(9)?
            .as_deref()
            .and_then(image_data_to_data_url)
            .map(|data_url| HistoryFavicon { data_url }),
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
