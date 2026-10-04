//! The matches one keyword search ranks: every match, or a window of the most recent ones.
//!
//! A word in almost every page title used to rank, group and count every match before cutting the
//! first page: about 10 s for the first page and 25 s for the totals at 14.4M visits. Here the
//! matching URLs are collected first, and when more than the window cap pass the filters only the
//! most recently archived ones are kept. Everything after this step (the visit list, the page list,
//! the totals) reads `temp.history_ranked_urls` instead of the search index, so it costs at most the
//! cap, whatever the word.
//!
//! ## Responsibilities
//! - Fill `temp.history_ranked_urls (url_id, score)` with the `url_id`s that match the search and
//!   have a visible visit inside the filters, scored like the visit list always scored them.
//! - Keep at most `cap` of them, the most recently archived (highest `url_id`), or the least recently
//!   archived for `oldest` order, and report that the window was hit.
//! - Pin the window's edge in the cursor so later pages read the same window.
//!
//! ## Not responsible for
//! - Building rows, pages or counts from the ranked URLs (`grouped`, `history`).
//! - Query parsing, lexical analysis and fuzzy matching (`search_query`, `search_lexical`).
//!
//! ## How the window is chosen
//! Each search index (the term index, the trigram index, or every URL for an operator-only search)
//! is read in `url_id` order, newest first, keeping only `url_id`s with a visible visit inside the
//! browser, profile, date and URL filters. Reading stops after `cap + 1` such `url_id`s per index.
//! The search index can return its matches in `url_id` order without ranking all of them, which is
//! why the window is "most recently archived" (`url_id` grows as pages are archived) and not "most
//! recently visited": an index has no visit-time order, and testing the newest visits one by one
//! against the index costs 20 to 70 µs per visit (measured; see `docs/architecture/ipc-performance.md`).
//! The trade-off: a page archived long ago and still visited today can fall outside the window of a
//! word that matches more than the cap. The notice under the result count says not every match was
//! ranked and that another word narrows the search.
//!
//! When the term index alone has more than `cap` matches, a URL that both indexes match keeps its
//! term score instead of the better of the two (see [`rank_matching_urls`]); the edge in the cursor
//! records this so later pages score the same way.
//!
//! ## Ways this can go wrong, and what guards each
//! 1. A rare or medium term is windowed, or returns something different. Below the cap the table
//!    holds every match with the same score as before (both indexes, best score per `url_id`), and
//!    `windowed` stays false; a test compares a term under the cap with and without the window.
//! 2. The window drops the best match of a term that fits. Only a term with more than `cap`
//!    matching URLs inside the filters is windowed; a test puts the best match in the oldest URL.
//! 3. A count claims exactness it does not have. A windowed response always has `windowed: true`
//!    and `total_exact: false`; a test checks both, and that the totals equal the walked rows.
//! 4. A cursor page skips or repeats rows at the window edge, or a backup between two pages moves
//!    the window. The edge is written into the cursor and read back on the next page; a test walks
//!    a windowed search one row at a time and compares it with one large page.
//! 5. Filters applied after the window, so a date filter on a common word finds nothing. The visit
//!    test runs inside the index scan, before the cap; a test filters a windowed word to its oldest
//!    URL's dates.
//!
//! ## Performance notes
//! - Cost is the index scan up to `cap + 1` qualifying `url_id`s (one visit-index probe each), one
//!   relevance score per kept `url_id`, and the inserts. The relevance function also reads the
//!   whole posting list of each search term once to weigh it, a fixed cost per word.
//! - A narrow date range is tested through the URLs visited in it (`temp.history_window_scope`)
//!   instead of one probe per match.
//! - A word that matches many URLs but few inside a narrow filter (a common word and one week)
//!   still reads matches until it fills the window or runs out; that is the slowest case left.
//!   Measured in `docs/architecture/ipc-performance.md` §6.

use super::*;

