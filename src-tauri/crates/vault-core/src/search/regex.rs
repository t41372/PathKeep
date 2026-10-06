//! Responsibilities: complete, time-sliced regex matching over canonical visible visits.
//! Not responsible for: lexical ranking, UI scheduling or a newest-record window.
//! Failure modes: an empty chunk ends search, repeated visits duplicate pages, a cursor
//! skips equal timestamps, cancellation waits for the full archive.
//! Tests: search::tests::regex_steps_scan_past_empty_chunks_and_page_without_duplicates.
use super::{SearchProgress, eligibility::Scope};
use anyhow::{Context, Result};
use regex::RegexBuilder;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use vault_store::{CancelToken, Db, Store, Value};

pub(super) struct Scan {
    pattern: regex::Regex,
    scope: Scope,
    seen: Vec<u64>,
    time: i64,
    id: i64,
    pub hits: Vec<vault_search::Hit>,
    pub count: u64,
    pub visits: u64,
    pub complete: bool,
    after: Option<vault_search::Hit>,
    limit: usize,
    processed: u64,
    winners: HashMap<u64, (i64, u64)>,
    oldest: bool,
    prefilter: Option<super::regex_prefilter::Prefilter>,
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
        let prefilter = super::regex_prefilter::Prefilter::start(indexer, store, pattern)?;
        Ok(Self {
            prefilter,
            pattern: build_history_regex(pattern)?,
            scope,
            seen: Vec::new(),
            time: i64::MAX,
            id: i64::MAX,
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
    pub(super) fn reset_scan(&mut self) {
        self.hits.clear();
        self.winners.clear();
        self.time = i64::MAX;
        self.id = i64::MAX;
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
        if let Some(prefilter) = &mut self.prefilter {
            let seq = store.read(Db::Archive, super::documents::watermark)?;
            if seq != prefilter.seq {
                self.prefilter = None;
                self.reset_scan();
            } else {
                match prefilter.step(budget) {
                    Ok(false) => return Ok(()),
                    Ok(true) if prefilter.pages == 0 => {
                        self.complete = true;
                        return Ok(());
                    }
                    Ok(true) => {}
                    Err(_) => {
                        self.prefilter = None;
                        self.reset_scan();
                    }
                }
            }
        }
        while !self.complete {
            cancel.check()?;
            let mut params = self.scope.params.clone();
            let time = params.len() + 1;
            params.push(Value::Integer(self.time));
            let id = params.len() + 1;
            params.push(Value::Integer(self.id));
            let sql = format!(
                "SELECT v.visit_time_ms,v.id,u.page_id,p.url,COALESCE(u.title,'') FROM visits v INDEXED BY idx_visits_visible_time_id CROSS JOIN urls u ON u.id=v.url_id CROSS JOIN pages p ON p.id=u.page_id JOIN source_profiles sp ON sp.id=v.source_profile_id WHERE {} AND (v.visit_time_ms<?{time} OR (v.visit_time_ms=?{time} AND v.id<?{id})) ORDER BY v.visit_time_ms DESC,v.id DESC LIMIT 512",
                self.scope.sql
            );
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
                self.time = time;
                self.id = id;
                self.processed += 1;
                if self.prefilter.as_ref().is_some_and(|filter| !filter.accepts(page)) {
                    continue;
                }
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
                self.winners.insert(page, (id, 1));
                self.hits.push(hit);
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
    fn compare(&self, a: &vault_search::Hit, b: &vault_search::Hit) -> std::cmp::Ordering {
        rank(a, b, self.oldest)
    }
    pub fn hydrate(&self, store: &Store) -> Result<Vec<super::SearchHit>> {
        let ids = self.hits.iter().map(|hit| self.winners[&hit.page_id].0).collect::<Vec<_>>();
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
        SearchProgress { documents_processed: self.processed, ..Default::default() }
    }
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
/// rules before sending them (`src/features/history/regex-dialect.ts`); the shared case table
/// `regex_dialect_cases` keeps the two in step.
pub(crate) fn build_history_regex(pattern: &str) -> Result<regex::Regex> {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .with_context(|| format!("invalid regex pattern `{pattern}`"))
}
