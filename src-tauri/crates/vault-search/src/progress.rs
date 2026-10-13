//! Responsibilities: Own a snapshot and resumable scoring/count traversal.
//! Not responsible for: Scheduling threads or presenting provisional results.
//! Failure modes: Steps skip/de-duplicate incorrectly; segment-local scores disagree;
//! deletes leak; counts use different filters; a cursor is reused in another session;
//! ongoing writes change pagination. Tested in contracts.

use crate::{
    Error, FilterPlan, Filters, Hit, Result, collector::RankCollector, error::checked,
    filter::SegmentFilter, statistics::Statistics,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tantivy::columnar::Column;
use tantivy::query::{EnableScoring, Query, Scorer, Weight};
use tantivy::{Searcher, TERMINATED};
use vault_crypto::EncryptedDirectory;

pub(crate) struct Snapshot {
    pub searcher: Searcher,
    pub _lease: Arc<()>,
    pub last_seq: u64,
    pub initial_build: bool,
    pub query: Box<dyn Query>,
    pub filters: Filters,
    pub plan: FilterPlan,
    pub directory: Option<EncryptedDirectory>,
}
/// Owns the index snapshot, query and immutable eligibility used by every page and count.
/// Drop sessions when finished (and on vault lock) to release old segments and plaintext.
#[derive(Clone)]
pub struct SearchSession {
    pub(crate) snapshot: Arc<Snapshot>,
}

/// A continuation belongs to one session. It is deliberately not serializable or
/// constructible: using it with another query, filters or generation returns an error.
#[derive(Clone)]
pub struct Cursor {
    snapshot: Arc<Snapshot>,
    after: Hit,
}
impl std::fmt::Debug for Cursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cursor").field("after", &self.after).finish_non_exhaustive()
    }
}

/// Documents processed are visited posting candidates, including rejected/tombstoned
/// pages. Segment completion is exact; it is not a guessed percentage or match count.
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    pub documents_processed: u64,
    pub segments_completed: usize,
    pub segments_total: usize,
    /// Exact completed field/segment metadata reads, before global BM25 is available.
    pub statistics_fields_processed: usize,
    pub statistics_fields_total: usize,
}
/// Only final steps have a continuation cursor. Provisional hits may change order.
#[derive(Clone, Debug)]
pub struct PageStep {
    pub hits: Vec<Hit>,
    pub next: Option<Cursor>,
    pub provisional: bool,
    pub progress: Progress,
}
/// Counts are monotone lower bounds until provisional becomes false.
#[derive(Clone, Copy, Debug)]
pub struct CountStep {
    pub count: u64,
    pub provisional: bool,
    pub progress: Progress,
}

impl SearchSession {
    /// Watermark of this reader, not a later writer commit.
    pub fn last_applied_seq(&self) -> u64 {
        self.snapshot.last_seq
    }
    /// Partial first-build readers never become complete just because indexing advanced.
    pub fn initial_build_incomplete(&self) -> bool {
        self.snapshot.initial_build
    }

