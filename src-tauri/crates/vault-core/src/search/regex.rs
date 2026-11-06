//! Responsibilities: complete, time-sliced regex matching over canonical visible visits.
//! Not responsible for: lexical ranking, UI scheduling or a newest-record window.
//! Failure modes: an empty chunk ends search, repeated visits duplicate pages, a cursor
//! skips equal timestamps, cancellation waits for the full archive, a pattern with a
//! literal fragment still reads every visit, most visited ranks a page before the walk has
//! counted all its visits.
//! Tests: search::tests::regex_steps_scan_past_empty_chunks_and_page_without_duplicates,
//! ported_regex, most_visited_tests and the plan tests below.
use super::{SearchProgress, eligibility::Scope};
use anyhow::{Context, Result};
use regex::RegexBuilder;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use vault_search::{Hit, RankCollector, SortOrder};
use vault_store::{CancelToken, Db, Store, Value};

/// Candidate pages read per query.
const CANDIDATE_PAGES: usize = 256;

/// Where the scan reads visits from.
enum Source {
    /// The index is still listing the pages that contain the pattern's fragment.
    Listing(Box<super::regex_prefilter::Prefilter>),
    /// Only those pages: the work follows the matches, not the archive's size.
    Candidates { pages: Vec<u64>, next: usize },
    /// No usable fragment: every visible visit after a (time, id) cursor, newest first, or
    /// oldest first for that order, so the first visit seen of a page is the one it goes by.
    Walk { time: i64, id: i64 },
}
impl Source {
    fn walk_start(oldest: bool) -> Self {
        if oldest {
            Source::Walk { time: i64::MIN, id: i64::MIN }
        } else {
            Source::Walk { time: i64::MAX, id: i64::MAX }
        }
    }
}

/// The visit a result row shows: the newest matching one, or the first for oldest first.
#[derive(Clone, Copy)]
enum Shown {
    Visit(i64),
    /// Candidate reads group by URL; the URL's visit at the winner's time is looked up when the
    /// row is shown.
    Url(i64),
}

/// What the scan keeps of a page it may show.
struct Winner {
    /// The time of the visit the order goes by.
    time: i64,
    /// The page's matching visits read so far; all of them once its key is final.
    visits: u64,
    shown: Shown,
}
impl Winner {
    fn hit(&self, page_id: u64) -> Hit {
        Hit { page_id, visit_ms: self.time, score: 0.0, visits: self.visits }
    }
}

