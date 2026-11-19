//! Responsibilities: replay canonical publication into bounded index batches.
//! Not responsible for: publishing source revisions, background scheduling or user queries.
//! A commit acknowledges only a checked canonical generation; interruption leaves the
//! prior commit or an explicitly incomplete new index, never a false current watermark.

use super::{SearchSyncReport, index, schema};
use crate::{Archive, Error, Result};
use tantivy::{IndexWriter, TantivyDocument, Term};
use vault_crypto::EncryptedDirectory;
use vault_store::{Db, params};

const WRITER_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const PAGE_BYTES: usize = 4 * 1024 * 1024;
const PAGE_RECORDS: i64 = 128;
const QUEUED_OPERATIONS: usize = 4096;

impl Archive {
    /// Explicitly brings searchable events to the current canonical generation.
    /// Missing or incompatible metadata starts a rebuild. Use `rebuild_search`
    /// to repair segment corruption whose commit metadata still appears current.
    pub fn synchronize_search(&self) -> Result<SearchSyncReport> {
        self.synchronize_search_with_cancel(&|| false)
    }

    /// Recreates the derived index explicitly, including damaged segment files
    /// that cannot be detected by a cheap commit-watermark check.
    pub fn rebuild_search(&self) -> Result<SearchSyncReport> {
        self.synchronize_search_inner(true, &|| false)
    }

    /// Lets a background owner cancel between bounded SQL/index batches. Partial
    /// drain commits retain the old watermark until the entire replay completes.
    pub fn synchronize_search_with_cancel(
        &self,
        cancel: &dyn Fn() -> bool,
    ) -> Result<SearchSyncReport> {
        self.synchronize_search_inner(false, cancel)
    }

