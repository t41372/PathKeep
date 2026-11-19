//! Switches source membership, line runs and their dependencies with the legacy source election.
//! Payload decoding and page construction are outside the publication transaction.
use super::*;
use crate::{archive::ProjectionAttempt, query::operation};
use std::collections::BTreeSet;
use vault_store::{OptionalRow, Session, params};

pub(crate) fn install(tx: &Session<'_>, attempt: &ProjectionAttempt) -> vault_store::Result<()> {
    let changed = tx.execute(
        "UPDATE ah_reader_version SET state='published' WHERE attempt=?1 AND
                 state='ready' AND revision=?2 AND source=?3",
        params![attempt.id, attempt.revision_id, attempt.source_id.clone()],
    )?;
    if changed != 1 {
        return Err(operation(Error::StaleCursor));
    }
    Ok(())
}

pub(super) fn source_key(source: &str) -> String {
    format!("source/{source}")
}
pub(super) fn rollout_key(harness: &str, rollout: &str) -> String {
    format!("rollout/{harness}/{rollout}")
}

pub(crate) fn refresh(
    tx: &Session<'_>,
    source: &str,
    prior: Option<&str>,
) -> vault_store::Result<()> {
    let mut keys = vec![source_key(source)];
    let mut affected = BTreeSet::new();
    if let Some(prior) = prior {
        affected.insert(prior.to_owned());
    }
    if let Some((id, harness, rollout, native)) = tx
        .query_one(
            "SELECT conversation_id,harness,rollout_native_id,native_id FROM ah_sessions
                 WHERE id=?1",
            [source],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<Option<String>>(2)?,
                    r.get::<String>(3)?,
                ))
            },
        )
        .optional()?
    {
        affected.insert(id);
        keys.push(format!("native/{harness}/{native}"));
        if let Some(rollout) = rollout {
            keys.push(rollout_key(&harness, &rollout));
        }
    }
    // A rename, retirement or change in copy authority can also resolve a formerly
    // ambiguous edge that names the previous native identity.
    let previous = tx.query(
        "SELECT h.conversation,CAST(m.descriptor AS TEXT) FROM ah_reader_member m JOIN ah_reader_head h ON
                 h.publication=m.publication WHERE m.source=?1",
        [source],
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
    )?;
    for (conversation, raw) in previous {
        affected.insert(conversation);
        let Ok(member) = serde_json::from_str::<SourceMember>(&raw) else { continue };
        keys.push(format!("native/{}/{}", member.harness, member.native_id));
        if let Some(rollout) = member.rollout_id {
            keys.push(rollout_key(&member.harness, &rollout))
        }
    }
    // Either index can route an invalidation if one dependency row has been lost.
    for key in keys {
        tx.execute(
            "INSERT OR REPLACE INTO ah_reader_dirty
            SELECT h.conversation,lower(hex(randomblob(16))) FROM (
                SELECT publication FROM ah_reader_dependency WHERE target=?1
                UNION SELECT publication FROM ah_reader_route WHERE target=?1
            ) d JOIN ah_reader_head h ON h.publication=d.publication",
            [key],
        )?;
    }
    for id in affected {
        tx.execute(
            "INSERT OR REPLACE INTO ah_reader_dirty VALUES(?1,lower(hex(randomblob(16))))",
            [id],
        )?;
    }
    Ok(())
}

pub(super) fn dependency(
    tx: &Session<'_>,
    publication: &str,
    key: String,
) -> vault_store::Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO ah_reader_dependency VALUES(?1,?2)",
        params![key.as_str(), publication],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO ah_reader_route(publication,target) VALUES(?1,?2)",
        params![publication, key],
    )?;
    Ok(())
}

use crate::Archive;
use sha2::{Digest, Sha256};
use vault_store::Priority;

struct ServingHead {
    owner: String,
    input: String,
    source: String,
    manifest: Vec<u8>,
}

