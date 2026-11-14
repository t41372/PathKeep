//! Responsibilities: reclaim bounded failed-capture staging without deleting retained evidence.
//! Not responsible for: retention policy, repairing corruption or choosing maintenance cadence.
use super::Archive;
use crate::{Error, Result};
use vault_store::{Db, OptionalRow, Priority, params};

impl Archive {
    /// Reclaims abandoned/duplicate manifests and chunks with no remaining references.
    /// Completed revisions are never eligible. Returns progress including removed revision
    /// markers, so callers can schedule small batches without scanning the corpus.
    pub fn prune_abandoned_capture_batch(&self, limit: usize) -> Result<u64> {
        if !(1..=256).contains(&limit) {
            return Err(Error::InvalidInput("capture cleanup limit must be 1..=256".into()));
        }
        Ok(self.store.write(Db::SourceEvidence, Priority::Batch, move |tx| {
            let mut revision = None;
            for state in ["abandoned", "duplicate"] {
                revision = tx
                    .query_one(
                        "SELECT id FROM ah_revisions WHERE state=?1 ORDER BY id LIMIT 1",
                        [state],
                        |row| Ok(row.get::<i64>(0)?),
                    )
                    .optional()?;
                if revision.is_some() {
                    break;
                }
            }
            let Some(revision) = revision else {
                return Ok(0);
            };
            let chunks = tx.query(
                "SELECT ordinal, chunk_hash FROM ah_revision_chunks WHERE revision_id=?1 ORDER
                    BY ordinal LIMIT ?2",
                params![revision, limit as i64],
                |row| Ok((row.get::<i64>(0)?, row.get::<String>(1)?)),
            )?;
            let mut changed = chunks.len() as u64;
            for (ordinal, hash) in chunks {
                tx.execute(
                    "DELETE FROM ah_revision_chunks WHERE revision_id=?1 AND ordinal=?2",
                    params![revision, ordinal],
                )?;
                let referenced: i64 = tx.query_one(
                    "SELECT EXISTS(SELECT 1 FROM ah_revision_chunks WHERE chunk_hash=?1)",
                    [hash.as_str()],
                    |row| Ok(row.get(0)?),
                )?;
                if referenced == 0 {
                    tx.execute("DELETE FROM ah_chunks WHERE hash=?1", [hash])?;
                }
            }
            let remains: i64 = tx.query_one(
                "SELECT EXISTS(SELECT 1 FROM ah_revision_chunks WHERE revision_id=?1)",
                [revision],
                |row| Ok(row.get(0)?),
            )?;
            if remains == 0 {
                // These tables are populated only after raw publication. Their explicit
                // absence guards cleanup against a future misuse of the staging state.
                let referenced: i64 = tx.query_one(
                    "SELECT EXISTS(SELECT 1 FROM ah_sources WHERE latest_revision_id=?1)",
                    [revision],
                    |row| Ok(row.get(0)?),
                )?;
                if referenced != 0 {
                    return Err(super::revisions::integrity(
                        "unfinished revision is a published source head",
                    ));
                }
                changed += tx.execute(
                    "
                    DELETE FROM ah_revisions WHERE id=?1 AND state IN ('abandoned', 'duplicate' )",
                    [revision],
                )?;
            }
            Ok(changed)
        })?)
    }
}