/// How many matching URLs one keyword search ranks.
///
/// Measured on the 14.4M-visit benchmark archive (`archive_scale_bench`, 3.6M URLs, release build,
/// 18-core Apple Silicon under load) with "topic", which is in every title: a fixed part of about
/// 200 ms (reading the word's posting lists, for the matches and for the relevance weight) plus
/// about 4 µs per ranked URL (the visit-index probe, the score, the insert, grouping its visits).
/// The first page of grouped results took 290 ms at 25,000 and 390 ms at 50,000 (10.5 s before),
/// and "topic" with a one-week filter 0.73 s and 1.0 s. The target machine is several times slower,
/// so the smaller cap keeps the worst common word near half a second there. 25,000 pages is 250
/// screens of results; a word on fewer pages (inside the filters) is ranked whole, exactly as before.
pub(super) const KEYWORD_WINDOW: usize = 25_000;

/// The window cap in effect. Debug builds read `PATHKEEP_DEBUG_SEARCH_WINDOW` so the E2E suite can
/// show the windowed header on its small fixture; release builds always use [`KEYWORD_WINDOW`].
pub(in crate::archive::history) fn keyword_window_cap() -> usize {
    #[cfg(debug_assertions)]
    if let Some(cap) = std::env::var("PATHKEEP_DEBUG_SEARCH_WINDOW")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|cap| *cap > 0)
    {
        return cap;
    }
    KEYWORD_WINDOW
}

/// Where a window ends, so a later page reads the same window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::archive::history) struct WindowEdge {
    /// True when the window holds the least recently archived URLs (`oldest` order).
    pub oldest_first: bool,
    /// True when a URL both indexes match kept its term score (see [`rank_matching_urls`]).
    pub term_scores_only: bool,
    /// The last `url_id` inside the window.
    pub url_id: i64,
}

/// What [`rank_matching_urls`] put in `temp.history_ranked_urls`.
#[derive(Debug, Clone, Copy)]
pub(in crate::archive::history) struct RankedUrls {
    /// How many `url_id`s the table holds.
    pub count: usize,
    /// Set when the window was hit; the table then holds only the URLs up to this edge.
    pub edge: Option<WindowEdge>,
}

impl RankedUrls {
    /// Marks a response built from a windowed table: the total becomes "at least", and the next
    /// cursor carries the edge so the following page reads the same window.
    pub(in crate::archive::history) fn label(
        &self,
        mut response: HistoryQueryResponse,
    ) -> HistoryQueryResponse {
        if let Some(edge) = self.edge {
            response.windowed = true;
            response.total_exact = false;
            response.next_cursor = response.next_cursor.map(|cursor| pin_cursor(edge, &cursor));
        }
        response
    }
}

/// Splits a pinned window edge (`w<o|n><t|b><url_id>|`) off the front of a cursor.
pub(in crate::archive::history) fn split_cursor(
    raw: Option<&str>,
) -> (Option<WindowEdge>, Option<&str>) {
    let Some(raw) = raw else { return (None, None) };
    let parsed = (|| {
        let rest = raw.strip_prefix('w')?;
        let mut flags = rest.chars();
        let oldest_first = match flags.next()? {
            'o' => true,
            'n' => false,
            _ => return None,
        };
        let term_scores_only = match flags.next()? {
            't' => true,
            'b' => false,
            _ => return None,
        };
        let (url_id, cursor) = flags.as_str().split_once('|')?;
        Some((WindowEdge { oldest_first, term_scores_only, url_id: url_id.parse().ok()? }, cursor))
    })();
    match parsed {
        Some((edge, cursor)) => (Some(edge), Some(cursor)),
        None => (None, Some(raw)),
    }
}

fn pin_cursor(edge: WindowEdge, cursor: &str) -> String {
    let order = if edge.oldest_first { 'o' } else { 'n' };
    let scores = if edge.term_scores_only { 't' } else { 'b' };
    format!("w{order}{scores}{}|{cursor}", edge.url_id)
}

