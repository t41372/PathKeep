//! Responsibilities: build SQL for visits after the saved row id, recent visits being reread,
//! and counts of visits whose page row is missing. Streams and counts use the same window.
//! Not responsible for: parsing columns, choosing a read plan or opening browser files.
use crate::{VisitWindow, native_visits::VisitTable};
use rusqlite::types::Value;

/// Returns SQL for visits joined to their page rows, ordered by visit id. Parsers use it to
/// read page metadata with each visit and save a cursor they can continue from.
pub(crate) fn visits(table: VisitTable, columns: &str, window: VisitWindow) -> String {
    let (visits, page, pages, _) = table.names();
    // Keep visits outside the page lookup even when ANALYZE makes scanning pages look cheaper.
    let select =
        format!("SELECT {columns} FROM {visits} CROSS JOIN {pages} ON {pages}.id={visits}.{page}");
    format!("{} ORDER BY 1", ranges(table, &select, window))
}

/// Returns SQL that counts visits in the read window with no page row. Those visits cannot
/// supply an address; backups report how many were left out of the parsed stream.
pub(crate) fn orphans(table: VisitTable, window: VisitWindow) -> String {
    let (visits, page, pages, _) = table.names();
    let window = ranges(table, &format!("SELECT {page} FROM {visits}"), window);
    format!(
        "SELECT COUNT(*) FROM ({window}) AS read_visits
         WHERE NOT EXISTS (SELECT 1 FROM {pages} WHERE {pages}.id=read_visits.{page})"
    )
}

/// A full read needs only a rowid seek. A recent read merges two disjoint ranges: new ids and
/// recent times at or below the cursor. Unary + keeps the rowid bound from driving the time
/// lookup. Chromium and Firefox seek recent times through their time-leading indexes. Safari
/// has only a page-first index: the recent range reads each visit's time through that narrower
/// covering index, while the new-id range still seeks by rowid. No index is named, so files
/// missing an index still return the same rows. Full reads never build a recent-id list.
fn ranges(table: VisitTable, select: &str, window: VisitWindow) -> String {
    let (visits, _, _, time) = table.names();
    let after = format!("{select} WHERE {visits}.id > ?1");
    match window.recent_since_ms {
        Some(_) => format!(
            "{after} UNION ALL {select}
             WHERE {visits}.id IN (SELECT id FROM {visits} WHERE {time} >= ?2 AND +id <= ?1)"
        ),
        None => after,
    }
}

/// Returns the saved row id and, for a recent read, the cutoff in the browser's time units.
/// Both queries bind these values so orphan counts describe exactly the visits being read.
pub(crate) fn parameters(table: VisitTable, window: VisitWindow) -> Vec<Value> {
    let mut values = vec![Value::Integer(window.after_id)];
    if window.recent_since_ms.is_some() {
        values.push(table.time_floor(window.recent_since_ms));
    }
    values
}

#[cfg(test)]
pub(crate) mod tests;
