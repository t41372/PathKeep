//! Responsibilities: execute literal term queries and fetch bounded canonical excerpts.
//! Not responsible for: index creation, repairs, conversation flattening or UI rendering.
//! Both watermark checks are necessary: publication can change while Tantivy is reading.

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;

use super::{SearchHit, SearchPage, SearchQuery, index, schema, tokenizer};
use crate::{Archive, Error, Result};
use tantivy::collector::{Count, TopDocs};
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::{IndexRecordOption, Value};
use tantivy::{IndexReader, TantivyDocument, Term};
use vault_store::{Db, params};

impl Archive {
    /// Searches the whole synchronized corpus. Terms are literal, case-insensitive
    /// and conjunctive; filters retain exact harness/project identity.
    pub fn search(&self, query: &SearchQuery) -> Result<SearchPage> {
        self.search_page(query, 0)
    }
    pub fn search_page(&self, query: &SearchQuery, offset: usize) -> Result<SearchPage> {
        self.initial_search.wait();
        let _job = self.jobs.enter(None)?;
        loop {
            let state = self.state_visibility()?;
            let page = self.search_current(query, offset)?;
            if self.state_visibility()? == state {
                return Ok(page);
            }
        }
    }

    fn search_current(&self, query: &SearchQuery, offset: usize) -> Result<SearchPage> {
        if query.limit == 0 || query.limit > 100 {
            return Err(Error::InvalidInput("search limit must be between 1 and 100".into()));
        }
        if query.text.len() > 4096 {
            return Err(Error::InvalidInput("search text must be at most 4096 bytes".into()));
        }
        let terms = tokenizer::query_terms(&query.text);
        if terms.is_empty() || terms.len() > 64 {
            return Err(Error::InvalidInput(
                "search requires between 1 and 64 distinct terms".into(),
            ));
        }
        let target = self.projection_generation()?;
        let opened = index::open(self)?;
        if opened.watermark != target {
            return Err(Error::IndexBehind);
        }
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = terms
            .iter()
            .map(|term| {
                let query = TermQuery::new(
                    Term::from_field_text(opened.fields.text, term),
                    IndexRecordOption::WithFreqs,
                );
                (Occur::Must, Box::new(query) as Box<dyn Query>)
            })
            .collect();
        if let Some(harness) = query.harness {
            clauses.push(exact(opened.fields.harness, &harness.to_string()));
        }
        if let Some(project) = &query.project_path {
            clauses.push(exact(opened.fields.project, project));
        }
        let hidden_query = crate::query::parents::hidden_query();
        let hidden = self.store.read(Db::AgentHistoryReadModel, move |tx| {
            tx.query(
                &format!("SELECT id FROM ah_sessions WHERE conversation_id IN ({hidden_query})"),
                (),
                |row| Ok(row.get::<String>(0)?),
            )
        })?;
        if !hidden.is_empty() {
            let terms = hidden.into_iter().map(|id| {
                Term::from_field_text(opened.fields.session_id, &schema::exact_term(&id))
            });
            clauses.push((Occur::MustNot, Box::new(tantivy::query::TermSetQuery::new(terms))));
        }
        #[cfg(test)]
        state_tests::after_filter();
        let reader: IndexReader = index::checked(
            opened.index.reader_builder().reload_policy(tantivy::ReloadPolicy::Manual).try_into(),
            &opened.directory,
        )?;
        let searcher = reader.searcher();
        let expression = BooleanQuery::new(clauses);
        let (top, total) = if offset as u64 >= searcher.num_docs() {
            (Vec::new(), index::checked(searcher.search(&expression, &Count), &opened.directory)?)
        } else {
            index::checked(
                searcher.search(
                    &expression,
                    &(TopDocs::with_limit(query.limit).and_offset(offset).order_by_score(), Count),
                ),
                &opened.directory,
            )?
        };
        let mut items = Vec::with_capacity(top.len());
        for (score, address) in top {
            let document: TantivyDocument =
                index::checked(searcher.doc(address), &opened.directory)?;
            let event = document
                .get_first(opened.fields.event_id)
                .and_then(|value| value.as_u64())
                .and_then(|value| i64::try_from(value).ok())
                .ok_or(Error::IndexBehind)?;
            let projection = document
                .get_first(opened.fields.projection_id)
                .and_then(|value| value.as_u64())
                .and_then(|value| i64::try_from(value).ok())
                .ok_or(Error::IndexBehind)?;
            items.push(self.search_hit(event, projection, score, &query.text)?);
        }
        let complete = self.search_coverage()?;
        if self.projection_generation()? != target {
            return Err(Error::IndexBehind);
        }
        Ok(SearchPage { items, next_cursor: None, total: total as u64, complete })
    }

    fn search_hit(
        &self,
        event: i64,
        projection: i64,
        score: f32,
        needle: &str,
    ) -> Result<SearchHit> {
        let needle = needle.trim().to_owned();
        let rows = self.store.read(Db::AgentHistoryReadModel, move |database| {
            database.query(
                "SELECT s.conversation_id, e.ordinal, substr(e.text, CASE WHEN
                    instr(lower(e.text), lower(?2))>0 THEN max(1, instr(lower(e.text),
                    lower(?2))-120) ELSE 1 END, 480), coalesce(u.put_away, 0) FROM ah_events e
                    JOIN ah_sessions s ON s.id=e.session_id AND s.projection_id=e.projection_id
                    LEFT JOIN ah_user_state u ON u.conversation_id=s.conversation_id WHERE e.id=?1
                    AND e.projection_id=?3 LIMIT 1",
                params![event, needle, projection],
                |row| {
                    Ok(SearchHit {
                        session_id: row.get(0)?,
                        event_id: event,
                        ordinal: row.get::<i64>(1)? as u64,
                        text: row.get(2)?,
                        score,
                        put_away: row.get::<i64>(3)? != 0,
                    })
                },
            )
        })?;
        rows.into_iter().next().ok_or(Error::IndexBehind)
    }

    fn search_coverage(&self) -> Result<bool> {
        self.normalization_complete()
    }
}

fn exact(field: tantivy::schema::Field, value: &str) -> (Occur, Box<dyn Query>) {
    (
        Occur::Must,
        Box::new(TermQuery::new(
            Term::from_field_text(field, &schema::exact_term(value)),
            IndexRecordOption::Basic,
        )),
    )
}