/// One index scan in `url_id` order, keeping candidates with a visible visit inside the browser,
/// profile and date filters on a URL the URL filters allow. `$id` is the candidate's `url_id`.
///
/// The visit test is either an index probe per candidate or, for a narrow date range, membership
/// in `temp.history_window_scope` (the URLs visited in that range, collected first). The URL test
/// only runs when a URL filter is set. Both switches are bound constants, so SQLite skips the
/// branch that is off.
macro_rules! window_scan {
    ($select:expr, $id:expr, $order:expr) => {
        concat!(
            $select,
            "\n  AND ",
            $id,
            " >= :lowUrlId\n  AND ",
            $id,
            " <= :highUrlId\n  AND (\n    (:scoped = 1 AND ",
            $id,
            " IN (SELECT url_id FROM temp.history_window_scope))\n    OR (:scoped = 0 AND EXISTS (\n      SELECT 1\n      FROM visits\n      WHERE visits.url_id = ",
            $id,
            r#"
        AND visits.reverted_at IS NULL
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
    ))
  )
  AND (:urlFiltered = 0 OR EXISTS (
    SELECT 1
    FROM urls
    WHERE urls.id = "#,
            $id,
            "\n",
            page_url_filters!(),
            "  ))\nORDER BY ",
            $id,
            " ",
            $order,
            "\nLIMIT :scanLimit\n"
        )
    };
}

/// The term index, scored exactly as the visit list has always scored it.
macro_rules! terms_scan {
    ($order:expr) => {
        window_scan!(
            r#"SELECT
  rowid,
  bm25(history_search_terms, 6.0, 12.0, 4.0, 5.0, 10.0, 4.0, 2.0, 7.0, 9.0, 6.0)
FROM search.history_search_terms
WHERE history_search_terms MATCH :ftsQuery"#,
            "history_search_terms.rowid",
            $order
        )
    };
}

/// The trigram index; its matches rank a little below an equal term match. With
/// `:termScoresOnly`, a URL the term scan already scored gets NULL here and keeps that score.
macro_rules! trigram_scan {
    ($order:expr) => {
        window_scan!(
            r#"SELECT
  rowid,
  CASE
    WHEN :termScoresOnly = 1
      AND history_search_trigram.rowid IN (SELECT url_id FROM temp.history_ranked_urls)
    THEN NULL
    ELSE bm25(history_search_trigram, 1.0) + 0.35
  END
FROM search.history_search_trigram
WHERE history_search_trigram MATCH :ftsQuery"#,
            "history_search_trigram.rowid",
            $order
        )
    };
}

/// Every URL, unranked: operator-only searches (`site:github.com` with no words).
macro_rules! every_url_scan {
    ($order:expr) => {
        window_scan!(
            r#"SELECT
  candidates.id,
  0.0
FROM urls AS candidates
WHERE 1"#,
            "candidates.id",
            $order
        )
    };
}

pub(super) const TERMS_NEWEST_SQL: &str = terms_scan!("DESC");
const TERMS_OLDEST_SQL: &str = terms_scan!("ASC");
const TRIGRAM_NEWEST_SQL: &str = trigram_scan!("DESC");
const TRIGRAM_OLDEST_SQL: &str = trigram_scan!("ASC");
const EVERY_URL_NEWEST_SQL: &str = every_url_scan!("DESC");
const EVERY_URL_OLDEST_SQL: &str = every_url_scan!("ASC");

/// A date range with at most this many visible visits is scanned through the URLs visited in it
/// (`temp.history_window_scope`) instead of probing each match's visits.
///
/// A common word with a narrow range ("topic", last 7 days) otherwise probes every match before
/// it gives up filling the window: 3.6M probes, 13 to 26 s at 14.4M visits. Collecting the range's
/// URLs costs about half a microsecond per visit, so 250,000 visits is about 125 ms; wider ranges
/// let most matches pass the probe, and the probe path fills the window quickly.
const SCOPE_MAX_VISITS: i64 = 250_000;

#[cfg(not(test))]
fn scope_max_visits() -> i64 {
    SCOPE_MAX_VISITS
}

#[cfg(test)]
thread_local! {
    /// Lets a test take the per-match probe path on an archive of a handful of visits.
    pub(in crate::archive::history) static SCOPE_MAX_VISITS_FOR_TEST: std::cell::Cell<i64> =
        const { std::cell::Cell::new(SCOPE_MAX_VISITS) };
}

#[cfg(test)]
fn scope_max_visits() -> i64 {
    SCOPE_MAX_VISITS_FOR_TEST.with(std::cell::Cell::get)
}

