//! Responsibilities: stream normalized canonical documents into index batches.
//! Not responsible for: committing the index or deciding query eligibility.
//! Failure modes: raw punctuation/accents fail token matching, invisible pages survive,
//! profiles duplicate documents, an interrupted stream appears complete.
//! Tests: search::tests and archive::search_documents::tests.

use crate::archive::search_lexical::index_tokens;
use vault_search::{Batch, PageDocument};
use vault_store::{Result, Session};

pub(super) const COLUMNS: &str = "d.page_id,d.url,d.title,d.search_terms,d.normalized_url,d.normalized_title,d.normalized_search_terms,d.cjk_grams,d.enrichment_text,d.notes_text,d.tags_text,d.compact_text,d.newest_visit_ms,d.domain";

pub(super) fn watermark(s: &Session<'_>) -> Result<u64> {
    s.query_one(
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='search_outbox'),0)",
        (),
        |r| Ok(r.get::<i64>(0)? as u64),
    )
}

pub(super) fn document(row: &vault_store::Row) -> Result<PageDocument> {
    let mut doc = PageDocument::new(row.get::<i64>(0)? as u64);
    for (index, text) in doc.text.iter_mut().enumerate() {
        let source = row.get::<String>(index + 1)?;
        *text = if index == vault_search::TextField::CjkGrams as usize {
            source
        } else {
            index_tokens(&source)
        };
    }
    doc.compact_text = row.get(11)?;
    doc.newest_visit_ms = row.get(12)?;
    doc.domain = row.get::<Option<String>>(13)?.unwrap_or_default();
    Ok(doc)
}

pub(super) fn batch(s: &Session<'_>, after: u64) -> Result<Option<Batch>> {
    let events = s.query(
        "SELECT seq,page_id FROM search_outbox WHERE seq>?1 ORDER BY seq LIMIT 10000",
        [after as i64],
        |r| Ok((r.get::<i64>(0)? as u64, r.get::<i64>(1)?)),
    )?;
    let Some(&(last_seq, _)) = events.last() else { return Ok(None) };
    let mut ids = events.into_iter().map(|(_, id)| id).collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT {COLUMNS} FROM search_documents d WHERE d.page_id IN ({placeholders}) AND d.newest_visit_ms IS NOT NULL ORDER BY d.page_id"
    );
    let mut upserts = s.query(&sql, ids.clone(), document)?;
    let members=s.query(&format!("SELECT page_id,source_profile_id FROM search_document_profiles WHERE page_id IN ({placeholders}) ORDER BY page_id,source_profile_id"),ids.clone(),|r|Ok((r.get::<i64>(0)? as u64,r.get::<i64>(1)? as u64)))?;
    let mut members = members.into_iter().peekable();
    for doc in &mut upserts {
        while members.peek().is_some_and(|(id, _)| *id < doc.page_id) {
            members.next();
        }
        while members.peek().is_some_and(|(id, _)| *id == doc.page_id) {
            doc.profile_ids.push(members.next().unwrap().1);
        }
    }
    Ok(Some(Batch { upserts, deletes: ids.into_iter().map(|id| id as u64).collect(), last_seq }))
}

/// One bounded canonical read. Page bounds include invisible anchors so a long revoked
/// range cannot turn one read request into a whole-archive scan.
pub(super) fn range(s: &Session<'_>, after: i64, through: i64) -> Result<(Vec<PageDocument>, i64)> {
    let ids=s.query("SELECT page_id FROM search_documents WHERE page_id>?1 AND page_id<=?2 ORDER BY page_id LIMIT 1024",vault_store::params![after,through],|r|Ok(r.get::<i64>(0)?))?;
    let Some(last) = ids.last().copied() else { return Ok((Vec::new(), through)) };
    let mut documents=s.query(&format!("SELECT {COLUMNS} FROM search_documents d WHERE d.page_id>?1 AND d.page_id<=?2 AND d.newest_visit_ms IS NOT NULL ORDER BY d.page_id"),vault_store::params![after,last],document)?;
    let profiles=s.query("SELECT page_id,source_profile_id FROM search_document_profiles WHERE page_id>?1 AND page_id<=?2 ORDER BY page_id,source_profile_id",vault_store::params![after,last],|r|Ok((r.get::<i64>(0)? as u64,r.get::<i64>(1)? as u64)))?;
    let mut profiles = profiles.into_iter().peekable();
    for document in &mut documents {
        while profiles.peek().is_some_and(|(id, _)| *id < document.page_id) {
            profiles.next();
        }
        while profiles.peek().is_some_and(|(id, _)| *id == document.page_id) {
            document.profile_ids.push(profiles.next().unwrap().1);
        }
    }
    Ok((documents, last))
}

/// Source iteration closes each Store read before feeding its rows to Tantivy. Changes
/// after the starting watermark are replayed before the owner publishes the generation.
pub(super) struct Source {
    store: std::sync::Arc<vault_store::Store>,
    cancel: vault_store::CancelToken,
    stop: vault_store::CancelToken,
    after: i64,
    through: i64,
    rows: std::vec::IntoIter<PageDocument>,
    failed: bool,
}
impl Source {
    pub fn new(
        store: std::sync::Arc<vault_store::Store>,
        cancel: vault_store::CancelToken,
        stop: vault_store::CancelToken,
        through: i64,
    ) -> Self {
        Self { store, cancel, stop, after: 0, through, rows: Vec::new().into_iter(), failed: false }
    }
}
impl Iterator for Source {
    type Item = vault_search::Result<PageDocument>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        loop {
            if self.cancel.check().is_err() || self.stop.check().is_err() {
                self.failed = true;
                return Some(Err(vault_search::Error::Cancelled));
            }
            if let Some(row) = self.rows.next() {
                return Some(Ok(row));
            }
            if self.after >= self.through {
                return None;
            }
            let (after, through) = (self.after, self.through);
            match self.store.read(vault_store::Db::Archive, move |s| range(s, after, through)) {
                Ok((rows, last)) => {
                    self.after = last;
                    self.rows = rows.into_iter();
                }
                Err(error) => {
                    self.failed = true;
                    return Some(Err(vault_search::Error::Input(error.to_string())));
                }
            }
        }
    }
}
