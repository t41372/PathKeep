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

fn source_key(source: &str) -> String {
    format!("source/{source}")
}
fn rollout_key(harness: &str, rollout: &str) -> String {
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
        if let Some(parent) = member.parent_id {
            affected.insert(parent);
        }
    }
    let parent = crate::query::parents::native_link("parent_session_id", "parent_path");
    if let Some(parent) = tx
        .query_one(&format!("SELECT {parent} FROM ah_sessions s WHERE s.id=?1"), [source], |r| {
            Ok(r.get::<Option<String>>(0)?)
        })
        .optional()?
        .flatten()
    {
        affected.insert(parent);
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

fn dependency(tx: &Session<'_>, publication: &str, key: String) -> vault_store::Result<()> {
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
                "INSERT OR IGNORE INTO ah_reader_retired SELECT publication FROM ah_reader_build",
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
                cursor.clone_from(&id);
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
                let mut after = String::new();
                let mut current = None;
                let mut digest = Sha256::new();
                loop {
                    let (conversation, owner) = (id.clone(), token.clone());
                    let facts = self.batch_facts(&id, &after)?;
                    let (last, elected, data) =
                        self.store.write(DB, Priority::Batch, move |tx| {
                            stage_members(tx, &conversation, &owner, after, &facts)
                        })?;
                    let Some(last) = last else { break };
                    after = last;
                    if elected.is_some() {
                        current = elected
                    }
                    digest.update(data);
                }
                if current.is_some() {
                    let mut after = String::new();
                    loop {
                        let (owner, parent) = (token.clone(), id.clone());
                        let batch = self.store.write(DB, Priority::Batch, move |tx| {
                            stage_children(tx, &owner, &parent, &after)
                        })?;
                        let Some(last) = batch.last() else { break };
                        after = last.clone();
                        for child in batch {
                            digest.update((child.len() as u64).to_le_bytes());
                            digest.update(child);
                        }
                    }
                }
                let digest = hex::encode(digest.finalize());
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
            tx.execute("INSERT OR IGNORE INTO ah_reader_retired SELECT h.publication FROM
                ah_reader_build b JOIN ah_reader_head h ON h.conversation=b.conversation
                WHERE b.conversation=h.conversation",())?;
            tx.execute("DELETE FROM ah_reader_head WHERE conversation IN (SELECT b.conversation FROM
                ah_reader_build b JOIN ah_reader_head h ON h.conversation=b.conversation
                WHERE b.conversation=h.conversation)",())?;
            tx.execute("INSERT OR IGNORE INTO ah_reader_head SELECT conversation,publication,current_source,input_digest
                FROM ah_reader_build WHERE current_source IS NOT NULL",())?;
            tx.execute("INSERT OR IGNORE INTO ah_reader_retired SELECT b.publication FROM ah_reader_build b
                WHERE NOT EXISTS(SELECT 1 FROM ah_reader_head h WHERE h.publication=b.publication)",())?;
            tx.execute("DELETE FROM ah_reader_dirty WHERE conversation IN (SELECT conversation FROM ah_reader_build)",())?;
            tx.execute("DELETE FROM ah_reader_build",())?;
            Ok(())
        })?;
        Ok(())
    }
}

fn stage_members(
    tx: &Session<'_>,
    id: &str,
    publication: &str,
    mut after: String,
    facts: &std::collections::HashMap<i64, evidence::Facts>,
) -> vault_store::Result<(Option<String>, Option<String>, Vec<u8>)> {
    let mut digest = Sha256::new();
    let mut current = None;
    let parent = crate::query::parents::native_link("parent_session_id", "parent_path");
    let fork = crate::query::parents::native_link("forked_from_native_id", "fork_path");
    let members = tx.query(
        &format!(
            "SELECT
                 s.id,v.token,v.revision,v.digest,v.byte_len,s.harness,s.source_root,s.relative_path,s.native_id,s.origin_mark,s.parser_version,
                 s.generation,s.copy_of_source_id,s.relation_json,v.records,v.selective,s.conversation_current,{parent},{fork},s.rollout_native_id,s.source_time,s.raw_head_current,v.max_frame,v.format_version
                 FROM ah_sessions s JOIN ah_reader_version v ON v.attempt=s.projection_id AND
                 v.state='published' WHERE s.conversation_id=?1 AND s.id>?2 ORDER BY s.id LIMIT 8"
        ),
        params![id, after.as_str()],
        |r| {
            let relation = serde_json::from_str(&r.get::<String>(13)?).map_err(operation)?;
            let id = r.get::<i64>(2)?;
            let source: String = r.get(0)?;
            let digest: String = r.get(3)?;
            let facts = facts.get(&id).ok_or_else(|| operation(Error::StaleCursor))?;
            facts.check(&source, &digest).map_err(operation)?;
            Ok((
                SourceMember {
                    transport: Transport::from_path(&r.get::<String>(7)?),
                    max_frame_bytes: r.get::<i64>(22)? as u64,
                    format_version: r.get::<Option<String>>(23)?.map(|v| v.parse()).transpose().map_err(operation)?,
                    rollout_id: r.get(19)?,
                    source_time: r.get(20)?,
                    raw_head_current: r.get::<i64>(21)? != 0,
                    observation: facts.observation.clone(),
                    parent_id: r.get(17)?,
                    fork_id: r.get(18)?,
                    source_id: r.get(0)?,
                    revision: r.get(2)?,
                    digest: r.get(3)?,
                    bytes: r.get::<i64>(4)? as u64,
                    harness: r.get(5)?,
                    root: r.get(6)?,
                    relative_path: r.get(7)?,
                    native_id: r.get(8)?,
                    origin: r.get(9)?,
                    parser: r.get(10)?,
                    generation: r.get(11)?,
                    copy_of: r.get(12)?,
                    relation,
                    own_records: r.get::<i64>(14)? as u64,line_records:0,line_visible_records:0,resolved_base:None,
                    rendered: r.get::<i64>(15)? != 0,
                },
                r.get::<String>(1)?,
                r.get::<i64>(16)? != 0,
            ))
        },
    )?;
    if members.is_empty() {
        return Ok((None, None, Vec::new()));
    }
    for (mut member, version, elected) in members {
        after.clone_from(&member.source_id);
        for native in [&member.relation.parent_native_id, &member.relation.forked_from_native_id]
            .into_iter()
            .flatten()
        {
            dependency(tx, publication, format!("native/{}/{native}", member.harness))?;
        }
        if elected {
            current = Some(after.clone());
        }
        digest.update([u8::from(elected)]);
        let status = line::describe_source(tx, after.clone()).map_err(operation)?;
        let mut total = 0i64;
        let mut visible_total = 0i64;
        let mut complete = status.complete;
        for segment in status.segments {
            dependency(tx, publication, source_key(&segment.source))?;
            let (owner, records, base, harness,revision_digest,parser) = tx.query_one(
                "SELECT v.token,v.records,s.lineage_base_native_id,s.harness,v.digest,s.parser_version FROM ah_sessions s
                 LEFT JOIN ah_reader_version v ON v.attempt=s.projection_id AND
                 v.state='published' WHERE s.id=?1",
                [segment.source.as_str()],
                |r| {
                    Ok((
                        r.get::<Option<String>>(0)?,
                        r.get::<Option<i64>>(1)?,
                        r.get::<Option<String>>(2)?,
                        r.get::<String>(3)?,r.get::<Option<String>>(4)?,r.get::<String>(5)?,
                    ))
                },
            )?;
            if let Some(base) = base {
                dependency(tx, publication, rollout_key(&harness, &base))?
            }
            let (Some(owner), Some(records)) = (owner, records) else {
                complete = false;
                continue;
            };
            let count = segment.end.map_or(records, |n| records.min(n as i64));
            if count > 0 {
                let selected = tx
                    .query_one(
                        "SELECT selected_visible FROM ah_reader_leaf WHERE owner=?1",
                        [owner.as_str()],
                        |r| Ok(r.get::<Option<i64>>(0)?),
                    )
                    .optional()?
                    .flatten();
                let visible = match selected {
                    Some(n) => n,
                    None => visible::prefix(tx, &owner, count)?,
                };
                digest.update(segment.source.as_bytes());
                digest.update(revision_digest.unwrap_or_default().as_bytes());
                digest.update(parser.as_bytes());
                digest.update(count.to_le_bytes());
                tx.execute(
                    "INSERT INTO ah_reader_line VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        publication,
                        after.clone(),
                        total,
                        owner,
                        segment.source,
                        count,
                        visible_total,
                        visible
                    ],
                )?;
                visible_total += visible;
                total = total
                    .checked_add(count)
                    .ok_or_else(|| operation(Error::LimitExceeded("line rank".into())))?;
            }
        }
        let reason = match status.issue {
            Some(line::LineageIssue::Cycle) => Some("lineage_cycle"),
            Some(line::LineageIssue::Unresolved) => Some("unresolved_base"),
            None if member.parser.starts_with("agent-history-unreadable/") => {
                Some("unreadable_generation")
            }
            None if !complete => Some("partial_source"),
            None => None,
        };
        digest.update(reason.unwrap_or_default());
        member.line_records = total as u64;
        member.line_visible_records = visible_total as u64;
        member.resolved_base = status.resolved_base;
        let descriptor = serde_json::to_string(&member).map_err(operation)?;
        digest.update(CODEC.to_le_bytes());
        digest.update(RENDERER.to_le_bytes());
        digest.update(descriptor.as_bytes());
        digest.update([u8::from(complete)]);
        digest.update(status.attachments.to_le_bytes());
        tx.execute(
            "INSERT INTO ah_reader_member VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                publication,
                after.clone(),
                version,
                descriptor,
                total,
                i64::from(complete),
                status.attachments as i64,
                reason,
                visible_total
            ],
        )?;
    }
    Ok((Some(after), current, digest.finalize().to_vec()))
}

fn stage_children(
    tx: &Session<'_>,
    owner: &str,
    parent: &str,
    after: &str,
) -> vault_store::Result<Vec<String>> {
    let link = crate::query::parents::native_link("parent_session_id", "parent_path");
    let children = tx.query(
        &format!(
            "SELECT s.conversation_id FROM ah_sessions s WHERE s.conversation_current=1 AND
                 s.parent_session_id IN (SELECT native_id FROM ah_sessions WHERE
                 conversation_id=?1) AND {link}=?1 AND s.conversation_id>?2 ORDER BY
                 s.conversation_id LIMIT 128"
        ),
        params![parent, after],
        |r| Ok(r.get::<String>(0)?),
    )?;
    let mut write = tx.prepare("INSERT INTO ah_reader_edge VALUES(?1,?2)")?;
    for child in &children {
        write.execute(params![owner, child.as_str()])?;
    }
    Ok(children)
}
