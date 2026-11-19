//! Resolves one bounded batch of expected members on read connections. Only prepared
//! membership, line and dependency records cross into the staging writer.
use super::*;
use crate::query::{line, operation};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use vault_store::{Session, params};

type MemberRow = (String, String, String, i64, i64, i64, Option<String>, i64);
type LineRow = (String, i64, String, String, i64, i64, i64);
#[derive(Default)]
pub(super) struct Batch {
    after: Option<String>,
    members: Vec<MemberRow>,
    lines: Vec<LineRow>,
    dependencies: BTreeSet<String>,
    pub elected: Option<String>,
    pub digest: [u8; 32],
}
impl Batch {
    pub fn last(&self) -> Option<&str> {
        self.after.as_deref()
    }
    pub fn write(self, archive: &crate::Archive, owner: &str, cancel: &CancelToken) -> Result<()> {
        let publication = owner.to_owned();
        let members = self.members;
        archive.store.write(DB, vault_store::Priority::Batch, move |tx| {
            let mut insert =
                tx.prepare("INSERT INTO ah_reader_member VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)")?;
            for (id, version, descriptor, total, complete, attachments, reason, visible) in members
            {
                insert.execute(params![
                    publication.clone(),
                    id,
                    version,
                    descriptor,
                    total,
                    complete,
                    attachments,
                    reason,
                    visible
                ])?;
            }
            Ok(())
        })?;
        let mut lines = self.lines.into_iter();
        let mut dependencies = self.dependencies.into_iter();
        loop {
            cancel.check().map_err(|_| Error::Cancelled)?;
            let rows: Vec<_> = lines.by_ref().take(128).collect();
            let keys: Vec<_> = dependencies.by_ref().take(128).collect();
            if rows.is_empty() && keys.is_empty() {
                return Ok(());
            }
            let publication = owner.to_owned();
            archive.store.write(DB, vault_store::Priority::Batch, move |tx| {
                let mut insert =
                    tx.prepare("INSERT INTO ah_reader_line VALUES(?1,?2,?3,?4,?5,?6,?7,?8)")?;
                for (id, start, version, source, count, before, visible) in rows {
                    insert.execute(params![
                        publication.clone(),
                        id,
                        start,
                        version,
                        source,
                        count,
                        before,
                        visible
                    ])?;
                }
                for key in keys {
                    catalog::dependency(tx, &publication, key)?;
                }
                Ok(())
            })?;
        }
    }
}
use super::catalog::{rollout_key, source_key};
pub(super) fn prepare(
    tx: &Session<'_>,
    id: &str,
    resolver: &mut line::Resolver,
    mut after: String,
    facts: &std::collections::HashMap<i64, evidence::Facts>,
) -> vault_store::Result<Batch> {
    let mut digest = Sha256::new();
    let mut batch = Batch::default();
    let parent = crate::query::parents::native_link("parent_session_id", "parent_path");
    let fork = crate::query::parents::native_link("forked_from_native_id", "fork_path");
    let members = tx.query(
        &format!(
            "SELECT
                 s.id,coalesce(v.token,''),s.revision_id,v.digest,v.byte_len,s.harness,s.source_root,s.relative_path,s.native_id,s.origin_mark,s.parser_version,
                 s.generation,s.copy_of_source_id,s.relation_json,coalesce(v.records,0),coalesce(v.selective,0),s.conversation_current,{parent},{fork},s.rollout_native_id,s.source_time,s.raw_head_current,coalesce(v.max_frame,0),v.format_version,s.parent_session_id
                 FROM ah_sessions s LEFT JOIN ah_reader_version v ON v.attempt=s.projection_id AND
                 v.state='published' AND v.codec={CODEC} WHERE s.conversation_id=?1 AND s.id>?2 ORDER BY s.id LIMIT 8"
        ),
        params![id, after.as_str()],
        |r| {
            let relation = serde_json::from_str(&r.get::<String>(13)?).map_err(operation)?;
            let id = r.get::<i64>(2)?;
            let source: String = r.get(0)?;

            let facts = facts.get(&id).ok_or_else(|| operation(Error::StaleCursor))?;
            let stored:Option<String>=r.get(3)?;
            let awaiting_rebuild=stored.is_none();
            let digest=stored.unwrap_or_else(||facts.digest.clone());
            facts.verify_identity(&source,&digest).map_err(operation)?;
            Ok((
                SourceMember { awaiting_rebuild,
                    parent_status:if r.get::<Option<String>>(17)?.is_some(){ParentStatus::Resolved}
                        else if r.get::<Option<String>>(24)?.is_some(){ParentStatus::Unresolved}else{ParentStatus::None},
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
                    digest,
                    bytes:facts.bytes,
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
                facts.retained,
            ))
        },
    )?;
    if members.is_empty() {
        return Ok(batch);
    }
    for (mut member, version, elected, retained) in members {
        batch.after = Some(member.source_id.clone());
        // Expiry removes this source from reader membership. Only retained sources can await rebuild.
        if !retained {
            continue;
        }
        after.clone_from(&member.source_id);
        for native in [&member.relation.parent_native_id, &member.relation.forked_from_native_id]
            .into_iter()
            .flatten()
        {
            batch.dependencies.insert(format!("native/{}/{native}", member.harness));
        }
        if elected {
            batch.elected = Some(after.clone());
        }
        digest.update([u8::from(elected)]);
        let status = resolver.describe(tx, after.clone()).map_err(operation)?;
        let mut total = 0i64;
        let mut visible_total = 0i64;
        let mut complete = status.complete && !member.awaiting_rebuild;
        let mut unresolved_pi = false;
        for segment in status.segments {
            batch.dependencies.insert(source_key(&segment.source));
            let (owner, records, base, harness,revision_digest,parser,generation) = tx.query_one(
                "SELECT v.token,v.records,s.lineage_base_native_id,s.harness,v.digest,s.parser_version,v.repair_generation FROM ah_sessions s
                 LEFT JOIN ah_reader_version v ON v.attempt=s.projection_id AND
                 v.state='published' AND v.codec=?2 WHERE s.id=?1",
                params![segment.source.as_str(),CODEC],
                |r| {
                    Ok((
                        r.get::<Option<String>>(0)?,
                        r.get::<Option<i64>>(1)?,
                        r.get::<Option<String>>(2)?,
                        r.get::<String>(3)?,r.get::<Option<String>>(4)?,r.get::<String>(5)?, r.get::<Option<i64>>(6)?.unwrap_or_default(),
                    ))
                },
            )?;
            if let Some(base) = base {
                batch.dependencies.insert(rollout_key(&harness, &base));
            }
            let (Some(owner), Some(records)) = (owner, records) else {
                complete = false;
                continue;
            };
            let count = segment.end.map_or(records, |n| records.min(n as i64));
            digest.update(segment.projection.to_le_bytes());
            digest.update(generation.to_le_bytes());
            if count > 0 {
                let visible = match leaves::selection(tx, &owner)? {
                    Some(selection) => {
                        unresolved_pi |= selection.boundary.is_some();
                        complete &= selection.boundary.is_none();
                        selection.visible as i64
                    }
                    None => visible::prefix(tx, &owner, count)?,
                };
                digest.update(segment.source.as_bytes());
                digest.update(revision_digest.unwrap_or_default().as_bytes());
                digest.update(parser.as_bytes());
                digest.update(count.to_le_bytes());
                batch.lines.push((
                    after.clone(),
                    total,
                    owner,
                    segment.source,
                    count,
                    visible_total,
                    visible,
                ));
                visible_total += visible;
                total = total
                    .checked_add(count)
                    .ok_or_else(|| operation(Error::LimitExceeded("line rank".into())))?;
            }
        }
        let reason = match status.issue {
            Some(line::LineageIssue::Cycle) => Some("lineage_cycle"),
            Some(line::LineageIssue::Unresolved) => Some("unresolved_base"),
            None if member.awaiting_rebuild => Some("awaiting_rebuild"),
            None if unresolved_pi => Some("unresolved_parent"),
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
        batch.members.push((
            after.clone(),
            version,
            descriptor,
            total,
            i64::from(complete),
            status.attachments as i64,
            reason.map(str::to_owned),
            visible_total,
        ));
    }
    batch.digest = digest.finalize().into();
    Ok(batch)
}
