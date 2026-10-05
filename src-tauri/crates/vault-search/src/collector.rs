//! Responsibilities: Keep an exact bounded top-k with recency and page-id ties.
//! Not responsible for: Loading canonical rows or caching complete result sets.
//! Failure modes: Equal scores are pruned; cursor equality repeats a page; reverse heap
//! ordering loses winners; memory grows with depth. Tested against brute-force ranking and
//! complete pagination in contracts.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// A page's complete ranking key. Scores stay as f32 bits inside in-process cursors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub page_id: u64,
    pub newest_visit_ms: i64,
    pub score: f32,
}
impl Eq for Hit {}
impl Ord for Hit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .total_cmp(&other.score)
            .then(self.newest_visit_ms.cmp(&other.newest_visit_ms))
            .then(self.page_id.cmp(&other.page_id))
    }
}
impl PartialOrd for Hit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(crate) struct RankCollector {
    heap: BinaryHeap<Reverse<Hit>>,
    limit: usize,
    after: Option<Hit>,
}
impl RankCollector {
    pub fn new(limit: usize, after: Option<Hit>) -> Self {
        Self { heap: BinaryHeap::new(), limit, after }
    }
    pub fn collect(&mut self, hit: Hit) {
        if self.after.is_some_and(|key| hit >= key) {
            return;
        }
        if self.heap.len() < self.limit {
            self.heap.push(Reverse(hit));
        } else if self.heap.peek().is_some_and(|worst| hit > worst.0) {
            *self.heap.peek_mut().unwrap() = Reverse(hit);
        }
    }
    pub fn hits(&self) -> Vec<Hit> {
        let mut hits: Vec<_> = self.heap.iter().map(|hit| hit.0).collect();
        hits.sort_unstable_by(|a, b| b.cmp(a));
        hits
    }
}
