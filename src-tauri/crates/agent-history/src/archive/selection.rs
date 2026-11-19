//! Responsibilities: maintain copy relations and choose current sources from native ordering.
//! Not responsible for: capture order or re-reading unchanged transcript bodies.
use crate::archive::ProjectionAttempt;
use crate::{Archive, Result, SessionMetadata, adapters::identity::ConversationKey};
use vault_store::{Db, OptionalRow, Session, params};

#[derive(Clone)]
struct Candidate {
    id: String,
    revision: i64,
    path: String,
    generation: u64,
    time: Option<i64>,
    source_time: Option<String>,
    copy: Option<String>,
    first_digest: String,
}

pub(super) struct Selection {
    copies: Vec<(String, Option<String>)>,
}

const COLUMNS: &str = "id,revision_id,relative_path,generation,updated_at,
    source_time,copy_of_source_id,first_record_digest";

fn candidate(row: &vault_store::Row) -> vault_store::Result<Candidate> {
    Ok(Candidate {
        id: row.get(0)?,
        revision: row.get(1)?,
        path: row.get(2)?,
        generation: row.get::<i64>(3)? as u64,
        time: row.get(4)?,
        source_time: row.get(5)?,
        copy: row.get(6)?,
        first_digest: row.get(7)?,
    })
}

pub(super) fn choose(
    archive: &Archive,
    conversation: &str,
    attempt: &ProjectionAttempt,
    key: &ConversationKey,
    metadata: &SessionMetadata,
) -> Result<Selection> {
    let source = archive.source(&attempt.source_id)?;
    let native = crate::adapters::adapter(source.harness.parse()?);
    let incoming = Candidate {
        id: attempt.source_id.clone(),
        revision: attempt.revision_id,
        path: source.relative_path.clone(),
        generation: key.generation,
        time: metadata.updated_at,
        source_time: native.source_time(&source.relative_path),
        copy: None,
        first_digest: key.first_record_digest.clone(),
    };
    let id = attempt.source_id.clone();
    let group = conversation.to_owned();
    let old = archive.store.read(Db::AgentHistoryReadModel, move |tx| {
        tx.query_one(
            &format!("SELECT {COLUMNS} FROM ah_sessions WHERE id=?1 AND conversation_id=?2"),
            params![id, group],
            candidate,
        )
        .optional()
    })?;
    if let Some(old) = old.filter(|member| member.copy.is_none()) {
        let before = archive.revision(old.revision)?;
        let after = archive.revision(incoming.revision)?;
        // Capture already proved this immutable prefix. Its copy descendants
        // remain prefixes too, so appending does not revisit the conversation.
        let extends = before.id == after.id
            || (before.byte_len <= after.byte_len
                && archive.store.read(Db::AgentHistoryEvidence, move |tx| {
                    tx.query_one(
                        "SELECT coalesce(a.manifest_revision_id, a.id)=
                            coalesce(b.manifest_revision_id, b.id) FROM ah_revisions a JOIN
                            ah_revisions b ON b.id=?2 WHERE a.id=?1",
                        params![before.id, after.id],
                        |row| Ok(row.get::<i64>(0)? != 0),
                    )
                })?);
        if extends {
            return Ok(Selection { copies: vec![(incoming.id, None)] });
        }
    }
    // A growing copy may follow a different branch even when its old bytes are a prefix.
    let mut candidates = load(archive, conversation)?;
    candidates.retain(|candidate| candidate.id != incoming.id);
    candidates.push(incoming);
    select(archive, candidates)
}

pub(super) fn remaining(
    archive: &Archive,
    conversation: &str,
    exclude: &str,
) -> Result<Option<Selection>> {
    let mut candidates = load(archive, conversation)?;
    candidates.retain(|candidate| candidate.id != exclude);
    if candidates.is_empty() { Ok(None) } else { select(archive, candidates).map(Some) }
}

fn load(archive: &Archive, id: &str) -> Result<Vec<Candidate>> {
    let id = id.to_owned();
    Ok(archive.store.read(Db::AgentHistoryReadModel, move |tx| {
        tx.query(
            &format!("SELECT {COLUMNS} FROM ah_sessions WHERE conversation_id=?1 ORDER BY id"),
            [id],
            candidate,
        )
    })?)
}

fn rank(candidate: &Candidate) -> (u64, Option<i64>, Option<&str>, &str) {
    (candidate.generation, candidate.time, candidate.source_time.as_deref(), &candidate.id)
}

