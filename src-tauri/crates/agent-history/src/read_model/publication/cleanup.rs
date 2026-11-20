//! Reclaims retired publications and unreferenced source versions in bounded writer
//! transactions. A failed owner stays on disk for a later pass. Evidence is never collected here.
use super::*;
use crate::Archive;
use vault_store::{OptionalRow, Priority, Session, Value, params};

enum Retired {
    Publication(i64),
    Version { attempt: Value, row: i64 },
}

// Only failed or concurrently collected owners need a cursor. Productive batches
// keep working on the same owner until its references and payloads have been drained.
struct Pass {
    epoch: i64,
    version_attempt: Value,
    version_row: i64,
}

impl Default for Pass {
    fn default() -> Self {
        Self { epoch: 0, version_attempt: Value::Integer(i64::MIN), version_row: i64::MIN }
    }
}

impl Archive {
    /// Drain all eligible retired reader state, leaving undeletable owners for a later pass.
    pub fn collect_reader_garbage(&self, cancel: &CancelToken) -> Result<()> {
        let mut pass = Pass::default();
        while self.prune_reader_batch(&mut pass, 256, cancel)?.is_some() {}
        Ok(())
    }

    /// Attempts one bounded batch. A full drain also visits owners behind a deferred deletion.
    pub fn prune_reader_pages(&self, limit: usize) -> Result<u64> {
        if !(1..=256).contains(&limit) {
            return Err(Error::InvalidInput("reader cleanup limit must be 1..=256".into()));
        }
        Ok(self.prune_reader_batch(&mut Pass::default(), limit, &CancelToken::new())?.unwrap_or(0))
    }

    fn prune_reader_batch(
        &self,
        pass: &mut Pass,
        limit: usize,
        cancel: &CancelToken,
    ) -> Result<Option<u64>> {
        cancel.check().map_err(|_| Error::Cancelled)?;
        if self.jobs.stopping() {
            return Err(Error::Cancelled);
        }
        let next = match self.next_retired(pass) {
            Ok(Some(owner)) => owner,
            Ok(None) => return Ok(None),
            Err(error) => {
                eprintln!("Conversation reader collection deferred: {error}");
                return Ok(None);
            }
        };
        let size = limit as i64;
        let result = match &next {
            Retired::Publication(epoch) => {
                let epoch = *epoch;
                self.store.write(DB, Priority::Batch, move |tx| prune_publication(tx, epoch, size))
            }
            Retired::Version { row, .. } => {
                let row = *row;
                self.store.write(DB, Priority::Batch, move |tx| prune_version(tx, row, size))
            }
        };
        if let Ok(n) = result.as_ref()
            && *n > 0
        {
            return Ok(Some(*n));
        }
        let (kind, row) = match next {
            Retired::Publication(epoch) => {
                pass.epoch = epoch;
                ("publication", epoch)
            }
            Retired::Version { attempt, row } => {
                pass.version_attempt = attempt;
                pass.version_row = row;
                ("version", row)
            }
        };
        if let Err(error) = result {
            eprintln!(
                "Conversation reader collection deferred for retired {kind} row {row}: {error}"
            );
        }
        Ok(Some(0))
    }

    fn next_retired(&self, pass: &Pass) -> Result<Option<Retired>> {
        let cutoff = self.jobs.collection_cutoff()?;
        let (epoch, attempt, after) = (pass.epoch, pass.version_attempt.clone(), pass.version_row);
        Ok(self.store.read(DB, move |tx| {
            let publication = tx.query_one(
                "SELECT epoch FROM ah_reader_retired WHERE epoch>?1 AND epoch<=?2 ORDER BY epoch LIMIT 1",
                params![epoch, cutoff], |r| Ok(r.get(0)?),
            ).optional()?;
            if let Some(epoch) = publication {
                return Ok(Some(Retired::Publication(epoch)));
            }
            tx.query_one(
                "SELECT attempt,rowid FROM ah_reader_version v
                 WHERE state='retired' AND (attempt,rowid)>(?1,?2)
                 AND NOT EXISTS(SELECT 1 FROM ah_reader_member WHERE version=v.token)
                 AND NOT EXISTS(SELECT 1 FROM ah_reader_line WHERE version=v.token)
                 ORDER BY attempt,rowid LIMIT 1",
                params![attempt, after], |r| Ok(Retired::Version { attempt: r.get_value(0)?, row: r.get(1)? }),
            ).optional()
        })?)
    }
}

