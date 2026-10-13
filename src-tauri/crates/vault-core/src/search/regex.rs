//! Responsibilities: complete, time-sliced regex matching over canonical visible visits.
//! Not responsible for: lexical ranking, UI scheduling or a newest-record window.
//! Failure modes: an empty chunk ends search, repeated visits duplicate pages, a cursor
//! skips equal timestamps, cancellation waits for the full archive, a pattern with a
//! literal fragment still reads every visit.
//! Tests: search::tests::regex_steps_scan_past_empty_chunks_and_page_without_duplicates,
//! ported_regex, and the plan tests below.
use super::{SearchProgress, eligibility::Scope};
use anyhow::{Context, Result};
use regex::RegexBuilder;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use vault_store::{CancelToken, Db, Store, Value};

/// Candidate pages read per query.
const CANDIDATE_PAGES: usize = 256;

/// Where the scan reads visits from.
enum Source {
    /// The index is still listing the pages that contain the pattern's fragment.
    Listing(Box<super::regex_prefilter::Prefilter>),
    /// Only those pages: the work follows the matches, not the archive's size.
    Candidates { pages: Vec<u64>, next: usize },
    /// No usable fragment: every visible visit, newest first, after a (time, id) cursor.
    Walk { time: i64, id: i64 },
}

/// The visit a result row shows: the newest matching one.
#[derive(Clone, Copy)]
enum Newest {
    Visit(i64),
    /// Candidate reads group by URL; the visit is looked up when the row is shown.
    Url {
        url: i64,
        time: i64,
    },
}

