//! Responsibilities: retire projection generations and reclaim their rows in bounded jobs.
//! Not responsible for: raw evidence retention, parsing or search-index mutation.
//! Failure modes: cleanup removes an active reparse; retirement leaves a writable stale
//! token; recovery drops published rows; search never learns that a view was retired.
use super::{current_projection_id, publish_outbox, retire, set_coverage, work_attempt};
use crate::{Error, Result, archive::Archive};
use vault_store::{Db, OptionalRow, Priority, params};

impl Archive {
    /// Removes a source's published view when its native generation is no longer
    /// current. Raw bytes stay available, active parser tokens are invalidated,
    /// and a search tombstone is committed with the visibility change.
    /// Returns whether a published view existed; retiring only staging work is false.
    pub fn retire_projection(&self, source_id: &str) -> Result<bool> {
        let snapshot = self.projection_generation()?;
        let source = source_id.to_owned();
        let conversation = self.store.read(Db::AgentHistoryReadModel, move |tx| {
            tx.query_one("SELECT conversation_id FROM ah_sessions WHERE id=?1", [source], |row| {
                Ok(row.get::<String>(0)?)
            })
            .optional()
        })?;
        let selection = match &conversation {
            Some(id) => super::super::selection::remaining(self, id, source_id)?,
            None => None,
        };
        let _publication = self
            .publication_gate
            .lock()
            .map_err(|_| Error::Integrity("publication gate was poisoned".into()))?;
        if self.projection_generation()? != snapshot {
            return Err(Error::SourceChanged("views changed during retirement".into()));
        }
        if conversation.is_some() {
            let source = source_id.to_owned();
            self.store.write(Db::AgentHistoryEvidence,Priority::Batch,move|tx|{
                tx.execute("INSERT OR IGNORE INTO ah_retiring_memberships SELECT source_id FROM ah_source_conversations WHERE source_id=?1",[source])?;
                Ok(())
            })?;
        }
        let retired_source = source_id.to_owned();
        let source_id = source_id.to_owned();
        let published =
            self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
                let published = current_projection_id(tx, &source_id)?;
                if let Some(projection_id) = published {
                    retire(tx, &source_id, projection_id)?;
                }
                if let Some(attempt) = work_attempt(tx, &source_id)? {
                    retire(tx, &source_id, attempt.id)?;
                }
                tx.execute(
                    "DELETE FROM ah_projection_work WHERE session_id=?1",
                    [source_id.as_str()],
                )?;
                let counter = tx
                    .query_one(
                        "SELECT attachment_generation FROM ah_sessions WHERE id=?1",
                        [source_id.as_str()],
                        |row| Ok(row.get::<i64>(0)?),
                    )
                    .optional()?
                    .unwrap_or(0);
                tx.execute(
                    "UPDATE ah_attachment_generations SET state='retired' WHERE id=?1",
                    [counter],
                )?;
                tx.execute("DELETE FROM ah_sessions WHERE id=?1", [source_id.as_str()])?;
                if let (Some(selection), Some(id)) = (selection, conversation.as_ref()) {
                    selection.publish(tx, id, None)?;
                }
                if published.is_some() {
                    crate::read_model::publication::refresh(
                        tx,
                        &source_id,
                        conversation.as_deref(),
                    )?;
                    publish_outbox(tx, &source_id, 0, 0)?;
                }
                // Intentional exclusion of a superseded native generation is not
                // a normalization failure in the currently searchable corpus.
                set_coverage(tx, &source_id, true)?;
                Ok(published.is_some())
            })?;
        // The evidence role owns current inventory membership, including when a
        // previous retirement committed its read-model half before owner loss.
        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            tx.execute(
                "DELETE FROM ah_source_conversations WHERE source_id=?1",
                [retired_source.as_str()],
            )?;
            tx.execute("DELETE FROM ah_retiring_memberships WHERE source_id=?1", [retired_source])?;
            Ok(())
        })?;
        self.flush_reader_heads()?;
        Ok(published)
    }

    /// Reclaims at most `limit` obsolete event rows plus their completed queue
    /// marker. Zero means no progress; canonical bytes are never removed here.
    pub fn prune_obsolete_projection_batch(&self, limit: usize) -> Result<u64> {
        if !(1..=256).contains(&limit) {
            return Err(Error::InvalidInput("projection cleanup limit must be 1..=256".into()));
        }
        self.store
            .write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
                let pair = tx
                    .query_one(
                        "SELECT session_id, projection_id FROM ah_projection_retired LIMIT 1",
                        (),
                        |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)?)),
                    )
                    .optional()?;
                let Some((source_id, projection_id)) = pair else {
                    return Ok(0);
                };
                let is_published = current_projection_id(tx, &source_id)? == Some(projection_id);
                let is_staging = work_attempt(tx, &source_id)?
                    .is_some_and(|attempt| attempt.id == projection_id);
                if is_published || is_staging {
                    return tx.execute(
                        "DELETE FROM ah_projection_retired WHERE session_id=?1 AND
                            projection_id=?2",
                        params![source_id, projection_id],
                    );
                }
                let removed = tx.execute(
                    "DELETE FROM ah_events WHERE id IN (SELECT id FROM ah_events WHERE
                        session_id=?1 AND projection_id=?2 LIMIT ?3)",
                    params![source_id.clone(), projection_id, limit as i64],
                )?;
                let references = tx.execute(
                    "DELETE FROM ah_event_references WHERE rowid IN (
                       SELECT rowid FROM ah_event_references WHERE projection_id=?1 LIMIT ?2)",
                    params![projection_id, limit as i64],
                )?;
                let marker = if removed < limit as u64 && references < limit as u64 {
                    tx.execute(
                        "DELETE FROM ah_projection_gaps WHERE projection_id=?1",
                        [projection_id],
                    )?;
                    tx.execute(
                        "DELETE FROM ah_projection_retired WHERE session_id=?1 AND
                            projection_id=?2",
                        params![source_id, projection_id],
                    )?
                } else {
                    0
                };
                Ok(removed + references + marker)
            })
            .map_err(Into::into)
    }

    /// Startup abandons parser attempts in bounded transactions. The published
    /// generation survives; its replacement's rows enter the cleanup queue.
    pub(crate) fn recover_projection_work(&self) -> Result<()> {
        loop {
            let count = self.store.write(Db::AgentHistoryReadModel, Priority::Batch, |tx| {
                let work = tx.query(
                    "SELECT session_id, projection_id FROM ah_projection_work LIMIT 128",
                    (),
                    |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)?)),
                )?;
                for (source_id, projection_id) in &work {
                    retire(tx, source_id, *projection_id)?;
                    tx.execute(
                        "DELETE FROM ah_projection_work WHERE session_id=?1",
                        [source_id.as_str()],
                    )?;
                }
                Ok(work.len())
            })?;
            if count == 0 {
                return Ok(());
            }
        }
    }
}