    /// Changes the expression while retaining the same reader generation. Optional AI
    /// can enumerate lexical ranks, then score their full union on this exact snapshot.
    pub fn requery(&self, ast: crate::Ast, filters: Filters) -> Result<Self> {
        let (_, fields) = crate::schema::schema();
        let query = ast.compile(&fields)?;
        let (query, plan) = checked(
            crate::filter::plan(&self.snapshot.searcher, &fields, &filters, query),
            self.snapshot.directory.as_ref(),
        )?;
        Ok(Self {
            snapshot: Arc::new(Snapshot {
                searcher: self.snapshot.searcher.clone(),
                _lease: self.snapshot._lease.clone(),
                last_seq: self.snapshot.last_seq,
                initial_build: self.snapshot.initial_build,
                query,
                filters,
                plan,
                directory: self.snapshot.directory.clone(),
            }),
        })
    }
    /// Candidate batches let the owner join canonical eligibility before collecting top-k.
    /// The batch size bounds memory, never the total number of reachable matches.
    pub fn start_matches(&self, scoring: bool) -> Result<MatchHandle> {
        Ok(MatchHandle { walker: Walker::new(self.snapshot.clone(), scoring)? })
    }
    /// Makes the selected execution plan available for profiling.
    pub fn plan(&self) -> FilterPlan {
        self.snapshot.plan
    }
    /// Completes a page without a time slice; memory is O(limit), independent of depth.
    pub fn page(&self, limit: usize, after: Option<&Cursor>) -> Result<PageStep> {
        self.start_page(limit, after)?.step(Duration::MAX)
    }
    /// Counts independently, without allocating a ranking heap or calculating scores.
    pub fn count(&self) -> Result<u64> {
        Ok(self.start_count()?.step(Duration::MAX)?.count)
    }
    /// Starts a page that a blocking worker can advance between UI progress updates.
    pub fn start_page(&self, limit: usize, after: Option<&Cursor>) -> Result<PageHandle> {
        if limit == 0 {
            return Err(Error::Input("page size must be positive".into()));
        }
        if after.is_some_and(|c| !Arc::ptr_eq(&c.snapshot, &self.snapshot)) {
            return Err(Error::Input("cursor belongs to a different search session".into()));
        }
        Ok(PageHandle {
            walker: Walker::new(self.snapshot.clone(), true)?,
            collector: RankCollector::new(
                limit,
                after.map(|c| c.after),
                self.snapshot.filters.order,
            ),
        })
    }
    /// Starts independent counting so ranking never waits for a total.
    pub fn start_count(&self) -> Result<CountHandle> {
        Ok(CountHandle { walker: Walker::new(self.snapshot.clone(), false)?, count: 0 })
    }
}
/// A bounded, resumable posting walk with the same scores as a ranked page.
pub struct MatchHandle {
    walker: Walker,
}
pub struct MatchStep {
    pub hits: Vec<Hit>,
    pub provisional: bool,
    pub progress: Progress,
}
impl MatchHandle {
    pub fn step(&mut self, budget: Duration, batch_size: usize) -> Result<MatchStep> {
        if batch_size == 0 {
            return Err(Error::Input("match batch size must be positive".into()));
        }
        let mut hits = Vec::with_capacity(batch_size.min(1024));
        self.walker.walk(budget, batch_size, |hit| hits.push(hit))?;
        Ok(MatchStep { hits, provisional: !self.walker.done(), progress: self.walker.progress() })
    }
}
/// Resumable global top-k; no index-sized match list and no increasing offset heap.
pub struct PageHandle {
    walker: Walker,
    collector: RankCollector,
}
impl PageHandle {
    /// Cooperative budget checked every 256 candidates and at segment boundaries.
    /// Segment/scorer initialization and a caller predicate cannot be preempted.
    pub fn step(&mut self, budget: Duration) -> Result<PageStep> {
        self.walker.step(budget, |hit| self.collector.collect(hit))?;
        let hits = self.collector.hits();
        let provisional = !self.walker.done();
        let next = if provisional {
            None
        } else {
            hits.last().map(|&after| Cursor { snapshot: self.walker.snapshot.clone(), after })
        };
        Ok(PageStep { hits, next, provisional, progress: self.walker.progress() })
    }
}
/// Resumable exact counting under the same snapshot and filters as ranking.
pub struct CountHandle {
    walker: Walker,
    count: u64,
}
impl CountHandle {
    /// Advances a monotone count under the same cooperative budget as ranking.
    pub fn step(&mut self, budget: Duration) -> Result<CountStep> {
        self.step_with(budget, |_| {})
    }
    /// Visit each newly counted page once, so callers can aggregate canonical measures
    /// without a second match walk or retaining a complete result set.
    pub fn step_with(&mut self, budget: Duration, mut visit: impl FnMut(u64)) -> Result<CountStep> {
        self.walker.step(budget, |hit| {
            self.count += 1;
            visit(hit.page_id);
        })?;
        Ok(CountStep {
            count: self.count,
            provisional: !self.walker.done(),
            progress: self.walker.progress(),
        })
    }
}

