//! Shared history pagination contract.
//!
//! ## Responsibilities
//! - Keep history cursor parsing and encoding consistent across SQL, lexical,
//!   fuzzy, regex, and export flows.
//! - Build the Explorer-facing pagination response envelope.
//! - Preserve relevance cursor compatibility while accepting legacy
//!   chronological cursors.
//!
//! ## Not responsible for
//! - Query planning or SQL execution.
//! - Fuzzy scoring or lexical analysis.
//! - Export artifact rendering.
//!
//! ## Dependencies
//! - Archive history response models.
//!
//! ## Performance notes
//! - Helpers operate only on the current page window and must not force full
//!   history result materialization.

use crate::models::{HistoryEntry, HistoryQueryResponse};

#[derive(Clone, Copy)]
pub(super) enum HistoryCursor {
    Chronological { visit_time: i64, id: i64 },
    Relevance { score: f64, visit_time: i64, id: i64 },
}

impl HistoryCursor {
    /// Allows newest/oldest callers to keep accepting old chronological
    /// cursors even when the current query could also produce relevance pages.
    pub(super) fn chronological(self) -> Option<(i64, i64)> {
        match self {
            HistoryCursor::Chronological { visit_time, id }
            | HistoryCursor::Relevance { visit_time, id, .. } => Some((visit_time, id)),
        }
    }

    /// Allows relevance callers to reject legacy cursors without changing the
    /// public cursor string parser.
    pub(super) fn relevance(self) -> Option<(f64, i64, i64)> {
        match self {
            HistoryCursor::Relevance { score, visit_time, id } => Some((score, visit_time, id)),
            HistoryCursor::Chronological { .. } => None,
        }
    }
}

/// Normalizes the public sort string into the backend ordering contract.
pub(super) fn normalize_history_sort(
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
pub(super) fn parse_history_cursor(cursor: Option<&str>) -> Option<HistoryCursor> {
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

/// Computes the number of pages for a result set, never returning zero.
pub(super) fn page_count(total: usize, page_size: usize) -> usize {
    if total == 0 || page_size == 0 { 1 } else { ((total - 1) / page_size) + 1 }
}

/// Builds the normalized response envelope shared by non-scored recall modes.
pub(super) fn build_history_response(
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
        total_exact: true,
        total_visits: None,
        windowed: false,
        regex_scan: None,
        page: normalized_page,
        page_size: normalized_page_size,
        page_count: normalized_page_count,
        has_previous,
        has_next,
        next_cursor: has_next.then(|| items.last().map(encode_history_cursor)).flatten(),
        items,
    }
}

/// Builds the response for `include_total: false` pages.
///
/// `has_more` comes from fetching one row past the page size, so no count query is needed.
/// `total` is the page size and `page_count` stays 1: neither is an archive-wide figure.
pub(super) fn build_uncounted_history_response(
    page_size: usize,
    start_index: usize,
    has_more: bool,
    scored_items: Vec<(HistoryEntry, f64)>,
    sort: &str,
) -> HistoryQueryResponse {
    let next_cursor = has_more
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
    let items: Vec<HistoryEntry> = scored_items.into_iter().map(|(entry, _)| entry).collect();
    HistoryQueryResponse {
        total: items.len(),
        total_exact: false,
        total_visits: None,
        windowed: false,
        regex_scan: None,
        page: 1,
        page_size: page_size.max(1),
        page_count: 1,
        has_previous: start_index > 0,
        has_next: has_more,
        next_cursor,
        items,
    }
}

/// Builds the normalized response envelope shared by BM25 and fuzzy recall.
pub(super) fn build_lexical_history_response(
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
        total_exact: true,
        total_visits: None,
        windowed: false,
        regex_scan: None,
        page: normalized_page,
        page_size: normalized_page_size,
        page_count: normalized_page_count,
        has_previous,
        has_next,
        next_cursor,
        items,
    }
}

/// Encodes one history row back into the opaque chronological cursor form.
fn encode_history_cursor(entry: &HistoryEntry) -> String {
    format!("{}|{}", entry.visit_time, entry.id)
}

/// Encodes one scored history row into the relevance cursor form.
fn encode_relevance_history_cursor(entry: &HistoryEntry, score: f64) -> String {
    format!("r|{score}|{}|{}", entry.visit_time, entry.id)
}

/// Where a list of pages (`group_by_url`) stopped: the sort key of its last row.
///
/// Pages sort by relevance score (relevance only), then by their most recent matching visit, then
/// by URL. The URL is unique per row, so two pages with the same score and time are never skipped
/// or repeated.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PageCursor {
    /// `None` unless the list is sorted by relevance.
    pub score: Option<f64>,
    pub last_time: i64,
    pub url: String,
}

/// Parses a page cursor (`p|<score>|<time>|<url>`). Visit cursors are not page cursors.
pub(super) fn parse_page_cursor(raw: Option<&str>) -> Option<PageCursor> {
    let mut parts = raw?.strip_prefix("p|")?.splitn(3, '|');
    let score = match parts.next()? {
        "" => None,
        value => Some(value.parse().ok()?),
    };
    Some(PageCursor {
        score,
        last_time: parts.next()?.parse().ok()?,
        url: parts.next()?.to_string(),
    })
}

/// Builds the response for one page of a `group_by_url` query.
///
/// Each row is a page's most recent matching visit with `visit_count` set, paired with the page's
/// relevance score. `totals` is `(pages, visits)` when they were counted.
pub(super) fn build_page_response(
    page_size: usize,
    after_cursor: bool,
    has_more: bool,
    rows: Vec<(HistoryEntry, f64)>,
    sort: &str,
    totals: Option<(usize, usize)>,
) -> HistoryQueryResponse {
    let next_cursor = has_more
        .then(|| {
            rows.last().map(|(entry, score)| {
                let score = if sort == "relevance" { score.to_string() } else { String::new() };
                format!("p|{score}|{}|{}", entry.visit_time, entry.url)
            })
        })
        .flatten();
    let items: Vec<HistoryEntry> = rows.into_iter().map(|(entry, _)| entry).collect();
    let page_size = page_size.max(1);
    HistoryQueryResponse {
        total: totals.map_or(items.len(), |(pages, _)| pages),
        total_exact: totals.is_some(),
        total_visits: totals.map(|(_, visits)| visits),
        windowed: false,
        regex_scan: None,
        page: 1,
        page_size,
        page_count: totals.map_or(1, |(pages, _)| page_count(pages, page_size)),
        has_previous: after_cursor,
        has_next: has_more,
        next_cursor,
        items,
    }
}
