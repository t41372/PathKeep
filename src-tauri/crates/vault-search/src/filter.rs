//! Responsibilities: Choose pushed intersections or fast-field checks by selectivity.
//! Not responsible for: Computing same-visit eligibility from canonical data.
//! Failure modes: Filters contribute to scores; missing domains match; multi-profile pages
//! disappear; pushed/post-filter plans disagree; stale recency controls ties.
//! Tested in contracts and canonical.

use crate::schema::Fields;
use std::sync::Arc;
use tantivy::columnar::Column;
use tantivy::query::{BooleanQuery, ConstScoreQuery, Query, TermQuery};
use tantivy::schema::IndexRecordOption;
use tantivy::{DocId, Searcher, SegmentReader, Term};

/// The predicate must capture immutable canonical eligibility for the session.
/// A profile membership alone cannot prove a date and profile match the same visit.
#[derive(Clone, Default)]
pub struct Filters {
    pub order: SortOrder,
    /// Complete caller-owned scoring inputs (for example global RRF ranks), never a top-N pool.
    pub score: Option<Arc<dyn Fn(u64, f32) -> f32 + Send + Sync>>,
    pub domain: Option<String>,
    pub profile_id: Option<u64>,
    /// Canonical recency for this snapshot, including revokes not yet indexed.
    pub newest_visit: Option<Arc<dyn Fn(u64) -> i64 + Send + Sync>>,
    /// Canonical visit counts in the caller's scope for this snapshot, which `Visits` ranks
    /// by. The index stores no counts, so without these every page counts zero.
    pub visit_count: Option<Arc<dyn Fn(u64) -> u64 + Send + Sync>>,
    pub eligibility: Option<Arc<dyn Fn(u64) -> bool + Send + Sync>>,
}

/// Exposes the selected plan so measurement can distinguish query and collector work.
#[derive(Clone, Copy, Debug, Default)]
pub struct FilterPlan {
    pub domain_pushed: bool,
    pub profile_pushed: bool,
}

pub(crate) fn plan(
    searcher: &Searcher,
    fields: &Fields,
    filters: &Filters,
    lexical: Box<dyn Query>,
) -> tantivy::Result<(Box<dyn Query>, FilterPlan)> {
    let mut queries = vec![lexical];
    let mut plan = FilterPlan::default();
    for (term, pushed) in [
        (
            filters.domain.as_ref().map(|s| Term::from_field_text(fields.domain, s)),
            &mut plan.domain_pushed,
        ),
        (
            filters.profile_id.map(|id| Term::from_field_u64(fields.profiles, id)),
            &mut plan.profile_pushed,
        ),
    ] {
        if let Some(term) = term {
            // Frequency is only a plan estimate: tombstones can overestimate it, never
            // change membership. Broad filters stay in the collector. Narrow filters
            // drive an intersection and never add to the lexical score.
            if searcher.doc_freq(&term)? <= searcher.num_docs() / 2 {
                *pushed = true;
                queries.push(Box::new(ConstScoreQuery::new(
                    Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
                    0.0,
                )));
            }
        }
    }
    Ok((
        if queries.len() == 1 {
            queries.pop().unwrap()
        } else {
            Box::new(BooleanQuery::intersection(queries))
        },
        plan,
    ))
}

pub(crate) struct SegmentFilter {
    domain: Option<(Column<u64>, Option<u64>)>,
    profile: Option<(Column<u64>, u64)>,
}
impl SegmentFilter {
    pub fn open(
        segment: &SegmentReader,
        filters: &Filters,
        plan: FilterPlan,
    ) -> tantivy::Result<Self> {
        let domain = if !plan.domain_pushed {
            if let Some(wanted) = &filters.domain {
                let column =
                    segment.fast_fields().str("domain")?.expect("domain is a schema fast field");
                let ord = column.dictionary().term_ord(wanted.as_bytes())?;
                Some((column.ords().clone(), ord))
            } else {
                None
            }
        } else {
            None
        };
        let profile = if !plan.profile_pushed {
            filters
                .profile_id
                .map(|id| segment.fast_fields().u64("profile_ids").map(|column| (column, id)))
                .transpose()?
        } else {
            None
        };
        Ok(Self { domain, profile })
    }
    pub fn accepts(&self, doc: DocId) -> bool {
        self.domain.as_ref().is_none_or(|(column, wanted)| {
            wanted.is_some_and(|ord| column.values_for_doc(doc).any(|value| value == ord))
        }) && self
            .profile
            .as_ref()
            .is_none_or(|(column, wanted)| column.values_for_doc(doc).any(|value| value == *wanted))
    }
}

/// Explicit caller ordering; relevance never switches to chronology automatically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    Relevance,
    Newest,
    Oldest,
    /// Most visited first: the page's visits in scope, then its newest visit. The walk still
    /// covers every posting; only the key differs, and it needs no BM25 statistics.
    Visits,
}
