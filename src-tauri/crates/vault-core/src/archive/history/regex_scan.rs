//! Regex search over the whole archive, one time-boxed chunk per request.
//!
//! A regex cannot use an index: every visit's URL and title is matched in Rust. One request cannot
//! scan 14.4M visits without the list waiting seconds, and the fixed window this replaced (the
//! newest 50,000 visits, 0.35% of such an archive) silently missed every older page. So each
//! request scans for a bounded time from where the last one stopped, and says how far it got.
//!
//! ## Responsibilities
//! - Scan visible visits inside the browser, profile, date and domain filters in sort order (newest
//!   first unless `oldest`), from the cursor, until the page is full, the time budget is spent or
//!   the visits run out. Each URL and title is matched case-insensitively with the `regex` crate.
//! - Return the matches, a cursor where the scan stopped, and the progress (`RegexScanProgress`).
//! - In grouped mode, one row per page among the visits this request scanned.
//!
//! ## Not responsible for
//! - Merging a page that shows up in two chunks. A grouped row counts the visits found in its
//!   chunk; the History list adds up rows of the same page.
//! - Keeping anything between requests. The cursor holds the whole state, so a scan the user
//!   abandons simply is not continued; nothing runs between chunks.
//!
//! ## The cursor
//! `x|<visit_time_ms>|<visit_id>` is the last visit the scan consumed: every visit before it in
//! sort order was returned or did not match, and the next request starts right after it. When the
//! page fills up, the visit that would have started a new row is not consumed, so it is read again
//! by the next request.
//!
//! ## Ways this can go wrong, and what guards each
//! 1. A match older than the first chunk is never found. The next request continues from the
//!    cursor; a test puts the only match past the first chunk's budget.
//! 2. A row repeated or skipped at a chunk edge. The cursor is the last consumed visit, not the last
//!    match; a test walks with a budget of a few rows and compares with one unbounded scan.
//! 3. A partial count shown as final. `total_exact` is true only when one response scanned the
//!    whole filtered range; otherwise `regex_scan.complete` is false and the count is "so far".
//! 4. A scan that keeps running after the user leaves. Each request returns within its budget and
//!    keeps no state; a test resumes from a cursor on a fresh connection.
//! 5. Filters applied after the regex. They are conditions of the scan query, so a date filter
//!    bounds the scan itself; a test filters out the newest matches.
//! 6. A dialect other than Rust's `regex` crate. Look-around is rejected before scanning; a test.
//!
//! ## Performance notes
//! - Memory per request: at most `limit + 1` matched visits, or `limit` pages plus one map entry
//!   per page. Scan rate and time to cover 14.4M visits are in `docs/architecture/ipc-performance.md`.

use super::grouped::PageFilters;
use super::pagination::page_count;
use super::*;
use crate::models::RegexScanProgress;
use std::time::{Duration, Instant};

/// How long one request scans before it returns what it found.
///
/// The History list asks for the next chunk as soon as one arrives while it has less than a screen
/// of rows, so this is the delay between visible updates, not a limit on what is searched. At about
/// 200 ms the count and the "searched back to" date move several times a second, and each chunk is
/// long enough that the per-request cost (opening the archive, preparing the query, about 2 ms)
/// stays around 1% of the scan.
pub(super) const REGEX_CHUNK_TIME: Duration = Duration::from_millis(200);

/// Rows one request scans at most, whatever the clock says: a backstop for a stalled clock and the
/// knob tests turn instead of time. About five seconds of scanning on the benchmark machine
/// (roughly 155,000 visits per 200 ms chunk).
const REGEX_CHUNK_ROWS: usize = 4_000_000;

/// How often the row loop reads the clock.
const CLOCK_EVERY_ROWS: usize = 256;

/// The limits of one chunk.
#[derive(Debug, Clone, Copy)]
pub(super) struct ScanBudget {
    pub time: Option<Duration>,
    pub rows: usize,
}