pub(super) struct Scan {
    pattern: regex::Regex,
    scope: Scope,
    source: Source,
    seen: Vec<u64>,
    pub hits: Vec<Hit>,
    pub count: u64,
    pub visits: u64,
    pub complete: bool,
    after: Option<Hit>,
    limit: usize,
    processed: u64,
    /// The pages the scan may still show. Usually only those in `hits`; while a walk ranks by
    /// visits, every matching page it has met (memory grows with the matches, not the archive),
    /// because a page's count is complete only once the walk has read all its visits.
    winners: HashMap<u64, Winner>,
    /// Pages whose count the last walk chunk changed, while a walk ranks by visits.
    touched: Vec<u64>,
    order: SortOrder,
}
impl Scan {
    pub fn new(
        pattern: &str,
        scope: Scope,
        limit: usize,
        order: SortOrder,
        indexer: &super::Indexer,
        store: &Store,
    ) -> Result<Self> {
        // A regex match has no relevance score: best match lists the newest matches first.
        let order = if order == SortOrder::Relevance { SortOrder::Newest } else { order };
        let source = match super::regex_prefilter::Prefilter::start(indexer, store, pattern)? {
            Some(prefilter) => Source::Listing(Box::new(prefilter)),
            None => Source::walk_start(order == SortOrder::Oldest),
        };
        Ok(Self {
            source,
            pattern: build_history_regex(pattern)?,
            scope,
            seen: Vec::new(),
            hits: Vec::new(),
            count: 0,
            visits: 0,
            complete: false,
            after: None,
            limit,
            processed: 0,
            winners: HashMap::new(),
            touched: Vec::new(),
            order,
        })
    }
    pub fn next_page(&mut self, after: Option<Hit>) {
        self.after = after;
        self.reset_scan();
    }
    fn oldest(&self) -> bool {
        self.order == SortOrder::Oldest
    }
    /// Whether a page's key can still change after the scan first meets it. Only its visit
    /// count can, and only in a walk: a candidate read counts every visit of its pages at once.
    fn counts_grow(&self) -> bool {
        self.order == SortOrder::Visits && matches!(self.source, Source::Walk { .. })
    }
    fn reset_scan(&mut self) {
        match &mut self.source {
            Source::Listing(_) => {}
            Source::Candidates { next, .. } => *next = 0,
            Source::Walk { .. } => self.source = Source::walk_start(self.oldest()),
        }
        self.hits.clear();
        self.winners.clear();
        self.touched.clear();
        self.seen.fill(0);
        self.count = 0;
        self.visits = 0;
        self.complete = false;
        self.processed = 0;
    }
    pub fn step(
        &mut self,
        store: &Store,
        cancel: &CancelToken,
        budget: Duration,
        returned: &[u64],
    ) -> Result<()> {
        let started = Instant::now();
        if let Source::Listing(prefilter) = &mut self.source {
            match prefilter.step(budget) {
                Ok(false) => return Ok(()),
                Ok(true) => {
                    let pages = prefilter.candidates();
                    self.source = Source::Candidates { pages, next: 0 };
                }
                // A damaged index can't vouch for its matches; read every visit instead.
                Err(_) => self.source = Source::walk_start(self.oldest()),
            }
        }
        while !self.complete {
            cancel.check()?;
            match self.source {
                Source::Candidates { .. } => self.read_candidates(store, returned)?,
                _ => self.walk_chunk(store, returned)?,
            }
            self.rank();
            if started.elapsed() >= budget {
                break;
            }
        }
        Ok(())
    }
    /// Puts the best `limit` pages the scan has met in `hits`, and forgets the rest when their
    /// keys are final. While a walk ranks by visits, keys grow: each chunk re-ranks the pages
    /// it counted, and the end of the walk ranks every matching page against the cursor once.
    fn rank(&mut self) {
        if self.counts_grow() {
            let mut top = RankCollector::new(self.limit, self.after, self.order);
            if self.complete {
                self.touched.clear();
                for (page, winner) in &self.winners {
                    top.collect(winner.hit(*page));
                }
            } else {
                self.touched.extend(self.hits.iter().map(|hit| hit.page_id));
                self.touched.sort_unstable();
                self.touched.dedup();
                for page in self.touched.drain(..) {
                    top.collect(self.winners[&page].hit(page));
                }
            }
            self.hits = top.hits();
            if !self.complete {
                return;
            }
        } else {
            let order = self.order;
            self.hits.sort_unstable_by(|a, b| order.compare(b, a));
            self.hits.truncate(self.limit);
        }
        let shown = self.hits.iter().map(|hit| hit.page_id).collect::<HashSet<_>>();
        self.winners.retain(|page, _| shown.contains(page));
    }
    /// Whether a page with this final key belongs after the cursor.
    fn after_cursor(&self, hit: &Hit) -> bool {
        self.after.is_none_or(|after| self.order.compare(hit, &after).is_lt())
    }
    /// The next candidate pages: one row per URL with a visible visit in scope.
    fn read_candidates(&mut self, store: &Store, returned: &[u64]) -> Result<()> {
        let Source::Candidates { pages, next } = &mut self.source else { unreachable!() };
        let batch = pages[*next..pages.len().min(*next + CANDIDATE_PAGES)].to_vec();
        *next += batch.len();
        self.complete = *next >= pages.len();
        if batch.is_empty() {
            return Ok(());
        }
        let oldest = self.oldest();
        let mut params = self.scope.params.clone();
        let first = params.len() + 1;
        params.extend(batch.iter().map(|page| Value::Integer(*page as i64)));
        let sql = candidates_sql(&self.scope.sql, first, batch.len(), oldest);
        let rows = store.read(Db::Archive, move |s| {
            s.query(&sql, params, |r| {
                Ok((
                    r.get::<i64>(0)?,
                    r.get::<i64>(1)? as u64,
                    r.get::<String>(2)?,
                    r.get::<String>(3)?,
                    r.get::<i64>(4)? as u64,
                    r.get::<i64>(5)?,
                ))
            })
        })?;
        // page -> (the matching visit time it goes by, its URL, matching visits)
        let mut matched: HashMap<u64, (i64, i64, u64)> = HashMap::new();
        for (url_id, page, url, title, visits, time) in rows {
            self.processed += visits;
            if !(self.pattern.is_match(&url) || self.pattern.is_match(&title)) {
                continue;
            }
            let entry = matched.entry(page).or_insert((time, url_id, 0));
            let goes_by = if oldest {
                (time, url_id) < (entry.0, entry.1)
            } else {
                (time, url_id) > (entry.0, entry.1)
            };
            if goes_by {
                (entry.0, entry.1) = (time, url_id);
            }
            entry.2 += visits;
        }
        // Every URL of these pages was read, so each page's key is final here.
        for (page, (time, url, visits)) in matched {
            self.visits += visits;
            self.count += 1;
            if super::session::already_returned(returned, page) {
                continue;
            }
            let winner = Winner { time, visits, shown: Shown::Url(url) };
            if !self.after_cursor(&winner.hit(page)) {
                continue;
            }
            self.hits.push(winner.hit(page));
            self.winners.insert(page, winner);
        }
        Ok(())
    }
    /// The next 512 visible visits past the cursor.
    fn walk_chunk(&mut self, store: &Store, returned: &[u64]) -> Result<()> {
        let Source::Walk { time: cursor_time, id: cursor_id } = self.source else { unreachable!() };
        let counts_grow = self.counts_grow();
        let mut params = self.scope.params.clone();
        let time = params.len() + 1;
        params.push(Value::Integer(cursor_time));
        let id = params.len() + 1;
        params.push(Value::Integer(cursor_id));
        let sql = chunk_sql(&self.scope.sql, time, id, self.oldest());
        let rows = store.read(Db::Archive, move |s| {
            s.query(&sql, params, |r| {
                Ok((
                    r.get::<i64>(0)?,
                    r.get::<i64>(1)?,
                    r.get::<i64>(2)? as u64,
                    r.get::<String>(3)?,
                    r.get::<String>(4)?,
                ))
            })
        })?;
        self.complete = rows.len() < 512;
        for (time, id, page, url, title) in rows {
            self.source = Source::Walk { time, id };
            self.processed += 1;
            if !(self.pattern.is_match(&url) || self.pattern.is_match(&title)) {
                continue;
            }
            self.visits += 1;
            let word = page as usize / 64;
            if self.seen.len() <= word {
                self.seen.resize(word + 1, 0);
            }
            let mask = 1u64 << (page % 64);
            if self.seen[word] & mask != 0 {
                if let Some(winner) = self.winners.get_mut(&page) {
                    winner.visits += 1;
                    if counts_grow {
                        self.touched.push(page);
                    }
                }
                continue;
            }
            self.seen[word] |= mask;
            self.count += 1;
            if super::session::already_returned(returned, page) {
                continue;
            }
            let winner = Winner { time, visits: 1, shown: Shown::Visit(id) };
            if counts_grow {
                // The count is known only once the walk ends; `rank` checks the cursor then.
                self.touched.push(page);
            } else if self.after_cursor(&winner.hit(page)) {
                self.hits.push(winner.hit(page));
            } else {
                continue;
            }
            self.winners.insert(page, winner);
        }
        Ok(())
    }
    pub fn hydrate(&self, store: &Store) -> Result<Vec<super::SearchHit>> {
        let shown = self
            .hits
            .iter()
            .map(|hit| {
                let winner = &self.winners[&hit.page_id];
                (hit.page_id, winner.shown, winner.time)
            })
            .collect::<Vec<_>>();
        let scope = self.scope.clone();
        let oldest = self.oldest();
        // A URL found by a candidate read shows its visit at the matching time.
        let ids = store.read(Db::Archive, move |s| {
            let sql = shown_visit_sql(&scope.sql, scope.params.len() + 1, oldest);
            let mut ids = Vec::with_capacity(shown.len());
            for (page, shown, time) in shown {
                match shown {
                    Shown::Visit(id) => ids.push(id),
                    Shown::Url(url) => {
                        let mut params = scope.params.clone();
                        params.extend([page as i64, url, time].map(Value::Integer));
                        ids.extend(s.query(&sql, params, |r| Ok(r.get::<i64>(0)?))?);
                    }
                }
            }
            Ok(ids)
        })?;
        let mut rows = super::visits::evidence_from(store, &ids, &self.scope)?
            .into_iter()
            .map(|hit| (hit.page_id as u64, hit))
            .collect::<HashMap<_, _>>();
        Ok(self
            .hits
            .iter()
            .filter_map(|hit| {
                let mut row = rows.remove(&hit.page_id)?;
                if !(self.pattern.is_match(&row.visit.url)
                    || row.visit.title.as_ref().is_some_and(|title| self.pattern.is_match(title)))
                {
                    return None;
                }
                row.visit.visit_count = Some(self.winners[&hit.page_id].visits);
                row.visit.enrichment_excerpt = None;
                Some(row)
            })
            .collect())
    }
    pub fn progress(&self) -> SearchProgress {
        let processed = match &self.source {
            Source::Listing(prefilter) => prefilter.processed,
            _ => self.processed,
        };
        SearchProgress { documents_processed: processed, ..Default::default() }
    }
}

