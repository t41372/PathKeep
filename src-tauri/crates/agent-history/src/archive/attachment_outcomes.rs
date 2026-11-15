//! Responsibilities: preserve redacted reference outcomes for offline reparse and query caches.
//! Not responsible for: probing live paths or storing targets in diagnostics.
use super::{Archive, references::reference_key};
use crate::{Reference, Result};
use vault_store::{Db, OptionalRow, Priority, params};

impl Archive {
    pub(crate) fn reference_reason(
        &self,
        revision: i64,
        reference: &Reference,
    ) -> Result<Option<Option<String>>> {
        let key = reference_key(reference)?;
        Ok(self.store.read(Db::AgentHistoryEvidence, move |tx| {
            tx.query_one(
                "SELECT reason FROM ah_reference_observations
                 WHERE parent_revision=?1 AND reference_key=?2 ORDER BY id DESC LIMIT 1",
                params![revision, key],
                |row| Ok(row.get::<Option<String>>(0)?),
            )
            .optional()
        })?)
    }

    pub(crate) fn record_reference_outcome(
        &self,
        revision: i64,
        reference: &Reference,
        reason: Option<&str>,
    ) -> Result<bool> {
        let key = reference_key(reference)?;
        let reason = reason.map(str::to_owned);
        let saved_key = key.clone();
        let saved_reason = reason.clone();
        self.store.write(Db::AgentHistoryEvidence, Priority::Batch, move |tx| {
            let previous = tx
                .query_one(
                    "SELECT reason FROM ah_reference_observations
                 WHERE parent_revision=?1 AND reference_key=?2 ORDER BY id DESC LIMIT 1",
                    params![revision, saved_key.clone()],
                    |row| Ok(row.get::<Option<String>>(0)?),
                )
                .optional()?;
            if previous != Some(saved_reason.clone()) {
                tx.execute(
                    "INSERT INTO ah_reference_observations(
                       parent_revision,reference_key,reason,observed_at) VALUES(?1,?2,?3,?4)",
                    params![
                        revision,
                        saved_key,
                        saved_reason,
                        chrono::Utc::now().timestamp_millis()
                    ],
                )?;
            }
            Ok(())
        })?;
        Ok(self.store.write(Db::AgentHistoryReadModel, Priority::Batch, move |tx| {
            let previous = tx
                .query_one(
                    "SELECT reason FROM ah_reference_outcomes
                 WHERE parent_revision=?1 AND reference_key=?2",
                    params![revision, key.clone()],
                    |row| Ok(row.get::<Option<String>>(0)?),
                )
                .optional()?;
            tx.execute(
                "INSERT INTO ah_reference_outcomes VALUES(?1,?2,?3)
                 ON CONFLICT(parent_revision,reference_key) DO UPDATE SET reason=excluded.reason",
                params![revision, key, reason.clone()],
            )?;
            Ok(previous != Some(reason))
        })?)
    }
}
