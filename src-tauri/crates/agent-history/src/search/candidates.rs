//! A selective mandatory word narrows verification to its posting list. Dense and regex searches
//! walk all selected records; the threshold only changes the query plan.
use super::{expression::Expression, index};
use crate::{Error, Result};
use std::collections::HashMap;
use tantivy::{DocSet, Searcher, TERMINATED, Term, schema::IndexRecordOption};
use vault_store::CancelToken;

#[derive(Default)]
pub(super) struct Selection {
    pub ordinals: Vec<i64>,
    /// For a single word, the positional index already knows the exact scoped frequency.
    pub occurrences: Option<u64>,
}
pub(super) type Selections = HashMap<i64, Selection>;
pub(super) struct RequiredWord {
    terms: Vec<Term>,
    frequency: u64,
    single: bool,
}
impl RequiredWord {
    pub fn choose(
        searcher: &Searcher,
        opened: &index::Opened,
        expression: &Expression,
        roles: &[super::Role],
    ) -> Result<Option<Self>> {
        let mut best: Option<Self> = None;
        for word in expression.terms() {
            let mut terms = Vec::new();
            let mut frequency = 0;
            for role in roles {
                let term = Term::from_field_text(opened.fields.readable[*role as usize], word);
                if !terms.contains(&term) {
                    frequency += index::checked(searcher.doc_freq(&term), &opened.directory)?;
                    terms.push(term);
                }
            }
            if best.as_ref().is_none_or(|b| frequency < b.frequency) {
                best = Some(Self { terms, frequency, single: expression.terms().len() == 1 });
            }
        }
        Ok(best)
    }
    pub fn common(&self) -> bool {
        self.frequency > 100_000
    }
    pub fn selectable(&self) -> bool {
        !self.common() || self.single
    }
    pub fn postings(
        &self,
        searcher: &Searcher,
        opened: &index::Opened,
        cancel: &CancelToken,
    ) -> Result<Selections> {
        use tantivy::postings::Postings;
        let mut result = Selections::new();
        for segment in searcher.segment_readers() {
            let projections =
                segment.fast_fields().u64("projection_id").map_err(|_| Error::IndexBehind)?;
            let ordinals = segment.fast_fields().u64("ordinal").map_err(|_| Error::IndexBehind)?;
            for term in &self.terms {
                let inv = index::checked(segment.inverted_index(term.field()), &opened.directory)?;
                let Some(mut postings) = inv.read_postings(term, IndexRecordOption::WithFreqs)?
                else {
                    continue;
                };
                while postings.doc() != TERMINATED {
                    cancel.check().map_err(|_| Error::Cancelled)?;
                    let doc = postings.doc();
                    if !segment.is_deleted(doc) {
                        let projection = projections.first(doc).ok_or(Error::IndexBehind)? as i64;
                        let ordinal = ordinals.first(doc).ok_or(Error::IndexBehind)? as i64;
                        let selection = result.entry(projection).or_default();
                        if self.single {
                            *selection.occurrences.get_or_insert(0) +=
                                u64::from(postings.term_freq());
                            if let Err(at) = selection.ordinals.binary_search(&ordinal)
                                && at < 3
                            {
                                selection.ordinals.insert(at, ordinal);
                                selection.ordinals.truncate(3);
                            }
                        } else {
                            selection.ordinals.push(ordinal);
                        }
                    }
                    postings.advance();
                }
            }
        }
        if !self.single {
            for sel in result.values_mut() {
                sel.ordinals.sort_unstable();
                sel.ordinals.dedup();
            }
        }
        index::checked(Ok(result), &opened.directory)
    }
}

/// A large title posting list goes through the ordinary ordered traversal.
pub(super) fn titles(
    searcher: &Searcher,
    opened: &index::Opened,
    expr: &Expression,
) -> Result<Option<Vec<String>>> {
    use tantivy::{DocAddress, TantivyDocument, schema::Value};
    let mut best = None;
    for word in expr.terms() {
        let term = Term::from_field_text(opened.fields.title, word);
        let freq = index::checked(searcher.doc_freq(&term), &opened.directory)?;
        if best.as_ref().is_none_or(|(f, _)| freq < *f) {
            best = Some((freq, term));
        }
    }
    let Some((freq, term)) = best else { return Ok(None) };
    if freq > 128 {
        return Ok(None);
    }
    let mut ids = vec![];
    for (i, segment) in searcher.segment_readers().iter().enumerate() {
        let inv = index::checked(segment.inverted_index(opened.fields.title), &opened.directory)?;
        if let Some(mut docs) = inv.read_postings(&term, IndexRecordOption::Basic)? {
            while docs.doc() != TERMINATED {
                if !segment.is_deleted(docs.doc()) {
                    let doc: TantivyDocument = index::checked(
                        searcher.doc(DocAddress::new(i as u32, docs.doc())),
                        &opened.directory,
                    )?;
                    ids.push(
                        doc.get_first(opened.fields.card_id)
                            .and_then(|v| v.as_str())
                            .ok_or(Error::IndexBehind)?
                            .to_owned(),
                    );
                }
                docs.advance();
            }
        }
    }
    index::checked(Ok(Some(ids)), &opened.directory)
}