/// One row per URL of the given pages with a visible visit in scope: its visits and its
/// newest visit's time, or its first's for oldest first.
fn candidates_sql(scope: &str, first: usize, pages: usize, oldest: bool) -> String {
    let keys = (first..first + pages).map(|n| format!("?{n}")).collect::<Vec<_>>().join(",");
    let time = if oldest { "MIN" } else { "MAX" };
    format!(
        "SELECT u.id,u.page_id,p.url,COALESCE(u.title,''),COUNT(*),{time}(v.visit_time_ms) FROM {} WHERE p.id IN ({keys}) AND {scope} GROUP BY u.id",
        super::eligibility::PAGE_VISITS
    )
}

/// The visit of one URL (of a page) at a given time, inside the scope: the last of those at
/// that time, or the first for oldest first.
fn shown_visit_sql(scope: &str, first: usize, oldest: bool) -> String {
    let (page, url, time) = (first, first + 1, first + 2);
    let order = if oldest { "ASC" } else { "DESC" };
    format!(
        "SELECT v.id FROM {} WHERE p.id=?{page} AND u.id=?{url} AND v.visit_time_ms=?{time} AND {scope} ORDER BY v.id {order} LIMIT 1",
        super::eligibility::PAGE_VISITS
    )
}

/// The next 512 visits past the cursor: older ones, or newer ones for oldest first. The
/// `visit_time_ms<=` (`>=`) bound is what lets the planner seek to the cursor: with only the
/// OR of the two cases, Turso scans the time index from its end for every chunk, so a scan of
/// the whole archive grows with its square.
fn chunk_sql(scope: &str, time: usize, id: usize, oldest: bool) -> String {
    let (bound, past, order) = if oldest { (">=", ">", "ASC") } else { ("<=", "<", "DESC") };
    format!(
        "SELECT v.visit_time_ms,v.id,u.page_id,p.url,COALESCE(u.title,'') FROM visits v INDEXED BY idx_visits_visible_time_id CROSS JOIN urls u ON u.id=v.url_id CROSS JOIN pages p ON p.id=u.page_id JOIN source_profiles sp ON sp.id=v.source_profile_id WHERE {scope} AND v.visit_time_ms{bound}?{time} AND (v.visit_time_ms{past}?{time} OR v.id{past}?{id}) ORDER BY v.visit_time_ms {order},v.id {order} LIMIT 512"
    )
}

