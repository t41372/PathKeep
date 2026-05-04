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
        page: normalized_page,
        page_size: normalized_page_size,
        page_count: normalized_page_count,
        has_previous,
        has_next,
        next_cursor: has_next.then(|| items.last().map(encode_history_cursor)).flatten(),
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
