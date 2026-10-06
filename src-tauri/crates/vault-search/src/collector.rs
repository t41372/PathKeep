//! Responsibilities: Keep an exact bounded top-k with recency and page-id ties.
//! Not responsible for: Loading canonical rows or caching complete result sets.
//! Failure modes: Equal scores are pruned; cursor equality repeats a page; reverse heap
//! ordering loses winners; memory grows with depth. Tested against brute-force ranking and
//! complete pagination in contracts.

use crate::SortOrder;
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
        hits.sort_unstable_by(|a, b| compare(b, a, self.order));
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
        compare(&self.hit, &other.hit, self.order)
    }
}
fn compare(a: &Hit, b: &Hit, order: SortOrder) -> Ordering {
    match order {
        SortOrder::Relevance => a.cmp(b),
        SortOrder::Newest => {
            a.newest_visit_ms.cmp(&b.newest_visit_ms).then(a.page_id.cmp(&b.page_id))
        }
        SortOrder::Oldest => {
            b.newest_visit_ms.cmp(&a.newest_visit_ms).then(a.page_id.cmp(&b.page_id))
        }
    }
}