/// Compiles a History regex: Rust `regex` syntax, case-insensitive.
///
/// The one place the dialect is decided. The History screen checks patterns against the same
/// rules before sending them (`src/features/history/regex-dialect.ts`); `history-tools.spec`
/// checks that a pattern Rust cannot run is explained there and never sent.
pub(crate) fn build_history_regex(pattern: &str) -> Result<regex::Regex> {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .with_context(|| format!("invalid regex pattern `{pattern}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_of(sql: impl Fn(&Scope) -> String, extra: &[i64]) -> String {
        let vault = crate::Vault::open_temp().unwrap();
        let mut scope = Scope::new(
            &super::super::SearchRequest::default(),
            &crate::archive::search_query::ParsedHistorySearchQuery::default(),
        );
        scope.pin_visits(100);
        let mut params = scope.params.clone();
        params.extend(extra.iter().map(|value| Value::Integer(*value)));
        let sql = format!("EXPLAIN QUERY PLAN {}", sql(&scope));
        vault
            .store()
            .read(Db::Archive, move |s| s.query(&sql, params, |r| Ok(r.get::<String>(3)?)))
            .unwrap()
            .join("\n")
    }

    #[test]
    fn each_chunk_seeks_to_the_cursor_instead_of_scanning_from_the_end() {
        for (oldest, bound) in [(false, "visit_time_ms<"), (true, "visit_time_ms>")] {
            let plan = plan_of(
                |scope| {
                    chunk_sql(&scope.sql, scope.params.len() + 1, scope.params.len() + 2, oldest)
                },
                &[1_000, 50],
            );
            assert!(
                plan.contains(&format!("SEARCH v USING INDEX idx_visits_visible_time_id ({bound}")),
                "{plan}"
            );
            assert!(!plan.to_lowercase().contains("temp b-tree"), "{plan}");
        }
    }

    #[test]
    fn candidate_pages_seek_their_urls_and_visits_without_scanning_the_archive() {
        let plan = plan_of(
            |scope| candidates_sql(&scope.sql, scope.params.len() + 1, 3, false),
            &[1, 2, 3],
        );
        assert!(plan.contains("idx_urls_page_id"), "{plan}");
        assert!(plan.contains("idx_visits_visible_url_time"), "{plan}");
        assert!(
            !plan.contains("SCAN p") && !plan.contains("SCAN u") && !plan.contains("SCAN v"),
            "{plan}"
        );

        let plan =
            plan_of(|scope| shown_visit_sql(&scope.sql, scope.params.len() + 1, false), &[1, 2, 3]);
        assert!(plan.contains("idx_visits_visible_url_time"), "{plan}");
        assert!(
            !plan.contains("SCAN p") && !plan.contains("SCAN u") && !plan.contains("SCAN v"),
            "{plan}"
        );
    }
}