/// What to match: the two index queries of a keyword search, or every URL.
pub(in crate::archive::history) enum Matching<'a> {
    Words(&'a LexicalQuery),
    EveryUrl,
}

/// Fills `temp.history_ranked_urls` with the matching URLs inside the filters, windowed at `cap`.
///
/// `pinned` is the edge from a previous page's cursor: the table then holds every qualifying match
/// up to that edge, scored the same way, so the page continues the window the first page saw.
///
/// When the term index alone has more than `cap` matches, the window is certain before the trigram
/// index is read, and a URL both indexes match keeps its term score: scoring it on the trigram index
/// too would read that index's whole posting list (about 280 ms for "topic") only to reorder pages
/// inside the window. Below the cap both scores are computed and the better one kept, as always.
pub(in crate::archive::history) fn rank_matching_urls(
    connection: &Connection,
    matching: Matching<'_>,
    filters: &PageFilters,
    sort: &str,
    pinned: Option<WindowEdge>,
    cap: usize,
) -> Result<RankedUrls> {
    let oldest_first = pinned.map_or(sort == "oldest", |edge| edge.oldest_first);
    let (low, high) = match pinned {
        Some(edge) if edge.oldest_first => (i64::MIN, edge.url_id),
        Some(edge) => (edge.url_id, i64::MAX),
        None => (i64::MIN, i64::MAX),
    };
    // One past the cap tells "exactly the cap" from "more than the cap".
    let scan_limit =
        if pinned.is_some() { -1 } else { i64::try_from(cap).unwrap_or(i64::MAX - 1) + 1 };
    connection.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS history_ranked_urls (
           url_id INTEGER PRIMARY KEY,
           score REAL NOT NULL
         );
         CREATE TEMP TABLE IF NOT EXISTS history_window_scope (url_id INTEGER PRIMARY KEY);
         DELETE FROM temp.history_ranked_urls;
         DELETE FROM temp.history_window_scope;",
    )?;
    let scoped = scope_to_date_range(connection, filters)?;
    let url_filtered = filters.domain_pattern.is_some() || advanced_filters_set(connection)?;

    let scan = |sql: &str,
                fts_query: Option<&str>,
                term_scores_only: bool|
     -> Result<Vec<(i64, Option<f64>)>> {
        let mut statement = connection.prepare_cached(sql)?;
        bind(&mut statement, ":ftsQuery", &fts_query)?;
        bind(&mut statement, ":lowUrlId", &low)?;
        bind(&mut statement, ":highUrlId", &high)?;
        bind(&mut statement, ":scanLimit", &scan_limit)?;
        bind(&mut statement, ":scoped", &i64::from(scoped))?;
        bind(&mut statement, ":urlFiltered", &i64::from(url_filtered))?;
        bind(&mut statement, ":termScoresOnly", &i64::from(term_scores_only))?;
        bind_filters(&mut statement, filters)?;
        let mut rows = statement.raw_query();
        let mut found = Vec::new();
        while let Some(row) = rows.next()? {
            found.push((row.get::<_, i64>(0)?, row.get::<_, Option<f64>>(1)?));
        }
        Ok(found)
    };

    let mut scores = std::collections::BTreeMap::<i64, f64>::new();
    let mut truncated = false;
    let mut term_scores_only = pinned.is_some_and(|edge| edge.term_scores_only);
    match matching {
        Matching::Words(lexical) => {
            if let Some(terms_query) = lexical.terms_query.as_deref() {
                let sql = if oldest_first { TERMS_OLDEST_SQL } else { TERMS_NEWEST_SQL };
                let found = scan(sql, Some(terms_query), false)?;
                truncated |= pinned.is_none() && found.len() > cap;
                term_scores_only |= truncated;
                scores.extend(found.into_iter().filter_map(|(id, score)| Some((id, score?))));
                // The trigram scan looks up the term matches in the table.
                insert_ranked(connection, scores.iter().map(|(id, score)| (*id, *score)))?;
            }
            if let Some(trigram_query) = lexical.trigram_query.as_deref() {
                let sql = if oldest_first { TRIGRAM_OLDEST_SQL } else { TRIGRAM_NEWEST_SQL };
                let found = scan(sql, Some(trigram_query), term_scores_only)?;
                truncated |= pinned.is_none() && found.len() > cap;
                let mut changed = Vec::new();
                for (url_id, score) in found {
                    let Some(score) = score else { continue };
                    let best = scores.entry(url_id).or_insert(f64::INFINITY);
                    if score < *best {
                        *best = score;
                        changed.push((url_id, score));
                    }
                }
                insert_ranked(connection, changed.into_iter())?;
            }
        }
        Matching::EveryUrl => {
            let sql = if oldest_first { EVERY_URL_OLDEST_SQL } else { EVERY_URL_NEWEST_SQL };
            let found = scan(sql, None, false)?;
            truncated |= pinned.is_none() && found.len() > cap;
            scores.extend(found.into_iter().filter_map(|(id, score)| Some((id, score?))));
            insert_ranked(connection, scores.iter().map(|(id, score)| (*id, *score)))?;
        }
    }

    // Each index returned its `cap + 1` nearest `url_id`s, so the `cap` nearest of their union are
    // exactly the `cap` nearest matches overall.
    let edge = if pinned.is_some() {
        pinned
    } else if truncated || scores.len() > cap {
        let nth = cap.saturating_sub(1);
        let last = if oldest_first { scores.keys().nth(nth) } else { scores.keys().rev().nth(nth) };
        let url_id = *last.context("a window of zero URLs")?;
        connection.execute(
            if oldest_first {
                "DELETE FROM temp.history_ranked_urls WHERE url_id > ?1"
            } else {
                "DELETE FROM temp.history_ranked_urls WHERE url_id < ?1"
            },
            params![url_id],
        )?;
        scores.retain(|id, _| if oldest_first { *id <= url_id } else { *id >= url_id });
        Some(WindowEdge { oldest_first, term_scores_only, url_id })
    } else {
        None
    };
    Ok(RankedUrls { count: scores.len(), edge })
}

