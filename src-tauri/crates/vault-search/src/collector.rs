//! Responsibilities: Keep an exact bounded top-k under each order's complete key, and define
//! that key once for every caller that ranks hits.
//! Not responsible for: Loading canonical rows, counting visits or caching complete result sets.
//! Failure modes: Equal scores, visit counts or times are pruned; cursor equality repeats a
//! page; reverse heap ordering loses winners; memory grows with depth; a caller ranking outside
//! the collector orders pages differently from a cursor. Tested against brute-force ranking and
//! complete pagination in contracts.

use crate::SortOrder;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// A page's complete ranking key. Scores stay as f32 bits inside in-process cursors.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub page_id: u64,
    /// The visit the order goes by: the page's newest, or its first when the order is
    /// `Oldest` ("when did I first…" asks for the first visit, not the latest one).
    pub visit_ms: i64,
    pub score: f32,
    /// The page's visits in the caller's scope, which `Visits` ranks by. The index holds no
    /// visit counts: this is zero unless the caller supplies them (`Filters::visit_count`) or
    /// sets them from its own canonical counts.
    pub visits: u64,
}
/// Equal exactly when [`Ord`] says so: the score is a float, and a derived equality would let
/// `NaN` and `-0.0` disagree with the order heaps and cursors rely on.
impl PartialEq for Hit {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Hit {}
impl Ord for Hit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then(self.visit_ms.cmp(&other.visit_ms))
            .then(self.page_id.cmp(&other.page_id))
            // Page IDs are unique, so this never reorders pages; two hits are equal only when
            // every field is.
            .then(self.visits.cmp(&other.visits))
    }
}
impl PartialOrd for Hit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl SortOrder {
    /// `Greater` when `a` is listed before `b`. Every order breaks its last tie by page ID, so
    /// two pages never compare equal and a cursor names one position. Callers that rank outside
    /// the collector (the archive's regex scan) use this too, so their cursors agree with it.
    pub fn compare(self, a: &Hit, b: &Hit) -> Ordering {
        match self {
            SortOrder::Relevance => a.cmp(b),
            SortOrder::Newest => a.visit_ms.cmp(&b.visit_ms).then(a.page_id.cmp(&b.page_id)),
            SortOrder::Oldest => b.visit_ms.cmp(&a.visit_ms).then(a.page_id.cmp(&b.page_id)),
            SortOrder::Visits => a
                .visits
                .cmp(&b.visits)
                .then(a.visit_ms.cmp(&b.visit_ms))
                .then(a.page_id.cmp(&b.page_id)),
        }
    }
}

pub struct RankCollector {
    heap: BinaryHeap<Reverse<Ranked>>,
    order: SortOrder,
    limit: usize,
    after: Option<Hit>,
}
impl RankCollector {
    pub fn new(limit: usize, after: Option<Hit>, order: SortOrder) -> Self {
        Self { heap: BinaryHeap::new(), limit, after, order }
    }
    pub fn collect(&mut self, hit: Hit) {
        let hit = Ranked { hit, order: self.order };
        if self.after.is_some_and(|key| hit >= Ranked { hit: key, order: self.order }) {
            return;
        }
        if self.heap.len() < self.limit {
            self.heap.push(Reverse(hit));
        } else if self.heap.peek().is_some_and(|worst| hit > worst.0) {
            *self.heap.peek_mut().unwrap() = Reverse(hit);
        }
    }
    pub fn hits(&self) -> Vec<Hit> {
        let mut hits: Vec<_> = self.heap.iter().map(|hit| hit.0.hit).collect();
        hits.sort_unstable_by(|a, b| self.order.compare(b, a));
        hits
    }
}

#[derive(Clone, Copy, Debug)]
struct Ranked {
    hit: Hit,
    order: SortOrder,
}
impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order.compare(&self.hit, &other.hit)
    }
}
