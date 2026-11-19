//! Reads a bounded display window from the selected publication. Exact bytes always belong to evidence.
use super::*;
use crate::{Archive, ParseContext, archive::RevisionInfo, query::page_limit};
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
        let (mut id, mut parent, mut kind, mut role, timestamp, mut parts, tokens_before) = event
            .map(|r| {
                let e = r.event;
                let tokens = e.payload.get("tokensBefore").and_then(Value::as_u64);
                (e.native_id, e.parent_id, e.kind, e.role, e.timestamp, r.parts, tokens)
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
            tokens_before,
        }
    }
}

impl Archive {
    /// Names the immutable membership and line that subsequent page requests must see.
    pub fn reader_header(&self, id: &str) -> Result<ReaderPublication> {
        let _guard = self.jobs.enter(None)?;
        let id = id.to_owned();
        let conversation = id.clone();
        let published = self
            .store
            .read(DB, move |tx| {
                tx.query_one(
                    "SELECT
                 h.conversation,h.publication,h.current_source,m.records,m.complete,m.attachments,h.input_digest,m.reason,v.revision,v.digest,m.visible,v.codec
                 FROM ah_reader_head h JOIN ah_reader_member m ON m.publication=h.publication AND
                 m.source=h.current_source JOIN ah_reader_version v ON v.token=m.version WHERE
                 h.conversation=?1",
                    [id],
                    |r| {
                        Ok((
                            ReaderPublication {
                                conversation_id: r.get(0)?,
                                publication: PublicationId(r.get(1)?),
                                current_source: r.get(2)?,
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
            })?;
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
        let limit = page_limit(limit)?;
        self.check_head(h)?;
        let token = h.publication.0.clone();
        let after = after.unwrap_or_default().to_owned();
        let members = self.store.read(DB, move |tx| {
            tx.query(
                "SELECT descriptor FROM ah_reader_member WHERE publication=?1 AND source>?2 ORDER
                 BY source LIMIT ?3",
                params![token, after, limit],
                |r| Ok(r.get::<String>(0)?),
            )
        })?;
        let members = members
            .into_iter()
            .map(|s| serde_json::from_str(&s))
            .collect::<std::result::Result<Vec<SourceMember>, _>>()?;
        let evidence = self.facts(members.iter().map(|m| m.revision).collect())?;
        for m in &members {
            evidence.get(&m.revision).ok_or(Error::StaleCursor)?.check(&m.source_id, &m.digest)?;
        }
        self.check_head(h)?;
        Ok(members)
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
        job.checkpoint(ReadPhase::Reading, 0)?;
        window_limit(first, count)?;
        self.check_head(h)?;
        let runs = self.line_runs(h, branch, first, count, false)?;
        let mut rows = Vec::new();
        for (owner, start, len) in runs {
            let lo = first.saturating_sub(start);
            let hi = (first + count as u64 - start).min(len);
            rows.extend(self.display_source(&owner, lo, (hi - lo) as usize, job)?);
        }
        self.check_head(h)?;
        Ok(rows)
    }
    pub(super) fn line_runs(
        &self,
        h: &ReaderPublication,
        branch: Option<&str>,
        first: u64,
        count: usize,
        visible: bool,
    ) -> Result<Vec<(String, u64, u64)>> {
        let branch = branch.unwrap_or(&h.current_source).to_owned();
        let token = h.publication.0.clone();
        let (rank, length) =
            if visible { ("first_visible", "visible") } else { ("first_rank", "records") };
        self.store
            .read(DB, move |tx| {
                let total = tx.query_one(&format!("SELECT {length} FROM ah_reader_member WHERE publication=?1 AND source=?2"), params![token.clone(), branch.clone()], |r| Ok(r.get::<i64>(0)?)).optional()?;
                let Some(total) = total else { return Ok(None) };
                if first >= total as u64 {
                    return Ok(Some(Vec::new()));
                }
                let start: i64 = tx.query_one(
                    &format!(
                        "SELECT {rank} FROM ah_reader_line WHERE publication=?1 AND branch=?2 AND
                 {rank}<=?3 AND {length}>0 ORDER BY {rank} DESC LIMIT 1"
                    ),
                    params![token.clone(), branch.clone(), first as i64],
                    |r| Ok(r.get(0)?),
                )?;
                tx.query(
                    &format!(
                        "SELECT version,{rank},{length} FROM ah_reader_line WHERE publication=?1 AND
                 branch=?2 AND {rank}>=?3 AND {rank}<?4 AND {length}>0 ORDER BY {rank}"
                    ),
                    params![token, branch, start, (first + count as u64) as i64],
                    |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)? as u64, r.get::<i64>(2)? as u64)),
                )
                .map(Some)
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
        window_limit(first, count)?;
        self.check_head(h)?;
        let mut rows = Vec::new();
        for (owner, start, len) in self.line_runs(h, branch, first, count, false)? {
            let lo = first.saturating_sub(start);
            let hi = (first + count as u64 - start).min(len);
            let (info, _) = self.version_info(&owner)?;
            for entry in self.source_outline(&owner, &info, lo, (hi - lo) as usize, job)? {
                rows.push((SourceId(info.source_id.clone()), entry));
            }
        }
        self.check_head(h)?;
        Ok(rows)
    }
    /// Resolves child links by the same root/path and ambiguity rules as the native session view.
    pub fn reader_children(
        &self,
        h: &ReaderPublication,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>> {
        let limit = page_limit(limit)?;
        self.check_head(h)?;
        let id = h.publication.0.clone();
        let after = after.unwrap_or_default().to_string();
        let rows = self.store.read(DB, move |tx| {
            tx.query(
                "SELECT child FROM ah_reader_edge WHERE publication=?1 AND child>?2 ORDER BY
                 child LIMIT ?3",
                params![id, after, limit],
                |r| Ok(r.get::<String>(0)?),
            )
        })?;
        self.check_head(h)?;
        Ok(rows)
    }
    pub(super) fn display_source(
        &self,
        owner: &str,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<DisplayRow>> {
        let (info, selective) = self.version_info(owner)?;
        let source = &info.source_id;
        if selective {
            match range::<DisplayRow>(self, owner, RENDERED, first, count, job) {
                Ok(rows)
                    if rows.iter().enumerate().all(|(i, r)| {
                        r.ordinal.0 == first + i as u64 && &r.source_id == source
                    }) =>
                {
                    return Ok(rows);
                }
                Ok(_) | Err(Error::Integrity(_)) => {}
                Err(error) => return Err(error),
            }
        }
        self.evidence_display(owner, &info, first, count, job)
    }
    pub(super) fn version_info(&self, owner: &str) -> Result<(RevisionInfo, bool)> {
        let key = owner.to_owned();
        let (source, id, digest, selective, codec) = self
            .store
            .read(DB, move |tx| {
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
    fn evidence_display(
        &self,
        owner: &str,
        info: &RevisionInfo,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<DisplayRow>> {
        let entries = self
            .source_outline(owner, info, first, count, job)?
            .into_iter()
            .map(|entry| (entry.locator, entry.context))
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        self.read_window_admitted(info, &entries, pages::TEXT_BYTES, job, &mut |r| {
            rows.push(r.preview.unwrap_or_else(|| {
                let mut display = DisplayRow::from_event(
                    &r.source_id.0,
                    &r.record,
                    r.interpretation,
                    pages::TEXT_BYTES,
                );
                display.truncated |= r.raw.next.is_some();
                display
            }));
            Ok(())
        })?;
        Ok(rows)
    }
    pub(super) fn source_outline(
        &self,
        owner: &str,
        info: &RevisionInfo,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<Entry>> {
        match range::<Entry>(self, owner, OUTLINE, first, count, job) {
            Ok(mut rows)
                if rows
                    .iter()
                    .enumerate()
                    .all(|(i, e)| e.locator.ordinal.0 == first + i as u64) =>
            {
                // The authenticated first entry supplies the transport for this source version.
                let compressed = rows.first().is_some_and(|e| e.locator.frame.is_some());
                let owner = Arc::new(LocatorOwner::new(info, compressed));
                for entry in &mut rows {
                    entry.locator.owner = Some(owner.clone());
                }
                return Ok(rows);
            }
            Ok(_) | Err(Error::Integrity(_)) => {}
            Err(error) => return Err(error),
        }
        let key = owner.to_owned();
        let (record_limit, decoded_limit) = self.store.read(DB, move |tx| {
            tx.query_one(
                "SELECT record_limit,decoded_limit FROM ah_reader_version WHERE token=?1",
                [key],
                |r| Ok((r.get::<i64>(0)? as usize, r.get::<i64>(1)? as u64)),
            )
        })?;
        let mut rows = Vec::new();
        let mut previous = None;
        let mut run_start = 0;
        let context = ParseContext { max_record_bytes: record_limit, ..Default::default() };
        let mut budget = job.clone();
        budget.max_decoded_bytes = budget.max_decoded_bytes.min(decoded_limit);
        let result =
            self.scan_revision_admitted(info.id, &context, &budget, &mut |loc, ctx, value| {
                let entry = Entry::interpret(loc, ctx, value, &mut previous, &mut run_start)?;
                if entry.locator.ordinal.0 >= first {
                    rows.push(entry)
                }
                if rows.len() == count {
                    return Err(Error::Cancelled);
                }
                Ok(())
            });
        match result {
            Err(Error::Cancelled) if rows.len() == count => {
                job.cancel.check().map_err(|_| Error::Cancelled)?;
                Ok(rows)
            }
            Ok(_) => Ok(rows),
            Err(e) => Err(e),
        }
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