/// Writes `(url_id, score)` rows into `temp.history_ranked_urls`, replacing a URL's earlier score.
fn insert_ranked(connection: &Connection, rows: impl Iterator<Item = (i64, f64)>) -> Result<()> {
    let transaction = connection.unchecked_transaction()?;
    {
        let mut insert = transaction.prepare_cached(
            "INSERT OR REPLACE INTO temp.history_ranked_urls (url_id, score) VALUES (?1, ?2)",
        )?;
        for (url_id, score) in rows {
            insert.execute(params![url_id, score])?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// For a date range with few visits, collects the URLs visited in it (inside the browser and
/// profile filters) into the empty `temp.history_window_scope` and returns true.
fn scope_to_date_range(connection: &Connection, filters: &PageFilters) -> Result<bool> {
    if filters.start_time_ms.is_none() && filters.end_time_ms.is_none() {
        return Ok(false);
    }
    let start = filters.start_time_ms.unwrap_or(i64::MIN);
    let end = filters.end_time_ms.unwrap_or(i64::MAX);
    let visits: i64 = connection.query_row(
        "SELECT COUNT(*) FROM (
           SELECT 1 FROM visits
           WHERE visits.reverted_at IS NULL
             AND visits.visit_time_ms >= ?1
             AND visits.visit_time_ms <= ?2
           LIMIT ?3
         )",
        params![start, end, scope_max_visits().saturating_add(1)],
        |row| row.get(0),
    )?;
    if visits > scope_max_visits() {
        return Ok(false);
    }
    let mut statement = connection.prepare_cached(
        "INSERT OR IGNORE INTO temp.history_window_scope (url_id)
         SELECT visits.url_id
         FROM visits
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
           )",
    )?;
    bind_filters(&mut statement, filters)?;
    statement.raw_execute()?;
    Ok(true)
}

/// True when any `site:` / `intitle:` / `tag:`-style operator filter is set.
fn advanced_filters_set(connection: &Connection) -> Result<bool> {
    let sql = ADVANCED_FILTER_TABLES
        .iter()
        .map(|table| format!("EXISTS (SELECT 1 FROM temp.{table})"))
        .collect::<Vec<_>>()
        .join(" OR ");
    Ok(connection.query_row(&format!("SELECT {sql}"), [], |row| row.get(0))?)
}
