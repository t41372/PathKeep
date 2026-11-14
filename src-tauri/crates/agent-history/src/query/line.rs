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
            .read(Db::Archive, move |tx| Ok(snapshot(tx, &id)))??
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
    let (complete, resolved) = append(tx, &current, None, &mut HashSet::new(), &mut segments)?;
    let mut attachments = 0;
    for segment in &segments {
        attachments += attachment_count(tx, segment)?;
    }
    Ok(Some(LineStatus { segments, complete, resolved, attachments }))
}

pub(super) struct LineStatus {
    pub segments: Vec<Segment>,
    pub complete: bool,
    pub resolved: bool,
    pub attachments: u64,
}

fn append(
    tx: &Session<'_>,
    source: &str,
    end: Option<u64>,
    visiting: &mut HashSet<String>,
    segments: &mut Vec<Segment>,
) -> Result<(bool, bool)> {
    if !visiting.insert(source.into()) {
        return Err(Error::Integrity("native lineage cycle".into()));
    }
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
    let mut complete = own && fresh && (coverage || !parsed);
    let mut resolved = true;
    if relation.base_native_id.is_some() {
        if let Some(ancestor) = base(tx, source, &relation)? {
            let physical = tx.query_one(
                "SELECT v.own_ordinal FROM ah_lineage_validation v
                 JOIN ah_sessions s ON s.revision_id=v.revision_id
                 WHERE s.id=?1 AND v.end_ordinal=?2 AND v.byte_len=?3 AND v.valid=1",
                params![
                    ancestor.clone(),
                    relation.base_end_ordinal.unwrap() as i64,
                    relation.base_end_byte.unwrap() as i64
                ],
                |row| Ok(row.get::<i64>(0)? as u64),
            )?;
            let status = append(tx, &ancestor, Some(physical), visiting, segments)?;
            complete &= status.0;
            resolved &= status.1;
        } else {
            complete = false;
            resolved = false;
        }
    }
    segments.push(Segment { source: source.into(), projection, end });
    visiting.remove(source);
    Ok((complete, resolved))
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

pub(super) fn base(
    tx: &Session<'_>,
    source: &str,
    relation: &SourceRelation,
) -> Result<Option<String>> {
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
        let digest = tx
            .query_one(
                "SELECT v.prefix_digest FROM ah_lineage_validation v
             JOIN ah_sessions s ON s.revision_id=v.revision_id
             WHERE s.id=?1 AND v.end_ordinal=?2 AND v.byte_len=?3 AND v.valid=1",
                params![source.clone(), end, length],
                |row| Ok(row.get::<String>(0)?),
            )
            .optional()?;
        if let Some(digest) = digest {
            if expected.as_ref().is_some_and(|expected| expected != &digest) {
                return Ok(None);
            }
            expected = Some(digest);
            chosen.get_or_insert(source);
        }
    }
    Ok(chosen)
}
