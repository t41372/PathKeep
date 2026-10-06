//! Responsibilities: pin posting walks, join canonical eligibility in bounded batches,
//! collect global top-k, count independently and bind continuation keys.
//! Not responsible for: transport scheduling, database ownership or index writes.
//! Failure modes: canonical scans delay every first result; counts block ranking;
//! provisional cursors escape; backup/revocation moves pages across cursors;
//! revokes leak during hydration; close retains handles.
//! Tests: search::tests, ported_grouped, ported_regex, engine canonical contracts.
use super::{
    Indexer, SearchCount, SearchProgress, SearchRequest, SearchStep,
    eligibility::{self, Membership, Scope},
    query,
};
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::{
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use vault_search::{Hit, MatchHandle, RankCollector, SortOrder};
use vault_store::{CancelToken, Db, Store};
pub(crate) type Scores = HashMap<u64, (f32, &'static str)>;
const CANDIDATE_BATCH: usize = 1024;

/// A session retains one immutable index snapshot and its canonical page membership.
pub(super) struct RunningSession {
    store: Weak<Store>,
    indexer: Weak<Indexer>,
    scope: Scope,
    scores: Option<Arc<Scores>>,
    notes: Vec<crate::AiSearchNote>,
    ast: vault_search::Ast,
    exact_ast: vault_search::Ast,
    fuzzy: Option<crate::archive::search_lexical::FuzzyQuery>,
    reader: Option<vault_search::SearchSession>,
    rank: Option<MatchHandle>,
    count: Option<MatchHandle>,
    collector: RankCollector,
    membership: Membership,
    canonical_seq: u64,
    rank_done: bool,
    count_done: bool,
    matched_pages: u64,
    matched_visits: u64,
    rank_progress: SearchProgress,
    count_progress: SearchProgress,
    cursor: Option<Hit>,
    after: Option<Hit>,
    token: Option<String>,
    page_size: usize,
    page: u64,
    returned: Vec<u64>,
    page_ids: Vec<u64>,
    order: SortOrder,
    cancel: CancelToken,
    regex: Option<super::regex::Scan>,
    fallback: Option<SearchRequest>,
}
impl RunningSession {
    pub(super) fn start_owned(
        store: &Arc<Store>,
        indexer: &Arc<Indexer>,
        request: SearchRequest,
        cancel: CancelToken,
    ) -> Result<Self> {
        anyhow::ensure!(request.page_size > 0, "page size must be positive");
        let mut query = if request.regex {
            query::build("", false)
        } else {
            query::build(&request.query, request.fuzzy)
        };
        let request = SearchRequest { starred: request.starred || query.starred, ..request };
        let mut scope = Scope::new(&request, &query.filters);
        let (canonical_seq, through) = store.read(Db::Archive, |s| {
            let through = s
                .query("SELECT id FROM visits ORDER BY id DESC LIMIT 1", (), |r| {
                    Ok(r.get::<i64>(0)?)
                })?
                .pop()
                .unwrap_or(0);
            Ok((super::documents::watermark(s)?, through))
        })?;
        scope.pin_visits(through);
        let regex = if request.regex {
            Some(super::regex::Scan::new(
                &request.query,
                scope.clone(),
                request.page_size + 1,
                request.sort.as_deref() == Some("oldest"),
                indexer,
                store,
            )?)
        } else {
            None
        };
        let fuzzy = query
            .filters
            .keyword_text
            .as_deref()
            .and_then(crate::archive::search_lexical::fuzzy_query);
        let fallback = (!request.fuzzy && fuzzy.is_some() && regex.is_none())
            .then(|| SearchRequest { fuzzy: true, ..request.clone() });
        // Date scopes use canonical range indexes; fuzzy candidates are validated per batch.
        // Ordinary ranking starts immediately; each posting batch resolves its own pages.
        let eager = !request.fuzzy
            && (request.start_time_ms.is_some()
                || request.end_time_ms.is_some()
                || query.filters.after_ms.is_some()
                || query.filters.before_ms.is_some());
        let indexed_seq = indexer.index()?.last_applied_seq()?;
        let membership = if eager && regex.is_none() && indexed_seq >= canonical_seq {
            eligibility::load(store, scope.clone(), cancel.clone())?
        } else {
            Membership::empty(store)?
        };
        let order = match request.sort.as_deref() {
            Some("newest") => SortOrder::Newest,
            Some("oldest") => SortOrder::Oldest,
            _ => SortOrder::Relevance,
        };
        let profile_id = if let Some(profile) = request.profile_id.clone() {
            let ids = store.read(Db::Archive, move |s| {
                s.query("SELECT id FROM source_profiles WHERE profile_key=?1", [profile], |r| {
                    Ok(r.get::<i64>(0)? as u64)
                })
            })?;
            let id = ids.into_iter().next();
            if id.is_none() {
                query.ast = vault_search::Ast::Or(Vec::new());
            }
            id
        } else {
            None
        };
        let reader = if regex.is_none() {
            Some(indexer.index()?.search(
                query.ast.clone(),
                vault_search::Filters { order, profile_id, ..Default::default() },
            )?)
        } else {
            None
        };
        let rank = reader.as_ref().map(|r| r.start_matches(true)).transpose()?;
        let count = reader.as_ref().map(|r| r.start_matches(false)).transpose()?;
        Ok(Self {
            store: Arc::downgrade(store),
            indexer: Arc::downgrade(indexer),
            scope,
            scores: None,
            notes: Vec::new(),
            ast: query.ast,
            exact_ast: query.exact,
            fuzzy: if request.fuzzy { fuzzy } else { None },
            reader,
            rank,
            count,
            collector: RankCollector::new(request.page_size + 1, None, order),
            membership,
            canonical_seq,
            rank_done: false,
            count_done: false,
            matched_pages: 0,
            matched_visits: 0,
            rank_progress: SearchProgress::default(),
            count_progress: SearchProgress::default(),
            cursor: None,
            after: None,
            token: None,
            page_size: request.page_size,
            page: 1,
            returned: Vec::new(),
            page_ids: Vec::new(),
            order,
            cancel,
            regex,
            fallback,
        })
    }
    pub fn step(&mut self, budget: Duration) -> Result<SearchStep> {
        let store = self.store.upgrade().filter(|s| s.is_open());
        if store.is_none() || self.cancel.check().is_err() {
            self.cancel();
            bail!("search session is closed or cancelled")
        }
        let store = store.unwrap();
        let canonical_seq = store.read(Db::Archive, super::documents::watermark)?;
        if canonical_seq != self.canonical_seq {
            self.canonical_seq = canonical_seq;
            if let Some(regex) = &mut self.regex {
                regex.reset_scan();
            } else {
                self.membership = Membership::empty(&store)?;
                let reader = self.reader.as_ref().context("search closed")?;
                self.rank = Some(reader.start_matches(true)?);
                self.count = Some(reader.start_matches(false)?);
                self.collector = RankCollector::new(self.page_size + 1, self.after, self.order);
            }
            self.rank_done = false;
            self.count_done = false;
            self.matched_pages = 0;
            self.matched_visits = 0;
            self.token = None;
        }

        if let Some(regex) = &mut self.regex {
            regex.step(&store, &self.cancel, budget, &self.returned)?;
            self.rank_done = regex.complete;
            self.count_done = regex.complete;
            self.matched_pages = regex.count;
            self.matched_visits = regex.visits;
            self.rank_progress = regex.progress();
            self.count_progress = regex.progress();
        } else {
            let rank_started = std::time::Instant::now();
            while !self.rank_done {
                let step = self
                    .rank
                    .as_mut()
                    .context("search closed")?
                    .step(budget / 2, CANDIDATE_BATCH)?;
                self.resolve(&store, &step.hits)?;
                for mut hit in step.hits {
                    if self.membership.contains(hit.page_id)
                        && !already_returned(&self.returned, hit.page_id)
                    {
                        hit.newest_visit_ms = self.membership.newest(hit.page_id);
                        self.collector.collect(hit);
                    }
                }
                self.rank_progress = step.progress.into();
                self.rank_done = !step.provisional;
                if rank_started.elapsed() >= budget / 2 {
                    break;
                }
            }
            let count_started = std::time::Instant::now();
            while !self.count_done {
                let step = self
                    .count
                    .as_mut()
                    .context("search closed")?
                    .step(budget / 2, CANDIDATE_BATCH)?;
                self.resolve(&store, &step.hits)?;
                for hit in step.hits {
                    if self.membership.contains(hit.page_id) {
                        self.matched_pages += 1;
                        self.matched_visits += self.membership.count(hit.page_id);
                    }
                }
                self.count_progress = step.progress.into();
                self.count_done = !step.provisional;
                if count_started.elapsed() >= budget / 2 {
                    break;
                }
            }
            if self.rank_done && self.count_done && self.matched_pages == 0 {
                if let Some(request) = self.fallback.take() {
                    let indexer = self.indexer.upgrade().context("search owner closed")?;
                    *self = Self::start_owned(&store, &indexer, request, self.cancel.clone())?;
                    return self.step(budget);
                }
            }
        }
        let mut hits =
            self.regex.as_ref().map_or_else(|| self.collector.hits(), |r| r.hits.clone());
        let has_next = self.rank_done && hits.len() > self.page_size;
        hits.truncate(self.page_size);
        if self.rank_done && self.token.is_none() && !hits.is_empty() {
            self.cursor = hits.last().copied();
            let mut bytes = [0u8; 16];
            getrandom::fill(&mut bytes)?;
            self.token = Some(hex::encode(bytes));
        }
        let scope = self.scope.clone();
        let (mut items, canonical_seq) = if let Some(regex) = &self.regex {
            (regex.hydrate(&store)?, store.read(Db::Archive, super::documents::watermark)?)
        } else {
            store.read(Db::Archive, move |s| {
                Ok((eligibility::hydrate(s, &scope, &hits)?, super::documents::watermark(s)?))
            })?
        };
        // A concurrent write invalidates this step's cached membership/count. Hydration
        // and its watermark share one canonical read; don't publish this mixed step.
        let changed = canonical_seq != self.canonical_seq;
        items.truncate(self.page_size);
        if changed {
            items.clear();
        }
        if self.rank_done && !changed {
            self.page_ids = items.iter().map(|item| item.page_id as u64).collect();
        }
        if let Some(scores) = &self.scores {
            for item in &mut items {
                item.reason = Some(scores[&(item.page_id as u64)].1.to_owned());
            }
        }
        let mut indexer = self.indexer.upgrade().context("search owner closed")?.state();
        indexer.target_seq = indexer.target_seq.max(canonical_seq);
        if indexer.phase == super::IndexerPhase::Ready && indexer.applied_seq < canonical_seq {
            indexer.phase = super::IndexerPhase::Updating;
        }
        let snapshot_seq =
            self.reader.as_ref().map_or(canonical_seq, |reader| reader.last_applied_seq());
        let coverage_complete = self.reader.as_ref().is_none_or(|reader| {
            !reader.initial_build_incomplete() && snapshot_seq >= canonical_seq
        });
        Ok(SearchStep {
            page: self.page,
            items,
            provisional: !self.rank_done || changed,
            progress: self.rank_progress.clone(),
            count: SearchCount { value: self.matched_pages, exact: self.count_done && !changed },
            visits: SearchCount { value: self.matched_visits, exact: self.count_done && !changed },
            count_progress: self.count_progress.clone(),
            cursor: if has_next && self.count_done && !changed { self.token.clone() } else { None },
            indexer,
            snapshot_seq,
            coverage_complete,
            search_notes: self.notes.clone(),
        })
    }
    fn resolve(&mut self, store: &Store, hits: &[Hit]) -> Result<()> {
        let ids = hits.iter().map(|h| h.page_id).collect::<Vec<_>>();
        self.membership.ensure(
            store,
            &self.scope,
            &ids,
            &self.cancel,
            &self.ast,
            self.reader.as_ref().map_or(0, |reader| reader.last_applied_seq()),
            self.fuzzy.as_ref().map(|fuzzy| (fuzzy, &self.exact_ast)),
        )?;
        Ok(())
    }
    fn collect_matches(&mut self) -> Result<Vec<Hit>> {
        let store = self.store.upgrade().context("search owner closed")?;
        let mut walker = self
            .reader
            .as_ref()
            .context("regex has no lexical rank stream")?
            .start_matches(true)?;
        let mut hits = Vec::new();
        loop {
            self.cancel.check()?;
            let step = walker.step(Duration::from_millis(8), CANDIDATE_BATCH)?;
            self.resolve(&store, &step.hits)?;
            hits.extend(step.hits.into_iter().filter_map(|mut hit| {
                if !self.membership.contains(hit.page_id) {
                    return None;
                }
                hit.newest_visit_ms = self.membership.newest(hit.page_id);
                Some(hit)
            }));
            if !step.provisional {
                break;
            }
        }
        if hits.is_empty()
            && let Some(request) = self.fallback.take()
        {
            let indexer = self.indexer.upgrade().context("search owner closed")?;
            *self = Self::start_owned(&store, &indexer, request, self.cancel.clone())?;
            return self.collect_matches();
        }
        hits.sort_unstable_by(|a, b| b.cmp(a));
        Ok(hits)
    }
    fn canonical_times(&mut self, ids: &[u64]) -> Result<Vec<i64>> {
        let store = self.store.upgrade().context("search owner closed")?;
        self.ast = vault_search::Ast::And(Vec::new());
        self.fuzzy = None;
        self.membership.retain_verified_membership();
        for ids in ids.chunks(CANDIDATE_BATCH) {
            self.cancel.check()?;
            let hits = ids
                .iter()
                .map(|id| Hit { page_id: *id, score: 0., newest_visit_ms: 0 })
                .collect::<Vec<_>>();
            self.resolve(&store, &hits)?;
        }
        Ok(ids.iter().map(|id| self.membership.newest(*id)).collect())
    }
    fn install_scores(&mut self, scores: Scores) -> Result<()> {
        let scores = Arc::new(scores);
        let filter = scores.clone();
        let scorer = scores.clone();
        let reader = self.reader.as_ref().context("search owner closed")?.requery(
            vault_search::Ast::And(Vec::new()),
            vault_search::Filters {
                eligibility: Some(Arc::new(move |id| filter.contains_key(&id))),
                score: Some(Arc::new(move |id, _| scorer[&id].0)),
                ..Default::default()
            },
        )?;
        self.rank = Some(reader.start_matches(true)?);
        self.count = Some(reader.start_matches(false)?);
        self.reader = Some(reader);
        self.ast = vault_search::Ast::And(Vec::new());
        self.fuzzy = None;
        self.fallback = None;

        self.collector = RankCollector::new(self.page_size + 1, None, SortOrder::Relevance);
        self.rank_done = false;
        self.count_done = false;
        self.matched_pages = 0;
        self.matched_visits = 0;
        self.cursor = None;
        self.token = None;
        self.page = 1;
        self.returned.clear();
        self.page_ids.clear();
        self.scores = Some(scores);
        Ok(())
    }
    pub fn next_page(&mut self, cursor: &str) -> Result<()> {
        if self.token.as_deref() != Some(cursor) {
            bail!("cursor does not belong to the current search page")
        }
        for id in &self.page_ids {
            let word = *id as usize / 64;
            if self.returned.len() <= word {
                self.returned.resize(word + 1, 0);
            }
            self.returned[word] |= 1 << (*id % 64);
        }
        self.page += 1;
        self.after = self.cursor;
        if let Some(regex) = &mut self.regex {
            regex.next_page(self.cursor);
            self.count_done = false;
            self.matched_pages = 0;
            self.matched_visits = 0;
        } else {
            self.rank =
                Some(self.reader.as_ref().context("search session closed")?.start_matches(true)?);
            self.collector = RankCollector::new(self.page_size + 1, self.cursor, self.order);
        }
        self.rank_done = false;
        self.cursor = None;
        self.token = None;
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.cancel.cancel();
        self.reader = None;
        self.rank = None;
        self.count = None;
        self.cursor = None;
        self.regex = None;
    }
}

pub(super) fn already_returned(bits: &[u64], id: u64) -> bool {
    bits.get(id as usize / 64).is_some_and(|word| word & (1 << (id % 64)) != 0)
}

/// Caller-owned handle whose reader and working buffers are revoked on Vault close.
pub struct SearchSession {
    inner: Arc<Mutex<Option<RunningSession>>>,
}
impl SearchSession {
    pub fn start(vault: &crate::Vault, request: SearchRequest) -> Result<Self> {
        Self::start_owned(vault.store(), vault.search(), request, CancelToken::new())
    }
    pub(super) fn start_owned(
        store: &Arc<Store>,
        indexer: &Arc<Indexer>,
        request: SearchRequest,
        cancel: CancelToken,
    ) -> Result<Self> {
        let running = RunningSession::start_owned(store, indexer, request, cancel.clone())
            .inspect_err(|error| {
                if error
                    .downcast_ref::<vault_search::Error>()
                    .is_some_and(|error| matches!(error, vault_search::Error::RebuildNeeded { .. }))
                {
                    indexer.request_repair();
                }
            })?;
        let inner = Arc::new(Mutex::new(Some(running)));
        indexer.track_session(Arc::downgrade(&inner), cancel)?;
        Ok(Self { inner })
    }
    pub fn step(&mut self, budget: Duration) -> Result<SearchStep> {
        let mut slot = self.inner.lock().unwrap();
        let running = slot.as_mut().context("search session closed")?;
        let result = running.step(budget);
        if result.as_ref().err().is_some_and(|error| {
            error
                .downcast_ref::<vault_search::Error>()
                .is_some_and(|error| matches!(error, vault_search::Error::RebuildNeeded { .. }))
        }) {
            if let Some(indexer) = running.indexer.upgrade() {
                indexer.request_repair();
            }
        }
        if slot.as_ref().is_some_and(|running| running.cancel.check().is_err()) {
            slot.take();
            bail!("search session closed or cancelled");
        }
        result
    }
    pub fn finish_page(&mut self) -> Result<SearchStep> {
        loop {
            let step = self.step(Duration::from_millis(8))?;
            if !step.provisional && step.count.exact {
                return Ok(step);
            }
        }
    }
    pub fn next_page(&mut self, cursor: &str) -> Result<()> {
        self.inner.lock().unwrap().as_mut().context("search session closed")?.next_page(cursor)
    }
    pub fn cancel(&mut self) {
        if let Some(mut running) = self.inner.lock().unwrap().take() {
            running.cancel();
        }
    }
    pub(crate) fn collect_matches(&mut self) -> Result<Vec<Hit>> {
        self.inner.lock().unwrap().as_mut().context("search session closed")?.collect_matches()
    }
    pub(crate) fn install_scores(&mut self, scores: Scores) -> Result<()> {
        self.inner.lock().unwrap().as_mut().context("search session closed")?.install_scores(scores)
    }
    pub(crate) fn canonical_times(&mut self, ids: &[u64]) -> Result<Vec<i64>> {
        self.inner.lock().unwrap().as_mut().context("search session closed")?.canonical_times(ids)
    }
    pub(crate) fn set_search_notes(&mut self, notes: Vec<crate::AiSearchNote>) -> Result<()> {
        self.inner.lock().unwrap().as_mut().context("search session closed")?.notes = notes;
        Ok(())
    }
    pub(crate) fn cancel_token(&self) -> Result<CancelToken> {
        Ok(self.inner.lock().unwrap().as_ref().context("search session closed")?.cancel.clone())
    }
    #[cfg(test)]
    pub(super) fn holds_snapshot(&self) -> bool {
        self.inner.lock().unwrap().as_ref().is_some_and(|running| running.reader.is_some())
    }
}
pub(super) struct RetainedSession {
    pub slot: Weak<Mutex<Option<RunningSession>>>,
    pub cancel: CancelToken,
}