    fn synchronize_search_inner(
        &self,
        force_rebuild: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<SearchSyncReport> {
        let _job = self.jobs.enter(None)?;
        let cancel = &|| cancel() || self.jobs.stopping();
        let _guard = self
            .search_gate
            .lock()
            .map_err(|_| Error::Integrity("search synchronization owner was interrupted".into()))?;
        if cancel() {
            return Err(Error::Cancelled);
        }
        let target = self.projection_generation()?;
        let (opened, rebuilt) = if force_rebuild {
            (index::recreate(self)?, true)
        } else {
            match index::open(self) {
                Ok(opened) if opened.watermark <= target => (opened, false),
                Ok(_) | Err(Error::IndexBehind) => (index::recreate(self)?, true),
                Err(error) => return Err(error),
            }
        };
        if !rebuilt && opened.watermark == target {
            return Ok(SearchSyncReport { indexed_events: 0, watermark: target, rebuilt });
        }
        let mut writer: IndexWriter<TantivyDocument> = index::checked(
            opened.index.writer_with_num_threads(1, WRITER_MEMORY_BYTES),
            &opened.directory,
        )?;
        let result =
            self.index_changes(&opened, &mut writer, target, rebuilt, cancel).and_then(|count| {
                if cancel() {
                    return Err(Error::Cancelled);
                }
                if self.projection_generation()? != target {
                    return Err(Error::IndexBehind);
                }
                commit(&mut writer, &opened.directory, target)?;
                Ok(count)
            });
        // Uncommitted changes are discarded on writer drop. Join even on cancellation:
        // a merge can still own directory handles from an earlier partial commit.
        let joined = index::checked(writer.wait_merging_threads(), &opened.directory);
        let indexed_events = result?;
        joined?;
        Ok(SearchSyncReport { indexed_events, watermark: target, rebuilt })
    }

    fn index_changes(
        &self,
        opened: &index::Opened,
        writer: &mut IndexWriter<TantivyDocument>,
        target: i64,
        rebuilt: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<u64> {
        let mut last_session = String::new();
        let mut count = 0;
        let mut queued_bytes = 0;
        let mut queued_records = 0;
        loop {
            if cancel() {
                return Err(Error::Cancelled);
            }
            let after = last_session.clone();
            let watermark = opened.watermark;
            let sessions: Vec<String> =
                self.store.read(Db::AgentHistoryReadModel, move |database| {
                    if rebuilt {
                        database.query(
                            "SELECT id FROM ah_sessions WHERE id > ?1 ORDER BY id LIMIT 64",
                            [after],
                            |row| Ok(row.get(0)?),
                        )
                    } else {
                        database.query(
                            "SELECT DISTINCT session_id FROM ah_outbox WHERE seq > ?1 AND seq <= ?2
                            AND session_id > ?3 ORDER BY session_id LIMIT 64",
                            params![watermark, target, after],
                            |row| Ok(row.get(0)?),
                        )
                    }
                })?;
            if sessions.is_empty() {
                break;
            }
            for session in sessions {
                writer.delete_term(Term::from_field_text(
                    opened.fields.session_id,
                    &schema::exact_term(&session),
                ));
                queued_bytes += session.len();
                queued_records += 1;
                if queued_bytes >= PAGE_BYTES || queued_records >= QUEUED_OPERATIONS {
                    commit(writer, &opened.directory, opened.watermark)?;
                    queued_bytes = 0;
                    queued_records = 0;
                }
                let Some(source) = self.index_source(&session)? else {
                    // A tombstone still deletes the prior indexed conversation.
                    last_session = session;
                    continue;
                };
                let mut after_ordinal = -1;
                loop {
                    if cancel() {
                        return Err(Error::Cancelled);
                    }
                    let page = self.index_event_page(&source, after_ordinal)?;
                    if page.is_empty() {
                        break;
                    }
                    for event in page {
                        after_ordinal = event.ordinal;
                        if event.text.is_empty() {
                            continue;
                        }
                        queued_bytes += event.byte_len();
                        queued_records += 1;
                        index::checked(
                            writer.add_document(opened.fields.document(event)),
                            &opened.directory,
                        )?;
                        count += 1;
                        if queued_bytes >= PAGE_BYTES || queued_records >= QUEUED_OPERATIONS {
                            // Tantivy's operation queue is sized in documents, not
                            // bytes, and sits outside its writer memory budget. A
                            // bounded drain prevents thousands of large messages
                            // queuing at once. Keep the old watermark: this partial
                            // index is unavailable until the full replay ends.
                            commit(writer, &opened.directory, opened.watermark)?;
                            queued_bytes = 0;
                            queued_records = 0;
                        }
                    }
                }
                last_session = session;
            }
            if self.projection_generation()? != target {
                return Err(Error::IndexBehind);
            }
        }
        Ok(count)
    }

    fn index_source(&self, session: &str) -> Result<Option<schema::EventSource>> {
        let session = session.to_owned();
        let rows = self.store.read(Db::AgentHistoryReadModel, move |database| {
            database.query(
                "SELECT id, projection_id, harness, coalesce(project_path, '' ) FROM
                    ah_sessions WHERE id=?1 AND (current_only=0 OR conversation_current=1) LIMIT 1",
                [session],
                |row| {
                    Ok(schema::EventSource {
                        session_id: row.get(0)?,
                        projection_id: row.get(1)?,
                        harness: row.get(2)?,
                        project: row.get(3)?,
                    })
                },
            )
        })?;
        Ok(rows.into_iter().next())
    }

    fn index_event_page(
        &self,
        source: &schema::EventSource,
        after_ordinal: i64,
    ) -> Result<Vec<schema::EventDocument>> {
        let source = source.clone();
        Ok(self.store.read(Db::AgentHistoryReadModel, move |database| {
            // Bind the publication once per source. Turso 0.8.3 adds an ORDER BY
            // sorter when these equalities come through the sessions JOIN, even
            // while selecting the right event index. A direct bounded seek avoids
            // sorting the remaining conversation on every page. The publication
            // watermark is checked before the final search commit.
            let mut statement = database.prepare(
                "SELECT id, ordinal, text FROM ah_events WHERE session_id=?1 AND
                    projection_id=?2 AND ordinal>?3 ORDER BY ordinal LIMIT ?4",
            )?;
            let mut rows = statement.query(params![
                source.session_id.clone(),
                source.projection_id,
                after_ordinal,
                PAGE_RECORDS,
            ])?;
            let mut page = Vec::new();
            let mut bytes = 0;
            while let Some(row) = rows.next()? {
                let event = schema::EventDocument {
                    event_id: row.get(0)?,
                    projection_id: source.projection_id,
                    ordinal: row.get(1)?,
                    session_id: source.session_id.clone(),
                    harness: source.harness.clone(),
                    project: source.project.clone(),
                    text: row.get(2)?,
                };
                bytes += event.byte_len();
                page.push(event);
                if bytes >= PAGE_BYTES {
                    break;
                }
            }
            Ok(page)
        })?)
    }
}

fn commit(
    writer: &mut IndexWriter<TantivyDocument>,
    directory: &Option<EncryptedDirectory>,
    watermark: i64,
) -> Result<()> {
    let payload = serde_json::to_string(&index::Commit { version: schema::VERSION, watermark })?;
    let mut prepared = index::checked(writer.prepare_commit(), directory)?;
    prepared.set_payload(&payload);
    index::checked(prepared.commit(), directory)?;
    Ok(())
}

#[cfg(test)]
mod tests;
