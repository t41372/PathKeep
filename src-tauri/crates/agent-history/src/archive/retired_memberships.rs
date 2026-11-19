//! Reconcile only flagged fallback memberships after an upgrade or interrupted retirement.
//! Historical run tallies, raw revisions and native source files are independent of this repair.
use super::Archive;
use crate::{Error, Result};
use vault_store::{Db, Priority};

impl Archive {
    pub(super) fn recover_retired_memberships(&self) -> Result<()> {
        loop {
            let ids=self.store.read(Db::AgentHistoryEvidence,|tx|tx.query(
                "SELECT source_id FROM ah_retiring_memberships ORDER BY source_id LIMIT 128",(),|r|Ok(r.get::<String>(0)?)
            )).map_err(|e|Error::Store(e.at(Db::AgentHistoryEvidence,vault_store::OpenStage::Recover)))?;
            if ids.is_empty() {
                return Ok(());
            }
            let membership = self
                .store
                .read(Db::AgentHistoryReadModel, move |tx| {
                    ids.into_iter()
                        .map(|id| {
                            let published = tx.query_one(
                                "SELECT EXISTS(SELECT 1 FROM ah_sessions WHERE id=?1)",
                                [id.as_str()],
                                |r| Ok(r.get::<i64>(0)? != 0),
                            )?;
                            Ok((id, published))
                        })
                        .collect::<vault_store::Result<Vec<_>>>()
                })
                .map_err(|e| {
                    Error::Store(e.at(Db::AgentHistoryReadModel, vault_store::OpenStage::Recover))
                })?;
            self.store
                .write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
                    for (id, published) in membership {
                        if !published {
                            tx.execute(
                                "DELETE FROM ah_source_conversations WHERE source_id=?1",
                                [id.as_str()],
                            )?;
                        }
                        tx.execute("DELETE FROM ah_retiring_memberships WHERE source_id=?1", [id])?;
                    }
                    Ok(())
                })
                .map_err(|e| {
                    Error::Store(e.at(Db::AgentHistoryEvidence, vault_store::OpenStage::Recover))
                })?;
        }
    }
}