fn select(archive: &Archive, mut candidates: Vec<Candidate>) -> Result<Selection> {
    use std::collections::HashMap;
    candidates.sort_by(|a, b| rank(b).cmp(&rank(a)));
    let mut identities = HashMap::new();
    for batch in candidates.chunks(128) {
        let ids = batch
            .iter()
            .map(|candidate| vault_store::Value::Integer(candidate.revision))
            .collect::<Vec<_>>();
        let placeholders = vec!["?"; ids.len()].join(",");
        let rows = archive.store.read(Db::AgentHistoryEvidence, move |tx| {
            tx.query(
                &format!(
                    "SELECT id,digest,byte_len FROM ah_revisions
                    WHERE id IN ({placeholders})"
                ),
                ids,
                |row| Ok((row.get::<i64>(0)?, (row.get::<String>(1)?, row.get::<i64>(2)?))),
            )
        })?;
        identities.extend(rows);
    }
    let mut equivalent = HashMap::new();
    let mut links: HashMap<String, Option<String>> = HashMap::new();
    let mut roots: Vec<&Candidate> = Vec::new();
    for first in &candidates {
        let identity = identities.get(&first.revision).ok_or_else(|| {
            crate::Error::Integrity(format!(
                "source {} refers to missing revision {}",
                first.id, first.revision
            ))
        })?;
        if let Some(representative) = equivalent.get(identity) {
            links.insert(first.id.clone(), Some(String::clone(representative)));
            continue;
        }
        equivalent.insert(identity.clone(), first.id.clone());
        let mut copy = None;
        let mut contained = Vec::new();
        for second in &roots {
            if first.first_digest != second.first_digest {
                continue;
            }
            let (prefix, _) = archive.sources_prefix(
                first.revision,
                &first.path,
                second.revision,
                &second.path,
            )?;
            if prefix {
                copy = Some(second.id.clone());
                break;
            }
            if archive.sources_prefix(second.revision, &second.path, first.revision, &first.path)?.0
            {
                links.insert(second.id.clone(), Some(first.id.clone()));
                contained.push(second.id.clone());
            }
        }
        roots.retain(|candidate| !contained.contains(&candidate.id));
        if copy.is_none() {
            roots.push(first);
        }
        links.insert(first.id.clone(), copy);
    }
    let mut copies = Vec::new();
    for candidate in candidates {
        let mut target = links[&candidate.id].clone();
        while let Some(next) = target.as_ref().and_then(|id| links.get(id)).and_then(Clone::clone) {
            target = Some(next);
        }
        copies.push((candidate.id, target));
    }
    Ok(Selection { copies })
}

impl Selection {
    pub(super) fn publish(
        self,
        tx: &Session<'_>,
        conversation: &str,
        incoming: Option<&str>,
    ) -> vault_store::Result<()> {
        for (id, copy) in self.copies {
            if let Some(target) = &copy {
                tx.execute(
                    "UPDATE ah_sessions SET copy_of_source_id=?1
                     WHERE copy_of_source_id=?2 AND id<>?1",
                    params![target.clone(), id.clone()],
                )?;
            }
            tx.execute(
                "UPDATE ah_sessions SET copy_of_source_id=?1 WHERE id=?2",
                params![copy, id],
            )?;
        }
        reelect(tx, conversation, incoming)
    }
}

pub(super) fn reelect(
    tx: &Session<'_>,
    conversation: &str,
    incoming: Option<&str>,
) -> vault_store::Result<()> {
    let previous = tx
        .query_one(
            "SELECT id FROM ah_sessions WHERE conversation_id=?1 AND conversation_current=1",
            [conversation],
            |row| Ok(row.get::<String>(0)?),
        )
        .optional()?;
    let next = tx
        .query_one(
            "SELECT id FROM ah_sessions WHERE conversation_id=?1 AND copy_of_source_id IS NULL
                ORDER BY generation DESC, updated_at DESC, source_time DESC, id DESC LIMIT 1",
            [conversation],
            |row| Ok(row.get::<String>(0)?),
        )
        .optional()?;
    if previous == next {
        return Ok(());
    }
    tx.execute(
        "UPDATE ah_sessions SET conversation_current=0 WHERE conversation_id=?1 AND
            conversation_current=1",
        [conversation],
    )?;
    if let Some(next) = &next {
        tx.execute("UPDATE ah_sessions SET conversation_current=1 WHERE id=?1", [next.as_str()])?;
    }
    for source in previous.into_iter().chain(next) {
        if incoming == Some(source.as_str()) {
            continue;
        }
        let (revision, projection) = tx.query_one(
            "SELECT revision_id, projection_id FROM ah_sessions WHERE id=?1",
            [source.as_str()],
            |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?)),
        )?;
        super::projection::publish_outbox(tx, &source, revision, projection)?;
    }
    Ok(())
}
