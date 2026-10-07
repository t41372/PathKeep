//! Responsibilities: preserve native timeline cursor parsing and page counts.
//! Not responsible for: search cursors, SQL execution or response hydration.
//! Failure modes: timestamp ties skipped or cursor kinds confused. Tests: timeline_tests.

#[derive(Clone, Copy)]
pub(super) enum HistoryCursor {
    Chronological { visit_time: i64, id: i64 },
    Relevance { _score: f64, visit_time: i64, id: i64 },
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
}

/// Parses the opaque cursor used by cursor-based history pagination.
pub(super) fn parse_history_cursor(cursor: Option<&str>) -> Option<HistoryCursor> {
    let raw = cursor?;
    if let Some(rest) = raw.strip_prefix("r|") {
        let mut parts = rest.split('|');
        return Some(HistoryCursor::Relevance {
            _score: parts.next()?.parse().ok()?,
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
