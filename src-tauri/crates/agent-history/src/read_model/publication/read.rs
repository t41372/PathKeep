//! Reads a bounded display window from the selected publication. Exact bytes always belong to evidence.
use super::*;
use crate::{Archive, archive::RevisionInfo, query::page_limit};
use serde_json::Value;
use vault_store::{OptionalRow, params};

impl DisplayRow {
    pub(super) fn interpret(
        source: &str,
        loc: &RecordLocator,
        ctx: &RecordContext,
        value: Option<Value>,
    ) -> Result<Self> {
        let interpreted = value.map(|v| ctx.interpret(loc.ordinal, v)).transpose()?;
        Ok(Self::from_event(source, loc, interpreted, pages::TEXT_BYTES))
    }
    pub(in crate::read_model) fn from_event(
        source: &str,
        loc: &RecordLocator,
        event: Option<InterpretedRecord>,
        budget: usize,
    ) -> Self {
        let (mut id, mut parent, mut kind, mut role, timestamp, mut parts, boundary) = event
            .map(|r| {
                let e = r.event;
                (e.native_id, e.parent_id, e.kind, e.role, e.timestamp, r.parts, r.boundary)
            })
            .unwrap_or_default();
        let mut left = budget.min(pages::TEXT_BYTES);
        let mut truncated = false;
        // IDs are exact anchors. An oversized identifier stays in the outline and raw
        // record. Keep a separate text allowance so metadata cannot displace the reply.
        let mut metadata = left;
        if kind.len() > metadata {
            kind = String::new();
            truncated = true;
        }
        metadata -= kind.len();
        for field in [&mut role, &mut id, &mut parent] {
            let size = field.as_ref().map_or(0, String::len);
            if size > metadata {
                *field = None;
                truncated = true;
            } else {
                metadata -= size;
            }
        }
        for part in &mut parts {
            if part.text.len() > left {
                let mut end = left;
                while !part.text.is_char_boundary(end) {
                    end -= 1
                }
                part.text.truncate(end);
                part.text.shrink_to_fit();
                truncated = true;
            }
            left -= part.text.len();
        }
        parts.retain(|p| !p.text.is_empty());
        parts.shrink_to_fit();
        Self {
            source_id: source.into(),
            ordinal: loc.ordinal,
            native_id: id,
            parent_id: parent,
            kind,
            role,
            timestamp,
            parts,
            record_bytes: loc.length,
            truncated,
            tokens_before: boundary.as_ref().and_then(|b| b.tokens_before),
            boundary,
        }
    }
}

