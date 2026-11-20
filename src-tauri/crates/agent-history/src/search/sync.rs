//! Responsibilities: replay canonical publication into bounded index batches.
//! Not responsible for: publishing source revisions, background scheduling or user queries.
//! A commit acknowledges only a checked canonical generation; interruption leaves the
//! prior commit or an explicitly incomplete new index, never a false current watermark.

use super::queue::{Queue, Queued};
use super::{SearchSyncReport, index, schema};
use crate::{Archive, Error, Result};
use tantivy::{IndexWriter, Term};
use vault_crypto::EncryptedDirectory;
use vault_store::{Db, params};

const WRITER_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const PAGE_BYTES: usize = 4 * 1024 * 1024;
const PAGE_RECORDS: i64 = 128;

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

    /// Cancellation before the final commit leaves the last complete searcher in service.
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
        let (opened, rebuilt) = match index::open(self) {
            Ok(index) => {
                let reset = force_rebuild || index.watermark > target;
                (index, reset)
            }
            Err(Error::IndexBehind) => (index::recreate(self)?, true),
            Err(err) => return Err(err),
        };
        if !rebuilt && opened.watermark == target && !self.search_cards_dirty()? {
            self.collect_search_selectors()?;
            return Ok(SearchSyncReport { indexed_events: 0, watermark: target, rebuilt });
        }
        let mut writer: IndexWriter<Queued> = index::checked(
            opened.index.writer_with_num_threads(1, WRITER_MEMORY_BYTES),
            &opened.directory,
        )?;
        if rebuilt {
            index::checked(writer.delete_all_documents(), &opened.directory)?;
        }
        let result =
            self.index_changes(&opened, &mut writer, target, rebuilt, cancel).and_then(|count| {
                if cancel() {
                    return Err(Error::Cancelled);
                }
                if self.projection_generation()? != target {
                    return Err(Error::IndexBehind);
                }
                self.index_titles(&opened, &writer, rebuilt, cancel)?;
                let _switch = self.search_switch.lock().unwrap();
                commit(&mut writer, &opened.directory, target)?;
                self.publish_search_sources(opened.watermark, target, rebuilt)?;
                Ok(count)
            });
        // Wait for background segment merges before releasing the archive lease.
        let joined = index::checked(writer.wait_merging_threads(), &opened.directory);
        let indexed_events = result?;
        joined?;
        self.collect_search_selectors()?;
        Ok(SearchSyncReport { indexed_events, watermark: target, rebuilt })
    }

    fn index_titles(
        &self,
        opened: &index::Opened,
        writer: &IndexWriter<Queued>,
        all: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<()> {
        let queue = std::sync::Arc::new(Queue::default());
        let mut after = String::new();
        loop {
            if cancel() {
                return Err(Error::Cancelled);
            }
            let rows = self.store.read(Db::AgentHistoryReadModel, move |s| {
                let sql = if all { "SELECT id,title FROM ah_chat_summary WHERE live=1 AND id>?1 ORDER BY id LIMIT 256" } else { "SELECT d.id,s.title FROM ah_search_dirty d LEFT JOIN ah_chat_summary s ON s.id=d.id AND s.live=1 WHERE d.id>?1 ORDER BY d.id LIMIT 256" };
                s.query(sql, [after], |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?)))
            })?;
            let Some((last, _)) = rows.last() else { return Ok(()) };
            after = last.clone();
            for (id, title) in rows {
                if !all {
                    writer.delete_term(Term::from_field_text(opened.fields.card_id, &id));
                }
                if let Some(title) = title {
                    let bytes = id.len() + title.len();
                    let mut doc = tantivy::TantivyDocument::default();
                    doc.add_text(opened.fields.card_id, id);
                    doc.add_text(opened.fields.title, title);
                    index::checked(writer.add_document(queue.wrap(doc, bytes)), &opened.directory)?;
                }
            }
        }
    }
    fn index_changes(
        &self,
        opened: &index::Opened,
        writer: &mut IndexWriter<Queued>,
        target: i64,
        rebuilt: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<u64> {
        let mut last_session = String::new();
        let mut count = 0;
        let queue = std::sync::Arc::new(Queue::default());
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
                if !rebuilt {
                    writer.delete_term(Term::from_field_text(
                        opened.fields.session_id,
                        &schema::exact_term(&session),
                    ));
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
                        if event.text.is_empty() && event.parts.is_empty() {
                            continue;
                        }
                        let bytes = event.byte_len();
                        index::checked(
                            writer.add_document(queue.wrap(opened.fields.document(event), bytes)),
                            &opened.directory,
                        )?;
                        count += 1;
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
                "SELECT id, projection_id, harness, coalesce(project_path, '' ), CASE WHEN harness='deepseek' THEN (SELECT json_extract(payload,'$.version') FROM ah_events WHERE session_id=ah_sessions.id AND projection_id=ah_sessions.projection_id AND ordinal=0) END,(SELECT publication FROM ah_chat_summary c WHERE c.id=ah_sessions.conversation_id AND c.live=1) FROM
                    ah_sessions WHERE id=?1 AND (current_only=0 OR conversation_current=1) LIMIT 1",
                [session],
                |row| {
                    Ok(schema::EventSource {
                        session_id: row.get(0)?,
                        projection_id: row.get(1)?,
                        harness: row.get(2)?,
                        project: row.get(3)?,
                        format: row.get(4)?,
                        publication: row.get(5)?,
                    })
                },
            )
        })?;
        Ok(rows.into_iter().next())
    }

    pub(super) fn index_event_page(
        &self,
        source: &schema::EventSource,
        after_ordinal: i64,
    ) -> Result<Vec<schema::EventDocument>> {
        self.search_record_page(source, after_ordinal, &[])
    }
    pub(super) fn search_record_page(
        &self,
        source: &schema::EventSource,
        after_ordinal: i64,
        ordinals: &[i64],
    ) -> Result<Vec<schema::EventDocument>> {
        let restrict = if ordinals.is_empty() {
            String::new()
        } else {
            format!(
                " AND ordinal IN ({})",
                ordinals.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
            )
        };
        let bound = source.clone();
        let raw = self.store.read(Db::AgentHistoryReadModel, move |database| {
            let mut statement = database.prepare(&format!(
                "SELECT id,ordinal,text,native_id,kind,role,payload,
                CASE WHEN ?5 IS NOT NULL THEN EXISTS(SELECT 1 FROM ah_chat_mirror m WHERE m.publication=?5 AND m.source=?1 AND m.ordinal=e.ordinal) ELSE 0 END
                FROM ah_events e WHERE session_id=?1 AND projection_id=?2 AND ordinal>?3 {restrict} ORDER BY ordinal LIMIT ?4"
            ))?;
            let publication = if bound.harness == "codex" { bound.publication } else { None };
            let mut rows = statement.query(params![
                bound.session_id,
                bound.projection_id,
                after_ordinal,
                PAGE_RECORDS,
                publication
            ])?;
            let mut page = Vec::new();
            let mut bytes = 0;
            while let Some(row) = rows.next()? {
                let text: String = row.get(2)?;
                let payload: String = row.get(6)?;
                bytes += text.len() + payload.len();
                let native = crate::NativeEvent {
                    references: vec![],
                    ordinal: row.get::<i64>(1)? as u64,
                    native_id: row.get(3)?,
                    parent_id: None,
                    kind: row.get(4)?,
                    role: row.get(5)?,
                    timestamp: None,
                    text,
                    payload: serde_json::Value::Null,
                };
                page.push((row.get::<i64>(0)?, native, payload, row.get::<i64>(7)? != 0));
                if bytes >= PAGE_BYTES {
                    break;
                }
            }
            Ok(page)
        })?;
        let harness = source.harness.parse()?;
        raw.into_iter()
            .map(|(event_id, mut native, payload, mirror)| {
                let parts = if mirror {
                    vec![]
                } else {
                    native.payload = serde_json::from_str(&payload)?;
                    super::expression::readable(&native, harness, source.format)
                };
                Ok(schema::EventDocument {
                    parts,
                    event_id,
                    ordinal: native.ordinal as i64,
                    native_id: native.native_id,
                    projection_id: source.projection_id,
                    session_id: source.session_id.clone(),
                    harness: source.harness.clone(),
                    project: source.project.clone(),
                    text: native.text,
                })
            })
            .collect()
    }
}

fn commit(
    writer: &mut IndexWriter<Queued>,
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