fn prune_publication(tx: &Session<'_>, epoch: i64, limit: i64) -> vault_store::Result<u64> {
    let Some(owner) = tx
        .query_one("SELECT publication FROM ah_reader_retired WHERE epoch=?1", [epoch], |r| {
            Ok(r.get_value(0)?)
        })
        .optional()?
    else {
        return Ok(0);
    };
    let mut changed = 0;
    for table in [
        "ah_chat_response",
        "ah_chat_prompt",
        "ah_chat_line_stats",
        "ah_chat_turn",
        "ah_chat_attachment",
        "ah_chat_call",
        "ah_chat_activity",
        "ah_chat_notice",
        "ah_chat_variant",
        "ah_chat_mirror",
        "ah_chat_mirror_rank",
        "ah_chat_extra",
        "ah_chat_insert_rank",
        "ah_chat_operation",
        "ah_chat_summary",
        "ah_reader_member",
        "ah_reader_line",
        "ah_reader_dependency",
        "ah_reader_route",
    ] {
        changed += delete_batch(tx, table, "publication", &owner, limit)?;
    }
    if changed == 0 {
        changed = tx.execute("DELETE FROM ah_reader_retired WHERE epoch=?1", [epoch])?
    }
    Ok(changed)
}

fn prune_version(tx: &Session<'_>, row: i64, limit: i64) -> vault_store::Result<u64> {
    // Recheck eligibility in the deleting transaction; an intervening collector can
    // finish the old version and SQLite may reuse its rowid.
    let Some(owner) = tx
        .query_one(
            "SELECT token FROM ah_reader_version v WHERE rowid=?1 AND state='retired'
        AND NOT EXISTS(SELECT 1 FROM ah_reader_member WHERE version=v.token)
        AND NOT EXISTS(SELECT 1 FROM ah_reader_line WHERE version=v.token)",
            [row],
            |r| Ok(r.get_value(0)?),
        )
        .optional()?
    else {
        return Ok(0);
    };
    let mut changed = 0;
    let pages = tx.query(
        "SELECT rowid,hash FROM ah_reader_page WHERE owner=?1 LIMIT ?2",
        params![owner.clone(), limit],
        |r| Ok((r.get::<i64>(0)?, r.get_value(1)?)),
    )?;
    for (page, hash) in pages {
        changed += tx.execute("DELETE FROM ah_reader_page WHERE rowid=?1", [page])?;
        // Compare the stored value verbatim. An obsolete malformed hash does not identify a live blob.
        tx.execute("DELETE FROM ah_reader_blob WHERE hash=?1 AND NOT EXISTS(SELECT 1 FROM ah_reader_page WHERE hash=?1)",params![hash])?;
    }
    for table in [
        "ah_reader_sort",
        "ah_reader_ambiguous_stage",
        "ah_reader_leaf",
        "ah_chat_leaf_size",
        "ah_chat_leaf_run",
        "ah_reader_walk",
        "ah_reader_walk_run",
    ] {
        changed += delete_batch(tx, table, "owner", &owner, limit)?;
    }
    if changed == 0 {
        changed = tx.execute("DELETE FROM ah_reader_version WHERE rowid=?1", [row])?
    }
    Ok(changed)
}

fn delete_batch(
    tx: &Session<'_>,
    table: &str,
    column: &str,
    owner: &Value,
    limit: i64,
) -> vault_store::Result<u64> {
    tx.execute(&format!("DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} WHERE {column}=?1 LIMIT ?2)"),params![owner.clone(),limit])
}