impl ScanBudget {
    /// The budget of one request. Debug builds read `PATHKEEP_DEBUG_REGEX_CHUNK_ROWS` so the E2E
    /// suite can split its small fixture into several chunks; release builds ignore it.
    pub(super) fn chunk() -> Self {
        #[cfg(debug_assertions)]
        if let Some(rows) = std::env::var("PATHKEEP_DEBUG_REGEX_CHUNK_ROWS")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|rows| *rows > 0)
        {
            return Self { time: Some(REGEX_CHUNK_TIME), rows };
        }
        Self { time: Some(REGEX_CHUNK_TIME), rows: REGEX_CHUNK_ROWS }
    }
}

/// One regex request, already normalized by `list_history`.
pub(super) struct RegexScan<'a> {
    pub regex: &'a regex::Regex,
    pub filters: &'a PageFilters,
    pub sort: &'a str,
    pub limit: usize,
    pub cursor: Option<&'a str>,
    /// `page` mode for the visit list: skip the matches of the earlier pages first.
    pub requested_page: Option<usize>,
    pub grouped: bool,
}

/// The visits of one scan in sort order, after the cursor, inside the filters. Regex mode has no
/// `site:`-style operators, so unlike the visit list this carries only the domain filter, which
/// keeps the per-row cost to the index walk and two lookups.
macro_rules! regex_scan_sql {
    ($cursor_comparison:literal, $direction:literal) => {
        concat!(
            r#"
SELECT
  visits.id,
  source_profiles.profile_key,
  urls.url,
  urls.title,
  visits.visit_time_ms,
  visits.visit_duration_ms,
  visits.transition_type,
  visits.source_visit_id,
  visits.app_id
FROM visits
JOIN urls
  ON urls.id = visits.url_id
JOIN source_profiles
  ON source_profiles.id = visits.source_profile_id
WHERE visits.reverted_at IS NULL
  AND (:profileId IS NULL OR source_profiles.profile_key = :profileId)
  AND (:browserKind IS NULL OR source_profiles.browser_kind = :browserKind)
  AND (:domainPattern IS NULL OR urls.url LIKE :domainPattern)
  AND visits.visit_time_ms >= :startTimeMs
  AND visits.visit_time_ms <= :endTimeMs
  AND (visits.visit_time_ms, visits.id) "#,
            $cursor_comparison,
            r#" (:cursorVisitTime, :cursorId)
ORDER BY visits.visit_time_ms "#,
            $direction,
            ", visits.id ",
            $direction,
            "\n"
        )
    };
}

pub(super) const REGEX_SCAN_NEWEST_SQL: &str = regex_scan_sql!("<", "DESC");
pub(super) const REGEX_SCAN_OLDEST_SQL: &str = regex_scan_sql!(">", "ASC");

/// Parses `x|<time>|<id>`; a plain visit cursor (`<time>|<id>`) is accepted as the same position.
fn parse_frontier(raw: Option<&str>) -> Option<(i64, i64)> {
    let raw = raw?;
    let raw = raw.strip_prefix("x|").unwrap_or(raw);
    let (time, id) = raw.split_once('|')?;
    Some((time.parse().ok()?, id.parse().ok()?))
}