impl Archive {
    pub(crate) fn resume_reader_finalization(&self) -> Result<()> {
        if self.reader_finalization_pending.load(std::sync::atomic::Ordering::Acquire) {
            match self.finalize_reader_work() {
                Ok(()) | Err(Error::ReaderRebuildRequired) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    pub(crate) fn finalize_reader_work(&self) -> Result<()> {
        use std::sync::{TryLockError, atomic::Ordering};
        let token = CancelToken::new();
        let _lease = self.jobs.track(&token).map_err(|error| match error {
            Error::Store(vault_store::Error::Closed) => Error::Cancelled,
            other => other,
        })?;
        let _gate = loop {
            token.check().map_err(|_| Error::Cancelled)?;
            match self.publication_gate.try_lock() {
                Ok(guard) => break guard,
                Err(TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_millis(5))
                }
                Err(TryLockError::Poisoned(_)) => {
                    return Err(Error::Integrity("publication lock".into()));
                }
            }
        };
        let parked = self.recover_reader_heads(&token)?;
        self.reader_finalization_pending.store(false, Ordering::Release);
        if parked {
            return Err(Error::ReaderRebuildRequired);
        }
        Ok(())
    }
    /// The caller serializes source publication. Large metadata sets are staged across
    /// writer transactions while the preceding immutable head is still readable.
    pub(crate) fn flush_reader_heads(&self) -> Result<()> {
        self.flush_reader_heads_controlled(&CancelToken::new())
    }
    pub(crate) fn flush_reader_heads_controlled(&self, cancel: &CancelToken) -> Result<()> {
        // Each parked decision is already durable and its own header reports it.
        // A producer publishing another source can continue processing its files.
        self.recover_reader_heads(cancel).map(|_| ())
    }
    pub(crate) fn recover_reader_heads(&self, cancel: &CancelToken) -> Result<bool> {
        cancel.check().map_err(|_| Error::Cancelled)?;
        let mut parked = false;
        let pending = self.store.read(DB, |tx| {
            tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_dirty)", (), |r| {
                Ok(r.get::<i64>(0)? != 0)
            })
        })?;
        if !pending {
            return Ok(false);
        }
        self.store.write(DB, Priority::Batch, |tx| {
            tx.execute(
                "INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT publication FROM ah_reader_build",
                (),
            )?;
            tx.execute("DELETE FROM ah_reader_build", ())?;
            tx.execute("DELETE FROM ah_reader_verified", ())?;
            Ok(())
        })?;
        let mut cursor = String::new();
        loop {
            let after = cursor.clone();
            let dirty=self.store.read(DB,move |tx|tx.query(
                "SELECT conversation,ticket FROM ah_reader_dirty WHERE conversation>?1 ORDER BY conversation LIMIT 64",
                [after],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?))))?;
            if dirty.is_empty() {
                break;
            }
            for (id, ticket) in dirty {
                cancel.check().map_err(|_| Error::Cancelled)?;
                cursor.clone_from(&id);
                parked |= self.prepare_decision(id, ticket, cancel)?;
            }
        }
        cancel.check().map_err(|_| Error::Cancelled)?;
        #[cfg(test)]
        super::tests::before_head_switch(self)?;
        parked |= self.revalidate_noops(cancel)?;
        self.store.write(DB, Priority::Batch, move |tx| {
            let stale=tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_build b LEFT JOIN
                ah_reader_dirty d ON d.conversation=b.conversation WHERE d.ticket IS NULL OR d.ticket<>b.ticket OR b.input_digest IS NULL)",
                (),|r|Ok(r.get::<i64>(0)?!=0))?;
            if stale{return Err(operation(Error::StaleCursor))}
            let invalid_noop=tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_verified n
                LEFT JOIN ah_reader_dirty d ON d.conversation=n.conversation
                LEFT JOIN ah_reader_head h ON h.conversation=n.conversation
                WHERE d.ticket IS NOT n.ticket OR (n.publication<>'' AND h.publication IS NOT n.publication))",(),|r|Ok(r.get::<i64>(0)?!=0))?;
            let missed=tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_dirty d WHERE NOT EXISTS(
                SELECT 1 FROM ah_reader_verified n WHERE n.conversation=d.conversation AND n.ticket=d.ticket)
                AND NOT EXISTS(SELECT 1 FROM ah_reader_build b WHERE b.conversation=d.conversation AND b.ticket=d.ticket))",(),|r|Ok(r.get::<i64>(0)?!=0))?;
            if invalid_noop || missed{return Err(operation(Error::StaleCursor))}
            tx.execute("DELETE FROM ah_reader_parked WHERE conversation IN (SELECT conversation FROM ah_reader_dirty)",())?;
            tx.execute("INSERT OR REPLACE INTO ah_reader_parked SELECT conversation,ticket FROM ah_reader_verified WHERE publication=''",())?;
            tx.execute("DELETE FROM ah_reader_dirty WHERE conversation IN (SELECT conversation FROM ah_reader_verified)",())?;
            tx.execute("DELETE FROM ah_reader_verified",())?;
            tx.execute("INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT h.publication FROM
                ah_reader_build b JOIN ah_reader_head h ON h.conversation=b.conversation
                WHERE b.conversation=h.conversation",())?;
            // This child-owned receipt also identifies old membership when a head row was lost.
            tx.execute("INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT w.publication
                FROM ah_reader_build b JOIN ah_reader_serving w ON w.conversation=b.conversation",())?;
            tx.execute("DELETE FROM ah_reader_serving WHERE conversation IN (SELECT conversation FROM ah_reader_build)",())?;
            tx.execute("DELETE FROM ah_reader_head WHERE conversation IN (SELECT b.conversation FROM
                ah_reader_build b JOIN ah_reader_head h ON h.conversation=b.conversation
                WHERE b.conversation=h.conversation)",())?;
            tx.execute("INSERT INTO ah_reader_head(conversation,publication,current_source,input_digest,parent,parent_status,awaiting_rebuild,manifest,members,stamp)
                SELECT b.conversation,b.publication,b.current_source,b.input_digest,
                json_extract(m.descriptor,'$.parent_id'), json_extract(m.descriptor,'$.parent_status'), EXISTS(SELECT 1 FROM ah_reader_member x WHERE x.publication=b.publication AND x.version=''),b.manifest,b.members,b.stamp
                FROM ah_reader_build b JOIN ah_reader_member m ON m.publication=b.publication AND m.source=b.current_source",())?;
            tx.execute("INSERT INTO ah_reader_serving SELECT h.conversation,h.publication,h.parent,h.stamp
                FROM ah_reader_build b JOIN ah_reader_head h ON h.publication=b.publication",())?;
            tx.execute("INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT b.publication FROM ah_reader_build b
                WHERE NOT EXISTS(SELECT 1 FROM ah_reader_head h WHERE h.publication=b.publication)",())?;
            tx.execute("DELETE FROM ah_reader_dirty WHERE conversation IN (SELECT conversation FROM ah_reader_build)",())?;
            tx.execute("DELETE FROM ah_reader_build",())?;
            Ok(())
        })?;
        self.jobs.request_collection();
        Ok(parked)
    }

    fn prepare_decision(&self, id: String, ticket: String, cancel: &CancelToken) -> Result<bool> {
        if let Err(error) = self.stage_conversation(&id, &ticket, cancel) {
            if !can_defer_reader_work(&error) {
                return Err(error);
            }
            eprintln!("Conversation {id} reader publication deferred: {error}");
            // Remove private work and preserve the failed ticket separately. A new
            // publication or explicit rebuild can retry it from retained evidence.
            self.store.write(DB,Priority::Batch,move|tx|{
                        tx.execute("INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT publication FROM ah_reader_build WHERE conversation=?1",[id.as_str()])?;
                        tx.execute("DELETE FROM ah_reader_build WHERE conversation=?1",[id.as_str()])?;
                        tx.execute("INSERT INTO ah_reader_verified VALUES(?1,?2,'')",params![id,ticket])?;
                        Ok(())
                    })?;
            return Ok(true);
        }
        Ok(false)
    }

    fn prior_head(&self, id: &str) -> Result<Option<ServingHead>> {
        let id = id.to_owned();
        Ok(self.store.read(DB,move|tx|{
            match manifest::valid_head(tx,&id){
                Ok(true)=>{},Ok(false)=>return Ok(None),
                Err(e) if e.is_row_data_error()=>return Ok(None),Err(e)=>return Err(e)
            }
            tx.query_one(
                "SELECT publication,input_digest,current_source,manifest FROM ah_reader_head WHERE conversation=?1",
                [id],|r|Ok(ServingHead{owner:r.get(0)?,input:r.get(1)?,source:r.get(2)?,manifest:r.get(3)?})
            ).optional()
        })?)
    }
    fn valid_serving_rows(
        &self,
        id: &str,
        owner: &str,
        digest: &str,
        current: &str,
        seal: &[u8],
        cancel: &CancelToken,
    ) -> Result<bool> {
        let validity = (|| -> Result<bool> {
            Ok(self.serves_current_versions(owner)?
                && self.publication_manifest(id, owner, digest, Some(current), cancel)? == seal)
        })();
        match validity {
            Err(error) if local_reader_damage(&error) => Ok(false),
            other => other,
        }
    }

    fn revalidate_noops(&self, cancel: &CancelToken) -> Result<bool> {
        let mut cursor = String::new();
        let mut parked = false;
        loop {
            let after = cursor.clone();
            let decisions=self.store.read(DB,move|tx|tx.query(
                "SELECT conversation,ticket,publication FROM ah_reader_verified WHERE conversation>?1 AND publication<>'' ORDER BY conversation LIMIT 64",
                [after],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?))))?;
            if decisions.is_empty() {
                break;
            }
            for (id, ticket, owner) in decisions {
                cursor.clone_from(&id);
                cancel.check().map_err(|_| Error::Cancelled)?;
                if let Some(head) = self.prior_head(&id)?
                    && head.owner == owner
                    && self.valid_serving_rows(
                        &id,
                        &owner,
                        &head.input,
                        &head.source,
                        &head.manifest,
                        cancel,
                    )?
                {
                    continue;
                }
                let key = id.clone();
                self.store.write(DB, Priority::Batch, move |tx| {
                    tx.execute("DELETE FROM ah_reader_verified WHERE conversation=?1", [key])
                })?;
                parked |= self.prepare_decision(id, ticket, cancel)?;
            }
        }
        Ok(parked)
    }
    fn stage_conversation(&self, id: &str, ticket: &str, cancel: &CancelToken) -> Result<()> {
        let mut resolver = membership::Resolver::default();
        let prior = self.prior_head(id)?;
        if let Some(ServingHead { owner, input: prior, source: current, manifest: seal }) = prior {
            let (elected, digest) = self.prepare_membership(id, None, &mut resolver, cancel)?;
            if elected.as_deref() == Some(&current)
                && prior == digest
                && self.valid_serving_rows(id, &owner, &digest, &current, &seal, cancel)?
            {
                let (id, ticket) = (id.to_owned(), ticket.to_owned());
                self.store.write(DB, Priority::Batch, move |tx| {
                    tx.execute(
                        "INSERT INTO ah_reader_verified VALUES(?1,?2,?3)",
                        params![id, ticket, owner],
                    )
                })?;
                return Ok(());
            }
        }
        let (conversation, expected) = (id.to_owned(), ticket.to_owned());
        let token = self.store.write(DB, Priority::Batch, move |tx| {
            let token: String =
                tx.query_one("SELECT lower(hex(randomblob(16)))", (), |r| Ok(r.get(0)?))?;
            tx.execute(
                "INSERT INTO ah_reader_build(conversation,publication,ticket) VALUES(?2,?1,?3)",
                params![token.clone(), conversation, expected],
            )?;
            Ok(token)
        })?;
        let (current, digest) = self.prepare_membership(id, Some(&token), &mut resolver, cancel)?;
        let manifest =
            self.publication_manifest(id, &token, &digest, current.as_deref(), cancel)?;
        let id = id.to_owned();
        self.store.write(DB,Priority::Batch,move|tx|{
                    let members=tx.query_one("SELECT count(*) FROM ah_reader_member WHERE publication=?1",[token.as_str()],|r|Ok(r.get::<i64>(0)?))?;
                    let stamp=if let Some(current)=current.as_ref(){
                        let (parent,status,awaiting)=tx.query_one("SELECT json_extract(descriptor,'$.parent_id'),json_extract(descriptor,'$.parent_status'),
                            EXISTS(SELECT 1 FROM ah_reader_member x WHERE x.publication=?1 AND x.version='') FROM ah_reader_member
                            WHERE publication=?1 AND source=?2",params![token.clone(),current.as_str()],|r|Ok((r.get::<Option<String>>(0)?,r.get::<String>(1)?,r.get::<i64>(2)?)))?;
                        Some(manifest::stamp(&token,"head",&(&id,current,&digest,parent,status,awaiting,members,&manifest))?)
                    }else{None};
                    tx.execute("UPDATE ah_reader_build SET input_digest=?1,current_source=?2,manifest=?3,members=?4,stamp=?5 WHERE publication=?6",
                        params![digest,current,manifest,members,stamp,token])
                })?;
        Ok(())
    }
    fn prepare_membership(
        &self,
        id: &str,
        owner: Option<&str>,
        resolver: &mut membership::Resolver,
        cancel: &CancelToken,
    ) -> Result<(Option<String>, String)> {
        let mut cursor = String::new();
        let mut current = None;
        let mut digest = Sha256::new();
        loop {
            cancel.check().map_err(|_| Error::Cancelled)?;
            let facts = self.batch_facts(id, &cursor)?;
            let (id, after) = (id.to_owned(), cursor.clone());
            let mut cache = std::mem::take(resolver);
            let (batch, cache) = self.store.read(DB, move |tx| {
                let batch = membership::prepare(tx, &id, &mut cache, after, &facts)?;
                Ok((batch, cache))
            })?;
            *resolver = cache;
            let Some(last) = batch.last() else { break };
            cursor = last.to_owned();
            if let Some(elected) = &batch.elected {
                current = Some(elected.clone());
            }
            digest.update(batch.digest);
            if let Some(owner) = owner {
                let owner = owner.to_owned();
                batch.write(self, &owner, cancel)?;
            }
        }
        Ok((current, hex::encode(digest.finalize())))
    }
}

pub(crate) fn expire_versions(
    tx: &Session<'_>,
    source: &str,
    selected: i64,
) -> vault_store::Result<u64> {
    let exists = tx.query_one(
        "SELECT EXISTS(SELECT 1 FROM ah_reader_version WHERE source=?1 AND state<>'retired')",
        [source],
        |r| Ok(r.get::<i64>(0)? != 0),
    )?;
    if !exists {
        return Ok(0);
    }
    let changed=tx.execute("UPDATE ah_reader_version SET state='retired' WHERE source=?1 AND revision<>?2 AND state<>'retired'",params![source,selected])?;
    if changed > 0 {
        crate::read_model::publication::refresh(tx, source, None)?
    }
    Ok(changed)
}
