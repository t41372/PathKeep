//! Responsibilities: Choose pushed intersections or fast-field checks by selectivity.
//! Not responsible for: Computing same-visit eligibility from canonical data.
//! Failure modes: Filters contribute to scores; missing domains match; multi-profile pages
//! disappear; pushed/post-filter plans disagree. Tested in contracts.

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
    pub domain: Option<String>,
    pub profile_id: Option<u64>,
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
