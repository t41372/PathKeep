//! Responsibilities: identify parser attempts and expose publication/coverage state.
//! Not responsible for: parsing, raw retention, search indexing or UI ordering.
//! A projection generation is independent of its raw revision: reparsing the same
//! bytes must not mutate the published interpretation or authorize a stale writer.
mod cleanup;
mod coverage;
mod write;

use super::Archive;
use crate::{Error, Result};
use vault_store::{Db, OptionalRow, Priority, Session, params};

/// An attempt's identity prevents a superseded parser from writing into a newer
/// interpretation of the same bytes. Every mutation checks all fields atomically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionAttempt {
    pub id: i64,
    pub source_id: String,
    pub revision_id: i64,
    pub parser_version: String,
}

/// Capture can publish the native interpretation even when its reader pages need repair.
#[derive(Clone, Copy)]
pub(crate) enum ReaderStage {
    Ready,
    AwaitingRebuild,
}

impl Archive {
    /// Allocates a staging generation while keeping the published one visible.
    /// Identical raw bytes and parser identity reuse the existing interpretation,
    /// including known limitations; an explicit force requests another attempt.
    pub fn prepare_projection(
        &self,
        source_id: &str,
        revision: i64,
        parser_version: &str,
        force: bool,
    ) -> Result<Option<ProjectionAttempt>> {
        self.require_revision_source(source_id, revision)?;
        if parser_version.is_empty() || parser_version.len() > 256 {
            return Err(Error::InvalidInput("parser identity must contain 1..=256 bytes".into()));
        }
        let source_id = source_id.to_owned();
        let parser_version = parser_version.to_owned();
        self.store
            .write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
                let current = tx
                    .query_one(
                        "SELECT s.revision_id, s.parser_version FROM ah_sessions s WHERE s.id=?1 AND (
                         EXISTS(SELECT 1 FROM ah_reader_version v WHERE v.attempt=s.projection_id AND v.codec=?2 AND v.renderer=?3 AND v.state='published')
                         OR EXISTS(SELECT 1 FROM ah_reader_head h JOIN ah_reader_member m ON m.publication=h.publication
                           WHERE h.conversation=s.conversation_id AND m.source=s.id AND m.version=''))",
                        params![source_id.clone(),crate::read_model::publication::CODEC,crate::read_model::publication::RENDERER],
                        |row| Ok((row.get::<i64>(0)?, row.get::<String>(1)?)),
                    )
                    .optional()?;
                if !force && current == Some((revision, parser_version.clone())) {
                    return Ok(None);
                }
                if let Some(previous) = work_attempt(tx, &source_id)? {
                    retire(tx, &source_id, previous.id)?;
                }
                let previous = tx.query_one(
                    "SELECT value FROM ah_meta WHERE key='next_projection_id'",
                    (),
                    |row| Ok(row.get::<String>(0)?),
                )?;
                let id = previous
                    .parse::<i64>()
                    .ok()
                    .filter(|id| *id >= 0)
                    .and_then(|id| id.checked_add(1))
                    .ok_or_else(|| {
                        operation("projection identity counter is invalid or exhausted")
                    })?;
                tx.execute(
                    "UPDATE ah_meta SET value=?1 WHERE key='next_projection_id'",
                    [id.to_string()],
                )?;
                tx.execute(
                    "INSERT INTO ah_projection_work(session_id, revision_id, projection_id,
                    parser_version) VALUES(?1, ?2, ?3, ?4) ON CONFLICT(session_id) DO UPDATE SET
                    revision_id=excluded.revision_id, projection_id=excluded.projection_id,
                    parser_version=excluded.parser_version",
                    params![source_id.clone(), revision, id, parser_version.clone()],
                )?;
                Ok(Some(ProjectionAttempt { id, source_id, revision_id: revision, parser_version }))
            })
            .map_err(Into::into)
    }

    /// The raw revision represented by the published interpretation. This does
    /// not identify a parser generation; use `current_projection_id` for cursors.
    pub fn current_projection(&self, source_id: &str) -> Result<Option<i64>> {
        let source_id = source_id.to_owned();
        self.store
            .read(Db::AgentHistoryReadModel, move |tx| {
                tx.query_one(
                    "SELECT revision_id FROM ah_sessions WHERE id=?1",
                    [source_id],
                    |row| Ok(row.get(0)?),
                )
                .optional()
            })
            .map_err(Into::into)
    }

    /// Identifies the visible parser generation even when source bytes are unchanged.
    pub fn current_projection_id(&self, source_id: &str) -> Result<Option<i64>> {
        let source_id = source_id.to_owned();
        self.store
            .read(Db::AgentHistoryReadModel, move |tx| current_projection_id(tx, &source_id))
            .map_err(Into::into)
    }

    /// A stable publication clock lets search and paginated readers reject stale state.
    pub fn projection_generation(&self) -> Result<i64> {
        self.store
            .read(Db::AgentHistoryReadModel, |tx| {
                tx.query_one("SELECT coalesce(max(seq), 0) FROM ah_outbox", (), |row| {
                    Ok(row.get(0)?)
                })
            })
            .map_err(Into::into)
    }

    fn require_revision_source(&self, source_id: &str, revision: i64) -> Result<()> {
        if self.revision(revision)?.source_id != source_id {
            return Err(Error::InvalidInput("revision belongs to another source".into()));
        }
        Ok(())
    }
}