impl Archive {
    /// Names the immutable membership and line that subsequent page requests must see.
    pub fn reader_header(&self, id: &str) -> Result<ReaderPublication> {
        let _guard = self.jobs.enter(None)?;
        self.resume_reader_finalization()?;
        let id = id.to_owned();
        let conversation = id.clone();
        let published = self
            .store
            .read(DB, move |tx| {
                if tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_reader_parked WHERE conversation=?1)",[id.as_str()],|r|Ok(r.get::<i64>(0)?!=0))?{
                    return Err(crate::query::operation(Error::ReaderRebuildRequired))
                }
                if let Some((owner,source))=tx.query_one("SELECT publication,current_source FROM ah_reader_head WHERE conversation=?1",[id.as_str()],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?))).optional()?{
                    if !manifest::valid_head(tx,&id)? {return Err(crate::query::operation(Error::ReaderRebuildRequired))}
                    let member=manifest::member(tx,&owner,&source)?.ok_or_else(||crate::query::operation(Error::ReaderRebuildRequired))?;
                    if member.1.is_empty(){return Err(crate::query::operation(Error::ReaderRebuildRequired))}
                    manifest::verify_version(tx,&member.1)?;
                }
                tx.query_one(
                    "SELECT
                 h.conversation,h.publication,h.current_source,m.records,m.complete AND NOT h.awaiting_rebuild,m.attachments,h.input_digest,CASE WHEN h.awaiting_rebuild THEN 'awaiting_rebuild' ELSE m.reason END,
                 coalesce(v.revision,CAST(json_extract(m.descriptor,'$.revision') AS INTEGER)),coalesce(v.digest,json_extract(m.descriptor,'$.digest')),m.visible,CASE WHEN m.version='' THEN ?2 ELSE coalesce(v.codec,0) END,h.parent,h.parent_status
                 FROM ah_reader_head h JOIN ah_reader_member m ON m.publication=h.publication AND
                 m.source=h.current_source LEFT JOIN ah_reader_version v ON v.token=m.version WHERE
                 h.conversation=?1",
                    params![id,CODEC],
                    |r| {
                        Ok((
                            ReaderPublication {
                                conversation_id: r.get(0)?,
                                publication: PublicationId(r.get(1)?),
                                current_source: r.get(2)?, parent_id:r.get(12)?, parent_status:match r.get::<String>(13)?.as_str(){
                                "resolved"=>ParentStatus::Resolved,"unresolved"=>ParentStatus::Unresolved,"none"=>ParentStatus::None,
                                _=>return Err(crate::query::operation(Error::ReaderRebuildRequired))
                                },
                                physical_records: r.get::<i64>(3)? as u64,
                                complete: r.get::<i64>(4)? != 0,
                                attachments_not_kept: r.get::<i64>(5)? as u64,
                                input_digest: r.get(6)?,
                                coverage_reason: r.get(7)?,
                                visible_records: r.get::<i64>(10)? as u64,
                            },
                            r.get::<i64>(8)?,
                            r.get::<String>(9)?, r.get::<i64>(11)?,
                        ))
                    },
                )
                .optional()
            }).map_err(derived_read_error)?;
        let Some((head, revision, digest, codec)) = published else {
            let old = self.store.read(DB, move |tx| {
                tx.query_one(
                    "SELECT revision_id FROM ah_sessions
                WHERE conversation_id=?1 AND conversation_current=1",
                    [conversation],
                    |r| Ok(r.get::<i64>(0)?),
                )
                .optional()
            })?;
            if let Some(id) = old
                && self.revision(id)?.bytes_retained
            {
                return Err(Error::ReaderRebuildRequired);
            }
            return Err(Error::ConversationNotFound);
        };
        if codec != CODEC {
            return Err(Error::ReaderRebuildRequired);
        }
        self.facts(vec![revision])?
            .get(&revision)
            .ok_or(Error::StaleCursor)?
            .check(&head.current_source, &digest)?;
        Ok(head)
    }
    pub(super) fn at_head<T>(
        &self,
        h: &ReaderPublication,
        read: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let _pin = self.jobs.pin_publication()?;
        self.check_head(h)?;
        let result = read();
        // A missing immutable record must not turn a superseded handle into a missing branch.
        if result.is_err() {
            let conversation = h.conversation_id.clone();
            let current = self.store.read(DB, move |tx| {
                tx.query_one(
                    "SELECT publication FROM ah_reader_head WHERE conversation=?1",
                    [conversation],
                    |r| Ok(r.get::<String>(0)?),
                )
                .optional()
            })?;
            if current.as_ref() != Some(&h.publication.0) {
                return Err(Error::StaleCursor);
            }
        } else {
            self.check_head(h)?;
        }
        result.map_err(|error| match error {
            Error::Store(store) if store.is_row_data_error() => Error::ReaderRebuildRequired,
            other => other,
        })
    }
    pub(super) fn check_head(&self, h: &ReaderPublication) -> Result<()> {
        match self.reader_header(&h.conversation_id) {
            Ok(current) if current.publication == h.publication => Ok(()),
            Ok(_) | Err(Error::ConversationNotFound) => Err(Error::StaleCursor),
            Err(e) => Err(e),
        }
    }
    /// Pages physical members, including copies and earlier rollouts.
    pub fn reader_sources(
        &self,
        h: &ReaderPublication,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SourceMember>> {
        let _guard = self.jobs.enter(None)?;
        self.at_head(h, || {
            let limit = page_limit(limit)?;
            let token = h.publication.0.clone();
            let after = after.unwrap_or_default().to_owned();
            let members = self.store.read(DB, move |tx| {
                let valid=tx.query_one("SELECT members=(SELECT count(*) FROM ah_reader_member WHERE publication=?1)
                    FROM ah_reader_head WHERE publication=?1",[token.as_str()],|r|Ok(r.get::<i64>(0)?!=0))?;
                if !valid{return Err(crate::query::operation(Error::ReaderRebuildRequired))}
                tx.query(&format!("SELECT {} FROM ah_reader_member WHERE publication=?1 AND source>?2 ORDER BY source LIMIT ?3",manifest::MEMBER),
                    params![token.clone(),after,limit],|r|{
                    let member=manifest::decode_member(r,&token)?;
                    serde_json::from_str::<SourceMember>(&member.2).map_err(crate::query::operation)
                })
            })?;
            let evidence = self.facts(members.iter().map(|m| m.revision).collect())?;
            for m in &members {
                evidence
                    .get(&m.revision)
                    .ok_or(Error::StaleCursor)?
                    .check(&m.source_id, &m.digest)?;
            }
            Ok(members)
        })
    }

    /// Reads the supplied publication and its proven source prefixes. None selects its current
    /// branch. Successful rows use only the revisions selected at the start of the read.
    /// Retention can expire a source during delivery, and a publication switch can stale this handle.
    pub fn reader_window(
        &self,
        h: &ReaderPublication,
        branch: Option<&str>,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<DisplayRow>> {
        let _guard = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            job.checkpoint(ReadPhase::Reading, 0)?;
            window_limit(first, count)?;
            let runs = self.line_runs(h, branch, first, count, false)?;
            let mut window = Window::default();
            for (owner, start, len) in runs {
                let lo = first.saturating_sub(start);
                let hi = (first + count as u64 - start).min(len);
                window.physical(&owner, lo..hi);
            }
            let rows = window.read(self, job)?;
            Ok(rows)
        })
    }
    pub(super) fn line_runs(
        &self,
        h: &ReaderPublication,
        branch: Option<&str>,
        first: u64,
        count: usize,
        visible: bool,
    ) -> Result<Vec<(String, u64, u64)>> {
        let h_id = h.conversation_id.clone();
        let branch = branch.unwrap_or(&h.current_source).to_owned();
        let token = h.publication.0.clone();
        let (rank, length) =
            if visible { ("first_visible", "visible") } else { ("first_rank", "records") };
        self.store
            .read(DB, move |tx| {
                let Some(member)=manifest::member(tx,&token,&branch)? else{
                    let expected=tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_sessions WHERE conversation_id=?1 AND id=?2)",
                        params![h_id,branch],|r|Ok(r.get::<i64>(0)?!=0))?;
                    if expected{return Err(crate::query::operation(Error::ReaderRebuildRequired))}
                    return Ok(None)
                };
                if member.1.is_empty(){return Err(crate::query::operation(Error::ReaderRebuildRequired))}
                let total=if visible{member.7}else{member.3};
                if first>=total as u64{return Ok(Some(Vec::new()))}
                let end=(first+count as u64).min(total as u64);
                let invalid=||crate::query::operation(Error::ReaderRebuildRequired);
                let start=tx.query_one(&format!("SELECT {rank} FROM ah_reader_line WHERE publication=?1 AND branch=?2 AND {rank}<=?3 AND {length}>0 ORDER BY {rank} DESC LIMIT 1"),
                    params![token.clone(),branch.clone(),first as i64],|r|Ok(r.get::<i64>(0)?)).optional()?.ok_or_else(invalid)?;
                let mut cursor=first;
                let runs=tx.query(&format!("SELECT branch,first_rank,version,source,records,first_visible,visible,stamp FROM ah_reader_line
                    WHERE publication=?1 AND branch=?2 AND {rank}>=?3 AND {rank}<?4 AND {length}>0 ORDER BY {rank}"),
                    params![token.clone(),branch,start,end as i64],|r|{
                    let line=manifest::decode_line(r,&token)?;
                    let (base,n)=if visible{(line.5,line.6)}else{(line.1,line.4)};
                    let base=u64::try_from(base).map_err(|_|invalid())?;let n=u64::try_from(n).map_err(|_|invalid())?;
                    let next=base.checked_add(n).ok_or_else(invalid)?;
                    if base.max(first)!=cursor || next<=cursor{return Err(invalid())}
                    cursor=next.min(end);Ok((line.2,base,n))
                })?;
                if cursor!=end{return Err(invalid())}
                Ok(Some(runs))
            })?
            .ok_or_else(|| Error::NotFound("conversation branch".into()))
    }
    /// Physical metadata at current-line or earlier-branch ranks, including gaps.
    pub fn reader_outline(
        &self,
        h: &ReaderPublication,
        branch: Option<&str>,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<(SourceId, OutlineEntry)>> {
        let _guard = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            window_limit(first, count)?;
            let mut rows = Vec::new();
            for (owner, start, len) in self.line_runs(h, branch, first, count, false)? {
                let lo = first.saturating_sub(start);
                let hi = (first + count as u64 - start).min(len);
                let (info, _) = self.version_info(&owner)?;
                for entry in self.source_outline(&owner, &info, lo, (hi - lo) as usize, job)? {
                    rows.push((SourceId(info.source_id.clone()), entry));
                }
            }
            Ok(rows)
        })
    }
    /// Resolves child links by the same root/path and ambiguity rules as the native session view.
    pub fn reader_children(
        &self,
        h: &ReaderPublication,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>> {
        let _guard = self.jobs.enter(None)?;
        self.at_head(h,||{
            let limit=page_limit(limit)?;
            let parent=h.conversation_id.clone();
            let after=after.unwrap_or_default().to_owned();
            Ok(self.store.read(DB,move|tx|{
                let rows=tx.query("SELECT conversation FROM ah_reader_head WHERE parent=?1 AND conversation>?2 ORDER BY conversation LIMIT ?3",
                    params![parent.clone(),after.clone(),limit],|r|Ok(r.get::<String>(0)?))?;
                let receipts=tx.query("SELECT conversation FROM ah_reader_serving WHERE parent=?1 AND conversation>?2 ORDER BY conversation LIMIT ?3",
                    params![parent,after,limit],|r|Ok(r.get::<String>(0)?))?;
                if rows!=receipts{return Err(crate::query::operation(Error::ReaderRebuildRequired))}
                Ok(rows)
            })?)
        })
    }
    pub(super) fn version_info(&self, owner: &str) -> Result<(RevisionInfo, bool)> {
        let key = owner.to_owned();
        let (source, id, digest, selective, codec) = self
            .store
            .read(DB, move |tx| {
                manifest::verify_version(tx, &key)?;
                tx.query_one(
                    "SELECT source,revision,digest,selective AND renderer=?2,codec FROM
                 ah_reader_version WHERE token=?1",
                    params![key, RENDERER],
                    |r| {
                        Ok((
                            r.get::<String>(0)?,
                            r.get::<i64>(1)?,
                            r.get::<String>(2)?,
                            r.get::<i64>(3)? != 0,
                            r.get::<i64>(4)?,
                        ))
                    },
                )
                .optional()
            })?
            .ok_or(Error::StaleCursor)?;
        if codec != CODEC {
            return Err(Error::ReaderRebuildRequired);
        }
        let info = self.revision(id)?;
        if info.digest != digest || info.source_id != source {
            return Err(Error::StaleCursor);
        }
        if !info.bytes_retained {
            return Err(Error::NotFound("revision bytes expired by retention".into()));
        }
        Ok((info, selective))
    }
    pub(super) fn source_outline(
        &self,
        owner: &str,
        info: &RevisionInfo,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<Entry>> {
        self.selected_outline(
            owner,
            info,
            &(first..first + count as u64).collect::<Vec<_>>(),
            &mut job.clone(),
        )
    }
}
pub(super) fn window_limit(first: u64, count: usize) -> Result<()> {
    if !(1..=200).contains(&count) || first > i64::MAX as u64 - count as u64 {
        return Err(Error::InvalidInput("invalid reader window".into()));
    }
    Ok(())
}
pub(super) fn range<T: serde::de::DeserializeOwned>(
    a: &Archive,
    owner: &str,
    family: i64,
    first: u64,
    count: usize,
    job: &JobControl,
) -> Result<Vec<T>> {
    let mut rows = Vec::new();
    while rows.len() < count {
        job.cancel.check().map_err(|_| Error::Cancelled)?;
        let rank = first + rows.len() as u64;
        let (start, page) = pages::load::<T>(a, owner, family, rank, job)?;
        rows.extend(page.into_iter().skip((rank - start) as usize).take(count - rows.len()));
    }
    Ok(rows)
}
