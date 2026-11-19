//! Responsibilities: resolve an entire native line inside one committed hot-store snapshot.
//! Not responsible for: raw file reads or invalidating unrelated conversations' cursors.
use crate::{Archive, Error, Result, adapters::identity::SourceRelation};
use serde::Serialize;
use std::collections::HashSet;
use vault_store::{Db, OptionalRow, Session, params};

#[derive(Clone, Serialize)]
pub(crate) struct Segment {
    pub source: String,
    pub projection: i64,
    pub end: Option<u64>,
}

impl Archive {
    pub(crate) fn line_stamp(&self, conversation: &str) -> Result<String> {
        let id = conversation.to_owned();
        let segments = self
            .store
            .read(Db::AgentHistoryReadModel, move |tx| Ok(snapshot(tx, &id)))??
            .ok_or_else(|| Error::NotFound(conversation.into()))?;
        super::cursor::scope(&segments)
    }
}

pub(super) fn snapshot(tx: &Session<'_>, conversation: &str) -> Result<Option<Vec<Segment>>> {
    Ok(describe(tx, conversation)?.map(|status| status.segments))
}

pub(super) fn describe(tx: &Session<'_>, conversation: &str) -> Result<Option<LineStatus>> {
    let current = tx
        .query_one(
            "SELECT id FROM ah_sessions WHERE conversation_id=?1 AND conversation_current=1",
            [conversation],
            |row| Ok(row.get::<String>(0)?),
        )
        .optional()?;
    let Some(current) = current else {
        return Ok(None);
    };
    let mut segments = Vec::new();
    let mut visited = HashSet::new();
    let mut source = current;
    let mut end = None;
    let mut complete = true;
    let mut attachments = 0;
    let issue = loop {
        if !visited.insert(source.clone()) {
            break Some(LineageIssue::Cycle);
        }
        let (segment, relation, covered) = read_segment(tx, &source, end)?;
        complete &= covered;
        attachments += attachment_count(tx, &segment)?;
        segments.push(segment);
        if relation.base_native_id.is_none() {
            break None;
        }
        let Some(predecessor) = base(tx, &source, &relation)? else {
            break Some(LineageIssue::Unresolved);
        };
        source = predecessor.source;
        end = Some(predecessor.own_end);
    };
    segments.reverse();
    Ok(Some(LineStatus { segments, complete: complete && issue.is_none(), issue, attachments }))
}

pub(super) enum LineageIssue {
    Cycle,
    Unresolved,
}

