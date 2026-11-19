//! Switches source membership, line runs and their dependencies with the legacy source election.
//! Payload decoding and page construction are outside the publication transaction.
use super::*;
use crate::{
    archive::ProjectionAttempt,
    query::{line, operation},
};
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
        "SELECT m.descriptor FROM ah_reader_member m JOIN ah_reader_head h ON
                 h.publication=m.publication WHERE m.source=?1",
        [source],
        |r| Ok(r.get::<String>(0)?),
    )?;
    for raw in previous {
        let member: SourceMember = serde_json::from_str(&raw).map_err(operation)?;
        keys.push(format!("native/{}/{}", member.harness, member.native_id));
        if let Some(rollout) = member.rollout_id {
            keys.push(rollout_key(&member.harness, &rollout))
        }
    }
    for key in keys {
        tx.execute(
            "INSERT OR REPLACE INTO ah_reader_dirty
            SELECT h.conversation,lower(hex(randomblob(16))) FROM ah_reader_dependency d
            JOIN ah_reader_head h ON h.publication=d.publication WHERE d.target=?1",
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
        params![key, publication],
    )?;
    Ok(())
}

use crate::Archive;
use sha2::{Digest, Sha256};
use vault_store::Priority;

impl Archive {
    /// The caller serializes source publication. Large metadata sets are staged across
    /// writer transactions while the preceding immutable head is still readable.
    pub(crate) fn flush_reader_heads(&self) -> Result<()> {
        self.flush_reader_heads_controlled(&CancelToken::new())
    }
    pub(crate) fn flush_reader_heads_controlled(&self, cancel: &CancelToken) -> Result<()> {
        cancel.check().map_err(|_| Error::Cancelled)?;
        let pending = self.store.read(DB, |tx| {
            tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_dirty)", (), |r| {
                Ok(r.get::<i64>(0)? != 0)
            })
        })?;
        if !pending {
            return Ok(());
        }
        self.store.write(DB, Priority::Batch, |tx| {
            tx.execute(
                "INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT publication FROM ah_reader_build",
                (),
            )?;
            tx.execute("DELETE FROM ah_reader_build", ())?;
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
                let mut resolver = line::Resolver::default();
                let conversation = id.clone();
                // A different backing version needs a pointer switch even when offline repair
                // reconstructs byte-identical logical inputs. Most appends take this fast path.
                let prior=self.store.read(DB,move|tx|tx.query_one("SELECT h.input_digest FROM ah_reader_head h WHERE h.conversation=?1 AND NOT EXISTS(
                    SELECT 1 FROM ah_sessions s LEFT JOIN ah_reader_version v ON v.attempt=s.projection_id AND v.state='published'
                    LEFT JOIN ah_reader_member m ON m.publication=h.publication AND m.source=s.id
                    WHERE s.conversation_id=h.conversation AND (m.source IS NULL OR m.version<>coalesce(v.token,'')))
                    AND NOT EXISTS(SELECT 1 FROM ah_reader_line l JOIN ah_reader_version v ON v.token=l.version
                    WHERE l.publication=h.publication AND v.state<>'published')",[conversation],|r|Ok(r.get::<String>(0)?)).optional())?;
                if let Some(prior) = prior {
                    let (_, digest) = self.prepare_membership(&id, None, &mut resolver, cancel)?;
                    if prior == digest {
                        let (key, expected) = (id.clone(), ticket.clone());
                        self.store.write(DB, Priority::Batch, move |tx| {
                            if tx.execute(
                                "DELETE FROM ah_reader_dirty WHERE conversation=?1 AND ticket=?2",
                                params![key, expected],
                            )? != 1
                            {
                                return Err(operation(Error::StaleCursor));
                            }
                            Ok(())
                        })?;
                        continue;
                    }
                }
                let (conversation, expected) = (id.clone(), ticket.clone());
                let token = self.store.write(DB, Priority::Batch, move |tx| {
                    let token: String =
                        tx.query_one("SELECT lower(hex(randomblob(16)))", (), |r| Ok(r.get(0)?))?;
                    tx.execute(
                        "INSERT INTO ah_reader_build VALUES(?1,?2,?3,NULL,NULL)",
                        params![token.clone(), conversation, expected],
                    )?;
                    Ok(token)
                })?;
                let (current, digest) =
                    self.prepare_membership(&id, Some(&token), &mut resolver, cancel)?;
                self.store.write(DB,Priority::Batch,move|tx|{
                tx.execute("UPDATE ah_reader_build SET input_digest=?1,current_source=?2 WHERE publication=?3",
                    params![digest,current,token])?;Ok(())
            })?;
            }
        }
        #[cfg(test)]
        super::tests::before_head_switch(self)?;
        self.store.write(DB, Priority::Batch, move |tx| {
            let stale=tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_build b LEFT JOIN
                ah_reader_dirty d ON d.conversation=b.conversation WHERE d.ticket IS NULL OR d.ticket<>b.ticket OR b.input_digest IS NULL)",
                (),|r|Ok(r.get::<i64>(0)?!=0))?;
            if stale{return Err(operation(Error::StaleCursor))}
            // Recheck the staged digest in the switch transaction as well.
            tx.execute("INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT b.publication FROM ah_reader_build b JOIN ah_reader_head h ON h.conversation=b.conversation AND h.input_digest=b.input_digest",())?;
            tx.execute("DELETE FROM ah_reader_dirty WHERE conversation IN (SELECT conversation FROM ah_reader_build WHERE publication IN (SELECT publication FROM ah_reader_retired))",())?;
            tx.execute("DELETE FROM ah_reader_build WHERE publication IN (SELECT publication FROM ah_reader_retired)",())?;
            tx.execute("INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT h.publication FROM
                ah_reader_build b JOIN ah_reader_head h ON h.conversation=b.conversation
                WHERE b.conversation=h.conversation",())?;
            tx.execute("DELETE FROM ah_reader_head WHERE conversation IN (SELECT b.conversation FROM
                ah_reader_build b JOIN ah_reader_head h ON h.conversation=b.conversation
                WHERE b.conversation=h.conversation)",())?;
            tx.execute("INSERT INTO ah_reader_head(conversation,publication,current_source,input_digest,parent,parent_status,awaiting_rebuild)
                SELECT b.conversation,b.publication,b.current_source,b.input_digest,
                json_extract(m.descriptor,'$.parent_id'), json_extract(m.descriptor,'$.parent_status'), EXISTS(SELECT 1 FROM ah_reader_member x WHERE x.publication=b.publication AND x.version='')
                FROM ah_reader_build b JOIN ah_reader_member m ON m.publication=b.publication AND m.source=b.current_source",())?;
            tx.execute("INSERT OR IGNORE INTO ah_reader_retired(publication) SELECT b.publication FROM ah_reader_build b
                WHERE NOT EXISTS(SELECT 1 FROM ah_reader_head h WHERE h.publication=b.publication)",())?;
            tx.execute("DELETE FROM ah_reader_dirty WHERE conversation IN (SELECT conversation FROM ah_reader_build)",())?;
            tx.execute("DELETE FROM ah_reader_build",())?;
            Ok(())
        })?;
        self.jobs.request_collection();
        Ok(())
    }

    fn prepare_membership(
        &self,
        id: &str,
        owner: Option<&str>,
        resolver: &mut line::Resolver,
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