struct ActiveSegment {
    scorer: Box<dyn Scorer>,
    ids: Column<u64>,
    newest: Option<Column<i64>>,
    filter: SegmentFilter,
}
struct Walker {
    snapshot: Arc<Snapshot>,
    weight: Option<Box<dyn Weight>>,
    statistics: Statistics,
    scoring: bool,
    segment: usize,
    active: Option<ActiveSegment>,
    processed: u64,
}
impl Walker {
    fn new(snapshot: Arc<Snapshot>, scoring: bool) -> Result<Self> {
        let statistics = Statistics::new(
            &*snapshot.query,
            &snapshot.searcher,
            scoring && snapshot.filters.order == crate::SortOrder::Relevance,
        );
        Ok(Self {
            snapshot,
            weight: None,
            statistics,
            scoring,
            segment: 0,
            active: None,
            processed: 0,
        })
    }
    fn done(&self) -> bool {
        self.segment == self.snapshot.searcher.segment_readers().len()
    }
    fn progress(&self) -> Progress {
        Progress {
            documents_processed: self.processed,
            segments_completed: self.segment,
            segments_total: self.snapshot.searcher.segment_readers().len(),
            statistics_fields_processed: self.statistics.processed,
            statistics_fields_total: self.statistics.total,
        }
    }
    fn step(&mut self, budget: Duration, collect: impl FnMut(Hit)) -> Result<()> {
        self.walk(budget, usize::MAX, collect)
    }
    fn walk(
        &mut self,
        budget: Duration,
        batch_size: usize,
        mut collect: impl FnMut(Hit),
    ) -> Result<()> {
        let mut collected = 0usize;
        let started = Instant::now();
        let mut chunk = 0;
        while !self.statistics.complete() {
            checked(
                self.statistics.advance(&self.snapshot.searcher),
                self.snapshot.directory.as_ref(),
            )?;
            if started.elapsed() >= budget {
                return Ok(());
            }
        }
        if self.weight.is_none() {
            let enabled =
                if self.scoring && self.snapshot.filters.order == crate::SortOrder::Relevance {
                    EnableScoring::enabled_from_statistics_provider(
                        &self.statistics,
                        &self.snapshot.searcher,
                    )
                } else {
                    EnableScoring::disabled_from_searcher(&self.snapshot.searcher)
                };
            self.weight = Some(checked(
                self.snapshot.query.weight(enabled),
                self.snapshot.directory.as_ref(),
            )?);
        }
        while !self.done() {
            let segment = &self.snapshot.searcher.segment_readers()[self.segment];
            if self.active.is_none() {
                let active = (|| -> tantivy::Result<_> {
                    Ok(ActiveSegment {
                        scorer: self
                            .weight
                            .as_ref()
                            .expect("statistics prepared the weight")
                            .scorer(segment, 1.0)?,
                        ids: segment.fast_fields().u64("page_id")?,
                        newest: if self.scoring {
                            Some(segment.fast_fields().i64("newest_visit_ms")?)
                        } else {
                            None
                        },
                        filter: SegmentFilter::open(
                            segment,
                            &self.snapshot.filters,
                            self.snapshot.plan,
                        )?,
                    })
                })();
                self.active = Some(checked(active, self.snapshot.directory.as_ref())?);
            }
            let active = self.active.as_mut().unwrap();
            while active.scorer.doc() != TERMINATED {
                let doc = active.scorer.doc();
                self.processed += 1;
                chunk += 1;
                if segment.alive_bitset().is_none_or(|alive| alive.is_alive(doc))
                    && active.filter.accepts(doc)
                {
                    let page_id =
                        active.ids.first(doc).expect("page_id is required by the schema writer");
                    if self
                        .snapshot
                        .filters
                        .eligibility
                        .as_ref()
                        .is_none_or(|accept| accept(page_id))
                    {
                        collected += 1;
                        collect(Hit {
                            page_id,
                            visit_ms: self.snapshot.filters.newest_visit.as_ref().map_or_else(
                                || active.newest.as_ref().and_then(|c| c.first(doc)).unwrap_or(0),
                                |newest| newest(page_id),
                            ),
                            score: if self.scoring
                                && self.snapshot.filters.order == crate::SortOrder::Relevance
                            {
                                let score = active.scorer.score();
                                self.snapshot
                                    .filters
                                    .score
                                    .as_ref()
                                    .map_or(score, |adjust| adjust(page_id, score))
                            } else {
                                0.0
                            },
                        });
                    }
                }
                active.scorer.advance();
                if collected == batch_size {
                    return Ok(());
                }
                if chunk == 256 {
                    chunk = 0;
                    if started.elapsed() >= budget {
                        return Ok(());
                    }
                }
            }
            self.active = None;
            self.segment += 1;
            if started.elapsed() >= budget {
                return Ok(());
            }
        }
        Ok(())
    }
}