pub(super) fn current_projection_id(
    tx: &Session<'_>,
    source_id: &str,
) -> vault_store::Result<Option<i64>> {
    tx.query_one("SELECT projection_id FROM ah_sessions WHERE id=?1", [source_id], |row| {
        Ok(row.get(0)?)
    })
    .optional()
}

pub(super) fn work_attempt(
    tx: &Session<'_>,
    source_id: &str,
) -> vault_store::Result<Option<ProjectionAttempt>> {
    tx.query_one(
        "SELECT projection_id, revision_id, parser_version FROM ah_projection_work WHERE
            session_id=?1",
        [source_id],
        |row| {
            Ok(ProjectionAttempt {
                id: row.get(0)?,
                source_id: source_id.to_owned(),
                revision_id: row.get(1)?,
                parser_version: row.get(2)?,
            })
        },
    )
    .optional()
}

pub(crate) fn require_work(
    tx: &Session<'_>,
    attempt: &ProjectionAttempt,
) -> vault_store::Result<()> {
    if work_attempt(tx, &attempt.source_id)?.as_ref() != Some(attempt) {
        return Err(operation("projection attempt has not started or was superseded"));
    }
    if current_projection_id(tx, &attempt.source_id)? == Some(attempt.id) {
        return Err(operation("published projection is immutable"));
    }
    Ok(())
}

pub(super) fn retire(
    tx: &Session<'_>,
    source_id: &str,
    projection_id: i64,
) -> vault_store::Result<()> {
    tx.execute("UPDATE ah_reader_version SET state='retired' WHERE attempt=?1", [projection_id])?;
    tx.execute(
        "INSERT OR IGNORE INTO ah_projection_retired(session_id, projection_id) VALUES(?1, ?2)",
        params![source_id, projection_id],
    )?;
    Ok(())
}

pub(super) fn publish_outbox(
    tx: &Session<'_>,
    source_id: &str,
    revision: i64,
    projection: i64,
) -> vault_store::Result<()> {
    let sequence = tx
        .insert(
            "INSERT INTO ah_outbox(session_id, revision_id, projection_id) VALUES(?1, ?2, ?3)",
            params![source_id, revision, projection],
        )?
        .ok_or_else(|| operation("publication did not create its search record"))?;
    tx.execute(
        "INSERT INTO ah_meta(key, value) VALUES('generation', ?1) ON CONFLICT(key) DO UPDATE
            SET value=excluded.value",
        [sequence.to_string()],
    )?;
    Ok(())
}

pub(super) fn set_coverage(
    tx: &Session<'_>,
    source_id: &str,
    complete: bool,
) -> vault_store::Result<()> {
    coverage::write_status(tx, source_id, Some((complete, true)), None, false).map(|_| ())
}

fn operation(message: &'static str) -> vault_store::Error {
    vault_store::Error::Operation(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message,
    )))
}
