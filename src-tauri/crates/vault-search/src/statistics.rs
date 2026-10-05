//! Responsibilities: gather exact snapshot BM25 statistics one field/segment at a time.
//! Not responsible for: scoring, matching, caching result sets, or live-only statistics.
//! Failure modes: handle creation performs cold I/O before any progress; partial totals
//! produce segment-local scores; deleted documents change Tantivy's statistics contract;
//! an I/O failure partially advances totals. Contracts compare all scores to independent
//! Tantivy scoring and require metadata work to yield before document collection.

use std::collections::BTreeMap;
use tantivy::query::{Bm25StatisticsProvider, Query};
use tantivy::schema::Field;
use tantivy::{Searcher, Term};

struct FieldStatistics {
    field: Field,
    tokens: u64,
    frequencies: BTreeMap<Term, u64>,
}
pub(crate) struct Statistics {
    fields: Vec<FieldStatistics>,
    documents: u64,
    pub processed: usize,
    pub total: usize,
}
impl Statistics {
    pub fn new(query: &dyn Query, searcher: &Searcher, scoring: bool) -> Self {
        let mut fields: BTreeMap<Field, BTreeMap<Term, u64>> = BTreeMap::new();
        if scoring {
            query.query_terms(&mut |term, _| {
                fields.entry(term.field()).or_default().insert(term.clone(), 0);
            });
        }
        let fields: Vec<_> = fields
            .into_iter()
            .map(|(field, frequencies)| FieldStatistics { field, frequencies, tokens: 0 })
            .collect();
        let total = fields.len() * searcher.segment_readers().len();
        // Match Tantivy: statistics include documents awaiting deletion merges.
        let documents = searcher.segment_readers().iter().map(|s| u64::from(s.max_doc())).sum();
        Self { fields, documents, processed: 0, total }
    }
    pub fn complete(&self) -> bool {
        self.processed == self.total
    }
    pub fn advance(&mut self, searcher: &Searcher) -> tantivy::Result<()> {
        let segment = &searcher.segment_readers()[self.processed / self.fields.len()];
        let field_index = self.processed % self.fields.len();
        let field = &mut self.fields[field_index];
        let inverted = segment.inverted_index(field.field)?;
        let frequencies: Vec<_> = field
            .frequencies
            .keys()
            .map(|term| inverted.doc_freq(term))
            .collect::<std::io::Result<_>>()?;
        // Publish these totals only after every read succeeded, so retrying a failed
        // step cannot count a segment twice. Scratch is proportional to the query.
        field.tokens += inverted.total_num_tokens();
        for (total, frequency) in field.frequencies.values_mut().zip(frequencies) {
            *total += u64::from(frequency);
        }
        self.processed += 1;
        Ok(())
    }
}
impl Bm25StatisticsProvider for Statistics {
    fn total_num_tokens(&self, field: Field) -> tantivy::Result<u64> {
        Ok(self
            .fields
            .iter()
            .find(|entry| entry.field == field)
            .expect("query exposed its scoring fields")
            .tokens)
    }
    fn total_num_docs(&self) -> tantivy::Result<u64> {
        Ok(self.documents)
    }
    fn doc_freq(&self, term: &Term) -> tantivy::Result<u64> {
        Ok(*self
            .fields
            .iter()
            .find(|entry| entry.field == term.field())
            .and_then(|entry| entry.frequencies.get(term))
            .expect("query exposed its scoring terms"))
    }
}
