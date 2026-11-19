//! Checks whether an offline reconstruction can retain its existing immutable page owner.
//! The canonical input digest never depends on a random staging token.
use super::*;
use crate::Archive;
use vault_store::params;

impl Archive {
    /// Tickets are durable even while the group remains available to other readers.
    pub fn has_reader_repair_work(store: &vault_store::Store) -> vault_store::Result<bool> {
        store.read(DB, |tx| {
            tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM ah_reader_parked) OR EXISTS(SELECT 1 FROM ah_reader_dirty)
                OR EXISTS(SELECT 1 FROM ah_reader_head WHERE awaiting_rebuild=1)",
            (), |r| Ok(r.get::<i64>(0)? != 0))
        })
    }

    /// Repair a checkpointed private pair. Freeze the reviewed IDs in its scratch
    /// store because resolving a dependency can retire several tickets together.
    pub fn rebuild_pending_readers(
        store: std::sync::Arc<vault_store::Store>,
        resources: crate::AgentHistoryResources,
        job: &JobControl,
    ) -> Result<u64> {
        use vault_store::{Db, Priority};
        let archive = Self::bind_store(store.clone(), resources)?;
        store.write(Db::Scratch, Priority::Batch, |s| {
            s.execute("CREATE TABLE repair_readers(id TEXT PRIMARY KEY)", ())
        })?;
        let mut cursor = String::new();
        loop {
            job.cancel.check()?;
            let batch = store.read(DB, move |s| s.query(
                "SELECT conversation FROM ah_reader_parked WHERE conversation>?1
                 UNION SELECT conversation FROM ah_reader_dirty WHERE conversation>?1
                 UNION SELECT conversation FROM ah_reader_head WHERE awaiting_rebuild=1 AND conversation>?1
                 ORDER BY conversation LIMIT 128", [cursor], |r| Ok(r.get::<String>(0)?)))?;
            let Some(last) = batch.last() else { break };
            cursor = last.clone();
            store.write(Db::Scratch, Priority::Batch, move |s| {
                for id in batch {
                    s.execute("INSERT INTO repair_readers VALUES(?1)", [id])?;
                }
                Ok(())
            })?;
        }
        // A dependent conversation may sort before the one supplying its base.
        // Finish all version repairs before publishing or verifying any of the heads.
        for_reviewed(&store, job, |id| {
            archive.rebuild_conversation_versions(&id, job)?;
            store.write(DB, Priority::Batch, move |s| {
                s.execute(
                    "INSERT OR IGNORE INTO ah_reader_dirty VALUES(?1,lower(hex(randomblob(16))))",
                    [id],
                )
            })?;
            Ok(())
        })?;
        archive.recover_reader_heads(&job.cancel)?;
        let mut recovered = 0;
        for_reviewed(&store, job, |id| {
            match archive.reader_header(&id) {
                Ok(_) => recovered += 1,
                Err(Error::ConversationNotFound) => {
                    // Removing the head of a retired conversation also resolves a ticket.
                    let retained = store.read(Db::AgentHistoryEvidence, move |s| s.query_one(
                        "SELECT EXISTS(SELECT 1 FROM ah_source_conversations WHERE conversation_id=?1)",
                        [id], |r| Ok(r.get::<i64>(0)? != 0)))?;
                    if retained {
                        return Err(Error::ReaderRebuildRequired);
                    }
                }
                Err(error) => return Err(error),
            }
            Ok(())
        })?;
        if Self::has_reader_repair_work(&store)? {
            return Err(Error::ReaderRebuildRequired);
        }
        archive.collect_reader_garbage(&job.cancel)?;
        archive.stop_jobs();
        Ok(recovered)
    }

    pub(super) fn same_reader_pages(&self, old: &str, new: &str, job: &JobControl) -> Result<bool> {
        #[cfg(test)]
        if super::tests::round_two::CHECK_GATE.replace(false) {
            assert!(
                self.publication_gate.try_lock().is_ok(),
                "immutable comparison holds publication gate"
            );
        }
        let (a, b) = (old.to_owned(), new.to_owned());
        match self
            .store
            .read(DB, move |tx| manifest::verify_version(tx, &a))
            .map_err(derived_read_error)
        {
            Err(Error::ReaderRebuildRequired) => return Ok(false),
            Err(error) => return Err(error),
            Ok(()) => {}
        }
        let a = old.to_owned();
        let metadata = self.store.read(DB, move |tx| {
            tx.query_one(
            "SELECT a.codec=b.codec AND a.renderer=b.renderer AND a.digest=b.digest AND a.byte_len=b.byte_len
             AND a.record_limit=b.record_limit AND a.decoded_limit=b.decoded_limit AND a.records=b.records
             AND a.max_frame=b.max_frame AND a.selective=b.selective AND a.format_version IS b.format_version
             AND a.header_ordinal IS b.header_ordinal AND a.visible_total=b.visible_total AND a.rank_pages=b.rank_pages
             AND a.pieces=b.pieces AND a.nonempty_blocks=b.nonempty_blocks AND a.ambiguous_edges=b.ambiguous_edges AND a.source=b.source AND a.revision=b.revision AND a.postings=b.postings AND a.lookup_pages=b.lookup_pages AND a.directory_pages=b.directory_pages AND a.leaf_count=b.leaf_count AND a.duplicate_ids=b.duplicate_ids
             AND (SELECT count(*) FROM ah_reader_page WHERE owner=a.token)=(SELECT count(*) FROM ah_reader_page WHERE owner=b.token)
             FROM ah_reader_version a JOIN ah_reader_version b ON b.token=?2 WHERE a.token=?1",
                params![a,b],
                |row| Ok(row.get::<i64>(0)?!=0)
            )
        })?;
        if !metadata {
            return Ok(false);
        }
        let owner = old.to_owned();
        let candidate = new.to_owned();
        let selections = self
            .store
            .read(DB, move |tx| {
                Ok((leaves::selection(tx, &owner)?, leaves::selection(tx, &candidate)?))
            })
            .map_err(Error::from);
        match selections {
            Ok((old_selection, new_selection)) => {
                if old_selection != new_selection {
                    return Ok(false);
                }
                if let Some(s) = old_selection {
                    let depth = (s.visible - u64::from(s.boundary.is_some())) as i64;
                    for first in (0..depth).step_by(200) {
                        job.cancel.check().map_err(|_| Error::Cancelled)?;
                        let (a, b) = (old.to_owned(), new.to_owned());
                        let same = self
                            .store
                            .read(DB, move |tx| {
                                let end = depth.min(first + 200);
                                Ok(runs::page(tx, &a, runs::SELECTED, depth, first, end)?
                                    == runs::page(tx, &b, runs::SELECTED, depth, first, end)?)
                            })
                            .map_err(Error::from);
                        match same {
                            Ok(true) => {}
                            Ok(false) => return Ok(false),
                            Err(e) if local_reader_damage(&e) => return Ok(false),
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
            Err(e) if local_reader_damage(&e) => return Ok(false),
            Err(e) => return Err(e),
        }
        if !self.walks_intact(old, job)? {
            return Ok(false);
        }
        let mut after = (0, 0, 0);
        loop {
            job.cancel.check().map_err(|_| Error::Cancelled)?;
            let (a, b) = (old.to_owned(), new.to_owned());
            let batch = self.store.read(DB, move |tx| {
                tx.query(
                    "SELECT n.family,n.first_rank,n.part,n.nbytes,n.hash,o.binding FROM ah_reader_page n
                LEFT JOIN ah_reader_page o ON o.owner=?1 AND o.family=n.family AND o.first_rank=n.first_rank AND o.part=n.part AND o.hash=n.hash AND o.nbytes=n.nbytes
                WHERE n.owner=?2 AND (n.family,n.first_rank,n.part)>(?3,?4,?5) ORDER BY n.family,n.first_rank,n.part LIMIT 256",
                    params![a,b,after.0,after.1,after.2],
                    |row| Ok((
                        row.get::<i64>(0)?, row.get::<i64>(1)?, row.get::<i64>(2)?,
                        row.get::<i64>(3)?, row.get::<Vec<u8>>(4)?, row.get::<Option<Vec<u8>>>(5)?,
                    )),
                )
            })?;
            if batch.is_empty() {
                return Ok(true);
            }
            for (family, rank, part, len, hash, stamp) in batch {
                if stamp != Some(pages::binding(old, family, rank, part, len, &hash)) {
                    return Ok(false);
                }
                after = (family, rank, part);
            }
        }
    }
}

fn for_reviewed(
    store: &vault_store::Store,
    job: &JobControl,
    mut visit: impl FnMut(String) -> Result<()>,
) -> Result<()> {
    let mut cursor = String::new();
    loop {
        job.cancel.check()?;
        let batch = store.read(Db::Scratch, move |s| {
            s.query(
                "SELECT id FROM repair_readers WHERE id>?1 ORDER BY id LIMIT 128",
                [cursor],
                |r| Ok(r.get::<String>(0)?),
            )
        })?;
        let Some(last) = batch.last() else { return Ok(()) };
        cursor = last.clone();
        for id in batch {
            visit(id)?;
        }
    }
}
