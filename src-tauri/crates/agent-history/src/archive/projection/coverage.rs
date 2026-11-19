//! Responsibilities: update current parsing/reference coverage without stale success.
//! Not responsible for: resolving references or rebuilding projection/search content.
use super::operation;
use crate::{Error, Result, archive::Archive};
use vault_store::{Db, OptionalRow, Priority, params};

impl Archive {
    /// Records current coverage while allowing failed capture to invalidate a
    /// previous interpretation. Restoring completeness requires that interpretation
    /// to represent the current raw head. Root-level discovery entries have no session.
    pub fn set_coverage(&self, source_id: &str, complete: bool) -> Result<()> {
        let affects = match self.source(source_id) {
            Ok(source) => source.kind == "transcript",
            Err(Error::NotFound(_)) => true,
            Err(error) => return Err(error),
        };
        let source_id = source_id.to_owned();
        if complete {
            let _publication = self
                .publication_gate
                .lock()
                .map_err(|_| Error::Integrity("publication gate was poisoned".into()))?;
            if let Some((raw_revision, parser_complete)) =
                self.current_coverage_target(&source_id)?
            {
                if !parser_complete {
                    return Err(Error::InvalidInput(
                        "combined coverage cannot exceed parser coverage".into(),
                    ));
                }
                let source = self.source(&source_id)?;
                if source.latest_revision_id != Some(raw_revision) {
                    return Err(Error::SourceChanged(
                        "coverage refers to a superseded raw revision".into(),
                    ));
                }
            }
            self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
                write_status(tx, &source_id, Some((true, affects)), None, false)
            })?;
        } else {
            self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
                write_status(tx, &source_id, Some((false, affects)), Some(false), false)
            })?;
        }
        Ok(())
    }

    /// Repairs coverage only for the exact published interpretation observed by
    /// the resolver. Reparsed bytes or a changed raw head reject a stale repair.
    pub fn update_projection_coverage(
        &self,
        source_id: &str,
        projection_id: i64,
        parser_complete: bool,
        combined_complete: bool,
    ) -> Result<()> {
        validate_coverage(parser_complete, combined_complete)?;
        let _publication = self
            .publication_gate
            .lock()
            .map_err(|_| Error::Integrity("publication gate was poisoned".into()))?;
        let source = self.source(source_id)?;
        let raw_revision = source
            .latest_revision_id
            .ok_or_else(|| Error::InvalidInput("source has no published raw revision".into()))?;
        let source_id = source_id.to_owned();
        let refresh = self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
            let parsed = tx.query_one(
                "SELECT complete FROM ah_sessions WHERE id=?1",
                [source_id.as_str()],
                |r| Ok(r.get::<i64>(0)? != 0),
            )?;
            let changed = tx.execute(
                "UPDATE ah_sessions SET complete=?1
                 WHERE id=?2 AND projection_id=?3 AND
                    revision_id=?4",
                params![i64::from(parser_complete), source_id.clone(), projection_id, raw_revision],
            )?;
            if changed != 1 {
                return Err(operation(
                    "coverage update belongs to a superseded projection or raw revision",
                ));
            }
            write_status(
                tx,
                &source_id,
                Some((combined_complete, true)),
                Some(true),
                parsed != parser_complete,
            )
        })?;
        if refresh {
            self.flush_reader_heads()?
        }
        Ok(())
    }

    pub(crate) fn reference_coverage_scope(&self, source: &str) -> Result<()> {
        let source = source.to_owned();
        self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
            let existing=tx.query_one("SELECT complete FROM ah_coverage WHERE source_id=?1 AND NOT EXISTS(SELECT 1 FROM ah_sessions WHERE id=?1)",[source.as_str()],|r|Ok(r.get::<i64>(0)?!=0)).optional()?;
            if let Some(complete)=existing{write_status(tx,&source,Some((complete,false)),None,false)?;}
            Ok(())
        })?;
        Ok(())
    }

    fn current_coverage_target(&self, source_id: &str) -> Result<Option<(i64, bool)>> {
        let source_id = source_id.to_owned();
        self.store
            .read(Db::AgentHistoryReadModel, move |tx| {
                tx.query_one(
                    "SELECT revision_id, complete FROM ah_sessions WHERE id=?1",
                    [source_id],
                    |row| Ok((row.get(0)?, row.get::<i64>(1)? != 0)),
                )
                .optional()
            })
            .map_err(Into::into)
    }
}

pub(super) fn validate_coverage(parser_complete: bool, combined_complete: bool) -> Result<()> {
    if combined_complete && !parser_complete {
        return Err(Error::InvalidInput("combined coverage cannot exceed parser coverage".into()));
    }
    Ok(())
}

/// Update coverage and freshness with their membership invalidation. Callers changing other
/// structural inputs can force the same ticket even when the coverage bits stay equal.
pub(super) fn write_status(
    tx: &vault_store::Session<'_>,
    source: &str,
    coverage: Option<(bool, bool)>,
    current: Option<bool>,
    force: bool,
) -> vault_store::Result<bool> {
    let mut changed = force;
    if let Some(fresh) = current {
        changed |= tx.execute(
            "UPDATE ah_sessions SET raw_head_current=?1 WHERE id=?2 AND raw_head_current<>?1",
            params![i64::from(fresh), source],
        )? > 0;
    }
    if let Some((complete, affects)) = coverage {
        changed |= tx.execute("INSERT INTO ah_coverage(source_id,complete,affects_conversations) VALUES(?1,?2,?3)
          ON CONFLICT(source_id) DO UPDATE SET complete=excluded.complete,affects_conversations=excluded.affects_conversations
          WHERE complete<>excluded.complete OR affects_conversations<>excluded.affects_conversations",params![source,i64::from(complete),i64::from(affects)])?>0;
    }
    if changed {
        crate::read_model::publication::refresh(tx, source, None)?;
    }
    Ok(changed)
}