/// Fingerprint the ordered published records, including inherited segments, when
/// salvage had to use a different physical source. Stream one event at a time.
pub(crate) fn content_digest(
    tx: &Session<'_>,
    id: &str,
    cancel: &vault_store::CancelToken,
) -> vault_store::Result<Option<[u8; 32]>> {
    use sha2::{Digest, Sha256};
    let line = describe(tx, id).map_err(|e| vault_store::Error::Operation(Box::new(e)))?;
    let Some(line) = line.filter(|line| line.issue.is_none()) else { return Ok(None) };
    let mut digest = Sha256::new();
    for segment in line.segments {
        tx.for_each(
            "SELECT native_id,parent_id,kind,role,CAST(timestamp AS TEXT),text,payload
             FROM ah_events WHERE session_id=?1 AND projection_id=?2 AND ordinal<?3 ORDER BY ordinal",
            params![segment.source, segment.projection, segment.end.map_or(i64::MAX, |n| n as i64)],
            |row| {
                cancel.check()?;
                for column in 0..7 {
                    let value = row.get::<Option<String>>(column)?;
                    match value {
                        Some(value) => {
                            digest.update((value.len() as u64).to_le_bytes());
                            digest.update(value.as_bytes());
                        }
                        None => digest.update(u64::MAX.to_le_bytes()),
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(Some(digest.finalize().into()))
}

pub(super) struct LineStatus {
    pub segments: Vec<Segment>,
    pub complete: bool,
    pub issue: Option<LineageIssue>,
    pub attachments: u64,
}

fn read_segment(
    tx: &Session<'_>,
    source: &str,
    end: Option<u64>,
) -> Result<(Segment, SourceRelation, bool)> {
    let (projection, relation, parsed, fresh, coverage) = tx.query_one(
        "SELECT projection_id,relation_json,complete,raw_head_current,
           coalesce((SELECT complete FROM ah_coverage WHERE source_id=s.id),1)
         FROM ah_sessions s WHERE id=?1",
        [source],
        |row| {
            Ok((
                row.get::<i64>(0)?,
                row.get::<String>(1)?,
                row.get::<i64>(2)? != 0,
                row.get::<i64>(3)? != 0,
                row.get::<i64>(4)? != 0,
            ))
        },
    )?;
    let relation: SourceRelation = serde_json::from_str(&relation)?;
    let mut own = parsed;
    if !own && let Some(end) = end {
        let (known, bad) = tx.query_one(
            "SELECT count(*),coalesce(sum(CASE WHEN ordinal IS NULL OR ordinal<?2
               THEN 1 ELSE 0 END),0) FROM ah_projection_gaps WHERE projection_id=?1",
            params![projection, end as i64],
            |row| Ok((row.get::<i64>(0)?, row.get::<i64>(1)?)),
        )?;
        own = known > 0 && bad == 0;
    }
    let segment = Segment { source: source.into(), projection, end };
    Ok((segment, relation, own && fresh && (coverage || !parsed)))
}

fn attachment_count(tx: &Session<'_>, segment: &Segment) -> Result<u64> {
    if let Some(end) = segment.end {
        Ok(tx.query_one(
            "SELECT coalesce((SELECT count FROM ah_attachment_counts c
               JOIN ah_sessions s ON s.attachment_generation=c.generation
               WHERE s.id=?1 AND c.ordinal<?2 ORDER BY c.ordinal DESC LIMIT 1),0)",
            params![segment.source.clone(), end as i64],
            |row| Ok(row.get::<i64>(0)? as u64),
        )?)
    } else {
        Ok(tx.query_one(
            "SELECT attachments_not_kept FROM ah_sessions WHERE id=?1",
            [segment.source.as_str()],
            |row| Ok(row.get::<i64>(0)? as u64),
        )?)
    }
}

pub(super) struct Predecessor {
    pub source: String,
    own_end: u64,
}

pub(super) fn base(
    tx: &Session<'_>,
    source: &str,
    relation: &SourceRelation,
) -> Result<Option<Predecessor>> {
    let Some(target) = &relation.base_native_id else {
        return Ok(None);
    };
    let candidates = tx.query(
        "SELECT p.id, p.conversation_id, CASE WHEN p.source_root=s.source_root THEN 0 ELSE 1
            END FROM ah_sessions p JOIN ah_sessions s ON s.id=?1 WHERE p.harness=s.harness AND
            p.rollout_native_id=?2 AND p.id<>s.id ORDER BY CASE WHEN p.source_root=s.source_root
            THEN 0 ELSE 1 END, p.id",
        params![source, target.clone()],
        |row| Ok((row.get::<String>(0)?, row.get::<String>(1)?, row.get::<i64>(2)?)),
    )?;
    let tier = candidates.first().map(|row| row.2);
    let candidates = candidates.into_iter().filter(|row| Some(row.2) == tier).collect::<Vec<_>>();
    let identities = candidates.iter().map(|row| &row.1).collect::<HashSet<_>>();
    if identities.len() != 1 {
        return Ok(None);
    }
    let (Some(end), Some(length)) = (relation.base_end_ordinal, relation.base_end_byte) else {
        return Ok(None);
    };
    let (Ok(end), Ok(length)) = (i64::try_from(end), i64::try_from(length)) else {
        return Ok(None);
    };
    let mut chosen = None;
    let mut expected = None;
    for (source, _, _) in candidates {
        let boundary = tx
            .query_one(
                "SELECT v.prefix_digest, v.own_ordinal FROM ah_lineage_validation v
             JOIN ah_sessions s ON s.revision_id=v.revision_id
             WHERE s.id=?1 AND v.end_ordinal=?2 AND v.byte_len=?3 AND v.valid=1",
                params![source.clone(), end, length],
                |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)? as u64)),
            )
            .optional()?;
        if let Some((digest, own_end)) = boundary {
            if expected.as_ref().is_some_and(|expected| expected != &digest) {
                return Ok(None);
            }
            expected = Some(digest);
            chosen.get_or_insert(Predecessor { source, own_end });
        }
    }
    Ok(chosen)
}