/// Runs one chunk of a regex search.
pub(super) fn scan_regex(
    connection: &Connection,
    request: &RegexScan<'_>,
    budget: ScanBudget,
) -> Result<HistoryQueryResponse> {
    let started = Instant::now();
    let frontier = parse_frontier(request.cursor);
    let filters = request.filters;
    let (start_bound, end_bound, cursor_time, cursor_id) =
        list_history_bounds(request.sort, filters.start_time_ms, filters.end_time_ms, frontier);
    // The cursor's time also bounds the time range, so the index seek starts at the cursor: with
    // only the row-value comparison SQLite seeks on the date bound and filters every visit the
    // earlier chunks already scanned, which made a whole-archive scan quadratic.
    let (start_bound, end_bound) = if request.sort == "oldest" {
        (start_bound.max(cursor_time), end_bound)
    } else {
        (start_bound, end_bound.min(cursor_time))
    };
    let sql = if request.sort == "oldest" { REGEX_SCAN_OLDEST_SQL } else { REGEX_SCAN_NEWEST_SQL };
    let mut statement = connection.prepare_cached(sql)?;
    let mut rows = statement.query(named_params! {
        ":profileId": filters.profile_id,
        ":browserKind": filters.browser_kind,
        ":domainPattern": filters.domain_pattern,
        ":startTimeMs": start_bound,
        ":endTimeMs": end_bound,
        ":cursorVisitTime": cursor_time,
        ":cursorId": cursor_id,
    })?;

    let limit = request.limit.max(1);
    let mut skip = if request.grouped {
        0
    } else {
        request.requested_page.map_or(0, |page| page.saturating_sub(1) * limit)
    };
    let skipped_any = skip > 0;
    let mut items: Vec<HistoryEntry> = Vec::new();
    let mut page_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut matched_visits = 0usize;
    let mut consumed: Option<(i64, i64)> = None;
    let mut scanned = 0usize;
    let mut exhausted = false;
    let mut full = false;

    loop {
        // Every request consumes at least one visit, so a cursor always moves forward.
        if scanned > 0
            && (scanned >= budget.rows
                || (scanned % CLOCK_EVERY_ROWS == 0
                    && budget.time.is_some_and(|time| started.elapsed() >= time)))
        {
            break;
        }
        let Some(row) = rows.next()? else {
            exhausted = true;
            break;
        };
        scanned += 1;
        let position = (row.get::<_, i64>(4)?, row.get::<_, i64>(0)?);
        // Match on the borrowed text; a full entry is only built for a match.
        let matches = request.regex.is_match(row.get_ref(2)?.as_str()?)
            || match row.get_ref(3)? {
                rusqlite::types::ValueRef::Text(title) => {
                    request.regex.is_match(&String::from_utf8_lossy(title))
                }
                _ => false,
            };
        if !matches {
            consumed = Some(position);
            continue;
        }
        let entry = history_entry_from_row(row)?;
        if request.grouped {
            if let Some(&index) = page_index.get(&entry.url) {
                let page = &mut items[index];
                page.visit_count = Some(page.visit_count.unwrap_or(0) + 1);
            } else if items.len() == limit {
                // A new row would overflow the page: leave this visit for the next request.
                full = true;
                break;
            } else {
                page_index.insert(entry.url.clone(), items.len());
                items.push(HistoryEntry { visit_count: Some(1), ..entry });
            }
            matched_visits += 1;
        } else if skip > 0 {
            skip -= 1;
        } else if items.len() == limit {
            full = true;
            break;
        } else {
            items.push(entry);
            matched_visits += 1;
        }
        consumed = Some(position);
    }

    let complete = exhausted && !full;
    // Exact only when this one response saw every visit inside the filters.
    let exact = complete && frontier.is_none() && !skipped_any;
    let scanned_to_ms = consumed.map(|(time, _)| time).or(frontier.map(|(time, _)| time));
    let next_cursor = (!complete)
        .then(|| consumed.or(frontier).map(|(time, id)| format!("x|{time}|{id}")))
        .flatten();
    let page = request.requested_page.unwrap_or(1).max(1);
    Ok(HistoryQueryResponse {
        total: items.len(),
        total_exact: exact,
        total_visits: request.grouped.then_some(matched_visits),
        windowed: false,
        regex_scan: Some(RegexScanProgress { scanned_to_ms, complete }),
        page: if exact { 1 } else { page },
        page_size: limit,
        page_count: if exact { page_count(items.len(), limit) } else { page },
        has_previous: frontier.is_some() || skipped_any,
        has_next: next_cursor.is_some(),
        next_cursor,
        items,
    })
}

#[cfg(test)]
mod tests;