pub(super) struct Scan {
    pattern: regex::Regex,
    scope: Scope,
    source: Source,
    seen: Vec<u64>,
    pub hits: Vec<vault_search::Hit>,
    pub count: u64,
    pub visits: u64,
    pub complete: bool,
    after: Option<vault_search::Hit>,
    limit: usize,
    processed: u64,
    winners: HashMap<u64, (Newest, u64)>,
    oldest: bool,
}
impl Scan {
    pub fn new(
        pattern: &str,
        scope: Scope,
        limit: usize,
        oldest: bool,
        indexer: &super::Indexer,
        store: &Store,
    ) -> Result<Self> {
        let source = match super::regex_prefilter::Prefilter::start(indexer, store, pattern)? {
            Some(prefilter) => Source::Listing(Box::new(prefilter)),
            None => Source::Walk { time: i64::MAX, id: i64::MAX },
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
            oldest,
        })
    }
    pub fn next_page(&mut self, after: Option<vault_search::Hit>) {
        self.after = after;
        self.reset_scan();
    }
    fn reset_scan(&mut self) {
        match &mut self.source {
            Source::Listing(_) => {}
            Source::Candidates { next, .. } => *next = 0,
            Source::Walk { time, id } => (*time, *id) = (i64::MAX, i64::MAX),
        }
        self.hits.clear();
        self.winners.clear();
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
                Err(_) => self.source = Source::Walk { time: i64::MAX, id: i64::MAX },
            }
        }
        while !self.complete {
            cancel.check()?;
            match self.source {
                Source::Candidates { .. } => self.read_candidates(store, returned)?,
                _ => self.walk_chunk(store, returned)?,
            }
            let oldest = self.oldest;
            self.hits.sort_unstable_by(|a, b| rank(b, a, oldest));
            self.hits.truncate(self.limit);
            self.winners.retain(|page, _| self.hits.iter().any(|hit| hit.page_id == *page));
            if started.elapsed() >= budget {
                break;
            }
        }
        Ok(())
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
        let mut params = self.scope.params.clone();
        let first = params.len() + 1;
        params.extend(batch.iter().map(|page| Value::Integer(*page as i64)));
        let sql = candidates_sql(&self.scope.sql, first, batch.len());
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
        // page -> (newest matching visit time, its URL, matching visits)
        let mut matched: HashMap<u64, (i64, i64, u64)> = HashMap::new();
        for (url_id, page, url, title, visits, time) in rows {
            self.processed += visits;
            if !(self.pattern.is_match(&url) || self.pattern.is_match(&title)) {
                continue;
            }
            let entry = matched.entry(page).or_insert((time, url_id, 0));
            if (time, url_id) > (entry.0, entry.1) {
                (entry.0, entry.1) = (time, url_id);
            }
            entry.2 += visits;
        }
        for (page, (time, url, visits)) in matched {
            self.visits += visits;
            self.count += 1;
            if super::session::already_returned(returned, page) {
                continue;
            }
            let hit = vault_search::Hit { page_id: page, newest_visit_ms: time, score: 0.0 };
            if self
                .after
                .is_some_and(|after| self.compare(&hit, &after) != std::cmp::Ordering::Less)
            {
                continue;
            }
            self.winners.insert(page, (Newest::Url { url, time }, visits));
            self.hits.push(hit);
        }
        Ok(())
    }
    /// The next 512 visible visits older than the cursor.
    fn walk_chunk(&mut self, store: &Store, returned: &[u64]) -> Result<()> {
        let Source::Walk { time: cursor_time, id: cursor_id } = self.source else { unreachable!() };
        let mut params = self.scope.params.clone();
        let time = params.len() + 1;
        params.push(Value::Integer(cursor_time));
        let id = params.len() + 1;
        params.push(Value::Integer(cursor_id));
        let sql = chunk_sql(&self.scope.sql, time, id);
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
                if let Some((_, count)) = self.winners.get_mut(&page) {
                    *count += 1;
                }
                continue;
            }
            self.seen[word] |= mask;
            self.count += 1;
            if super::session::already_returned(returned, page) {
                continue;
            }
            let hit = vault_search::Hit { page_id: page, newest_visit_ms: time, score: 0.0 };
            if self
                .after
                .is_some_and(|after| self.compare(&hit, &after) != std::cmp::Ordering::Less)
            {
                continue;
            }
            self.winners.insert(page, (Newest::Visit(id), 1));
            self.hits.push(hit);
        }
        Ok(())
    }
    fn compare(&self, a: &vault_search::Hit, b: &vault_search::Hit) -> std::cmp::Ordering {
        rank(a, b, self.oldest)
    }
    pub fn hydrate(&self, store: &Store) -> Result<Vec<super::SearchHit>> {
        let newest = self
            .hits
            .iter()
            .map(|hit| (hit.page_id, self.winners[&hit.page_id].0))
            .collect::<Vec<_>>();
        let scope = self.scope.clone();
        // A URL found by a candidate read shows its newest visit at the matching time.
        let ids = store.read(Db::Archive, move |s| {
            let sql = newest_visit_sql(&scope.sql, scope.params.len() + 1);
            let mut ids = Vec::with_capacity(newest.len());
            for (page, newest) in newest {
                match newest {
                    Newest::Visit(id) => ids.push(id),
                    Newest::Url { url, time } => {
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
                row.visit.visit_count = Some(self.winners[&hit.page_id].1);
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

/// One row per URL of the given pages with a visible visit in scope: its visits and newest time.
fn candidates_sql(scope: &str, first: usize, pages: usize) -> String {
    let keys = (first..first + pages).map(|n| format!("?{n}")).collect::<Vec<_>>().join(",");
    format!(
        "SELECT u.id,u.page_id,p.url,COALESCE(u.title,''),COUNT(*),MAX(v.visit_time_ms) FROM {} WHERE p.id IN ({keys}) AND {scope} GROUP BY u.id",
        super::eligibility::PAGE_VISITS
    )
}

/// The newest visit of one URL (of a page) at a given time, inside the scope.
fn newest_visit_sql(scope: &str, first: usize) -> String {
    let (page, url, time) = (first, first + 1, first + 2);
    format!(
        "SELECT v.id FROM {} WHERE p.id=?{page} AND u.id=?{url} AND v.visit_time_ms=?{time} AND {scope} ORDER BY v.id DESC LIMIT 1",
        super::eligibility::PAGE_VISITS
    )
}

/// The next 512 visits older than the cursor. The `visit_time_ms<=` bound is
/// what lets the planner seek to the cursor: with only the OR of the two cases,
/// Turso scans the time index from the newest visit for every chunk, so a scan
/// of the whole archive grows with its square.
fn chunk_sql(scope: &str, time: usize, id: usize) -> String {
    format!(
        "SELECT v.visit_time_ms,v.id,u.page_id,p.url,COALESCE(u.title,'') FROM visits v INDEXED BY idx_visits_visible_time_id CROSS JOIN urls u ON u.id=v.url_id CROSS JOIN pages p ON p.id=u.page_id JOIN source_profiles sp ON sp.id=v.source_profile_id WHERE {scope} AND v.visit_time_ms<=?{time} AND (v.visit_time_ms<?{time} OR v.id<?{id}) ORDER BY v.visit_time_ms DESC,v.id DESC LIMIT 512"
    )
}

fn rank(a: &vault_search::Hit, b: &vault_search::Hit, oldest: bool) -> std::cmp::Ordering {
    (if oldest {
        b.newest_visit_ms.cmp(&a.newest_visit_ms)
    } else {
        a.newest_visit_ms.cmp(&b.newest_visit_ms)
    })
    .then(a.page_id.cmp(&b.page_id))
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
    fn each_chunk_seeks_to_the_cursor_instead_of_scanning_from_the_newest_visit() {
        let plan = plan_of(
            |scope| chunk_sql(&scope.sql, scope.params.len() + 1, scope.params.len() + 2),
            &[1_000, 50],
        );
        assert!(
            plan.contains("SEARCH v USING INDEX idx_visits_visible_time_id (visit_time_ms<"),
            "{plan}"
        );
        assert!(!plan.to_lowercase().contains("temp b-tree"), "{plan}");
    }

    #[test]
    fn candidate_pages_seek_their_urls_and_visits_without_scanning_the_archive() {
        let plan =
            plan_of(|scope| candidates_sql(&scope.sql, scope.params.len() + 1, 3), &[1, 2, 3]);
        assert!(plan.contains("idx_urls_page_id"), "{plan}");
        assert!(plan.contains("idx_visits_visible_url_time"), "{plan}");
        assert!(
            !plan.contains("SCAN p") && !plan.contains("SCAN u") && !plan.contains("SCAN v"),
            "{plan}"
        );

        let plan =
            plan_of(|scope| newest_visit_sql(&scope.sql, scope.params.len() + 1), &[1, 2, 3]);
        assert!(plan.contains("idx_visits_visible_url_time"), "{plan}");
        assert!(
            !plan.contains("SCAN p") && !plan.contains("SCAN u") && !plan.contains("SCAN v"),
            "{plan}"
        );
    }
}
